//! Declarative style dynamics: conditional variants, transitions, and keyframe
//! animations.
//!
//! These are **data**, not closures, so a server render compiles them into real CSS and a
//! static page stays responsive, hoverable and animated with no script. A native driver
//! resolves the same data against its own window and pointer state.

use smallvec::SmallVec;

use crate::condition::{Composable, ConditionShape};

/// A value and the conditions that override it: what a property is set to, before any
/// one view spells it out.
///
/// Every setter that takes a color or a border takes one of these, and a plain value
/// converts into a [`Fixed`](Self::Fixed) one—so `set_color(Color::WHITE)` and
/// `set_color(palette.card)` are the same call. Setting one fans it out into the
/// property and the conditional variants a view already carries, so nothing new reaches
/// a driver and the web still compiles the whole thing to one interned class.
///
/// This is what a design token is: a component library's palette is a struct of these,
/// and the library owns that struct outright. Nothing here knows what a palette is.
///
/// Generic over the condition vocabulary, which each facade aliases away—the terminal's
/// conditions are cells and its own background, the GUI's are pixels and the reader's
/// preference, and a token is expressed in whichever medium it belongs to.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum Adaptive<T, C = Condition> {
  /// One value, whatever holds.
  Fixed(T),
  /// A value overridden while a condition holds; later variants win over earlier ones.
  Conditional {
    /// The value applied when no variant's condition holds.
    base: T,
    /// The overriding values, in declaration order.
    variants: SmallVec<[(C, T); 1]>,
  },
}

impl<T, C> From<T> for Adaptive<T, C> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::Fixed(value)
  }
}

impl<T, C> Adaptive<T, C> {
  /// This value, overridden by `value` while `condition` holds.
  pub fn when(self, condition: C, value: T) -> Self {
    let (base, mut variants) = self.parts();

    variants.push((condition, value));

    Self::Conditional { base, variants }
  }

  /// The value applied when no variant's condition holds.
  #[inline(always)]
  pub const fn base(&self) -> &T {
    match self {
      Self::Fixed(base) | Self::Conditional { base, .. } => base,
    }
  }

  /// The overriding values, in declaration order.
  #[inline(always)]
  pub fn variants(&self) -> &[(C, T)] {
    match self {
      Self::Fixed(_) => &[],
      Self::Conditional { variants, .. } => variants,
    }
  }

  /// The base and the variants, for a setter fanning them into its own slots.
  #[inline(always)]
  pub fn parts(self) -> (T, SmallVec<[(C, T); 1]>) {
    match self {
      Self::Fixed(base) => (base, SmallVec::new()),
      Self::Conditional { base, variants } => (base, variants),
    }
  }

  /// This value with `wrap` applied to the base and to every variant—for a token that
  /// paints a property built from it, such as a color becoming a border.
  pub fn map<U>(self, wrap: impl Fn(T) -> U) -> Adaptive<U, C> {
    match self {
      Self::Fixed(base) => Adaptive::Fixed(wrap(base)),
      Self::Conditional { base, variants } => Adaptive::Conditional {
        base: wrap(base),
        variants: variants
          .into_iter()
          .map(|(condition, value)| (condition, wrap(value)))
          .collect(),
      },
    }
  }
}

impl<T, C: Composable> Adaptive<T, C> {
  /// A value that follows the platform's color scheme.
  #[inline(always)]
  pub fn schemes(light: T, dark: T) -> Self {
    Self::Fixed(light).when(C::dark_mode(), dark)
  }

  /// Every value this takes while `condition` also holds, base included—for a setter
  /// applying a token under a condition of its own, which narrows the token's own
  /// conditions rather than discarding them.
  ///
  /// Two inline, where a token stores one: a token that follows the color scheme is a
  /// base plus one variant, and narrowing turns both of those into variants.
  pub fn narrowed(self, condition: C) -> SmallVec<[(C, T); 2]> {
    let (base, variants) = self.parts();
    let mut narrowed: SmallVec<[(C, T); 2]> = variants
      .into_iter()
      .map(|(inner, value)| (condition.clone().both(inner), value))
      .collect();

    narrowed.insert(0, (condition, base));

    narrowed
  }
}

/// A condition under which a style variant applies.
///
/// Environment conditions (viewport, color scheme, motion preference) and interaction
/// states (hover, focus, press) are the leaves; [`and`](Self::and) and [`or`](Self::or)
/// compose them to any depth.
#[derive(Debug, Clone, PartialEq)]
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
  /// Every subcondition holds; empty always holds.
  And(Vec<Condition>),
  /// Any subcondition holds; empty never holds.
  Or(Vec<Condition>),
}

impl Condition {
  /// A condition holding while every one of `conditions` holds; empty always holds.
  #[inline(always)]
  pub fn and(conditions: impl IntoIterator<Item = Self>) -> Self {
    Self::And(conditions.into_iter().collect())
  }

  /// A condition holding while any one of `conditions` holds; empty never holds.
  #[inline(always)]
  pub fn or(conditions: impl IntoIterator<Item = Self>) -> Self {
    Self::Or(conditions.into_iter().collect())
  }
}

impl Composable for Condition {
  #[inline(always)]
  fn shape(&self) -> ConditionShape<'_, Self> {
    match self {
      Self::And(subconditions) => ConditionShape::And(subconditions),
      Self::Or(subconditions) => ConditionShape::Or(subconditions),
      _ => ConditionShape::Leaf(self),
    }
  }

  #[inline(always)]
  fn dark_mode() -> Self {
    Self::DarkMode
  }

  #[inline(always)]
  fn both(self, other: Self) -> Self {
    Self::And(vec![self, other])
  }
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
