//! Motion on the terminal's frame clock: core's runner, given each property as the
//! node's resolution changes it and laid over the resolved style.

use skene_foundation::{
  Animation,
  BoxField,
  BoxFieldVisitor,
  BoxValue,
  IntoBoxValue,
  IntoRunValue,
  RunField,
  RunFieldVisitor,
  RunValue,
  Transition,
  interpolate::Motion,
};

use crate::{
  TerminalProperty,
  TerminalStyle,
  records::{PlainBox, PlainRun},
};

/// What a box-like node runs: its properties' motion.
#[derive(Default)]
pub(crate) struct BoxMotion {
  properties: Motion<TerminalProperty, BoxValue<i32, TerminalStyle>>,
}

/// What a run runs: its properties' motion.
#[derive(Default)]
pub(crate) struct RunMotion {
  properties: Motion<TerminalProperty, RunValue<TerminalStyle>>,
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
    transitions: &[Transition<TerminalProperty>],
    keyframes: &[Animation<PlainBox>],
    clock: Clock,
  ) -> Presented<PlainBox> {
    if let Some(previous) = previous {
      PlainBox::for_each_field(&mut RetargetBox {
        motion: &mut self.properties,
        previous,
        target: &target,
        transitions,
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
    transitions: &[Transition<TerminalProperty>],
    keyframes: &[Animation<PlainRun>],
    clock: Clock,
  ) -> Presented<PlainRun> {
    if let Some(previous) = previous {
      PlainRun::for_each_field(&mut RetargetRun {
        motion: &mut self.properties,
        previous,
        target: &target,
        transitions,
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
  motion: &'a mut Motion<TerminalProperty, BoxValue<i32, TerminalStyle>>,
  previous: &'a PlainBox,
  target: &'a PlainBox,
  transitions: &'a [Transition<TerminalProperty>],
  clock: Clock,
}

impl BoxFieldVisitor<i32, TerminalStyle> for RetargetBox<'_> {
  fn field<F: BoxField<i32, TerminalStyle>>(&mut self) {
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

/// Writes every box property's running tween and animations over the target, each
/// sample held to its property's range as CSS holds an interpolated value.
struct PresentBox<'a> {
  motion: &'a mut Motion<TerminalProperty, BoxValue<i32, TerminalStyle>>,
  style: &'a mut PlainBox,
  keyframes: &'a [Animation<PlainBox>],
  clock: Clock,
}

impl BoxFieldVisitor<i32, TerminalStyle> for PresentBox<'_> {
  fn field<F: BoxField<i32, TerminalStyle>>(&mut self) {
    let tweened = self.motion.sample(F::KEY, self.clock.now);

    if tweened.is_none() && self.keyframes.is_empty() {
      return;
    }

    let target = F::get(self.style).clone().map(IntoBoxValue::into_box_value);
    let underlying = tweened
      .map(|sample| sample.in_range_of(F::KEY))
      .or_else(|| target.clone());
    let value = self.motion.animate(
      self.keyframes,
      underlying,
      |frame| F::get(frame).clone().map(IntoBoxValue::into_box_value),
      self.clock.now,
      self.clock.reduced,
    );
    let value = if value == target {
      value
    }
    else {
      value.map(|sample| sample.in_range_of(F::KEY))
    };

    *F::get_mut(self.style) = value.and_then(F::Value::from_box_value);
  }
}

/// Retargets every run property whose target moved.
struct RetargetRun<'a> {
  motion: &'a mut Motion<TerminalProperty, RunValue<TerminalStyle>>,
  previous: &'a PlainRun,
  target: &'a PlainRun,
  transitions: &'a [Transition<TerminalProperty>],
  clock: Clock,
}

impl RunFieldVisitor<TerminalStyle> for RetargetRun<'_> {
  fn field<F: RunField<TerminalStyle>>(&mut self) {
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
  motion: &'a mut Motion<TerminalProperty, RunValue<TerminalStyle>>,
  style: &'a mut PlainRun,
  keyframes: &'a [Animation<PlainRun>],
  clock: Clock,
}

impl RunFieldVisitor<TerminalStyle> for PresentRun<'_> {
  fn field<F: RunField<TerminalStyle>>(&mut self) {
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
  use skene_foundation::{Animation, Color, Easing, Property, Transition};

  use super::{BoxMotion, Clock};
  use crate::records::PlainBox;

  fn colored(color: Color) -> PlainBox {
    PlainBox {
      background: Some(color),
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
    let transitions = [Transition::new(Property::Background, 1.).set_easing(Easing::Linear)];
    let mut motion = BoxMotion::default();
    let first = motion.present(None, colored(black), &transitions, &[], at(0.));

    assert_eq!(first.style.background, Some(black));
    assert!(!first.active);

    let moved = motion.present(
      Some(&colored(black)),
      colored(white),
      &transitions,
      &[],
      at(0.),
    );

    assert_eq!(moved.style.background, Some(black));
    assert!(moved.active);

    let held = colored(white);
    let midway = motion.present(Some(&held), colored(white), &transitions, &[], at(0.5));

    assert_eq!(midway.style.background, Some(black.mix(white, 0.5)));
    assert!(midway.active);

    let done = motion.present(Some(&held), colored(white), &transitions, &[], at(1.));

    assert_eq!(done.style.background, Some(white));
    assert!(!done.active);

    let reduced = motion.present(
      Some(&held),
      colored(black),
      &transitions,
      &[],
      Clock {
        now: 1.,
        reduced: true,
      },
    );

    assert_eq!(reduced.style.background, Some(black));
    assert!(!reduced.active);
  }

  /// A keyframe animation setting a color at its end alone moves from the resolved
  /// color toward it.
  #[test]
  fn an_animation_without_a_first_frame_starts_from_the_target() {
    let black = Color::srgb(0., 0., 0.);
    let white = Color::srgb(1., 1., 1.);
    let fade = [Animation::new(1., [(100., colored(white))]).set_easing(Easing::Linear)];
    let mut motion = BoxMotion::default();
    let first = motion.present(None, colored(black), &[], &fade, at(0.));

    assert_eq!(first.style.background, Some(black));
    assert!(first.active);

    let midway = motion.present(Some(&colored(black)), colored(black), &[], &fade, at(0.5));

    assert_eq!(midway.style.background, Some(black.mix(white, 0.5)));
  }
}
