//! The terminal's own typed style vocabulary: cells, colors, bold—and nothing a
//! terminal cannot render.
//!
//! Lengths are `u32` cells. There is no font size and no transition: what the medium
//! lacks, the types lack, so a terminal app is first-class at compile time rather than
//! best-effort at run time. Mouse hover, pointer shapes and one-cell borders are in: a
//! compatible terminal renders them, and an incompatible one degrades quietly.

use skene_core::{
  Arrangement,
  Color,
  Composable,
  ConditionShape,
  Dimension,
  Flexibility,
  GridFlow,
  GridPlacement,
  GridTrack,
  Inset,
  ItemAlignment,
  Line,
  MouseCursor,
  Overflow,
  Spacing,
  Wrap,
};

/// A box's static style: each property the builder fixed to a value.
///
/// Also the style a conditional variant carries, built with the chainable setters.
#[derive(Clone, Default, PartialEq)]
#[must_use]
pub struct BoxStyle {
  /// The width: cells, a percentage, or auto.
  pub width: Option<Dimension<u32>>,
  /// The height: cells, a percentage, or auto.
  pub height: Option<Dimension<u32>>,
  /// The minimum width: cells, a percentage, or auto.
  pub min_width: Option<Dimension<u32>>,
  /// The minimum height: cells, a percentage, or auto.
  pub min_height: Option<Dimension<u32>>,
  /// The maximum width: cells, a percentage, or auto.
  pub max_width: Option<Dimension<u32>>,
  /// The maximum height: cells, a percentage, or auto.
  pub max_height: Option<Dimension<u32>>,
  /// The width-to-height ratio the unsized axis follows, in cells.
  pub aspect_ratio: Option<f32>,
  /// The margin on each edge.
  pub margin: Option<Inset<Dimension<u32>>>,
  /// The padding on each edge, in cells.
  pub padding: Option<Inset<u32>>,
  /// The shift from each edge, keeping the box in flow.
  pub offset: Option<Inset<Dimension<u32>>>,
  /// The edges the box is pinned to, out of flow, against the nearest offset or
  /// pinned ancestor; wins when `offset` is also set.
  pub pinned: Option<Inset<Dimension<u32>>>,
  /// The background color.
  pub color: Option<Color>,
  /// The border color; a border is always one cell of box-drawing characters.
  pub border: Option<Color>,
  /// The spacing between children, per axis, in cells.
  pub spacing: Option<Spacing<u32>>,
  /// The pointer shape shown over the box, in terminals that honor it (OSC 22).
  pub cursor: Option<MouseCursor>,
  /// How content overflowing the box behaves; `Scroll` and `Auto` make it scrollable.
  pub overflow: Option<Overflow>,
  /// The shadow color, cast one cell down and right and mixed into the colors beneath.
  pub shadow: Option<Color>,
  /// Whether and how a flex container wraps its children.
  pub wrap: Option<Wrap>,
  /// The box's flexibility as a flex child: its initial size and its grow and
  /// shrink weights.
  pub flex: Option<Flexibility<u32>>,
  /// Distribution of children along the main (flex) or inline (grid) axis.
  pub arrangement: Option<Arrangement>,
  /// Alignment of children on the cross (flex) or block (grid) axis.
  pub item_alignment: Option<ItemAlignment>,
  /// Distribution of wrapped lines or grid tracks along the cross or block axis.
  pub line_arrangement: Option<Arrangement>,
  /// Alignment of grid children on the inline axis.
  pub item_horizontal_alignment: Option<ItemAlignment>,
  /// The box's own cross- or block-axis alignment, overriding its parent's
  /// `item_alignment`.
  pub self_alignment: Option<ItemAlignment>,
  /// The box's own inline-axis alignment in a grid, overriding its parent's
  /// `item_horizontal_alignment`.
  pub self_horizontal_alignment: Option<ItemAlignment>,
  /// The grid's explicit column tracks.
  pub grid_columns: Option<Vec<GridTrack<u32>>>,
  /// The grid's explicit row tracks.
  pub grid_rows: Option<Vec<GridTrack<u32>>>,
  /// The sizes given to implicitly created columns, cycled.
  pub grid_implicit_columns: Option<Vec<GridTrack<u32>>>,
  /// The sizes given to implicitly created rows, cycled.
  pub grid_implicit_rows: Option<Vec<GridTrack<u32>>>,
  /// The direction auto-placed grid children fill.
  pub grid_flow: Option<GridFlow>,
  /// The box's row placement as a grid child.
  pub grid_row_placement: Option<Line<GridPlacement>>,
  /// The box's column placement as a grid child.
  pub grid_column_placement: Option<Line<GridPlacement>>,
}

impl BoxStyle {
  /// Creates an empty style.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets the width: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_width(mut self, width: impl Into<Dimension<u32>>) -> Self {
    self.width = Some(width.into());

    self
  }

  /// Sets the height: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_height(mut self, height: impl Into<Dimension<u32>>) -> Self {
    self.height = Some(height.into());

    self
  }

  /// Sets the minimum width: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_min_width(mut self, min_width: impl Into<Dimension<u32>>) -> Self {
    self.min_width = Some(min_width.into());

    self
  }

  /// Sets the minimum height: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_min_height(mut self, min_height: impl Into<Dimension<u32>>) -> Self {
    self.min_height = Some(min_height.into());

    self
  }

  /// Sets the maximum width: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_max_width(mut self, max_width: impl Into<Dimension<u32>>) -> Self {
    self.max_width = Some(max_width.into());

    self
  }

  /// Sets the maximum height: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_max_height(mut self, max_height: impl Into<Dimension<u32>>) -> Self {
    self.max_height = Some(max_height.into());

    self
  }

  /// Sets the width-to-height ratio the unsized axis follows, in cells.
  #[inline(always)]
  pub const fn set_aspect_ratio(mut self, aspect_ratio: f32) -> Self {
    self.aspect_ratio = Some(aspect_ratio);

    self
  }

  /// Sets the margin on each edge.
  #[inline(always)]
  pub const fn set_margin(mut self, margin: Inset<Dimension<u32>>) -> Self {
    self.margin = Some(margin);

    self
  }

  /// Sets the padding on each edge, in cells.
  #[inline(always)]
  pub const fn set_padding(mut self, padding: Inset<u32>) -> Self {
    self.padding = Some(padding);

    self
  }

  /// Sets the shift from each edge, keeping the box in flow.
  #[inline(always)]
  pub const fn set_offset(mut self, offset: Inset<Dimension<u32>>) -> Self {
    self.offset = Some(offset);

    self
  }

  /// Pins the box to the given edges, out of flow, against the nearest offset or
  /// pinned ancestor.
  #[inline(always)]
  pub const fn set_pinned(mut self, pinned: Inset<Dimension<u32>>) -> Self {
    self.pinned = Some(pinned);

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
  pub const fn set_border(mut self, border: Color) -> Self {
    self.border = Some(border);

    self
  }

  /// Sets the spacing between children: one value for both axes, or a [`Spacing`]
  /// per axis.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: impl Into<Spacing<u32>>) -> Self {
    self.spacing = Some(spacing.into());

    self
  }

  /// Sets the pointer shape shown over the box, in terminals that honor it.
  #[inline(always)]
  pub const fn set_cursor(mut self, cursor: MouseCursor) -> Self {
    self.cursor = Some(cursor);

    self
  }

  /// Sets how content overflowing the box behaves.
  #[inline(always)]
  pub const fn set_overflow(mut self, overflow: Overflow) -> Self {
    self.overflow = Some(overflow);

    self
  }

  /// Sets the shadow color, cast one cell down and right; its alpha is the strength of the mix into
  /// the colors beneath.
  #[inline(always)]
  pub const fn set_shadow(mut self, shadow: Color) -> Self {
    self.shadow = Some(shadow);

    self
  }

  /// Sets whether and how a flex container wraps its children.
  #[inline(always)]
  pub const fn set_wrap(mut self, wrap: Wrap) -> Self {
    self.wrap = Some(wrap);

    self
  }

  /// Sets the box's flexibility as a flex child.
  #[inline(always)]
  pub const fn set_flex(mut self, flex: Flexibility<u32>) -> Self {
    self.flex = Some(flex);

    self
  }

  /// Sets the distribution of children along the main (flex) or inline (grid) axis.
  #[inline(always)]
  pub const fn set_arrangement(mut self, arrangement: Arrangement) -> Self {
    self.arrangement = Some(arrangement);

    self
  }

  /// Sets the alignment of children on the cross (flex) or block (grid) axis.
  #[inline(always)]
  pub const fn set_item_alignment(mut self, item_alignment: ItemAlignment) -> Self {
    self.item_alignment = Some(item_alignment);

    self
  }

  /// Sets the distribution of wrapped lines or grid tracks along the cross or block axis.
  #[inline(always)]
  pub const fn set_line_arrangement(mut self, line_arrangement: Arrangement) -> Self {
    self.line_arrangement = Some(line_arrangement);

    self
  }

  /// Sets the alignment of grid children on the inline axis.
  #[inline(always)]
  pub const fn set_item_horizontal_alignment(
    mut self,
    item_horizontal_alignment: ItemAlignment,
  ) -> Self {
    self.item_horizontal_alignment = Some(item_horizontal_alignment);

    self
  }

  /// Sets the box's own cross- or block-axis alignment.
  #[inline(always)]
  pub const fn set_self_alignment(mut self, self_alignment: ItemAlignment) -> Self {
    self.self_alignment = Some(self_alignment);

    self
  }

  /// Sets the box's own inline-axis alignment in a grid.
  #[inline(always)]
  pub const fn set_self_horizontal_alignment(
    mut self,
    self_horizontal_alignment: ItemAlignment,
  ) -> Self {
    self.self_horizontal_alignment = Some(self_horizontal_alignment);

    self
  }

  /// Sets the grid's explicit column tracks.
  #[inline(always)]
  pub fn set_grid_columns(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.grid_columns = Some(tracks.into_iter().collect());

    self
  }

  /// Sets the grid's explicit row tracks.
  #[inline(always)]
  pub fn set_grid_rows(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.grid_rows = Some(tracks.into_iter().collect());

    self
  }

  /// Sets the sizes given to implicitly created columns, cycled.
  #[inline(always)]
  pub fn set_grid_implicit_columns(
    mut self,
    tracks: impl IntoIterator<Item = GridTrack<u32>>,
  ) -> Self {
    self.grid_implicit_columns = Some(tracks.into_iter().collect());

    self
  }

  /// Sets the sizes given to implicitly created rows, cycled.
  #[inline(always)]
  pub fn set_grid_implicit_rows(
    mut self,
    tracks: impl IntoIterator<Item = GridTrack<u32>>,
  ) -> Self {
    self.grid_implicit_rows = Some(tracks.into_iter().collect());

    self
  }

  /// Sets the direction auto-placed grid children fill.
  #[inline(always)]
  pub const fn set_grid_flow(mut self, grid_flow: GridFlow) -> Self {
    self.grid_flow = Some(grid_flow);

    self
  }

  /// Sets the box's row placement as a grid child.
  #[inline(always)]
  pub const fn set_grid_row_placement(mut self, grid_row_placement: Line<GridPlacement>) -> Self {
    self.grid_row_placement = Some(grid_row_placement);

    self
  }

  /// Sets the box's column placement as a grid child.
  #[inline(always)]
  pub const fn set_grid_column_placement(
    mut self,
    grid_column_placement: Line<GridPlacement>,
  ) -> Self {
    self.grid_column_placement = Some(grid_column_placement);

    self
  }
}

/// A small control's style: the glyph-level properties a control that owns no box
/// honors.
///
/// Everything box-shaped—sizing, padding, borders, backgrounds, shadows—belongs to a
/// wrapping [`Container`](crate::Container); a control is a glyph and styles like one.
#[derive(Clone, Copy, Default, PartialEq)]
#[must_use]
pub struct ControlStyle {
  /// The marks' color.
  pub color: Option<Color>,
  /// Whether the marks are bold.
  pub bold: Option<bool>,
  /// The pointer shape shown over the control, in terminals that honor it (OSC 22).
  pub cursor: Option<MouseCursor>,
}

impl ControlStyle {
  /// Creates an empty style.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets the marks' color.
  #[inline(always)]
  pub const fn set_color(mut self, color: Color) -> Self {
    self.color = Some(color);

    self
  }

  /// Sets whether the marks are bold.
  #[inline(always)]
  pub const fn set_bold(mut self, bold: bool) -> Self {
    self.bold = Some(bold);

    self
  }

  /// Sets the pointer shape shown over the control, in terminals that honor it.
  #[inline(always)]
  pub const fn set_cursor(mut self, cursor: MouseCursor) -> Self {
    self.cursor = Some(cursor);

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

/// A condition under which a style variant applies: the terminal's size, the node's
/// focus, or the mouse hovering it—composable to any depth with [`and`](Self::and) and
/// [`or`](Self::or).
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub enum Condition {
  /// The terminal is at least this wide, in cells.
  MinWidth(u32),
  /// The terminal is at most this wide, in cells.
  MaxWidth(u32),
  /// The terminal is at least this tall, in cells.
  MinHeight(u32),
  /// The terminal is at most this tall, in cells.
  MaxHeight(u32),
  /// The node has keyboard focus.
  Focused,
  /// The mouse is over the node, in terminals that report it.
  Hovered,
  /// The terminal background is dark; an undetectable background counts as dark.
  DarkMode,
  /// The terminal background is light.
  LightMode,
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
}

/// One box property's value, for a reactive update.
#[derive(Clone)]
pub(crate) enum BoxProp {
  Width(Dimension<u32>),
  Height(Dimension<u32>),
  MinWidth(Dimension<u32>),
  MinHeight(Dimension<u32>),
  MaxWidth(Dimension<u32>),
  MaxHeight(Dimension<u32>),
  AspectRatio(f32),
  Margin(Inset<Dimension<u32>>),
  Padding(Inset<u32>),
  Offset(Inset<Dimension<u32>>),
  Pinned(Inset<Dimension<u32>>),
  Color(Color),
  Border(Color),
  Spacing(Spacing<u32>),
  Cursor(MouseCursor),
  Overflow(Overflow),
  Shadow(Color),
  Wrap(Wrap),
  Flex(Flexibility<u32>),
  Arrangement(Arrangement),
  ItemAlignment(ItemAlignment),
  LineArrangement(Arrangement),
  ItemHorizontalAlignment(ItemAlignment),
  SelfAlignment(ItemAlignment),
  SelfHorizontalAlignment(ItemAlignment),
  GridColumns(Vec<GridTrack<u32>>),
  GridRows(Vec<GridTrack<u32>>),
  GridImplicitColumns(Vec<GridTrack<u32>>),
  GridImplicitRows(Vec<GridTrack<u32>>),
  GridFlow(GridFlow),
  GridRowPlacement(Line<GridPlacement>),
  GridColumnPlacement(Line<GridPlacement>),
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
    BoxProp::AspectRatio(value) => style.aspect_ratio = Some(value),
    BoxProp::Margin(value) => style.margin = Some(value),
    BoxProp::Padding(value) => style.padding = Some(value),
    BoxProp::Offset(value) => style.offset = Some(value),
    BoxProp::Pinned(value) => style.pinned = Some(value),
    BoxProp::Color(value) => style.color = Some(value),
    BoxProp::Border(value) => style.border = Some(value),
    BoxProp::Spacing(value) => style.spacing = Some(value),
    BoxProp::Cursor(value) => style.cursor = Some(value),
    BoxProp::Overflow(value) => style.overflow = Some(value),
    BoxProp::Shadow(value) => style.shadow = Some(value),
    BoxProp::Wrap(value) => style.wrap = Some(value),
    BoxProp::Flex(value) => style.flex = Some(value),
    BoxProp::Arrangement(value) => style.arrangement = Some(value),
    BoxProp::ItemAlignment(value) => style.item_alignment = Some(value),
    BoxProp::LineArrangement(value) => style.line_arrangement = Some(value),
    BoxProp::ItemHorizontalAlignment(value) => style.item_horizontal_alignment = Some(value),
    BoxProp::SelfAlignment(value) => style.self_alignment = Some(value),
    BoxProp::SelfHorizontalAlignment(value) => style.self_horizontal_alignment = Some(value),
    BoxProp::GridColumns(value) => style.grid_columns = Some(value),
    BoxProp::GridRows(value) => style.grid_rows = Some(value),
    BoxProp::GridImplicitColumns(value) => style.grid_implicit_columns = Some(value),
    BoxProp::GridImplicitRows(value) => style.grid_implicit_rows = Some(value),
    BoxProp::GridFlow(value) => style.grid_flow = Some(value),
    BoxProp::GridRowPlacement(value) => style.grid_row_placement = Some(value),
    BoxProp::GridColumnPlacement(value) => style.grid_column_placement = Some(value),
  }
}

/// One text-run property's value, for a reactive update.
#[derive(Clone, Copy)]
pub(crate) enum RunProp {
  Color(Color),
  Bold(bool),
}
