use crate::{
  access::{Set, With},
  signal::Signal,
};

/// A valueless signal: notify it to re-run every effect that tracked it.
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct Trigger {
  signal: Signal<()>,
}

impl Trigger {
  /// Creates a trigger owned by the current [`Owner`](crate::Owner).
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      signal: Signal::opaque(()),
    }
  }

  /// Re-runs every effect that tracked this trigger.
  #[inline(always)]
  pub fn notify(&self) {
    self.signal.set(());
  }

  /// Subscribes the running effect to this trigger.
  #[inline(always)]
  pub fn track(&self) {
    self.signal.with(|()| {});
  }
}

impl Default for Trigger {
  fn default() -> Self {
    Self::new()
  }
}
