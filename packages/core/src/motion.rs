//! Motion declared as data: transitions and keyframe animations, compiled or run by
//! every platform.

pub mod interpolate;

use std::fmt::Debug;

use super::{NoProperty, Property, PropertyKey};

/// The property a transition targets: every animatable property, or one by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum StyleProperty<P = NoProperty> {
  /// Every animatable property.
  All,
  /// A property both facades share.
  Shared(Property),
  /// A property the medium's style extension adds.
  Extension(P),
}

impl<P: PartialEq> StyleProperty<P> {
  /// Whether a transition of this target interpolates the property `key`.
  #[inline(always)]
  #[must_use]
  pub fn covers(&self, key: &PropertyKey<P>) -> bool {
    match (self, key) {
      (Self::All, _) => true,
      (Self::Shared(mine), PropertyKey::Shared(theirs)) => mine == theirs,
      (Self::Extension(mine), PropertyKey::Extension(theirs)) => mine == theirs,
      (Self::Shared(_), PropertyKey::Extension(_))
      | (Self::Extension(_), PropertyKey::Shared(_)) => false,
    }
  }
}

impl<P> From<Property> for StyleProperty<P> {
  #[inline(always)]
  fn from(property: Property) -> Self {
    Self::Shared(property)
  }
}

impl<P> From<PropertyKey<P>> for StyleProperty<P> {
  #[inline(always)]
  fn from(key: PropertyKey<P>) -> Self {
    match key {
      PropertyKey::Shared(property) => Self::Shared(property),
      PropertyKey::Extension(property) => Self::Extension(property),
    }
  }
}

/// A timing curve.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum Easing {
  /// Constant speed.
  Linear,
  /// Slow start and end, faster middle.
  Ease,
  /// Slow start.
  EaseIn,
  /// Slow end.
  EaseOut,
  /// Slow start and end.
  EaseInOut,
  /// A custom cubic-bezier curve (`x1`, `y1`, `x2`, `y2`).
  CubicBezier(f32, f32, f32, f32),
}

/// An automatic interpolation of one property between its changing values.
///
/// `P` names the medium's own properties; a transition of a shared property is written
/// the same on every facade: `Transition::new(Property::Color, 0.3)`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Transition<P = NoProperty> {
  /// The property to interpolate.
  pub property: StyleProperty<P>,
  /// The interpolation length, in seconds.
  pub duration: f32,
  /// The delay before it starts, in seconds.
  pub delay: f32,
  /// The timing curve.
  pub easing: Easing,
  /// Whether a reader preferring reduced motion sees the change as a cut instead.
  pub respects_reduced_motion: bool,
}

impl<P> Transition<P> {
  /// A transition of `property` over `duration` seconds, easing with no delay,
  /// respecting reduced motion.
  #[inline(always)]
  pub fn new(property: impl Into<StyleProperty<P>>, duration: f32) -> Self {
    Self {
      property: property.into(),
      duration,
      delay: 0.,
      easing: Easing::Ease,
      respects_reduced_motion: true,
    }
  }

  /// Sets the delay before the transition starts, in seconds.
  #[inline(always)]
  pub const fn set_delay(mut self, delay: f32) -> Self {
    self.delay = delay;

    self
  }

  /// Sets the timing curve.
  #[inline(always)]
  pub const fn set_easing(mut self, easing: Easing) -> Self {
    self.easing = easing;

    self
  }

  /// Sets whether a reader preferring reduced motion sees the change as a cut; on by
  /// default.
  #[inline(always)]
  pub const fn set_respect_reduced_motion(mut self, respect: bool) -> Self {
    self.respects_reduced_motion = respect;

    self
  }
}

/// A keyframe animation over a style: each step is a percentage of the way through and
/// the style at that point.
///
/// A keyframe is a style in the [`Keyframe`](super::Keyframe) shape, so each of its
/// leaves is a fixed value, a token or an expression.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Animation<S> {
  /// The steps, each a percentage (`0.` to `100.`) and the style at that point.
  pub keyframes: Vec<(f32, S)>,
  /// One cycle's length, in seconds.
  pub duration: f32,
  /// The delay before it starts, in seconds.
  pub delay: f32,
  /// The timing curve.
  pub easing: Easing,
  /// How many times it plays, or `None` to repeat forever.
  pub iterations: Option<u32>,
  /// Whether every other cycle plays in reverse.
  pub alternate: bool,
  /// Whether a reader preferring reduced motion sees the final frame alone.
  pub respects_reduced_motion: bool,
}

impl<S> Animation<S> {
  /// An animation over `keyframes` lasting `duration` seconds, playing once, easing, with
  /// no delay, respecting reduced motion.
  pub fn new(duration: f32, keyframes: impl IntoIterator<Item = (f32, S)>) -> Self {
    Self {
      keyframes: keyframes.into_iter().collect(),
      duration,
      delay: 0.,
      easing: Easing::Ease,
      iterations: Some(1),
      alternate: false,
      respects_reduced_motion: true,
    }
  }

  /// Sets the delay before the animation starts, in seconds.
  #[inline(always)]
  pub fn set_delay(mut self, delay: f32) -> Self {
    self.delay = delay;

    self
  }

  /// Sets the timing curve.
  #[inline(always)]
  pub fn set_easing(mut self, easing: Easing) -> Self {
    self.easing = easing;

    self
  }

  /// Sets how many times the animation plays.
  #[inline(always)]
  pub fn set_iterations(mut self, iterations: u32) -> Self {
    self.iterations = Some(iterations);

    self
  }

  /// Makes the animation repeat forever.
  #[inline(always)]
  pub fn set_infinite(mut self) -> Self {
    self.iterations = None;

    self
  }

  /// Makes every other cycle play in reverse.
  #[inline(always)]
  pub fn set_alternate(mut self) -> Self {
    self.alternate = true;

    self
  }

  /// Sets whether a reader preferring reduced motion sees the final frame alone; on by
  /// default.
  #[inline(always)]
  pub fn set_respect_reduced_motion(mut self, respect: bool) -> Self {
    self.respects_reduced_motion = respect;

    self
  }

  /// Where the animation is `elapsed` seconds after it began, as a percentage through
  /// its keyframes: the first frame before the delay, the last iteration's end once it
  /// has played out.
  #[must_use]
  pub fn offset(&self, elapsed: f64) -> f32 {
    let played = elapsed - f64::from(self.delay);

    if played <= 0. {
      return 0.;
    }

    if self.duration <= 0. {
      return self.end_offset();
    }

    let cycles = played / f64::from(self.duration);

    if self
      .iterations
      .is_some_and(|iterations| cycles >= f64::from(iterations))
    {
      return self.end_offset();
    }

    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let iteration = cycles.floor() as u64;
    #[expect(clippy::cast_possible_truncation)]
    let local = cycles.fract() as f32;
    let reversed = self.alternate && iteration % 2 == 1;
    let directed = if reversed { 1. - local } else { local };

    self.easing.apply(directed) * 100.
  }

  /// Whether the animation has played out `elapsed` seconds after it began.
  #[inline(always)]
  #[must_use]
  pub fn is_done(&self, elapsed: f64) -> bool {
    match self.iterations {
      Some(iterations) => {
        elapsed >= f64::from(self.delay) + f64::from(self.duration) * f64::from(iterations)
      }
      None => false,
    }
  }

  /// The offset the animation rests at after its last iteration.
  fn end_offset(&self) -> f32 {
    let last = self.iterations.unwrap_or(1).saturating_sub(1);

    if self.alternate && last % 2 == 1 {
      0.
    }
    else {
      100.
    }
  }

  /// The animation with each keyframe mapped through `map`.
  pub fn map_keyframes<T>(&self, mut map: impl FnMut(&S) -> T) -> Animation<T> {
    Animation {
      keyframes: self
        .keyframes
        .iter()
        .map(|(percent, style)| (*percent, map(style)))
        .collect(),
      duration: self.duration,
      delay: self.delay,
      easing: self.easing,
      iterations: self.iterations,
      alternate: self.alternate,
      respects_reduced_motion: self.respects_reduced_motion,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{Animation, Easing, Property, StyleProperty, Transition};
  use crate::{NoProperty, PropertyKey};

  /// A transition named after a shared property covers exactly that property; `All`
  /// covers every one.
  #[test]
  fn targets_cover_their_properties() {
    let transition = Transition::<NoProperty>::new(Property::Color, 0.2);

    assert!(
      transition
        .property
        .covers(&PropertyKey::Shared(Property::Color))
    );
    assert!(
      !transition
        .property
        .covers(&PropertyKey::Shared(Property::Width))
    );
    assert!(StyleProperty::<NoProperty>::All.covers(&PropertyKey::Shared(Property::Width)));
    assert!(transition.respects_reduced_motion);
  }

  /// An animation's offset runs from its first frame to its last, alternates when
  /// asked, and rests at the end of its final iteration.
  #[test]
  #[expect(clippy::float_cmp)]
  fn offsets_follow_iterations_and_alternation() {
    let animation = Animation::new(1., [(0., ()), (100., ())])
      .set_easing(Easing::Linear)
      .set_iterations(2)
      .set_alternate()
      .set_delay(0.5);

    assert_eq!(animation.offset(0.), 0.);
    assert_eq!(animation.offset(0.25), 0.);
    assert_eq!(animation.offset(1.), 50.);
    assert_eq!(animation.offset(1.5), 100.);
    assert_eq!(animation.offset(2.25), 25.);
    assert_eq!(animation.offset(3.), 0.);
    assert!(animation.is_done(2.5));
    assert!(!animation.is_done(2.4));

    let forever = Animation::new(1., [(0., ()), (100., ())]).set_infinite();

    assert!(!forever.is_done(1e6));
  }
}
