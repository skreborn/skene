use fixed::types::I26F6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(transparent)]
#[must_use]
/// Fixed-point fractional number with 26-bit integer and 6-bit fractional precision.
pub struct Number(I26F6);

impl Number {
  /// Zero.
  pub const ZERO: Self = Self(I26F6::ZERO);
}

macro_rules! impl_number_conv {
  ($prim:ty) => {
    impl From<$prim> for Number {
      #[inline(always)]
      fn from(value: $prim) -> Self {
        Self(I26F6::saturating_from_num(value))
      }
    }

    impl From<Number> for $prim {
      #[inline(always)]
      fn from(value: Number) -> Self {
        I26F6::saturating_to_num(value.0)
      }
    }
  };
}

impl_number_conv!(u8);
impl_number_conv!(i8);
impl_number_conv!(u16);
impl_number_conv!(i16);
impl_number_conv!(u32);
impl_number_conv!(i32);
impl_number_conv!(u64);
impl_number_conv!(i64);
impl_number_conv!(f32);
impl_number_conv!(f64);
