//! The one resolution of a node's style: each property's conditions against the
//! terminal environment and the node's interaction pick a leaf, and the leaf resolves
//! through tokens, expressions and slots to a plain value.

use skene_core::{
  Color,
  ColorScheme,
  Dimension,
  Size,
  next::{
    Annotation,
    BoxField,
    BoxFieldVisitor,
    BoxStyle,
    BoxValue,
    Condition,
    Conditional,
    ContainerName,
    DriverLeaf,
    ExprEnv,
    ExprNode,
    ExprValue,
    IntoBoxValue,
    IntoRunValue,
    Leaf,
    Media,
    MediaEnv,
    NodeId,
    RunField,
    RunFieldVisitor,
    RunStyle,
    Scalar,
    Scene,
    Slot,
    State,
    StaticLeaf,
    StaticValue,
    TokenName,
    Value,
  },
};
use skene_reactive::untrack;

use crate::{
  BorderGlyphs,
  TerminalExt,
  Tui,
  ext::{BOLD_WEIGHT, TerminalValue},
  store::{Handle, Interaction, Kind, PlainBox, PlainRun, SlotValue, Store},
  token::TokenValue,
};

/// How deep a chain of tokens and expressions naming tokens is followed.
const MAX_DEPTH: usize = 32;

/// The font size expressions in `em` evaluate against: one cell.
const EM: f32 = 1.;

/// A value a leaf resolved to, erased over the property.
#[derive(Debug, Clone, PartialEq)]
enum Resolved {
  Color(Color),
  Number(f32),
  Dimension(Dimension<u32>),
  Text(String),
}

impl Resolved {
  fn into_box_value(self) -> Option<BoxValue<u32, TerminalExt>> {
    match self {
      Self::Color(color) => Some(BoxValue::Color(color)),
      Self::Number(number) => Some(BoxValue::Number(number)),
      Self::Dimension(dimension) => Some(BoxValue::Dimension(dimension)),
      Self::Text(_) => None,
    }
  }

  fn into_run_value(self) -> Option<skene_core::next::RunValue<TerminalExt>> {
    match self {
      Self::Color(color) => Some(skene_core::next::RunValue::Color(color)),
      Self::Number(number) => Some(skene_core::next::RunValue::Extension(TerminalValue::Bold(
        number >= BOLD_WEIGHT,
      ))),
      Self::Dimension(_) | Self::Text(_) => None,
    }
  }

  fn into_expr_value(self) -> Option<ExprValue> {
    match self {
      Self::Color(color) => Some(ExprValue::Color(color)),
      Self::Number(number) => Some(ExprValue::Number(number)),
      Self::Dimension(Dimension::Length(length)) => Some(ExprValue::Length(length.to_f32())),
      Self::Dimension(Dimension::Fraction(_) | Dimension::Auto) | Self::Text(_) => None,
    }
  }
}

impl From<ExprValue> for Resolved {
  fn from(value: ExprValue) -> Self {
    match value {
      ExprValue::Color(color) => Self::Color(color),
      ExprValue::Number(number) => Self::Number(number),
      ExprValue::Length(length) => Self::Dimension(Dimension::Length(u32::from_f32(length))),
    }
  }
}

/// What one node's properties resolve against: the environment, the node's
/// interaction, and the named containers above it.
pub(crate) struct Env<'a> {
  store: &'a Store,
  handle: Handle,
  node: Option<NodeId>,
  viewport: Size<u32>,
  scheme: ColorScheme,
  interaction: Interaction,
  containers: Vec<(ContainerName, Option<Size<u32>>)>,
}

impl<'a> Env<'a> {
  /// The environment of `handle` in `store`.
  pub(crate) fn of(store: &'a Store, handle: Handle) -> Self {
    Self {
      store,
      handle,
      node: store.record(handle).and_then(|record| record.node),
      viewport: store.viewport(),
      scheme: store.scheme(),
      interaction: store.interaction(handle),
      containers: store.containers_above(handle),
    }
  }

  /// The size of the nearest container of `name`, of any name for `None`.
  fn container(&self, name: Option<ContainerName>) -> Option<Size<u32>> {
    self
      .containers
      .iter()
      .find(|(container, _)| name.is_none_or(|name| name == *container))
      .and_then(|(_, size)| *size)
  }

  fn state(&self, state: State) -> bool {
    if state.is_control_state() {
      return self
        .interaction
        .control
        .is_some_and(|control| control.holds(state));
    }

    match state {
      State::Hovered => self.interaction.hovered,
      State::Focused | State::FocusVisible => self.interaction.focused,
      State::Pressed => self.interaction.pressed,
      State::Checked | State::Indeterminate | State::Disabled | State::Invalid | State::Open => {
        false
      }
    }
  }

  fn with_media_env<R>(&self, operation: impl FnOnce(&MediaEnv<'_, u32>) -> R) -> R {
    let container = |name: Option<ContainerName>| self.container(name);

    operation(&MediaEnv {
      viewport: self.viewport,
      scheme: self.scheme,
      reduced_motion: false,
      reduced_transparency: false,
      high_contrast: false,
      container: &container,
    })
  }

  /// The leaf of `value` in force: the last variant whose condition holds, else the
  /// base.
  fn winning<'v, L>(&self, value: &'v Conditional<L, Condition<u32>>) -> &'v L {
    self.with_media_env(|env| {
      value
        .variants()
        .iter()
        .rev()
        .find(|(condition, _)| condition.holds_in(env, &|state| self.state(state)))
        .map_or(value.base(), |(_, leaf)| leaf)
    })
  }

  /// The leaf of a definition in force: the last variant whose media condition holds,
  /// else the base.
  fn winning_media<'v, L>(&self, definition: &'v Conditional<L, Media<u32>>) -> &'v L {
    self.with_media_env(|env| {
      definition
        .variants()
        .iter()
        .rev()
        .find(|(condition, _)| condition.holds_in(env))
        .map_or(definition.base(), |(_, leaf)| leaf)
    })
  }

  /// The value of the token `name` in force at the node, through its definition.
  fn token(&self, name: TokenName, depth: usize) -> Option<Resolved> {
    if depth > MAX_DEPTH {
      return None;
    }

    let node = self.node?;
    let value = Scene::<Tui>::with(|scene| scene.resolve_token(node, name))?;

    match value {
      TokenValue::Color(definition) => self.static_leaf(self.winning_media(&definition), depth),
      TokenValue::Number(definition) => self.static_leaf(self.winning_media(&definition), depth),
      TokenValue::Dimension(definition) => self.static_leaf(self.winning_media(&definition), depth),
      TokenValue::Text(definition) => self.static_leaf(self.winning_media(&definition), depth),
    }
  }

  /// The value of an expression at the node.
  fn expr(&self, node: &ExprNode, depth: usize) -> Option<Resolved> {
    if depth > MAX_DEPTH {
      return None;
    }

    let env = ExprEnv {
      viewport: Size::new(self.viewport.width.to_f32(), self.viewport.height.to_f32()),
      container: self
        .container(None)
        .map(|size| Size::new(size.width.to_f32(), size.height.to_f32())),
      em: EM,
    };

    node
      .evaluate(&env, &|name| {
        self
          .token(name, depth + 1)
          .and_then(Resolved::into_expr_value)
      })
      .map(Resolved::from)
  }

  fn static_leaf<T: Clone + Into<Resolved>>(
    &self,
    leaf: &StaticLeaf<T>,
    depth: usize,
  ) -> Option<Resolved> {
    match leaf {
      StaticLeaf::Fixed(fixed) => Some(fixed.clone().into()),
      StaticLeaf::Token(reference) => self.token(reference.name(), depth + 1),
      StaticLeaf::Expr(expr) => self.expr(expr.node(), depth + 1),
    }
  }

  /// The latest value written into `slot` on the node.
  fn slot(&self, slot: Slot) -> Option<SlotValue> {
    self.store.record(self.handle)?.slots.get(&slot).cloned()
  }

  /// The color leaf `leaf` resolves to at the node.
  pub(crate) fn color_leaf(&self, leaf: &StaticLeaf<Color>) -> Option<Color> {
    match self.static_leaf(leaf, 0)? {
      Resolved::Color(color) => Some(color),
      Resolved::Number(_) | Resolved::Dimension(_) | Resolved::Text(_) => None,
    }
  }

  /// The winning leaf of a driver's value, resolved to the property's type.
  fn box_value<T: IntoBoxValue<u32, TerminalExt>>(
    &self,
    value: &StaticValue<T, Condition<u32>>,
  ) -> Option<T> {
    match self.winning(value) {
      DriverLeaf::Static(StaticLeaf::Fixed(fixed)) => Some(fixed.clone()),
      DriverLeaf::Static(StaticLeaf::Token(reference)) => self
        .token(reference.name(), 0)
        .and_then(Resolved::into_box_value)
        .and_then(T::from_box_value),
      DriverLeaf::Static(StaticLeaf::Expr(expr)) => self
        .expr(expr.node(), 0)
        .and_then(Resolved::into_box_value)
        .and_then(T::from_box_value),
      DriverLeaf::Slot(slot) => match self.slot(*slot)? {
        SlotValue::Box(value) => T::from_box_value(value),
        SlotValue::Run(_) => None,
      },
    }
  }

  /// The winning leaf of a driver's run value, resolved to the property's type.
  fn run_value<T: IntoRunValue<TerminalExt>>(
    &self,
    value: &StaticValue<T, Condition<u32>>,
  ) -> Option<T> {
    match self.winning(value) {
      DriverLeaf::Static(StaticLeaf::Fixed(fixed)) => Some(fixed.clone()),
      DriverLeaf::Static(StaticLeaf::Token(reference)) => self
        .token(reference.name(), 0)
        .and_then(Resolved::into_run_value)
        .and_then(T::from_run_value),
      DriverLeaf::Static(StaticLeaf::Expr(expr)) => self
        .expr(expr.node(), 0)
        .and_then(Resolved::into_run_value)
        .and_then(T::from_run_value),
      DriverLeaf::Slot(slot) => match self.slot(*slot)? {
        SlotValue::Run(value) => T::from_run_value(value),
        SlotValue::Box(_) => None,
      },
    }
  }

  /// The winning leaf of a declared value, resolved to the property's type; a derived
  /// leaf is read once, untracked.
  fn declared_value<T: IntoBoxValue<u32, TerminalExt>>(
    &self,
    value: &Value<T, Condition<u32>>,
  ) -> Option<T> {
    match self.winning(value) {
      Leaf::Static(StaticLeaf::Fixed(fixed)) => Some(fixed.clone()),
      Leaf::Static(StaticLeaf::Token(reference)) => self
        .token(reference.name(), 0)
        .and_then(Resolved::into_box_value)
        .and_then(T::from_box_value),
      Leaf::Static(StaticLeaf::Expr(expr)) => self
        .expr(expr.node(), 0)
        .and_then(Resolved::into_box_value)
        .and_then(T::from_box_value),
      Leaf::Derived(compute) => Some(untrack(|| compute())),
    }
  }

  /// The per-child layout properties a wrapper annotated the node with, resolved into
  /// an otherwise empty box style.
  pub(crate) fn resolve_run_layout(&self) -> PlainBox {
    let mut layout = PlainBox::default();
    let Some(annotation) = self
      .node
      .and_then(|node| Scene::<Tui>::with(|scene| scene.record(node)?.annotation().cloned()))
    else {
      return layout;
    };
    let Annotation {
      flex,
      self_alignment,
      self_horizontal_alignment,
      pinned,
      grid_row_placement,
      grid_column_placement,
    } = annotation;

    layout.flex = flex.as_ref().and_then(|value| self.declared_value(value));
    layout.self_alignment = self_alignment
      .as_ref()
      .and_then(|value| self.declared_value(value));
    layout.self_horizontal_alignment = self_horizontal_alignment
      .as_ref()
      .and_then(|value| self.declared_value(value));
    layout.pinned = pinned.as_ref().and_then(|value| self.declared_value(value));
    layout.grid_row_placement = grid_row_placement
      .as_ref()
      .and_then(|value| self.declared_value(value));
    layout.grid_column_placement = grid_column_placement
      .as_ref()
      .and_then(|value| self.declared_value(value));

    layout
  }

  /// The box style of the record, every property resolved; with its border's ink.
  pub(crate) fn resolve_box(&self) -> Option<(PlainBox, Option<Color>)> {
    let record = self.store.record(self.handle)?;
    let source = &record.kind.box_record()?.props.style;
    let mut visitor = ResolveBox {
      env: self,
      source,
      target: PlainBox::default(),
    };

    BoxStyle::<u32, skene_core::next::Static<Condition<u32>>, TerminalExt>::for_each_field(
      &mut visitor,
    );

    let mut style = visitor.target;

    if style.ext.border_glyphs.is_none()
      && style.ext.corner_radius.is_some_and(|radius| radius > 0.)
    {
      style.ext.border_glyphs = Some(BorderGlyphs::ROUNDED);
    }

    let border_ink = style
      .border
      .as_ref()
      .and_then(|border| self.color_leaf(&border.color));

    Some((style, border_ink))
  }

  /// The run style of the record, every property resolved.
  pub(crate) fn resolve_run(&self) -> Option<PlainRun> {
    let record = self.store.record(self.handle)?;
    let Kind::Run { props, .. } = &record.kind
    else {
      return None;
    };
    let mut visitor = ResolveRun {
      env: self,
      source: &props.style,
      target: PlainRun::default(),
    };

    RunStyle::<skene_core::next::Static<Condition<u32>>, TerminalExt>::for_each_field(&mut visitor);

    let mut style = visitor.target;

    if style.ext.bold.is_none() {
      style.ext.bold = style.ext.weight.map(|weight| weight >= BOLD_WEIGHT);
    }

    Some(style)
  }
}

impl From<Color> for Resolved {
  #[inline(always)]
  fn from(color: Color) -> Self {
    Self::Color(color)
  }
}

impl From<f32> for Resolved {
  #[inline(always)]
  fn from(number: f32) -> Self {
    Self::Number(number)
  }
}

impl From<Dimension<u32>> for Resolved {
  #[inline(always)]
  fn from(dimension: Dimension<u32>) -> Self {
    Self::Dimension(dimension)
  }
}

impl From<String> for Resolved {
  #[inline(always)]
  fn from(text: String) -> Self {
    Self::Text(text)
  }
}

/// Resolves every box property of the source into the target.
struct ResolveBox<'a, 'e> {
  env: &'a Env<'e>,
  source: &'a BoxStyle<u32, skene_core::next::Static<Condition<u32>>, TerminalExt>,
  target: PlainBox,
}

impl BoxFieldVisitor<u32, TerminalExt> for ResolveBox<'_, '_> {
  fn field<F: BoxField<u32, TerminalExt>>(&mut self) {
    if let Some(value) = F::get(self.source) {
      *F::get_mut(&mut self.target) = self.env.box_value(value);
    }
  }
}

/// Resolves every run property of the source into the target.
struct ResolveRun<'a, 'e> {
  env: &'a Env<'e>,
  source: &'a RunStyle<skene_core::next::Static<Condition<u32>>, TerminalExt>,
  target: PlainRun,
}

impl RunFieldVisitor<TerminalExt> for ResolveRun<'_, '_> {
  fn field<F: RunField<TerminalExt>>(&mut self) {
    if let Some(value) = F::get(self.source) {
      *F::get_mut(&mut self.target) = self.env.run_value(value);
    }
  }
}
