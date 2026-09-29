//! The unit a facade measures lengths in.

use std::fmt::Debug;

use num_traits::ConstZero;

/// A facade's length unit: logical pixels on pixel media, cells in the terminal.
///
/// Every length-valued property, condition and token is generic over it; an expression
/// evaluates through `f32` and lands back in it.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a length unit",
  label = "a facade measures lengths in `f32` (logical pixels) or `u32` (cells)"
)]
pub trait Scalar: Copy + PartialEq + PartialOrd + Debug + Default + ConstZero + 'static {
  /// This length as an `f32`, for expression evaluation.
  fn to_f32(self) -> f32;

  /// The length nearest to `value`; a unit that cannot hold it rounds and clamps.
  fn from_f32(value: f32) -> Self;

  /// A bit pattern equal for equal values, for hashing.
  fn hash_bits(self) -> u64;
}

impl Scalar for f32 {
  #[inline(always)]
  fn to_f32(self) -> f32 {
    self
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

impl Scalar for u32 {
  #[inline(always)]
  #[expect(
    clippy::cast_precision_loss,
    reason = "a cell count stays far below the integers f32 holds exactly"
  )]
  fn to_f32(self) -> f32 {
    self as f32
  }

  #[inline(always)]
  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "the value is rounded and clamped into the cell range first"
  )]
  fn from_f32(value: f32) -> Self {
    value.round().clamp(0., Self::MAX as f32) as Self
  }

  #[inline(always)]
  fn hash_bits(self) -> u64 {
    u64::from(self)
  }
}

#[cfg(test)]
mod tests {
  use super::Scalar;

  /// Cells round to the nearest whole and never go negative.
  #[test]
  fn cells_round_and_clamp() {
    assert_eq!(u32::from_f32(2.4), 2);
    assert_eq!(u32::from_f32(2.5), 3);
    assert_eq!(u32::from_f32(-3.), 0);
    assert_eq!(u32::from_f32(1e12), u32::MAX);
  }

  /// Both zeros hash alike, as they compare alike.
  #[test]
  fn signed_zeros_hash_alike() {
    assert_eq!(0f32.hash_bits(), (-0f32).hash_bits());
    assert_ne!(1f32.hash_bits(), 2f32.hash_bits());
  }
}
