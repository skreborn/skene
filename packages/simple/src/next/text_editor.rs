//! A ready-made multi-line text editor in the Simple design language.

use std::marker::PhantomData;

use skene_core::next::{AnyKind, Driver, NodeId, View};
use skene_reactive::Signal;
use skene_views::{CornerRadius, RawTextEditor, TextEntry};

use super::shape::{define_defaults, field_chrome};

/// The lines an editor shows unless told otherwise.
const LINES: u16 = 3;

/// A ready-made multi-line text editor: full-width field chrome whose hairline border
/// becomes the ring while focused, showing several lines at once.
///
/// The value flows both ways, exactly as on [`RawTextEditor`]; for custom chrome,
/// compose the raw control directly.
#[must_use]
pub struct SimpleTextEditor<D: Driver> {
  entry: TextEntry,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> SimpleTextEditor<D> {
  /// An empty editor showing three lines.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      entry: TextEntry::new().set_lines(LINES),
      driver: PhantomData,
    }
  }

  /// Sets the text.
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.entry = self.entry.set_value(value);

    self
  }

  /// Derives the text from the signals `value` reads.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.entry = self.entry.derive_value(value);

    self
  }

  /// Sets the hint shown while the value is empty.
  #[inline(always)]
  pub fn set_placeholder(mut self, placeholder: impl Into<String>) -> Self {
    self.entry = self.entry.set_placeholder(placeholder);

    self
  }

  /// Sets the lines shown at once; at least two, so the entry stays an editor.
  #[inline(always)]
  pub fn set_lines(mut self, lines: u16) -> Self {
    self.entry = self.entry.set_lines(lines.max(2));

    self
  }

  /// Runs `action` with the text an edit asks for.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.entry = self.entry.handle_change(action);

    self
  }

  /// Binds the text to `signal` in both directions.
  #[inline(always)]
  pub fn bind_value(mut self, signal: Signal<String>) -> Self {
    self.entry = self.entry.bind_value(signal);

    self
  }
}

impl<D: Driver> Default for SimpleTextEditor<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> AnyKind for SimpleTextEditor<D> {}

impl<D: Driver> View<D> for SimpleTextEditor<D>
where
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<D>();

    field_chrome::<D, TextEntry>(RawTextEditor::new(self.entry)).build()
  }
}
