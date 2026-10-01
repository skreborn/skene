//! The process's own terminal as a [`Device`]: raw mode, the window size, the bytes
//! written and read, and the process signals that concern the terminal.

use std::{
  fs,
  io::{self, IsTerminal as _, Read as _, Write},
  mem,
  os::{
    fd::{AsFd, AsRawFd as _, BorrowedFd, OwnedFd},
    unix::net::UnixStream,
  },
  ptr,
  sync::{
    Arc,
    OnceLock,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
  },
  thread,
  time::Duration,
};

use libc::c_int;
use rustix::termios::{self, OptionalActions, Termios};

use crate::device::{Device, Note, WindowSize};

/// How many bytes one read takes at most.
pub(crate) const READ_SIZE: usize = 4096;

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

/// The terminal the loop runs in: the process's own.
///
/// Writes buffer until [`flush`](Write::flush); [`wait`](Device::wait) reads what the
/// terminal sent, with the process signals caught meanwhile. The mode the terminal was
/// opened in is restored when the value drops, the output still buffered written unless
/// the thread is panicking, and every signal caught is handed back as it was.
pub(crate) struct Tty {
  /// Where the terminal's input is read.
  input: Fd,
  /// Where the terminal's output is written.
  output: Fd,
  /// The output written and not yet flushed.
  pending: Vec<u8>,
  /// The mode the terminal was opened in.
  original: Termios,
  /// The process signals caught.
  signals: Signals,
}

impl Tty {
  /// Opens the terminal and catches the process signals that concern it; see
  /// [`Signals::catch`].
  pub(crate) fn open() -> io::Result<Self> {
    let input = open_input()?;
    let output = open_output()?;
    let original = termios::tcgetattr(&output)?;

    Ok(Self {
      input,
      output,
      pending: Vec::with_capacity(READ_SIZE),
      original,
      signals: Signals::catch()?,
    })
  }

  /// Reads what the terminal sent into `input`; the end of input is an error.
  fn read_available(&self, input: &mut Vec<u8>) -> io::Result<()> {
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

    input.extend_from_slice(&buffer[..read]);

    Ok(())
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

impl Device for Tty {
  fn enter_raw_mode(&mut self) -> io::Result<()> {
    let mut raw = termios::tcgetattr(&self.output)?;

    raw.make_raw();
    termios::tcsetattr(&self.output, OptionalActions::Flush, &raw)?;

    Ok(())
  }

  fn restore_mode(&mut self) -> io::Result<()> {
    termios::tcsetattr(&self.output, OptionalActions::Now, &self.original)?;

    Ok(())
  }

  fn size(&self) -> Option<WindowSize> {
    let size = termios::tcgetwinsize(&self.output).ok()?;

    Some(WindowSize {
      columns: size.ws_col,
      rows: size.ws_row,
      pixel_width: Some(size.ws_xpixel).filter(|pixels| *pixels > 0),
      pixel_height: Some(size.ws_ypixel).filter(|pixels| *pixels > 0),
    })
  }

  fn wait(&mut self, timeout: Duration, input: &mut Vec<u8>) -> io::Result<Vec<Note>> {
    let [readable, _] = ready([self.input.as_fd(), self.signals.wake.as_fd()], timeout)?;

    if readable {
      self.read_available(input)?;

      while ready([self.input.as_fd()], Duration::ZERO)?[0] {
        self.read_available(input)?;
      }
    }

    Ok(self.signals.take())
  }

  fn waker(&self) -> Arc<dyn Fn() + Send + Sync> {
    Arc::new(wake)
  }

  fn stop(&mut self) {
    self.signals.stop();
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
    let _ = Device::restore_mode(self);
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

  /// Takes the signals caught since the last call, as notes: a request to end first.
  fn take(&mut self) -> Vec<Note> {
    self.drain();
    WOKEN.store(false, Ordering::Release);

    let mut notes = Vec::new();

    if let Some(signal) = self.ending() {
      notes.push(Note::EndRequested(signal));
    }

    match JOB.swap(NO_JOB, Ordering::AcqRel) {
      STOP => notes.push(Note::StopRequested),
      CONTINUE => notes.push(Note::Continued),
      _ => {}
    }

    notes
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
  /// catches stops; see [`Device::stop`].
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

  wake();
}

/// Wakes the loop from its wait, from any thread or a signal handler; a wake already
/// pending covers it.
pub(crate) fn wake() {
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
pub(crate) fn ready<const N: usize>(
  fds: [BorrowedFd<'_>; N],
  timeout: Duration,
) -> io::Result<[bool; N]> {
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
pub(crate) fn ready<const N: usize>(
  fds: [BorrowedFd<'_>; N],
  timeout: Duration,
) -> io::Result<[bool; N]> {
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
      let ready: Vec<std::os::fd::RawFd> = FdSetIter::new(&set).collect();

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

    assert_eq!(signals.take(), [Note::Continued]);
    assert_eq!(signals.take(), []);

    raise(libc::SIGTERM);

    assert_eq!(signals.take(), [Note::EndRequested(libc::SIGTERM)]);
    assert_eq!(signals.take(), [Note::EndRequested(libc::SIGTERM)]);

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
