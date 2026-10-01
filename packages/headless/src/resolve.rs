//! The platform's resolution of a property: the environment and the node's interaction
//! feed the shared [`Resolver`], and the presented value lays the node's motion over
//! what it resolved to.

use skene_foundation::{
  Animation,
  BoxKeyframe,
  ContainerName,
  Environment,
  NodeId,
  PropertyKey,
  ResolutionContext,
  ResolutionSource,
  RunKeyframe,
  Size,
  Slot,
  Transition,
  web::WebProperty,
};

use crate::{Headless, SlotLeaf, SlotValue, records::Kind, with_records};

/// Where headless resolution reads what the scene does not hold: the platform's
/// records.
pub(crate) struct Records;

impl ResolutionSource<Headless> for Records {
  fn environment(&self) -> Environment<f32> {
    with_records(|records| records.environment())
  }

  fn container_size(&self, name: ContainerName) -> Option<Size<f32>> {
    with_records(|records| records.media().signals.container_sizes).get_untracked(name)
  }

  fn slot(&self, node: NodeId, slot: Slot) -> Option<SlotLeaf> {
    with_records(|records| records.record(node)?.slots.get(&slot).cloned())
  }

  fn sizes_text(&self) -> bool {
    true
  }

  fn generation(&self) -> u64 {
    with_records(|records| records.environment_generation)
  }
}

/// What one node's properties resolve against: the environment, the node's interaction
/// and the named containers above it.
pub(crate) type Env = ResolutionContext<'static, Headless, Records>;

/// A box animation reading `key` from each keyframe, each leaf resolved.
fn box_frames(
  env: &Env,
  animation: &Animation<BoxKeyframe<Headless>>,
  key: PropertyKey<WebProperty>,
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
  key: PropertyKey<WebProperty>,
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
  transitions: Vec<Transition<WebProperty>>,
  animations: Vec<Animation<Option<SlotValue>>>,
}

impl DeclaredMotion {
  /// The motion of `kind` bearing on `key`; `None` for a node with no style.
  fn of(env: &Env, kind: &Kind, key: PropertyKey<WebProperty>) -> Option<Self> {
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

/// The value `key` of `node` presents at the records' clock: the
/// tween in flight where `target` moved under a transition, else `target`, with the
/// node's animations laid over.
fn present(
  env: &Env,
  node: NodeId,
  kind: &Kind,
  key: PropertyKey<WebProperty>,
  target: Option<SlotValue>,
) -> Option<SlotValue> {
  let Some(declared) = DeclaredMotion::of(env, kind, key)
  else {
    return target;
  };

  with_records(|records| {
    let now = records.clock;
    let reduced = records.reduced_motion.unwrap_or(false);
    let Some(record) = records.record_mut(node)
    else {
      return target;
    };
    let motion = &mut record.motion;

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
pub(crate) fn resolve(node: NodeId, key: PropertyKey<WebProperty>) -> Option<SlotValue> {
  let target = resolve_target(node, key);
  let env = Env::of(&Records, node)?;
  let kind = with_records(|records| match &records.record(node)?.kind {
    Kind::Box(props) => Some(Kind::Box(props.clone())),
    Kind::Control(props) => Some(Kind::Control(props.clone())),
    Kind::Overlay(props) => Some(Kind::Overlay(props.clone())),
    Kind::Run(props) => Some(Kind::Run(props.clone())),
    Kind::Anchor | Kind::Scope => None,
  })?;

  present(&env, node, &kind, key, target)
}

/// The property `key` of `node` resolved, motion aside: the value it is heading for.
fn resolve_target(node: NodeId, key: PropertyKey<WebProperty>) -> Option<SlotValue> {
  let env = Env::of(&Records, node)?;
  let kind = with_records(|records| match &records.record(node)?.kind {
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
