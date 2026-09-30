//! Motion on the terminal's frame clock: core's runner, given each property as the
//! node's resolution changes it and laid over the resolved style.

use skene_core::{
  Animation,
  BoxDynamics,
  BoxField,
  BoxFieldVisitor,
  BoxValue,
  IntoBoxValue,
  IntoRunValue,
  RunDynamics,
  RunField,
  RunFieldVisitor,
  RunValue,
  Transition,
  interpolate::Motion,
};

use crate::{
  TuiProperty,
  TuiStyle,
  store::{PlainBox, PlainRun},
};

/// What a box-like node runs: its properties' motion.
#[derive(Default)]
pub(crate) struct BoxMotion {
  properties: Motion<TuiProperty, BoxValue<u32, TuiStyle>>,
}

/// What a run runs: its properties' motion.
#[derive(Default)]
pub(crate) struct RunMotion {
  properties: Motion<TuiProperty, RunValue<TuiStyle>>,
}

/// What presenting a node at one clock reading produced.
pub(crate) struct Presented<S> {
  /// The style to lay out and paint.
  pub(crate) style: S,
  /// Whether a tween or an animation is still running, so another frame is due.
  pub(crate) active: bool,
}

/// The clock reading a node presents at, and whether the reader prefers reduced motion.
#[derive(Clone, Copy)]
pub(crate) struct Clock {
  pub(crate) now: f64,
  pub(crate) reduced: bool,
}

impl BoxMotion {
  /// Presents a box: retargets every property whose target moved since `previous`,
  /// lays the running tweens and animations over the target, and reports whether any
  /// still runs.
  pub(crate) fn present(
    &mut self,
    previous: Option<&PlainBox>,
    target: PlainBox,
    dynamics: &BoxDynamics<u32, TuiStyle>,
    keyframes: &[Animation<PlainBox>],
    clock: Clock,
  ) -> Presented<PlainBox> {
    if let Some(previous) = previous {
      PlainBox::for_each_field(&mut RetargetBox {
        motion: &mut self.properties,
        previous,
        target: &target,
        transitions: &dynamics.transitions,
        clock,
      });
    }

    let mut style = target;

    PlainBox::for_each_field(&mut PresentBox {
      motion: &mut self.properties,
      style: &mut style,
      keyframes,
      clock,
    });

    Presented {
      style,
      active: self
        .properties
        .is_running(keyframes, clock.now, clock.reduced),
    }
  }
}

impl RunMotion {
  /// Presents a run: retargets every property whose target moved since `previous`,
  /// lays the running tweens and animations over the target, and reports whether any
  /// still runs.
  pub(crate) fn present(
    &mut self,
    previous: Option<&PlainRun>,
    target: PlainRun,
    dynamics: &RunDynamics<TuiStyle>,
    keyframes: &[Animation<PlainRun>],
    clock: Clock,
  ) -> Presented<PlainRun> {
    if let Some(previous) = previous {
      PlainRun::for_each_field(&mut RetargetRun {
        motion: &mut self.properties,
        previous,
        target: &target,
        transitions: &dynamics.transitions,
        clock,
      });
    }

    let mut style = target;

    PlainRun::for_each_field(&mut PresentRun {
      motion: &mut self.properties,
      style: &mut style,
      keyframes,
      clock,
    });

    Presented {
      style,
      active: self
        .properties
        .is_running(keyframes, clock.now, clock.reduced),
    }
  }
}

/// Retargets every box property whose target moved.
struct RetargetBox<'a> {
  motion: &'a mut Motion<TuiProperty, BoxValue<u32, TuiStyle>>,
  previous: &'a PlainBox,
  target: &'a PlainBox,
  transitions: &'a [Transition<TuiProperty>],
  clock: Clock,
}

impl BoxFieldVisitor<u32, TuiStyle> for RetargetBox<'_> {
  fn field<F: BoxField<u32, TuiStyle>>(&mut self) {
    let (previous, target) = (F::get(self.previous), F::get(self.target));

    if previous == target {
      return;
    }

    self.motion.retarget(
      F::KEY,
      previous.clone().map(IntoBoxValue::into_box_value).as_ref(),
      target.clone().map(IntoBoxValue::into_box_value).as_ref(),
      self.transitions,
      self.clock.now,
      self.clock.reduced,
    );
  }
}

/// Writes every box property's running tween and animations over the target.
struct PresentBox<'a> {
  motion: &'a mut Motion<TuiProperty, BoxValue<u32, TuiStyle>>,
  style: &'a mut PlainBox,
  keyframes: &'a [Animation<PlainBox>],
  clock: Clock,
}

impl BoxFieldVisitor<u32, TuiStyle> for PresentBox<'_> {
  fn field<F: BoxField<u32, TuiStyle>>(&mut self) {
    let tweened = self.motion.sample(F::KEY, self.clock.now);

    if tweened.is_none() && self.keyframes.is_empty() {
      return;
    }

    let underlying =
      tweened.or_else(|| F::get(self.style).clone().map(IntoBoxValue::into_box_value));
    let value = self.motion.animate(
      self.keyframes,
      underlying,
      |frame| F::get(frame).clone().map(IntoBoxValue::into_box_value),
      self.clock.now,
      self.clock.reduced,
    );

    *F::get_mut(self.style) = value.and_then(F::Value::from_box_value);
  }
}

/// Retargets every run property whose target moved.
struct RetargetRun<'a> {
  motion: &'a mut Motion<TuiProperty, RunValue<TuiStyle>>,
  previous: &'a PlainRun,
  target: &'a PlainRun,
  transitions: &'a [Transition<TuiProperty>],
  clock: Clock,
}

impl RunFieldVisitor<TuiStyle> for RetargetRun<'_> {
  fn field<F: RunField<TuiStyle>>(&mut self) {
    let (previous, target) = (F::get(self.previous), F::get(self.target));

    if previous == target {
      return;
    }

    self.motion.retarget(
      F::KEY,
      previous.clone().map(IntoRunValue::into_run_value).as_ref(),
      target.clone().map(IntoRunValue::into_run_value).as_ref(),
      self.transitions,
      self.clock.now,
      self.clock.reduced,
    );
  }
}

/// Writes every run property's running tween and animations over the target.
struct PresentRun<'a> {
  motion: &'a mut Motion<TuiProperty, RunValue<TuiStyle>>,
  style: &'a mut PlainRun,
  keyframes: &'a [Animation<PlainRun>],
  clock: Clock,
}

impl RunFieldVisitor<TuiStyle> for PresentRun<'_> {
  fn field<F: RunField<TuiStyle>>(&mut self) {
    let tweened = self.motion.sample(F::KEY, self.clock.now);

    if tweened.is_none() && self.keyframes.is_empty() {
      return;
    }

    let underlying =
      tweened.or_else(|| F::get(self.style).clone().map(IntoRunValue::into_run_value));
    let value = self.motion.animate(
      self.keyframes,
      underlying,
      |frame| F::get(frame).clone().map(IntoRunValue::into_run_value),
      self.clock.now,
      self.clock.reduced,
    );

    *F::get_mut(self.style) = value.and_then(F::Value::from_run_value);
  }
}

#[cfg(test)]
mod tests {
  use skene_core::{Animation, BoxDynamics, Color, Easing, Property, Transition};

  use super::{BoxMotion, Clock};
  use crate::store::PlainBox;

  fn colored(color: Color) -> PlainBox {
    PlainBox {
      color: Some(color),
      ..PlainBox::default()
    }
  }

  const fn at(now: f64) -> Clock {
    Clock {
      now,
      reduced: false,
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
    let first = motion.present(None, colored(black), &dynamics, &[], at(0.));

    assert_eq!(first.style.color, Some(black));
    assert!(!first.active);

    let moved = motion.present(
      Some(&colored(black)),
      colored(white),
      &dynamics,
      &[],
      at(0.),
    );

    assert_eq!(moved.style.color, Some(black));
    assert!(moved.active);

    let held = colored(white);
    let midway = motion.present(Some(&held), colored(white), &dynamics, &[], at(0.5));

    assert_eq!(midway.style.color, Some(black.mix(white, 0.5)));
    assert!(midway.active);

    let done = motion.present(Some(&held), colored(white), &dynamics, &[], at(1.));

    assert_eq!(done.style.color, Some(white));
    assert!(!done.active);

    let reduced = motion.present(
      Some(&held),
      colored(black),
      &dynamics,
      &[],
      Clock {
        now: 1.,
        reduced: true,
      },
    );

    assert_eq!(reduced.style.color, Some(black));
    assert!(!reduced.active);
  }

  /// A keyframe animation setting a color at its end alone moves from the resolved
  /// color toward it.
  #[test]
  fn an_animation_without_a_first_frame_starts_from_the_target() {
    let black = Color::srgb(0., 0., 0.);
    let white = Color::srgb(1., 1., 1.);
    let dynamics = BoxDynamics::default();
    let fade = [Animation::new(1., [(100., colored(white))]).set_easing(Easing::Linear)];
    let mut motion = BoxMotion::default();
    let first = motion.present(None, colored(black), &dynamics, &fade, at(0.));

    assert_eq!(first.style.color, Some(black));
    assert!(first.active);

    let midway = motion.present(
      Some(&colored(black)),
      colored(black),
      &dynamics,
      &fade,
      at(0.5),
    );

    assert_eq!(midway.style.color, Some(black.mix(white, 0.5)));
  }
}
