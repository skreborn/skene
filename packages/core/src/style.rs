//! Declarative style dynamics: conditional variants, transitions, and keyframe
//! animations.
//!
//! These are **data**, not closures, so a server render compiles them into real CSS and a
//! static page stays responsive, hoverable and animated with no script. A native driver
//! resolves the same data against its own window and pointer state.

/// A condition under which a style variant applies.
///
/// Environment conditions (viewport, color scheme, motion preference) and interaction
/// states (hover, focus, press) compose: a variant guarded by several conditions applies
/// only when all of them hold.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum Condition {
  /// The viewport is at least this wide, in logical pixels.
  MinWidth(f32),
  /// The viewport is at most this wide, in logical pixels.
  MaxWidth(f32),
  /// The viewport is at least this tall, in logical pixels.
  MinHeight(f32),
  /// The viewport is at most this tall, in logical pixels.
  MaxHeight(f32),
  /// The user prefers a dark color scheme.
  DarkMode,
  /// The user prefers a light color scheme.
  LightMode,
  /// The user prefers reduced motion.
  ReducedMotion,
  /// The pointer is over the element.
  Hovered,
  /// The element has focus.
  Focused,
  /// The element has visibly-indicated (keyboard) focus.
  FocusVisible,
  /// The element is being activated (pressed).
  Pressed,
  /// The element is disabled.
  Disabled,
}

/// A styleable property, named for transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
  /// The padding.
  Padding,
  /// The background color.
  Background,
  /// The border color.
  BorderColor,
  /// The border width.
  BorderWidth,
  /// The corner radius.
  BorderRadius,
  /// The gap between children.
  Spacing,
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
