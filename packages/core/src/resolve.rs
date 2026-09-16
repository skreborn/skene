//! The resolution of a node's style, for a driver that resolves at runtime: each
//! property's conditions against the environment and the node's interaction pick a
//! leaf, and the leaf resolves through tokens, expressions and slots to a plain value.
//!
//! A driver builds a [`Resolver`] from what it knows about a node—the environment,
//! the interaction, the containers above it, and how to look up a token or a slot—and
//! asks it for whole styles or single leaves. The web compiles the same model to CSS
//! and never resolves.

use skene_reactive::untrack;

use crate::{
  BoxStyle,
  BoxValue,
  BoxVisitor,
  Color,
  ColorScheme,
  Condition,
  Conditional,
  ContainerName,
  ControlState,
  Definition,
  Dimension,
  DriverLeaf,
  ExprEnv,
  ExprNode,
  ExprValue,
  IntoBoxValue,
  IntoRunValue,
  Keyframe,
  Leaf,
  Media,
  MediaEnv,
  Plain,
  PropertyKey,
  RunStyle,
  RunValue,
  RunVisitor,
  Scalar,
  Size,
  Slot,
  State,
  Static,
  StaticLeaf,
  StaticValue,
  StyleExtension,
  TokenName,
  Value,
  box_style::{BoxField, BoxFieldVisitor},
  run_style::{RunField, RunFieldVisitor},
};

/// How deep a chain of tokens and expressions naming tokens is followed.
const MAX_DEPTH: usize = 32;

/// The erased token definition a runtime-resolving driver stores per token: a color,
/// a number, a dimension in the medium's unit, or a text.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum TokenValue<N: Scalar> {
  /// A color token.
  Color(Definition<Color, N>),
  /// A unitless number token.
  Number(Definition<f32, N>),
  /// A dimension token, in the medium's unit.
  Dimension(Definition<Dimension<N>, N>),
  /// A text token.
  Text(Definition<String, N>),
}

impl<N: Scalar> From<Definition<Color, N>> for TokenValue<N> {
  #[inline(always)]
  fn from(definition: Definition<Color, N>) -> Self {
    Self::Color(definition)
  }
}

impl<N: Scalar> From<Definition<f32, N>> for TokenValue<N> {
  #[inline(always)]
  fn from(definition: Definition<f32, N>) -> Self {
    Self::Number(definition)
  }
}

impl<N: Scalar> From<Definition<Dimension<N>, N>> for TokenValue<N> {
  #[inline(always)]
  fn from(definition: Definition<Dimension<N>, N>) -> Self {
    Self::Dimension(definition)
  }
}

impl<N: Scalar> From<Definition<String, N>> for TokenValue<N> {
  #[inline(always)]
  fn from(definition: Definition<String, N>) -> Self {
    Self::Text(definition)
  }
}

/// A property's value erased over the property: what a slot's leaf resolves to, and
/// what a driver's motion interpolates.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum SlotValue<N: Scalar, X: StyleExtension = ()> {
  /// A box property's value.
  Box(BoxValue<N, X>),
  /// A run property's value.
  Run(RunValue<X>),
}

/// What a derived leaf last wrote into a node's slot: a value, or a token or
/// expression resolved where the slot is read.
pub type SlotLeaf<N, X = ()> = StaticLeaf<SlotValue<N, X>>;

/// A value a leaf resolved to, erased over the property.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum Resolved<N: Scalar> {
  /// A color.
  Color(Color),
  /// A unitless number.
  Number(f32),
  /// A dimension.
  Dimension(Dimension<N>),
  /// A text.
  Text(String),
}

impl<N: Scalar> Resolved<N> {
  /// The value as a box property's, where the box vocabulary has its kind.
  #[must_use]
  pub fn into_box_value<X: StyleExtension>(self) -> Option<BoxValue<N, X>> {
    match self {
      Self::Color(color) => Some(BoxValue::Color(color)),
      Self::Number(number) => Some(BoxValue::Number(number)),
      Self::Dimension(dimension) => Some(BoxValue::Dimension(dimension)),
      Self::Text(_) => None,
    }
  }

  /// The value as a run property's, where the run vocabulary has its kind; a number
  /// reaches a run through the extension's reading of it.
  #[must_use]
  pub fn into_run_value<X: StyleExtension>(self) -> Option<RunValue<X>> {
    match self {
      Self::Color(color) => Some(RunValue::Color(color)),
      Self::Number(number) => X::run_value_from_number(number),
      Self::Dimension(_) | Self::Text(_) => None,
    }
  }

  /// The value as an expression's input, where expressions have its kind.
  #[must_use]
  pub fn into_expr_value(self) -> Option<ExprValue> {
    match self {
      Self::Color(color) => Some(ExprValue::Color(color)),
      Self::Number(number) => Some(ExprValue::Number(number)),
      Self::Dimension(Dimension::Length(length)) => Some(ExprValue::Length(length.to_f32())),
      Self::Dimension(Dimension::Fraction(_) | Dimension::Auto) | Self::Text(_) => None,
    }
  }

  /// The value as a color, when it is one.
  #[must_use]
  pub fn into_color(self) -> Option<Color> {
    match self {
      Self::Color(color) => Some(color),
      Self::Number(_) | Self::Dimension(_) | Self::Text(_) => None,
    }
  }
}

impl<N: Scalar> From<ExprValue> for Resolved<N> {
  #[inline(always)]
  fn from(value: ExprValue) -> Self {
    match value {
      ExprValue::Color(color) => Self::Color(color),
      ExprValue::Number(number) => Self::Number(number),
      ExprValue::Length(length) => Self::Dimension(Dimension::Length(N::from_f32(length))),
    }
  }
}

impl<N: Scalar> From<Color> for Resolved<N> {
  #[inline(always)]
  fn from(color: Color) -> Self {
    Self::Color(color)
  }
}

impl<N: Scalar> From<f32> for Resolved<N> {
  #[inline(always)]
  fn from(number: f32) -> Self {
    Self::Number(number)
  }
}

impl<N: Scalar> From<Dimension<N>> for Resolved<N> {
  #[inline(always)]
  fn from(dimension: Dimension<N>) -> Self {
    Self::Dimension(dimension)
  }
}

impl<N: Scalar> From<String> for Resolved<N> {
  #[inline(always)]
  fn from(text: String) -> Self {
    Self::Text(text)
  }
}

/// Whether a node, or the control it belongs to, holds focus, and whether that focus
/// shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[must_use]
pub enum Focus {
  /// Focus is elsewhere.
  #[default]
  None,
  /// Focus is held without showing: reached by a pointer press.
  Held,
  /// Focus is held and shows: reached by the keyboard, or held by a text entry, and
  /// not since a pointer press.
  Shown,
}

impl Focus {
  /// The focus `held` reports, shown when `shown`.
  #[inline(always)]
  pub const fn new(held: bool, shown: bool) -> Self {
    match (held, shown) {
      (false, _) => Self::None,
      (true, false) => Self::Held,
      (true, true) => Self::Shown,
    }
  }
}

/// A node's interaction state, as one resolution reads it: its own hover, focus and
/// press, and the state of the control enclosing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[must_use]
pub struct Interaction {
  /// Whether the pointer is over the node.
  pub hovered: bool,
  /// The focus the node, or the control it belongs to, holds.
  pub focus: Focus,
  /// Whether a press is held on the node.
  pub pressed: bool,
  /// The enclosing control's state, when the node is inside one.
  pub control: Option<ControlState>,
}

impl Interaction {
  /// Whether `state` holds: a control-scoped state against the enclosing control, the
  /// node's own against its interaction.
  #[must_use]
  pub fn holds(&self, state: State) -> bool {
    if state.is_control_state() {
      return self.control.is_some_and(|control| control.holds(state));
    }

    match state {
      State::Hovered => self.hovered,
      State::Focused => self.focus != Focus::None,
      State::FocusVisible => self.focus == Focus::Shown,
      State::Pressed => self.pressed,
      State::Checked | State::Indeterminate | State::Disabled | State::Invalid | State::Open => {
        false
      }
    }
  }
}

/// What one node's properties resolve against, and how its tokens and slots are read.
///
/// `token` looks a name up through the scene from the node; `slot` reads the node's
/// latest derived write; `em` is the font size an `em` expression evaluates against,
/// in the medium's unit.
#[must_use]
pub struct Resolver<'a, N: Scalar, X: StyleExtension = ()> {
  /// The viewport's size.
  pub viewport: Size<N>,
  /// The color scheme in force.
  pub scheme: ColorScheme,
  /// Whether the reader prefers reduced motion.
  pub reduced_motion: bool,
  /// The node's interaction.
  pub interaction: Interaction,
  /// The named containers above the node, nearest first, with their measured sizes.
  pub containers: &'a [(ContainerName, Option<Size<N>>)],
  /// The font size an `em` evaluates against.
  pub em: f32,
  /// The token definition in force at the node.
  pub token: &'a dyn Fn(TokenName) -> Option<TokenValue<N>>,
  /// The node's latest derived write into a slot.
  pub slot: &'a dyn Fn(Slot) -> Option<SlotLeaf<N, X>>,
}

impl<N: Scalar, X: StyleExtension> Resolver<'_, N, X> {
  /// The size of the nearest container of `name`, of any name for `None`.
  #[must_use]
  pub fn container(&self, name: Option<ContainerName>) -> Option<Size<N>> {
    self
      .containers
      .iter()
      .find(|(container, _)| name.is_none_or(|name| name == *container))
      .and_then(|(_, size)| *size)
  }

  fn with_media_env<R>(&self, operation: impl FnOnce(&MediaEnv<'_, N>) -> R) -> R {
    let container = |name: Option<ContainerName>| self.container(name);

    operation(&MediaEnv {
      viewport: self.viewport,
      scheme: self.scheme,
      reduced_motion: self.reduced_motion,
      reduced_transparency: false,
      high_contrast: false,
      container: &container,
    })
  }

  /// The leaf of `value` in force: the last variant whose condition holds, else the
  /// base.
  pub fn winning<'v, L>(&self, value: &'v Conditional<L, Condition<N>>) -> &'v L {
    self.with_media_env(|env| {
      value
        .variants()
        .iter()
        .rev()
        .find(|(condition, _)| condition.holds_in(env, &|state| self.interaction.holds(state)))
        .map_or(value.base(), |(_, leaf)| leaf)
    })
  }

  /// The leaf of a definition in force: the last variant whose media condition holds,
  /// else the base.
  pub fn winning_media<'v, L>(&self, definition: &'v Conditional<L, Media<N>>) -> &'v L {
    self.with_media_env(|env| {
      definition
        .variants()
        .iter()
        .rev()
        .find(|(condition, _)| condition.holds_in(env))
        .map_or(definition.base(), |(_, leaf)| leaf)
    })
  }

  /// The value of the token `name` in force at the node, through its definition;
  /// `None` for an undefined token or a chain deeper than a driver follows.
  #[must_use]
  pub fn token(&self, name: TokenName) -> Option<Resolved<N>> {
    self.token_at(name, 0)
  }

  fn token_at(&self, name: TokenName, depth: usize) -> Option<Resolved<N>> {
    if depth > MAX_DEPTH {
      return None;
    }

    match (self.token)(name)? {
      TokenValue::Color(definition) => self.leaf_at(self.winning_media(&definition), depth),
      TokenValue::Number(definition) => self.leaf_at(self.winning_media(&definition), depth),
      TokenValue::Dimension(definition) => self.leaf_at(self.winning_media(&definition), depth),
      TokenValue::Text(definition) => self.leaf_at(self.winning_media(&definition), depth),
    }
  }

  /// The value of an expression at the node.
  #[must_use]
  pub fn expr(&self, node: &ExprNode) -> Option<Resolved<N>> {
    self.expr_at(node, 0)
  }

  fn expr_at(&self, node: &ExprNode, depth: usize) -> Option<Resolved<N>> {
    if depth > MAX_DEPTH {
      return None;
    }

    let env = ExprEnv {
      viewport: Size::new(self.viewport.width.to_f32(), self.viewport.height.to_f32()),
      container: self
        .container(None)
        .map(|size| Size::new(size.width.to_f32(), size.height.to_f32())),
      em: self.em,
    };

    node
      .evaluate(&env, &|name| {
        self
          .token_at(name, depth + 1)
          .and_then(Resolved::into_expr_value)
      })
      .map(Resolved::from)
  }

  /// A static leaf resolved at the node: a fixed value as itself, a token through its
  /// definition, an expression evaluated.
  #[must_use]
  pub fn leaf<T: Clone + Into<Resolved<N>>>(&self, leaf: &StaticLeaf<T>) -> Option<Resolved<N>> {
    self.leaf_at(leaf, 0)
  }

  fn leaf_at<T: Clone + Into<Resolved<N>>>(
    &self,
    leaf: &StaticLeaf<T>,
    depth: usize,
  ) -> Option<Resolved<N>> {
    match leaf {
      StaticLeaf::Fixed(fixed) => Some(fixed.clone().into()),
      StaticLeaf::Token(reference) => self.token_at(reference.name(), depth + 1),
      StaticLeaf::Expr(expr) => self.expr_at(expr.node(), depth + 1),
    }
  }

  /// A color leaf resolved at the node.
  #[must_use]
  pub fn color_leaf(&self, leaf: &StaticLeaf<Color>) -> Option<Color> {
    self.leaf(leaf).and_then(Resolved::into_color)
  }

  /// The winning leaf of a driver's box value, resolved to the property's type: a
  /// static leaf through tokens and expressions, a slot through the leaf last written
  /// into it.
  #[must_use]
  pub fn box_value<T: IntoBoxValue<N, X>>(
    &self,
    value: &StaticValue<T, Condition<N>>,
  ) -> Option<T> {
    match self.winning(value) {
      DriverLeaf::Static(leaf) => self.keyframe_box_value(leaf),
      DriverLeaf::Slot(slot) => self.box_leaf(&(self.slot)(*slot)?, |value| match value {
        SlotValue::Box(value) => T::from_box_value(value),
        SlotValue::Run(_) => None,
      }),
    }
  }

  /// The winning leaf of a driver's run value, resolved to the property's type: a
  /// static leaf through tokens and expressions, a slot through the leaf last written
  /// into it.
  #[must_use]
  pub fn run_value<T: IntoRunValue<X>>(&self, value: &StaticValue<T, Condition<N>>) -> Option<T> {
    match self.winning(value) {
      DriverLeaf::Static(leaf) => self.keyframe_run_value(leaf),
      DriverLeaf::Slot(slot) => self.run_leaf(&(self.slot)(*slot)?, |value| match value {
        SlotValue::Run(value) => T::from_run_value(value),
        SlotValue::Box(_) => None,
      }),
    }
  }

  /// A keyframe's box leaf, resolved to the property's type.
  #[must_use]
  pub fn keyframe_box_value<T: IntoBoxValue<N, X>>(&self, leaf: &StaticLeaf<T>) -> Option<T> {
    self.box_leaf(leaf, |fixed| Some(fixed))
  }

  /// A keyframe's run leaf, resolved to the property's type.
  #[must_use]
  pub fn keyframe_run_value<T: IntoRunValue<X>>(&self, leaf: &StaticLeaf<T>) -> Option<T> {
    self.run_leaf(leaf, |fixed| Some(fixed))
  }

  fn box_leaf<V: Clone, T: IntoBoxValue<N, X>>(
    &self,
    leaf: &StaticLeaf<V>,
    fixed: impl FnOnce(V) -> Option<T>,
  ) -> Option<T> {
    match leaf {
      StaticLeaf::Fixed(value) => fixed(value.clone()),
      StaticLeaf::Token(reference) => self
        .token(reference.name())
        .and_then(Resolved::into_box_value)
        .and_then(T::from_box_value),
      StaticLeaf::Expr(expr) => self
        .expr(expr.node())
        .and_then(Resolved::into_box_value)
        .and_then(T::from_box_value),
    }
  }

  fn run_leaf<V: Clone, T: IntoRunValue<X>>(
    &self,
    leaf: &StaticLeaf<V>,
    fixed: impl FnOnce(V) -> Option<T>,
  ) -> Option<T> {
    match leaf {
      StaticLeaf::Fixed(value) => fixed(value.clone()),
      StaticLeaf::Token(reference) => self
        .token(reference.name())
        .and_then(Resolved::into_run_value)
        .and_then(T::from_run_value),
      StaticLeaf::Expr(expr) => self
        .expr(expr.node())
        .and_then(Resolved::into_run_value)
        .and_then(T::from_run_value),
    }
  }

  /// The winning leaf of a declared value, resolved to the property's type; a derived
  /// leaf is read once, untracked.
  #[must_use]
  pub fn declared_value<T: IntoBoxValue<N, X>>(&self, value: &Value<T, Condition<N>>) -> Option<T> {
    match self.winning(value) {
      Leaf::Static(leaf) => self.keyframe_box_value(leaf),
      Leaf::Derived(compute) => self.keyframe_box_value(&untrack(|| compute())),
    }
  }

  /// A driver's box style with every property resolved.
  pub fn box_style(&self, style: &BoxStyle<N, Static<Condition<N>>, X>) -> BoxStyle<N, Plain, X> {
    let mut visitor = ResolveBox {
      resolver: self,
      source: style,
      target: BoxStyle::default(),
    };

    BoxStyle::<N, Static<Condition<N>>, X>::for_each_field(&mut visitor);

    visitor.target
  }

  /// A driver's run style with every property resolved.
  pub fn run_style(&self, style: &RunStyle<Static<Condition<N>>, X>) -> RunStyle<Plain, X> {
    let mut visitor = ResolveRun {
      resolver: self,
      source: style,
      target: RunStyle::default(),
    };

    RunStyle::<Static<Condition<N>>, X>::for_each_field(&mut visitor);

    visitor.target
  }

  /// A box keyframe with every leaf resolved at the node.
  pub fn box_keyframe(&self, keyframe: &BoxStyle<N, Keyframe, X>) -> BoxStyle<N, Plain, X> {
    let mut visitor = ResolveBoxKeyframe {
      resolver: self,
      source: keyframe,
      target: BoxStyle::default(),
    };

    BoxStyle::<N, Keyframe, X>::for_each_field(&mut visitor);

    visitor.target
  }

  /// A run keyframe with every leaf resolved at the node.
  pub fn run_keyframe(&self, keyframe: &RunStyle<Keyframe, X>) -> RunStyle<Plain, X> {
    let mut visitor = ResolveRunKeyframe {
      resolver: self,
      source: keyframe,
      target: RunStyle::default(),
    };

    RunStyle::<Keyframe, X>::for_each_field(&mut visitor);

    visitor.target
  }
}

/// Resolves every box property of the source into the target.
struct ResolveBox<'a, 'r, N: Scalar, X: StyleExtension> {
  resolver: &'a Resolver<'r, N, X>,
  source: &'a BoxStyle<N, Static<Condition<N>>, X>,
  target: BoxStyle<N, Plain, X>,
}

impl<N: Scalar, X: StyleExtension> BoxFieldVisitor<N, X> for ResolveBox<'_, '_, N, X> {
  fn field<F: BoxField<N, X>>(&mut self) {
    if let Some(value) = F::get(self.source) {
      *F::get_mut(&mut self.target) = self.resolver.box_value(value);
    }
  }
}

/// Resolves every leaf of a box keyframe into the target.
struct ResolveBoxKeyframe<'a, 'r, N: Scalar, X: StyleExtension> {
  resolver: &'a Resolver<'r, N, X>,
  source: &'a BoxStyle<N, Keyframe, X>,
  target: BoxStyle<N, Plain, X>,
}

impl<N: Scalar, X: StyleExtension> BoxFieldVisitor<N, X> for ResolveBoxKeyframe<'_, '_, N, X> {
  fn field<F: BoxField<N, X>>(&mut self) {
    if let Some(leaf) = F::get(self.source) {
      *F::get_mut(&mut self.target) = self.resolver.keyframe_box_value(leaf);
    }
  }
}

/// Resolves every run property of the source into the target.
struct ResolveRun<'a, 'r, N: Scalar, X: StyleExtension> {
  resolver: &'a Resolver<'r, N, X>,
  source: &'a RunStyle<Static<Condition<N>>, X>,
  target: RunStyle<Plain, X>,
}

impl<N: Scalar, X: StyleExtension> RunFieldVisitor<X> for ResolveRun<'_, '_, N, X> {
  fn field<F: RunField<X>>(&mut self) {
    if let Some(value) = F::get(self.source) {
      *F::get_mut(&mut self.target) = self.resolver.run_value(value);
    }
  }
}

/// Resolves every leaf of a run keyframe into the target.
struct ResolveRunKeyframe<'a, 'r, N: Scalar, X: StyleExtension> {
  resolver: &'a Resolver<'r, N, X>,
  source: &'a RunStyle<Keyframe, X>,
  target: RunStyle<Plain, X>,
}

impl<N: Scalar, X: StyleExtension> RunFieldVisitor<X> for ResolveRunKeyframe<'_, '_, N, X> {
  fn field<F: RunField<X>>(&mut self) {
    if let Some(leaf) = F::get(self.source) {
      *F::get_mut(&mut self.target) = self.resolver.keyframe_run_value(leaf);
    }
  }
}

/// Finds one property of a plain style by key.
struct FindValue<K, V> {
  key: K,
  found: Option<V>,
}

impl<N: Scalar, X: StyleExtension> BoxVisitor<N, Plain, X>
  for FindValue<PropertyKey<X::Property>, BoxValue<N, X>>
{
  fn value<T: IntoBoxValue<N, X>>(&mut self, key: PropertyKey<X::Property>, value: &Option<T>) {
    if key == self.key {
      self.found = value.clone().map(IntoBoxValue::into_box_value);
    }
  }
}

impl<X: StyleExtension> RunVisitor<Plain, X> for FindValue<PropertyKey<X::Property>, RunValue<X>> {
  fn value<T: IntoRunValue<X>>(&mut self, key: PropertyKey<X::Property>, value: &Option<T>) {
    if key == self.key {
      self.found = value.clone().map(IntoRunValue::into_run_value);
    }
  }
}

impl<N: Scalar, X: StyleExtension> BoxStyle<N, Plain, X> {
  /// The value of the property `key`, erased; `None` where the style leaves it unset.
  #[must_use]
  pub fn value(&self, key: PropertyKey<X::Property>) -> Option<BoxValue<N, X>> {
    let mut find = FindValue { key, found: None };

    self.visit(&mut find);

    find.found
  }
}

impl<X: StyleExtension> RunStyle<Plain, X> {
  /// The value of the property `key`, erased; `None` where the style leaves it unset.
  #[must_use]
  pub fn value(&self, key: PropertyKey<X::Property>) -> Option<RunValue<X>> {
    let mut find = FindValue { key, found: None };

    self.visit(&mut find);

    find.found
  }
}
