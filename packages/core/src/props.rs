//! The typed properties a driver realizes.
//!
//! The built-in views describe themselves with these types and hand them to the active
//! driver: the static properties batched at creation ([`BoxProps`], [`RunProps`]), and a
//! reactive property's new value one at a time ([`BoxProp`], [`RunProp`]). No platform
//! vocabulary appears here; each driver maps them to its own sink.

use crate::{
  AlignContent,
  AlignItems,
  AlignSelf,
  Animation,
  Border,
  Color,
  Condition,
  Dimension,
  FlexWrap,
  GridAutoFlow,
  GridPlacement,
  GridTrack,
  Inset,
  JustifyContent,
  JustifyItems,
  JustifySelf,
  Layout,
  Line,
  MouseCursor,
  Overflow,
  Placement,
  Position,
  Role,
  Transition,
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
  /// How the box is positioned; the inset offsets it.
  pub position: Option<Position>,
  /// The offset from each edge, for a positioned box.
  pub inset: Option<Inset<Dimension<f32>>>,
  /// The background color.
  pub color: Option<Color>,
  /// The border.
  pub border: Option<Border>,
  /// The corner radius, in logical pixels.
  pub border_radius: Option<f32>,
  /// The mouse cursor shown over the box.
  pub cursor: Option<MouseCursor>,
  /// The gap between rows of children, in logical pixels.
  pub row_gap: Option<f32>,
  /// The gap between columns of children, in logical pixels.
  pub column_gap: Option<f32>,
  /// Whether and how a flex container wraps its children.
  pub flex_wrap: Option<FlexWrap>,
  /// How much the box grows to fill its flex line's free space.
  pub flex_grow: Option<f32>,
  /// How much the box shrinks when its flex line overflows.
  pub flex_shrink: Option<f32>,
  /// The box's initial main-axis size as a flex item.
  pub flex_basis: Option<Dimension<f32>>,
  /// Distribution of children along the main (flex) or inline (grid) axis.
  pub justify_content: Option<JustifyContent>,
  /// Alignment of children on the cross (flex) or block (grid) axis.
  pub align_items: Option<AlignItems>,
  /// Distribution of wrapped lines or grid tracks along the cross or block axis.
  pub align_content: Option<AlignContent>,
  /// Alignment of grid children on the inline axis.
  pub justify_items: Option<JustifyItems>,
  /// The box's own cross- or block-axis alignment, overriding its parent's
  /// `align_items`.
  pub align_self: Option<AlignSelf>,
  /// The box's own inline-axis alignment in a grid, overriding its parent's
  /// `justify_items`.
  pub justify_self: Option<JustifySelf>,
  /// The grid's explicit column tracks.
  pub grid_template_columns: Option<Vec<GridTrack<f32>>>,
  /// The grid's explicit row tracks.
  pub grid_template_rows: Option<Vec<GridTrack<f32>>>,
  /// The sizes given to implicitly created columns, cycled.
  pub grid_auto_columns: Option<Vec<GridTrack<f32>>>,
  /// The sizes given to implicitly created rows, cycled.
  pub grid_auto_rows: Option<Vec<GridTrack<f32>>>,
  /// The direction auto-placed grid children fill.
  pub grid_auto_flow: Option<GridAutoFlow>,
  /// The box's row placement as a grid child.
  pub grid_row: Option<Line<GridPlacement>>,
  /// The box's column placement as a grid child.
  pub grid_column: Option<Line<GridPlacement>>,
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

  /// Sets how the box is positioned.
  #[inline(always)]
  pub const fn set_position(mut self, position: Position) -> Self {
    self.position = Some(position);

    self
  }

  /// Sets the offset from each edge, for a positioned box.
  #[inline(always)]
  pub const fn set_inset(mut self, inset: Inset<Dimension<f32>>) -> Self {
    self.inset = Some(inset);

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

  /// Sets the gap between children on both axes, in logical pixels.
  #[inline(always)]
  pub const fn set_gap(mut self, gap: f32) -> Self {
    self.row_gap = Some(gap);
    self.column_gap = Some(gap);

    self
  }

  /// Sets the gap between rows of children, in logical pixels.
  #[inline(always)]
  pub const fn set_row_gap(mut self, row_gap: f32) -> Self {
    self.row_gap = Some(row_gap);

    self
  }

  /// Sets the gap between columns of children, in logical pixels.
  #[inline(always)]
  pub const fn set_column_gap(mut self, column_gap: f32) -> Self {
    self.column_gap = Some(column_gap);

    self
  }

  /// Sets whether and how a flex container wraps its children.
  #[inline(always)]
  pub const fn set_flex_wrap(mut self, flex_wrap: FlexWrap) -> Self {
    self.flex_wrap = Some(flex_wrap);

    self
  }

  /// Sets how much the box grows to fill its flex line's free space.
  #[inline(always)]
  pub const fn set_flex_grow(mut self, flex_grow: f32) -> Self {
    self.flex_grow = Some(flex_grow);

    self
  }

  /// Sets how much the box shrinks when its flex line overflows.
  #[inline(always)]
  pub const fn set_flex_shrink(mut self, flex_shrink: f32) -> Self {
    self.flex_shrink = Some(flex_shrink);

    self
  }

  /// Sets the box's initial main-axis size as a flex item.
  #[inline(always)]
  pub fn set_flex_basis(mut self, flex_basis: impl Into<Dimension<f32>>) -> Self {
    self.flex_basis = Some(flex_basis.into());

    self
  }

  /// Sets the distribution of children along the main (flex) or inline (grid) axis.
  #[inline(always)]
  pub const fn set_justify_content(mut self, justify_content: JustifyContent) -> Self {
    self.justify_content = Some(justify_content);

    self
  }

  /// Sets the alignment of children on the cross (flex) or block (grid) axis.
  #[inline(always)]
  pub const fn set_align_items(mut self, align_items: AlignItems) -> Self {
    self.align_items = Some(align_items);

    self
  }

  /// Sets the distribution of wrapped lines or grid tracks along the cross or block
  /// axis.
  #[inline(always)]
  pub const fn set_align_content(mut self, align_content: AlignContent) -> Self {
    self.align_content = Some(align_content);

    self
  }

  /// Sets the alignment of grid children on the inline axis.
  #[inline(always)]
  pub const fn set_justify_items(mut self, justify_items: JustifyItems) -> Self {
    self.justify_items = Some(justify_items);

    self
  }

  /// Sets the box's own cross- or block-axis alignment.
  #[inline(always)]
  pub const fn set_align_self(mut self, align_self: AlignSelf) -> Self {
    self.align_self = Some(align_self);

    self
  }

  /// Sets the box's own inline-axis alignment in a grid.
  #[inline(always)]
  pub const fn set_justify_self(mut self, justify_self: JustifySelf) -> Self {
    self.justify_self = Some(justify_self);

    self
  }

  /// Sets the grid's explicit column tracks.
  #[inline(always)]
  pub fn set_grid_template_columns(
    mut self,
    tracks: impl IntoIterator<Item = GridTrack<f32>>,
  ) -> Self {
    self.grid_template_columns = Some(tracks.into_iter().collect());

    self
  }

  /// Sets the grid's explicit row tracks.
  #[inline(always)]
  pub fn set_grid_template_rows(
    mut self,
    tracks: impl IntoIterator<Item = GridTrack<f32>>,
  ) -> Self {
    self.grid_template_rows = Some(tracks.into_iter().collect());

    self
  }

  /// Sets the sizes given to implicitly created columns, cycled.
  #[inline(always)]
  pub fn set_grid_auto_columns(mut self, tracks: impl IntoIterator<Item = GridTrack<f32>>) -> Self {
    self.grid_auto_columns = Some(tracks.into_iter().collect());

    self
  }

  /// Sets the sizes given to implicitly created rows, cycled.
  #[inline(always)]
  pub fn set_grid_auto_rows(mut self, tracks: impl IntoIterator<Item = GridTrack<f32>>) -> Self {
    self.grid_auto_rows = Some(tracks.into_iter().collect());

    self
  }

  /// Sets the direction auto-placed grid children fill.
  #[inline(always)]
  pub const fn set_grid_auto_flow(mut self, grid_auto_flow: GridAutoFlow) -> Self {
    self.grid_auto_flow = Some(grid_auto_flow);

    self
  }

  /// Sets the box's row placement as a grid child.
  #[inline(always)]
  pub const fn set_grid_row(mut self, grid_row: Line<GridPlacement>) -> Self {
    self.grid_row = Some(grid_row);

    self
  }

  /// Sets the box's column placement as a grid child.
  #[inline(always)]
  pub const fn set_grid_column(mut self, grid_column: Line<GridPlacement>) -> Self {
    self.grid_column = Some(grid_column);

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
  /// How the box is positioned.
  Position(Position),
  /// The offset from each edge, for a positioned box.
  Inset(Inset<Dimension<f32>>),
  /// The background color.
  Color(Color),
  /// The border.
  Border(Border),
  /// The corner radius, in logical pixels.
  BorderRadius(f32),
  /// The mouse cursor shown over the box.
  Cursor(MouseCursor),
  /// The gap between rows of children, in logical pixels.
  RowGap(f32),
  /// The gap between columns of children, in logical pixels.
  ColumnGap(f32),
  /// Whether and how a flex container wraps its children.
  FlexWrap(FlexWrap),
  /// How much the box grows to fill its flex line's free space.
  FlexGrow(f32),
  /// How much the box shrinks when its flex line overflows.
  FlexShrink(f32),
  /// The box's initial main-axis size as a flex item.
  FlexBasis(Dimension<f32>),
  /// Distribution of children along the main (flex) or inline (grid) axis.
  JustifyContent(JustifyContent),
  /// Alignment of children on the cross (flex) or block (grid) axis.
  AlignItems(AlignItems),
  /// Distribution of wrapped lines or grid tracks along the cross or block axis.
  AlignContent(AlignContent),
  /// Alignment of grid children on the inline axis.
  JustifyItems(JustifyItems),
  /// The box's own cross- or block-axis alignment.
  AlignSelf(AlignSelf),
  /// The box's own inline-axis alignment in a grid.
  JustifySelf(JustifySelf),
  /// The grid's explicit column tracks.
  GridTemplateColumns(Vec<GridTrack<f32>>),
  /// The grid's explicit row tracks.
  GridTemplateRows(Vec<GridTrack<f32>>),
  /// The sizes given to implicitly created columns, cycled.
  GridAutoColumns(Vec<GridTrack<f32>>),
  /// The sizes given to implicitly created rows, cycled.
  GridAutoRows(Vec<GridTrack<f32>>),
  /// The direction auto-placed grid children fill.
  GridAutoFlow(GridAutoFlow),
  /// The box's row placement as a grid child.
  GridRow(Line<GridPlacement>),
  /// The box's column placement as a grid child.
  GridColumn(Line<GridPlacement>),
  /// How content exceeding the box's bounds is treated.
  Overflow(Overflow),
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

/// The realization surface every form control shares: styling plus focus placement.
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
#[must_use]
pub struct ButtonProps {
  /// The shared control surface.
  pub control: ControlProps,
}

/// Everything a driver needs to realize a single-line text input.
#[must_use]
pub struct InputProps {
  /// The hint shown while the value is empty.
  pub placeholder: Option<String>,
  /// The shared control surface.
  pub control: ControlProps,
}

/// Everything a driver needs to realize a checkbox.
#[must_use]
pub struct CheckboxProps {
  /// The shared control surface.
  pub control: ControlProps,
}

/// Everything a driver needs to realize one radio button.
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
#[must_use]
pub struct SelectOption {
  /// The value the choice stands for.
  pub value: String,
  /// The text shown for the choice.
  pub label: String,
}

/// Everything a driver needs to realize a select control.
#[must_use]
pub struct SelectProps {
  /// The choices, in order.
  pub options: Vec<SelectOption>,
  /// The shared control surface.
  pub control: ControlProps,
}

/// Everything a driver needs to realize a multi-line text area.
#[must_use]
pub struct TextareaProps {
  /// The hint shown while the value is empty.
  pub placeholder: Option<String>,
  /// The visible height, in text rows.
  pub rows: Option<u16>,
  /// The shared control surface.
  pub control: ControlProps,
}

/// Everything a driver needs to realize a slider.
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
