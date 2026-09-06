//! The properties only one medium has, hung off the shared styles.

use std::fmt::Debug;

use super::{BoxFieldVisitor, PropertyName, RunFieldVisitor, RunValue, Scalar, Shape};

/// A medium's own properties, added to [`BoxStyle`](super::BoxStyle) and
/// [`RunStyle`](super::RunStyle) beside the shared ones.
///
/// A facade implements it once on a zero-sized marker: the field blocks per shape, the
/// names of its properties, the erased value an update of one carries, and the field
/// enumeration the shared machinery walks. `()` is the extension with nothing in it.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a style extension",
  label = "a facade's medium-specific properties are a zero-sized type implementing `StyleExtension`; `()` adds none"
)]
pub trait StyleExtension: Copy + Default + PartialEq + Debug + 'static {
  /// The box properties, one field per property in shape `S`.
  type BoxFields<S: Shape>: Default + Clone + PartialEq + Debug;
  /// The run properties, one field per property in shape `S`.
  type RunFields<S: Shape>: Default + Clone + PartialEq + Debug;
  /// The names of the properties.
  type Property: PropertyName;
  /// An erased value of one of the properties, for a derived slot's update.
  type Value: Clone + PartialEq + Debug + 'static;

  /// Visits every box field, in declaration order.
  fn for_each_box_field<N: Scalar, V: BoxFieldVisitor<N, Self>>(visitor: &mut V);

  /// Visits every run field, in declaration order.
  fn for_each_run_field<V: RunFieldVisitor<Self>>(visitor: &mut V);

  /// The run value a numeric token or expression yields for this medium's run
  /// properties, if any; a medium with no numeric run property yields none.
  #[inline(always)]
  #[must_use]
  fn run_value_from_number(number: f32) -> Option<RunValue<Self>> {
    let _ = number;

    None
  }

  /// The value `amount` of the way from `from` to `to`, for a driver running motion on
  /// its own clock; an extension whose values have no in-between cuts over halfway.
  #[inline(always)]
  #[must_use]
  fn interpolate_value(from: &Self::Value, to: &Self::Value, amount: f32) -> Self::Value {
    crate::motion::interpolate::snap(from, to, amount)
  }
}

/// The property names of an extension with no properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoProperty {}

impl PropertyName for NoProperty {
  #[inline(always)]
  fn name(&self) -> &'static str {
    match *self {}
  }
}

/// The erased value of an extension with no properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoValue {}

impl StyleExtension for () {
  type BoxFields<S: Shape> = ();
  type Property = NoProperty;
  type RunFields<S: Shape> = ();
  type Value = NoValue;

  #[inline(always)]
  fn for_each_box_field<N: Scalar, V: BoxFieldVisitor<N, Self>>(_: &mut V) {}

  #[inline(always)]
  fn for_each_run_field<V: RunFieldVisitor<Self>>(_: &mut V) {}
}
