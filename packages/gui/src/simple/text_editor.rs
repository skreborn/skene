//! A ready-made multi-line text area in the Simple design language.

use skene_core::{Dimension, View};

use crate::{HasDecoration, RawTextEditor, simple};

/// A ready-made multi-line text area: full-width field chrome whose border brightens
/// on focus.
///
/// The value flows both ways, exactly as on [`RawTextEditor`]; for custom styling,
/// compose the raw control directly.
#[must_use]
pub struct SimpleTextEditor {
  raw: RawTextEditor,
}

impl SimpleTextEditor {
  /// Creates an empty text area.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      raw: RawTextEditor::new(),
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

  /// Sets the visible height, in text lines.
  #[inline(always)]
  pub fn set_lines(mut self, lines: u16) -> Self {
    self.raw = self.raw.set_lines(lines);

    self
  }

  /// Sets the change handler, run with the new value on every edit, in real time.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.raw = self.raw.handle_change(action);

    self
  }

  /// Binds the value to `signal` in both directions: the text area shows the signal and
  /// each edit writes it back.
  #[inline(always)]
  pub fn bind_value(mut self, signal: skene_reactive::Signal<String>) -> Self {
    self.raw = self.raw.bind_value(signal);

    self
  }
}

impl Default for SimpleTextEditor {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl View for SimpleTextEditor {
  fn build(self) -> skene_core::NodeId {
    simple::field_chrome(self.raw, &crate::SimplePalette::current())
      .set_width(Dimension::Fraction(1.))
      .build()
  }
}

impl crate::HasFocus for SimpleTextEditor {
  #[inline(always)]
  fn focus_core(&mut self) -> &mut crate::FocusCore {
    crate::HasFocus::focus_core(&mut self.raw)
  }
}
