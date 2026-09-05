//! A ready-made single-line text field in the Simple design language.

use std::marker::PhantomData;

use skene_core::next::{AnyKind, Driver, NodeId, View};
use skene_reactive::Signal;
use skene_views::{CornerRadius, RawTextField, TextEntry};

use super::shape::{SimpleUnit, define_defaults, field_chrome};

/// A ready-made single-line text field: full-width field chrome whose hairline border
/// becomes the ring while focused.
///
/// The value flows both ways, exactly as on [`RawTextField`]; for custom chrome,
/// compose the raw control directly.
#[must_use]
pub struct SimpleTextField<D: Driver> {
  entry: TextEntry,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> SimpleTextField<D> {
  /// An empty field.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      entry: TextEntry::new(),
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

impl<D: Driver> Default for SimpleTextField<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> AnyKind for SimpleTextField<D> {}

impl<D: Driver> View<D> for SimpleTextField<D>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<D>();

    field_chrome::<D, TextEntry>(RawTextField::new(self.entry)).build()
  }
}
