//! Terminal state capture and restore, for the dev loop's process swaps.
//!
//! A swap kills the running app mid-raw-mode; reapplying the state captured before
//! the first spawn keeps each next instance's own saved "original" sane, so the final
//! quit restores a working terminal. Best-effort everywhere: without a terminal, or on
//! a platform with neither termios nor a console, capture yields `None`.

/// The terminal's captured state: termios on Unix, console modes on Windows.
#[cfg(unix)]
pub struct Tty(rustix::termios::Termios);

#[cfg(unix)]
impl Tty {
  /// Captures the terminal's current state, when stdin is a terminal.
  pub fn capture() -> Option<Self> {
    rustix::termios::tcgetattr(std::io::stdin()).ok().map(Self)
  }

  /// Reapplies the captured state.
  pub fn restore(&self) {
    let _ = rustix::termios::tcsetattr(
      std::io::stdin(),
      rustix::termios::OptionalActions::Now,
      &self.0,
    );
  }
}

/// The terminal's captured state: termios on Unix, console modes on Windows.
#[cfg(windows)]
pub struct Tty {
  input: u32,
  output: u32,
}

#[cfg(windows)]
impl Tty {
  /// Captures the console's current input and output modes, when there is a console.
  pub fn capture() -> Option<Self> {
    use windows_sys::Win32::System::Console::{
      GetConsoleMode,
      GetStdHandle,
      STD_INPUT_HANDLE,
      STD_OUTPUT_HANDLE,
    };

    let mut input = 0;
    let mut output = 0;

    // SAFETY: the std handles are queried and read into locals; no ownership moves.
    unsafe {
      let input_handle = GetStdHandle(STD_INPUT_HANDLE);
      let output_handle = GetStdHandle(STD_OUTPUT_HANDLE);

      if GetConsoleMode(input_handle, &mut input) == 0
        || GetConsoleMode(output_handle, &mut output) == 0
      {
        return None;
      }
    }

    Some(Self { input, output })
  }

  /// Reapplies the captured modes.
  pub fn restore(&self) {
    use windows_sys::Win32::System::Console::{
      GetStdHandle,
      STD_INPUT_HANDLE,
      STD_OUTPUT_HANDLE,
      SetConsoleMode,
    };

    // SAFETY: the std handles are queried and written from captured locals.
    unsafe {
      let _ = SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), self.input);
      let _ = SetConsoleMode(GetStdHandle(STD_OUTPUT_HANDLE), self.output);
    }
  }
}

/// The terminal's captured state; on a platform with neither termios nor a console
/// there is nothing to capture.
#[cfg(not(any(unix, windows)))]
pub struct Tty;

#[cfg(not(any(unix, windows)))]
impl Tty {
  /// Captures nothing: the platform offers no terminal state to hold.
  pub fn capture() -> Option<Self> {
    None
  }

  /// Reapplies nothing.
  pub fn restore(&self) {}
}
