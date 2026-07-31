//! The typed properties a driver realizes.
//!
//! The built-in views describe themselves with these types and hand them to the active
//! driver: the static properties batched at creation ([`BoxProps`], [`RunProps`]), and a
//! reactive property's new value one at a time ([`BoxProp`], [`RunProp`]). No platform
//! vocabulary appears here; each driver maps them to its own sink.

use crate::{
  Border,
  Color,
  CrossAxisAlignment,
  Inset,
  Layout,
  MainAxisAlignment,
  MouseCursor,
  Role,
};

/// A box's static style: each property the builder fixed to a value.
#[derive(Clone, Default)]
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
}

/// A text run's static style: each property the builder or the inherited theme fixed to a
/// value.
#[derive(Clone, Copy, Default)]
#[must_use]
pub struct RunStyle {
  /// The text color.
  pub color: Option<Color>,
  /// The font size, in logical pixels.
  pub font_size: Option<f32>,
  /// The font weight.
  pub font_weight: Option<f32>,
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
}
