//! The typed properties a driver realizes.
//!
//! The built-in views describe themselves with these types and hand them to the active
//! driver: the static properties batched at creation ([`BoxProps`], [`RunProps`]), and a
//! reactive property's new value one at a time ([`BoxProp`], [`RunProp`]). No platform
//! vocabulary appears here; each driver maps them to its own sink.

use crate::{
  Animation,
  Border,
  Color,
  Condition,
  CrossAxisAlignment,
  Inset,
  Layout,
  MainAxisAlignment,
  MouseCursor,
  Role,
  Transition,
};

/// A box's static style: each property the builder fixed to a value.
///
/// Also the style a conditional variant or an animation keyframe carries, built with the
/// chainable setters.
#[derive(Clone, Copy, Default, PartialEq)]
#[must_use]
pub struct BoxStyle {
  /// The fixed width, in logical pixels.
  pub width: Option<f32>,
  /// The fixed height, in logical pixels.
  pub height: Option<f32>,
  /// The minimum width, in logical pixels.
  pub min_width: Option<f32>,
  /// The minimum height, in logical pixels.
  pub min_height: Option<f32>,
  /// The maximum width, in logical pixels.
  pub max_width: Option<f32>,
  /// The maximum height, in logical pixels.
  pub max_height: Option<f32>,
  /// The padding on each edge.
  pub padding: Option<Inset<f32>>,
  /// The background color.
  pub color: Option<Color>,
  /// The border.
  pub border: Option<Border>,
  /// The corner radius, in logical pixels.
  pub border_radius: Option<f32>,
  /// The mouse cursor shown over the box.
  pub cursor: Option<MouseCursor>,
  /// The gap between children, in logical pixels.
  pub spacing: Option<f32>,
  /// The children's alignment along the main axis.
  pub main_axis_alignment: Option<MainAxisAlignment>,
  /// The children's alignment along the cross axis.
  pub cross_axis_alignment: Option<CrossAxisAlignment>,
}

impl BoxStyle {
  /// Creates an empty style.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets the width, in logical pixels.
  #[inline(always)]
  pub const fn set_width(mut self, width: f32) -> Self {
    self.width = Some(width);

    self
  }

  /// Sets the height, in logical pixels.
  #[inline(always)]
  pub const fn set_height(mut self, height: f32) -> Self {
    self.height = Some(height);

    self
  }

  /// Sets the minimum width, in logical pixels.
  #[inline(always)]
  pub const fn set_min_width(mut self, min_width: f32) -> Self {
    self.min_width = Some(min_width);

    self
  }

  /// Sets the minimum height, in logical pixels.
  #[inline(always)]
  pub const fn set_min_height(mut self, min_height: f32) -> Self {
    self.min_height = Some(min_height);

    self
  }

  /// Sets the maximum width, in logical pixels.
  #[inline(always)]
  pub const fn set_max_width(mut self, max_width: f32) -> Self {
    self.max_width = Some(max_width);

    self
  }

  /// Sets the maximum height, in logical pixels.
  #[inline(always)]
  pub const fn set_max_height(mut self, max_height: f32) -> Self {
    self.max_height = Some(max_height);

    self
  }

  /// Sets the padding on each edge.
  #[inline(always)]
  pub const fn set_padding(mut self, padding: Inset<f32>) -> Self {
    self.padding = Some(padding);

    self
  }

  /// Sets the background color.
  #[inline(always)]
  pub const fn set_color(mut self, color: Color) -> Self {
    self.color = Some(color);

    self
  }

  /// Sets the border.
  #[inline(always)]
  pub const fn set_border(mut self, border: Border) -> Self {
    self.border = Some(border);

    self
  }

  /// Sets the corner radius, in logical pixels.
  #[inline(always)]
  pub const fn set_border_radius(mut self, border_radius: f32) -> Self {
    self.border_radius = Some(border_radius);

    self
  }

  /// Sets the mouse cursor shown over the box.
  #[inline(always)]
  pub const fn set_cursor(mut self, cursor: MouseCursor) -> Self {
    self.cursor = Some(cursor);

    self
  }

  /// Sets the gap between children, in logical pixels.
  #[inline(always)]
  pub const fn set_spacing(mut self, spacing: f32) -> Self {
    self.spacing = Some(spacing);

    self
  }

  /// Sets the children's alignment along the main axis.
  #[inline(always)]
  pub const fn set_main_axis_alignment(mut self, alignment: MainAxisAlignment) -> Self {
    self.main_axis_alignment = Some(alignment);

    self
  }

  /// Sets the children's alignment along the cross axis.
  #[inline(always)]
  pub const fn set_cross_axis_alignment(mut self, alignment: CrossAxisAlignment) -> Self {
    self.cross_axis_alignment = Some(alignment);

    self
  }
}

/// One box property's value, for a reactive update.
#[derive(Clone)]
#[must_use]
pub enum BoxProp {
  /// The width, in logical pixels.
  Width(f32),
  /// The height, in logical pixels.
  Height(f32),
  /// The minimum width, in logical pixels.
  MinWidth(f32),
  /// The minimum height, in logical pixels.
  MinHeight(f32),
  /// The maximum width, in logical pixels.
  MaxWidth(f32),
  /// The maximum height, in logical pixels.
  MaxHeight(f32),
  /// The padding on each edge.
  Padding(Inset<f32>),
  /// The background color.
  Color(Color),
  /// The border.
  Border(Border),
  /// The corner radius, in logical pixels.
  BorderRadius(f32),
  /// The mouse cursor shown over the box.
  Cursor(MouseCursor),
  /// The gap between children, in logical pixels.
  Spacing(f32),
  /// The children's alignment along the main axis.
  MainAxisAlignment(MainAxisAlignment),
  /// The children's alignment along the cross axis.
  CrossAxisAlignment(CrossAxisAlignment),
}

/// Everything a driver needs to realize a box.
#[must_use]
pub struct BoxProps {
  /// The layout mode.
  pub layout: Layout,
  /// The semantic role, if any.
  pub role: Option<Role>,
  /// The static style.
  pub style: BoxStyle,
  /// Conditional variants, each applying while its condition holds; later entries win
  /// over earlier ones.
  pub variants: Vec<(Condition, BoxStyle)>,
  /// The property transitions.
  pub transitions: Vec<Transition>,
  /// The keyframe animations.
  pub animations: Vec<Animation<BoxStyle>>,
}

/// Everything a driver needs to realize a single-line text input.
#[must_use]
pub struct InputProps {
  /// The hint shown while the value is empty.
  pub placeholder: Option<String>,
  /// The static style.
  pub style: BoxStyle,
  /// Conditional variants, each applying while its condition holds; later entries win
  /// over earlier ones.
  pub variants: Vec<(Condition, BoxStyle)>,
  /// The property transitions.
  pub transitions: Vec<Transition>,
  /// The keyframe animations.
  pub animations: Vec<Animation<BoxStyle>>,
}

/// A text run's static style: each property the builder or the inherited theme fixed to a
/// value.
///
/// Also the style a conditional variant or an animation keyframe carries, built with the
/// chainable setters.
#[derive(Clone, Copy, Default, PartialEq)]
#[must_use]
pub struct RunStyle {
  /// The text color.
  pub color: Option<Color>,
  /// The font size, in logical pixels.
  pub font_size: Option<f32>,
  /// The font weight.
  pub font_weight: Option<f32>,
}

impl RunStyle {
  /// Creates an empty style.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets the text color.
  #[inline(always)]
  pub const fn set_color(mut self, color: Color) -> Self {
    self.color = Some(color);

    self
  }

  /// Sets the font size, in logical pixels.
  #[inline(always)]
  pub const fn set_font_size(mut self, font_size: f32) -> Self {
    self.font_size = Some(font_size);

    self
  }

  /// Sets the font weight.
  #[inline(always)]
  pub const fn set_font_weight(mut self, font_weight: f32) -> Self {
    self.font_weight = Some(font_weight);

    self
  }
}

/// One text-run property's value, for a reactive update.
#[derive(Clone, Copy)]
#[must_use]
pub enum RunProp {
  /// The text color.
  Color(Color),
  /// The font size, in logical pixels.
  FontSize(f32),
  /// The font weight.
  FontWeight(f32),
}

/// How a text run participates in the surrounding layout.
#[derive(Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum RunDisplay {
  /// Its own line.
  Block,
  /// Flows with the surrounding text.
  Inline,
  /// An inline-level box laid out among the surrounding text.
  InlineBox,
}

/// Everything a driver needs to realize a text run.
#[must_use]
pub struct RunProps {
  /// How the run participates in the surrounding layout.
  pub display: RunDisplay,
  /// The link destination, when the run is a link.
  pub link: Option<String>,
  /// The static style.
  pub style: RunStyle,
  /// Conditional variants, each applying while its condition holds; later entries win
  /// over earlier ones.
  pub variants: Vec<(Condition, RunStyle)>,
  /// The property transitions.
  pub transitions: Vec<Transition>,
  /// The keyframe animations.
  pub animations: Vec<Animation<RunStyle>>,
}
