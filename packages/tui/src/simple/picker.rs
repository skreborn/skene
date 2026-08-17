//! A ready-made dropdown choice in the Simple design language, in cells.

use skene_core::{NodeId, View};

use crate::{RawPicker, SimplePalette, simple};

/// A ready-made select: the current label and indicator, its popup a Simple card with
/// the highlight bar in the palette's secondary surface.
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

impl View for SimplePicker {
  fn build(self) -> NodeId {
    let palette = SimplePalette::current();

    self
      .raw
      // The popup's whole style and the highlight's color pair take no conditional
      // variant, so both resolve against the scheme in force rather than carrying one.
      .set_popup_style(
        crate::props::BoxStyle::new()
          .set_color(simple::painted(&palette.card))
          .set_border(simple::painted(&palette.border))
          .set_border_glyphs(simple::GLYPHS),
      )
      .set_highlight(
        simple::painted(&palette.secondary.surface),
        simple::painted(&palette.foreground),
      )
      .build()
  }
}

impl crate::HasFocus for SimplePicker {
  #[inline(always)]
  fn focus_core(&mut self) -> &mut crate::FocusCore {
    crate::HasFocus::focus_core(&mut self.raw)
  }
}
