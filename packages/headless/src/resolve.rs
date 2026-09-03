//! The driver's resolution of a property: the conditions against the environment and
//! the node's state pick a leaf, and the leaf resolves through tokens, expressions and
//! slots to a value.

use skene_core::{
  Color,
  ColorScheme,
  Dimension,
  Size,
  next::{
    BoxValue,
    BoxVisitor,
    Condition,
    Conditional,
    ContainerName,
    DriverLeaf,
    ExprEnv,
    ExprNode,
    ExprValue,
    IntoBoxValue,
    IntoRunValue,
    Media,
    MediaEnv,
    NoProperty,
    NodeId,
    NodeRecord,
    PropertyKey,
    RunValue,
    RunVisitor,
    Scene,
    Slot,
    State,
    Static,
    StaticLeaf,
    StaticValue,
    TokenName,
  },
};

use crate::{
  Headless,
  HeadlessTokenValue,
  store::{DEFAULT_VIEWPORT, Kind, SlotValue},
  with_store,
};

/// How deep a chain of tokens and expressions naming tokens is followed.
const MAX_DEPTH: usize = 32;

/// The font size expressions in `em` evaluate against.
const EM: f32 = 16.;

/// A value a leaf resolved to, erased over the property.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum Resolved {
  /// A color.
  Color(Color),
  /// A unitless number.
  Number(f32),
  /// A dimension.
  Dimension(Dimension<f32>),
}

impl Resolved {
  fn into_box_value(self) -> BoxValue<f32> {
    match self {
      Self::Color(color) => BoxValue::Color(color),
      Self::Number(number) => BoxValue::Number(number),
      Self::Dimension(dimension) => BoxValue::Dimension(dimension),
    }
  }

  fn into_run_value(self) -> Option<RunValue> {
    match self {
      Self::Color(color) => Some(RunValue::Color(color)),
      Self::Number(_) | Self::Dimension(_) => None,
    }
  }

  fn into_expr_value(self) -> Option<ExprValue> {
    match self {
      Self::Color(color) => Some(ExprValue::Color(color)),
      Self::Number(number) => Some(ExprValue::Number(number)),
      Self::Dimension(Dimension::Length(length)) => Some(ExprValue::Length(length)),
      Self::Dimension(Dimension::Fraction(_) | Dimension::Auto) => None,
    }
  }
}

impl From<ExprValue> for Resolved {
  fn from(value: ExprValue) -> Self {
    match value {
      ExprValue::Color(color) => Self::Color(color),
      ExprValue::Number(number) => Self::Number(number),
      ExprValue::Length(length) => Self::Dimension(Dimension::Length(length)),
    }
  }
}

impl From<Color> for Resolved {
  fn from(color: Color) -> Self {
    Self::Color(color)
  }
}

impl From<f32> for Resolved {
  fn from(number: f32) -> Self {
    Self::Number(number)
  }
}

impl From<Dimension<f32>> for Resolved {
  fn from(dimension: Dimension<f32>) -> Self {
    Self::Dimension(dimension)
  }
}

/// What one node's properties resolve against: the environment, the node's state, and
/// the named containers above it.
pub struct Env {
  node: NodeId,
  viewport: Size<f32>,
  scheme: ColorScheme,
  reduced_motion: bool,
  hovered: bool,
  focused: bool,
  containers: Vec<(ContainerName, Option<Size<f32>>)>,
}

impl Env {
  /// The environment of `node`; `None` for a stale id.
  pub fn of(node: NodeId) -> Option<Self> {
    let (handle, names) = Scene::<Headless>::with(|scene| {
      let handle = *scene.record(node)?.sink();
      let mut names = Vec::new();
      let mut current = scene.parent(node);

      while let Some(ancestor) = current {
        if let Some(name) = scene.record(ancestor).and_then(NodeRecord::container) {
          names.push(name);
        }

        current = scene.parent(ancestor);
      }

      Some((handle, names))
    })?;

    with_store(|store| {
      let record = store.record(handle)?;

      Some(Self {
        node,
        viewport: store.viewport.unwrap_or(DEFAULT_VIEWPORT),
        scheme: store.scheme.unwrap_or(ColorScheme::Light),
        reduced_motion: store.reduced_motion.unwrap_or(false),
        hovered: record.hovered,
        focused: store.focused == Some(handle),
        containers: names
          .into_iter()
          .map(|name| (name, store.container_sizes.get(&name).copied()))
          .collect(),
      })
    })
  }

  /// The size of the nearest container of `name`, of any name for `None`.
  fn container(&self, name: Option<ContainerName>) -> Option<Size<f32>> {
    self
      .containers
      .iter()
      .find(|(container, _)| name.is_none_or(|name| name == *container))
      .and_then(|(_, size)| *size)
  }

  fn state(&self, state: State) -> bool {
    match state {
      State::Hovered => self.hovered,
      State::Focused | State::FocusVisible => self.focused,
      State::Pressed
      | State::Checked
      | State::Indeterminate
      | State::Disabled
      | State::Invalid
      | State::Open => false,
    }
  }

  fn with_media_env<R>(&self, operation: impl FnOnce(&MediaEnv<'_, f32>) -> R) -> R {
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
  pub fn winning<'a, L>(&self, value: &'a Conditional<L, Condition<f32>>) -> &'a L {
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
  fn winning_media<'a, L>(&self, definition: &'a Conditional<L, Media<f32>>) -> &'a L {
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
  pub fn token(&self, name: TokenName, depth: usize) -> Option<Resolved> {
    if depth > MAX_DEPTH {
      return None;
    }

    let value = Scene::<Headless>::with(|scene| scene.resolve_token(self.node, name))?;

    match value {
      HeadlessTokenValue::Color(definition) => {
        self.static_leaf(self.winning_media(&definition), depth)
      }
      HeadlessTokenValue::Number(definition) => {
        self.static_leaf(self.winning_media(&definition), depth)
      }
      HeadlessTokenValue::Dimension(definition) => {
        self.static_leaf(self.winning_media(&definition), depth)
      }
    }
  }

  /// The value of an expression at the node.
  pub fn expr(&self, node: &ExprNode, depth: usize) -> Option<Resolved> {
    if depth > MAX_DEPTH {
      return None;
    }

    let env = ExprEnv {
      viewport: self.viewport,
      container: self.container(None),
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
  pub fn slot(&self, slot: Slot) -> Option<SlotValue> {
    let handle =
      Scene::<Headless>::with(|scene| scene.record(self.node).map(|record| *record.sink()))?;

    with_store(|store| store.record(handle)?.slots.get(&slot).cloned())
  }

  /// The winning leaf of a driver's value, resolved to a value of the property's
  /// erasure.
  fn driver_leaf<T, V>(
    &self,
    value: &StaticValue<T, Condition<f32>>,
    fixed: impl FnOnce(T) -> V,
    resolved: impl FnOnce(Resolved) -> Option<V>,
    slot: impl FnOnce(SlotValue) -> Option<V>,
  ) -> Option<V>
  where
    T: Clone,
  {
    match self.winning(value) {
      DriverLeaf::Static(StaticLeaf::Fixed(value)) => Some(fixed(value.clone())),
      DriverLeaf::Static(StaticLeaf::Token(reference)) => {
        self.token(reference.name(), 0).and_then(resolved)
      }
      DriverLeaf::Static(StaticLeaf::Expr(expr)) => self.expr(expr.node(), 0).and_then(resolved),
      DriverLeaf::Slot(index) => self.slot(*index).and_then(slot),
    }
  }
}

/// Finds the box property `key` and resolves it.
struct FindBox<'a> {
  key: PropertyKey<NoProperty>,
  env: &'a Env,
  found: Option<BoxValue<f32>>,
}

impl BoxVisitor<f32, Static<Condition<f32>>, ()> for FindBox<'_> {
  fn value<T: IntoBoxValue<f32, ()>>(
    &mut self,
    key: PropertyKey<NoProperty>,
    value: &Option<StaticValue<T, Condition<f32>>>,
  ) {
    if key != self.key {
      return;
    }

    let Some(value) = value
    else {
      return;
    };

    self.found = self.env.driver_leaf(
      value,
      IntoBoxValue::into_box_value,
      |resolved| Some(resolved.into_box_value()),
      |slot| match slot {
        SlotValue::Box(value) => Some(value),
        SlotValue::Run(_) => None,
      },
    );
  }
}

/// Finds the run property `key` and resolves it.
struct FindRun<'a> {
  key: PropertyKey<NoProperty>,
  env: &'a Env,
  found: Option<RunValue>,
}

impl RunVisitor<Static<Condition<f32>>, ()> for FindRun<'_> {
  fn value<T: IntoRunValue<()>>(
    &mut self,
    key: PropertyKey<NoProperty>,
    value: &Option<StaticValue<T, Condition<f32>>>,
  ) {
    if key != self.key {
      return;
    }

    let Some(value) = value
    else {
      return;
    };

    self.found = self.env.driver_leaf(
      value,
      IntoRunValue::into_run_value,
      Resolved::into_run_value,
      |slot| match slot {
        SlotValue::Run(value) => Some(value),
        SlotValue::Box(_) => None,
      },
    );
  }
}

/// The property `key` of `node` resolved: a box's as a [`BoxValue`], a run's as a
/// [`RunValue`].
pub fn resolve(node: NodeId, key: PropertyKey<NoProperty>) -> Option<SlotValue> {
  let env = Env::of(node)?;
  let handle = Scene::<Headless>::with(|scene| scene.record(node).map(|record| *record.sink()))?;
  let kind = with_store(|store| match &store.record(handle)?.kind {
    Kind::Box(props) => Some(Kind::Box(props.clone())),
    Kind::Run(props) => Some(Kind::Run(props.clone())),
    Kind::Anchor | Kind::Scope => None,
  })?;

  match kind {
    Kind::Box(props) => {
      let mut find = FindBox {
        key,
        env: &env,
        found: None,
      };

      props.style.visit(&mut find);

      find.found.map(SlotValue::Box)
    }
    Kind::Run(props) => {
      let mut find = FindRun {
        key,
        env: &env,
        found: None,
      };

      props.style.visit(&mut find);

      find.found.map(SlotValue::Run)
    }
    Kind::Anchor | Kind::Scope => None,
  }
}
