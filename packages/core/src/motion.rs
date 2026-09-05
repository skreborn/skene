//! Motion declared as data: transitions and keyframe animations, compiled or run by
//! every driver.

/// A styleable property, named for transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum StyleProperty {
  /// Every animatable property.
  All,
  /// The width.
  Width,
  /// The height.
  Height,
  /// The minimum width.
  MinWidth,
  /// The minimum height.
  MinHeight,
  /// The maximum width.
  MaxWidth,
  /// The maximum height.
  MaxHeight,
  /// The width-to-height ratio.
  AspectRatio,
  /// The margin.
  Margin,
  /// The padding.
  Padding,
  /// The positioned offset.
  Inset,
  /// The background color.
  Background,
  /// The border color.
  BorderColor,
  /// The border width.
  BorderWidth,
  /// The corner radius.
  BorderRadius,
  /// The spacing between children.
  Spacing,
  /// The flexibility as a flex child.
  Flex,
  /// The text color.
  TextColor,
  /// The font size.
  FontSize,
  /// The font weight.
  FontWeight,
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
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Transition {
  /// The property to interpolate.
  pub property: StyleProperty,
  /// The interpolation length, in seconds.
  pub duration: f32,
  /// The delay before it starts, in seconds.
  pub delay: f32,
  /// The timing curve.
  pub easing: Easing,
}

impl Transition {
  /// A transition of `property` over `duration` seconds, easing with no delay.
  #[inline(always)]
  pub const fn new(property: StyleProperty, duration: f32) -> Self {
    Self {
      property,
      duration,
      delay: 0.,
      easing: Easing::Ease,
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
}

/// A keyframe animation over a style: each step is a percentage of the way through and
/// the style at that point.
#[derive(Clone, PartialEq)]
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
}

impl<S> Animation<S> {
  /// An animation over `keyframes` lasting `duration` seconds, playing once, easing, with
  /// no delay.
  pub fn new(duration: f32, keyframes: impl IntoIterator<Item = (f32, S)>) -> Self {
    Self {
      keyframes: keyframes.into_iter().collect(),
      duration,
      delay: 0.,
      easing: Easing::Ease,
      iterations: Some(1),
      alternate: false,
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
}
