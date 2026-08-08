//! A box that decorates itself and lays out its children, in cells.

use std::rc::Rc;

use skene_core::{
  Arrangement,
  Color,
  Dimension,
  FlexDirection,
  Flexibility,
  GridFlow,
  GridPlacement,
  GridTrack,
  ItemAlignment,
  KeyEvent,
  Layout,
  Line,
  LineArrangement,
  MouseCursor,
  NodeId,
  Overflow,
  Prevent,
  Source,
  Spacing,
  View,
  Wrap,
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
  offset: Option<Source<skene_core::Inset<Dimension<u32>>>>,
  pinned: Option<Source<skene_core::Inset<Dimension<u32>>>>,
  color: Option<Source<Color>>,
  border: Option<Source<Color>>,
  spacing: Option<Source<Spacing<u32>>>,
  cursor: Option<Source<MouseCursor>>,
  overflow: Option<Source<Overflow>>,
  shadow: Option<Source<Color>>,
  wrap: Option<Source<Wrap>>,
  flex: Option<Source<Flexibility<u32>>>,
  main_axis_arrangement: Option<Source<Arrangement>>,
  cross_axis_alignment: Option<Source<ItemAlignment>>,
  line_arrangement: Option<Source<LineArrangement>>,
  main_axis_item_alignment: Option<Source<ItemAlignment>>,
  self_cross_axis_alignment: Option<Source<ItemAlignment>>,
  self_main_axis_alignment: Option<Source<ItemAlignment>>,
  grid_columns: Option<Source<Vec<GridTrack<u32>>>>,
  grid_rows: Option<Source<Vec<GridTrack<u32>>>>,
  grid_implicit_columns: Option<Source<Vec<GridTrack<u32>>>>,
  grid_implicit_rows: Option<Source<Vec<GridTrack<u32>>>>,
  grid_flow: Option<Source<GridFlow>>,
  grid_row_placement: Option<Source<Line<GridPlacement>>>,
  grid_column_placement: Option<Source<Line<GridPlacement>>>,
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
      offset: None,
      pinned: None,
      color: None,
      border: None,
      spacing: None,
      cursor: None,
      overflow: None,
      shadow: None,
      wrap: None,
      flex: None,
      main_axis_arrangement: None,
      cross_axis_alignment: None,
      line_arrangement: None,
      main_axis_item_alignment: None,
      self_cross_axis_alignment: None,
      self_main_axis_alignment: None,
      grid_columns: None,
      grid_rows: None,
      grid_implicit_columns: None,
      grid_implicit_rows: None,
      grid_flow: None,
      grid_row_placement: None,
      grid_column_placement: None,
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
    Container::mode(Layout::Flow)
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
  /// `grid_columns` (and friends) for any other track layout.
  #[inline(always)]
  pub fn grid(columns: u16) -> Self {
    Container::mode(Layout::Grid).set_grid_columns(GridTrack::Flex(1.).repeated(columns))
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

  /// Sets a static spacing between children: one value for both axes, or a
  /// [`Spacing`] per axis.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: impl Into<Spacing<u32>>) -> Self {
    self.spacing = Some(Source::Static(spacing.into()));

    self
  }

  /// Sets a derived spacing between children.
  #[inline(always)]
  pub fn derive_spacing<S: Into<Spacing<u32>>>(
    mut self,
    spacing: impl Fn() -> S + 'static,
  ) -> Self {
    self.spacing = Some(Source::Derived(Rc::new(move || spacing().into())));

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

  /// Sets a static in-flow shift from each edge.
  #[inline(always)]
  pub fn set_offset(mut self, offset: skene_core::Inset<Dimension<u32>>) -> Self {
    self.offset = Some(Source::Static(offset));

    self
  }

  /// Sets a derived in-flow shift.
  #[inline(always)]
  pub fn derive_offset(
    mut self,
    offset: impl Fn() -> skene_core::Inset<Dimension<u32>> + 'static,
  ) -> Self {
    self.offset = Some(Source::Derived(Rc::new(offset)));

    self
  }

  /// Statically pins the box to the given edges, out of flow, against the nearest
  /// offset or pinned ancestor.
  #[inline(always)]
  pub fn set_pinned(mut self, pinned: skene_core::Inset<Dimension<u32>>) -> Self {
    self.pinned = Some(Source::Static(pinned));

    self
  }

  /// Derives the edges the box is pinned to.
  #[inline(always)]
  pub fn derive_pinned(
    mut self,
    pinned: impl Fn() -> skene_core::Inset<Dimension<u32>> + 'static,
  ) -> Self {
    self.pinned = Some(Source::Derived(Rc::new(pinned)));

    self
  }

  /// Sets a static flex wrapping mode.
  #[inline(always)]
  pub fn set_wrap(mut self, wrap: Wrap) -> Self {
    self.wrap = Some(Source::Static(wrap));

    self
  }

  /// Sets a derived flex wrapping mode.
  #[inline(always)]
  pub fn derive_wrap(mut self, wrap: impl Fn() -> Wrap + 'static) -> Self {
    self.wrap = Some(Source::Derived(Rc::new(wrap)));

    self
  }

  /// Sets a static flexibility as a flex child.
  #[inline(always)]
  pub fn set_flex(mut self, flex: Flexibility<u32>) -> Self {
    self.flex = Some(Source::Static(flex));

    self
  }

  /// Sets a derived flexibility.
  #[inline(always)]
  pub fn derive_flex(mut self, flex: impl Fn() -> Flexibility<u32> + 'static) -> Self {
    self.flex = Some(Source::Derived(Rc::new(flex)));

    self
  }

  /// Sets a static distribution of children along the main (flex) or inline (grid) axis.
  #[inline(always)]
  pub fn set_main_axis_arrangement(mut self, main_axis_arrangement: Arrangement) -> Self {
    self.main_axis_arrangement = Some(Source::Static(main_axis_arrangement));

    self
  }

  /// Sets a derived distribution of children along the main (flex) or inline (grid) axis.
  #[inline(always)]
  pub fn derive_main_axis_arrangement(
    mut self,
    main_axis_arrangement: impl Fn() -> Arrangement + 'static,
  ) -> Self {
    self.main_axis_arrangement = Some(Source::Derived(Rc::new(main_axis_arrangement)));

    self
  }

  /// Sets a static alignment of children on the cross (flex) or block (grid) axis.
  #[inline(always)]
  pub fn set_cross_axis_alignment(mut self, cross_axis_alignment: ItemAlignment) -> Self {
    self.cross_axis_alignment = Some(Source::Static(cross_axis_alignment));

    self
  }

  /// Sets a derived alignment of children on the cross (flex) or block (grid) axis.
  #[inline(always)]
  pub fn derive_cross_axis_alignment(
    mut self,
    cross_axis_alignment: impl Fn() -> ItemAlignment + 'static,
  ) -> Self {
    self.cross_axis_alignment = Some(Source::Derived(Rc::new(cross_axis_alignment)));

    self
  }

  /// Sets a static distribution of wrapped lines or grid tracks along the cross or block axis.
  #[inline(always)]
  pub fn set_line_arrangement(mut self, line_arrangement: LineArrangement) -> Self {
    self.line_arrangement = Some(Source::Static(line_arrangement));

    self
  }

  /// Sets a derived distribution of wrapped lines or grid tracks along the cross or block axis.
  #[inline(always)]
  pub fn derive_line_arrangement(
    mut self,
    line_arrangement: impl Fn() -> LineArrangement + 'static,
  ) -> Self {
    self.line_arrangement = Some(Source::Derived(Rc::new(line_arrangement)));

    self
  }

  /// Sets a static alignment of grid children on the inline axis.
  #[inline(always)]
  pub fn set_main_axis_item_alignment(mut self, main_axis_item_alignment: ItemAlignment) -> Self {
    self.main_axis_item_alignment = Some(Source::Static(main_axis_item_alignment));

    self
  }

  /// Sets a derived alignment of grid children on the inline axis.
  #[inline(always)]
  pub fn derive_main_axis_item_alignment(
    mut self,
    main_axis_item_alignment: impl Fn() -> ItemAlignment + 'static,
  ) -> Self {
    self.main_axis_item_alignment = Some(Source::Derived(Rc::new(main_axis_item_alignment)));

    self
  }

  /// Sets a static cross- or block-axis alignment of the box itself.
  #[inline(always)]
  pub fn set_self_cross_axis_alignment(mut self, self_cross_axis_alignment: ItemAlignment) -> Self {
    self.self_cross_axis_alignment = Some(Source::Static(self_cross_axis_alignment));

    self
  }

  /// Sets a derived cross- or block-axis alignment of the box itself.
  #[inline(always)]
  pub fn derive_self_cross_axis_alignment(
    mut self,
    self_cross_axis_alignment: impl Fn() -> ItemAlignment + 'static,
  ) -> Self {
    self.self_cross_axis_alignment = Some(Source::Derived(Rc::new(self_cross_axis_alignment)));

    self
  }

  /// Sets a static inline-axis alignment of the box itself in a grid.
  #[inline(always)]
  pub fn set_self_main_axis_alignment(mut self, self_main_axis_alignment: ItemAlignment) -> Self {
    self.self_main_axis_alignment = Some(Source::Static(self_main_axis_alignment));

    self
  }

  /// Sets a derived inline-axis alignment of the box itself in a grid.
  #[inline(always)]
  pub fn derive_self_main_axis_alignment(
    mut self,
    self_main_axis_alignment: impl Fn() -> ItemAlignment + 'static,
  ) -> Self {
    self.self_main_axis_alignment = Some(Source::Derived(Rc::new(self_main_axis_alignment)));

    self
  }

  /// Sets static explicit grid column tracks.
  #[inline(always)]
  pub fn set_grid_columns(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.grid_columns = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived explicit grid column tracks.
  #[inline(always)]
  pub fn derive_grid_columns(mut self, tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static) -> Self {
    self.grid_columns = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets static explicit grid row tracks.
  #[inline(always)]
  pub fn set_grid_rows(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.grid_rows = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived explicit grid row tracks.
  #[inline(always)]
  pub fn derive_grid_rows(mut self, tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static) -> Self {
    self.grid_rows = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets static sizes for implicitly created grid columns, cycled.
  #[inline(always)]
  pub fn set_grid_implicit_columns(
    mut self,
    tracks: impl IntoIterator<Item = GridTrack<u32>>,
  ) -> Self {
    self.grid_implicit_columns = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived sizes for implicitly created grid columns, cycled.
  #[inline(always)]
  pub fn derive_grid_implicit_columns(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static,
  ) -> Self {
    self.grid_implicit_columns = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets static sizes for implicitly created grid rows, cycled.
  #[inline(always)]
  pub fn set_grid_implicit_rows(
    mut self,
    tracks: impl IntoIterator<Item = GridTrack<u32>>,
  ) -> Self {
    self.grid_implicit_rows = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived sizes for implicitly created grid rows, cycled.
  #[inline(always)]
  pub fn derive_grid_implicit_rows(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static,
  ) -> Self {
    self.grid_implicit_rows = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets a static direction auto-placed grid children fill.
  #[inline(always)]
  pub fn set_grid_flow(mut self, grid_flow: GridFlow) -> Self {
    self.grid_flow = Some(Source::Static(grid_flow));

    self
  }

  /// Sets a derived direction auto-placed grid children fill.
  #[inline(always)]
  pub fn derive_grid_flow(mut self, grid_flow: impl Fn() -> GridFlow + 'static) -> Self {
    self.grid_flow = Some(Source::Derived(Rc::new(grid_flow)));

    self
  }

  /// Sets a static row placement of the box as a grid child.
  #[inline(always)]
  pub fn set_grid_row_placement(mut self, grid_row_placement: Line<GridPlacement>) -> Self {
    self.grid_row_placement = Some(Source::Static(grid_row_placement));

    self
  }

  /// Sets a derived row placement of the box as a grid child.
  #[inline(always)]
  pub fn derive_grid_row_placement(
    mut self,
    grid_row_placement: impl Fn() -> Line<GridPlacement> + 'static,
  ) -> Self {
    self.grid_row_placement = Some(Source::Derived(Rc::new(grid_row_placement)));

    self
  }

  /// Sets a static column placement of the box as a grid child.
  #[inline(always)]
  pub fn set_grid_column_placement(mut self, grid_column_placement: Line<GridPlacement>) -> Self {
    self.grid_column_placement = Some(Source::Static(grid_column_placement));

    self
  }

  /// Sets a derived column placement of the box as a grid child.
  #[inline(always)]
  pub fn derive_grid_column_placement(
    mut self,
    grid_column_placement: impl Fn() -> Line<GridPlacement> + 'static,
  ) -> Self {
    self.grid_column_placement = Some(Source::Derived(Rc::new(grid_column_placement)));

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
      self.offset,
      &mut style.offset,
      &mut binders,
      BoxProp::Offset,
      update_box,
    );

    split_prop(
      self.pinned,
      &mut style.pinned,
      &mut binders,
      BoxProp::Pinned,
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
      self.spacing,
      &mut style.spacing,
      &mut binders,
      BoxProp::Spacing,
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
      self.wrap,
      &mut style.wrap,
      &mut binders,
      BoxProp::Wrap,
      update_box,
    );

    split_prop(
      self.flex,
      &mut style.flex,
      &mut binders,
      BoxProp::Flex,
      update_box,
    );

    split_prop(
      self.main_axis_arrangement,
      &mut style.main_axis_arrangement,
      &mut binders,
      BoxProp::Arrangement,
      update_box,
    );

    split_prop(
      self.cross_axis_alignment,
      &mut style.cross_axis_alignment,
      &mut binders,
      BoxProp::CrossAxisAlignment,
      update_box,
    );

    split_prop(
      self.line_arrangement,
      &mut style.line_arrangement,
      &mut binders,
      BoxProp::LineArrangement,
      update_box,
    );

    split_prop(
      self.main_axis_item_alignment,
      &mut style.main_axis_item_alignment,
      &mut binders,
      BoxProp::MainAxisItemAlignment,
      update_box,
    );

    split_prop(
      self.self_cross_axis_alignment,
      &mut style.self_cross_axis_alignment,
      &mut binders,
      BoxProp::SelfCrossAxisAlignment,
      update_box,
    );

    split_prop(
      self.self_main_axis_alignment,
      &mut style.self_main_axis_alignment,
      &mut binders,
      BoxProp::SelfMainAxisAlignment,
      update_box,
    );

    split_prop(
      self.grid_columns,
      &mut style.grid_columns,
      &mut binders,
      BoxProp::GridColumns,
      update_box,
    );

    split_prop(
      self.grid_rows,
      &mut style.grid_rows,
      &mut binders,
      BoxProp::GridRows,
      update_box,
    );

    split_prop(
      self.grid_implicit_columns,
      &mut style.grid_implicit_columns,
      &mut binders,
      BoxProp::GridImplicitColumns,
      update_box,
    );

    split_prop(
      self.grid_implicit_rows,
      &mut style.grid_implicit_rows,
      &mut binders,
      BoxProp::GridImplicitRows,
      update_box,
    );

    split_prop(
      self.grid_flow,
      &mut style.grid_flow,
      &mut binders,
      BoxProp::GridFlow,
      update_box,
    );

    split_prop(
      self.grid_row_placement,
      &mut style.grid_row_placement,
      &mut binders,
      BoxProp::GridRowPlacement,
      update_box,
    );

    split_prop(
      self.grid_column_placement,
      &mut style.grid_column_placement,
      &mut binders,
      BoxProp::GridColumnPlacement,
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
