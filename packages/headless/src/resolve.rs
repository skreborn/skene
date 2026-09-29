//! The platform's resolution of a property: the environment and the node's interaction
//! feed the shared [`Resolver`], and the presented value lays the node's motion over
//! what it resolved to.

use skene_common::typography;
use skene_core::{
  Animation,
  BoxKeyframe,
  ContainerName,
  Environment,
  Interaction,
  NoProperty,
  NodeId,
  NodeRecord,
  PropertyKey,
  Resolved,
  Resolver,
  RunKeyframe,
  Scene,
  Size,
  TokenName,
  Transition,
};

use crate::{Headless, SlotValue, TokenValue, store::Kind, with_store};

/// The font size an `em` evaluates against where no typography defines one.
const EM: f32 = 16.;

/// What one node's properties resolve against: the environment, the node's interaction
/// and the named containers above it.
pub(crate) struct Env {
  /// The node, or `None` for the root the root definitions hang off.
  node: Option<NodeId>,
  /// The reader's environment.
  environment: Environment<f32>,
  /// The node's interaction.
  interaction: Interaction,
  /// The named containers above the node, nearest first, with their sizes.
  containers: Vec<(ContainerName, Option<Size<f32>>)>,
}

impl Env {
  /// The environment of `node`; `None` for a stale id.
  pub(crate) fn of(node: NodeId) -> Option<Self> {
    let (handle, interaction, names) = Scene::<Headless>::with(|scene| {
      let handle = *scene.record(node)?.sink();
      let interaction = scene.interaction(node);
      let mut names = Vec::new();
      let mut current = scene.parent(node);

      while let Some(ancestor) = current {
        if let Some(name) = scene.record(ancestor).and_then(NodeRecord::container) {
          names.push(name);
        }

        current = scene.parent(ancestor);
      }

      Some((handle, interaction, names))
    })?;

    with_store(|store| {
      store.record(handle)?;

      Some(Self {
        node: Some(node),
        environment: store.environment(),
        interaction,
        containers: names
          .into_iter()
          .map(|name| (name, store.container_sizes.get(&name).copied()))
          .collect(),
      })
    })
  }

  /// The environment of the root: the platform's, with no container above it.
  fn root() -> Self {
    with_store(|store| Self {
      node: None,
      environment: store.environment(),
      interaction: Interaction::default(),
      containers: Vec::new(),
    })
  }

  /// The environment a token's definition resolves in: the defining node's, else the
  /// root's.
  fn of_scope(scope: Option<NodeId>) -> Option<Self> {
    match scope {
      Some(node) => Self::of(node),
      None => Some(Self::root()),
    }
  }

  /// The definition of `name` in force here, with the node defining it.
  fn scoped_token(&self, name: TokenName) -> Option<(Option<NodeId>, TokenValue)> {
    match self.node {
      Some(node) => Scene::<Headless>::with(|scene| scene.resolve_scoped_token(node, name)),
      None => Scene::<Headless>::with(|scene| scene.root_definitions().get(name).cloned())
        .map(|value| (None, value)),
    }
  }

  /// The value of the token `name` in force here, resolved where it is defined.
  fn token(&self, name: TokenName) -> Option<Resolved<f32>> {
    Resolver::<f32>::nested(|| {
      let (scope, definition) = self.scoped_token(name)?;

      Self::of_scope(scope)?.with_resolver(|resolver| resolver.definition(&definition))
    })
  }

  /// The font size in force here: the typography's size, resolved where it is
  /// defined, else [`EM`].
  fn font_size(&self) -> f32 {
    Resolver::<f32>::nested(|| {
      let (scope, definition) = self.scoped_token(typography::SIZE.name())?;

      Self::of_scope(scope)?.with_resolver_at(EM, |resolver| resolver.definition(&definition))
    })
    .and_then(|resolved| match resolved {
      Resolved::Length(size) => Some(size),
      _ => None,
    })
    .unwrap_or(EM)
  }

  /// Runs `operation` with the resolver of this environment, reading tokens where they
  /// are defined and slots through the store.
  pub(crate) fn with_resolver<R>(&self, operation: impl FnOnce(&Resolver<'_, f32>) -> R) -> R {
    self.with_resolver_at(self.font_size(), operation)
  }

  /// Runs `operation` with the resolver of this environment, an `em` evaluating against
  /// `em`.
  fn with_resolver_at<R>(&self, em: f32, operation: impl FnOnce(&Resolver<'_, f32>) -> R) -> R {
    let token = |name| self.token(name);
    let slot = |slot| {
      let node = self.node?;
      let handle =
        Scene::<Headless>::with(|scene| scene.record(node).map(|record| *record.sink()))?;

      with_store(|store| store.record(handle)?.slots.get(&slot).cloned())
    };

    operation(&Resolver {
      environment: self.environment,
      interaction: self.interaction,
      containers: &self.containers,
      em,
      token: &token,
      slot: &slot,
    })
  }
}

/// A box animation reading `key` from each keyframe, each leaf resolved.
fn box_frames(
  env: &Env,
  animation: &Animation<BoxKeyframe<Headless>>,
  key: PropertyKey<NoProperty>,
) -> Animation<Option<SlotValue>> {
  animation.map_keyframes(|style| {
    env
      .with_resolver(|resolver| resolver.box_keyframe(style).value(key))
      .map(SlotValue::Box)
  })
}

/// A run animation reading `key` from each keyframe, each leaf resolved.
fn run_frames(
  env: &Env,
  animation: &Animation<RunKeyframe<Headless>>,
  key: PropertyKey<NoProperty>,
) -> Animation<Option<SlotValue>> {
  animation.map_keyframes(|style| {
    env
      .with_resolver(|resolver| resolver.run_keyframe(style).value(key))
      .map(SlotValue::Run)
  })
}

/// The motion declared on a node that bears on one property: its transitions, and its
/// animations reading the property from each keyframe, resolved.
struct DeclaredMotion {
  transitions: Vec<Transition<NoProperty>>,
  animations: Vec<Animation<Option<SlotValue>>>,
}

impl DeclaredMotion {
  /// The motion of `kind` bearing on `key`; `None` for a node with no style.
  fn of(env: &Env, kind: &Kind, key: PropertyKey<NoProperty>) -> Option<Self> {
    let box_props = match kind {
      Kind::Box(props) => &**props,
      Kind::Control(props) => &props.box_props,
      Kind::Overlay(props) => &props.box_props,
      Kind::Run(props) => {
        return Some(Self {
          transitions: props.dynamics.transitions.clone(),
          animations: props
            .dynamics
            .animations
            .iter()
            .map(|animation| run_frames(env, animation, key))
            .collect(),
        });
      }
      Kind::Anchor | Kind::Scope => return None,
    };

    Some(Self {
      transitions: box_props.dynamics.transitions.clone(),
      animations: box_props
        .dynamics
        .animations
        .iter()
        .map(|animation| box_frames(env, animation, key))
        .collect(),
    })
  }
}

/// The value `key` of the node behind `handle` presents at the store's clock: the
/// tween in flight where `target` moved under a transition, else `target`, with the
/// node's animations laid over.
fn present(
  env: &Env,
  node: NodeId,
  handle: crate::Handle,
  kind: &Kind,
  key: PropertyKey<NoProperty>,
  target: Option<SlotValue>,
) -> Option<SlotValue> {
  let Some(declared) = DeclaredMotion::of(env, kind, key)
  else {
    return target;
  };

  with_store(|store| {
    let now = store.clock;
    let reduced = store.reduced_motion.unwrap_or(false);
    let Some(record) = store.record_mut(handle)
    else {
      return target;
    };
    let motion = &mut record.motion;

    motion.node = Some(node);

    if let Some(previous) = motion.targets.insert(key, target.clone()) {
      motion.runner.retarget(
        key,
        previous.as_ref(),
        target.as_ref(),
        &declared.transitions,
        now,
        reduced,
      );
    }

    let underlying = motion.runner.sample(key, now).or(target);

    motion
      .runner
      .animate(&declared.animations, underlying, Clone::clone, now, reduced)
  })
}

/// The property `key` of `node` resolved and presented: a box's as a [`BoxValue`], a
/// run's as a [`RunValue`], in flight where a transition or an animation moves it.
pub(crate) fn resolve(node: NodeId, key: PropertyKey<NoProperty>) -> Option<SlotValue> {
  let target = resolve_target(node, key);
  let env = Env::of(node)?;
  let handle = Scene::<Headless>::with(|scene| scene.record(node).map(|record| *record.sink()))?;
  let kind = with_store(|store| match &store.record(handle)?.kind {
    Kind::Box(props) => Some(Kind::Box(props.clone())),
    Kind::Control(props) => Some(Kind::Control(props.clone())),
    Kind::Overlay(props) => Some(Kind::Overlay(props.clone())),
    Kind::Run(props) => Some(Kind::Run(props.clone())),
    Kind::Anchor | Kind::Scope => None,
  })?;

  present(&env, node, handle, &kind, key, target)
}

/// The property `key` of `node` resolved, motion aside: the value it is heading for.
fn resolve_target(node: NodeId, key: PropertyKey<NoProperty>) -> Option<SlotValue> {
  let env = Env::of(node)?;
  let handle = Scene::<Headless>::with(|scene| scene.record(node).map(|record| *record.sink()))?;
  let kind = with_store(|store| match &store.record(handle)?.kind {
    Kind::Box(props) => Some(Kind::Box(props.clone())),
    Kind::Control(props) => Some(Kind::Control(props.clone())),
    Kind::Overlay(props) => Some(Kind::Overlay(props.clone())),
    Kind::Run(props) => Some(Kind::Run(props.clone())),
    Kind::Anchor | Kind::Scope => None,
  })?;
  let box_style = match &kind {
    Kind::Box(props) => Some(&props.style),
    Kind::Control(props) => Some(&props.box_props.style),
    Kind::Overlay(props) => Some(&props.box_props.style),
    Kind::Run(_) | Kind::Anchor | Kind::Scope => None,
  };

  if let Some(style) = box_style {
    return env
      .with_resolver(|resolver| resolver.box_style(style).value(key))
      .map(SlotValue::Box);
  }

  match kind {
    Kind::Run(props) => env
      .with_resolver(|resolver| resolver.run_style(&props.style).value(key))
      .map(SlotValue::Run),
    Kind::Box(_) | Kind::Control(_) | Kind::Overlay(_) | Kind::Anchor | Kind::Scope => None,
  }
}
