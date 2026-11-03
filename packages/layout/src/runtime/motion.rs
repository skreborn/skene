//! Motion on a platform's frame clock: the foundation's runner, given each property as
//! the node's resolution changes it and laid over the resolved style, at the reading of
//! the runtime's [`Clock`].

use std::time::Instant;

use skene_foundation::{
  Animation,
  BoxField,
  BoxFieldVisitor,
  BoxStyle,
  BoxValue,
  IntoBoxValue,
  IntoRunValue,
  Plain,
  RunField,
  RunFieldVisitor,
  RunStyle,
  RunValue,
  Scalar,
  StyleExtension,
  Transition,
  interpolate::{Interpolate, Motion},
};

/// What a box-like node runs: its properties' motion.
#[derive(Debug)]
#[must_use]
pub struct BoxMotion<N: Scalar, X: StyleExtension> {
  properties: Motion<X::Property, BoxValue<N, X>>,
}

/// What a run runs: its properties' motion.
#[derive(Debug)]
#[must_use]
pub struct RunMotion<X: StyleExtension> {
  properties: Motion<X::Property, RunValue<X>>,
}

/// What presenting a node at one [`Sample`] produced.
#[derive(Debug)]
#[must_use]
pub struct Presented<S> {
  /// The style to lay out and paint.
  pub style: S,
  /// Whether a tween or an animation is still running, so another frame is due.
  pub active: bool,
}

/// The clock reading a node presents at, and whether the reader prefers reduced motion.
#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct Sample {
  /// The frame clock's reading, in seconds.
  pub now: f64,
  /// Whether the reader prefers reduced motion.
  pub reduced: bool,
}

/// The clock motion samples against, in seconds: the wall clock in a launched app, a
/// hand-stepped one under an in-process test.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub enum Clock {
  /// The wall clock since `Instant`.
  Real(Instant),
  /// A reading stepped by hand.
  Manual(f64),
}

impl Clock {
  /// The current reading, in seconds.
  #[inline(always)]
  #[must_use]
  pub fn now(&self) -> f64 {
    match self {
      Self::Real(epoch) => epoch.elapsed().as_secs_f64(),
      Self::Manual(seconds) => *seconds,
    }
  }

  /// Steps the reading forward by `seconds`; a wall clock becomes a hand-stepped one
  /// at its current reading.
  #[inline(always)]
  pub fn advance(&mut self, seconds: f64) {
    *self = Self::Manual(self.now() + seconds);
  }
}

impl<N: Scalar, X: StyleExtension> BoxMotion<N, X> {
  /// A box at rest: no tween in flight, its animations counting from the first sample
  /// it presents at.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      properties: Motion::new(),
    }
  }
}

impl<N: Scalar, X: StyleExtension> Default for BoxMotion<N, X> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<N: Scalar + Interpolate, X: StyleExtension> BoxMotion<N, X> {
  /// Presents a box: retargets every property whose target moved since `previous`,
  /// lays the running tweens and animations over a copy of the target, and reports
  /// whether any still runs.
  pub fn present(
    &mut self,
    previous: Option<&BoxStyle<N, Plain, X>>,
    target: &BoxStyle<N, Plain, X>,
    transitions: &[Transition<X::Property>],
    keyframes: &[Animation<BoxStyle<N, Plain, X>>],
    sample: Sample,
  ) -> Presented<BoxStyle<N, Plain, X>> {
    if let Some(previous) = previous {
      BoxStyle::<N, Plain, X>::for_each_field(&mut RetargetBox {
        motion: &mut self.properties,
        previous,
        target,
        transitions,
        sample,
      });
    }

    let mut style = target.clone();

    BoxStyle::<N, Plain, X>::for_each_field(&mut PresentBox {
      motion: &mut self.properties,
      style: &mut style,
      keyframes,
      sample,
    });

    Presented {
      style,
      active: self
        .properties
        .is_running(keyframes, sample.now, sample.reduced),
    }
  }
}

impl<X: StyleExtension> RunMotion<X> {
  /// A run at rest: no tween in flight, its animations counting from the first sample
  /// it presents at.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      properties: Motion::new(),
    }
  }
}

impl<X: StyleExtension> Default for RunMotion<X> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<X: StyleExtension> RunMotion<X> {
  /// Presents a run: retargets every property whose target moved since `previous`,
  /// lays the running tweens and animations over a copy of the target, and reports
  /// whether any still runs.
  pub fn present(
    &mut self,
    previous: Option<&RunStyle<Plain, X>>,
    target: &RunStyle<Plain, X>,
    transitions: &[Transition<X::Property>],
    keyframes: &[Animation<RunStyle<Plain, X>>],
    sample: Sample,
  ) -> Presented<RunStyle<Plain, X>> {
    if let Some(previous) = previous {
      RunStyle::<Plain, X>::for_each_field(&mut RetargetRun {
        motion: &mut self.properties,
        previous,
        target,
        transitions,
        sample,
      });
    }

    let mut style = target.clone();

    RunStyle::<Plain, X>::for_each_field(&mut PresentRun {
      motion: &mut self.properties,
      style: &mut style,
      keyframes,
      sample,
    });

    Presented {
      style,
      active: self
        .properties
        .is_running(keyframes, sample.now, sample.reduced),
    }
  }
}

/// Retargets every box property whose target moved.
struct RetargetBox<'a, N: Scalar, X: StyleExtension> {
  motion: &'a mut Motion<X::Property, BoxValue<N, X>>,
  previous: &'a BoxStyle<N, Plain, X>,
  target: &'a BoxStyle<N, Plain, X>,
  transitions: &'a [Transition<X::Property>],
  sample: Sample,
}

impl<N: Scalar + Interpolate, X: StyleExtension> BoxFieldVisitor<N, X> for RetargetBox<'_, N, X> {
  fn field<F: BoxField<N, X>>(&mut self) {
    let (previous, target) = (F::get(self.previous), F::get(self.target));

    if previous == target {
      return;
    }

    self.motion.retarget(
      F::KEY,
      previous.clone().map(IntoBoxValue::into_box_value).as_ref(),
      target.clone().map(IntoBoxValue::into_box_value).as_ref(),
      self.transitions,
      self.sample.now,
      self.sample.reduced,
    );
  }
}

/// Writes every box property's running tween and animations over the target, each
/// sample held to its property's range as CSS holds an interpolated value.
struct PresentBox<'a, N: Scalar, X: StyleExtension> {
  motion: &'a mut Motion<X::Property, BoxValue<N, X>>,
  style: &'a mut BoxStyle<N, Plain, X>,
  keyframes: &'a [Animation<BoxStyle<N, Plain, X>>],
  sample: Sample,
}

impl<N: Scalar + Interpolate, X: StyleExtension> BoxFieldVisitor<N, X> for PresentBox<'_, N, X> {
  fn field<F: BoxField<N, X>>(&mut self) {
    let tweened = self.motion.sample(F::KEY, self.sample.now);

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
      self.sample.now,
      self.sample.reduced,
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
struct RetargetRun<'a, X: StyleExtension> {
  motion: &'a mut Motion<X::Property, RunValue<X>>,
  previous: &'a RunStyle<Plain, X>,
  target: &'a RunStyle<Plain, X>,
  transitions: &'a [Transition<X::Property>],
  sample: Sample,
}

impl<X: StyleExtension> RunFieldVisitor<X> for RetargetRun<'_, X> {
  fn field<F: RunField<X>>(&mut self) {
    let (previous, target) = (F::get(self.previous), F::get(self.target));

    if previous == target {
      return;
    }

    self.motion.retarget(
      F::KEY,
      previous.clone().map(IntoRunValue::into_run_value).as_ref(),
      target.clone().map(IntoRunValue::into_run_value).as_ref(),
      self.transitions,
      self.sample.now,
      self.sample.reduced,
    );
  }
}

/// Writes every run property's running tween and animations over the target.
struct PresentRun<'a, X: StyleExtension> {
  motion: &'a mut Motion<X::Property, RunValue<X>>,
  style: &'a mut RunStyle<Plain, X>,
  keyframes: &'a [Animation<RunStyle<Plain, X>>],
  sample: Sample,
}

impl<X: StyleExtension> RunFieldVisitor<X> for PresentRun<'_, X> {
  fn field<F: RunField<X>>(&mut self) {
    let tweened = self.motion.sample(F::KEY, self.sample.now);

    if tweened.is_none() && self.keyframes.is_empty() {
      return;
    }

    let underlying =
      tweened.or_else(|| F::get(self.style).clone().map(IntoRunValue::into_run_value));
    let value = self.motion.animate(
      self.keyframes,
      underlying,
      |frame| F::get(frame).clone().map(IntoRunValue::into_run_value),
      self.sample.now,
      self.sample.reduced,
    );

    *F::get_mut(self.style) = value.and_then(F::Value::from_run_value);
  }
}

#[cfg(test)]
mod tests {
  use skene_foundation::{
    Animation,
    BoxStyle,
    Color,
    Easing,
    Plain,
    Property,
    Transition,
    terminal::TerminalStyle,
  };

  use super::{BoxMotion, Sample};

  type PlainBox = BoxStyle<i32, Plain, TerminalStyle>;

  fn colored(color: Color) -> PlainBox {
    PlainBox {
      background: Some(color),
      ..PlainBox::default()
    }
  }

  const fn at(now: f64) -> Sample {
    Sample {
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
    let first = motion.present(None, &colored(black), &transitions, &[], at(0.));

    assert_eq!(first.style.background, Some(black));
    assert!(!first.active);

    let moved = motion.present(
      Some(&colored(black)),
      &colored(white),
      &transitions,
      &[],
      at(0.),
    );

    assert_eq!(moved.style.background, Some(black));
    assert!(moved.active);

    let held = colored(white);
    let midway = motion.present(Some(&held), &colored(white), &transitions, &[], at(0.5));

    assert_eq!(midway.style.background, Some(black.mix(white, 0.5)));
    assert!(midway.active);

    let done = motion.present(Some(&held), &colored(white), &transitions, &[], at(1.));

    assert_eq!(done.style.background, Some(white));
    assert!(!done.active);

    let reduced = motion.present(
      Some(&held),
      &colored(black),
      &transitions,
      &[],
      Sample {
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
    let first = motion.present(None, &colored(black), &[], &fade, at(0.));

    assert_eq!(first.style.background, Some(black));
    assert!(first.active);

    let midway = motion.present(Some(&colored(black)), &colored(black), &[], &fade, at(0.5));

    assert_eq!(midway.style.background, Some(black.mix(white, 0.5)));
  }
}
