//! A ready-made multi-line text editor in the Simple design language.

use std::marker::PhantomData;

use skene_common::{CornerRadius, RawTextEditor, TextEntry};
use skene_core::{AnyKind, NodeId, Platform, View};
use skene_reactive::Signal;

use super::shape::{SimpleUnit, define_defaults, field_chrome};

/// The lines an editor shows unless told otherwise.
const LINES: u16 = 3;

/// A ready-made multi-line text editor: full-width field chrome whose hairline border
/// becomes the ring while focused, showing several lines at once.
///
/// The value flows both ways, exactly as on [`RawTextEditor`]; for custom chrome,
/// compose the raw control directly.
#[derive(Debug)]
#[must_use]
pub struct SimpleTextEditor<P: Platform> {
  entry: TextEntry,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> SimpleTextEditor<P> {
  /// An empty editor showing three lines.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      entry: TextEntry::new().set_lines(LINES),
      platform: PhantomData,
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

impl<P: Platform> Default for SimpleTextEditor<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> AnyKind for SimpleTextEditor<P> {}

impl<P: Platform> View<P> for SimpleTextEditor<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<P>();

    field_chrome::<P, TextEntry>(RawTextEditor::new(self.entry)).build()
  }
}
