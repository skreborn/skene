//! A ready-made dropdown choice in the Simple design language.

use skene_core::View;

use crate::{RawPicker, simple, simple::simple_surface};

/// A ready-made select: field chrome around the platform dropdown, its border
/// brightening on focus.
///
/// The chosen value flows both ways, exactly as on [`RawPicker`]; for custom styling,
/// compose the raw control directly.
#[must_use]
pub struct SimplePicker {
  raw: RawPicker,
}

impl SimplePicker {
  /// Creates a select with no options.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      raw: RawPicker::new(),
    }
  }

  /// Sets the options, in order, each a `(value, label)` pair.
  #[inline(always)]
  pub fn set_options<V: Into<String>, L: Into<String>>(
    mut self,
    options: impl IntoIterator<Item = (V, L)>,
  ) -> Self {
    self.raw = self.raw.set_options(options);

    self
  }

  /// Sets a static selected value.
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.raw = self.raw.set_value(value);

    self
  }

  /// Sets a derived selected value, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.raw = self.raw.derive_value(value);

    self
  }

  /// Sets the change handler, run with the chosen value on every choice.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.raw = self.raw.handle_change(action);

    self
  }

  /// Binds the selection to `signal` in both directions: the select shows the signal
  /// and each choice writes it back.
  #[inline(always)]
  pub fn bind_value(mut self, signal: skene_reactive::Signal<String>) -> Self {
    self.raw = self.raw.bind_value(signal);

    self
  }
}

impl Default for SimplePicker {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

simple_surface!(SimplePicker);

impl View for SimplePicker {
  fn build(self) -> skene_core::NodeId {
    let (condition, style) = simple::field_focus();

    self
      .raw
      .set_style(simple::field_style())
      .set_when(condition, style)
      .build()
  }
}
