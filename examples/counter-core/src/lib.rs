//! Counter business logic, shared by every shell.

use skene::prelude::*;

/// The counter's state and behavior.
#[derive(Clone, Copy)]
#[must_use]
pub struct Counter {
  count: RwSignal<u64>,
}

impl Counter {
  /// Creates a counter at zero.
  pub fn new() -> Self {
    Self {
      count: RwSignal::new(0),
    }
  }

  /// Returns the current count.
  #[must_use]
  pub fn get(&self) -> u64 {
    self.count.get()
  }

  /// Increments the count.
  pub fn increment(&self) {
    self.count.update(|value| *value = value.saturating_add(1));
  }

  /// Decrements the count.
  pub fn decrement(&self) {
    self.count.update(|value| *value = value.saturating_sub(1));
  }
}

impl Default for Counter {
  fn default() -> Self {
    Counter::new()
  }
}
