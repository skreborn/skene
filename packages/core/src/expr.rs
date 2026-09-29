//! Typed expressions over lengths, numbers and colors.
//!
//! The tree inside an expression is untyped; the phantom is the only way in, and every
//! constructor and operator is defined per type, so an ill-typed expression does not
//! compile. Lengths and numbers have the ring operations with `min`, `max` and `clamp`;
//! colors have the named functions and nothing else.

use std::{
  fmt::Debug,
  hash::{Hash, Hasher},
  marker::PhantomData,
  ops::{Add, Div, Mul, Sub},
};

use super::{Definition, Leaf, Scalar, StaticLeaf, TokenName, TokenRef, Value};
use crate::{Color, Dimension, Edge, Inset, Size, Spacing};

/// A length, in the facade's scalar unit.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[must_use]
pub struct Length(pub f32);

/// A unitless number.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[must_use]
pub struct Number(pub f32);

mod sealed {
  pub trait Sealed {}
}

/// A quantity arithmetic is defined on: [`Length`] or [`Number`].
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a quantity",
  label = "arithmetic is defined on `Expr<Length>` and `Expr<Number>` only"
)]
pub trait Quantity: sealed::Sealed + Copy + Debug + 'static {
  /// The node standing for a literal `value` of this quantity.
  fn literal(value: f32) -> ExprNode;
}

impl sealed::Sealed for Length {}

impl Quantity for Length {
  #[inline(always)]
  fn literal(value: f32) -> ExprNode {
    ExprNode::Length(value)
  }
}

impl sealed::Sealed for Number {}

impl Quantity for Number {
  #[inline(always)]
  fn literal(value: f32) -> ExprNode {
    ExprNode::Number(value)
  }
}

/// A typed expression: a well-typed [`ExprNode`] tree behind a phantom.
///
/// Every operation is defined per type, so a platform walking [`node`](Self::node) meets
/// only trees the constructors could build.
///
/// ```compile_fail,E0277
/// use skene_core::{Expr, Length};
///
/// let width = Expr::<Length>::vw(50.);
/// let _area = width.clone() * width;
/// ```
#[must_use]
pub struct Expr<T> {
  node: ExprNode,
  ty: PhantomData<fn() -> T>,
}

impl<T> Expr<T> {
  #[inline(always)]
  pub(crate) const fn from_node(node: ExprNode) -> Self {
    Self {
      node,
      ty: PhantomData,
    }
  }

  /// The expression tree, for a platform rendering or evaluating it.
  #[inline(always)]
  pub const fn node(&self) -> &ExprNode {
    &self.node
  }

  /// The expression tree, by value.
  #[inline(always)]
  pub fn into_node(self) -> ExprNode {
    self.node
  }
}

impl<T: ExprOutput> Expr<T> {
  /// The expression's value in `env`, with `resolve` answering each token it names;
  /// `None` when a token is unresolved or the arithmetic has no finite result.
  #[inline(always)]
  pub fn evaluate(
    &self,
    env: &ExprEnv,
    resolve: &impl Fn(TokenName) -> Option<ExprValue>,
  ) -> Option<T> {
    T::from_expr_value(self.node.evaluate(env, resolve)?)
  }
}

impl<T> Clone for Expr<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self::from_node(self.node.clone())
  }
}

impl<T> PartialEq for Expr<T> {
  #[inline(always)]
  fn eq(&self, other: &Self) -> bool {
    self.node == other.node
  }
}

impl<T> Hash for Expr<T> {
  #[inline(always)]
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.node.hash(state);
  }
}

impl<T> Debug for Expr<T> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_tuple("Expr").field(&self.node).finish()
  }
}

/// One node of an expression tree.
///
/// Lengths are in the facade's scalar unit; the relative units are percentages of their
/// reference, and `Em` a multiple of the font size. Color amounts follow the [`Color`]
/// helpers they evaluate through.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum ExprNode {
  /// A unitless number.
  Number(f32),
  /// A length, in the facade's scalar unit.
  Length(f32),
  /// A percentage of the viewport's width.
  ViewportWidth(f32),
  /// A percentage of the viewport's height.
  ViewportHeight(f32),
  /// A percentage of the nearest named container's width; the viewport's without one.
  ContainerWidth(f32),
  /// A percentage of the nearest named container's height; the viewport's without one.
  ContainerHeight(f32),
  /// A multiple of the font size in force.
  Em(f32),
  /// The inset the display keeps clear at an edge of the viewport.
  SafeArea(Edge),
  /// A token's value.
  Token(TokenName),
  /// A literal color.
  Color(Color),
  /// The sum of two quantities of one kind.
  Add(Box<Self>, Box<Self>),
  /// The difference of two quantities of one kind.
  Sub(Box<Self>, Box<Self>),
  /// A quantity scaled by a number.
  Mul(Box<Self>, Box<Self>),
  /// A quantity divided by a number.
  Div(Box<Self>, Box<Self>),
  /// The smaller of two quantities of one kind.
  Min(Box<Self>, Box<Self>),
  /// The larger of two quantities of one kind.
  Max(Box<Self>, Box<Self>),
  /// A quantity held between a lower and an upper bound of its kind.
  Clamp(Box<Self>, Box<Self>, Box<Self>),
  /// A color mixed toward another in Oklab by an amount, `0.` to `1.`.
  Mix(Box<Self>, Box<Self>, f32),
  /// A color with its alpha replaced.
  WithAlpha(Box<Self>, f32),
  /// A color with its Oklch lightness raised by an amount, held at `1.`.
  Lighter(Box<Self>, f32),
  /// A color with its Oklch lightness lowered by an amount, held at `0.`.
  Darker(Box<Self>, f32),
  /// A color with its Oklch chroma replaced.
  WithChroma(Box<Self>, f32),
  /// A color with its Oklch hue replaced, in degrees.
  WithHue(Box<Self>, f32),
}

impl ExprNode {
  /// The node's value in `env`, with `resolve` answering each token it names; `None`
  /// when a token is unresolved or the arithmetic has no finite result.
  pub fn evaluate(
    &self,
    env: &ExprEnv,
    resolve: &impl Fn(TokenName) -> Option<ExprValue>,
  ) -> Option<ExprValue> {
    let container = env.container.unwrap_or(env.viewport);

    match self {
      Self::Number(value) => Some(ExprValue::Number(*value)),
      Self::Length(value) => Some(ExprValue::Length(*value)),
      Self::ViewportWidth(percent) => Some(ExprValue::Length(env.viewport.width * percent / 100.)),
      Self::ViewportHeight(percent) => {
        Some(ExprValue::Length(env.viewport.height * percent / 100.))
      }
      Self::ContainerWidth(percent) => Some(ExprValue::Length(container.width * percent / 100.)),
      Self::ContainerHeight(percent) => Some(ExprValue::Length(container.height * percent / 100.)),
      Self::Em(multiple) => Some(ExprValue::Length(env.em * multiple)),
      Self::SafeArea(edge) => Some(ExprValue::Length(env.safe_area.edge(*edge))),
      Self::Token(name) => resolve(*name),
      Self::Color(color) => Some(ExprValue::Color(*color)),
      Self::Add(lhs, rhs) => lhs
        .evaluate(env, resolve)?
        .combine_same(rhs.evaluate(env, resolve)?, |lhs, rhs| lhs + rhs),
      Self::Sub(lhs, rhs) => lhs
        .evaluate(env, resolve)?
        .combine_same(rhs.evaluate(env, resolve)?, |lhs, rhs| lhs - rhs),
      Self::Min(lhs, rhs) => lhs
        .evaluate(env, resolve)?
        .combine_same(rhs.evaluate(env, resolve)?, f32::min),
      Self::Max(lhs, rhs) => lhs
        .evaluate(env, resolve)?
        .combine_same(rhs.evaluate(env, resolve)?, f32::max),
      Self::Mul(lhs, rhs) => lhs
        .evaluate(env, resolve)?
        .multiply(rhs.evaluate(env, resolve)?),
      Self::Div(lhs, rhs) => lhs
        .evaluate(env, resolve)?
        .divide(rhs.evaluate(env, resolve)?),
      Self::Clamp(value, lower, upper) => value
        .evaluate(env, resolve)?
        .combine_same(upper.evaluate(env, resolve)?, f32::min)?
        .combine_same(lower.evaluate(env, resolve)?, f32::max),
      Self::Mix(lhs, rhs, amount) => {
        let lhs = lhs.evaluate(env, resolve)?.color()?;
        let rhs = rhs.evaluate(env, resolve)?.color()?;

        Some(ExprValue::Color(lhs.mix(rhs, *amount)))
      }
      Self::WithAlpha(color, alpha) => color
        .evaluate(env, resolve)?
        .map_color(|color| color.with_alpha(*alpha)),
      Self::Lighter(color, amount) => color
        .evaluate(env, resolve)?
        .map_color(|color| color.with_lightness((color.lightness() + amount).min(1.))),
      Self::Darker(color, amount) => color
        .evaluate(env, resolve)?
        .map_color(|color| color.with_lightness((color.lightness() - amount).max(0.))),
      Self::WithChroma(color, chroma) => color
        .evaluate(env, resolve)?
        .map_color(|color| color.with_chroma(*chroma)),
      Self::WithHue(color, hue) => color
        .evaluate(env, resolve)?
        .map_color(|color| color.with_hue(*hue)),
    }
  }
}

impl Hash for ExprNode {
  fn hash<H: Hasher>(&self, state: &mut H) {
    std::mem::discriminant(self).hash(state);

    match self {
      Self::Number(value)
      | Self::Length(value)
      | Self::ViewportWidth(value)
      | Self::ViewportHeight(value)
      | Self::ContainerWidth(value)
      | Self::ContainerHeight(value)
      | Self::Em(value) => state.write_u64(value.hash_bits()),
      Self::SafeArea(edge) => edge.hash(state),
      Self::Token(name) => name.hash(state),
      Self::Color(color) => color.hash(state),
      Self::Add(lhs, rhs)
      | Self::Sub(lhs, rhs)
      | Self::Mul(lhs, rhs)
      | Self::Div(lhs, rhs)
      | Self::Min(lhs, rhs)
      | Self::Max(lhs, rhs) => {
        lhs.hash(state);
        rhs.hash(state);
      }
      Self::Clamp(value, lower, upper) => {
        value.hash(state);
        lower.hash(state);
        upper.hash(state);
      }
      Self::Mix(lhs, rhs, amount) => {
        lhs.hash(state);
        rhs.hash(state);
        state.write_u64(amount.hash_bits());
      }
      Self::WithAlpha(color, amount)
      | Self::Lighter(color, amount)
      | Self::Darker(color, amount)
      | Self::WithChroma(color, amount)
      | Self::WithHue(color, amount) => {
        color.hash(state);
        state.write_u64(amount.hash_bits());
      }
    }
  }
}

/// An evaluated expression.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum ExprValue {
  /// A length, in the facade's scalar unit.
  Length(f32),
  /// A unitless number.
  Number(f32),
  /// A color.
  Color(Color),
}

impl ExprValue {
  /// Combines two quantities of one kind; `None` across kinds or without a finite
  /// result.
  fn combine_same(self, other: Self, combine: impl FnOnce(f32, f32) -> f32) -> Option<Self> {
    match (self, other) {
      (Self::Length(lhs), Self::Length(rhs)) => Self::finite_length(combine(lhs, rhs)),
      (Self::Number(lhs), Self::Number(rhs)) => Self::finite_number(combine(lhs, rhs)),
      _ => None,
    }
  }

  /// Multiplies a quantity by a number, either way round; `None` for anything else or
  /// without a finite result.
  fn multiply(self, other: Self) -> Option<Self> {
    match (self, other) {
      (Self::Length(lhs), Self::Number(rhs)) | (Self::Number(rhs), Self::Length(lhs)) => {
        Self::finite_length(lhs * rhs)
      }
      (Self::Number(lhs), Self::Number(rhs)) => Self::finite_number(lhs * rhs),
      _ => None,
    }
  }

  /// Divides a quantity by a number; `None` for anything else, a number divided by a
  /// length among it, or without a finite result.
  fn divide(self, other: Self) -> Option<Self> {
    match (self, other) {
      (Self::Length(lhs), Self::Number(rhs)) => Self::finite_length(lhs / rhs),
      (Self::Number(lhs), Self::Number(rhs)) => Self::finite_number(lhs / rhs),
      _ => None,
    }
  }

  #[inline(always)]
  fn finite_length(value: f32) -> Option<Self> {
    value.is_finite().then_some(Self::Length(value))
  }

  #[inline(always)]
  fn finite_number(value: f32) -> Option<Self> {
    value.is_finite().then_some(Self::Number(value))
  }

  #[inline(always)]
  const fn color(self) -> Option<Color> {
    match self {
      Self::Color(color) => Some(color),
      Self::Length(_) | Self::Number(_) => None,
    }
  }

  #[inline(always)]
  fn map_color(self, map: impl FnOnce(Color) -> Color) -> Option<Self> {
    self.color().map(|color| Self::Color(map(color)))
  }
}

/// The environment an expression's relative units resolve against.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct ExprEnv {
  /// The viewport's size, in the facade's scalar unit.
  pub viewport: Size<f32>,
  /// The nearest named container's size, when the node is inside one.
  pub container: Option<Size<f32>>,
  /// The font size in force, in the facade's scalar unit.
  pub em: f32,
  /// The insets the display keeps clear of the viewport's edges, in the facade's scalar
  /// unit.
  pub safe_area: Inset<f32>,
}

/// A type an evaluated expression lands in.
#[diagnostic::on_unimplemented(
  message = "an expression cannot evaluate to `{Self}`",
  label = "expressions evaluate to lengths, numbers, colors and the properties made of one"
)]
pub trait ExprOutput: Sized {
  /// This type's reading of `value`; `None` when the kinds differ.
  fn from_expr_value(value: ExprValue) -> Option<Self>;
}

impl ExprOutput for Length {
  #[inline(always)]
  fn from_expr_value(value: ExprValue) -> Option<Self> {
    match value {
      ExprValue::Length(length) => Some(Self(length)),
      ExprValue::Number(_) | ExprValue::Color(_) => None,
    }
  }
}

impl ExprOutput for Number {
  #[inline(always)]
  fn from_expr_value(value: ExprValue) -> Option<Self> {
    match value {
      ExprValue::Number(number) => Some(Self(number)),
      ExprValue::Length(_) | ExprValue::Color(_) => None,
    }
  }
}

impl ExprOutput for f32 {
  #[inline(always)]
  fn from_expr_value(value: ExprValue) -> Option<Self> {
    Number::from_expr_value(value).map(|Number(number)| number)
  }
}

impl ExprOutput for Color {
  #[inline(always)]
  fn from_expr_value(value: ExprValue) -> Option<Self> {
    value.color()
  }
}

impl<N: Scalar> ExprOutput for Dimension<N> {
  #[inline(always)]
  fn from_expr_value(value: ExprValue) -> Option<Self> {
    Length::from_expr_value(value).map(|Length(length)| Self::Length(N::from_f32(length)))
  }
}

impl<N: Scalar> ExprOutput for Inset<N> {
  #[inline(always)]
  fn from_expr_value(value: ExprValue) -> Option<Self> {
    Length::from_expr_value(value).map(|Length(length)| Self::all(N::from_f32(length)))
  }
}

impl<N: Scalar> ExprOutput for Spacing<N> {
  #[inline(always)]
  fn from_expr_value(value: ExprValue) -> Option<Self> {
    Length::from_expr_value(value).map(|Length(length)| Self::all(N::from_f32(length)))
  }
}

impl<N: Scalar> ExprOutput for Inset<Dimension<N>> {
  #[inline(always)]
  fn from_expr_value(value: ExprValue) -> Option<Self> {
    Dimension::from_expr_value(value).map(Self::all)
  }
}

impl<Q: Quantity> Add for Expr<Q> {
  type Output = Self;

  #[inline(always)]
  fn add(self, rhs: Self) -> Self {
    Self::from_node(ExprNode::Add(Box::new(self.node), Box::new(rhs.node)))
  }
}

impl<Q: Quantity> Add<f32> for Expr<Q> {
  type Output = Self;

  #[inline(always)]
  fn add(self, rhs: f32) -> Self {
    self + Self::from_node(Q::literal(rhs))
  }
}

impl<Q: Quantity> Sub for Expr<Q> {
  type Output = Self;

  #[inline(always)]
  fn sub(self, rhs: Self) -> Self {
    Self::from_node(ExprNode::Sub(Box::new(self.node), Box::new(rhs.node)))
  }
}

impl<Q: Quantity> Sub<f32> for Expr<Q> {
  type Output = Self;

  #[inline(always)]
  fn sub(self, rhs: f32) -> Self {
    self - Self::from_node(Q::literal(rhs))
  }
}

impl<Q: Quantity> Mul<Expr<Number>> for Expr<Q> {
  type Output = Self;

  #[inline(always)]
  fn mul(self, rhs: Expr<Number>) -> Self {
    Self::from_node(ExprNode::Mul(Box::new(self.node), Box::new(rhs.node)))
  }
}

impl<Q: Quantity> Mul<f32> for Expr<Q> {
  type Output = Self;

  #[inline(always)]
  fn mul(self, rhs: f32) -> Self {
    self * Expr::<Number>::from(rhs)
  }
}

impl<Q: Quantity> Div<Expr<Number>> for Expr<Q> {
  type Output = Self;

  #[inline(always)]
  fn div(self, rhs: Expr<Number>) -> Self {
    Self::from_node(ExprNode::Div(Box::new(self.node), Box::new(rhs.node)))
  }
}

impl<Q: Quantity> Div<f32> for Expr<Q> {
  type Output = Self;

  #[inline(always)]
  fn div(self, rhs: f32) -> Self {
    self / Expr::<Number>::from(rhs)
  }
}

impl<Q: Quantity> Expr<Q> {
  /// The smaller of this and `other`.
  #[inline(always)]
  pub fn min(self, other: impl Into<Self>) -> Self {
    Self::from_node(ExprNode::Min(
      Box::new(self.node),
      Box::new(other.into().node),
    ))
  }

  /// The larger of this and `other`.
  #[inline(always)]
  pub fn max(self, other: impl Into<Self>) -> Self {
    Self::from_node(ExprNode::Max(
      Box::new(self.node),
      Box::new(other.into().node),
    ))
  }

  /// This, held between `lower` and `upper`; `lower` wins where the two cross, as in
  /// CSS's `clamp()`.
  #[inline(always)]
  pub fn clamp(self, lower: impl Into<Self>, upper: impl Into<Self>) -> Self {
    Self::from_node(ExprNode::Clamp(
      Box::new(self.node),
      Box::new(lower.into().node),
      Box::new(upper.into().node),
    ))
  }
}

impl<Q: Quantity> From<f32> for Expr<Q> {
  #[inline(always)]
  fn from(value: f32) -> Self {
    Self::from_node(Q::literal(value))
  }
}

impl<Q: Quantity> From<TokenRef<Q>> for Expr<Q> {
  #[inline(always)]
  fn from(reference: TokenRef<Q>) -> Self {
    Self::from_node(ExprNode::Token(reference.name()))
  }
}

impl Expr<Length> {
  /// `percent` of the viewport's width.
  #[inline(always)]
  pub const fn vw(percent: f32) -> Self {
    Self::from_node(ExprNode::ViewportWidth(percent))
  }

  /// `percent` of the viewport's height.
  #[inline(always)]
  pub const fn vh(percent: f32) -> Self {
    Self::from_node(ExprNode::ViewportHeight(percent))
  }

  /// `percent` of the nearest named container's width; the viewport's without one.
  #[inline(always)]
  pub const fn cqw(percent: f32) -> Self {
    Self::from_node(ExprNode::ContainerWidth(percent))
  }

  /// `percent` of the nearest named container's height; the viewport's without one.
  #[inline(always)]
  pub const fn cqh(percent: f32) -> Self {
    Self::from_node(ExprNode::ContainerHeight(percent))
  }

  /// The inset the display keeps clear at `edge` of the viewport—for a notch, rounded
  /// corners or a home indicator—and zero where it keeps none.
  #[inline(always)]
  pub const fn safe_area(edge: Edge) -> Self {
    Self::from_node(ExprNode::SafeArea(edge))
  }

  /// `multiple` of the font size in force.
  #[inline(always)]
  pub const fn em(multiple: f32) -> Self {
    Self::from_node(ExprNode::Em(multiple))
  }
}

impl Expr<Color> {
  /// This color mixed toward `other` in Oklab: `0.` is this color, `1.` is `other`.
  #[inline(always)]
  pub fn mix(self, other: impl Into<Self>, amount: f32) -> Self {
    Self::from_node(ExprNode::Mix(
      Box::new(self.node),
      Box::new(other.into().node),
      amount,
    ))
  }

  /// This color with its alpha replaced, `0.` to `1.`.
  #[inline(always)]
  pub fn with_alpha(self, alpha: f32) -> Self {
    Self::from_node(ExprNode::WithAlpha(Box::new(self.node), alpha))
  }

  /// This color with its Oklch lightness raised by `amount`, held at `1.`.
  #[inline(always)]
  pub fn lighter(self, amount: f32) -> Self {
    Self::from_node(ExprNode::Lighter(Box::new(self.node), amount))
  }

  /// This color with its Oklch lightness lowered by `amount`, held at `0.`.
  #[inline(always)]
  pub fn darker(self, amount: f32) -> Self {
    Self::from_node(ExprNode::Darker(Box::new(self.node), amount))
  }

  /// This color with its Oklch chroma replaced.
  #[inline(always)]
  pub fn with_chroma(self, chroma: f32) -> Self {
    Self::from_node(ExprNode::WithChroma(Box::new(self.node), chroma))
  }

  /// This color with its Oklch hue replaced, in degrees.
  #[inline(always)]
  pub fn with_hue(self, hue: f32) -> Self {
    Self::from_node(ExprNode::WithHue(Box::new(self.node), hue))
  }
}

impl From<Color> for Expr<Color> {
  #[inline(always)]
  fn from(color: Color) -> Self {
    Self::from_node(ExprNode::Color(color))
  }
}

impl From<TokenRef<Color>> for Expr<Color> {
  #[inline(always)]
  fn from(reference: TokenRef<Color>) -> Self {
    Self::from_node(ExprNode::Token(reference.name()))
  }
}

/// The conversions carrying a quantity's expression into every property type measured
/// in it, and into each leaf holding one.
macro_rules! quantity_targets {
  ($quantity:ty => $([$($generics:tt)*] $target:ty),+ $(,)?) => {$(
    impl<$($generics)*> From<Expr<$quantity>> for Expr<$target> {
      #[inline(always)]
      fn from(expr: Expr<$quantity>) -> Self {
        Self::from_node(expr.node)
      }
    }

    impl<$($generics)*> From<Expr<$quantity>> for StaticLeaf<$target> {
      #[inline(always)]
      fn from(expr: Expr<$quantity>) -> Self {
        Self::Expr(Expr::from(expr))
      }
    }

    impl<$($generics)*> From<Expr<$quantity>> for Leaf<$target> {
      #[inline(always)]
      fn from(expr: Expr<$quantity>) -> Self {
        Self::Static(StaticLeaf::from(expr))
      }
    }

    impl<$($generics)* C> From<Expr<$quantity>> for Value<$target, C> {
      #[inline(always)]
      fn from(expr: Expr<$quantity>) -> Self {
        Self::new(Leaf::from(expr))
      }
    }

    impl<$($generics)* M: Scalar> From<Expr<$quantity>> for Definition<$target, M> {
      #[inline(always)]
      fn from(expr: Expr<$quantity>) -> Self {
        Self::new(StaticLeaf::from(expr))
      }
    }
  )+};
}

quantity_targets!(Length =>
  [N: Scalar,] Dimension<N>,
  [N: Scalar,] Inset<N>,
  [N: Scalar,] Spacing<N>,
  [N: Scalar,] Inset<Dimension<N>>,
);

quantity_targets!(Number => [] f32);

/// The conversions carrying a length token's reference into every property type
/// measured in lengths, and into each leaf holding one: the property reads the token by
/// its name.
macro_rules! length_token_targets {
  ($([$($generics:tt)*] $target:ty),+ $(,)?) => {$(
    impl<$($generics)*> From<TokenRef<Length>> for StaticLeaf<$target> {
      #[inline(always)]
      fn from(reference: TokenRef<Length>) -> Self {
        Self::Token(TokenRef::new(reference.name().as_str()))
      }
    }

    impl<$($generics)*> From<TokenRef<Length>> for Leaf<$target> {
      #[inline(always)]
      fn from(reference: TokenRef<Length>) -> Self {
        Self::Static(StaticLeaf::from(reference))
      }
    }

    impl<$($generics)* C> From<TokenRef<Length>> for Value<$target, C> {
      #[inline(always)]
      fn from(reference: TokenRef<Length>) -> Self {
        Self::new(Leaf::from(reference))
      }
    }

    impl<$($generics)* M: Scalar> From<TokenRef<Length>> for Definition<$target, M> {
      #[inline(always)]
      fn from(reference: TokenRef<Length>) -> Self {
        Self::new(StaticLeaf::from(reference))
      }
    }
  )+};
}

length_token_targets!(
  [N: Scalar,] Dimension<N>,
  [N: Scalar,] Inset<N>,
  [N: Scalar,] Spacing<N>,
  [N: Scalar,] Inset<Dimension<N>>,
);

#[cfg(test)]
mod tests {
  use super::{Expr, ExprEnv, ExprValue, Length, Number};
  use crate::{Color, Dimension, Edge, Inset, Size, TokenRef};

  /// A safe-area inset evaluates to the environment's inset at its edge.
  #[test]
  fn a_safe_area_evaluates_to_its_inset() {
    let bottom = Expr::<Length>::safe_area(Edge::Bottom) + 8.;

    assert_eq!(
      bottom.node().evaluate(&env(), &|_| None),
      Some(ExprValue::Length(42.))
    );
    assert_eq!(
      Expr::<Length>::safe_area(Edge::Left)
        .node()
        .evaluate(&env(), &|_| None),
      Some(ExprValue::Length(0.))
    );
  }

  fn env() -> ExprEnv {
    ExprEnv {
      viewport: Size::new(1000., 500.),
      container: Some(Size::new(200., 100.)),
      em: 16.,
      safe_area: Inset {
        top: 44.,
        right: 0.,
        bottom: 34.,
        left: 0.,
      },
    }
  }

  fn tokens(name: crate::TokenName) -> Option<ExprValue> {
    match name.as_str() {
      "gutter" => Some(ExprValue::Length(8.)),
      "ratio" => Some(ExprValue::Number(1.5)),
      "ink" => Some(ExprValue::Color(Color::oklab(0.2, 0.1, 0.))),
      _ => None,
    }
  }

  /// A clamp holds a token's length between a container unit and a literal, and lands
  /// in the property type of either facade.
  #[test]
  fn clamp_over_a_token_and_a_container_unit() {
    let expr = Expr::from(TokenRef::<Length>::new("gutter")).clamp(Expr::cqw(10.), 40.);

    assert_eq!(
      Expr::<Dimension<f32>>::from(expr.clone()).evaluate(&env(), &tokens),
      Some(Dimension::Length(20.))
    );
    assert_eq!(
      Expr::<Dimension<u32>>::from(expr.clone()).evaluate(&env(), &tokens),
      Some(Dimension::Length(20))
    );
    assert_eq!(
      Expr::<Inset<f32>>::from(expr).evaluate(&env(), &tokens),
      Some(Inset::all(20.))
    );
  }

  /// A length divided by a number is a length, and a number divided by a length is
  /// nothing, as in CSS, even where tokens read through references of the other kind
  /// make the division check.
  #[test]
  fn only_a_number_divides() {
    let halved = Expr::from(TokenRef::<Length>::new("gutter")) / 2.;
    let inverted =
      Expr::from(TokenRef::<Length>::new("ratio")) / Expr::from(TokenRef::<Number>::new("gutter"));

    assert_eq!(halved.evaluate(&env(), &tokens), Some(Length(4.)));
    assert_eq!(inverted.evaluate(&env(), &tokens), None);
  }

  /// Crossed bounds resolve to the lower one, as CSS's `clamp()` does.
  #[test]
  fn a_crossed_clamp_takes_its_lower_bound() {
    let expr = Expr::<Length>::from(50.).clamp(80., 20.);

    assert_eq!(expr.evaluate(&env(), &tokens), Some(Length(80.)));
  }

  /// Ring arithmetic follows the environment, and a missing token yields nothing.
  #[test]
  fn arithmetic_follows_the_environment() {
    let width = (Expr::<Length>::vw(50.) + 10.) * Expr::from(TokenRef::<Number>::new("ratio"));

    assert_eq!(width.evaluate(&env(), &tokens), Some(Length(765.)));
    assert_eq!(
      (Expr::<Length>::em(2.) / 4.).evaluate(&env(), &tokens),
      Some(Length(8.))
    );
    assert_eq!(
      (Expr::<Length>::vh(10.) - Expr::from(TokenRef::<Length>::new("missing")))
        .evaluate(&env(), &tokens),
      None
    );
    assert_eq!(
      (Expr::<Number>::from(1.) / 0.).evaluate(&env(), &tokens),
      None
    );
  }

  /// Without a container the container units read the viewport.
  #[test]
  fn container_units_fall_back_to_the_viewport() {
    let env = ExprEnv {
      container: None,
      ..env()
    };

    assert_eq!(Expr::cqh(50.).evaluate(&env, &tokens), Some(Length(250.)));
  }

  /// Mixing interpolates every Oklab component linearly toward the other color.
  #[test]
  fn mix_interpolates_in_oklab() {
    let mixed = Expr::from(TokenRef::<Color>::new("ink"))
      .mix(Color::oklab(0.8, -0.1, 0.2), 0.25)
      .evaluate(&env(), &tokens)
      .unwrap()
      .to_oklab();
    let expected = [0.35, 0.05, 0.05, 1.];

    for (component, expected) in mixed.into_iter().zip(expected) {
      assert!(
        (component - expected).abs() < 1e-4,
        "{mixed:?} should be {expected:?}",
      );
    }
  }

  /// The lightness functions move in Oklch and stop at the ends of the range.
  #[test]
  fn lightness_moves_in_oklch() {
    let base = Color::oklch(0.5, 0.1, 30.);

    assert_eq!(
      Expr::from(base).lighter(0.2).evaluate(&env(), &tokens),
      Some(base.with_lightness(0.7))
    );
    assert_eq!(
      Expr::from(base).darker(0.9).evaluate(&env(), &tokens),
      Some(base.with_lightness(0.))
    );
    assert_eq!(
      Expr::from(base)
        .with_hue(90.)
        .with_chroma(0.2)
        .with_alpha(0.5)
        .evaluate(&env(), &tokens),
      Some(base.with_hue(90.).with_chroma(0.2).with_alpha(0.5))
    );
  }

  /// Equal trees compare and hash alike whatever their phantom.
  #[test]
  fn trees_compare_by_structure() {
    let make = || Expr::<Length>::vw(10.).min(Expr::em(1.)) + 4.;

    assert_eq!(make(), make());
    assert_ne!(make(), make() + 1.);
  }
}
