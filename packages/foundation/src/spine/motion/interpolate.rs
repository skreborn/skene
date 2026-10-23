//! The motion every platform running it on its own clock shares: the easing curves, the
//! in-between value per value type, and the runner that samples a node's transitions
//! and keyframe animations.

use super::{Animation, Easing, Transition};
use crate::spine::{
  AspectRatio,
  BoxValue,
  Color,
  Dimension,
  Flexibility,
  GridTemplate,
  GridTrack,
  Inset,
  Length,
  PropertyKey,
  PropertyName,
  RunValue,
  Scalar,
  SlotValue,
  Spacing,
  StyleExtension,
  TemplateEntry,
  TrackLimit,
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
/// medium's unit, an aspect ratio moves through its logarithm, and a value per edge or
/// axis moves per component. A visibility is
/// visible throughout a change to or from visible, and a list of grid tracks moves per
/// track where the lists are as long and each pair of tracks moves or matches, a
/// template's line names matching too. A pair
/// with no in-between (a keyword, a mode, a length and `Auto`) is discrete. All of it
/// is as in CSS: a transition changes a discrete pair at once, and a keyframe animation
/// halfway through the interval.
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
#[must_use]
pub(in crate::spine) fn snap<T: Clone>(from: &T, to: &T, amount: f32) -> T {
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

impl Interpolate for Length {
  #[inline(always)]
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(Self(lerp(self.0, to.0, amount)))
  }
}

impl Interpolate for AspectRatio {
  #[inline(always)]
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    if self.is_degenerate() || to.is_degenerate() {
      return None;
    }

    Some(Self(lerp(self.0.ln(), to.0.ln(), amount).exp()))
  }
}

impl Interpolate for i32 {
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
      end: self.end.interpolate(&to.end, amount)?,
      bottom: self.bottom.interpolate(&to.bottom, amount)?,
      start: self.start.interpolate(&to.start, amount)?,
    })
  }
}

impl<T: Interpolate> Interpolate for Spacing<T> {
  #[inline(always)]
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(Self {
      horizontal: self.horizontal.interpolate(&to.horizontal, amount)?,
      vertical: self.vertical.interpolate(&to.vertical, amount)?,
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

/// Whether a box goes undrawn: drawn through every change to or from drawn, as CSS
/// interpolates `visibility`.
impl Interpolate for bool {
  #[inline(always)]
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(if self == to || amount <= 0. {
      *self
    }
    else if amount >= 1. {
      *to
    }
    else {
      false
    })
  }
}

impl<T: Interpolate + PartialEq> Interpolate for TrackLimit<T> {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    match (self, to) {
      (Self::Length(from), Self::Length(to)) => Some(Self::Length(from.interpolate(to, amount)?)),
      (Self::Fraction(from), Self::Fraction(to)) => Some(Self::Fraction(lerp(*from, *to, amount))),
      (Self::Flex(from), Self::Flex(to)) => Some(Self::Flex(lerp(*from, *to, amount))),
      (from, to) => (from == to).then(|| from.clone()),
    }
  }
}

impl<T: Interpolate + PartialEq> Interpolate for GridTrack<T> {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    match (self, to) {
      (Self::Length(from), Self::Length(to)) => Some(Self::Length(from.interpolate(to, amount)?)),
      (Self::Fraction(from), Self::Fraction(to)) => Some(Self::Fraction(lerp(*from, *to, amount))),
      (Self::Flex(from), Self::Flex(to)) => Some(Self::Flex(lerp(*from, *to, amount))),
      (Self::FitContent(from), Self::FitContent(to)) => {
        Some(Self::FitContent(from.interpolate(to, amount)?))
      }
      (Self::Between(from_min, from_max), Self::Between(to_min, to_max)) => Some(Self::Between(
        from_min.interpolate(to_min, amount)?,
        from_max.interpolate(to_max, amount)?,
      )),
      (Self::Adaptive(from_min, from_max), Self::Adaptive(to_min, to_max)) => Some(Self::Adaptive(
        from_min.interpolate(to_min, amount)?,
        from_max.interpolate(to_max, amount)?,
      )),
      (from, to) => (from == to).then(|| from.clone()),
    }
  }
}

impl<T: Interpolate + PartialEq> Interpolate for GridTemplate<T> {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    if self.entries().len() != to.entries().len() {
      return None;
    }

    self
      .entries()
      .iter()
      .zip(to.entries())
      .try_fold(Self::new(), |template, pair| match pair {
        (TemplateEntry::Track(from), TemplateEntry::Track(to)) => {
          Some(template.add_track(from.interpolate(to, amount)?))
        }
        (TemplateEntry::Line(from), TemplateEntry::Line(to)) if from == to => {
          Some(template.add_line(*from))
        }
        _ => None,
      })
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

impl<N: Scalar + Interpolate, X: StyleExtension> Interpolate for BoxValue<N, X> {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    match (self, to) {
      (Self::Dimension(from), Self::Dimension(to)) => {
        Some(Self::Dimension(from.interpolate(to, amount)?))
      }
      (Self::Number(from), Self::Number(to)) => Some(Self::Number(from.interpolate(to, amount)?)),
      (Self::AspectRatio(from), Self::AspectRatio(to)) => {
        Some(Self::AspectRatio(from.interpolate(to, amount)?))
      }
      (Self::Length(from), Self::Length(to)) => Some(Self::Length(from.interpolate(to, amount)?)),
      (Self::DimensionInset(from), Self::DimensionInset(to)) => {
        Some(Self::DimensionInset(from.interpolate(to, amount)?))
      }
      (Self::Inset(from), Self::Inset(to)) => Some(Self::Inset(from.interpolate(to, amount)?)),
      (Self::Color(from), Self::Color(to)) => Some(Self::Color(from.interpolate(to, amount)?)),
      (Self::Spacing(from), Self::Spacing(to)) => {
        Some(Self::Spacing(from.interpolate(to, amount)?))
      }
      (Self::Flexibility(from), Self::Flexibility(to)) => {
        Some(Self::Flexibility(from.interpolate(to, amount)?))
      }
      (Self::Tracks(from), Self::Tracks(to)) if from.len() == to.len() => Some(Self::Tracks(
        from
          .iter()
          .zip(to)
          .map(|(from, to)| from.interpolate(to, amount))
          .collect::<Option<_>>()?,
      )),
      (Self::Template(from), Self::Template(to)) => {
        Some(Self::Template(from.interpolate(to, amount)?))
      }
      (Self::Hidden(from), Self::Hidden(to)) => Some(Self::Hidden(from.interpolate(to, amount)?)),
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
      (Self::Length(from), Self::Length(to)) => Some(Self::Length(from.interpolate(to, amount)?)),
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
struct Tween<V> {
  from: V,
  to: V,
  reversing_from: V,
  shortening: f32,
  start: f64,
  duration: f32,
  delay: f32,
  easing: Easing,
}

impl<V: Interpolate> Tween<V> {
  /// The linear progress at clock reading `now`, `0.` to `1.`.
  fn progress(&self, now: f64) -> f32 {
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
  fn is_done(&self, now: f64) -> bool {
    self.progress(now) >= 1.
  }

  /// The value shown at clock reading `now`.
  fn sample(&self, now: f64) -> V {
    let amount = self.easing.apply(self.progress(now));

    self
      .from
      .interpolate(&self.to, amount)
      .unwrap_or_else(|| snap(&self.from, &self.to, amount))
  }

  /// The share of its full duration a tween reversing this one at `now` runs: the
  /// eased share of the way this one has come, as CSS's reversing shortening factor.
  fn reversing_shortening(&self, now: f64) -> f32 {
    (self.easing.apply(self.progress(now)) * self.shortening + (1. - self.shortening))
      .abs()
      .clamp(0., 1.)
  }
}

/// One node's motion on a platform's own clock: the tween of each property in flight,
/// and the clock reading its keyframe animations count from.
///
/// A platform [retargets](Self::retarget) a property whenever the value it resolves to
/// changes, shows what [`sample`](Self::sample) answers while a tween runs, lays the
/// node's keyframe animations over with [`animate`](Self::animate), and asks
/// [`is_running`](Self::is_running) whether another frame is due.
#[derive(Debug, Clone)]
#[must_use]
pub struct Motion<P, V> {
  tweens: Vec<(PropertyKey<P>, Tween<V>)>,
  started: Option<f64>,
}

impl<P, V> Motion<P, V> {
  /// Motion whose animations count from the first clock reading it is given.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      tweens: Vec::new(),
      started: None,
    }
  }

  /// Motion whose animations count from clock reading `clock`.
  #[inline(always)]
  pub const fn started_at(clock: f64) -> Self {
    Self {
      tweens: Vec::new(),
      started: Some(clock),
    }
  }

  /// The seconds the animations have played at clock reading `now`, their clock
  /// starting there unless it has.
  #[inline(always)]
  fn elapsed(&mut self, now: f64) -> f64 {
    now - *self.started.get_or_insert(now)
  }
}

impl<P, V> Default for Motion<P, V> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: PropertyName, V: Interpolate + PartialEq> Motion<P, V> {
  /// Judges the value `key` resolves to moving from `previous` to `target` at clock
  /// reading `now`, as CSS starts a transition.
  ///
  /// Under the last of `transitions` covering the property, a tween starts from the
  /// value showing toward `target`, unless `reduced` motion skips that transition, the
  /// pair is discrete, or the property shows `target` already. A change back to where
  /// a running tween began reverses it over the share of its duration it had played.
  /// Otherwise the property's tween stops, and `target` shows at once.
  pub fn retarget(
    &mut self,
    key: PropertyKey<P>,
    previous: Option<&V>,
    target: Option<&V>,
    transitions: &[Transition<P>],
    now: f64,
    reduced: bool,
  ) {
    if previous == target {
      return;
    }

    let running = self
      .tweens
      .iter()
      .position(|(held, _)| *held == key)
      .map(|index| self.tweens.swap_remove(index).1)
      .filter(|tween| !tween.is_done(now));
    let Some(to) = target
    else {
      return;
    };
    let Some(from) = running
      .as_ref()
      .map_or_else(|| previous.cloned(), |tween| Some(tween.sample(now)))
    else {
      return;
    };
    let Some(transition) = transitions
      .iter()
      .rev()
      .find(|transition| transition.property.covers(&key))
      .filter(|transition| !(reduced && transition.respects_reduced_motion))
    else {
      return;
    };

    if transition.duration.max(0.) + transition.delay <= 0.
      || from == *to
      || !from.interpolates_with(to)
    {
      return;
    }

    let (reversing_from, shortening) = match running {
      Some(running) if running.reversing_from == *to => {
        let shortening = running.reversing_shortening(now);

        (running.to, shortening)
      }
      _ => (from.clone(), 1.),
    };
    let delay = if transition.delay < 0. {
      transition.delay * shortening
    }
    else {
      transition.delay
    };

    self.tweens.push((
      key,
      Tween {
        from,
        to: to.clone(),
        reversing_from,
        shortening,
        start: now,
        duration: transition.duration.max(0.) * shortening,
        delay,
        easing: transition.easing,
      },
    ));
  }

  /// The value `key`'s tween shows at clock reading `now`; `None` where none runs.
  #[must_use]
  pub fn sample(&self, key: PropertyKey<P>, now: f64) -> Option<V> {
    self
      .tweens
      .iter()
      .find(|(held, _)| *held == key)
      .map(|(_, tween)| tween)
      .filter(|tween| !tween.is_done(now))
      .map(|tween| tween.sample(now))
  }

  /// `underlying` with `animations` laid over in declaration order at clock reading
  /// `now`, `read` taking the property from a keyframe; `None` where nothing sets it.
  ///
  /// An animation setting the property samples its keyframes where it stands, one
  /// respecting reduced motion resting on its final frame where `reduced`; where the
  /// property is missing at `0%` or `100%`, it moves from or to the value beneath, as
  /// CSS's neutral keyframes do.
  pub fn animate<S>(
    &mut self,
    animations: &[Animation<S>],
    underlying: Option<V>,
    read: impl Fn(&S) -> Option<V>,
    now: f64,
    reduced: bool,
  ) -> Option<V> {
    if animations.is_empty() {
      return underlying;
    }

    let elapsed = self.elapsed(now);

    animations.iter().fold(underlying, |value, animation| {
      let frames: Vec<(f32, V)> = animation
        .keyframes
        .iter()
        .filter_map(|(percent, frame)| Some((*percent, read(frame)?)))
        .collect();

      sample_keyframes(
        &frames,
        animation.offset_under(elapsed, reduced),
        animation.easing,
        value.as_ref(),
      )
      .or(value)
    })
  }

  /// Whether a tween runs or one of `animations` plays at clock reading `now`, so
  /// another frame is due; the tweens that have finished are forgotten.
  pub fn is_running<S>(&mut self, animations: &[Animation<S>], now: f64, reduced: bool) -> bool {
    self.tweens.retain(|(_, tween)| !tween.is_done(now));

    if !self.tweens.is_empty() {
      return true;
    }

    if animations.is_empty() {
      return false;
    }

    let elapsed = self.elapsed(now);

    animations
      .iter()
      .any(|animation| animation.plays(elapsed, reduced))
  }
}

/// The value of `frames` at `offset` percent: the two frames around it interpolated
/// through `easing` over their interval; `None` for no frames.
///
/// Where no frame stands at `0%` or `100%`, `underlying` stands there, as CSS's neutral
/// keyframes do, and without it the nearest frame holds past either end. Of frames
/// sharing an offset, the last declared stands, and a discrete pair turns halfway
/// through its eased interval, as in CSS.
fn sample_keyframes<V: Interpolate>(
  frames: &[(f32, V)],
  offset: f32,
  easing: Easing,
  underlying: Option<&V>,
) -> Option<V> {
  if frames.is_empty() {
    return None;
  }

  let neutral = |edge: f32, at_edge: fn(f32) -> bool| {
    underlying
      .filter(|_| !frames.iter().any(|(percent, _)| at_edge(*percent)))
      .map(|value| (edge, value))
  };
  let first = neutral(0., |percent| percent <= 0.);
  let last = neutral(100., |percent| percent >= 100.);
  let all = || {
    first
      .into_iter()
      .chain(frames.iter().map(|(percent, value)| (*percent, value)))
      .chain(last)
  };
  let before = all()
    .filter(|(percent, _)| *percent <= offset)
    .max_by(|(left, _), (right, _)| left.total_cmp(right));
  let after = all()
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
  use super::{Interpolate, Motion, sample_keyframes};
  use crate::spine::{
    Animation,
    AspectRatio,
    BoxValue,
    Color,
    Dimension,
    Easing,
    GridName,
    GridTemplate,
    GridTrack,
    Inset,
    NoProperty,
    Property,
    PropertyKey,
    TrackLimit,
    Transition,
  };

  const WIDTH: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Width);
  const HEIGHT: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Height);

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
    assert_eq!(0i32.interpolate(&10, 0.26), Some(3));
    assert_eq!(0i32.interpolate(&-10, 0.26), Some(-3));
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
      Inset::<f32>::all(0.).interpolate(&Inset::all(4.), 0.25),
      Some(Inset::all(1.))
    );
  }

  /// A retargeted property waits out its transition's delay, samples its curve, and
  /// runs until the transition ends.
  #[test]
  fn a_retarget_tweens_under_its_transition() {
    let transitions = [Transition::<NoProperty>::new(Property::Width, 1.)
      .set_delay(0.5)
      .set_easing(Easing::Linear)];
    let mut motion = Motion::<NoProperty, f32>::new();

    motion.retarget(WIDTH, Some(&0.), Some(&100.), &transitions, 2., false);

    assert_eq!(motion.sample(WIDTH, 2.25), Some(0.));
    assert!((motion.sample(WIDTH, 3.).unwrap() - 50.).abs() < 1e-4);
    assert_eq!(motion.sample(WIDTH, 3.5), None);
    assert!(motion.is_running::<()>(&[], 3.4, false));
    assert!(!motion.is_running::<()>(&[], 3.5, false));
  }

  /// A change no transition covers, one reduced motion skips, and a discrete pair each
  /// show the target at once.
  #[test]
  fn uncovered_skipped_and_discrete_changes_cut() {
    let transitions = [Transition::<NoProperty>::new(Property::Width, 1.)];
    let mut motion = Motion::<NoProperty, Dimension<f32>>::new();
    let (short, long) = (Dimension::Length(0.), Dimension::Length(10.));

    motion.retarget(HEIGHT, Some(&short), Some(&long), &transitions, 0., false);
    motion.retarget(WIDTH, Some(&short), Some(&long), &transitions, 0., true);

    assert_eq!(motion.sample(HEIGHT, 0.5), None);
    assert_eq!(motion.sample(WIDTH, 0.5), None);

    motion.retarget(
      WIDTH,
      Some(&long),
      Some(&Dimension::Auto),
      &transitions,
      0.,
      false,
    );

    assert_eq!(motion.sample(WIDTH, 0.5), None);

    motion.retarget(
      WIDTH,
      Some(&Dimension::Auto),
      Some(&short),
      &transitions,
      0.,
      false,
    );
    motion.retarget(WIDTH, Some(&short), Some(&long), &transitions, 0., false);

    assert!(motion.sample(WIDTH, 0.5).is_some());
  }

  /// A change back to where a running tween began reverses it over the eased share it
  /// had played, and reversing that reversal runs the rest, as CSS's reversing
  /// shortening does.
  #[test]
  fn a_reversal_shortens_by_the_share_played() {
    let transitions =
      [Transition::<NoProperty>::new(Property::Width, 1.).set_easing(Easing::Linear)];
    let mut motion = Motion::<NoProperty, f32>::new();

    motion.retarget(WIDTH, Some(&0.), Some(&100.), &transitions, 0., false);
    motion.retarget(WIDTH, Some(&100.), Some(&0.), &transitions, 0.25, false);

    assert_eq!(motion.sample(WIDTH, 0.375), Some(12.5));
    assert_eq!(motion.sample(WIDTH, 0.5), None);

    motion.retarget(WIDTH, Some(&0.), Some(&100.), &transitions, 0.375, false);

    assert_eq!(motion.sample(WIDTH, 0.8125), Some(56.25));
    assert!(motion.is_running::<()>(&[], 1.24, false));
    assert!(!motion.is_running::<()>(&[], 1.25, false));
  }

  /// Animations lay over in declaration order, each over the value beneath it; one
  /// setting nothing leaves the value alone, and a reader preferring reduced motion
  /// sees each on its final frame.
  #[test]
  fn animations_lay_over_in_order() {
    let fade = Animation::new(1., [(100., Some(0f32))]).set_easing(Easing::Linear);
    let pulse = Animation::new(1., [(50., Some(20f32))]).set_easing(Easing::Linear);
    let idle = Animation::new(1., [(50., None)]);
    let animations = [fade, pulse, idle];
    let mut motion = Motion::<NoProperty, f32>::started_at(0.);

    assert_eq!(
      motion.animate(&animations, Some(40.), |frame| *frame, 0.25, false),
      Some(25.)
    );
    assert_eq!(
      motion.animate(&animations, Some(40.), |frame| *frame, 0.25, true),
      Some(0.)
    );
    assert_eq!(
      motion.animate(&animations[2..], Some(7.), |frame| *frame, 0.5, false),
      Some(7.)
    );
    assert!(motion.is_running(&animations, 0.5, false));
    assert!(!motion.is_running(&animations, 0.5, true));
  }

  /// Keyframes interpolate between their neighbours and hold past either end; colors
  /// mix in Oklab.
  #[test]
  fn keyframes_sample_between_neighbours() {
    let frames = [(0., 0f32), (50., 10.), (100., 30.)];

    assert_eq!(
      sample_keyframes(&frames, 25., Easing::Linear, None),
      Some(5.)
    );
    assert_eq!(
      sample_keyframes(&frames, 75., Easing::Linear, None),
      Some(20.)
    );
    assert_eq!(
      sample_keyframes(&frames[1..], 10., Easing::Linear, None),
      Some(10.)
    );

    let black = Color::srgb(0., 0., 0.);
    let white = Color::srgb(1., 1., 1.);
    let mid = sample_keyframes(&[(0., black), (100., white)], 50., Easing::Linear, None).unwrap();

    assert_eq!(Some(mid), black.interpolate(&white, 0.5));
    assert_eq!(
      sample_keyframes::<f32>(&[], 50., Easing::Linear, None),
      None
    );
  }

  /// The easing curves each keyframe interval on its own, as CSS's timing function
  /// does, and a discrete pair turns halfway through its eased interval.
  #[test]
  fn easing_applies_per_interval() {
    let frames = [(0., 0f32), (50., 10.), (100., 30.)];
    let eased = Easing::EaseIn.apply(0.5);

    assert!(
      (sample_keyframes(&frames, 25., Easing::EaseIn, None).unwrap() - 10. * eased).abs() < 1e-4
    );
    assert!(
      (sample_keyframes(&frames, 75., Easing::EaseIn, None).unwrap() - (10. + 20. * eased)).abs()
        < 1e-4
    );

    let sizes = [(0., Dimension::<f32>::Auto), (100., Dimension::Length(10.))];

    assert_eq!(
      sample_keyframes(&sizes, 40., Easing::Linear, None),
      Some(Dimension::Auto)
    );
    assert_eq!(
      sample_keyframes(&sizes, 60., Easing::Linear, None),
      Some(Dimension::Length(10.))
    );
  }

  /// Of two frames at one offset the last declared stands, on either side of it.
  #[test]
  fn a_repeated_offset_takes_its_last_frame() {
    let frames = [(0., 0f32), (50., 10.), (50., 20.), (100., 40.)];

    assert_eq!(
      sample_keyframes(&frames, 25., Easing::Linear, None),
      Some(10.)
    );
    assert_eq!(
      sample_keyframes(&frames, 50., Easing::Linear, None),
      Some(20.)
    );
    assert_eq!(
      sample_keyframes(&frames, 75., Easing::Linear, None),
      Some(30.)
    );
  }

  /// A property missing at `0%` or `100%` moves from or to the underlying value, and
  /// holds its nearest frame where there is none.
  #[test]
  fn missing_ends_take_the_underlying_value() {
    let middle = [(50., 10f32)];

    assert_eq!(
      sample_keyframes(&middle, 25., Easing::Linear, Some(&0.)),
      Some(5.)
    );
    assert_eq!(
      sample_keyframes(&middle, 75., Easing::Linear, Some(&30.)),
      Some(20.)
    );
    assert_eq!(
      sample_keyframes(&middle, 25., Easing::Linear, None),
      Some(10.)
    );
    assert_eq!(
      sample_keyframes(&[(100., 40f32)], 25., Easing::Linear, Some(&0.)),
      Some(10.)
    );
    assert_eq!(
      sample_keyframes(&[(0., 40f32)], 25., Easing::Linear, Some(&0.)),
      Some(30.)
    );
  }

  /// A box stays drawn through a change to or from drawn, and grid tracks move per
  /// track where the lists are as long and each pair moves or matches.
  #[test]
  fn hiding_and_tracks_interpolate_as_css_does() {
    let (visible, hidden) = (false, true);

    assert_eq!(visible.interpolate(&hidden, 0.), Some(visible));
    assert_eq!(visible.interpolate(&hidden, 0.9), Some(visible));
    assert_eq!(visible.interpolate(&hidden, 1.), Some(hidden));
    assert_eq!(hidden.interpolate(&visible, 0.1), Some(visible));
    assert_eq!(hidden.interpolate(&hidden, 0.5), Some(hidden));

    let tracks = |length: f32, flex: f32, minimum: f32| {
      BoxValue::<f32>::Tracks(vec![
        GridTrack::Length(length),
        GridTrack::Flex(flex),
        GridTrack::Auto,
        GridTrack::Between(TrackLimit::Length(minimum), TrackLimit::Flex(1.)),
      ])
    };

    assert_eq!(
      tracks(0., 1., 0.).interpolate(&tracks(10., 3., 20.), 0.5),
      Some(tracks(5., 2., 10.))
    );
    assert_eq!(
      tracks(0., 1., 0.).interpolate(&BoxValue::Tracks(vec![GridTrack::Length(10.)]), 0.5),
      None
    );
    assert_eq!(
      BoxValue::<f32>::Tracks(vec![GridTrack::Auto])
        .interpolate(&BoxValue::Tracks(vec![GridTrack::MinContent]), 0.5),
      None
    );
    assert_eq!(
      GridTrack::Adaptive(TrackLimit::Length(100f32), TrackLimit::Flex(1.)).interpolate(
        &GridTrack::Adaptive(TrackLimit::Length(200.), TrackLimit::Flex(1.)),
        0.5
      ),
      Some(GridTrack::Adaptive(
        TrackLimit::Length(150.),
        TrackLimit::Flex(1.)
      ))
    );
  }

  /// A template moves track by track while it names the same lines alike, and is
  /// discrete where the names differ.
  #[test]
  fn a_template_moves_while_its_names_match() {
    let template = |length: f32, name: &'static str| {
      GridTemplate::new()
        .add_track(GridTrack::Length(length))
        .add_line(GridName::new(name))
    };

    assert_eq!(
      template(0., "a").interpolate(&template(10., "a"), 0.5),
      Some(template(5., "a"))
    );
    assert_eq!(
      template(0., "a").interpolate(&template(10., "b"), 0.5),
      None
    );
  }

  /// An aspect ratio moves through its logarithm, and a degenerate one is discrete.
  #[test]
  fn an_aspect_ratio_moves_through_its_logarithm() {
    let halfway = BoxValue::<f32>::AspectRatio(AspectRatio(1.))
      .interpolate(&BoxValue::AspectRatio(AspectRatio(4.)), 0.5);

    assert!(
      matches!(halfway, Some(BoxValue::AspectRatio(AspectRatio(ratio))) if (ratio - 2.).abs() < 1e-5)
    );
    assert_eq!(AspectRatio(0.).interpolate(&AspectRatio(4.), 0.5), None);
    assert_eq!(
      AspectRatio(1.).interpolate(&AspectRatio(f32::INFINITY), 0.5),
      None
    );
  }
}
