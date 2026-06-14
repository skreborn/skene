use crate::{SignalUpdate, SignalWith, signal::RwSignal};

#[derive(Debug)]
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
  pub fn notify(&self) {
    self.signal.set(());
  }

  pub fn track(&self) {
    self.signal.with(|_| {});
  }

  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    Trigger {
      signal: RwSignal::new(()),
    }
  }
}
