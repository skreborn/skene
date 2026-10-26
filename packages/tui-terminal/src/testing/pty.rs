//! A pseudo-terminal the real loop runs in under test.

use std::{
  fs,
  io::{self, Read as _, Write},
  os::{
    fd::{AsFd as _, OwnedFd},
    unix::net::UnixStream,
  },
  sync::Arc,
  time::{Duration, Instant},
};

use rustix::{
  pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt},
  termios::{self, OptionalActions, Termios, Winsize},
};

use crate::{
  device::{Device, Note, WindowSize},
  tty::{READ_SIZE, ready},
};

/// The app's side of a pseudo-terminal, a [`Device`] the real loop runs on.
///
/// The loop runs on it through [`Launch::run_on`](crate::Launch::run_on) while a test
/// drives the other side, its [`PtyHost`], as a user at a terminal would. A wait wakes on
/// input, on its own waker and on a resize the host makes, as the window-change signal
/// a terminal raises wakes the app on a real one.
#[derive(Debug)]
pub struct Pty {
  /// The pseudo-terminal's app side.
  terminal: OwnedFd,
  /// The output written and not yet flushed.
  pending: Vec<u8>,
  /// The mode the pseudo-terminal was opened in.
  original: Termios,
  /// The wake pipe: its read end, readable once woken, and its write end.
  wake: Arc<(UnixStream, UnixStream)>,
}

/// The test's side of a pseudo-terminal: what a user types goes in, what the app draws
/// comes out.
#[derive(Debug)]
pub struct PtyHost {
  /// The pseudo-terminal's controlling side.
  controller: OwnedFd,
  /// The app side's wake pipe, woken by a resize.
  wake: Arc<(UnixStream, UnixStream)>,
}

impl Pty {
  /// Opens a pseudo-terminal of `size`, returning the app's side and the test's.
  ///
  /// # Errors
  ///
  /// Where the system has no pseudo-terminal to give.
  pub fn open(size: WindowSize) -> io::Result<(Self, PtyHost)> {
    let controller = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)?;

    grantpt(&controller)?;
    unlockpt(&controller)?;

    let name = ptsname(&controller, Vec::new())?;
    let path = name
      .to_str()
      .map_err(|_| io::Error::other("a pseudo-terminal's name that is no path"))?;
    let terminal: OwnedFd = fs::OpenOptions::new()
      .read(true)
      .write(true)
      .open(path)?
      .into();
    let original = termios::tcgetattr(&terminal)?;
    let (read, write) = UnixStream::pair()?;

    read.set_nonblocking(true)?;
    write.set_nonblocking(true)?;

    let wake = Arc::new((read, write));
    let host = PtyHost {
      controller,
      wake: Arc::clone(&wake),
    };

    host.resize(size)?;

    Ok((
      Self {
        terminal,
        pending: Vec::with_capacity(READ_SIZE),
        original,
        wake,
      },
      host,
    ))
  }

  /// Empties the wake pipe.
  fn drain_wake(&self) {
    let mut buffer = [0; 64];
    let mut wake = &self.wake.0;

    while wake.read(&mut buffer).is_ok_and(|read| read > 0) {}
  }
}

impl Write for Pty {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.pending.extend_from_slice(bytes);

    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    let mut written = 0;

    while written < self.pending.len() {
      match rustix::io::write(&self.terminal, &self.pending[written..]) {
        Ok(count) => written += count,
        Err(rustix::io::Errno::INTR) => {}
        Err(error) => {
          self.pending.clear();

          return Err(error.into());
        }
      }
    }

    self.pending.clear();

    Ok(())
  }
}

impl Device for Pty {
  fn enter_raw_mode(&mut self) -> io::Result<()> {
    let mut raw = termios::tcgetattr(&self.terminal)?;

    raw.make_raw();
    termios::tcsetattr(&self.terminal, OptionalActions::Flush, &raw)?;

    Ok(())
  }

  fn restore_mode(&mut self) -> io::Result<()> {
    termios::tcsetattr(&self.terminal, OptionalActions::Now, &self.original)?;

    Ok(())
  }

  fn size(&self) -> Option<WindowSize> {
    let size = termios::tcgetwinsize(&self.terminal).ok()?;

    Some(WindowSize {
      columns: size.ws_col,
      rows: size.ws_row,
      pixel_width: Some(size.ws_xpixel).filter(|pixels| *pixels > 0),
      pixel_height: Some(size.ws_ypixel).filter(|pixels| *pixels > 0),
    })
  }

  fn wait(&mut self, timeout: Duration, input: &mut Vec<u8>) -> io::Result<Vec<Note>> {
    let [readable, woken] = ready([self.terminal.as_fd(), self.wake.0.as_fd()], timeout)?;

    if woken {
      self.drain_wake();
    }

    if readable {
      let mut buffer = [0; READ_SIZE];

      loop {
        let read = match rustix::io::read(&self.terminal, &mut buffer) {
          Ok(read) => read,
          Err(rustix::io::Errno::INTR) => continue,
          Err(error) => return Err(error.into()),
        };

        if read == 0 {
          return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "the pseudo-terminal's input ended",
          ));
        }

        input.extend_from_slice(&buffer[..read]);

        if !ready([self.terminal.as_fd()], Duration::ZERO)?[0] {
          break;
        }
      }
    }

    Ok(Vec::new())
  }

  fn waker(&self) -> Arc<dyn Fn() + Send + Sync> {
    let wake = Arc::clone(&self.wake);

    Arc::new(move || {
      let _ = (&wake.1).write(&[0]);
    })
  }
}

impl Drop for Pty {
  fn drop(&mut self) {
    if std::thread::panicking() {
      self.pending.clear();
    }

    let _ = self.flush();
    let _ = Device::restore_mode(self);
  }
}

impl PtyHost {
  /// Types `bytes` into the terminal, as a user's keys and a terminal's reports arrive.
  ///
  /// # Errors
  ///
  /// Where the app's side is gone.
  pub fn type_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
    let mut written = 0;

    while written < bytes.len() {
      match rustix::io::write(&self.controller, &bytes[written..]) {
        Ok(count) => written += count,
        Err(rustix::io::Errno::INTR) => {}
        Err(error) => return Err(error.into()),
      }
    }

    Ok(())
  }

  /// What the app drew within `timeout`: every byte it wrote, waiting for the first up to
  /// `timeout`, then taking whatever follows at once; empty where nothing came.
  ///
  /// # Errors
  ///
  /// Where the terminal cannot be read for a reason other than the app's side closing.
  pub fn drawn(&mut self, timeout: Duration) -> io::Result<Vec<u8>> {
    let mut drawn = Vec::new();
    let mut buffer = [0; READ_SIZE];
    let deadline = Instant::now() + timeout;

    while ready(
      [self.controller.as_fd()],
      if drawn.is_empty() {
        deadline.saturating_duration_since(Instant::now())
      }
      else {
        Duration::from_millis(10)
      },
    )?[0]
    {
      match rustix::io::read(&self.controller, &mut buffer) {
        Ok(0) | Err(rustix::io::Errno::IO) => break,
        Ok(read) => drawn.extend_from_slice(&buffer[..read]),
        Err(rustix::io::Errno::INTR) => {}
        Err(error) => return Err(error.into()),
      }
    }

    Ok(drawn)
  }

  /// Resizes the terminal's window to `size`, waking the app's side.
  ///
  /// # Errors
  ///
  /// Where the system refuses the size.
  pub fn resize(&self, size: WindowSize) -> io::Result<()> {
    termios::tcsetwinsize(
      &self.controller,
      Winsize {
        ws_col: size.columns,
        ws_row: size.rows,
        ws_xpixel: size.pixel_width.unwrap_or(0),
        ws_ypixel: size.pixel_height.unwrap_or(0),
      },
    )?;

    let _ = (&self.wake.1).write(&[1]);

    Ok(())
  }
}
