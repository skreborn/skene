//! A ready-made radio button in the Simple design language, in cells.

use skene_core::{NodeId, View};

use crate::RawRadio;

/// A ready-made radio button: the theme's marks with the Simple interaction feedback.
///
/// Members of one group share its name, exactly as on [`RawRadio`]; for custom styling,
/// compose the raw control directly.
#[must_use]
pub struct SimpleRadio {
  raw: RawRadio,
}

impl SimpleRadio {
  /// Creates the `group` member standing for `value`, unchecked.
  #[inline(always)]
  pub fn new(group: impl Into<String>, value: impl Into<String>) -> Self {
    Self {
      raw: RawRadio::new(group, value),
    }
  }

  /// Sets the label after the mark; clicking it selects too.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.raw = self.raw.set_label(label);

    self
  }

  /// Sets a static checked state.
  #[inline(always)]
  pub fn set_checked(mut self, checked: bool) -> Self {
    self.raw = self.raw.set_checked(checked);

    self
  }

  /// Sets a derived checked state, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_checked(mut self, checked: impl Fn() -> bool + 'static) -> Self {
    self.raw = self.raw.derive_checked(checked);

    self
  }

  /// Sets the toggle handler, run with the new state when this member's state changes.
  #[inline(always)]
  pub fn handle_toggle(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.raw = self.raw.handle_toggle(action);

    self
  }

  /// Binds the member to `signal` holding the group's selected value: the member shows
  /// as checked while the signal equals its value, and selecting it writes its value
  /// back.
  #[inline(always)]
  pub fn bind_value(mut self, signal: skene_reactive::Signal<String>) -> Self {
    self.raw = self.raw.bind_value(signal);

    self
  }
}

impl View for SimpleRadio {
  fn build(self) -> NodeId {
    self.raw.build()
  }
}
