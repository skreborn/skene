//! A ready-made single-line text field in the Simple design language.

use std::marker::PhantomData;

use skene_foundation::{
  AnyKind,
  BasicTextField,
  CornerRadius,
  NodeId,
  Platform,
  TextEntry,
  View,
  Wraps,
};
use skene_reactive::Signal;

use super::shape::{SimpleUnit, define_defaults, field_chrome};

/// A ready-made single-line text field: full-width field chrome whose hairline border
/// becomes the ring while focused.
///
/// The value flows both ways, exactly as on [`BasicTextField`]; for custom chrome,
/// compose the raw control directly.
#[doc(hidden)]
#[derive(Debug)]
#[must_use]
pub struct SimpleTextField<P: Platform> {
  entry: TextEntry,
  label: Option<String>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> SimpleTextField<P> {
  /// An empty field.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      entry: TextEntry::new(),
      label: None,
      platform: PhantomData,
    }
  }

  /// Sets the caption placed before the field, which is also its accessible name.
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
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

    let mut control = field_chrome::<P, TextEntry>(BasicTextField::new(self.entry));

    if let Some(label) = self.label {
      control = control.set_label(label);
    }

    control.build()
  }
}

/// What a platform's `SimpleTextField` carries.
pub trait SimpleTextFieldBase<P: Platform>: Wraps<Inner = SimpleTextField<P>> {
  /// An empty field.
  #[must_use]
  fn new() -> Self {
    Self::wrap(SimpleTextField::new())
  }

  /// Sets the caption placed before the field, which is also its accessible name.
  #[must_use]
  fn set_label(self, label: impl Into<String>) -> Self {
    Self::wrap(self.into_inner().set_label(label))
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
