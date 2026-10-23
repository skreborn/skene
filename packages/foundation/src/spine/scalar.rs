//! The unit a facade measures lengths in.

use std::fmt::Debug;

/// A facade's length unit: logical pixels on pixel media, cells in the terminal.
///
/// Every length-valued property, condition and token is generic over it; an expression
/// evaluates through `f32` and lands back in it.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a length unit",
  label = "a facade measures lengths in `f32` (logical pixels) or `i32` (cells)"
)]
pub trait Scalar:
  Copy + PartialEq + PartialOrd + Debug + Default + IntoScalar<Self> + 'static
{
  /// Zero in this unit.
  const ZERO: Self;

  /// This length as an `f32`, for expression evaluation.
  fn to_f32(self) -> f32;

  /// The length `value` is in this unit.
  fn from_i32(value: i32) -> Self;

  /// The length nearest to `value`; a unit that cannot hold it rounds and clamps.
  fn from_f32(value: f32) -> Self;

  /// A bit pattern equal for equal values, for hashing.
  fn hash_bits(self) -> u64;
}

impl Scalar for f32 {
  const ZERO: Self = 0.;

  #[inline(always)]
  fn to_f32(self) -> f32 {
    self
  }

  #[inline(always)]
  #[expect(
    clippy::cast_precision_loss,
    reason = "a length stays far below the integers f32 holds exactly"
  )]
  fn from_i32(value: i32) -> Self {
    value as Self
  }

  #[inline(always)]
  fn from_f32(value: f32) -> Self {
    value
  }

  #[inline(always)]
  fn hash_bits(self) -> u64 {
    if self == 0. {
      0
    }
    else {
      u64::from(self.to_bits())
    }
  }
}

impl Scalar for i32 {
  const ZERO: Self = 0;

  #[inline(always)]
  #[expect(
    clippy::cast_precision_loss,
    reason = "a cell count stays far below the integers f32 holds exactly"
  )]
  fn to_f32(self) -> f32 {
    self as f32
  }

  #[inline(always)]
  fn from_i32(value: i32) -> Self {
    value
  }

  #[inline(always)]
  #[expect(
    clippy::cast_possible_truncation,
    reason = "the value is rounded first, and the cast saturates at the cell range"
  )]
  fn from_f32(value: f32) -> Self {
    value.round() as Self
  }

  #[inline(always)]
  fn hash_bits(self) -> u64 {
    u64::from(self.cast_unsigned())
  }
}

/// A plain number in a facade's length unit: `16`, `-4` or `16.5`.
///
/// What every length-valued constructor takes, beside the conversions every length
/// property's value has from both numbers. An `f32` in a unit of whole cells rounds to
/// the nearest; an `i32` is exact in either unit. Where a length is a
/// [`Dimension`](crate::spine::Dimension), as a margin's edges are, a plain number is a
/// fixed length and a dimension stands as itself. Code generic over the platform writes
/// its lengths as an app does:
///
/// ```
/// use skene_foundation::{Container, DecorationBase, HasChild, Inset, Realization, Text};
///
/// fn card<P: Realization>() -> Container<P> {
///   Container::new()
///     .set_padding(Inset::all(4))
///     .set_child(Text::new("Card"))
/// }
/// ```
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a length",
  label = "not a length here",
  note = "lengths are plain numbers: `16`, `-4` or `16.5`, an `i32` or an `f32`"
)]
pub trait IntoScalar<N>: Copy {
  /// This number in the unit.
  fn into_scalar(self) -> N;
}

impl<N: Scalar> IntoScalar<N> for i32 {
  #[inline(always)]
  fn into_scalar(self) -> N {
    N::from_i32(self)
  }
}

impl<N: Scalar> IntoScalar<N> for f32 {
  #[inline(always)]
  fn into_scalar(self) -> N {
    N::from_f32(self)
  }
}

#[cfg(test)]
mod tests {
  use super::{IntoScalar, Scalar};

  /// Cells round to the nearest whole, half away from zero, and clamp to the cell range.
  #[test]
  fn cells_round_and_clamp() {
    assert_eq!(i32::from_f32(2.4), 2);
    assert_eq!(i32::from_f32(2.5), 3);
    assert_eq!(i32::from_f32(-2.5), -3);
    assert_eq!(i32::from_f32(-3.4), -3);
    assert_eq!(i32::from_f32(1e12), i32::MAX);
    assert_eq!(i32::from_f32(-1e12), i32::MIN);
    assert_eq!(i32::from_f32(f32::NAN), 0);
  }

  /// Negative cells keep their sign through `f32` and hash apart from their magnitude.
  #[test]
  fn negative_cells_round_trip() {
    assert_eq!(i32::from_f32((-4).to_f32()), -4);
    assert_eq!((-4i32).to_f32(), -4.);
    assert_ne!((-4i32).hash_bits(), 4i32.hash_bits());
    assert_eq!(0i32.hash_bits(), 0);
  }

  /// A plain number lands in either unit: an integer exactly, a float rounded to the
  /// nearest cell.
  #[test]
  fn plain_numbers_land_in_either_unit() {
    assert_eq!(IntoScalar::<f32>::into_scalar(-4), -4.);
    assert_eq!(IntoScalar::<f32>::into_scalar(16.5), 16.5);
    assert_eq!(IntoScalar::<i32>::into_scalar(-4), -4);
    assert_eq!(IntoScalar::<i32>::into_scalar(1.5), 2);
    assert_eq!(IntoScalar::<i32>::into_scalar(1.4), 1);
  }

  /// Both zeros hash alike, as they compare alike.
  #[test]
  fn signed_zeros_hash_alike() {
    assert_eq!(0f32.hash_bits(), (-0f32).hash_bits());
    assert_ne!(1f32.hash_bits(), 2f32.hash_bits());
  }
}
