//! Motion on the terminal's frame clock: a changed resolution starts a tween where a
//! transition names the property, keyframe animations sample against the clock, and
//! the presented style is the resolved one with every running tween and animation laid
//! over it.

use skene_core::{
  Animation,
  BoxDynamics,
  BoxField,
  BoxFieldVisitor,
  BoxValue,
  Color,
  IntoBoxValue,
  IntoRunValue,
  Property,
  PropertyKey,
  RunDynamics,
  RunField,
  RunFieldVisitor,
  RunValue,
  Transition,
  interpolate::{Interpolate as _, Tween, sample_keyframes},
};

use crate::{
  TuiExt,
  TuiProperty,
  store::{PlainBox, PlainRun},
};

/// A property's key on the terminal.
type Key = PropertyKey<TuiProperty>;

/// What a box-like node runs: a tween per property in flight, the border ink's tween,
/// and the clock reading its animations began at.
#[derive(Default)]
pub(crate) struct BoxMotion {
  tweens: Vec<(Key, Tween<BoxValue<u32, TuiExt>>)>,
  ink: Option<Tween<Color>>,
  started: Option<f64>,
}

/// What a run runs: a tween per property in flight, and the clock reading its
/// animations began at.
#[derive(Default)]
pub(crate) struct RunMotion {
  tweens: Vec<(Key, Tween<RunValue<TuiExt>>)>,
  started: Option<f64>,
}

/// What a node showed before a change: its presented style and its resolved target,
/// so a tween starts where the eye left off and a change is judged against the value
/// the node was heading for.
pub(crate) struct Previous<'a, S> {
  pub(crate) presented: &'a S,
  pub(crate) target: &'a S,
}

/// What presenting a node at one clock reading produced.
pub(crate) struct Presented<S> {
  /// The style to lay out and paint.
  pub(crate) style: S,
  /// Whether a tween or an animation is still running, so another frame is due.
  pub(crate) active: bool,
}

/// The last transition of `dynamics` covering `key`, unless reduced motion skips it.
fn covering<'a, P: PartialEq>(
  transitions: &'a [Transition<P>],
  key: &PropertyKey<P>,
  reduced: bool,
) -> Option<&'a Transition<P>> {
  transitions
    .iter()
    .rev()
    .find(|transition| transition.property.covers(key))
    .filter(|transition| !(reduced && transition.respects_reduced_motion))
}

/// The offset each animation samples at: its own at `elapsed`, or its resting end
/// when reduced motion skips it.
fn offsets<S>(animations: &[Animation<S>], elapsed: f64, reduced: bool) -> Vec<f32> {
  animations
    .iter()
    .map(|animation| {
      if reduced && animation.respects_reduced_motion {
        animation.rest_offset()
      }
      else {
        animation.offset(elapsed)
      }
    })
    .collect()
}

/// Whether any of `animations` still plays `elapsed` seconds in.
fn any_playing<S>(animations: &[Animation<S>], elapsed: f64, reduced: bool) -> bool {
  animations
    .iter()
    .any(|animation| !(animation.is_done(elapsed) || reduced && animation.respects_reduced_motion))
}

impl BoxMotion {
  /// Presents a box: starts a tween for every property whose target moved under a
  /// transition, samples the running tweens and animations at `now`, and reports
  /// whether any still runs.
  pub(crate) fn present(
    &mut self,
    previous: Option<&Previous<'_, PlainBox>>,
    target: PlainBox,
    dynamics: &BoxDynamics<u32, TuiExt>,
    keyframes: &[Animation<PlainBox>],
    now: f64,
    reduced: bool,
  ) -> Presented<PlainBox> {
    if let Some(previous) = previous {
      let mut start = StartBoxTweens {
        motion: self,
        previous,
        target: &target,
        dynamics,
        now,
        reduced,
      };

      PlainBox::for_each_field(&mut start);
    }

    self.tweens.retain(|(_, tween)| !tween.is_done(now));

    let mut style = target;
    let mut apply = ApplyBoxTweens {
      motion: self,
      style: &mut style,
      now,
    };

    PlainBox::for_each_field(&mut apply);

    let elapsed = now - *self.started.get_or_insert(now);

    if !keyframes.is_empty() {
      let mut animate = AnimateBox {
        keyframes,
        offsets: offsets(keyframes, elapsed, reduced),
        style: &mut style,
      };

      PlainBox::for_each_field(&mut animate);
    }

    Presented {
      style,
      active: !self.tweens.is_empty() || any_playing(keyframes, elapsed, reduced),
    }
  }

  /// Presents the border's ink: a tween where the border transitions, else the target.
  pub(crate) fn present_ink(
    &mut self,
    previous: Option<(Option<Color>, Option<Color>)>,
    target: Option<Color>,
    dynamics: &BoxDynamics<u32, TuiExt>,
    now: f64,
    reduced: bool,
  ) -> (Option<Color>, bool) {
    if let Some((presented, previous_target)) = previous
      && previous_target != target
    {
      self.ink = match (presented, target) {
        (Some(from), Some(to)) => covering(
          &dynamics.transitions,
          &PropertyKey::Shared(Property::Border),
          reduced,
        )
        .map(|transition| Tween::new(from, to, now, transition)),
        _ => None,
      };
    }

    if self.ink.as_ref().is_some_and(|tween| tween.is_done(now)) {
      self.ink = None;
    }

    match &self.ink {
      Some(tween) => (Some(tween.sample(now)), true),
      None => (target, false),
    }
  }
}

impl RunMotion {
  /// Presents a run: starts a tween for every property whose target moved under a
  /// transition, samples the running tweens and animations at `now`, and reports
  /// whether any still runs.
  pub(crate) fn present(
    &mut self,
    previous: Option<&Previous<'_, PlainRun>>,
    target: PlainRun,
    dynamics: &RunDynamics<TuiExt>,
    keyframes: &[Animation<PlainRun>],
    now: f64,
    reduced: bool,
  ) -> Presented<PlainRun> {
    if let Some(previous) = previous {
      let mut start = StartRunTweens {
        motion: self,
        previous,
        target: &target,
        dynamics,
        now,
        reduced,
      };

      PlainRun::for_each_field(&mut start);
    }

    self.tweens.retain(|(_, tween)| !tween.is_done(now));

    let mut style = target;
    let mut apply = ApplyRunTweens {
      motion: self,
      style: &mut style,
      now,
    };

    PlainRun::for_each_field(&mut apply);

    let elapsed = now - *self.started.get_or_insert(now);

    if !keyframes.is_empty() {
      let mut animate = AnimateRun {
        keyframes,
        offsets: offsets(keyframes, elapsed, reduced),
        style: &mut style,
      };

      PlainRun::for_each_field(&mut animate);
    }

    Presented {
      style,
      active: !self.tweens.is_empty() || any_playing(keyframes, elapsed, reduced),
    }
  }
}

/// Starts or replaces the tween of every box property whose target moved.
struct StartBoxTweens<'a> {
  motion: &'a mut BoxMotion,
  previous: &'a Previous<'a, PlainBox>,
  target: &'a PlainBox,
  dynamics: &'a BoxDynamics<u32, TuiExt>,
  now: f64,
  reduced: bool,
}

impl BoxFieldVisitor<u32, TuiExt> for StartBoxTweens<'_> {
  fn field<F: BoxField<u32, TuiExt>>(&mut self) {
    let next = F::get(self.target);

    if F::get(self.previous.target) == next {
      return;
    }

    self.motion.tweens.retain(|(key, _)| *key != F::KEY);

    let (Some(from), Some(to)) = (F::get(self.previous.presented), next)
    else {
      return;
    };
    let Some(transition) = covering(&self.dynamics.transitions, &F::KEY, self.reduced)
    else {
      return;
    };
    let (from, to) = (from.clone().into_box_value(), to.clone().into_box_value());

    if from.interpolates_with(&to) {
      self
        .motion
        .tweens
        .push((F::KEY, Tween::new(from, to, self.now, transition)));
    }
  }
}

/// Writes every running box tween's sample over the target.
struct ApplyBoxTweens<'a> {
  motion: &'a BoxMotion,
  style: &'a mut PlainBox,
  now: f64,
}

impl BoxFieldVisitor<u32, TuiExt> for ApplyBoxTweens<'_> {
  fn field<F: BoxField<u32, TuiExt>>(&mut self) {
    if let Some((_, tween)) = self.motion.tweens.iter().find(|(key, _)| *key == F::KEY) {
      *F::get_mut(self.style) = F::Value::from_box_value(tween.sample(self.now));
    }
  }
}

/// Lays every animation's sample over the style, in declaration order.
struct AnimateBox<'a> {
  keyframes: &'a [Animation<PlainBox>],
  offsets: Vec<f32>,
  style: &'a mut PlainBox,
}

impl BoxFieldVisitor<u32, TuiExt> for AnimateBox<'_> {
  fn field<F: BoxField<u32, TuiExt>>(&mut self) {
    for (animation, offset) in self.keyframes.iter().zip(&self.offsets) {
      let frames: Vec<(f32, BoxValue<u32, TuiExt>)> = animation
        .keyframes
        .iter()
        .filter_map(|(percent, style)| {
          F::get(style)
            .clone()
            .map(|value| (*percent, value.into_box_value()))
        })
        .collect();

      if let Some(value) = sample_keyframes(&frames, *offset, animation.easing) {
        *F::get_mut(self.style) = F::Value::from_box_value(value);
      }
    }
  }
}

/// Starts or replaces the tween of every run property whose target moved.
struct StartRunTweens<'a> {
  motion: &'a mut RunMotion,
  previous: &'a Previous<'a, PlainRun>,
  target: &'a PlainRun,
  dynamics: &'a RunDynamics<TuiExt>,
  now: f64,
  reduced: bool,
}

impl RunFieldVisitor<TuiExt> for StartRunTweens<'_> {
  fn field<F: RunField<TuiExt>>(&mut self) {
    let next = F::get(self.target);

    if F::get(self.previous.target) == next {
      return;
    }

    self.motion.tweens.retain(|(key, _)| *key != F::KEY);

    let (Some(from), Some(to)) = (F::get(self.previous.presented), next)
    else {
      return;
    };
    let Some(transition) = covering(&self.dynamics.transitions, &F::KEY, self.reduced)
    else {
      return;
    };
    let (from, to) = (from.clone().into_run_value(), to.clone().into_run_value());

    if from.interpolates_with(&to) {
      self
        .motion
        .tweens
        .push((F::KEY, Tween::new(from, to, self.now, transition)));
    }
  }
}

/// Writes every running run tween's sample over the target.
struct ApplyRunTweens<'a> {
  motion: &'a RunMotion,
  style: &'a mut PlainRun,
  now: f64,
}

impl RunFieldVisitor<TuiExt> for ApplyRunTweens<'_> {
  fn field<F: RunField<TuiExt>>(&mut self) {
    if let Some((_, tween)) = self.motion.tweens.iter().find(|(key, _)| *key == F::KEY) {
      *F::get_mut(self.style) = F::Value::from_run_value(tween.sample(self.now));
    }
  }
}

/// Lays every animation's sample over the style, in declaration order.
struct AnimateRun<'a> {
  keyframes: &'a [Animation<PlainRun>],
  offsets: Vec<f32>,
  style: &'a mut PlainRun,
}

impl RunFieldVisitor<TuiExt> for AnimateRun<'_> {
  fn field<F: RunField<TuiExt>>(&mut self) {
    for (animation, offset) in self.keyframes.iter().zip(&self.offsets) {
      let frames: Vec<(f32, RunValue<TuiExt>)> = animation
        .keyframes
        .iter()
        .filter_map(|(percent, style)| {
          F::get(style)
            .clone()
            .map(|value| (*percent, value.into_run_value()))
        })
        .collect();

      if let Some(value) = sample_keyframes(&frames, *offset, animation.easing) {
        *F::get_mut(self.style) = F::Value::from_run_value(value);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use skene_core::{BoxDynamics, Color, Easing, Property, Transition};

  use super::{BoxMotion, Previous};
  use crate::store::PlainBox;

  fn colored(color: Color) -> PlainBox {
    PlainBox {
      color: Some(color),
      ..PlainBox::default()
    }
  }

  /// A moved target under a transition presents the in-between value and stays
  /// active until the transition ends; without a transition the target shows at once.
  #[test]
  fn a_moved_target_tweens_under_its_transition() {
    let black = Color::srgb(0., 0., 0.);
    let white = Color::srgb(1., 1., 1.);
    let dynamics = BoxDynamics {
      transitions: vec![Transition::new(Property::Color, 1.).set_easing(Easing::Linear)],
      animations: Vec::new(),
    };
    let mut motion = BoxMotion::default();
    let first = motion.present(None, colored(black), &dynamics, &[], 0., false);

    assert_eq!(first.style.color, Some(black));
    assert!(!first.active);

    let previous = Previous {
      presented: &first.style,
      target: &colored(black),
    };
    let moved = motion.present(Some(&previous), colored(white), &dynamics, &[], 0., false);

    assert_eq!(moved.style.color, Some(black));
    assert!(moved.active);

    let held = colored(white);
    let previous = Previous {
      presented: &moved.style,
      target: &held,
    };
    let midway = motion.present(Some(&previous), colored(white), &dynamics, &[], 0.5, false);

    assert_eq!(midway.style.color, Some(black.mix(white, 0.5)));
    assert!(midway.active);

    let previous = Previous {
      presented: &midway.style,
      target: &held,
    };
    let done = motion.present(Some(&previous), colored(white), &dynamics, &[], 1., false);

    assert_eq!(done.style.color, Some(white));
    assert!(!done.active);

    let previous = Previous {
      presented: &done.style,
      target: &held,
    };
    let reduced = motion.present(Some(&previous), colored(black), &dynamics, &[], 1., true);

    assert_eq!(reduced.style.color, Some(black));
    assert!(!reduced.active);
  }
}
