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

use super::{Definition, Leaf, Scalar, StaticLeaf, TokenName, TokenRef, TokenReferences, Value};
use crate::spine::{AspectRatio, Color, Dimension, Edge, Inset, Size, Spacing};

/// A length, in the facade's scalar unit.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[must_use]
pub struct Length(pub f32);

impl From<f32> for Length {
  #[inline(always)]
  fn from(length: f32) -> Self {
    Self(length)
  }
}

impl From<i32> for Length {
  #[inline(always)]
  fn from(length: i32) -> Self {
    Self(f32::from_i32(length))
  }
}

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
/// use skene_foundation::{Expr, Length};
///
/// let width = Expr::<Length>::viewport_width(0.5);
/// let _area = width.clone() * width;
/// ```
#[must_use]
pub struct Expr<T> {
  node: ExprNode,
  ty: PhantomData<fn() -> T>,
}

impl<T> Expr<T> {
  #[inline(always)]
  pub(in crate::spine) const fn from_node(node: ExprNode) -> Self {
    Self {
      node,
      ty: PhantomData,
    }
  }

  /// The expression tree, for a platform rendering or evaluating it.
  #[inline(always)]
  #[must_use]
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
  pub fn evaluate(
    &self,
    env: &ExprEnv,
    resolve: &impl Fn(TokenName) -> Option<ExprValue>,
  ) -> Option<T> {
    T::from_expr_value(self.node.evaluate(env, resolve)?)
  }
}

impl<T> TokenReferences for Expr<T> {
  #[inline(always)]
  fn for_each_token(&self, visit: &mut dyn FnMut(TokenName)) {
    self.node.for_each_token(visit);
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
/// Lengths are in the facade's scalar unit; the relative lengths are fractions of their
/// reference, and `FontSize` a multiple of the font size. Color amounts follow the
/// [`Color`] helpers they evaluate through.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum ExprNode {
  /// A unitless number.
  Number(f32),
  /// A length, in the facade's scalar unit.
  Length(f32),
  /// A fraction of the viewport's width.
  ViewportWidth(f32),
  /// A fraction of the viewport's height.
  ViewportHeight(f32),
  /// A fraction of the nearest named container's width; the viewport's without one.
  ContainerWidth(f32),
  /// A fraction of the nearest named container's height; the viewport's without one.
  ContainerHeight(f32),
  /// A multiple of the font size in force.
  FontSize(f32),
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
      Self::ViewportWidth(fraction) => Some(ExprValue::Length(env.viewport.width * fraction)),
      Self::ViewportHeight(fraction) => Some(ExprValue::Length(env.viewport.height * fraction)),
      Self::ContainerWidth(fraction) => Some(ExprValue::Length(container.width * fraction)),
      Self::ContainerHeight(fraction) => Some(ExprValue::Length(container.height * fraction)),
      Self::FontSize(multiple) => Some(ExprValue::Length(env.font_size * multiple)),
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
      | Self::FontSize(value) => state.write_u64(value.hash_bits()),
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

impl TokenReferences for ExprNode {
  fn for_each_token(&self, visit: &mut dyn FnMut(TokenName)) {
    match self {
      Self::Number(_)
      | Self::Length(_)
      | Self::ViewportWidth(_)
      | Self::ViewportHeight(_)
      | Self::ContainerWidth(_)
      | Self::ContainerHeight(_)
      | Self::FontSize(_)
      | Self::SafeArea(_)
      | Self::Color(_) => {}
      Self::Token(name) => visit(*name),
      Self::Add(lhs, rhs)
      | Self::Sub(lhs, rhs)
      | Self::Mul(lhs, rhs)
      | Self::Div(lhs, rhs)
      | Self::Min(lhs, rhs)
      | Self::Max(lhs, rhs)
      | Self::Mix(lhs, rhs, _) => {
        lhs.for_each_token(visit);
        rhs.for_each_token(visit);
      }
      Self::Clamp(value, lower, upper) => {
        value.for_each_token(visit);
        lower.for_each_token(visit);
        upper.for_each_token(visit);
      }
      Self::WithAlpha(color, _)
      | Self::Lighter(color, _)
      | Self::Darker(color, _)
      | Self::WithChroma(color, _)
      | Self::WithHue(color, _) => color.for_each_token(visit),
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
  pub font_size: f32,
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

impl ExprOutput for AspectRatio {
  #[inline(always)]
  fn from_expr_value(value: ExprValue) -> Option<Self> {
    Number::from_expr_value(value).map(|Number(number)| Self(number))
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

impl<Q: Quantity> Add<i32> for Expr<Q> {
  type Output = Self;

  #[inline(always)]
  fn add(self, rhs: i32) -> Self {
    self + Self::from(rhs)
  }
}

impl<Q: Quantity> Sub for Expr<Q> {
  type Output = Self;

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

impl<Q: Quantity> Sub<i32> for Expr<Q> {
  type Output = Self;

  #[inline(always)]
  fn sub(self, rhs: i32) -> Self {
    self - Self::from(rhs)
  }
}

impl<Q: Quantity> Mul<Expr<Number>> for Expr<Q> {
  type Output = Self;

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
  pub fn min(self, other: impl Into<Self>) -> Self {
    Self::from_node(ExprNode::Min(
      Box::new(self.node),
      Box::new(other.into().node),
    ))
  }

  /// The larger of this and `other`.
  pub fn max(self, other: impl Into<Self>) -> Self {
    Self::from_node(ExprNode::Max(
      Box::new(self.node),
      Box::new(other.into().node),
    ))
  }

  /// This, held between `lower` and `upper`; `lower` wins where the two cross, as in
  /// CSS's `clamp()`.
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

impl<Q: Quantity> From<i32> for Expr<Q> {
  #[inline(always)]
  fn from(value: i32) -> Self {
    Self::from(f32::from_i32(value))
  }
}

impl From<TokenRef<Length>> for Expr<Length> {
  #[inline(always)]
  fn from(reference: TokenRef<Length>) -> Self {
    Self::from_node(ExprNode::Token(reference.name()))
  }
}

impl From<TokenRef<f32>> for Expr<Number> {
  #[inline(always)]
  fn from(reference: TokenRef<f32>) -> Self {
    Self::from_node(ExprNode::Token(reference.name()))
  }
}

impl Expr<Length> {
  /// `fraction` of the viewport's width: `0.5` is half of it.
  #[inline(always)]
  pub const fn viewport_width(fraction: f32) -> Self {
    Self::from_node(ExprNode::ViewportWidth(fraction))
  }

  /// `fraction` of the viewport's height: `0.5` is half of it.
  #[inline(always)]
  pub const fn viewport_height(fraction: f32) -> Self {
    Self::from_node(ExprNode::ViewportHeight(fraction))
  }

  /// `fraction` of the nearest named container's width, the viewport's without one:
  /// `0.5` is half of it.
  #[inline(always)]
  pub const fn container_width(fraction: f32) -> Self {
    Self::from_node(ExprNode::ContainerWidth(fraction))
  }

  /// `fraction` of the nearest named container's height, the viewport's without one:
  /// `0.5` is half of it.
  #[inline(always)]
  pub const fn container_height(fraction: f32) -> Self {
    Self::from_node(ExprNode::ContainerHeight(fraction))
  }

  /// The inset the display keeps clear at `edge` of the viewport—for a notch, rounded
  /// corners or a home indicator—and zero where it keeps none.
  #[inline(always)]
  pub const fn safe_area(edge: Edge) -> Self {
    Self::from_node(ExprNode::SafeArea(edge))
  }

  /// `multiple` of the font size in force.
  #[inline(always)]
  pub const fn font_size(multiple: f32) -> Self {
    Self::from_node(ExprNode::FontSize(multiple))
  }
}

impl Expr<Color> {
  /// This color mixed toward `other` in Oklab: `0.` is this color, `1.` is `other`.
  pub fn mix(self, other: impl Into<Self>, amount: f32) -> Self {
    Self::from_node(ExprNode::Mix(
      Box::new(self.node),
      Box::new(other.into().node),
      amount,
    ))
  }

  /// This color with its alpha replaced, `0.` to `1.`.
  pub fn with_alpha(self, alpha: f32) -> Self {
    Self::from_node(ExprNode::WithAlpha(Box::new(self.node), alpha))
  }

  /// This color with its Oklch lightness raised by `amount`, held at `1.`.
  pub fn lighter(self, amount: f32) -> Self {
    Self::from_node(ExprNode::Lighter(Box::new(self.node), amount))
  }

  /// This color with its Oklch lightness lowered by `amount`, held at `0.`.
  pub fn darker(self, amount: f32) -> Self {
    Self::from_node(ExprNode::Darker(Box::new(self.node), amount))
  }

  /// This color with its Oklch chroma replaced.
  pub fn with_chroma(self, chroma: f32) -> Self {
    Self::from_node(ExprNode::WithChroma(Box::new(self.node), chroma))
  }

  /// This color with its Oklch hue replaced, in degrees.
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

quantity_targets!(Number => [] f32, [] AspectRatio);

/// The conversions carrying a token's reference into every property type measured in
/// its kind, and into each leaf holding one: the property reads the token by its name.
macro_rules! token_targets {
  ($token:ty => $([$($generics:tt)*] $target:ty),+ $(,)?) => {$(
    impl<$($generics)*> From<TokenRef<$token>> for StaticLeaf<$target> {
      #[inline(always)]
      fn from(reference: TokenRef<$token>) -> Self {
        Self::Token(reference.name())
      }
    }

    impl<$($generics)*> From<TokenRef<$token>> for Leaf<$target> {
      #[inline(always)]
      fn from(reference: TokenRef<$token>) -> Self {
        Self::Static(StaticLeaf::from(reference))
      }
    }

    impl<$($generics)* C> From<TokenRef<$token>> for Value<$target, C> {
      #[inline(always)]
      fn from(reference: TokenRef<$token>) -> Self {
        Self::new(Leaf::from(reference))
      }
    }

    impl<$($generics)* M: Scalar> From<TokenRef<$token>> for Definition<$target, M> {
      #[inline(always)]
      fn from(reference: TokenRef<$token>) -> Self {
        Self::new(StaticLeaf::from(reference))
      }
    }
  )+};
}

token_targets!(Length =>
  [N: Scalar,] Dimension<N>,
  [N: Scalar,] Inset<N>,
  [N: Scalar,] Spacing<N>,
  [N: Scalar,] Inset<Dimension<N>>,
);

token_targets!(f32 => [] AspectRatio);

/// The conversions letting a plain number, an `i32` or an `f32`, stand for a length
/// literal at every leaf, in the medium's unit and keeping its fraction.
macro_rules! length_literals {
  ($($number:ty),+) => {$(
    impl From<$number> for StaticLeaf<Length> {
      /// A number as a length literal, in the medium's unit.
      #[inline(always)]
      fn from(length: $number) -> Self {
        Self::Fixed(Length::from(length))
      }
    }

    impl From<$number> for Leaf<Length> {
      /// A number as a length literal, in the medium's unit.
      #[inline(always)]
      fn from(length: $number) -> Self {
        Self::Static(StaticLeaf::from(length))
      }
    }

    impl<C> From<$number> for Value<Length, C> {
      /// A number as a length literal, in the medium's unit.
      #[inline(always)]
      fn from(length: $number) -> Self {
        Self::new(Leaf::from(length))
      }
    }

    impl<M: Scalar> From<$number> for Definition<Length, M> {
      /// A number as a length literal, in the medium's unit.
      #[inline(always)]
      fn from(length: $number) -> Self {
        Self::new(StaticLeaf::from(length))
      }
    }
  )+};
}

length_literals!(i32, f32);

impl From<f32> for StaticLeaf<AspectRatio> {
  /// A number as a ratio to one.
  #[inline(always)]
  fn from(ratio: f32) -> Self {
    Self::Fixed(AspectRatio(ratio))
  }
}

impl From<f32> for Leaf<AspectRatio> {
  /// A number as a ratio to one.
  #[inline(always)]
  fn from(ratio: f32) -> Self {
    Self::Static(StaticLeaf::from(ratio))
  }
}

impl<C> From<f32> for Value<AspectRatio, C> {
  /// A number as a ratio to one.
  #[inline(always)]
  fn from(ratio: f32) -> Self {
    Self::new(Leaf::from(ratio))
  }
}

#[cfg(test)]
mod tests {
  use super::{Expr, ExprEnv, ExprValue, Length, Number};
  use crate::spine::{Color, Dimension, Edge, Inset, Size, TokenRef};

  /// A safe-area inset evaluates to the environment's inset at its edge.
  #[test]
  fn a_safe_area_evaluates_to_its_inset() {
    let bottom = Expr::<Length>::safe_area(Edge::Bottom) + 8.;

    assert_eq!(
      bottom.node().evaluate(&env(), &|_| None),
      Some(ExprValue::Length(42.))
    );
    assert_eq!(
      Expr::<Length>::safe_area(Edge::Start)
        .node()
        .evaluate(&env(), &|_| None),
      Some(ExprValue::Length(0.))
    );
  }

  fn env() -> ExprEnv {
    ExprEnv {
      viewport: Size::new(1000., 500.),
      container: Some(Size::new(200., 100.)),
      font_size: 16.,
      safe_area: Inset {
        top: 44.,
        end: 0.,
        bottom: 34.,
        start: 0.,
      },
    }
  }

  fn tokens(name: crate::spine::TokenName) -> Option<ExprValue> {
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
    let expr = Expr::from(TokenRef::<Length>::new("gutter")).clamp(Expr::container_width(0.1), 40.);

    assert_eq!(
      Expr::<Dimension<f32>>::from(expr.clone()).evaluate(&env(), &tokens),
      Some(Dimension::Length(20.))
    );
    assert_eq!(
      Expr::<Dimension<i32>>::from(expr.clone()).evaluate(&env(), &tokens),
      Some(Dimension::Length(20))
    );
    assert_eq!(
      Expr::<Inset<f32>>::from(expr).evaluate(&env(), &tokens),
      Some(Inset::all(20))
    );
  }

  /// A length divided by a number is a length, and a number divided by a length is
  /// nothing, as in CSS, even where tokens read through references of the other kind
  /// make the division check.
  #[test]
  fn only_a_number_divides() {
    let halved = Expr::from(TokenRef::<Length>::new("gutter")) / 2.;
    let inverted =
      Expr::from(TokenRef::<Length>::new("ratio")) / Expr::from(TokenRef::<f32>::new("gutter"));

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
    let width =
      (Expr::<Length>::viewport_width(0.5) + 10.) * Expr::from(TokenRef::<f32>::new("ratio"));

    assert_eq!(width.evaluate(&env(), &tokens), Some(Length(765.)));
    assert_eq!(
      (Expr::<Length>::font_size(2.) / 4.).evaluate(&env(), &tokens),
      Some(Length(8.))
    );
    assert_eq!(
      (Expr::<Length>::viewport_height(0.1) - Expr::from(TokenRef::<Length>::new("missing")))
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

    assert_eq!(
      Expr::container_height(0.5).evaluate(&env, &tokens),
      Some(Length(250.))
    );
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
    let make = || Expr::<Length>::viewport_width(0.1).min(Expr::font_size(1.)) + 4.;

    assert_eq!(make(), make());
    assert_ne!(make(), make() + 1.);
  }
}
