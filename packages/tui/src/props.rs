//! The terminal's own typed style vocabulary: cells, colors, bold—and nothing a
//! terminal cannot render.
//!
//! Lengths are `u16` cells. There is no radius, no cursor, no font size, and no
//! transition: what the medium lacks, the types lack, so a terminal app is first-class
//! at compile time rather than best-effort at run time.

use skene_core::{Color, CrossAxisAlignment, Inset, MainAxisAlignment};

/// A box's static style: each property the builder fixed to a value.
///
/// Also the style a conditional variant carries, built with the chainable setters.
#[derive(Clone, Copy, Default, PartialEq)]
#[must_use]
pub struct BoxStyle {
  /// The fixed width, in cells.
  pub width: Option<u16>,
  /// The fixed height, in cells.
  pub height: Option<u16>,
  /// The minimum width, in cells.
  pub min_width: Option<u16>,
  /// The minimum height, in cells.
  pub min_height: Option<u16>,
  /// The maximum width, in cells.
  pub max_width: Option<u16>,
  /// The maximum height, in cells.
  pub max_height: Option<u16>,
  /// The padding on each edge, in cells.
  pub padding: Option<Inset<u16>>,
  /// The background color.
  pub color: Option<Color>,
  /// The border color; a border is always one cell of box-drawing characters.
  pub border: Option<Color>,
  /// The gap between children, in cells.
  pub spacing: Option<u16>,
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

  /// Sets the width, in cells.
  #[inline(always)]
  pub const fn set_width(mut self, width: u16) -> Self {
    self.width = Some(width);

    self
  }

  /// Sets the height, in cells.
  #[inline(always)]
  pub const fn set_height(mut self, height: u16) -> Self {
    self.height = Some(height);

    self
  }

  /// Sets the minimum width, in cells.
  #[inline(always)]
  pub const fn set_min_width(mut self, min_width: u16) -> Self {
    self.min_width = Some(min_width);

    self
  }

  /// Sets the minimum height, in cells.
  #[inline(always)]
  pub const fn set_min_height(mut self, min_height: u16) -> Self {
    self.min_height = Some(min_height);

    self
  }

  /// Sets the maximum width, in cells.
  #[inline(always)]
  pub const fn set_max_width(mut self, max_width: u16) -> Self {
    self.max_width = Some(max_width);

    self
  }

  /// Sets the maximum height, in cells.
  #[inline(always)]
  pub const fn set_max_height(mut self, max_height: u16) -> Self {
    self.max_height = Some(max_height);

    self
  }

  /// Sets the padding on each edge, in cells.
  #[inline(always)]
  pub const fn set_padding(mut self, padding: Inset<u16>) -> Self {
    self.padding = Some(padding);

    self
  }

  /// Sets the background color.
  #[inline(always)]
  pub const fn set_color(mut self, color: Color) -> Self {
    self.color = Some(color);

    self
  }

  /// Sets a one-cell border in the given color.
  #[inline(always)]
  pub const fn set_border(mut self, color: Color) -> Self {
    self.border = Some(color);

    self
  }

  /// Sets the gap between children, in cells.
  #[inline(always)]
  pub const fn set_spacing(mut self, spacing: u16) -> Self {
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

/// A text run's static style.
///
/// Also the style a conditional variant carries, built with the chainable setters.
#[derive(Clone, Copy, Default, PartialEq)]
#[must_use]
pub struct RunStyle {
  /// The text color.
  pub color: Option<Color>,
  /// Whether the text is bold.
  pub bold: Option<bool>,
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

  /// Sets whether the text is bold.
  #[inline(always)]
  pub const fn set_bold(mut self, bold: bool) -> Self {
    self.bold = Some(bold);

    self
  }
}

/// A condition under which a style variant applies: the terminal's size, or the node's
/// focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Condition {
  /// The terminal is at least this wide, in cells.
  MinWidth(u16),
  /// The terminal is at most this wide, in cells.
  MaxWidth(u16),
  /// The terminal is at least this tall, in cells.
  MinHeight(u16),
  /// The terminal is at most this tall, in cells.
  MaxHeight(u16),
  /// The node has keyboard focus.
  Focused,
}

/// One box property's value, for a reactive update.
#[derive(Clone, Copy)]
pub(crate) enum BoxProp {
  Width(u16),
  Height(u16),
  MinWidth(u16),
  MinHeight(u16),
  MaxWidth(u16),
  MaxHeight(u16),
  Padding(Inset<u16>),
  Color(Color),
  Border(Color),
  Spacing(u16),
  MainAxisAlignment(MainAxisAlignment),
  CrossAxisAlignment(CrossAxisAlignment),
}

/// One text-run property's value, for a reactive update.
#[derive(Clone, Copy)]
pub(crate) enum RunProp {
  Color(Color),
  Bold(bool),
}

/// Folds one changed property into a stored box style.
pub(crate) fn apply_box_prop(style: &mut BoxStyle, prop: BoxProp) {
  match prop {
    BoxProp::Width(value) => style.width = Some(value),
    BoxProp::Height(value) => style.height = Some(value),
    BoxProp::MinWidth(value) => style.min_width = Some(value),
    BoxProp::MinHeight(value) => style.min_height = Some(value),
    BoxProp::MaxWidth(value) => style.max_width = Some(value),
    BoxProp::MaxHeight(value) => style.max_height = Some(value),
    BoxProp::Padding(value) => style.padding = Some(value),
    BoxProp::Color(value) => style.color = Some(value),
    BoxProp::Border(value) => style.border = Some(value),
    BoxProp::Spacing(value) => style.spacing = Some(value),
    BoxProp::MainAxisAlignment(value) => style.main_axis_alignment = Some(value),
    BoxProp::CrossAxisAlignment(value) => style.cross_axis_alignment = Some(value),
  }
}
