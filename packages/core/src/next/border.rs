//! A box's border.

use std::hash::{Hash, Hasher};

use super::{Scalar, StaticLeaf};
use crate::Color;

/// A solid border: a width in the facade's scalar unit and a color that may be a token
/// or an expression.
///
/// The terminal's width is always one cell; the corner radius is a property of the
/// medium's extension.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Border<N> {
  /// The width.
  pub width: N,
  /// The color.
  pub color: StaticLeaf<Color>,
}

impl<N> Border<N> {
  /// A border of `width` in `color`.
  #[inline(always)]
  pub fn new(width: N, color: impl Into<StaticLeaf<Color>>) -> Self {
    Self {
      width,
      color: color.into(),
    }
  }
}

impl<N: Scalar> Hash for Border<N> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    state.write_u64(self.width.hash_bits());
    self.color.hash(state);
  }
}
