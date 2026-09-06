//! The driver's resolution of a property: the environment and the node's interaction
//! feed the shared [`Resolver`], and the presented value lays the node's motion over
//! what it resolved to.

use skene_core::{
  Animation,
  Border,
  BoxKeyframe,
  BoxValue,
  ColorScheme,
  ContainerName,
  Interaction,
  NoProperty,
  NodeId,
  NodeRecord,
  Property,
  PropertyKey,
  Resolver,
  RunKeyframe,
  Scene,
  Size,
  Transition,
  interpolate::{Tween, sample_keyframes},
};

use crate::{
  Headless,
  SlotValue,
  store::{DEFAULT_VIEWPORT, Kind},
  with_store,
};

/// The font size expressions in `em` evaluate against.
const EM: f32 = 16.;

/// What one node's properties resolve against: the environment, the node's interaction
/// and the named containers above it.
pub struct Env {
  node: NodeId,
  viewport: Size<f32>,
  scheme: ColorScheme,
  reduced_motion: bool,
  interaction: Interaction,
  containers: Vec<(ContainerName, Option<Size<f32>>)>,
}

impl Env {
  /// The environment of `node`; `None` for a stale id.
  pub fn of(node: NodeId) -> Option<Self> {
    let (handle, control, names) = Scene::<Headless>::with(|scene| {
      let handle = *scene.record(node)?.sink();
      let control = scene.enclosing_control(node).and_then(|control| {
        let record = scene.record(control)?;

        Some((*record.sink(), record.control_state()?))
      });
      let mut names = Vec::new();
      let mut current = scene.parent(node);

      while let Some(ancestor) = current {
        if let Some(name) = scene.record(ancestor).and_then(NodeRecord::container) {
          names.push(name);
        }

        current = scene.parent(ancestor);
      }

      Some((handle, control, names))
    })?;

    with_store(|store| {
      let record = store.record(handle)?;
      let focus_holder = control.map_or(handle, |(control, _)| control);

      Some(Self {
        node,
        viewport: store.viewport.unwrap_or(DEFAULT_VIEWPORT),
        scheme: store.scheme_in_force(),
        reduced_motion: store.reduced_motion.unwrap_or(false),
        interaction: Interaction {
          hovered: record.hovered,
          focused: store.focused == Some(focus_holder),
          pressed: false,
          control: control.map(|(_, state)| state),
        },
        containers: names
          .into_iter()
          .map(|name| (name, store.container_sizes.get(&name).copied()))
          .collect(),
      })
    })
  }

  /// Runs `operation` with the resolver of this environment, reading tokens through
  /// the scene and slots through the store.
  pub fn with_resolver<R>(&self, operation: impl FnOnce(&Resolver<'_, f32>) -> R) -> R {
    let node = self.node;
    let token = move |name| Scene::<Headless>::with(|scene| scene.resolve_token(node, name));
    let slot = move |slot| {
      let handle =
        Scene::<Headless>::with(|scene| scene.record(node).map(|record| *record.sink()))?;

      with_store(|store| store.record(handle)?.slots.get(&slot).cloned())
    };

    operation(&Resolver {
      viewport: self.viewport,
      scheme: self.scheme,
      reduced_motion: self.reduced_motion,
      interaction: self.interaction,
      containers: &self.containers,
      em: EM,
      token: &token,
      slot: &slot,
    })
  }
}

/// The keyframes of one animation that set one property, each leaf resolved: the
/// percentage and the value at it.
type Frames = Vec<(f32, SlotValue)>;

/// The keyframes of a box animation that set `key`, each leaf resolved.
fn box_frames(
  env: &Env,
  animation: &Animation<BoxKeyframe<Headless>>,
  key: PropertyKey<NoProperty>,
) -> Frames {
  animation
    .keyframes
    .iter()
    .filter_map(|(percent, style)| {
      env
        .with_resolver(|resolver| resolver.box_keyframe(style).value(key))
        .map(|value| (*percent, SlotValue::Box(value)))
    })
    .collect()
}

/// The keyframes of a run animation that set `key`, each leaf resolved.
fn run_frames(
  env: &Env,
  animation: &Animation<RunKeyframe<Headless>>,
  key: PropertyKey<NoProperty>,
) -> Frames {
  animation
    .keyframes
    .iter()
    .filter_map(|(percent, style)| {
      env
        .with_resolver(|resolver| resolver.run_keyframe(style).value(key))
        .map(|value| (*percent, SlotValue::Run(value)))
    })
    .collect()
}

/// The last transition of `transitions` covering `key`, unless reduced motion skips it.
fn covering(
  transitions: &[Transition<NoProperty>],
  key: PropertyKey<NoProperty>,
  reduced: bool,
) -> Option<&Transition<NoProperty>> {
  transitions
    .iter()
    .rev()
    .find(|transition| transition.property.covers(&key))
    .filter(|transition| !(reduced && transition.respects_reduced_motion))
}

/// The motion declared on a node that bears on one property: its transitions, and each
/// animation's timing with the frames setting the property, resolved.
struct DeclaredMotion {
  transitions: Vec<Transition<NoProperty>>,
  animations: Vec<(Animation<()>, Frames)>,
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
            .map(|animation| {
              (
                animation.map_keyframes(|_| ()),
                run_frames(env, animation, key),
              )
            })
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
        .map(|animation| {
          (
            animation.map_keyframes(|_| ()),
            box_frames(env, animation, key),
          )
        })
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
  target: SlotValue,
) -> SlotValue {
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

    let previous = motion.targets.insert(key, target.clone());

    if let Some(previous) = previous
      && previous != target
    {
      let from = motion
        .tweens
        .remove(&key)
        .map_or(previous, |tween| tween.sample(now));

      if let Some(transition) = covering(&declared.transitions, key, reduced) {
        motion
          .tweens
          .insert(key, Tween::new(from, target.clone(), now, transition));
      }
    }

    if motion
      .tweens
      .get(&key)
      .is_some_and(|tween| tween.is_done(now))
    {
      motion.tweens.remove(&key);
    }

    let mut presented = motion
      .tweens
      .get(&key)
      .map_or(target, |tween| tween.sample(now));
    let elapsed = now - motion.started;

    for (animation, frames) in &declared.animations {
      let offset = if reduced && animation.respects_reduced_motion {
        animation.offset(f64::MAX)
      }
      else {
        animation.offset(elapsed)
      };

      if let Some(value) = sample_keyframes(frames, offset) {
        presented = value;
      }
    }

    presented
  })
}

/// The property `key` of `node` resolved and presented: a box's as a [`BoxValue`], a
/// run's as a [`RunValue`], in flight where a transition or an animation moves it.
pub fn resolve(node: NodeId, key: PropertyKey<NoProperty>) -> Option<SlotValue> {
  let target = resolve_target(node, key)?;
  let env = Env::of(node)?;
  let handle = Scene::<Headless>::with(|scene| scene.record(node).map(|record| *record.sink()))?;
  let kind = with_store(|store| match &store.record(handle)?.kind {
    Kind::Box(props) => Some(Kind::Box(props.clone())),
    Kind::Control(props) => Some(Kind::Control(props.clone())),
    Kind::Overlay(props) => Some(Kind::Overlay(props.clone())),
    Kind::Run(props) => Some(Kind::Run(props.clone())),
    Kind::Anchor | Kind::Scope => None,
  })?;

  Some(present(&env, node, handle, &kind, key, target))
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

/// The border of the box `node`, its color leaf resolved to a fixed color.
pub fn resolve_border(node: NodeId) -> Option<Border<f32>> {
  let SlotValue::Box(BoxValue::Border(border)) =
    resolve(node, PropertyKey::Shared(Property::Border))?
  else {
    return None;
  };
  let env = Env::of(node)?;
  let color = env.with_resolver(|resolver| resolver.color_leaf(&border.color))?;

  Some(Border::new(border.width, color))
}
