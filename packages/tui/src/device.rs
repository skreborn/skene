//! Where the terminal loop runs: a [`Device`] carries the bytes a terminal sends and
//! receives, reports its window's size and wakes the loop's wait.
//!
//! [`launch`](crate::launch) runs on the process's own terminal; a test's
//! pseudo-terminal ([`testing::Pty`](crate::testing::Pty)), an SSH session or a web
//! terminal is another device, driving the same loop through
//! [`Launch::run_on`](crate::Launch::run_on).

use std::{io, io::Write, sync::Arc, time::Duration};

/// The window's size in cells, and in pixels where the terminal knows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct WindowSize {
  /// The columns of cells.
  pub columns: u16,
  /// The rows of cells.
  pub rows: u16,
  /// The width in pixels, where the terminal reports it.
  pub pixel_width: Option<u16>,
  /// The height in pixels, where the terminal reports it.
  pub pixel_height: Option<u16>,
}

impl WindowSize {
  /// A window of `columns` by `rows` cells, its pixels unknown.
  #[inline(always)]
  pub const fn cells(columns: u16, rows: u16) -> Self {
    Self {
      columns,
      rows,
      pixel_width: None,
      pixel_height: None,
    }
  }
}

/// What a device learned while the loop waited, besides the terminal's input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Note {
  /// The app was asked to end, by the signal numbered.
  EndRequested(i32),
  /// The app was asked to stop, as a job control stop asks: the loop hands the terminal
  /// back and [`stop`](Device::stop)s.
  StopRequested,
  /// The app continued after a stop: the loop takes the terminal over afresh.
  Continued,
}

/// A terminal the loop runs in: the bytes it sends and receives, its window's size, and
/// a wake for the loop's wait.
///
/// What the loop writes buffers until [`flush`](Write::flush).
#[diagnostic::on_unimplemented(
  message = "`{Self}` is no terminal the loop can run in",
  label = "a device carries a terminal's bytes both ways, reports its size and wakes a wait"
)]
pub trait Device: Write {
  /// Puts the terminal in raw mode: every byte as typed, nothing echoed, no signal keys.
  ///
  /// # Errors
  ///
  /// Where the terminal refuses the mode.
  fn enter_raw_mode(&mut self) -> io::Result<()>;

  /// Restores the mode the terminal was in before [`enter_raw_mode`](Self::enter_raw_mode).
  ///
  /// # Errors
  ///
  /// Where the terminal refuses the mode.
  fn restore_mode(&mut self) -> io::Result<()>;

  /// The window's size, where the terminal reports one; a change to it wakes a
  /// [`wait`](Self::wait), as a terminal's window-change signal does.
  fn size(&self) -> Option<WindowSize>;

  /// Waits up to `timeout` for input, a wake or a note, appends what the terminal sent
  /// meanwhile to `input` and returns what else the device learned.
  ///
  /// # Errors
  ///
  /// Where the terminal is gone: its input ended or can no longer be read.
  fn wait(&mut self, timeout: Duration, input: &mut Vec<u8>) -> io::Result<Vec<Note>>;

  /// A wake for [`wait`](Self::wait), callable from any thread.
  fn waker(&self) -> Arc<dyn Fn() + Send + Sync>;

  /// Stops the app as a job control stop does, returning once it continues; returns at
  /// once where the device has no job control.
  fn stop(&mut self) {}
}
