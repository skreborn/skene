//! The terminal device: raw mode, the window size, the bytes written, and the input
//! read and decoded into events.

use std::{
  collections::VecDeque,
  fs,
  io::{self, BufWriter, IsTerminal as _, Write},
  os::fd::{AsFd, BorrowedFd, OwnedFd, RawFd},
  time::Duration,
};

use rustix::termios::{self, OptionalActions, Termios};

use crate::parse::{Event, Parser, WindowSize};

/// How many bytes one read takes at most.
const READ_SIZE: usize = 4096;

/// A descriptor of the terminal: the process's own, or `/dev/tty` opened where the
/// process's is not a terminal.
#[derive(Debug)]
enum Fd {
  Owned(OwnedFd),
  Borrowed(BorrowedFd<'static>),
}

impl AsFd for Fd {
  fn as_fd(&self) -> BorrowedFd<'_> {
    match self {
      Self::Owned(fd) => fd.as_fd(),
      Self::Borrowed(fd) => *fd,
    }
  }
}

impl Write for Fd {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    Ok(rustix::io::write(&*self, bytes)?)
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

/// The controlling terminal, opened afresh.
fn open_dev_tty() -> io::Result<Fd> {
  Ok(Fd::Owned(
    fs::OpenOptions::new()
      .read(true)
      .write(true)
      .open("/dev/tty")?
      .into(),
  ))
}

/// The terminal's output: standard output where it is a terminal, `/dev/tty` else.
fn open_output() -> io::Result<Fd> {
  if io::stdout().is_terminal() {
    Ok(Fd::Borrowed(rustix::stdio::stdout()))
  }
  else {
    open_dev_tty()
  }
}

/// The terminal's input: standard input where it is a terminal, `/dev/tty` else.
fn open_input() -> io::Result<Fd> {
  if io::stdin().is_terminal() {
    Ok(Fd::Borrowed(rustix::stdio::stdin()))
  }
  else {
    open_dev_tty()
  }
}

/// The terminal the loop runs in.
///
/// Writes buffer until [`flush`](Write::flush); [`wait`](Self::wait) reads what the
/// terminal sent and decodes it. The mode the terminal was opened in is restored
/// when the value drops.
pub(crate) struct Tty {
  input: Fd,
  output: BufWriter<Fd>,
  original: Termios,
  parser: Parser,
  events: VecDeque<Event>,
  /// The window size last reported, so a change reads as a resize.
  size: Option<WindowSize>,
  /// The dev launcher's control pipe, and the partial line read from it so far.
  control: Option<(BorrowedFd<'static>, Vec<u8>)>,
}

impl Tty {
  /// Opens the terminal.
  pub(crate) fn open() -> io::Result<Self> {
    let input = open_input()?;
    let output = open_output()?;
    let original = termios::tcgetattr(&output)?;
    let mut tty = Self {
      input,
      output: BufWriter::with_capacity(READ_SIZE, output),
      original,
      parser: Parser::default(),
      events: VecDeque::new(),
      size: None,
      control: None,
    };

    tty.size = tty.size();

    Ok(tty)
  }

  /// Listens on the dev launcher's control pipe at `fd`, which stays open for the
  /// process's whole life and across a swap into the next binary.
  pub(crate) fn set_control(&mut self, fd: RawFd) {
    // SAFETY: the launcher opened the descriptor for this process and never closes
    // it on its side while the app runs; it is inherited by the next binary too.
    let fd = unsafe { BorrowedFd::borrow_raw(fd) };

    self.control = Some((fd, Vec::new()));
  }

  /// Puts the terminal in raw mode: no echo, no line buffering, no signal keys.
  pub(crate) fn enter_raw_mode(&mut self) -> io::Result<()> {
    let mut raw = termios::tcgetattr(self.output.get_ref())?;

    raw.make_raw();
    termios::tcsetattr(self.output.get_ref(), OptionalActions::Flush, &raw)?;

    Ok(())
  }

  /// Restores the mode the terminal was opened in.
  pub(crate) fn restore_mode(&mut self) -> io::Result<()> {
    termios::tcsetattr(self.output.get_ref(), OptionalActions::Now, &self.original)?;

    Ok(())
  }

  /// The window's size, when the terminal reports one.
  pub(crate) fn size(&self) -> Option<WindowSize> {
    let size = termios::tcgetwinsize(self.output.get_ref()).ok()?;

    Some(WindowSize {
      columns: size.ws_col,
      rows: size.ws_row,
      pixel_width: Some(size.ws_xpixel).filter(|pixels| *pixels > 0),
      pixel_height: Some(size.ws_ypixel).filter(|pixels| *pixels > 0),
    })
  }

  /// Waits up to `timeout` for input, then takes every event the input so far
  /// decodes to, with a resize where the window's size changed since the last call;
  /// an error when the terminal is gone.
  pub(crate) fn wait(&mut self, timeout: Duration) -> io::Result<Vec<Event>> {
    if readable(&self.input, timeout)? {
      self.read_available()?;

      while readable(&self.input, Duration::ZERO)? {
        self.read_available()?;
      }
    }

    let mut events: Vec<Event> = self.events.drain(..).collect();

    while let Some(event) = self.parser.pop() {
      events.push(event);
    }

    self.read_control(&mut events)?;

    if let Some(size) = self.size()
      && self.size != Some(size)
    {
      self.size = Some(size);
      events.push(Event::Resized(size));
    }

    Ok(events)
  }

  /// Reads every line the launcher wrote on the control pipe into `events`; the
  /// pipe closing means the launcher is gone, and the app runs on.
  fn read_control(&mut self, events: &mut Vec<Event>) -> io::Result<()> {
    let Some((fd, pending)) = &mut self.control
    else {
      return Ok(());
    };

    while readable(fd, Duration::ZERO)? {
      let mut buffer = [0; READ_SIZE];
      let read = match rustix::io::read(*fd, &mut buffer) {
        Ok(read) => read,
        Err(rustix::io::Errno::INTR) => continue,
        Err(error) => return Err(error.into()),
      };

      if read == 0 {
        self.control = None;

        return Ok(());
      }

      pending.extend_from_slice(&buffer[..read]);

      while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
        let line: Vec<u8> = pending.drain(..=end).collect();

        events.push(Event::Control(
          String::from_utf8_lossy(&line[..line.len() - 1]).into_owned(),
        ));
      }
    }

    Ok(())
  }

  /// Reads what the terminal sent into the parser; the end of input is an error.
  fn read_available(&mut self) -> io::Result<()> {
    let mut buffer = [0; READ_SIZE];
    let read = loop {
      match rustix::io::read(&self.input, &mut buffer) {
        Ok(read) => break read,
        Err(rustix::io::Errno::INTR) => {}
        Err(error) => return Err(error.into()),
      }
    };

    if read == 0 {
      return Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "the terminal's input ended",
      ));
    }

    self.parser.feed(&buffer[..read], read == buffer.len());

    Ok(())
  }

  /// Installs a panic hook that writes `restore` to the terminal and returns it to
  /// the mode it was opened in before the panic prints.
  pub(crate) fn set_panic_hook(&self, restore: impl Fn(&mut dyn Write) + Send + Sync + 'static) {
    let original = self.original.clone();
    let hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
      if let Ok(mut output) = open_output() {
        restore(&mut output);

        let _ = output.flush();
        let _ = termios::tcsetattr(&output, OptionalActions::Now, &original);
      }

      hook(info);
    }));
  }
}

impl Write for Tty {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.output.write(bytes)
  }

  fn flush(&mut self) -> io::Result<()> {
    self.output.flush()
  }
}

impl Drop for Tty {
  fn drop(&mut self) {
    let _ = self.flush();
    let _ = self.restore_mode();
  }
}

/// Whether `fd` has input to read within `timeout`.
#[cfg(not(target_os = "macos"))]
fn readable(fd: &impl AsFd, timeout: Duration) -> io::Result<bool> {
  use rustix::event::{PollFd, PollFlags, Timespec, poll};

  let timespec = Timespec::try_from(timeout).map_err(io::Error::other)?;
  let mut fds = [PollFd::new(fd, PollFlags::IN)];

  match poll(&mut fds, Some(&timespec)) {
    Ok(_) => Ok(fds[0].revents().intersects(PollFlags::IN | PollFlags::HUP)),
    Err(rustix::io::Errno::INTR) => Ok(false),
    Err(error) => Err(error.into()),
  }
}

/// Whether `fd` has input to read within `timeout`; `select`, since `poll` does not
/// watch `/dev/tty` on macOS.
#[cfg(target_os = "macos")]
fn readable(fd: &impl AsFd, timeout: Duration) -> io::Result<bool> {
  use std::os::fd::AsRawFd as _;

  use rustix::event::{
    FdSetElement,
    FdSetIter,
    Timespec,
    fd_set_insert,
    fd_set_num_elements,
    select,
  };

  let timespec = Timespec::try_from(timeout).map_err(io::Error::other)?;
  let raw = fd.as_fd().as_raw_fd();
  let count = raw + 1;
  let mut set = vec![FdSetElement::default(); fd_set_num_elements(1, count)];

  fd_set_insert(&mut set, raw);

  // SAFETY: `set` holds `count` descriptors' worth of elements, as `select` requires.
  match unsafe { select(count, Some(&mut set), None, None, Some(&timespec)) } {
    Ok(_) => Ok(FdSetIter::new(&set).any(|ready| ready == raw)),
    Err(rustix::io::Errno::INTR) => Ok(false),
    Err(error) => Err(error.into()),
  }
}
