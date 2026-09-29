//! The interpolation every platform running motion on its own clock shares: the easing
//! curves, the in-between value per value type, and the tween that samples them.

use super::{Easing, Transition};
use crate::{
  Border,
  BoxValue,
  Color,
  Dimension,
  Flexibility,
  Inset,
  RunValue,
  Scalar,
  SlotValue,
  Spacing,
  StaticLeaf,
  StyleExtension,
};

/// How many halvings the bezier solve refines the parameter through.
const BEZIER_STEPS: u32 = 24;

impl Easing {
  /// The eased progress at the linear `progress`, both `0.` to `1.`.
  #[must_use]
  pub fn apply(self, progress: f32) -> f32 {
    let progress = progress.clamp(0., 1.);

    match self {
      Self::Linear => progress,
      Self::Ease => cubic_bezier(0.25, 0.1, 0.25, 1., progress),
      Self::EaseIn => cubic_bezier(0.42, 0., 1., 1., progress),
      Self::EaseOut => cubic_bezier(0., 0., 0.58, 1., progress),
      Self::EaseInOut => cubic_bezier(0.42, 0., 0.58, 1., progress),
      Self::CubicBezier(x1, y1, x2, y2) => cubic_bezier(x1, y1, x2, y2, progress),
    }
  }
}

/// The `y` of the cubic bezier through `(0, 0)`, `(x1, y1)`, `(x2, y2)`, `(1, 1)` at
/// `x`, the parameter found by bisection.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
  let axis = |control_1: f32, control_2: f32, parameter: f32| {
    let inverse = 1. - parameter;

    3. * inverse * inverse * parameter * control_1
      + 3. * inverse * parameter * parameter * control_2
      + parameter * parameter * parameter
  };
  let (mut low, mut high) = (0f32, 1f32);
  let mut parameter = x;

  for _ in 0..BEZIER_STEPS {
    parameter = f32::midpoint(low, high);

    if axis(x1, x2, parameter) < x {
      low = parameter;
    }
    else {
      high = parameter;
    }
  }

  axis(y1, y2, parameter)
}

/// A value with an in-between: what a transition or a keyframe animation samples.
///
/// Colors mix in Oklab, lengths and numbers move linearly and land back in the
/// medium's unit, and a value per edge or axis moves per component. A pair with no
/// in-between (a keyword, a mode, a length and `Auto`) is discrete, as in CSS: a
/// transition changes it at once, and a keyframe animation halfway through the
/// interval.
#[diagnostic::on_unimplemented(
  message = "`{Self}` has no in-between value",
  label = "a value a transition samples implements `Interpolate`, answering `None` for a discrete pair"
)]
pub trait Interpolate: Clone {
  /// The value `amount` of the way from this one to `to`: `0.` is this value, `1.` is
  /// `to`; `None` for a discrete pair.
  #[must_use]
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self>;

  /// Whether this value and `to` have in-betweens, so a transition moves between them.
  #[inline(always)]
  #[must_use]
  fn interpolates_with(&self, to: &Self) -> bool {
    self.interpolate(to, 0.).is_some()
  }
}

/// The in-between of a discrete pair: `from` until halfway, `to` after.
#[inline(always)]
#[must_use]
pub(crate) fn snap<T: Clone>(from: &T, to: &T, amount: f32) -> T {
  if amount < 0.5 {
    from.clone()
  }
  else {
    to.clone()
  }
}

#[inline(always)]
fn lerp(from: f32, to: f32, amount: f32) -> f32 {
  from + (to - from) * amount
}

impl Interpolate for f32 {
  #[inline(always)]
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(lerp(*self, *to, amount))
  }
}

impl Interpolate for u32 {
  #[inline(always)]
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(Self::from_f32(lerp(self.to_f32(), to.to_f32(), amount)))
  }
}

impl Interpolate for Color {
  #[inline(always)]
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(self.mix(*to, amount))
  }
}

impl<T: Interpolate> Interpolate for Inset<T> {
  #[inline(always)]
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(Self {
      top: self.top.interpolate(&to.top, amount)?,
      right: self.right.interpolate(&to.right, amount)?,
      bottom: self.bottom.interpolate(&to.bottom, amount)?,
      left: self.left.interpolate(&to.left, amount)?,
    })
  }
}

impl<T: Interpolate> Interpolate for Spacing<T> {
  #[inline(always)]
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(Self {
      row: self.row.interpolate(&to.row, amount)?,
      column: self.column.interpolate(&to.column, amount)?,
    })
  }
}

impl<T: Interpolate> Interpolate for Dimension<T> {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    match (self, to) {
      (Self::Length(from), Self::Length(to)) => Some(Self::Length(from.interpolate(to, amount)?)),
      (Self::Fraction(from), Self::Fraction(to)) => Some(Self::Fraction(lerp(*from, *to, amount))),
      _ => None,
    }
  }
}

impl<N: Interpolate> Interpolate for Flexibility<N> {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(Self {
      grow_factor: lerp(self.grow_factor, to.grow_factor, amount),
      shrink_factor: lerp(self.shrink_factor, to.shrink_factor, amount),
      initial: self.initial.interpolate(&to.initial, amount)?,
    })
  }
}

impl<N: Interpolate> Interpolate for Border<N> {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    let color = match (&self.color, &to.color) {
      (StaticLeaf::Fixed(from), StaticLeaf::Fixed(to)) => {
        StaticLeaf::Fixed(from.interpolate(to, amount)?)
      }
      _ => return None,
    };

    Some(Self {
      width: self.width.interpolate(&to.width, amount)?,
      color,
    })
  }
}

impl<N: Scalar + Interpolate, X: StyleExtension> Interpolate for BoxValue<N, X> {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    match (self, to) {
      (Self::Dimension(from), Self::Dimension(to)) => {
        Some(Self::Dimension(from.interpolate(to, amount)?))
      }
      (Self::Number(from), Self::Number(to)) => Some(Self::Number(from.interpolate(to, amount)?)),
      (Self::DimensionInset(from), Self::DimensionInset(to)) => {
        Some(Self::DimensionInset(from.interpolate(to, amount)?))
      }
      (Self::Inset(from), Self::Inset(to)) => Some(Self::Inset(from.interpolate(to, amount)?)),
      (Self::Color(from), Self::Color(to)) => Some(Self::Color(from.interpolate(to, amount)?)),
      (Self::Border(from), Self::Border(to)) => Some(Self::Border(from.interpolate(to, amount)?)),
      (Self::Spacing(from), Self::Spacing(to)) => {
        Some(Self::Spacing(from.interpolate(to, amount)?))
      }
      (Self::Flexibility(from), Self::Flexibility(to)) => {
        Some(Self::Flexibility(from.interpolate(to, amount)?))
      }
      (Self::Extension(from), Self::Extension(to)) => {
        Some(Self::Extension(X::interpolate_value(from, to, amount)?))
      }
      _ => None,
    }
  }
}

impl<N: Scalar + Interpolate, X: StyleExtension> Interpolate for SlotValue<N, X> {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    match (self, to) {
      (Self::Box(from), Self::Box(to)) => Some(Self::Box(from.interpolate(to, amount)?)),
      (Self::Run(from), Self::Run(to)) => Some(Self::Run(from.interpolate(to, amount)?)),
      _ => None,
    }
  }
}

impl<X: StyleExtension> Interpolate for RunValue<X> {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    match (self, to) {
      (Self::Color(from), Self::Color(to)) => Some(Self::Color(from.interpolate(to, amount)?)),
      (Self::Extension(from), Self::Extension(to)) => {
        Some(Self::Extension(X::interpolate_value(from, to, amount)?))
      }
      _ => None,
    }
  }
}

/// One property in flight: from the value it showed to the value it is due, over a
/// transition's timing, against a clock in seconds.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Tween<V> {
  /// The value the property showed when the change arrived.
  pub from: V,
  /// The value the property is due.
  pub to: V,
  /// The clock reading the change arrived at.
  pub start: f64,
  /// The interpolation length, in seconds.
  pub duration: f32,
  /// The delay before it starts, in seconds.
  pub delay: f32,
  /// The timing curve.
  pub easing: Easing,
}

impl<V: Interpolate> Tween<V> {
  /// A tween from `from` to `to` starting at `start`, timed by `transition`.
  #[inline(always)]
  pub const fn new<P>(from: V, to: V, start: f64, transition: &Transition<P>) -> Self {
    Self {
      from,
      to,
      start,
      duration: transition.duration,
      delay: transition.delay,
      easing: transition.easing,
    }
  }

  /// The linear progress at clock reading `now`, `0.` to `1.`.
  #[must_use]
  pub fn progress(&self, now: f64) -> f32 {
    let played = now - self.start - f64::from(self.delay);

    if played <= 0. {
      return 0.;
    }

    if self.duration <= 0. {
      return 1.;
    }

    #[expect(
      clippy::cast_possible_truncation,
      reason = "progress lies in 0..=1, which f32 holds closely enough"
    )]
    let progress = (played / f64::from(self.duration)).min(1.) as f32;

    progress
  }

  /// Whether the tween has reached its end at clock reading `now`.
  #[inline(always)]
  #[must_use]
  pub fn is_done(&self, now: f64) -> bool {
    self.progress(now) >= 1.
  }

  /// The value shown at clock reading `now`; a discrete pair shows `to` from halfway.
  #[inline(always)]
  pub fn sample(&self, now: f64) -> V {
    let amount = self.easing.apply(self.progress(now));

    self
      .from
      .interpolate(&self.to, amount)
      .unwrap_or_else(|| snap(&self.from, &self.to, amount))
  }
}

/// The value of `frames` at `offset` percent: the two frames around it interpolated
/// through `easing` over their interval, the nearest one alone past either end; `None`
/// for no frames.
///
/// Of frames sharing an offset, the last declared stands, and a discrete pair turns
/// halfway through its eased interval, as in CSS.
pub fn sample_keyframes<V: Interpolate>(
  frames: &[(f32, V)],
  offset: f32,
  easing: Easing,
) -> Option<V> {
  let before = frames
    .iter()
    .filter(|(percent, _)| *percent <= offset)
    .max_by(|(left, _), (right, _)| left.total_cmp(right));
  let after = frames
    .iter()
    .rev()
    .filter(|(percent, _)| *percent >= offset)
    .min_by(|(left, _), (right, _)| left.total_cmp(right));

  match (before, after) {
    (Some((from_percent, from)), Some((to_percent, to))) => {
      let span = to_percent - from_percent;

      if span <= 0. {
        return Some(to.clone());
      }

      let amount = easing.apply((offset - from_percent) / span);

      Some(
        from
          .interpolate(to, amount)
          .unwrap_or_else(|| snap(from, to, amount)),
      )
    }
    (Some((_, value)), None) | (None, Some((_, value))) => Some(value.clone()),
    (None, None) => None,
  }
}

#[cfg(test)]
mod tests {
  use super::{Interpolate, Tween, sample_keyframes};
  use crate::{Color, Dimension, Easing, Inset, NoProperty, Property, Transition};

  /// The named curves start at rest and end at rest, and the linear one is the
  /// identity.
  #[test]
  fn easings_run_from_zero_to_one() {
    for easing in [
      Easing::Linear,
      Easing::Ease,
      Easing::EaseIn,
      Easing::EaseOut,
      Easing::EaseInOut,
      Easing::CubicBezier(0.3, 0., 0.7, 1.),
    ] {
      assert!(easing.apply(0.).abs() < 1e-4, "{easing:?} at 0");
      assert!((easing.apply(1.) - 1.).abs() < 1e-4, "{easing:?} at 1");
    }

    assert!((Easing::Linear.apply(0.3) - 0.3).abs() < 1e-6);
    assert!(Easing::EaseIn.apply(0.5) < 0.5);
    assert!(Easing::EaseOut.apply(0.5) > 0.5);
  }

  /// Cells round at the sample, lengths keep their kind, and a length and `Auto` are
  /// discrete.
  #[test]
  fn lengths_interpolate_per_kind() {
    assert_eq!(0u32.interpolate(&10, 0.26), Some(3));
    assert_eq!(
      Dimension::Length(0f32).interpolate(&Dimension::Length(10.), 0.5),
      Some(Dimension::Length(5.))
    );
    assert_eq!(
      Dimension::<f32>::Auto.interpolate(&Dimension::Length(10.), 0.4),
      None
    );
    assert!(!Dimension::<f32>::Auto.interpolates_with(&Dimension::Length(10.)));
    assert_eq!(
      Inset::all(0f32).interpolate(&Inset::all(4.), 0.25),
      Some(Inset::all(1.))
    );
  }

  /// A tween waits out its delay, samples its curve, and reports when it is done.
  #[test]
  #[expect(
    clippy::float_cmp,
    reason = "a linear tween's samples at these instants are exact"
  )]
  fn tweens_sample_their_transition() {
    let transition = Transition::<NoProperty>::new(Property::Width, 1.)
      .set_delay(0.5)
      .set_easing(Easing::Linear);
    let tween = Tween::new(0f32, 100., 2., &transition);

    assert_eq!(tween.sample(2.25), 0.);
    assert!((tween.sample(3.) - 50.).abs() < 1e-4);
    assert_eq!(tween.sample(4.), 100.);
    assert!(!tween.is_done(3.4));
    assert!(tween.is_done(3.5));
  }

  /// Keyframes interpolate between their neighbours and hold past either end; colors
  /// mix in Oklab.
  #[test]
  fn keyframes_sample_between_neighbours() {
    let frames = [(0., 0f32), (50., 10.), (100., 30.)];

    assert_eq!(sample_keyframes(&frames, 25., Easing::Linear), Some(5.));
    assert_eq!(sample_keyframes(&frames, 75., Easing::Linear), Some(20.));
    assert_eq!(
      sample_keyframes(&frames[1..], 10., Easing::Linear),
      Some(10.)
    );

    let black = Color::srgb(0., 0., 0.);
    let white = Color::srgb(1., 1., 1.);
    let mid = sample_keyframes(&[(0., black), (100., white)], 50., Easing::Linear).unwrap();

    assert_eq!(Some(mid), black.interpolate(&white, 0.5));
    assert_eq!(sample_keyframes::<f32>(&[], 50., Easing::Linear), None);
  }

  /// The easing curves each keyframe interval on its own, as CSS's timing function
  /// does, and a discrete pair turns halfway through its eased interval.
  #[test]
  fn easing_applies_per_interval() {
    let frames = [(0., 0f32), (50., 10.), (100., 30.)];
    let eased = Easing::EaseIn.apply(0.5);

    assert!((sample_keyframes(&frames, 25., Easing::EaseIn).unwrap() - 10. * eased).abs() < 1e-4);
    assert!(
      (sample_keyframes(&frames, 75., Easing::EaseIn).unwrap() - (10. + 20. * eased)).abs() < 1e-4
    );

    let sizes = [(0., Dimension::<f32>::Auto), (100., Dimension::Length(10.))];

    assert_eq!(
      sample_keyframes(&sizes, 40., Easing::Linear),
      Some(Dimension::Auto)
    );
    assert_eq!(
      sample_keyframes(&sizes, 60., Easing::Linear),
      Some(Dimension::Length(10.))
    );
  }

  /// Of two frames at one offset the last declared stands, on either side of it.
  #[test]
  fn a_repeated_offset_takes_its_last_frame() {
    let frames = [(0., 0f32), (50., 10.), (50., 20.), (100., 40.)];

    assert_eq!(sample_keyframes(&frames, 25., Easing::Linear), Some(10.));
    assert_eq!(sample_keyframes(&frames, 50., Easing::Linear), Some(20.));
    assert_eq!(sample_keyframes(&frames, 75., Easing::Linear), Some(30.));
  }
}
