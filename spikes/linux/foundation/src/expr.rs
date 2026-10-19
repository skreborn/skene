//! Values computed where a property is read, from what is in force there.

use std::marker::PhantomData;

use crate::{Color, Fill, Value};

/// A value computed where it is read, from what is in force there, as CSS computes
/// `currentColor` where a declaration is used: the ink, at an alpha of its own.
///
/// ```
/// use skene_foundation::{Color, Expr, Fill};
///
/// let faint = Fill::from(Expr::<Color>::ink().with_alpha(0.25));
///
/// assert_eq!(faint, Fill::Ink(Some(0.25)));
/// ```
#[must_use]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Expr<T> {
  alpha: Option<f32>,
  kind: PhantomData<fn() -> T>,
}

impl Expr<Color> {
  /// The ink in force: the color the node's text is drawn in, as it inherits it.
  #[inline(always)]
  pub const fn ink() -> Self {
    Self {
      alpha: None,
      kind: PhantomData,
    }
  }

  /// This color at `alpha`, `0` clear to `1` opaque, in place of its own.
  #[inline(always)]
  pub const fn with_alpha(self, alpha: f32) -> Self {
    Self {
      alpha: Some(alpha),
      ..self
    }
  }
}

impl From<Expr<Color>> for Fill {
  #[inline(always)]
  fn from(expr: Expr<Color>) -> Self {
    Self::Ink(expr.alpha)
  }
}

impl From<Expr<Color>> for Value<Fill> {
  #[inline(always)]
  fn from(expr: Expr<Color>) -> Self {
    Self::from(Fill::from(expr))
  }
}
