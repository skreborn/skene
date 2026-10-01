//! A ready-made multi-line text editor in the Simple design language.

use std::marker::PhantomData;

use skene_foundation::{
  AnyKind,
  BasicTextEditor,
  CornerRadius,
  NodeId,
  Platform,
  TextEntry,
  View,
  Wraps,
};
use skene_reactive::Signal;

use super::shape::{SimpleUnit, define_defaults, field_chrome};

/// The lines an editor shows unless told otherwise.
const LINES: u16 = 3;

/// A ready-made multi-line text editor: full-width field chrome whose hairline border
/// becomes the ring while focused, showing several lines at once.
///
/// The value flows both ways, exactly as on [`BasicTextEditor`]; for custom chrome,
/// compose the raw control directly.
#[doc(hidden)]
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
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.entry = self.entry.set_value(value);

    self
  }

  /// Derives the text from the signals `value` reads.
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.entry = self.entry.derive_value(value);

    self
  }

  /// Sets the hint shown while the value is empty.
  pub fn set_placeholder(mut self, placeholder: impl Into<String>) -> Self {
    self.entry = self.entry.set_placeholder(placeholder);

    self
  }

  /// Sets the lines shown at once; at least two.
  #[inline(always)]
  pub fn set_lines(mut self, lines: u16) -> Self {
    self.entry = self.entry.set_lines(lines.max(2));

    self
  }

  /// Runs `action` with the text an edit asks for.
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

    field_chrome::<P, TextEntry>(BasicTextEditor::new(self.entry)).build()
  }
}

/// What a platform's `SimpleTextEditor` carries.
pub trait SimpleTextEditorBase<P: Platform>: Wraps<Inner = SimpleTextEditor<P>> {
  /// An empty editor showing three lines.
  #[must_use]
  fn new() -> Self {
    Self::wrap(SimpleTextEditor::new())
  }

  /// Sets the text.
  #[must_use]
  fn set_value(self, value: impl Into<String>) -> Self {
    Self::wrap(self.into_inner().set_value(value))
  }

  /// Derives the text from the signals `value` reads.
  #[must_use]
  fn derive_value(self, value: impl Fn() -> String + 'static) -> Self {
    Self::wrap(self.into_inner().derive_value(value))
  }

  /// Sets the hint shown while the value is empty.
  #[must_use]
  fn set_placeholder(self, placeholder: impl Into<String>) -> Self {
    Self::wrap(self.into_inner().set_placeholder(placeholder))
  }

  /// Sets the lines shown at once; at least two.
  #[must_use]
  fn set_lines(self, lines: u16) -> Self {
    Self::wrap(self.into_inner().set_lines(lines))
  }

  /// Runs `action` with the text an edit asks for.
  #[must_use]
  fn handle_change(self, action: impl Fn(&str) + 'static) -> Self {
    Self::wrap(self.into_inner().handle_change(action))
  }

  /// Binds the text to `signal` in both directions.
  #[must_use]
  fn bind_value(self, signal: Signal<String>) -> Self {
    Self::wrap(self.into_inner().bind_value(signal))
  }
}
