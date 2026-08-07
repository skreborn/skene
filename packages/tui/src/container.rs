//! A box that decorates itself and lays out its children, in cells.

use std::rc::Rc;

use skene_core::{
  AlignContent,
  AlignItems,
  AlignSelf,
  Color,
  Dimension,
  FlexDirection,
  FlexWrap,
  GridAutoFlow,
  GridPlacement,
  GridTrack,
  JustifyContent,
  JustifyItems,
  JustifySelf,
  KeyEvent,
  Layout,
  Line,
  MouseCursor,
  NodeId,
  Overflow,
  Position,
  Prevent,
  Source,
  View,
};

use crate::{
  Children,
  Inset,
  events::{PointerEvent, ScrollEvent},
  props::{BoxProp, BoxStyle, Condition},
  realize::{Binders, split_prop},
  state::NodeKind,
  style::box_taffy_style,
  with_state,
};

/// A box that decorates itself and lays out its children.
///
/// Constructed in a layout mode: [`new`](Container::new) (block normal flow),
/// [`row`](Container::row) / [`column`](Container::column) (flex),
/// [`grid`](Container::grid), or [`stack`](Container::stack) (children overlapping in
/// one cell). Children are set with `set_children` or `derive_children`; decoration,
/// sizing, and layout are set with the chainable `set_`/`derive_` methods, in cells.
#[must_use]
pub struct Container {
  children: Children,
  layout: Layout,
  width: Option<Source<Dimension<u32>>>,
  height: Option<Source<Dimension<u32>>>,
  min_width: Option<Source<Dimension<u32>>>,
  min_height: Option<Source<Dimension<u32>>>,
  max_width: Option<Source<Dimension<u32>>>,
  max_height: Option<Source<Dimension<u32>>>,
  aspect_ratio: Option<Source<f32>>,
  margin: Option<Source<skene_core::Inset<Dimension<u32>>>>,
  padding: Option<Source<Inset>>,
  position: Option<Source<Position>>,
  inset: Option<Source<skene_core::Inset<Dimension<u32>>>>,
  color: Option<Source<Color>>,
  border: Option<Source<Color>>,
  row_gap: Option<Source<u32>>,
  column_gap: Option<Source<u32>>,
  cursor: Option<Source<MouseCursor>>,
  overflow: Option<Source<Overflow>>,
  shadow: Option<Source<Color>>,
  flex_wrap: Option<Source<FlexWrap>>,
  flex_grow: Option<Source<f32>>,
  flex_shrink: Option<Source<f32>>,
  flex_basis: Option<Source<Dimension<u32>>>,
  justify_content: Option<Source<JustifyContent>>,
  align_items: Option<Source<AlignItems>>,
  align_content: Option<Source<AlignContent>>,
  justify_items: Option<Source<JustifyItems>>,
  align_self: Option<Source<AlignSelf>>,
  justify_self: Option<Source<JustifySelf>>,
  grid_template_columns: Option<Source<Vec<GridTrack<u32>>>>,
  grid_template_rows: Option<Source<Vec<GridTrack<u32>>>>,
  grid_auto_columns: Option<Source<Vec<GridTrack<u32>>>>,
  grid_auto_rows: Option<Source<Vec<GridTrack<u32>>>>,
  grid_auto_flow: Option<Source<GridAutoFlow>>,
  grid_row: Option<Source<Line<GridPlacement>>>,
  grid_column: Option<Source<Line<GridPlacement>>>,
  variants: Vec<(Condition, BoxStyle)>,
  click_handler: Option<Rc<skene_core::ClickHandler>>,
  pointer_enter_handler: Option<Rc<dyn Fn()>>,
  pointer_leave_handler: Option<Rc<dyn Fn()>>,
  focus_handler: Option<Rc<dyn Fn()>>,
  blur_handler: Option<Rc<dyn Fn()>>,
  key_down_handler: Option<Rc<skene_core::KeyHandler>>,
  pointer_down_handler: Option<Rc<crate::events::PointerHandler>>,
  pointer_up_handler: Option<Rc<crate::events::PointerHandler>>,
  pointer_move_handler: Option<Rc<crate::events::PointerHandler>>,
  #[expect(clippy::type_complexity)]
  scroll_handler: Option<Rc<dyn Fn(&ScrollEvent)>>,
  anchor: Option<crate::Anchor>,
}

impl Container {
  #[inline(always)]
  fn mode(layout: Layout) -> Self {
    Self {
      children: Children::default(),
      layout,
      width: None,
      height: None,
      min_width: None,
      min_height: None,
      max_width: None,
      max_height: None,
      aspect_ratio: None,
      margin: None,
      padding: None,
      position: None,
      inset: None,
      color: None,
      border: None,
      row_gap: None,
      column_gap: None,
      cursor: None,
      overflow: None,
      shadow: None,
      flex_wrap: None,
      flex_grow: None,
      flex_shrink: None,
      flex_basis: None,
      justify_content: None,
      align_items: None,
      align_content: None,
      justify_items: None,
      align_self: None,
      justify_self: None,
      grid_template_columns: None,
      grid_template_rows: None,
      grid_auto_columns: None,
      grid_auto_rows: None,
      grid_auto_flow: None,
      grid_row: None,
      grid_column: None,
      variants: Vec::new(),
      click_handler: None,
      pointer_enter_handler: None,
      pointer_leave_handler: None,
      focus_handler: None,
      blur_handler: None,
      key_down_handler: None,
      pointer_down_handler: None,
      pointer_up_handler: None,
      pointer_move_handler: None,
      scroll_handler: None,
      anchor: None,
    }
  }

  /// A block container: children flow in normal document order, contained within it.
  #[inline(always)]
  pub fn new() -> Self {
    Container::mode(Layout::Block)
  }

  /// A flex container laying children out left to right.
  #[inline(always)]
  pub fn row() -> Self {
    Container::flex(FlexDirection::Row)
  }

  /// A flex container laying children out top to bottom.
  #[inline(always)]
  pub fn column() -> Self {
    Container::flex(FlexDirection::Column)
  }

  /// A flex container laying children out along `direction`.
  #[inline(always)]
  pub fn flex(direction: FlexDirection) -> Self {
    Container::mode(Layout::Flex { direction })
  }

  /// A grid container with `columns` equal-width columns; set
  /// `grid_template_columns` (and friends) for any other track layout.
  #[inline(always)]
  pub fn grid(columns: u16) -> Self {
    Container::mode(Layout::Grid).set_grid_template_columns(GridTrack::Fr(1.).repeated(columns))
  }

  /// A container overlapping its children in one cell, stacked in order (last on top).
  #[inline(always)]
  pub fn stack() -> Self {
    Container::mode(Layout::Stack)
  }

  /// Sets a static width: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_width(mut self, width: impl Into<Dimension<u32>>) -> Self {
    self.width = Some(Source::Static(width.into()));

    self
  }

  /// Sets a derived width.
  #[inline(always)]
  pub fn derive_width<D: Into<Dimension<u32>>>(mut self, width: impl Fn() -> D + 'static) -> Self {
    self.width = Some(Source::Derived(Rc::new(move || width().into())));

    self
  }

  /// Sets a static height: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_height(mut self, height: impl Into<Dimension<u32>>) -> Self {
    self.height = Some(Source::Static(height.into()));

    self
  }

  /// Sets a derived height.
  #[inline(always)]
  pub fn derive_height<D: Into<Dimension<u32>>>(
    mut self,
    height: impl Fn() -> D + 'static,
  ) -> Self {
    self.height = Some(Source::Derived(Rc::new(move || height().into())));

    self
  }

  /// Sets a static minimum width: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_min_width(mut self, min_width: impl Into<Dimension<u32>>) -> Self {
    self.min_width = Some(Source::Static(min_width.into()));

    self
  }

  /// Sets a derived minimum width.
  #[inline(always)]
  pub fn derive_min_width<D: Into<Dimension<u32>>>(
    mut self,
    min_width: impl Fn() -> D + 'static,
  ) -> Self {
    self.min_width = Some(Source::Derived(Rc::new(move || min_width().into())));

    self
  }

  /// Sets a static minimum height: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_min_height(mut self, min_height: impl Into<Dimension<u32>>) -> Self {
    self.min_height = Some(Source::Static(min_height.into()));

    self
  }

  /// Sets a derived minimum height.
  #[inline(always)]
  pub fn derive_min_height<D: Into<Dimension<u32>>>(
    mut self,
    min_height: impl Fn() -> D + 'static,
  ) -> Self {
    self.min_height = Some(Source::Derived(Rc::new(move || min_height().into())));

    self
  }

  /// Sets a static maximum width: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_max_width(mut self, max_width: impl Into<Dimension<u32>>) -> Self {
    self.max_width = Some(Source::Static(max_width.into()));

    self
  }

  /// Sets a derived maximum width.
  #[inline(always)]
  pub fn derive_max_width<D: Into<Dimension<u32>>>(
    mut self,
    max_width: impl Fn() -> D + 'static,
  ) -> Self {
    self.max_width = Some(Source::Derived(Rc::new(move || max_width().into())));

    self
  }

  /// Sets a static maximum height: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_max_height(mut self, max_height: impl Into<Dimension<u32>>) -> Self {
    self.max_height = Some(Source::Static(max_height.into()));

    self
  }

  /// Sets a derived maximum height.
  #[inline(always)]
  pub fn derive_max_height<D: Into<Dimension<u32>>>(
    mut self,
    max_height: impl Fn() -> D + 'static,
  ) -> Self {
    self.max_height = Some(Source::Derived(Rc::new(move || max_height().into())));

    self
  }

  /// Sets a static padding, in cells.
  #[inline(always)]
  pub fn set_padding(mut self, padding: Inset) -> Self {
    self.padding = Some(Source::Static(padding));

    self
  }

  /// Sets a derived padding, in cells.
  #[inline(always)]
  pub fn derive_padding(mut self, padding: impl Fn() -> Inset + 'static) -> Self {
    self.padding = Some(Source::Derived(Rc::new(padding)));

    self
  }

  /// Sets a static background color.
  #[inline(always)]
  pub fn set_color(mut self, color: Color) -> Self {
    self.color = Some(Source::Static(color));

    self
  }

  /// Sets a derived background color.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.color = Some(Source::Derived(Rc::new(color)));

    self
  }

  /// Sets a static one-cell border in the given color.
  #[inline(always)]
  pub fn set_border(mut self, color: Color) -> Self {
    self.border = Some(Source::Static(color));

    self
  }

  /// Sets a derived border color.
  #[inline(always)]
  pub fn derive_border(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.border = Some(Source::Derived(Rc::new(color)));

    self
  }

  /// Sets a static gap between children on both axes, in cells.
  #[inline(always)]
  pub fn set_gap(mut self, gap: u32) -> Self {
    self.row_gap = Some(Source::Static(gap));
    self.column_gap = Some(Source::Static(gap));

    self
  }

  /// Sets a derived gap between children on both axes, in cells.
  #[inline(always)]
  pub fn derive_gap(mut self, gap: impl Fn() -> u32 + 'static) -> Self {
    let gap: Rc<dyn Fn() -> u32> = Rc::new(gap);

    self.row_gap = Some(Source::Derived(Rc::clone(&gap)));
    self.column_gap = Some(Source::Derived(gap));

    self
  }

  /// Sets a static gap between rows of children, in cells.
  #[inline(always)]
  pub fn set_row_gap(mut self, row_gap: u32) -> Self {
    self.row_gap = Some(Source::Static(row_gap));

    self
  }

  /// Sets a derived gap between rows of children, in cells.
  #[inline(always)]
  pub fn derive_row_gap(mut self, row_gap: impl Fn() -> u32 + 'static) -> Self {
    self.row_gap = Some(Source::Derived(Rc::new(row_gap)));

    self
  }

  /// Sets a static gap between columns of children, in cells.
  #[inline(always)]
  pub fn set_column_gap(mut self, column_gap: u32) -> Self {
    self.column_gap = Some(Source::Static(column_gap));

    self
  }

  /// Sets a derived gap between columns of children, in cells.
  #[inline(always)]
  pub fn derive_column_gap(mut self, column_gap: impl Fn() -> u32 + 'static) -> Self {
    self.column_gap = Some(Source::Derived(Rc::new(column_gap)));

    self
  }

  /// Sets a static pointer shape, shown over the box in terminals that honor it.
  #[inline(always)]
  pub fn set_cursor(mut self, cursor: MouseCursor) -> Self {
    self.cursor = Some(Source::Static(cursor));

    self
  }

  /// Sets a derived pointer shape.
  #[inline(always)]
  pub fn derive_cursor(mut self, cursor: impl Fn() -> MouseCursor + 'static) -> Self {
    self.cursor = Some(Source::Derived(Rc::new(cursor)));

    self
  }

  /// Sets how content overflowing the box behaves; `Scroll` and `Auto` make it
  /// scrollable by mouse wheel and `PageUp`/`PageDown`.
  #[inline(always)]
  pub fn set_overflow(mut self, overflow: Overflow) -> Self {
    self.overflow = Some(Source::Static(overflow));

    self
  }

  /// Sets a derived overflow behavior.
  #[inline(always)]
  pub fn derive_overflow(mut self, overflow: impl Fn() -> Overflow + 'static) -> Self {
    self.overflow = Some(Source::Derived(Rc::new(overflow)));

    self
  }

  /// Sets the shadow color, cast one cell down and right; its alpha is the strength of
  /// the mix into the colors beneath.
  #[inline(always)]
  pub fn set_shadow(mut self, color: Color) -> Self {
    self.shadow = Some(Source::Static(color));

    self
  }

  /// Sets a derived shadow color.
  #[inline(always)]
  pub fn derive_shadow(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.shadow = Some(Source::Derived(Rc::new(color)));

    self
  }

  /// Sets a static width-to-height ratio the unsized axis follows, in cells.
  #[inline(always)]
  pub fn set_aspect_ratio(mut self, aspect_ratio: f32) -> Self {
    self.aspect_ratio = Some(Source::Static(aspect_ratio));

    self
  }

  /// Sets a derived width-to-height ratio the unsized axis follows, in cells.
  #[inline(always)]
  pub fn derive_aspect_ratio(mut self, aspect_ratio: impl Fn() -> f32 + 'static) -> Self {
    self.aspect_ratio = Some(Source::Derived(Rc::new(aspect_ratio)));

    self
  }

  /// Sets a static margin on each edge.
  #[inline(always)]
  pub fn set_margin(mut self, margin: skene_core::Inset<Dimension<u32>>) -> Self {
    self.margin = Some(Source::Static(margin));

    self
  }

  /// Sets a derived margin on each edge.
  #[inline(always)]
  pub fn derive_margin(
    mut self,
    margin: impl Fn() -> skene_core::Inset<Dimension<u32>> + 'static,
  ) -> Self {
    self.margin = Some(Source::Derived(Rc::new(margin)));

    self
  }

  /// Sets a static positioning mode; the inset offsets the box.
  #[inline(always)]
  pub fn set_position(mut self, position: Position) -> Self {
    self.position = Some(Source::Static(position));

    self
  }

  /// Sets a derived positioning mode; the inset offsets the box.
  #[inline(always)]
  pub fn derive_position(mut self, position: impl Fn() -> Position + 'static) -> Self {
    self.position = Some(Source::Derived(Rc::new(position)));

    self
  }

  /// Sets a static offset from each edge, for a positioned box.
  #[inline(always)]
  pub fn set_inset(mut self, inset: skene_core::Inset<Dimension<u32>>) -> Self {
    self.inset = Some(Source::Static(inset));

    self
  }

  /// Sets a derived offset from each edge, for a positioned box.
  #[inline(always)]
  pub fn derive_inset(
    mut self,
    inset: impl Fn() -> skene_core::Inset<Dimension<u32>> + 'static,
  ) -> Self {
    self.inset = Some(Source::Derived(Rc::new(inset)));

    self
  }

  /// Sets a static flex wrapping mode.
  #[inline(always)]
  pub fn set_flex_wrap(mut self, flex_wrap: FlexWrap) -> Self {
    self.flex_wrap = Some(Source::Static(flex_wrap));

    self
  }

  /// Sets a derived flex wrapping mode.
  #[inline(always)]
  pub fn derive_flex_wrap(mut self, flex_wrap: impl Fn() -> FlexWrap + 'static) -> Self {
    self.flex_wrap = Some(Source::Derived(Rc::new(flex_wrap)));

    self
  }

  /// Sets a static share of the flex line's free space the box grows by.
  #[inline(always)]
  pub fn set_flex_grow(mut self, flex_grow: f32) -> Self {
    self.flex_grow = Some(Source::Static(flex_grow));

    self
  }

  /// Sets a derived share of the flex line's free space the box grows by.
  #[inline(always)]
  pub fn derive_flex_grow(mut self, flex_grow: impl Fn() -> f32 + 'static) -> Self {
    self.flex_grow = Some(Source::Derived(Rc::new(flex_grow)));

    self
  }

  /// Sets a static rate the box shrinks by when its flex line overflows.
  #[inline(always)]
  pub fn set_flex_shrink(mut self, flex_shrink: f32) -> Self {
    self.flex_shrink = Some(Source::Static(flex_shrink));

    self
  }

  /// Sets a derived rate the box shrinks by when its flex line overflows.
  #[inline(always)]
  pub fn derive_flex_shrink(mut self, flex_shrink: impl Fn() -> f32 + 'static) -> Self {
    self.flex_shrink = Some(Source::Derived(Rc::new(flex_shrink)));

    self
  }

  /// Sets a static initial main-axis size as a flex item.
  #[inline(always)]
  pub fn set_flex_basis(mut self, flex_basis: impl Into<Dimension<u32>>) -> Self {
    self.flex_basis = Some(Source::Static(flex_basis.into()));

    self
  }

  /// Sets a derived initial main-axis size as a flex item.
  #[inline(always)]
  pub fn derive_flex_basis<D: Into<Dimension<u32>>>(
    mut self,
    flex_basis: impl Fn() -> D + 'static,
  ) -> Self {
    self.flex_basis = Some(Source::Derived(Rc::new(move || flex_basis().into())));

    self
  }

  /// Sets a static distribution of children along the main (flex) or inline (grid) axis.
  #[inline(always)]
  pub fn set_justify_content(mut self, justify_content: JustifyContent) -> Self {
    self.justify_content = Some(Source::Static(justify_content));

    self
  }

  /// Sets a derived distribution of children along the main (flex) or inline (grid) axis.
  #[inline(always)]
  pub fn derive_justify_content(
    mut self,
    justify_content: impl Fn() -> JustifyContent + 'static,
  ) -> Self {
    self.justify_content = Some(Source::Derived(Rc::new(justify_content)));

    self
  }

  /// Sets a static alignment of children on the cross (flex) or block (grid) axis.
  #[inline(always)]
  pub fn set_align_items(mut self, align_items: AlignItems) -> Self {
    self.align_items = Some(Source::Static(align_items));

    self
  }

  /// Sets a derived alignment of children on the cross (flex) or block (grid) axis.
  #[inline(always)]
  pub fn derive_align_items(mut self, align_items: impl Fn() -> AlignItems + 'static) -> Self {
    self.align_items = Some(Source::Derived(Rc::new(align_items)));

    self
  }

  /// Sets a static distribution of wrapped lines or grid tracks along the cross or block axis.
  #[inline(always)]
  pub fn set_align_content(mut self, align_content: AlignContent) -> Self {
    self.align_content = Some(Source::Static(align_content));

    self
  }

  /// Sets a derived distribution of wrapped lines or grid tracks along the cross or block axis.
  #[inline(always)]
  pub fn derive_align_content(
    mut self,
    align_content: impl Fn() -> AlignContent + 'static,
  ) -> Self {
    self.align_content = Some(Source::Derived(Rc::new(align_content)));

    self
  }

  /// Sets a static alignment of grid children on the inline axis.
  #[inline(always)]
  pub fn set_justify_items(mut self, justify_items: JustifyItems) -> Self {
    self.justify_items = Some(Source::Static(justify_items));

    self
  }

  /// Sets a derived alignment of grid children on the inline axis.
  #[inline(always)]
  pub fn derive_justify_items(
    mut self,
    justify_items: impl Fn() -> JustifyItems + 'static,
  ) -> Self {
    self.justify_items = Some(Source::Derived(Rc::new(justify_items)));

    self
  }

  /// Sets a static cross- or block-axis alignment of the box itself.
  #[inline(always)]
  pub fn set_align_self(mut self, align_self: AlignSelf) -> Self {
    self.align_self = Some(Source::Static(align_self));

    self
  }

  /// Sets a derived cross- or block-axis alignment of the box itself.
  #[inline(always)]
  pub fn derive_align_self(mut self, align_self: impl Fn() -> AlignSelf + 'static) -> Self {
    self.align_self = Some(Source::Derived(Rc::new(align_self)));

    self
  }

  /// Sets a static inline-axis alignment of the box itself in a grid.
  #[inline(always)]
  pub fn set_justify_self(mut self, justify_self: JustifySelf) -> Self {
    self.justify_self = Some(Source::Static(justify_self));

    self
  }

  /// Sets a derived inline-axis alignment of the box itself in a grid.
  #[inline(always)]
  pub fn derive_justify_self(mut self, justify_self: impl Fn() -> JustifySelf + 'static) -> Self {
    self.justify_self = Some(Source::Derived(Rc::new(justify_self)));

    self
  }

  /// Sets static explicit grid column tracks.
  #[inline(always)]
  pub fn set_grid_template_columns(
    mut self,
    tracks: impl IntoIterator<Item = GridTrack<u32>>,
  ) -> Self {
    self.grid_template_columns = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived explicit grid column tracks.
  #[inline(always)]
  pub fn derive_grid_template_columns(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static,
  ) -> Self {
    self.grid_template_columns = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets static explicit grid row tracks.
  #[inline(always)]
  pub fn set_grid_template_rows(
    mut self,
    tracks: impl IntoIterator<Item = GridTrack<u32>>,
  ) -> Self {
    self.grid_template_rows = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived explicit grid row tracks.
  #[inline(always)]
  pub fn derive_grid_template_rows(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static,
  ) -> Self {
    self.grid_template_rows = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets static sizes for implicitly created grid columns, cycled.
  #[inline(always)]
  pub fn set_grid_auto_columns(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.grid_auto_columns = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived sizes for implicitly created grid columns, cycled.
  #[inline(always)]
  pub fn derive_grid_auto_columns(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static,
  ) -> Self {
    self.grid_auto_columns = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets static sizes for implicitly created grid rows, cycled.
  #[inline(always)]
  pub fn set_grid_auto_rows(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.grid_auto_rows = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived sizes for implicitly created grid rows, cycled.
  #[inline(always)]
  pub fn derive_grid_auto_rows(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static,
  ) -> Self {
    self.grid_auto_rows = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets a static direction auto-placed grid children fill.
  #[inline(always)]
  pub fn set_grid_auto_flow(mut self, grid_auto_flow: GridAutoFlow) -> Self {
    self.grid_auto_flow = Some(Source::Static(grid_auto_flow));

    self
  }

  /// Sets a derived direction auto-placed grid children fill.
  #[inline(always)]
  pub fn derive_grid_auto_flow(
    mut self,
    grid_auto_flow: impl Fn() -> GridAutoFlow + 'static,
  ) -> Self {
    self.grid_auto_flow = Some(Source::Derived(Rc::new(grid_auto_flow)));

    self
  }

  /// Sets a static row placement of the box as a grid child.
  #[inline(always)]
  pub fn set_grid_row(mut self, grid_row: Line<GridPlacement>) -> Self {
    self.grid_row = Some(Source::Static(grid_row));

    self
  }

  /// Sets a derived row placement of the box as a grid child.
  #[inline(always)]
  pub fn derive_grid_row(mut self, grid_row: impl Fn() -> Line<GridPlacement> + 'static) -> Self {
    self.grid_row = Some(Source::Derived(Rc::new(grid_row)));

    self
  }

  /// Sets a static column placement of the box as a grid child.
  #[inline(always)]
  pub fn set_grid_column(mut self, grid_column: Line<GridPlacement>) -> Self {
    self.grid_column = Some(Source::Static(grid_column));

    self
  }

  /// Sets a derived column placement of the box as a grid child.
  #[inline(always)]
  pub fn derive_grid_column(
    mut self,
    grid_column: impl Fn() -> Line<GridPlacement> + 'static,
  ) -> Self {
    self.grid_column = Some(Source::Derived(Rc::new(grid_column)));

    self
  }

  /// Adds a conditional style variant, applied while `condition` holds.
  #[inline(always)]
  pub fn set_when(mut self, condition: Condition, style: BoxStyle) -> Self {
    self.variants.push((condition, style));

    self
  }

  /// Sets the click handler (`Enter` while focused, or a mouse click); its
  /// [`Prevent`] verdict is honored per the dispatch contract.
  #[inline(always)]
  pub fn handle_click<R: Into<Prevent>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self.click_handler = Some(Rc::new(move || action().into()));

    self
  }

  /// Sets the pointer-enter handler, run when the mouse moves onto the box.
  #[inline(always)]
  pub fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self.pointer_enter_handler = Some(Rc::new(action));

    self
  }

  /// Sets the pointer-leave handler, run when the mouse moves off the box.
  #[inline(always)]
  pub fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self.pointer_leave_handler = Some(Rc::new(action));

    self
  }

  /// Sets the focus handler.
  #[inline(always)]
  pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.focus_handler = Some(Rc::new(action));

    self
  }

  /// Sets the blur handler.
  #[inline(always)]
  pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.blur_handler = Some(Rc::new(action));

    self
  }

  /// Sets the key-press handler, run for keys pressed while the box or a descendant is
  /// focused; its [`Prevent`] verdict is honored per the dispatch contract.
  #[inline(always)]
  pub fn handle_key_down<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&KeyEvent) -> R + 'static,
  ) -> Self {
    self.key_down_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-down handler, run for presses on the box or within it; its
  /// [`Prevent`] verdict is honored per the dispatch contract.
  #[inline(always)]
  pub fn handle_pointer_down<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.pointer_down_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-up handler, run for releases on the box or within it; its
  /// [`Prevent`] verdict is honored per the dispatch contract.
  #[inline(always)]
  pub fn handle_pointer_up<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.pointer_up_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-move handler, run for moves over the box or within it; its
  /// [`Prevent`] verdict is honored per the dispatch contract.
  #[inline(always)]
  pub fn handle_pointer_move<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.pointer_move_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the scroll handler, run with the new offsets when the box scrolls.
  ///
  /// The box scrolls only with an overflow that allows it; see
  /// [`set_overflow`](Self::set_overflow).
  #[inline(always)]
  pub fn handle_scroll(mut self, action: impl Fn(&ScrollEvent) + 'static) -> Self {
    self.scroll_handler = Some(Rc::new(action));

    self
  }

  /// Binds `anchor` to the box, so an overlay's
  /// [`set_anchor`](crate::Overlay::set_anchor) positions against it once built.
  #[inline(always)]
  pub fn bind_anchor(mut self, anchor: &crate::Anchor) -> Self {
    self.anchor = Some(anchor.clone());

    self
  }
}

impl Default for Container {
  #[inline(always)]
  fn default() -> Self {
    Container::new()
  }
}

impl skene_core::HasChildren<crate::TuiOps> for Container {
  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl View for Container {
  // One `split_prop` call per property; length is the property count, not complexity.
  #[expect(clippy::too_many_lines)]
  fn build(self) -> NodeId {
    let mut style = BoxStyle::new();
    let mut binders = Binders::new();

    split_prop(
      self.width,
      &mut style.width,
      &mut binders,
      BoxProp::Width,
      update_box,
    );

    split_prop(
      self.height,
      &mut style.height,
      &mut binders,
      BoxProp::Height,
      update_box,
    );

    split_prop(
      self.min_width,
      &mut style.min_width,
      &mut binders,
      BoxProp::MinWidth,
      update_box,
    );

    split_prop(
      self.min_height,
      &mut style.min_height,
      &mut binders,
      BoxProp::MinHeight,
      update_box,
    );

    split_prop(
      self.max_width,
      &mut style.max_width,
      &mut binders,
      BoxProp::MaxWidth,
      update_box,
    );

    split_prop(
      self.max_height,
      &mut style.max_height,
      &mut binders,
      BoxProp::MaxHeight,
      update_box,
    );

    split_prop(
      self.aspect_ratio,
      &mut style.aspect_ratio,
      &mut binders,
      BoxProp::AspectRatio,
      update_box,
    );

    split_prop(
      self.margin,
      &mut style.margin,
      &mut binders,
      BoxProp::Margin,
      update_box,
    );

    split_prop(
      self.padding,
      &mut style.padding,
      &mut binders,
      BoxProp::Padding,
      update_box,
    );

    split_prop(
      self.position,
      &mut style.position,
      &mut binders,
      BoxProp::Position,
      update_box,
    );

    split_prop(
      self.inset,
      &mut style.inset,
      &mut binders,
      BoxProp::Inset,
      update_box,
    );

    split_prop(
      self.color,
      &mut style.color,
      &mut binders,
      BoxProp::Color,
      update_box,
    );

    split_prop(
      self.border,
      &mut style.border,
      &mut binders,
      BoxProp::Border,
      update_box,
    );

    split_prop(
      self.row_gap,
      &mut style.row_gap,
      &mut binders,
      BoxProp::RowGap,
      update_box,
    );

    split_prop(
      self.column_gap,
      &mut style.column_gap,
      &mut binders,
      BoxProp::ColumnGap,
      update_box,
    );

    split_prop(
      self.cursor,
      &mut style.cursor,
      &mut binders,
      BoxProp::Cursor,
      update_box,
    );

    split_prop(
      self.overflow,
      &mut style.overflow,
      &mut binders,
      BoxProp::Overflow,
      update_box,
    );

    split_prop(
      self.shadow,
      &mut style.shadow,
      &mut binders,
      BoxProp::Shadow,
      update_box,
    );

    split_prop(
      self.flex_wrap,
      &mut style.flex_wrap,
      &mut binders,
      BoxProp::FlexWrap,
      update_box,
    );

    split_prop(
      self.flex_grow,
      &mut style.flex_grow,
      &mut binders,
      BoxProp::FlexGrow,
      update_box,
    );

    split_prop(
      self.flex_shrink,
      &mut style.flex_shrink,
      &mut binders,
      BoxProp::FlexShrink,
      update_box,
    );

    split_prop(
      self.flex_basis,
      &mut style.flex_basis,
      &mut binders,
      BoxProp::FlexBasis,
      update_box,
    );

    split_prop(
      self.justify_content,
      &mut style.justify_content,
      &mut binders,
      BoxProp::JustifyContent,
      update_box,
    );

    split_prop(
      self.align_items,
      &mut style.align_items,
      &mut binders,
      BoxProp::AlignItems,
      update_box,
    );

    split_prop(
      self.align_content,
      &mut style.align_content,
      &mut binders,
      BoxProp::AlignContent,
      update_box,
    );

    split_prop(
      self.justify_items,
      &mut style.justify_items,
      &mut binders,
      BoxProp::JustifyItems,
      update_box,
    );

    split_prop(
      self.align_self,
      &mut style.align_self,
      &mut binders,
      BoxProp::AlignSelf,
      update_box,
    );

    split_prop(
      self.justify_self,
      &mut style.justify_self,
      &mut binders,
      BoxProp::JustifySelf,
      update_box,
    );

    split_prop(
      self.grid_template_columns,
      &mut style.grid_template_columns,
      &mut binders,
      BoxProp::GridTemplateColumns,
      update_box,
    );

    split_prop(
      self.grid_template_rows,
      &mut style.grid_template_rows,
      &mut binders,
      BoxProp::GridTemplateRows,
      update_box,
    );

    split_prop(
      self.grid_auto_columns,
      &mut style.grid_auto_columns,
      &mut binders,
      BoxProp::GridAutoColumns,
      update_box,
    );

    split_prop(
      self.grid_auto_rows,
      &mut style.grid_auto_rows,
      &mut binders,
      BoxProp::GridAutoRows,
      update_box,
    );

    split_prop(
      self.grid_auto_flow,
      &mut style.grid_auto_flow,
      &mut binders,
      BoxProp::GridAutoFlow,
      update_box,
    );

    split_prop(
      self.grid_row,
      &mut style.grid_row,
      &mut binders,
      BoxProp::GridRow,
      update_box,
    );

    split_prop(
      self.grid_column,
      &mut style.grid_column,
      &mut binders,
      BoxProp::GridColumn,
      update_box,
    );

    let taffy = box_taffy_style(self.layout, &style, false);
    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Box {
          layout: self.layout,
          style,
          variants: self.variants,
        },
        taffy,
      )
    });

    for binder in binders {
      binder(node);
    }

    with_state(|state| {
      if let Some(action) = self.click_handler {
        state
          .click
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.pointer_enter_handler {
        state
          .pointer_enter
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.pointer_leave_handler {
        state
          .pointer_leave
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.focus_handler {
        state
          .focus
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.blur_handler {
        state
          .blur
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.key_down_handler {
        state
          .key_down
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.pointer_down_handler {
        state
          .pointer_down
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.pointer_up_handler {
        state
          .pointer_up
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.pointer_move_handler {
        state
          .pointer_move
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.scroll_handler {
        state
          .scroll
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }
    });

    if let Some(anchor) = &self.anchor {
      anchor.assign(node);
    }

    match self.layout {
      // Overlap the children in one grid cell.
      Layout::Stack => self.children.mount_with(node, |child| {
        with_state(|state| state.with_node(child, |record| record.in_stack = true));

        child
      }),
      _ => self.children.mount(node),
    }

    node
  }
}

/// Applies one changed box property.
pub(crate) fn update_box(node: NodeId, prop: BoxProp) {
  with_state(|state| {
    state.with_node(node, |record| {
      if let NodeKind::Box { style, .. } | NodeKind::Input { style, .. } = &mut record.kind {
        crate::props::apply_box_prop(style, prop);
      }
    });

    state.dirty.set(true);
  });
}
