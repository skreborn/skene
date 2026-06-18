use crate::{SignalUpdate, SignalWith, signal::RwSignal};

/// A valueless signal: notify it to re-run every effect that tracked it.
#[derive(Debug)]
#[must_use]
pub struct Trigger {
  signal: RwSignal<()>,
}

impl Copy for Trigger {}

impl Clone for Trigger {
  fn clone(&self) -> Self {
    *self
  }
}

impl Trigger {
  /// Re-runs every effect that tracked this trigger.
  #[inline(always)]
  pub fn notify(&self) {
    self.signal.set(());
  }

  /// Subscribes the running effect to this trigger.
  #[inline(always)]
  pub fn track(&self) {
    self.signal.with(|_| {});
  }

  /// Creates a trigger.
  #[inline(always)]
  pub fn new() -> Self {
    Trigger {
      signal: RwSignal::new(()),
    }
  }
}

impl Default for Trigger {
  fn default() -> Self {
    Self::new()
  }
}
