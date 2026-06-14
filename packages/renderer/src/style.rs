//! The abstract style model.
//!
//! This is deliberately **semantic and backend-agnostic**: it describes *what*
//! a node should look like, not *how* any particular backend expresses that. In
//! particular it contains no CSS — the DOM backend is solely responsible for
//! translating a [`Style`] into CSS. A native backend would translate the same
//! [`Style`] into paint/layout commands, a TUI backend into cell attributes.
//!
//! [`Style`] is a flat bag of optional properties so that styles can be cheaply
//! merged (a later style overrides earlier set properties) and compared. App
//! code never assembles a [`Style`] directly: each widget exposes only the
//! properties that make sense for it and assembles a [`Style`] internally.

/// An RGBA color, 8 bits per channel.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub struct Color {
  pub r: u8,
  pub g: u8,
  pub b: u8,
  pub a: u8,
}

impl Color {
  pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
  pub const BLACK: Color = Color::rgb(0, 0, 0);
  pub const WHITE: Color = Color::rgb(255, 255, 255);

  #[inline(always)]
  pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
  }

  #[inline(always)]
  pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
  }
}

/// A length value.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Length {
  /// A length in (CSS/logical) pixels.
  Px(f64),
  /// A percentage of the relevant containing measure.
  Pct(f64),
  /// Automatic / content-driven sizing.
  Auto,
}

impl Length {
  pub const ZERO: Length = Length::Px(0.0);
}

impl From<f64> for Length {
  #[inline(always)]
  fn from(v: f64) -> Self {
    Length::Px(v)
  }
}

impl From<i32> for Length {
  #[inline(always)]
  fn from(v: i32) -> Self {
    Length::Px(f64::from(v))
  }
}

/// How children are laid out.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Display {
  Flex,
  Block,
  None,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Axis {
  Horizontal,
  Vertical,
}

/// Alignment along a flex container's main axis (maps to `justify-content`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MainAxisAlignment {
  Start,
  End,
  Center,
  SpaceBetween,
  SpaceAround,
  SpaceEvenly,
}

/// Alignment along a flex container's cross axis (maps to `align-items`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CrossAxisAlignment {
  Start,
  End,
  Center,
  Stretch,
  Baseline,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Cursor {
  Default,
  Pointer,
  Text,
}

/// A flat, mergeable bag of style properties. Every property is optional; an
/// unset property means "inherit / use the backend default".
///
/// This is the wire format between widgets and the backend, not an app-facing
/// API. Widgets set only the subset of fields they expose as typed props.
#[derive(Clone, PartialEq, Debug, Default)]
#[must_use]
pub struct Style {
  pub display: Option<Display>,
  pub flex_direction: Option<Axis>,
  pub main_axis_align: Option<MainAxisAlignment>,
  pub cross_axis_align: Option<CrossAxisAlignment>,
  pub gap: Option<Length>,
  pub flex_grow: Option<f32>,
  pub flex_shrink: Option<f32>,
  pub flex_basis: Option<Length>,

  pub width: Option<Length>,
  pub height: Option<Length>,
  pub min_width: Option<Length>,
  pub min_height: Option<Length>,
  pub max_width: Option<Length>,
  pub max_height: Option<Length>,

  pub padding_top: Option<Length>,
  pub padding_right: Option<Length>,
  pub padding_bottom: Option<Length>,
  pub padding_left: Option<Length>,

  pub background: Option<Color>,
  pub color: Option<Color>,
  pub font_size: Option<Length>,
  pub font_weight: Option<u16>,

  pub border_width: Option<Length>,
  pub border_color: Option<Color>,
  pub border_radius: Option<Length>,

  pub cursor: Option<Cursor>,
}

impl Style {
  #[inline(always)]
  pub fn new() -> Self {
    Style::default()
  }

  /// Merge `other` on top of `self`: every property set in `other` overrides
  /// the corresponding property in `self`.
  pub fn merge(mut self, other: &Style) -> Self {
    macro_rules! over {
            ($($f:ident),* $(,)?) => {
                $( if other.$f.is_some() { self.$f = other.$f; } )*
            };
        }
    over!(
      display,
      flex_direction,
      main_axis_align,
      cross_axis_align,
      gap,
      flex_grow,
      flex_shrink,
      flex_basis,
      width,
      height,
      min_width,
      min_height,
      max_width,
      max_height,
      padding_top,
      padding_right,
      padding_bottom,
      padding_left,
      background,
      color,
      font_size,
      font_weight,
      border_width,
      border_color,
      border_radius,
      cursor,
    );
    self
  }
}

// TODO: Font size shouldn't be a length, just a number
// TODO: Color should be oklch (and convertable from various standards)

/// Fluent setters used by widgets to assemble a [`Style`]. These only set
/// fields — they have no knowledge of any backend or of CSS.
impl Style {
  #[inline(always)]
  pub fn display(mut self, v: Display) -> Self {
    self.display = Some(v);
    self
  }
  #[inline(always)]
  pub fn flex_direction(mut self, v: Axis) -> Self {
    self.flex_direction = Some(v);
    self
  }
  #[inline(always)]
  pub fn main_axis_align(mut self, v: MainAxisAlignment) -> Self {
    self.main_axis_align = Some(v);
    self
  }
  #[inline(always)]
  pub fn cross_axis_align(mut self, v: CrossAxisAlignment) -> Self {
    self.cross_axis_align = Some(v);
    self
  }
  #[inline(always)]
  pub fn gap(mut self, v: impl Into<Length>) -> Self {
    self.gap = Some(v.into());
    self
  }
  #[inline(always)]
  pub fn flex_grow(mut self, v: f32) -> Self {
    self.flex_grow = Some(v);
    self
  }
  #[inline(always)]
  pub fn flex_shrink(mut self, v: f32) -> Self {
    self.flex_shrink = Some(v);
    self
  }
  #[inline(always)]
  pub fn flex_basis(mut self, v: impl Into<Length>) -> Self {
    self.flex_basis = Some(v.into());
    self
  }

  #[inline(always)]
  pub fn width(mut self, v: impl Into<Length>) -> Self {
    self.width = Some(v.into());
    self
  }
  #[inline(always)]
  pub fn height(mut self, v: impl Into<Length>) -> Self {
    self.height = Some(v.into());
    self
  }
  #[inline(always)]
  pub fn min_width(mut self, v: impl Into<Length>) -> Self {
    self.min_width = Some(v.into());
    self
  }
  #[inline(always)]
  pub fn min_height(mut self, v: impl Into<Length>) -> Self {
    self.min_height = Some(v.into());
    self
  }
  #[inline(always)]
  pub fn max_width(mut self, v: impl Into<Length>) -> Self {
    self.max_width = Some(v.into());
    self
  }
  #[inline(always)]
  pub fn max_height(mut self, v: impl Into<Length>) -> Self {
    self.max_height = Some(v.into());
    self
  }

  pub fn padding(mut self, v: impl Into<Length>) -> Self {
    let v = v.into();
    self.padding_top = Some(v);
    self.padding_right = Some(v);
    self.padding_bottom = Some(v);
    self.padding_left = Some(v);
    self
  }
  pub fn padding_horiz(mut self, v: impl Into<Length>) -> Self {
    let v = v.into();
    self.padding_left = Some(v);
    self.padding_right = Some(v);
    self
  }
  pub fn padding_vert(mut self, v: impl Into<Length>) -> Self {
    let v = v.into();
    self.padding_top = Some(v);
    self.padding_bottom = Some(v);
    self
  }

  #[inline(always)]
  pub fn background(mut self, v: Color) -> Self {
    self.background = Some(v);
    self
  }
  #[inline(always)]
  pub fn color(mut self, v: Color) -> Self {
    self.color = Some(v);
    self
  }
  #[inline(always)]
  pub fn font_size(mut self, v: impl Into<Length>) -> Self {
    self.font_size = Some(v.into());
    self
  }
  #[inline(always)]
  pub fn font_weight(mut self, v: u16) -> Self {
    self.font_weight = Some(v);
    self
  }

  #[inline(always)]
  pub fn border(mut self, width: impl Into<Length>, color: Color) -> Self {
    self.border_width = Some(width.into());
    self.border_color = Some(color);
    self
  }
  #[inline(always)]
  pub fn border_radius(mut self, v: impl Into<Length>) -> Self {
    self.border_radius = Some(v.into());
    self
  }

  #[inline(always)]
  pub fn cursor(mut self, v: Cursor) -> Self {
    self.cursor = Some(v);
    self
  }
}
