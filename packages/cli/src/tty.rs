//! Terminal state capture and restore, for the dev loop's process swaps.
//!
//! The state captured before the first spawn is reapplied before each later one, so
//! every instance starts from a working terminal and the final quit restores it.
//! Best-effort everywhere: without a terminal, or on a platform without termios,
//! capture yields `None`.

/// The terminal's captured state: its termios.
#[cfg(unix)]
pub(crate) struct Tty(rustix::termios::Termios);

#[cfg(unix)]
impl Tty {
  /// Captures the terminal's current state, when stdin is a terminal.
  pub(crate) fn capture() -> Option<Self> {
    rustix::termios::tcgetattr(std::io::stdin()).ok().map(Self)
  }

  /// Reapplies the captured state.
  pub(crate) fn restore(&self) {
    let _ = rustix::termios::tcsetattr(
      std::io::stdin(),
      rustix::termios::OptionalActions::Now,
      &self.0,
    );
  }
}

/// The terminal's captured state; without termios there is nothing to capture.
#[cfg(not(unix))]
pub(crate) struct Tty;

#[cfg(not(unix))]
impl Tty {
  /// Captures nothing: the platform offers no terminal state to hold.
  pub(crate) const fn capture() -> Option<Self> {
    None
  }

  /// Reapplies nothing.
  pub(crate) const fn restore(&self) {}
}
