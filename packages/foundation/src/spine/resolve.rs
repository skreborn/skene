//! The resolution of a node's style, for a platform that resolves at runtime: each
//! property's conditions against the environment and the node's interaction pick a
//! leaf, and the leaf resolves through tokens, expressions and slots to a plain value.
//!
//! A platform builds a [`Resolver`] from what it knows about a node—the environment,
//! the interaction, the containers above it, and how to look up a token or a slot—and
//! asks it for whole styles or single leaves. The web compiles the same model to CSS
//! and never resolves.

use std::cell::Cell;

use skene_reactive::untrack;

use crate::spine::{
  BoxStyle,
  BoxValue,
  BoxVisitor,
  Color,
  Condition,
  Conditional,
  ContainerName,
  ControlState,
  Definition,
  Dimension,
  Environment,
  ExprEnv,
  ExprNode,
  ExprValue,
  Inset,
  IntoBoxValue,
  IntoRunValue,
  Keyframe,
  Leaf,
  Length,
  Media,
  MediaEnv,
  NodeId,
  NodeRecord,
  Plain,
  Platform,
  PlatformLeaf,
  PropertyKey,
  RunStyle,
  RunValue,
  RunVisitor,
  Scalar,
  Scene,
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
  scene::TokenScope,
  typography,
};

/// How deep a chain of tokens and expressions naming tokens is followed.
const MAX_DEPTH: usize = 32;

thread_local! {
  /// How many token definitions the thread is resolving, one inside another.
  static DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The erased token definition a runtime-resolving platform stores per token: a color,
/// a number, a length or a dimension in the medium's unit, or a text.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum TokenValue<N: Scalar> {
  /// A color token.
  Color(Definition<Color, N>),
  /// A unitless number token.
  Number(Definition<f32, N>),
  /// A length token, in the medium's unit.
  Length(Definition<Length, N>),
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

impl<N: Scalar> From<Definition<Length, N>> for TokenValue<N> {
  #[inline(always)]
  fn from(definition: Definition<Length, N>) -> Self {
    Self::Length(definition)
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
/// what a platform's motion interpolates.
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

impl<N: Scalar, X: StyleExtension> From<StaticLeaf<BoxValue<N, X>>> for SlotLeaf<N, X> {
  /// A box property's leaf, erased over the property.
  #[inline(always)]
  fn from(leaf: StaticLeaf<BoxValue<N, X>>) -> Self {
    leaf.map(SlotValue::Box)
  }
}

impl<N: Scalar, X: StyleExtension> From<StaticLeaf<RunValue<X>>> for SlotLeaf<N, X> {
  /// A run property's leaf, erased over the property.
  #[inline(always)]
  fn from(leaf: StaticLeaf<RunValue<X>>) -> Self {
    leaf.map(SlotValue::Run)
  }
}

/// A value a leaf resolved to, erased over the property.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum Resolved<N: Scalar> {
  /// A color.
  Color(Color),
  /// A unitless number.
  Number(f32),
  /// A length in the medium's unit, its fractions kept.
  Length(f32),
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
      Self::Length(length) => Some(BoxValue::Length(Length(length))),
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
      Self::Length(length) => Some(RunValue::Length(Length(length))),
      Self::Dimension(_) | Self::Text(_) => None,
    }
  }

  /// The value as an expression's input, where expressions have its kind.
  #[must_use]
  pub fn into_expr_value(self) -> Option<ExprValue> {
    match self {
      Self::Color(color) => Some(ExprValue::Color(color)),
      Self::Number(number) => Some(ExprValue::Number(number)),
      Self::Length(length) => Some(ExprValue::Length(length)),
      Self::Dimension(Dimension::Length(length)) => Some(ExprValue::Length(length.to_f32())),
      Self::Dimension(Dimension::Fraction(_) | Dimension::Auto) | Self::Text(_) => None,
    }
  }

  /// The value as a color, when it is one.
  #[must_use]
  pub fn into_color(self) -> Option<Color> {
    match self {
      Self::Color(color) => Some(color),
      Self::Number(_) | Self::Length(_) | Self::Dimension(_) | Self::Text(_) => None,
    }
  }
}

impl<N: Scalar> From<ExprValue> for Resolved<N> {
  #[inline(always)]
  fn from(value: ExprValue) -> Self {
    match value {
      ExprValue::Color(color) => Self::Color(color),
      ExprValue::Number(number) => Self::Number(number),
      ExprValue::Length(length) => Self::Length(length),
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

impl<N: Scalar> From<Length> for Resolved<N> {
  #[inline(always)]
  fn from(length: Length) -> Self {
    Self::Length(length.0)
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

/// A node's interaction state, as one resolution reads it: its own hover, focus, press
/// and drags, and the state of the control enclosing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[must_use]
#[expect(
  clippy::struct_excessive_bools,
  reason = "one bool per interaction state is the domain shape"
)]
pub struct Interaction {
  /// Whether the pointer is over the node.
  pub hovered: bool,
  /// The focus the node, or the control it belongs to, holds.
  pub focus: Focus,
  /// Whether a press is held on the node.
  pub pressed: bool,
  /// Whether a drag the node offered is in progress.
  pub dragged: bool,
  /// Whether a drag the node takes is over it.
  pub targeted: bool,
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
      State::Dragged => self.dragged,
      State::Targeted => self.targeted,
      State::Checked | State::Indeterminate | State::Disabled | State::Invalid | State::Open => {
        false
      }
    }
  }
}

/// What one node's properties resolve against, and how its tokens and slots are read.
///
/// `token` gives the value of the token in force at the node, which the platform
/// resolves where it is defined, as CSS computes a custom property; `slot` reads the
/// node's latest derived write; `em` is the font size an `em` expression evaluates
/// against, in the medium's unit.
#[must_use]
pub struct Resolver<'a, N: Scalar, X: StyleExtension = ()> {
  /// The reader's environment.
  pub environment: Environment<N>,
  /// The node's interaction.
  pub interaction: Interaction,
  /// Set once a condition reads the interaction: a resolution that leaves it unset
  /// holds whatever the interaction becomes.
  pub interaction_read: &'a Cell<bool>,
  /// The named containers above the node, nearest first, with their measured sizes.
  pub containers: &'a [(ContainerName, Option<Size<N>>)],
  /// The font size in force, which a multiple of the font size evaluates against.
  pub font_size: f32,
  /// The value of the token in force at the node, resolved where it is defined.
  pub token: &'a dyn Fn(TokenName) -> Option<Resolved<N>>,
  /// The node's latest derived write into a slot.
  pub slot: &'a dyn Fn(Slot) -> Option<SlotLeaf<N, X>>,
}

impl<N: Scalar, X: StyleExtension> std::fmt::Debug for Resolver<'_, N, X> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Resolver").finish_non_exhaustive()
  }
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
      environment: self.environment,
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
        .find(|(condition, _)| {
          condition.holds_in(env, &|state| {
            self.interaction_read.set(true);
            self.interaction.holds(state)
          })
        })
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

  /// The value of the token `name` in force at the node, resolved where it is defined;
  /// `None` for an undefined token.
  #[must_use]
  pub fn token(&self, name: TokenName) -> Option<Resolved<N>> {
    (self.token)(name)
  }

  /// The value `definition` gives its token in this environment: its winning variant,
  /// resolved here.
  #[must_use]
  pub fn definition(&self, definition: &TokenValue<N>) -> Option<Resolved<N>> {
    match definition {
      TokenValue::Color(definition) => self.leaf(self.winning_media(definition)),
      TokenValue::Number(definition) => self.leaf(self.winning_media(definition)),
      TokenValue::Length(definition) => self.leaf(self.winning_media(definition)),
      TokenValue::Dimension(definition) => self.leaf(self.winning_media(definition)),
      TokenValue::Text(definition) => self.leaf(self.winning_media(definition)),
    }
  }

  /// Runs `resolve`, a token definition's resolution, one definition deeper; `None`
  /// past the deepest chain of definitions naming tokens a platform follows, so a
  /// token defined through itself resolves to nothing.
  pub fn nested<R>(resolve: impl FnOnce() -> Option<R>) -> Option<R> {
    /// Steps back out of one definition when dropped.
    struct Leave;

    impl Drop for Leave {
      fn drop(&mut self) {
        DEPTH.with(|depth| depth.set(depth.get() - 1));
      }
    }

    if DEPTH.with(|depth| depth.replace(depth.get() + 1)) >= MAX_DEPTH {
      DEPTH.with(|depth| depth.set(depth.get() - 1));

      return None;
    }

    let _leave = Leave;

    resolve()
  }

  /// The value of an expression at the node.
  #[must_use]
  pub fn expr(&self, node: &ExprNode) -> Option<Resolved<N>> {
    let env = ExprEnv {
      viewport: Size::new(
        self.environment.viewport.width.to_f32(),
        self.environment.viewport.height.to_f32(),
      ),
      container: self
        .container(None)
        .map(|size| Size::new(size.width.to_f32(), size.height.to_f32())),
      font_size: self.font_size,
      safe_area: Inset {
        top: self.environment.safe_area.top.to_f32(),
        right: self.environment.safe_area.right.to_f32(),
        bottom: self.environment.safe_area.bottom.to_f32(),
        left: self.environment.safe_area.left.to_f32(),
      },
    };

    node
      .evaluate(&env, &|name| {
        self.token(name).and_then(Resolved::into_expr_value)
      })
      .map(Resolved::from)
  }

  /// A static leaf resolved at the node: a fixed value as itself, a token through its
  /// definition, an expression evaluated.
  #[must_use]
  pub fn leaf<T: Clone + Into<Resolved<N>>>(&self, leaf: &StaticLeaf<T>) -> Option<Resolved<N>> {
    match leaf {
      StaticLeaf::Fixed(fixed) => Some(fixed.clone().into()),
      StaticLeaf::Token(name) => self.token(*name),
      StaticLeaf::Expr(expr) => self.expr(expr.node()),
    }
  }

  /// A color leaf resolved at the node.
  #[must_use]
  pub fn color_leaf(&self, leaf: &StaticLeaf<Color>) -> Option<Color> {
    self.leaf(leaf).and_then(Resolved::into_color)
  }

  /// The winning leaf of a platform's box value, resolved to the property's type: a
  /// static leaf through tokens and expressions, a slot through the leaf last written
  /// into it.
  #[must_use]
  pub fn box_value<T: IntoBoxValue<N, X>>(
    &self,
    value: &StaticValue<T, Condition<N>>,
  ) -> Option<T> {
    match self.winning(value) {
      PlatformLeaf::Static(leaf) => self.keyframe_box_value(leaf),
      PlatformLeaf::Slot(slot) => self.box_leaf(&(self.slot)(*slot)?, |value| match value {
        SlotValue::Box(value) => T::from_box_value(value),
        SlotValue::Run(_) => None,
      }),
    }
  }

  /// The winning leaf of a platform's run value, resolved to the property's type: a
  /// static leaf through tokens and expressions, a slot through the leaf last written
  /// into it.
  #[must_use]
  pub fn run_value<T: IntoRunValue<X>>(&self, value: &StaticValue<T, Condition<N>>) -> Option<T> {
    match self.winning(value) {
      PlatformLeaf::Static(leaf) => self.keyframe_run_value(leaf),
      PlatformLeaf::Slot(slot) => self.run_leaf(&(self.slot)(*slot)?, |value| match value {
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
      StaticLeaf::Token(name) => self
        .token(*name)
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
      StaticLeaf::Token(name) => self
        .token(*name)
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

  /// A platform's box style with every property resolved.
  pub fn box_style(&self, style: &BoxStyle<N, Static<Condition<N>>, X>) -> BoxStyle<N, Plain, X> {
    let mut visitor = ResolveBox {
      resolver: self,
      source: style,
      target: BoxStyle::default(),
    };

    BoxStyle::<N, Static<Condition<N>>, X>::for_each_field(&mut visitor);

    visitor.target
  }

  /// A platform's run style with every property resolved.
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

/// Where a runtime-resolving platform's [`ResolutionContext`] reads what the scene does
/// not hold: the environment, container sizes, slots and whether text takes sizes.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is no source a platform's resolution reads",
  label = "a resolution source gives the environment, container sizes, slots and whether text takes sizes"
)]
pub trait ResolutionSource<P: Platform> {
  /// The reader's environment.
  fn environment(&self) -> Environment<P::Scalar>;

  /// The size last measured for the named container `name`.
  fn container_size(&self, name: ContainerName) -> Option<Size<P::Scalar>>;

  /// The latest derived write into `slot` on `node`.
  fn slot(&self, node: NodeId, slot: Slot) -> Option<SlotLeaf<P::Scalar, P::StyleExt>>;

  /// Whether the medium draws text at sizes other than its
  /// [`FONT_SIZE`](Platform::FONT_SIZE), the
  /// typography's [`SIZE`](typography::SIZE) then giving the font size.
  fn sizes_text(&self) -> bool;

  /// A count that moves whenever the environment, a container's size or whether text
  /// takes sizes changes: what a token resolved to where it is defined is kept until it
  /// moves.
  fn generation(&self) -> u64;
}

/// What one node's properties resolve against: the environment, the node's interaction
/// and the named containers above it, from the scene and a platform's `source`.
#[must_use]
pub struct ResolutionContext<'a, P: Platform, S> {
  source: &'a S,
  node: Option<NodeId>,
  environment: Environment<P::Scalar>,
  interaction: Interaction,
  interaction_read: Cell<bool>,
  containers: Vec<(ContainerName, Option<Size<P::Scalar>>)>,
}

impl<P: Platform, S> std::fmt::Debug for ResolutionContext<'_, P, S> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ResolutionContext")
      .field("node", &self.node)
      .finish_non_exhaustive()
  }
}

impl<'a, P, S> ResolutionContext<'a, P, S>
where
  P: Platform<TokenValue = TokenValue<<P as Platform>::Scalar>>,
  S: ResolutionSource<P>,
{
  /// The context of `node`; `None` for a stale id.
  pub fn of(source: &'a S, node: NodeId) -> Option<Self> {
    let (interaction, names) = Scene::<P>::read(|scene| {
      scene.record(node)?;

      let mut names = Vec::new();
      let mut current = scene.placed_parent(node);

      while let Some(ancestor) = current {
        if let Some(name) = scene.record(ancestor).and_then(NodeRecord::container) {
          names.push(name);
        }

        current = scene.placed_parent(ancestor);
      }

      Some((scene.interaction(node), names))
    })?;

    Some(Self {
      source,
      node: Some(node),
      environment: source.environment(),
      interaction,
      interaction_read: Cell::new(false),
      containers: names
        .into_iter()
        .map(|name| (name, source.container_size(name)))
        .collect(),
    })
  }

  /// The context of the root the root definitions hang off: the platform's environment,
  /// with no interaction and no container above it.
  pub fn root(source: &'a S) -> Self {
    Self {
      source,
      node: None,
      environment: source.environment(),
      interaction: Interaction::default(),
      interaction_read: Cell::new(false),
      containers: Vec::new(),
    }
  }

  /// The context a token's definition resolves in: the defining node's, or the
  /// root's.
  fn of_scope(source: &'a S, scope: TokenScope) -> Option<Self> {
    match scope {
      TokenScope::Node(node) => Self::of(source, node),
      TokenScope::Root => Some(Self::root(source)),
    }
  }

  /// Where `name` in force here is defined; `None` where nothing defines it.
  fn token_scope(&self, name: TokenName) -> Option<TokenScope> {
    match self.node {
      Some(node) => Scene::<P>::with(|scene| scene.token_scope(node, name)),
      None => {
        Scene::<P>::read(|scene| scene.root_definitions().get(name).map(|_| TokenScope::Root))
      }
    }
  }

  /// The value of the token `name` in force here, resolved where it is defined, once
  /// per defining node until the definitions or the source's generation move.
  fn token(&self, name: TokenName) -> Option<Resolved<P::Scalar>> {
    let scope = self.token_scope(name)?;
    let generation = self.source.generation();

    if let Some(value) = Scene::<P>::with(|scene| scene.resolved_token(scope, name, generation)) {
      return value;
    }

    let value = Resolver::<P::Scalar, P::StyleExt>::nested(|| {
      let definition = Scene::<P>::read(|scene| scene.definition_on(scope, name))?;

      Self::of_scope(self.source, scope)?.with_resolver(|resolver| resolver.definition(&definition))
    });

    Scene::<P>::with(|scene| scene.keep_resolved_token(scope, name, generation, value.clone()));

    value
  }

  /// The font size in force here: the typography's [`SIZE`](typography::SIZE), resolved
  /// where it is defined once per defining node, where the source sizes text, else the
  /// platform's [`FONT_SIZE`](Platform::FONT_SIZE).
  fn font_size(&self) -> f32 {
    if !self.source.sizes_text() {
      return P::FONT_SIZE;
    }

    let size = typography::SIZE.name();
    let Some(scope) = self.token_scope(size)
    else {
      return P::FONT_SIZE;
    };
    let generation = self.source.generation();

    if let Some(size) = Scene::<P>::with(|scene| scene.resolved_font_size(scope, generation)) {
      return size;
    }

    let resolved = Resolver::<P::Scalar, P::StyleExt>::nested(|| {
      let definition = Scene::<P>::read(|scene| scene.definition_on(scope, size))?;

      Self::of_scope(self.source, scope)?
        .with_resolver_at(P::FONT_SIZE, |resolver| resolver.definition(&definition))
    })
    .and_then(|resolved| match resolved {
      Resolved::Length(size) => Some(size),
      _ => None,
    })
    .unwrap_or(P::FONT_SIZE);

    Scene::<P>::with(|scene| scene.keep_font_size(scope, generation, resolved));

    resolved
  }

  /// Whether a resolution in this context has read the interaction: what one that has
  /// not resolved holds whatever the interaction becomes.
  #[inline(always)]
  #[must_use]
  pub const fn read_interaction(&self) -> bool {
    self.interaction_read.get()
  }

  /// Runs `operation` with the resolver of this context, reading tokens where they are
  /// defined and slots through the source.
  pub fn with_resolver<R>(
    &self,
    operation: impl FnOnce(&Resolver<'_, P::Scalar, P::StyleExt>) -> R,
  ) -> R {
    self.with_resolver_at(self.font_size(), operation)
  }

  /// Runs `operation` with the resolver of this context, a multiple of the font size
  /// evaluating against `font_size`.
  fn with_resolver_at<R>(
    &self,
    font_size: f32,
    operation: impl FnOnce(&Resolver<'_, P::Scalar, P::StyleExt>) -> R,
  ) -> R {
    let token = |name| self.token(name);
    let slot = |slot| self.source.slot(self.node?, slot);

    operation(&Resolver {
      environment: self.environment,
      interaction: self.interaction,
      interaction_read: &self.interaction_read,
      containers: &self.containers,
      font_size,
      token: &token,
      slot: &slot,
    })
  }
}
