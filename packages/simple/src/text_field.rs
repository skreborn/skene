//! A ready-made single-line text field in the Simple design language.

use std::marker::PhantomData;

use skene_foundation::{AnyKind, BasicTextField, CornerRadius, NodeId, Platform, TextEntry, View};
use skene_reactive::Signal;

use super::shape::{SimpleUnit, define_defaults, field_chrome};

/// A ready-made single-line text field: full-width field chrome whose hairline border
/// becomes the ring while focused.
///
/// The value flows both ways, exactly as on [`BasicTextField`]; for custom chrome,
/// compose the raw control directly.
#[derive(Debug)]
#[must_use]
pub struct SimpleTextField<P: Platform> {
  entry: TextEntry,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> SimpleTextField<P> {
  /// An empty field.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      entry: TextEntry::new(),
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

impl<P: Platform> Default for SimpleTextField<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> AnyKind for SimpleTextField<P> {}

impl<P: Platform> View<P> for SimpleTextField<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<P>();

    field_chrome::<P, TextEntry>(BasicTextField::new(self.entry)).build()
  }
}
