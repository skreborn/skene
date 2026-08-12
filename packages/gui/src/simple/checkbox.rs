//! A ready-made checkbox in the Simple design language.

use skene_core::View;

use crate::RawCheckbox;

/// A ready-made checkbox: the platform's ink-drawn box and mark, with the Simple
/// interaction feedback.
///
/// The checked state flows both ways, exactly as on [`RawCheckbox`]; for custom
/// styling, compose the raw control directly.
#[must_use]
pub struct SimpleCheckbox {
  raw: RawCheckbox,
}

impl SimpleCheckbox {
  /// Creates an unchecked checkbox.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      raw: RawCheckbox::new(),
    }
  }

  /// Sets the label beside the box; clicking it toggles too.
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

  /// Sets the toggle handler, run with the new state on every change.
  #[inline(always)]
  pub fn handle_toggle(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.raw = self.raw.handle_toggle(action);

    self
  }

  /// Binds the state to `signal` in both directions: the checkbox shows the signal and
  /// each toggle writes it back.
  #[inline(always)]
  pub fn bind_checked(mut self, signal: skene_reactive::Signal<bool>) -> Self {
    self.raw = self.raw.bind_checked(signal);

    self
  }
}

impl Default for SimpleCheckbox {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl View for SimpleCheckbox {
  fn build(self) -> skene_core::NodeId {
    self.raw.build()
  }
}

impl crate::HasFocus for SimpleCheckbox {
  #[inline(always)]
  fn focus_core(&mut self) -> &mut crate::FocusCore {
    crate::HasFocus::focus_core(&mut self.raw)
  }
}
