//! A ready-made single-line text input in the Simple design language, in cells.

use skene_core::{Dimension, NodeId, View};

use crate::{RawTextField, simple, simple::simple_surface};

/// A ready-made single-line text input: full-width field chrome whose border brightens
/// to the ring under the pointer and inks fully on focus.
///
/// The value flows both ways, exactly as on [`RawTextField`]; for custom styling,
/// compose the raw control directly.
#[must_use]
pub struct SimpleTextField {
  raw: RawTextField,
}

impl SimpleTextField {
  /// Creates an empty input.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      raw: RawTextField::new(),
    }
  }

  /// Sets a static value.
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.raw = self.raw.set_value(value);

    self
  }

  /// Sets a derived value, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.raw = self.raw.derive_value(value);

    self
  }

  /// Sets the hint shown while the value is empty.
  #[inline(always)]
  pub fn set_placeholder(mut self, placeholder: impl Into<String>) -> Self {
    self.raw = self.raw.set_placeholder(placeholder);

    self
  }

  /// Sets the change handler, run with the new value on every edit, in real time.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.raw = self.raw.handle_change(action);

    self
  }

  /// Binds the value to `signal` in both directions: the input shows the signal and each
  /// edit writes it back.
  #[inline(always)]
  pub fn bind_value(mut self, signal: skene_reactive::Signal<String>) -> Self {
    self.raw = self.raw.bind_value(signal);

    self
  }
}

impl Default for SimpleTextField {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

simple_surface!(SimpleTextField);

impl View for SimpleTextField {
  fn build(self) -> NodeId {
    let (hover_condition, hover_style) = simple::field_hover();
    let (focus_condition, focus_style) = simple::field_focus();

    self
      .raw
      .set_style(simple::field_style().set_width(Dimension::Fraction(1.)))
      .set_when(hover_condition, hover_style)
      .set_when(focus_condition, focus_style)
      .build()
  }
}
