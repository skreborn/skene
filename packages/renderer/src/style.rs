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
//!
//! Sizes are plain `f64` logical pixels.

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

/// Spacing on the four edges of a box, in logical pixels (à la Flutter's
/// `EdgeInsets`). A single `f64` converts to equal insets on all sides, so a
/// widget's `padding` prop accepts both `40.0` and `EdgeInsets::symmetric(..)`.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub struct EdgeInsets {
  pub top: f64,
  pub right: f64,
  pub bottom: f64,
  pub left: f64,
}

impl EdgeInsets {
  pub const ZERO: EdgeInsets = EdgeInsets::all(0.0);

  /// Equal insets on all four sides.
  #[inline(always)]
  pub const fn all(value: f64) -> EdgeInsets {
    EdgeInsets {
      top: value,
      right: value,
      bottom: value,
      left: value,
    }
  }

  /// Insets that are symmetric about each axis.
  #[inline(always)]
  pub const fn symmetric(horizontal: f64, vertical: f64) -> EdgeInsets {
    EdgeInsets {
      top: vertical,
      right: horizontal,
      bottom: vertical,
      left: horizontal,
    }
  }

  /// Insets on the left and right edges only.
  #[inline(always)]
  pub const fn horizontal(value: f64) -> EdgeInsets {
    EdgeInsets::symmetric(value, 0.0)
  }

  /// Insets on the top and bottom edges only.
  #[inline(always)]
  pub const fn vertical(value: f64) -> EdgeInsets {
    EdgeInsets::symmetric(0.0, value)
  }
}

impl From<f64> for EdgeInsets {
  #[inline(always)]
  fn from(value: f64) -> Self {
    EdgeInsets::all(value)
  }
}

/// A solid border: a width (logical pixels) and a color. The corner radius is
/// *not* part of this — it describes the overall shape of the box and lives on
/// [`Style::border_radius`].
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub struct Border {
  pub width: f64,
  pub color: Color,
}

impl Border {
  #[inline(always)]
  pub const fn new(width: f64, color: Color) -> Border {
    Border { width, color }
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
  pub main_axis_alignment: Option<MainAxisAlignment>,
  pub cross_axis_alignment: Option<CrossAxisAlignment>,
  pub gap: Option<f64>,
  pub flex_grow: Option<f32>,
  pub flex_shrink: Option<f32>,
  pub flex_basis: Option<f64>,

  pub width: Option<f64>,
  pub height: Option<f64>,
  pub min_width: Option<f64>,
  pub min_height: Option<f64>,
  pub max_width: Option<f64>,
  pub max_height: Option<f64>,

  pub padding: Option<EdgeInsets>,

  pub background: Option<Color>,
  pub color: Option<Color>,
  pub font_size: Option<f64>,
  pub font_weight: Option<u16>,

  pub border: Option<Border>,
  pub border_radius: Option<f64>,

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
      main_axis_alignment,
      cross_axis_alignment,
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
      padding,
      background,
      color,
      font_size,
      font_weight,
      border,
      border_radius,
      cursor,
    );
    self
  }
}

// TODO: All containers should default to taking up the full width/height
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
  pub fn main_axis_alignment(mut self, v: MainAxisAlignment) -> Self {
    self.main_axis_alignment = Some(v);
    self
  }
  #[inline(always)]
  pub fn cross_axis_alignment(mut self, v: CrossAxisAlignment) -> Self {
    self.cross_axis_alignment = Some(v);
    self
  }
  #[inline(always)]
  pub fn gap(mut self, v: f64) -> Self {
    self.gap = Some(v);
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
  pub fn flex_basis(mut self, v: f64) -> Self {
    self.flex_basis = Some(v);
    self
  }

  #[inline(always)]
  pub fn width(mut self, v: f64) -> Self {
    self.width = Some(v);
    self
  }
  #[inline(always)]
  pub fn height(mut self, v: f64) -> Self {
    self.height = Some(v);
    self
  }
  #[inline(always)]
  pub fn min_width(mut self, v: f64) -> Self {
    self.min_width = Some(v);
    self
  }
  #[inline(always)]
  pub fn min_height(mut self, v: f64) -> Self {
    self.min_height = Some(v);
    self
  }
  #[inline(always)]
  pub fn max_width(mut self, v: f64) -> Self {
    self.max_width = Some(v);
    self
  }
  #[inline(always)]
  pub fn max_height(mut self, v: f64) -> Self {
    self.max_height = Some(v);
    self
  }

  #[inline(always)]
  pub fn padding(mut self, v: EdgeInsets) -> Self {
    self.padding = Some(v);
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
  pub fn font_size(mut self, v: f64) -> Self {
    self.font_size = Some(v);
    self
  }
  #[inline(always)]
  pub fn font_weight(mut self, v: u16) -> Self {
    self.font_weight = Some(v);
    self
  }

  #[inline(always)]
  pub fn border(mut self, v: Border) -> Self {
    self.border = Some(v);
    self
  }
  #[inline(always)]
  pub fn border_radius(mut self, v: f64) -> Self {
    self.border_radius = Some(v);
    self
  }

  #[inline(always)]
  pub fn cursor(mut self, v: Cursor) -> Self {
    self.cursor = Some(v);
    self
  }
}
