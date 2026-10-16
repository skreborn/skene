//! What fills a box or its border ([`Fill`]): one color, or a gradient whose stops lie along
//! a line ([`LinearGradient`]), out from a center ([`RadialGradient`]) or around it
//! ([`SweepGradient`]).
//!
//! A gradient lies over the box it fills as CSS lays its gradients over the border box, so a
//! platform that keeps the style unresolved maps it whole (`linear-gradient`,
//! `radial-gradient`, `conic-gradient`) and one that draws places it the same way. A stop's
//! color is fixed or a color token, resolved where the fill is drawn, so a theme's
//! redefinition re-inks a gradient as it re-inks a color.

use std::rc::Rc;

use crate::{Color, Property, TokenRef, TokenValue, Tokens, Value, value::Leaf};

/// An angle, as a compass bearing: `0` points up and it grows clockwise, so `90` points to the
/// trailing side and `180` down.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub struct Angle(f32);

impl Angle {
  /// An angle of `degrees`.
  #[inline(always)]
  pub const fn degrees(degrees: f32) -> Self {
    Self(degrees)
  }

  /// An angle of `turns` whole turns.
  #[inline(always)]
  pub const fn turns(turns: f32) -> Self {
    Self(turns * 360.)
  }

  /// The angle in degrees.
  #[inline(always)]
  #[must_use]
  pub const fn in_degrees(self) -> f32 {
    self.0
  }

  /// The angle in radians.
  #[inline(always)]
  #[must_use]
  pub const fn in_radians(self) -> f32 {
    self.0.to_radians()
  }
}

/// A point of a box, as fractions of its width and height from its top leading corner:
/// `(0, 0)` is that corner, `(1, 1)` the bottom trailing one.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub struct UnitPoint {
  /// How far across, a fraction of the width.
  pub x: f32,
  /// How far down, a fraction of the height.
  pub y: f32,
}

impl UnitPoint {
  /// The top leading corner.
  pub const TOP_LEADING: Self = Self::new(0., 0.);
  /// The middle of the top edge.
  pub const TOP: Self = Self::new(0.5, 0.);
  /// The top trailing corner.
  pub const TOP_TRAILING: Self = Self::new(1., 0.);
  /// The middle of the leading edge.
  pub const LEADING: Self = Self::new(0., 0.5);
  /// The center.
  pub const CENTER: Self = Self::new(0.5, 0.5);
  /// The middle of the trailing edge.
  pub const TRAILING: Self = Self::new(1., 0.5);
  /// The bottom leading corner.
  pub const BOTTOM_LEADING: Self = Self::new(0., 1.);
  /// The middle of the bottom edge.
  pub const BOTTOM: Self = Self::new(0.5, 1.);
  /// The bottom trailing corner.
  pub const BOTTOM_TRAILING: Self = Self::new(1., 1.);

  /// The point `x` of the way across and `y` of the way down.
  #[inline(always)]
  pub const fn new(x: f32, y: f32) -> Self {
    Self { x, y }
  }
}

impl Default for UnitPoint {
  #[inline(always)]
  fn default() -> Self {
    Self::CENTER
  }
}

/// A stop's color: fixed, or the color token of this name, resolved where the fill is drawn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StopColor {
  /// A fixed color.
  Fixed(Color),
  /// A color token, by name.
  Token(&'static str),
}

impl StopColor {
  /// The color now, a token resolved through `tokens`; `None` where it is defined nowhere
  /// above.
  #[must_use]
  pub fn resolve(self, tokens: &Tokens) -> Option<Color> {
    match self {
      Self::Fixed(color) => Some(color),
      Self::Token(name) => match tokens.resolve(name)? {
        TokenValue::Color(color) => Some(color),
        TokenValue::Number(_) => None,
      },
    }
  }

  /// The fixed color, where it is one.
  #[inline(always)]
  #[must_use]
  pub const fn fixed(self) -> Option<Color> {
    match self {
      Self::Fixed(color) => Some(color),
      Self::Token(_) => None,
    }
  }
}

impl From<Color> for StopColor {
  #[inline(always)]
  fn from(color: Color) -> Self {
    Self::Fixed(color)
  }
}

impl From<TokenRef<Color>> for StopColor {
  #[inline(always)]
  fn from(token: TokenRef<Color>) -> Self {
    Self::Token(token.name())
  }
}

/// A gradient's stop: a fraction of the way along it, and the color there.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GradientStop {
  /// How far along, `0` its start and `1` its end.
  pub at: f32,
  /// The color there.
  pub color: StopColor,
}

/// A gradient's stops, in the order added: between two, the color moves from one to the
/// other in sRGB, premultiplied; before the first and after the last, it holds.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Stops(pub Rc<[GradientStop]>);

impl Stops {
  /// These stops with `color` added `at` a fraction of the way along.
  fn with(&self, at: f32, color: StopColor) -> Self {
    let mut stops = self.0.to_vec();

    stops.push(GradientStop { at, color });

    Self(stops.into())
  }

  /// These stops with every token resolved through `tokens`; `None` where one is defined
  /// nowhere above.
  fn resolved(&self, tokens: &Tokens) -> Option<Self> {
    if self.0.iter().all(|stop| stop.color.fixed().is_some()) {
      return Some(self.clone());
    }

    self
      .0
      .iter()
      .map(|stop| {
        Some(GradientStop {
          at: stop.at,
          color: StopColor::Fixed(stop.color.resolve(tokens)?),
        })
      })
      .collect::<Option<Vec<_>>>()
      .map(|stops| Self(stops.into()))
  }
}

/// A gradient along a line through the box's center at an angle, long enough that the
/// corners its ends point at take the first and the last stop's colors (CSS's
/// `linear-gradient(<angle>, …)`).
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct LinearGradient {
  /// The way the line runs, from its start to its end.
  pub angle: Angle,
  /// The stops.
  pub stops: Stops,
}

impl LinearGradient {
  /// A gradient running toward `angle` (`Angle::degrees(180.)` runs down), with no stops.
  #[inline(always)]
  pub fn new(angle: Angle) -> Self {
    Self {
      angle,
      stops: Stops::default(),
    }
  }

  /// Adds a stop of `color` `at` a fraction of the way along, `0` to `1`.
  pub fn add_stop(mut self, at: f32, color: impl Into<StopColor>) -> Self {
    self.stops = self.stops.with(at, color.into());

    self
  }
}

/// A gradient out from a center (CSS's `radial-gradient`): to the ellipse through the box's
/// farthest corner, shaped as the box is, unless a radius makes it a circle.
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct RadialGradient {
  /// Where it starts.
  pub center: UnitPoint,
  /// The circle's radius in pixels, or `None` for the ellipse through the farthest corner.
  pub radius: Option<f32>,
  /// The stops.
  pub stops: Stops,
}

impl RadialGradient {
  /// A gradient out from the center to the ellipse through the farthest corner, with no stops.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      center: UnitPoint::CENTER,
      radius: None,
      stops: Stops::default(),
    }
  }

  /// Where it starts; the center where unset.
  pub const fn set_center(mut self, center: UnitPoint) -> Self {
    self.center = center;

    self
  }

  /// A circle of `radius` pixels where its last stop lies, in place of the ellipse through
  /// the farthest corner.
  pub fn set_radius(mut self, radius: impl crate::IntoUnit<f32>) -> Self {
    self.radius = Some(radius.into_unit());

    self
  }

  /// Adds a stop of `color` `at` a fraction of the way out, `0` to `1`.
  pub fn add_stop(mut self, at: f32, color: impl Into<StopColor>) -> Self {
    self.stops = self.stops.with(at, color.into());

    self
  }
}

impl Default for RadialGradient {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

/// A gradient around a center, one clockwise turn from a starting angle (CSS's
/// `conic-gradient`).
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct SweepGradient {
  /// What it turns around.
  pub center: UnitPoint,
  /// Where the turn starts.
  pub start: Angle,
  /// The stops.
  pub stops: Stops,
}

impl SweepGradient {
  /// A gradient around the center, from straight up, with no stops.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      center: UnitPoint::CENTER,
      start: Angle::default(),
      stops: Stops::default(),
    }
  }

  /// What it turns around; the center where unset.
  pub const fn set_center(mut self, center: UnitPoint) -> Self {
    self.center = center;

    self
  }

  /// Where the turn starts; straight up where unset.
  pub const fn set_start(mut self, start: Angle) -> Self {
    self.start = start;

    self
  }

  /// Adds a stop of `color` `at` a fraction of the turn, `0` to `1`.
  pub fn add_stop(mut self, at: f32, color: impl Into<StopColor>) -> Self {
    self.stops = self.stops.with(at, color.into());

    self
  }
}

impl Default for SweepGradient {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

/// What fills a box or its border: one color, or a gradient. Every color converts into it,
/// so `set_background(Color::WHITE)` and `set_background(palette.card)` stay one call.
#[derive(Clone, PartialEq, Debug)]
pub enum Fill {
  /// One color.
  Color(Color),
  /// A gradient along a line.
  Linear(LinearGradient),
  /// A gradient out from a center.
  Radial(RadialGradient),
  /// A gradient around a center.
  Sweep(SweepGradient),
}

impl Fill {
  /// The stops, where it is a gradient.
  #[must_use]
  pub const fn stops(&self) -> Option<&Stops> {
    match self {
      Self::Color(_) => None,
      Self::Linear(gradient) => Some(&gradient.stops),
      Self::Radial(gradient) => Some(&gradient.stops),
      Self::Sweep(gradient) => Some(&gradient.stops),
    }
  }

  /// The one color, where it is one.
  #[must_use]
  pub const fn color(&self) -> Option<Color> {
    match self {
      Self::Color(color) => Some(*color),
      _ => None,
    }
  }

  /// Whether it draws nothing: a transparent color, or a gradient whose every stop is.
  #[must_use]
  pub fn is_clear(&self) -> bool {
    match self {
      Self::Color(color) => color.alpha == 0,
      _ => self.stops().is_some_and(|stops| {
        stops
          .0
          .iter()
          .all(|stop| stop.color.fixed().is_some_and(|color| color.alpha == 0))
      }),
    }
  }
}

impl Property for Fill {
  #[inline(always)]
  fn from_token(value: &TokenValue) -> Option<Self> {
    match value {
      TokenValue::Color(color) => Some(Self::Color(*color)),
      TokenValue::Number(_) => None,
    }
  }

  fn resolve_tokens(&self, tokens: &Tokens) -> Option<Self> {
    Some(match self {
      Self::Color(color) => Self::Color(*color),
      Self::Linear(gradient) => Self::Linear(LinearGradient {
        stops: gradient.stops.resolved(tokens)?,
        ..gradient.clone()
      }),
      Self::Radial(gradient) => Self::Radial(RadialGradient {
        stops: gradient.stops.resolved(tokens)?,
        ..gradient.clone()
      }),
      Self::Sweep(gradient) => Self::Sweep(SweepGradient {
        stops: gradient.stops.resolved(tokens)?,
        ..gradient.clone()
      }),
    })
  }
}

impl From<Color> for Fill {
  #[inline(always)]
  fn from(color: Color) -> Self {
    Self::Color(color)
  }
}

impl From<LinearGradient> for Fill {
  #[inline(always)]
  fn from(gradient: LinearGradient) -> Self {
    Self::Linear(gradient)
  }
}

impl From<RadialGradient> for Fill {
  #[inline(always)]
  fn from(gradient: RadialGradient) -> Self {
    Self::Radial(gradient)
  }
}

impl From<SweepGradient> for Fill {
  #[inline(always)]
  fn from(gradient: SweepGradient) -> Self {
    Self::Sweep(gradient)
  }
}

/// A fill of one color, as a fixed value.
impl From<Color> for Value<Fill> {
  #[inline(always)]
  fn from(color: Color) -> Self {
    Self::leaf(Leaf::Fixed(Fill::Color(color)))
  }
}

/// A fill of one color token's color.
impl From<TokenRef<Color>> for Value<Fill> {
  #[inline(always)]
  fn from(token: TokenRef<Color>) -> Self {
    Self::leaf(Leaf::Token(token.name()))
  }
}

/// A gradient along a line, as a fixed value.
impl From<LinearGradient> for Value<Fill> {
  #[inline(always)]
  fn from(gradient: LinearGradient) -> Self {
    Self::leaf(Leaf::Fixed(Fill::Linear(gradient)))
  }
}

/// A gradient out from a center, as a fixed value.
impl From<RadialGradient> for Value<Fill> {
  #[inline(always)]
  fn from(gradient: RadialGradient) -> Self {
    Self::leaf(Leaf::Fixed(Fill::Radial(gradient)))
  }
}

/// A gradient around a center, as a fixed value.
impl From<SweepGradient> for Value<Fill> {
  #[inline(always)]
  fn from(gradient: SweepGradient) -> Self {
    Self::leaf(Leaf::Fixed(Fill::Sweep(gradient)))
  }
}
