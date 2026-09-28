//! The terminal device: raw mode, the window size, the bytes written, the input read
//! and decoded into events, and the process signals that concern the terminal.

use std::{
  collections::VecDeque,
  fs,
  io::{self, IsTerminal as _, Read as _, Write},
  mem,
  os::{
    fd::{AsFd, AsRawFd as _, BorrowedFd, OwnedFd, RawFd},
    unix::net::UnixStream,
  },
  ptr,
  sync::{
    OnceLock,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use libc::c_int;
use rustix::termios::{self, OptionalActions, Termios};

use crate::parse::{Event, Parser, WindowSize};

/// How many bytes one read takes at most.
const READ_SIZE: usize = 4096;

/// How long an escape the input ends in waits for the rest of its sequence before it
/// reads as a key, while a lone escape can be one.
const ESCAPE_GRACE: Duration = Duration::from_millis(50);

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
/// terminal sent and decodes it, with the process signals caught meanwhile. The mode
/// the terminal was opened in is restored when the value drops, the output still
/// buffered written unless the thread is panicking, and every signal caught is handed
/// back as it was.
pub(crate) struct Tty {
  input: Fd,
  output: Fd,
  pending: Vec<u8>,
  original: Termios,
  parser: Parser,
  events: VecDeque<Event>,
  /// The window size last reported, so a change reads as a resize.
  size: Option<WindowSize>,
  /// The dev launcher's control pipe, and the partial line read from it so far.
  control: Option<(BorrowedFd<'static>, Vec<u8>)>,
  signals: Signals,
  /// When the terminal last sent anything.
  last_read: Instant,
  /// Whether every key arrives as an escape sequence of its own, so a lone escape is
  /// never a key.
  unambiguous_escapes: bool,
}

impl Tty {
  /// Opens the terminal and catches the process signals that concern it; see
  /// [`Signals::catch`].
  pub(crate) fn open() -> io::Result<Self> {
    let input = open_input()?;
    let output = open_output()?;
    let original = termios::tcgetattr(&output)?;
    let mut tty = Self {
      input,
      output,
      pending: Vec::with_capacity(READ_SIZE),
      original,
      parser: Parser::default(),
      events: VecDeque::new(),
      size: None,
      control: None,
      signals: Signals::catch()?,
      last_read: Instant::now(),
      unambiguous_escapes: false,
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
    let mut raw = termios::tcgetattr(&self.output)?;

    raw.make_raw();
    termios::tcsetattr(&self.output, OptionalActions::Flush, &raw)?;

    Ok(())
  }

  /// Opens a reply window for queries just written, which must end with the primary
  /// device attributes request: until its reply, a string sequence reads as a reply.
  pub(crate) const fn open_reply_window(&mut self) {
    self.parser.open_reply_window();
  }

  /// Sets whether every key arrives as an escape sequence of its own, as under the
  /// kitty keyboard protocol's flags the loop pushes: then an escape the input ends in
  /// always waits for the rest of its sequence, and otherwise reads as a key after
  /// [`ESCAPE_GRACE`] without more input.
  pub(crate) const fn set_unambiguous_escapes(&mut self, unambiguous: bool) {
    self.unambiguous_escapes = unambiguous;
  }

  /// Restores the mode the terminal was opened in.
  pub(crate) fn restore_mode(&mut self) -> io::Result<()> {
    termios::tcsetattr(&self.output, OptionalActions::Now, &self.original)?;

    Ok(())
  }

  /// The window's size, when the terminal reports one.
  pub(crate) fn size(&self) -> Option<WindowSize> {
    let size = termios::tcgetwinsize(&self.output).ok()?;

    Some(WindowSize {
      columns: size.ws_col,
      rows: size.ws_row,
      pixel_width: Some(size.ws_xpixel).filter(|pixels| *pixels > 0),
      pixel_height: Some(size.ws_ypixel).filter(|pixels| *pixels > 0),
    })
  }

  /// Waits up to `timeout` for input or a signal, then takes the signals caught and
  /// every event the input so far decodes to, with a resize where the window's size
  /// changed since the last call; an error when the terminal is gone. An escape the
  /// input ends in shortens the wait to what remains of its grace.
  pub(crate) fn wait(&mut self, timeout: Duration) -> io::Result<Vec<Event>> {
    let held = self.parser.holds_escape() && !self.unambiguous_escapes;
    let timeout = if held {
      timeout.min(ESCAPE_GRACE.saturating_sub(self.last_read.elapsed()))
    }
    else {
      timeout
    };
    let [input, _] = ready([self.input.as_fd(), self.signals.wake.as_fd()], timeout)?;

    if input {
      self.read_available()?;

      while ready([self.input.as_fd()], Duration::ZERO)?[0] {
        self.read_available()?;
      }
    }
    else if held && self.last_read.elapsed() >= ESCAPE_GRACE {
      self.parser.resolve_escape();
    }

    let mut events = self.signals.take();

    events.extend(self.events.drain(..));

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

    while ready([fd.as_fd()], Duration::ZERO)?[0] {
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

    self.last_read = Instant::now();
    self.parser.feed(&buffer[..read]);

    Ok(())
  }

  /// Stops the process as a stop signal's default action does, where the terminal
  /// catches stops: returns once the process continues, or at once where the system
  /// refuses the stop, as it does in an orphaned process group.
  pub(crate) fn stop(&mut self) {
    self.signals.stop();
  }

  /// Closes the terminal as dropping it does; a signal asking the process to end that
  /// was caught while it was open then ends the process as its default action does.
  pub(crate) fn close(self) {
    let ending = self.signals.ending();

    drop(self);

    if let Some(signal) = ending {
      // SAFETY: raising a signal has no preconditions.
      unsafe { libc::raise(signal) };
    }
  }

  /// Installs a panic hook that, for a panic on the calling thread, writes `restore`
  /// to the terminal and returns it to the mode it was opened in before the panic
  /// prints; a panic on another thread is left to the hook installed before.
  pub(crate) fn set_panic_hook(&self, restore: impl Fn(&mut dyn Write) + Send + Sync + 'static) {
    let original = self.original.clone();
    let owner = thread::current().id();
    let hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
      if thread::current().id() == owner
        && let Ok(mut output) = open_output()
      {
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
    self.pending.extend_from_slice(bytes);

    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    let written = self.output.write_all(&self.pending);

    self.pending.clear();

    written
  }
}

impl Drop for Tty {
  fn drop(&mut self) {
    if thread::panicking() {
      self.pending.clear();
    }

    let _ = self.flush();
    let _ = self.restore_mode();
  }
}

/// The signals asking the process to end.
const END_SIGNALS: [c_int; 4] = [libc::SIGTERM, libc::SIGHUP, libc::SIGINT, libc::SIGQUIT];

/// No job control signal is waiting to be reported.
const NO_JOB: u8 = 0;

/// A stop signal is waiting to be reported.
const STOP: u8 = 1;

/// A continue signal is waiting to be reported.
const CONTINUE: u8 = 2;

/// The signal asking the process to end that was caught, or 0 for none.
static ENDING: AtomicI32 = AtomicI32::new(0);

/// The job control signal caught last and not yet reported.
static JOB: AtomicU8 = AtomicU8::new(NO_JOB);

/// Whether a byte waits in the wake pipe.
static WOKEN: AtomicBool = AtomicBool::new(false);

/// The wake pipe's write end while signals are caught, -1 otherwise.
static WAKE: AtomicI32 = AtomicI32::new(-1);

/// The wake pipe, made once for the process: its read end, and its write end.
static PIPE: OnceLock<(UnixStream, UnixStream)> = OnceLock::new();

/// The process signals that concern the terminal, caught while the value lives and
/// handed back as they were when it drops.
struct Signals {
  /// The wake pipe's read end, readable once a signal is caught.
  wake: &'static UnixStream,
  /// Each signal caught, with the disposition it had.
  taken: Vec<(c_int, libc::sigaction)>,
}

impl Signals {
  /// Catches each signal whose disposition is the default: those asking the process to
  /// end, the stop and continue pair where both are, and the window's size change.
  ///
  /// A signal asking the process to end is reported until the process ends; a second
  /// one ends the process at once, as its default action would.
  fn catch() -> io::Result<Self> {
    let (read, write) = if let Some(pipe) = PIPE.get() {
      pipe
    }
    else {
      let (read, write) = UnixStream::pair()?;

      read.set_nonblocking(true)?;
      write.set_nonblocking(true)?;

      PIPE.get_or_init(|| (read, write))
    };
    let mut signals = Self {
      wake: read,
      taken: Vec::new(),
    };

    ENDING.store(0, Ordering::Release);
    JOB.store(NO_JOB, Ordering::Release);
    signals.drain();
    WAKE.store(write.as_raw_fd(), Ordering::Release);

    for signal in END_SIGNALS {
      signals.catch_group(&[signal]);
    }

    signals.catch_group(&[libc::SIGTSTP, libc::SIGCONT]);
    signals.catch_group(&[libc::SIGWINCH]);

    Ok(signals)
  }

  /// Catches every one of `group` where each has the default disposition, and none
  /// otherwise.
  fn catch_group(&mut self, group: &[c_int]) {
    let dispositions: Option<Vec<libc::sigaction>> =
      group.iter().map(|signal| disposition(*signal)).collect();
    let Some(dispositions) = dispositions
    else {
      return;
    };

    if dispositions
      .iter()
      .any(|disposition| disposition.sa_sigaction != libc::SIG_DFL)
    {
      return;
    }

    // SAFETY: an all-zero `sigaction` is a valid value, filled in before use.
    let mut catching: libc::sigaction = unsafe { mem::zeroed() };

    catching.sa_sigaction = catch as extern "C" fn(c_int) as libc::sighandler_t;
    catching.sa_flags = libc::SA_RESTART;

    for (signal, previous) in group.iter().zip(dispositions) {
      // SAFETY: `catching` names a handler that is async-signal-safe.
      if unsafe { libc::sigaction(*signal, &raw const catching, ptr::null_mut()) } == 0 {
        self.taken.push((*signal, previous));
      }
    }
  }

  /// Takes the signals caught since the last call, as events: a request to end first.
  fn take(&mut self) -> Vec<Event> {
    self.drain();
    WOKEN.store(false, Ordering::Release);

    let mut events = Vec::new();

    if let Some(signal) = self.ending() {
      events.push(Event::EndRequested(signal));
    }

    match JOB.swap(NO_JOB, Ordering::AcqRel) {
      STOP => events.push(Event::StopRequested),
      CONTINUE => events.push(Event::Continued),
      _ => {}
    }

    events
  }

  /// The signal asking the process to end that was caught, if one was and the value
  /// catches it.
  fn ending(&self) -> Option<c_int> {
    let signal = ENDING.load(Ordering::Acquire);

    self
      .taken
      .iter()
      .any(|(taken, _)| *taken == signal)
      .then_some(signal)
  }

  /// Empties the wake pipe.
  fn drain(&self) {
    let mut buffer = [0; 64];
    let mut wake = self.wake;

    while wake.read(&mut buffer).is_ok_and(|read| read > 0) {}
  }

  /// Stops the process as a stop signal's default action does, where the value
  /// catches stops; see [`Tty::stop`].
  fn stop(&self) {
    if !self.taken.iter().any(|(taken, _)| *taken == libc::SIGTSTP) {
      return;
    }

    // SAFETY: an all-zero `sigaction` is a valid value, and names the default action.
    let default: libc::sigaction = unsafe { mem::zeroed() };
    // SAFETY: as above.
    let mut caught: libc::sigaction = unsafe { mem::zeroed() };

    // SAFETY: the stop signal acts by default until the process continues, and is then
    // caught as before.
    unsafe {
      libc::sigaction(libc::SIGTSTP, &raw const default, &raw mut caught);
      libc::raise(libc::SIGTSTP);
      libc::sigaction(libc::SIGTSTP, &raw const caught, ptr::null_mut());
    }

    let _ = JOB.compare_exchange(CONTINUE, NO_JOB, Ordering::AcqRel, Ordering::Acquire);
  }
}

impl Drop for Signals {
  fn drop(&mut self) {
    for (signal, previous) in self.taken.drain(..).rev() {
      if disposition(signal).is_some_and(|current| {
        current.sa_sigaction == catch as extern "C" fn(c_int) as libc::sighandler_t
      }) {
        // SAFETY: `previous` is the disposition `sigaction` reported for `signal`.
        unsafe { libc::sigaction(signal, &raw const previous, ptr::null_mut()) };
      }
    }

    WAKE.store(-1, Ordering::Release);
  }
}

/// The disposition of `signal`, when the system reports one.
fn disposition(signal: c_int) -> Option<libc::sigaction> {
  // SAFETY: an all-zero `sigaction` is a valid value, overwritten by the report.
  let mut disposition: libc::sigaction = unsafe { mem::zeroed() };

  // SAFETY: a null action asks for the disposition alone.
  (unsafe { libc::sigaction(signal, ptr::null(), &raw mut disposition) } == 0)
    .then_some(disposition)
}

/// Records `signal` for the loop and wakes it; a second signal asking the process to
/// end ends it at once, as its default action would.
extern "C" fn catch(signal: c_int) {
  match signal {
    libc::SIGTSTP => JOB.store(STOP, Ordering::Release),
    libc::SIGCONT => JOB.store(CONTINUE, Ordering::Release),
    libc::SIGWINCH => {}
    _ => {
      if ENDING.swap(signal, Ordering::AcqRel) != 0 {
        // SAFETY: an all-zero `sigaction` names the default action; `sigaction` and
        // `raise` are async-signal-safe, and the signal raised acts once the handler
        // returns.
        unsafe {
          let default: libc::sigaction = mem::zeroed();

          libc::sigaction(signal, &raw const default, ptr::null_mut());
          libc::raise(signal);
        }

        return;
      }
    }
  }

  if !WOKEN.swap(true, Ordering::AcqRel) {
    let fd = WAKE.load(Ordering::Acquire);

    if fd >= 0 {
      // SAFETY: `write` is async-signal-safe, and reads one byte of a live array.
      unsafe { libc::write(fd, [0_u8].as_ptr().cast(), 1) };
    }
  }
}

/// Which of `fds` have input to read, waiting up to `timeout` for one to.
#[cfg(not(target_os = "macos"))]
fn ready<const N: usize>(fds: [BorrowedFd<'_>; N], timeout: Duration) -> io::Result<[bool; N]> {
  use rustix::event::{PollFd, PollFlags, Timespec, poll};

  let timespec = Timespec::try_from(timeout).map_err(io::Error::other)?;
  let mut polled = fds.map(|fd| PollFd::from_borrowed_fd(fd, PollFlags::IN));

  match poll(&mut polled, Some(&timespec)) {
    Ok(_) => Ok(polled.map(|fd| fd.revents().intersects(PollFlags::IN | PollFlags::HUP))),
    Err(rustix::io::Errno::INTR) => Ok([false; N]),
    Err(error) => Err(error.into()),
  }
}

/// Which of `fds` have input to read, waiting up to `timeout` for one to; `select`,
/// since `poll` does not watch `/dev/tty` on macOS.
#[cfg(target_os = "macos")]
fn ready<const N: usize>(fds: [BorrowedFd<'_>; N], timeout: Duration) -> io::Result<[bool; N]> {
  use rustix::event::{
    FdSetElement,
    FdSetIter,
    Timespec,
    fd_set_insert,
    fd_set_num_elements,
    select,
  };

  let timespec = Timespec::try_from(timeout).map_err(io::Error::other)?;
  let raw = fds.map(|fd| fd.as_raw_fd());
  let count = raw.iter().max().map_or(0, |highest| highest + 1);
  let mut set = vec![FdSetElement::default(); fd_set_num_elements(N, count)];

  for fd in raw {
    fd_set_insert(&mut set, fd);
  }

  // SAFETY: `set` holds `count` descriptors' worth of elements, as `select` requires.
  match unsafe { select(count, Some(&mut set), None, None, Some(&timespec)) } {
    Ok(_) => {
      let ready: Vec<RawFd> = FdSetIter::new(&set).collect();

      Ok(raw.map(|fd| ready.contains(&fd)))
    }
    Err(rustix::io::Errno::INTR) => Ok([false; N]),
    Err(error) => Err(error.into()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Whether the terminal's handler catches `signal`.
  fn caught(signal: c_int) -> bool {
    disposition(signal).is_some_and(|current| {
      current.sa_sigaction == catch as extern "C" fn(c_int) as libc::sighandler_t
    })
  }

  /// Raises `signal`, returning once its handler has.
  fn raise(signal: c_int) {
    // SAFETY: raising a signal has no preconditions.
    unsafe { libc::raise(signal) };
  }

  /// Sets the action of `signal` to `action`, answering the one it had.
  fn set_action(signal: c_int, action: libc::sighandler_t) -> libc::sigaction {
    // SAFETY: an all-zero `sigaction` is a valid value, filled in before use.
    let mut replacement: libc::sigaction = unsafe { mem::zeroed() };
    // SAFETY: as above.
    let mut previous: libc::sigaction = unsafe { mem::zeroed() };

    replacement.sa_sigaction = action;

    // SAFETY: the action is the default or ignoring.
    unsafe { libc::sigaction(signal, &raw const replacement, &raw mut previous) };

    previous
  }

  /// A signal at its default is caught and reported, one ignored is left alone, a
  /// request to end stands once made, and every signal is handed back as it was.
  #[test]
  fn signals_are_caught_reported_and_handed_back() {
    let hangup = set_action(libc::SIGHUP, libc::SIG_IGN);
    let mut signals = Signals::catch().expect("catch the signals");

    assert!(caught(libc::SIGTERM));
    assert!(caught(libc::SIGCONT));
    assert!(caught(libc::SIGWINCH));
    assert!(!caught(libc::SIGHUP), "an ignored signal stays ignored");

    raise(libc::SIGWINCH);

    assert!(
      ready([signals.wake.as_fd()], Duration::from_secs(5)).expect("poll the wake pipe")[0],
      "a signal wakes the loop"
    );
    assert_eq!(signals.take(), []);

    raise(libc::SIGCONT);

    assert_eq!(signals.take(), [Event::Continued]);
    assert_eq!(signals.take(), []);

    raise(libc::SIGTERM);

    assert_eq!(signals.take(), [Event::EndRequested(libc::SIGTERM)]);
    assert_eq!(signals.take(), [Event::EndRequested(libc::SIGTERM)]);

    drop(signals);

    assert!(!caught(libc::SIGTERM));
    assert_eq!(
      disposition(libc::SIGTERM).map(|disposition| disposition.sa_sigaction),
      Some(libc::SIG_DFL)
    );
    assert_eq!(
      disposition(libc::SIGHUP).map(|disposition| disposition.sa_sigaction),
      Some(libc::SIG_IGN)
    );

    set_action(libc::SIGHUP, hangup.sa_sigaction);
  }
}
