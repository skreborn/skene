//! The typed properties a driver realizes.
//!
//! The built-in views describe themselves with these types and hand them to the active
//! driver: the static properties batched at creation ([`BoxProps`], [`RunProps`]), and a
//! reactive property's new value one at a time ([`BoxProp`], [`RunProp`]). No platform
//! vocabulary appears here; each driver maps them to its own sink.

use crate::{
  Animation,
  Arrangement,
  Border,
  Color,
  Condition,
  Dimension,
  Flexibility,
  GridFlow,
  GridPlacement,
  GridTrack,
  Inset,
  ItemAlignment,
  Layout,
  Line,
  MouseCursor,
  Overflow,
  Placement,
  Role,
  Spacing,
  Transition,
  Wrap,
};

/// A box's static style: each property the builder fixed to a value.
///
/// Also the style a conditional variant or an animation keyframe carries, built with the
/// chainable setters.
#[derive(Clone, Default, PartialEq)]
#[must_use]
pub struct BoxStyle {
  /// The width.
  pub width: Option<Dimension<f32>>,
  /// The height.
  pub height: Option<Dimension<f32>>,
  /// The minimum width.
  pub min_width: Option<Dimension<f32>>,
  /// The minimum height.
  pub min_height: Option<Dimension<f32>>,
  /// The maximum width.
  pub max_width: Option<Dimension<f32>>,
  /// The maximum height.
  pub max_height: Option<Dimension<f32>>,
  /// The width-to-height ratio the unsized axis follows.
  pub aspect_ratio: Option<f32>,
  /// The margin on each edge.
  pub margin: Option<Inset<Dimension<f32>>>,
  /// The padding on each edge, in logical pixels.
  pub padding: Option<Inset<f32>>,
  /// The shift from each edge, keeping the box in flow.
  pub offset: Option<Inset<Dimension<f32>>>,
  /// The edges the box is pinned to, out of flow, against the nearest offset or
  /// pinned ancestor; wins when `offset` is also set.
  pub pinned: Option<Inset<Dimension<f32>>>,
  /// The background color.
  pub color: Option<Color>,
  /// The border.
  pub border: Option<Border>,
  /// The corner radius, in logical pixels.
  pub border_radius: Option<f32>,
  /// The mouse cursor shown over the box.
  pub cursor: Option<MouseCursor>,
  /// The spacing between children, per axis, in logical pixels.
  pub spacing: Option<Spacing<f32>>,
  /// Whether and how a flex container wraps its children.
  pub wrap: Option<Wrap>,
  /// The box's flexibility as a flex child: its initial size and its grow and
  /// shrink weights.
  pub flex: Option<Flexibility<f32>>,
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
  pub grid_columns: Option<Vec<GridTrack<f32>>>,
  /// The grid's explicit row tracks.
  pub grid_rows: Option<Vec<GridTrack<f32>>>,
  /// The sizes given to implicitly created columns, cycled.
  pub grid_implicit_columns: Option<Vec<GridTrack<f32>>>,
  /// The sizes given to implicitly created rows, cycled.
  pub grid_implicit_rows: Option<Vec<GridTrack<f32>>>,
  /// The direction auto-placed grid children fill.
  pub grid_flow: Option<GridFlow>,
  /// The box's row placement as a grid child.
  pub grid_row_placement: Option<Line<GridPlacement>>,
  /// The box's column placement as a grid child.
  pub grid_column_placement: Option<Line<GridPlacement>>,
  /// How content exceeding the box's bounds is treated.
  pub overflow: Option<Overflow>,
}

impl BoxStyle {
  /// Creates an empty style.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets the width: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_width(mut self, width: impl Into<Dimension<f32>>) -> Self {
    self.width = Some(width.into());

    self
  }

  /// Sets the height: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_height(mut self, height: impl Into<Dimension<f32>>) -> Self {
    self.height = Some(height.into());

    self
  }

  /// Sets the minimum width: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_min_width(mut self, min_width: impl Into<Dimension<f32>>) -> Self {
    self.min_width = Some(min_width.into());

    self
  }

  /// Sets the minimum height: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_min_height(mut self, min_height: impl Into<Dimension<f32>>) -> Self {
    self.min_height = Some(min_height.into());

    self
  }

  /// Sets the maximum width: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_max_width(mut self, max_width: impl Into<Dimension<f32>>) -> Self {
    self.max_width = Some(max_width.into());

    self
  }

  /// Sets the maximum height: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_max_height(mut self, max_height: impl Into<Dimension<f32>>) -> Self {
    self.max_height = Some(max_height.into());

    self
  }

  /// Sets the width-to-height ratio the unsized axis follows.
  #[inline(always)]
  pub const fn set_aspect_ratio(mut self, aspect_ratio: f32) -> Self {
    self.aspect_ratio = Some(aspect_ratio);

    self
  }

  /// Sets the margin on each edge.
  #[inline(always)]
  pub const fn set_margin(mut self, margin: Inset<Dimension<f32>>) -> Self {
    self.margin = Some(margin);

    self
  }

  /// Sets the padding on each edge.
  #[inline(always)]
  pub const fn set_padding(mut self, padding: Inset<f32>) -> Self {
    self.padding = Some(padding);

    self
  }

  /// Sets the shift from each edge, keeping the box in flow.
  #[inline(always)]
  pub const fn set_offset(mut self, offset: Inset<Dimension<f32>>) -> Self {
    self.offset = Some(offset);

    self
  }

  /// Pins the box to the given edges, out of flow, against the nearest offset or
  /// pinned ancestor.
  #[inline(always)]
  pub const fn set_pinned(mut self, pinned: Inset<Dimension<f32>>) -> Self {
    self.pinned = Some(pinned);

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

  /// Sets the spacing between children: one value for both axes, or a [`Spacing`]
  /// per axis.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: impl Into<Spacing<f32>>) -> Self {
    self.spacing = Some(spacing.into());

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
  pub const fn set_flex(mut self, flex: Flexibility<f32>) -> Self {
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

  /// Sets the distribution of wrapped lines or grid tracks along the cross or block
  /// axis.
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
  pub fn set_grid_columns(mut self, tracks: impl IntoIterator<Item = GridTrack<f32>>) -> Self {
    self.grid_columns = Some(tracks.into_iter().collect());

    self
  }

  /// Sets the grid's explicit row tracks.
  #[inline(always)]
  pub fn set_grid_rows(mut self, tracks: impl IntoIterator<Item = GridTrack<f32>>) -> Self {
    self.grid_rows = Some(tracks.into_iter().collect());

    self
  }

  /// Sets the sizes given to implicitly created columns, cycled.
  #[inline(always)]
  pub fn set_grid_implicit_columns(
    mut self,
    tracks: impl IntoIterator<Item = GridTrack<f32>>,
  ) -> Self {
    self.grid_implicit_columns = Some(tracks.into_iter().collect());

    self
  }

  /// Sets the sizes given to implicitly created rows, cycled.
  #[inline(always)]
  pub fn set_grid_implicit_rows(
    mut self,
    tracks: impl IntoIterator<Item = GridTrack<f32>>,
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

  /// Sets how content exceeding the box's bounds is treated.
  #[inline(always)]
  pub const fn set_overflow(mut self, overflow: Overflow) -> Self {
    self.overflow = Some(overflow);

    self
  }
}

/// One box property's value, for a reactive update.
#[derive(Clone)]
#[must_use]
pub enum BoxProp {
  /// The width.
  Width(Dimension<f32>),
  /// The height.
  Height(Dimension<f32>),
  /// The minimum width.
  MinWidth(Dimension<f32>),
  /// The minimum height.
  MinHeight(Dimension<f32>),
  /// The maximum width.
  MaxWidth(Dimension<f32>),
  /// The maximum height.
  MaxHeight(Dimension<f32>),
  /// The width-to-height ratio the unsized axis follows.
  AspectRatio(f32),
  /// The margin on each edge.
  Margin(Inset<Dimension<f32>>),
  /// The padding on each edge, in logical pixels.
  Padding(Inset<f32>),
  /// The shift from each edge, keeping the box in flow.
  Offset(Inset<Dimension<f32>>),
  /// The edges the box is pinned to, out of flow.
  Pinned(Inset<Dimension<f32>>),
  /// The background color.
  Color(Color),
  /// The border.
  Border(Border),
  /// The corner radius, in logical pixels.
  BorderRadius(f32),
  /// The mouse cursor shown over the box.
  Cursor(MouseCursor),
  /// The spacing between children, per axis, in logical pixels.
  Spacing(Spacing<f32>),
  /// Whether and how a flex container wraps its children.
  Wrap(Wrap),
  /// The box's flexibility as a flex child.
  Flex(Flexibility<f32>),
  /// Distribution of children along the main axis.
  Arrangement(Arrangement),
  /// Alignment of children on the cross axis.
  ItemAlignment(ItemAlignment),
  /// Distribution of wrapped lines—or grid rows—along the cross axis.
  LineArrangement(Arrangement),
  /// Alignment of grid children within their cells, on the main axis.
  ItemHorizontalAlignment(ItemAlignment),
  /// The box's own cross-axis alignment, overriding its parent's.
  SelfAlignment(ItemAlignment),
  /// The box's own main-axis alignment in a grid, overriding its parent's.
  SelfHorizontalAlignment(ItemAlignment),
  /// The grid's explicit column tracks.
  GridColumns(Vec<GridTrack<f32>>),
  /// The grid's explicit row tracks.
  GridRows(Vec<GridTrack<f32>>),
  /// The sizes given to implicitly created columns, cycled.
  GridImplicitColumns(Vec<GridTrack<f32>>),
  /// The sizes given to implicitly created rows, cycled.
  GridImplicitRows(Vec<GridTrack<f32>>),
  /// The direction auto-placed grid children fill.
  GridFlow(GridFlow),
  /// The box's row placement as a grid child.
  GridRowPlacement(Line<GridPlacement>),
  /// The box's column placement as a grid child.
  GridColumnPlacement(Line<GridPlacement>),
  /// How content exceeding the box's bounds is treated.
  Overflow(Overflow),
}

/// Everything a driver needs to realize a box.
#[derive(Clone)]
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

/// The realization surface every form control shares: styling plus focus placement.
#[derive(Clone)]
#[must_use]
pub struct ControlProps {
  /// The static style.
  pub style: BoxStyle,
  /// Conditional variants, each applying while its condition holds; later entries win
  /// over earlier ones.
  pub variants: Vec<(Condition, BoxStyle)>,
  /// The property transitions.
  pub transitions: Vec<Transition>,
  /// The keyframe animations.
  pub animations: Vec<Animation<BoxStyle>>,
  /// The position in the keyboard tab order: `Some(0)` joins it in document order,
  /// negative removes the element from it while keeping it programmatically focusable.
  pub tab_index: Option<i32>,
  /// Whether the element takes focus when the page loads.
  pub autofocus: bool,
}

/// Everything a driver needs to realize an overlay: content above the app, opened and
/// closed by the facade.
#[derive(Clone)]
#[must_use]
pub struct OverlayProps {
  /// Whether the overlay is modal: it traps focus and blocks interaction beneath it.
  pub modal: bool,
  /// Where the overlay sits.
  pub placement: Placement,
  /// The backdrop color painted over the app while open, if any.
  pub backdrop: Option<Color>,
  /// The content box's static style.
  pub style: BoxStyle,
  /// Conditional variants, each applying while its condition holds; later entries win
  /// over earlier ones.
  pub variants: Vec<(Condition, BoxStyle)>,
}

/// Everything a driver needs to realize a button.
#[derive(Clone)]
#[must_use]
pub struct ButtonProps {
  /// The shared control surface.
  pub control: ControlProps,
}

/// Everything a driver needs to realize a single-line text input.
#[derive(Clone)]
#[must_use]
pub struct TextFieldProps {
  /// The hint shown while the value is empty.
  pub placeholder: Option<String>,
  /// The shared control surface.
  pub control: ControlProps,
}

/// Everything a driver needs to realize a checkbox.
#[derive(Clone)]
#[must_use]
pub struct CheckboxProps {
  /// The shared control surface.
  pub control: ControlProps,
}

/// Everything a driver needs to realize one radio button.
#[derive(Clone)]
#[must_use]
pub struct RadioProps {
  /// The group the radio belongs to; the platform keeps one member checked and moves
  /// between members with the arrow keys.
  pub group: String,
  /// The value this member stands for.
  pub value: String,
  /// The shared control surface.
  pub control: ControlProps,
}

/// One choice of a select control.
#[derive(Clone)]
#[must_use]
pub struct PickerOption {
  /// The value the choice stands for.
  pub value: String,
  /// The text shown for the choice.
  pub label: String,
}

/// Everything a driver needs to realize a select control.
#[derive(Clone)]
#[must_use]
pub struct PickerProps {
  /// The choices, in order.
  pub options: Vec<PickerOption>,
  /// The shared control surface.
  pub control: ControlProps,
}

/// Everything a driver needs to realize a multi-line text area.
#[derive(Clone)]
#[must_use]
pub struct TextEditorProps {
  /// The hint shown while the value is empty.
  pub placeholder: Option<String>,
  /// The visible height, in text lines.
  pub lines: Option<u16>,
  /// The shared control surface.
  pub control: ControlProps,
}

/// Everything a driver needs to realize a slider.
#[derive(Clone)]
#[must_use]
pub struct SliderProps {
  /// The smallest selectable value.
  pub min: f32,
  /// The largest selectable value.
  pub max: f32,
  /// The step between selectable values; a platform default when absent.
  pub step: Option<f32>,
  /// The shared control surface.
  pub control: ControlProps,
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
#[derive(Clone)]
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
