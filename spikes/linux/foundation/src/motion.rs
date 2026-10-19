//! Motion on a platform's own frame clock, for a realization that resolves every style
//! ([`crate::seam::style::Keep::Nothing`]): the easing curves, the in-between of each value a
//! transition or a keyframe animation moves ([`Interpolate`]), and the runner a realization
//! keeps per node ([`Motion`]), fed from the point that reports a node's resolved style
//! changed, never through the reactive graph per frame.
//!
//! A realization hands the runner the node's resolved properties each time they change
//! ([`Motion::update`]), which starts, reverses or stops a tween per property as CSS starts a
//! transition, and asks it what to show at each frame's clock reading ([`Motion::present`]),
//! keyframe animations laid over the tweens, and whether another frame is due.

use crate::{
  Color, Corners, FontWeight, Offset, Ring, Shadows,
  fill::{Fill, GradientStop, LinearGradient, RadialGradient, StopColor, Stops, SweepGradient},
  seam::record::{CompiledAnimation, Prop, Resolved, Transition},
  value::{Border, Curve, Dimension, Inset, Shadow, Spacing},
};

/// How many halvings the bezier solve refines the parameter through.
const BEZIER_STEPS: u32 = 24;

impl Curve {
  /// The eased progress at the linear `progress`, both `0` to `1`.
  #[must_use]
  pub fn apply(self, progress: f32) -> f32 {
    let progress = progress.clamp(0., 1.);

    match self {
      Self::Linear => progress,
      Self::EaseInOut => cubic_bezier(0.42, 0., 0.58, 1., progress),
      Self::EaseIn => cubic_bezier(0.42, 0., 1., 1., progress),
      Self::EaseOut => cubic_bezier(0., 0., 0.58, 1., progress),
      Self::Bezier(x1, y1, x2, y2) => cubic_bezier(x1, y1, x2, y2, progress),
    }
  }
}

/// The `y` of the cubic bezier through `(0, 0)`, `(x1, y1)`, `(x2, y2)`, `(1, 1)` at `x`, the
/// parameter found by bisection.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
  let axis = |control_1: f32, control_2: f32, parameter: f32| {
    let inverse = 1. - parameter;

    3. * inverse * inverse * parameter * control_1
      + 3. * inverse * parameter * parameter * control_2
      + parameter * parameter * parameter
  };
  let (mut low, mut high) = (0_f32, 1_f32);
  let mut parameter = x;

  for _ in 0..BEZIER_STEPS {
    parameter = f32::midpoint(low, high);

    if axis(x1, x2, parameter) < x {
      low = parameter;
    } else {
      high = parameter;
    }
  }

  axis(y1, y2, parameter)
}

/// A value with an in-between: what a transition or a keyframe animation samples.
///
/// Colors mix in Oklab, numbers move linearly, a value of parts (corners, a ring, an offset,
/// shadows of the same count and kinds, a border of one style) moves per part, and two
/// gradients of the same kind and stop count move per stop, a color moving to a gradient as
/// the gradient of that color throughout. A pair with no in-between is discrete: a transition
/// changes it at once, a keyframe animation halfway through the interval.
#[diagnostic::on_unimplemented(
  message = "`{Self}` has no in-between value",
  label = "a value a transition samples implements `Interpolate`, answering `None` for a discrete pair"
)]
pub trait Interpolate: Clone {
  /// The value `amount` of the way from this one to `to`: `0` is this value, `1` is `to`;
  /// `None` for a discrete pair.
  #[must_use]
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self>;

  /// Whether this value and `to` have in-betweens, so a transition moves between them.
  #[must_use]
  fn interpolates_with(&self, to: &Self) -> bool {
    self.interpolate(to, 0.).is_some()
  }
}

/// The in-between of a discrete pair: `from` until halfway, `to` after.
fn snap<T: Clone>(from: &T, to: &T, amount: f32) -> T {
  if amount < 0.5 {
    from.clone()
  } else {
    to.clone()
  }
}

fn lerp(from: f32, to: f32, amount: f32) -> f32 {
  from + (to - from) * amount
}

impl Interpolate for f32 {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(lerp(*self, *to, amount))
  }
}

impl Interpolate for Color {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(self.mix(*to, amount))
  }
}

impl Interpolate for Corners {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(Self {
      top_leading: lerp(self.top_leading, to.top_leading, amount),
      top_trailing: lerp(self.top_trailing, to.top_trailing, amount),
      bottom_trailing: lerp(self.bottom_trailing, to.bottom_trailing, amount),
      bottom_leading: lerp(self.bottom_leading, to.bottom_leading, amount),
    })
  }
}

impl Interpolate for Dimension {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    match (self, to) {
      (Self::Length(from), Self::Length(to)) => Some(Self::Length(lerp(*from, *to, amount))),
      (Self::Fraction(from), Self::Fraction(to)) => Some(Self::Fraction(lerp(*from, *to, amount))),
      _ => None,
    }
  }
}

impl Interpolate for Inset {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(Self {
      top: lerp(self.top, to.top, amount),
      trailing: lerp(self.trailing, to.trailing, amount),
      bottom: lerp(self.bottom, to.bottom, amount),
      leading: lerp(self.leading, to.leading, amount),
    })
  }
}

impl Interpolate for Spacing {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(Self {
      horizontal: lerp(self.horizontal, to.horizontal, amount),
      vertical: lerp(self.vertical, to.vertical, amount),
    })
  }
}

impl Interpolate for Ring {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    Some(Self {
      width: lerp(self.width, to.width, amount),
      offset: lerp(self.offset, to.offset, amount),
    })
  }
}

impl Interpolate for Offset {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    (self.of_size == to.of_size).then(|| Self {
      x: lerp(self.x, to.x, amount),
      y: lerp(self.y, to.y, amount),
      of_size: self.of_size,
    })
  }
}

impl Interpolate for Border {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    (self.style == to.style).then(|| Self {
      width: lerp(self.width, to.width, amount),
      style: self.style,
    })
  }
}

impl Interpolate for Shadow {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    (self.inset == to.inset).then(|| Self {
      x: lerp(self.x, to.x, amount),
      y: lerp(self.y, to.y, amount),
      blur: lerp(self.blur, to.blur, amount),
      spread: lerp(self.spread, to.spread, amount),
      color: self.color.mix(to.color, amount),
      inset: self.inset,
    })
  }
}

impl Interpolate for Shadows {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    if self.0.len() != to.0.len() {
      return None;
    }

    self
      .0
      .iter()
      .zip(to.0.iter())
      .map(|(from, to)| from.interpolate(to, amount))
      .collect::<Option<Vec<_>>>()
      .map(|shadows| Self(shadows.into()))
  }
}

impl Interpolate for FontWeight {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "a weight between two weights"
    )]
    let weight = lerp(f32::from(self.0), f32::from(to.0), amount)
      .round()
      .clamp(1., 1000.) as u16;

    Some(Self(weight))
  }
}

impl Interpolate for Stops {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    if self.0.len() != to.0.len() {
      return None;
    }

    self
      .0
      .iter()
      .zip(to.0.iter())
      .map(|(from, to)| {
        Some(GradientStop {
          at: lerp(from.at, to.at, amount),
          color: StopColor::Fixed(from.color.fixed()?.mix(to.color.fixed()?, amount)),
        })
      })
      .collect::<Option<Vec<_>>>()
      .map(|stops| Self(stops.into()))
  }
}

impl Fill {
  /// The gradient `shape` has, every stop in `color`: the gradient a color moves to `shape`
  /// through.
  fn spread(color: Color, shape: &Self) -> Self {
    let stops = |stops: &Stops| {
      Stops(
        stops
          .0
          .iter()
          .map(|stop| GradientStop {
            at: stop.at,
            color: StopColor::Fixed(color),
          })
          .collect(),
      )
    };

    match shape {
      Self::Color(_) | Self::Ink(_) => Self::Color(color),
      Self::Linear(gradient) => Self::Linear(LinearGradient {
        stops: stops(&gradient.stops),
        ..gradient.clone()
      }),
      Self::Radial(gradient) => Self::Radial(RadialGradient {
        stops: stops(&gradient.stops),
        ..gradient.clone()
      }),
      Self::Sweep(gradient) => Self::Sweep(SweepGradient {
        stops: stops(&gradient.stops),
        ..gradient.clone()
      }),
    }
  }
}

impl Interpolate for Fill {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    match (self, to) {
      (Self::Color(from), Self::Color(to)) => Some(Self::Color(from.mix(*to, amount))),
      (Self::Ink(from), Self::Ink(to)) => Some(Self::Ink(Some(lerp(
        from.unwrap_or(1.),
        to.unwrap_or(1.),
        amount,
      )))),
      (Self::Ink(_), _) | (_, Self::Ink(_)) => None,
      (Self::Color(from), gradient) => Self::spread(*from, gradient).interpolate(gradient, amount),
      (gradient, Self::Color(to)) => gradient.interpolate(&Self::spread(*to, gradient), amount),
      (Self::Linear(from), Self::Linear(to)) => Some(Self::Linear(LinearGradient {
        angle: crate::fill::Angle::degrees(lerp(
          from.angle.in_degrees(),
          to.angle.in_degrees(),
          amount,
        )),
        stops: from.stops.interpolate(&to.stops, amount)?,
      })),
      (Self::Radial(from), Self::Radial(to)) => {
        let radius = match (from.radius, to.radius) {
          (None, None) => None,
          (Some(from), Some(to)) => Some(lerp(from, to, amount)),
          _ => return None,
        };

        Some(Self::Radial(RadialGradient {
          center: crate::fill::UnitPoint::new(
            lerp(from.center.x, to.center.x, amount),
            lerp(from.center.y, to.center.y, amount),
          ),
          radius,
          stops: from.stops.interpolate(&to.stops, amount)?,
        }))
      }
      (Self::Sweep(from), Self::Sweep(to)) => Some(Self::Sweep(SweepGradient {
        center: crate::fill::UnitPoint::new(
          lerp(from.center.x, to.center.x, amount),
          lerp(from.center.y, to.center.y, amount),
        ),
        start: crate::fill::Angle::degrees(lerp(
          from.start.in_degrees(),
          to.start.in_degrees(),
          amount,
        )),
        stops: from.stops.interpolate(&to.stops, amount)?,
      })),
      _ => None,
    }
  }
}

impl Interpolate for Resolved {
  fn interpolate(&self, to: &Self, amount: f32) -> Option<Self> {
    match (self, to) {
      (Self::Color(from), Self::Color(to)) => Some(Self::Color(from.interpolate(to, amount)?)),
      (Self::Fill(from), Self::Fill(to)) => Some(Self::Fill(from.interpolate(to, amount)?)),
      (Self::Number(from), Self::Number(to)) => Some(Self::Number(from.interpolate(to, amount)?)),
      (Self::Corners(from), Self::Corners(to)) => {
        Some(Self::Corners(from.interpolate(to, amount)?))
      }
      (Self::Border(from), Self::Border(to)) => Some(Self::Border(from.interpolate(to, amount)?)),
      (Self::Ring(from), Self::Ring(to)) => Some(Self::Ring(from.interpolate(to, amount)?)),
      (Self::Offset(from), Self::Offset(to)) => Some(Self::Offset(from.interpolate(to, amount)?)),
      (Self::Shadows(from), Self::Shadows(to)) => {
        Some(Self::Shadows(from.interpolate(to, amount)?))
      }
      (Self::FontWeight(from), Self::FontWeight(to)) => {
        Some(Self::FontWeight(from.interpolate(to, amount)?))
      }
      (Self::Dimension(from), Self::Dimension(to)) => {
        Some(Self::Dimension(from.interpolate(to, amount)?))
      }
      (Self::Inset(from), Self::Inset(to)) => Some(Self::Inset(from.interpolate(to, amount)?)),
      (Self::Spacing(from), Self::Spacing(to)) => {
        Some(Self::Spacing(from.interpolate(to, amount)?))
      }
      _ => None,
    }
  }
}

/// One property in flight: from the value it showed to the value it is due, over a
/// transition's timing, against a clock in seconds.
#[derive(Clone, PartialEq, Debug)]
struct Tween {
  from: Resolved,
  to: Resolved,
  reversing_from: Resolved,
  shortening: f32,
  start: f64,
  duration: f64,
  delay: f64,
  curve: Curve,
}

impl Tween {
  /// The linear progress at clock reading `now`, `0` to `1`.
  fn progress(&self, now: f64) -> f32 {
    let played = now - self.start - self.delay;

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
    let progress = (played / self.duration).min(1.) as f32;

    progress
  }

  /// Whether the tween has reached its end at clock reading `now`.
  fn is_done(&self, now: f64) -> bool {
    self.progress(now) >= 1.
  }

  /// The value shown at clock reading `now`.
  fn sample(&self, now: f64) -> Resolved {
    let amount = self.curve.apply(self.progress(now));

    self
      .from
      .interpolate(&self.to, amount)
      .unwrap_or_else(|| snap(&self.from, &self.to, amount))
  }

  /// The share of its full duration a tween reversing this one at `now` runs: the eased
  /// share of the way this one has come, as CSS's reversing shortening factor.
  fn reversing_shortening(&self, now: f64) -> f32 {
    (self.curve.apply(self.progress(now)) * self.shortening + (1. - self.shortening))
      .abs()
      .clamp(0., 1.)
  }
}

/// A node's keyframe animations with each keyframe's properties resolved: what
/// [`crate::seam::style::CompiledStyle::animations`] holds.
pub type Animations = [CompiledAnimation];

/// One node's motion on a platform's own clock: the tween of each property in flight, and the
/// clock reading its keyframe animations count from.
#[derive(Clone, Debug, Default)]
#[must_use]
pub struct Motion {
  tweens: Vec<(Prop, Tween)>,
  started: Option<f64>,
}

impl Motion {
  /// Motion with nothing in flight, its animations counting from the first clock reading it
  /// presents at.
  pub const fn new() -> Self {
    Self {
      tweens: Vec::new(),
      started: None,
    }
  }

  /// Starts the keyframe animations over, from the next clock reading presented at: what a
  /// node whose animations changed does.
  pub const fn restart(&mut self) {
    self.started = None;
  }

  /// The seconds the animations have played at clock reading `now`, their clock starting
  /// there unless it has.
  fn elapsed(&mut self, now: f64) -> f64 {
    now - *self.started.get_or_insert(now)
  }

  /// Judges `prop`'s resolved value moving from `previous` to `target` at clock reading
  /// `now`, as CSS starts a transition.
  ///
  /// Under the transition naming the property, a tween starts from the value showing toward
  /// `target`, unless `reduced` motion cuts that transition, the pair is discrete, or the
  /// property shows `target` already. A property that lays out tweens as any does; the
  /// realization lays out again each frame it moves.
  /// A change back to where a running tween began reverses it over the share of its duration
  /// it had played. Otherwise the property's tween stops, and `target` shows at once.
  pub fn retarget(
    &mut self,
    prop: Prop,
    previous: Option<&Resolved>,
    target: Option<&Resolved>,
    transitions: &[Transition],
    now: f64,
    reduced: bool,
  ) {
    if previous == target {
      return;
    }

    let running = self
      .tweens
      .iter()
      .position(|(held, _)| *held == prop)
      .map(|index| self.tweens.swap_remove(index).1)
      .filter(|tween| !tween.is_done(now));
    let Some(to) = target else {
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
      .find(|transition| transition.prop == prop)
      .filter(|transition| !(reduced && transition.respects_reduced_motion))
    else {
      return;
    };

    if transition.duration + transition.delay == 0 || from == *to || !from.interpolates_with(to) {
      return;
    }

    let (reversing_from, shortening) = match running {
      Some(running) if running.reversing_from == *to => {
        let shortening = running.reversing_shortening(now);

        (running.to, shortening)
      }
      _ => (from.clone(), 1.),
    };

    self.tweens.push((
      prop,
      Tween {
        from,
        to: to.clone(),
        reversing_from,
        shortening,
        start: now,
        duration: f64::from(transition.duration) / 1000. * f64::from(shortening),
        delay: f64::from(transition.delay) / 1000.,
        curve: transition.curve,
      },
    ));
  }

  /// Retargets every property whose resolved value differs between `previous` and `target`
  /// ([`Motion::retarget`]): what a realization calls each time a node's style resolves
  /// anew, whatever changed it (a condition, a token, a derived value).
  pub fn update(
    &mut self,
    previous: &[(Prop, Resolved)],
    target: &[(Prop, Resolved)],
    transitions: &[Transition],
    now: f64,
    reduced: bool,
  ) {
    fn find(props: &[(Prop, Resolved)], prop: Prop) -> Option<&Resolved> {
      props
        .iter()
        .find(|(name, _)| *name == prop)
        .map(|(_, value)| value)
    }

    let mut props: Vec<Prop> = previous
      .iter()
      .chain(target)
      .map(|(prop, _)| *prop)
      .collect();

    props.sort();
    props.dedup();

    for prop in props {
      self.retarget(
        prop,
        find(previous, prop),
        find(target, prop),
        transitions,
        now,
        reduced,
      );
    }
  }

  /// The value `prop`'s tween shows at clock reading `now`; `None` where none runs.
  #[must_use]
  pub fn sample(&self, prop: Prop, now: f64) -> Option<Resolved> {
    self
      .tweens
      .iter()
      .find(|(held, _)| *held == prop)
      .map(|(_, tween)| tween)
      .filter(|tween| !tween.is_done(now))
      .map(|tween| tween.sample(now))
  }

  /// What a node shows at clock reading `now`: `target` with each running tween laid over
  /// its property, then `animations` in the order added, each setting what its keyframes
  /// set where it stands (its last frame alone where `reduced` motion skips it); and whether
  /// a tween or an animation still runs, so another frame is due.
  pub fn present(
    &mut self,
    target: &[(Prop, Resolved)],
    animations: &Animations,
    now: f64,
    reduced: bool,
  ) -> (Vec<(Prop, Resolved)>, bool) {
    let mut shown: Vec<(Prop, Resolved)> = target
      .iter()
      .map(|(prop, value)| {
        (
          *prop,
          self.sample(*prop, now).unwrap_or_else(|| value.clone()),
        )
      })
      .collect();

    if !animations.is_empty() {
      let elapsed = self.elapsed(now);
      let mut props: Vec<Prop> = animations
        .iter()
        .flat_map(|animation| &animation.keyframes)
        .flat_map(|(_, props)| props.iter().map(|(prop, _)| *prop))
        .collect();

      props.sort();
      props.dedup();

      for prop in props {
        let at = shown.iter().position(|(name, _)| *name == prop);
        let underlying = at.map(|at| shown[at].1.clone());
        let value = animations.iter().fold(underlying, |value, animation| {
          let frames: Vec<(f32, Resolved)> = animation
            .keyframes
            .iter()
            .filter_map(|(offset, props)| {
              props
                .iter()
                .find(|(name, _)| *name == prop)
                .map(|(_, value)| (*offset, value.clone()))
            })
            .collect();

          sample_keyframes(
            &frames,
            animation.offset_under(elapsed, reduced),
            animation.curve,
            value.as_ref(),
          )
          .or(value)
        });

        match (at, value) {
          (Some(at), Some(value)) => shown[at].1 = value,
          (None, Some(value)) => shown.push((prop, value)),
          _ => {}
        }
      }
    }

    (shown, self.is_running(animations, now, reduced))
  }

  /// Whether a tween runs or one of `animations` plays at clock reading `now`; the tweens
  /// that have finished are forgotten.
  pub fn is_running(&mut self, animations: &Animations, now: f64, reduced: bool) -> bool {
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

/// The value of `frames` at `offset` (a fraction of the way through): the two frames around
/// it interpolated through `curve` over their interval; `None` for no frames.
///
/// Where no frame stands at `0` or `1`, `underlying` stands there, as CSS's neutral keyframes
/// do, and without it the nearest frame holds past either end. Of frames sharing an offset,
/// the last added stands, and a discrete pair turns halfway through its eased interval.
fn sample_keyframes(
  frames: &[(f32, Resolved)],
  offset: f32,
  curve: Curve,
  underlying: Option<&Resolved>,
) -> Option<Resolved> {
  if frames.is_empty() {
    return None;
  }

  let neutral = |edge: f32, at_edge: fn(f32) -> bool| {
    underlying
      .filter(|_| !frames.iter().any(|(at, _)| at_edge(*at)))
      .map(|value| (edge, value))
  };
  let first = neutral(0., |at| at <= 0.);
  let last = neutral(1., |at| at >= 1.);
  let all = || {
    first
      .into_iter()
      .chain(frames.iter().map(|(at, value)| (*at, value)))
      .chain(last)
  };
  let before = all()
    .filter(|(at, _)| *at <= offset)
    .max_by(|(left, _), (right, _)| left.total_cmp(right));
  let after = all()
    .rev()
    .filter(|(at, _)| *at >= offset)
    .min_by(|(left, _), (right, _)| left.total_cmp(right));

  match (before, after) {
    (Some((from_at, from)), Some((to_at, to))) => {
      let span = to_at - from_at;

      if span <= 0. {
        return Some(to.clone());
      }

      let amount = curve.apply((offset - from_at) / span);

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
  use std::rc::Rc;

  use super::{Interpolate, Motion};
  use crate::{
    Color, Expr, Fill, LinearGradient,
    fill::Angle,
    seam::record::{Animation, Prop, Resolved, Transition, keys},
    value::Curve,
  };

  fn background(color: u32) -> Vec<(Prop, Resolved)> {
    vec![(
      Prop::Background,
      Resolved::Fill(Fill::Color(Color::rgb(color))),
    )]
  }

  fn shown(props: &[(Prop, Resolved)]) -> Color {
    match &props[0].1 {
      Resolved::Fill(Fill::Color(color)) => *color,
      other => panic!("{other:?}"),
    }
  }

  /// The named curves start and end at rest, and the linear one is the identity.
  #[test]
  fn curves_run_from_zero_to_one() {
    for curve in [
      Curve::Linear,
      Curve::EaseIn,
      Curve::EaseOut,
      Curve::EaseInOut,
      Curve::Bezier(0.3, 0., 0.7, 1.),
    ] {
      assert!(curve.apply(0.).abs() < 1e-4, "{curve:?} at 0");
      assert!((curve.apply(1.) - 1.).abs() < 1e-4, "{curve:?} at 1");
    }

    assert!((Curve::Linear.apply(0.3) - 0.3).abs() < 1e-6);
    assert!(Curve::EaseIn.apply(0.5) < 0.5);
  }

  /// A change under a transition moves through in-betweens to its target and stops there; a
  /// reader preferring reduced motion sees the cut.
  #[test]
  fn a_change_tweens_then_rests() {
    let transitions = [Transition::new(keys::BACKGROUND, 200).set_curve(Curve::Linear)];
    let (white, black) = (background(0xffffff), background(0x000000));
    let mut motion = Motion::new();

    motion.update(&white, &black, &transitions, 10., false);

    let (half, running) = motion.present(&black, &[], 10.1, false);

    assert!(running);
    assert!(
      (0x50..0xb0).contains(&shown(&half).red),
      "{:?}",
      shown(&half)
    );
    assert_eq!(
      shown(&motion.present(&black, &[], 10.2, false).0),
      Color::BLACK
    );
    assert!(!motion.present(&black, &[], 10.2, false).1);

    let mut reduced = Motion::new();

    reduced.update(&white, &black, &transitions, 10., true);
    assert_eq!(
      shown(&reduced.present(&black, &[], 10., true).0),
      Color::BLACK
    );
  }

  /// A color moves to a gradient as that gradient in the color, and two gradients of a kind
  /// move per stop.
  #[test]
  fn fills_move_per_stop() {
    let gradient = Fill::Linear(
      LinearGradient::new(Angle::degrees(90.))
        .add_stop(0., Color::BLACK)
        .add_stop(1., Color::WHITE),
    );
    let half = Fill::Color(Color::WHITE)
      .interpolate(&gradient, 0.5)
      .expect("a color moves to a gradient");
    let Fill::Linear(half) = half else {
      panic!("{half:?}");
    };

    assert_eq!(half.stops.0.len(), 2);
    assert_eq!(half.stops.0[1].color.fixed(), Some(Color::WHITE));
    assert_ne!(half.stops.0[0].color.fixed(), Some(Color::WHITE));
  }

  /// The ink moves between its alphas, the ink in force taking none for its own, and cuts to
  /// or from a fill of another kind; drawn, it is the node's ink at its alpha.
  #[test]
  fn the_ink_moves_between_its_alphas() {
    let faint = Fill::from(Expr::<Color>::ink().with_alpha(0.25));
    let strong = Fill::from(Expr::<Color>::ink().with_alpha(0.75));

    assert_eq!(faint.interpolate(&strong, 0.5), Some(Fill::Ink(Some(0.5))));
    assert_eq!(
      Fill::Ink(None).interpolate(&faint, 0.5),
      Some(Fill::Ink(Some(0.625)))
    );
    assert_eq!(faint.interpolate(&Fill::Color(Color::WHITE), 0.5), None);
    assert_eq!(
      faint.in_ink(Color::BLACK).into_owned(),
      Fill::Color(Color::BLACK.with_alpha(64))
    );
  }

  /// An animation sets its keyframes' values where it stands, neutral at an edge it leaves
  /// open, and rests at its end.
  #[test]
  fn keyframes_play_and_rest() {
    let animation = Animation::new(1000)
      .set_curve(Curve::Linear)
      .map_keyframes(|_| -> Rc<[(Prop, Resolved)]> { Rc::from([]) });
    let animation = Animation {
      keyframes: vec![(1., Rc::from(background(0x000000)))],
      ..animation
    };
    let white = background(0xffffff);
    let mut motion = Motion::new();

    assert_eq!(
      shown(
        &motion
          .present(&white, std::slice::from_ref(&animation), 0., false)
          .0
      ),
      Color::WHITE
    );

    let (half, running) = motion.present(&white, std::slice::from_ref(&animation), 0.5, false);

    assert!(running);
    assert!((0x50..0xb0).contains(&shown(&half).red));
    assert!(!motion.present(&white, &[animation], 1.5, false).1);
  }
}
