use std::rc::Rc;

use skene_core::{
  AlignContent,
  AlignItems,
  AlignSelf,
  Animation,
  BoxStyle,
  Color,
  Condition,
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
  Line,
  MouseCursor,
  Overflow,
  PointerEvent,
  Position,
  Prevent,
  ScrollEvent,
  Source,
  Transition,
};

use crate::{Border, Children, HasChildren, Inset, Layout, Role};

/// A box that decorates itself and lays out its children.
///
/// Constructed in a layout mode: [`new`](Container::new) (block normal flow),
/// [`row`](Container::row) / [`column`](Container::column) (flex),
/// [`grid`](Container::grid), or [`stack`](Container::stack) (children overlapping in
/// one cell). Children are set with [`set_children`](HasChildren::set_children) or
/// `derive_children`; decoration, sizing, and layout are set with the chainable
/// `set_`/`derive_` methods. Each platform realizes it its own way.
#[must_use]
pub struct Container {
  pub(crate) children: Children,
  pub(crate) layout: Layout,
  pub(crate) role: Option<Role>,
  pub(crate) width: Option<Source<Dimension<f32>>>,
  pub(crate) height: Option<Source<Dimension<f32>>>,
  pub(crate) min_width: Option<Source<Dimension<f32>>>,
  pub(crate) min_height: Option<Source<Dimension<f32>>>,
  pub(crate) max_width: Option<Source<Dimension<f32>>>,
  pub(crate) max_height: Option<Source<Dimension<f32>>>,
  pub(crate) aspect_ratio: Option<Source<f32>>,
  pub(crate) margin: Option<Source<skene_core::Inset<Dimension<f32>>>>,
  pub(crate) padding: Option<Source<Inset>>,
  pub(crate) position: Option<Source<Position>>,
  pub(crate) inset: Option<Source<skene_core::Inset<Dimension<f32>>>>,
  pub(crate) color: Option<Source<Color>>,
  pub(crate) border: Option<Source<Border>>,
  pub(crate) border_radius: Option<Source<f32>>,
  pub(crate) cursor: Option<Source<MouseCursor>>,
  pub(crate) row_gap: Option<Source<f32>>,
  pub(crate) column_gap: Option<Source<f32>>,
  pub(crate) flex_wrap: Option<Source<FlexWrap>>,
  pub(crate) flex_grow: Option<Source<f32>>,
  pub(crate) flex_shrink: Option<Source<f32>>,
  pub(crate) flex_basis: Option<Source<Dimension<f32>>>,
  pub(crate) justify_content: Option<Source<JustifyContent>>,
  pub(crate) align_items: Option<Source<AlignItems>>,
  pub(crate) align_content: Option<Source<AlignContent>>,
  pub(crate) justify_items: Option<Source<JustifyItems>>,
  pub(crate) align_self: Option<Source<AlignSelf>>,
  pub(crate) justify_self: Option<Source<JustifySelf>>,
  pub(crate) grid_template_columns: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_template_rows: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_auto_columns: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_auto_rows: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_auto_flow: Option<Source<GridAutoFlow>>,
  pub(crate) grid_row: Option<Source<Line<GridPlacement>>>,
  pub(crate) grid_column: Option<Source<Line<GridPlacement>>>,
  pub(crate) overflow: Option<Source<Overflow>>,
  pub(crate) anchor: Option<crate::Anchor>,
  pub(crate) variants: Vec<(Condition, BoxStyle)>,
  pub(crate) transitions: Vec<Transition>,
  pub(crate) animations: Vec<Animation<BoxStyle>>,
  pub(crate) click_handler: Option<Rc<skene_core::ClickHandler>>,
  pub(crate) pointer_enter_handler: Option<Rc<dyn Fn()>>,
  pub(crate) pointer_leave_handler: Option<Rc<dyn Fn()>>,
  pub(crate) focus_handler: Option<Rc<dyn Fn()>>,
  pub(crate) blur_handler: Option<Rc<dyn Fn()>>,
  pub(crate) key_down_handler: Option<Rc<skene_core::KeyHandler>>,
  pub(crate) pointer_down_handler: Option<Rc<skene_core::PointerHandler>>,
  pub(crate) pointer_up_handler: Option<Rc<skene_core::PointerHandler>>,
  pub(crate) pointer_move_handler: Option<Rc<skene_core::PointerHandler>>,
  #[expect(clippy::type_complexity)]
  pub(crate) scroll_handler: Option<Rc<dyn Fn(&ScrollEvent)>>,
}

impl Container {
  #[inline(always)]
  fn mode(layout: Layout) -> Self {
    Self {
      children: Children::default(),
      layout,
      role: None,
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
      border_radius: None,
      cursor: None,
      row_gap: None,
      column_gap: None,
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
      overflow: None,
      anchor: None,
      variants: Vec::new(),
      transitions: Vec::new(),
      animations: Vec::new(),
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

  /// Sets the semantic role.
  #[inline(always)]
  pub fn set_role(mut self, role: Role) -> Self {
    self.role = Some(role);

    self
  }

  /// Sets a static width: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_width(mut self, width: impl Into<Dimension<f32>>) -> Self {
    self.width = Some(Source::Static(width.into()));

    self
  }

  /// Sets a derived width.
  #[inline(always)]
  pub fn derive_width<D: Into<Dimension<f32>>>(mut self, width: impl Fn() -> D + 'static) -> Self {
    self.width = Some(Source::Derived(Rc::new(move || width().into())));

    self
  }

  /// Sets a static height: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_height(mut self, height: impl Into<Dimension<f32>>) -> Self {
    self.height = Some(Source::Static(height.into()));

    self
  }

  /// Sets a derived height.
  #[inline(always)]
  pub fn derive_height<D: Into<Dimension<f32>>>(
    mut self,
    height: impl Fn() -> D + 'static,
  ) -> Self {
    self.height = Some(Source::Derived(Rc::new(move || height().into())));

    self
  }

  /// Sets a static minimum width: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_min_width(mut self, min_width: impl Into<Dimension<f32>>) -> Self {
    self.min_width = Some(Source::Static(min_width.into()));

    self
  }

  /// Sets a derived minimum width.
  #[inline(always)]
  pub fn derive_min_width<D: Into<Dimension<f32>>>(
    mut self,
    min_width: impl Fn() -> D + 'static,
  ) -> Self {
    self.min_width = Some(Source::Derived(Rc::new(move || min_width().into())));

    self
  }

  /// Sets a static minimum height: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_min_height(mut self, min_height: impl Into<Dimension<f32>>) -> Self {
    self.min_height = Some(Source::Static(min_height.into()));

    self
  }

  /// Sets a derived minimum height.
  #[inline(always)]
  pub fn derive_min_height<D: Into<Dimension<f32>>>(
    mut self,
    min_height: impl Fn() -> D + 'static,
  ) -> Self {
    self.min_height = Some(Source::Derived(Rc::new(move || min_height().into())));

    self
  }

  /// Sets a static maximum width: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_max_width(mut self, max_width: impl Into<Dimension<f32>>) -> Self {
    self.max_width = Some(Source::Static(max_width.into()));

    self
  }

  /// Sets a derived maximum width.
  #[inline(always)]
  pub fn derive_max_width<D: Into<Dimension<f32>>>(
    mut self,
    max_width: impl Fn() -> D + 'static,
  ) -> Self {
    self.max_width = Some(Source::Derived(Rc::new(move || max_width().into())));

    self
  }

  /// Sets a static maximum height: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_max_height(mut self, max_height: impl Into<Dimension<f32>>) -> Self {
    self.max_height = Some(Source::Static(max_height.into()));

    self
  }

  /// Sets a derived maximum height.
  #[inline(always)]
  pub fn derive_max_height<D: Into<Dimension<f32>>>(
    mut self,
    max_height: impl Fn() -> D + 'static,
  ) -> Self {
    self.max_height = Some(Source::Derived(Rc::new(move || max_height().into())));

    self
  }

  /// Sets a static padding.
  #[inline(always)]
  pub fn set_padding(mut self, padding: Inset) -> Self {
    self.padding = Some(Source::Static(padding));

    self
  }

  /// Sets a derived padding.
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

  /// Sets a static border.
  #[inline(always)]
  pub fn set_border(mut self, border: Border) -> Self {
    self.border = Some(Source::Static(border));

    self
  }

  /// Sets a derived border.
  #[inline(always)]
  pub fn derive_border(mut self, border: impl Fn() -> Border + 'static) -> Self {
    self.border = Some(Source::Derived(Rc::new(border)));

    self
  }

  /// Sets a static border radius.
  #[inline(always)]
  pub fn set_border_radius(mut self, border_radius: f32) -> Self {
    self.border_radius = Some(Source::Static(border_radius));

    self
  }

  /// Sets a derived border radius.
  #[inline(always)]
  pub fn derive_border_radius(mut self, border_radius: impl Fn() -> f32 + 'static) -> Self {
    self.border_radius = Some(Source::Derived(Rc::new(border_radius)));

    self
  }

  /// Sets a static mouse cursor.
  #[inline(always)]
  pub fn set_cursor(mut self, cursor: MouseCursor) -> Self {
    self.cursor = Some(Source::Static(cursor));

    self
  }

  /// Sets a derived mouse cursor.
  #[inline(always)]
  pub fn derive_cursor(mut self, cursor: impl Fn() -> MouseCursor + 'static) -> Self {
    self.cursor = Some(Source::Derived(Rc::new(cursor)));

    self
  }

  /// Sets a static gap between children, on both axes.
  #[inline(always)]
  pub fn set_gap(mut self, gap: f32) -> Self {
    self.row_gap = Some(Source::Static(gap));
    self.column_gap = Some(Source::Static(gap));

    self
  }

  /// Sets a derived gap between children, on both axes.
  #[inline(always)]
  pub fn derive_gap(mut self, gap: impl Fn() -> f32 + 'static) -> Self {
    let gap: Rc<dyn Fn() -> f32> = Rc::new(gap);

    self.row_gap = Some(Source::Derived(Rc::clone(&gap)));
    self.column_gap = Some(Source::Derived(gap));

    self
  }

  /// Sets a static gap between rows of children.
  #[inline(always)]
  pub fn set_row_gap(mut self, row_gap: f32) -> Self {
    self.row_gap = Some(Source::Static(row_gap));

    self
  }

  /// Sets a derived gap between rows of children.
  #[inline(always)]
  pub fn derive_row_gap(mut self, row_gap: impl Fn() -> f32 + 'static) -> Self {
    self.row_gap = Some(Source::Derived(Rc::new(row_gap)));

    self
  }

  /// Sets a static gap between columns of children.
  #[inline(always)]
  pub fn set_column_gap(mut self, column_gap: f32) -> Self {
    self.column_gap = Some(Source::Static(column_gap));

    self
  }

  /// Sets a derived gap between columns of children.
  #[inline(always)]
  pub fn derive_column_gap(mut self, column_gap: impl Fn() -> f32 + 'static) -> Self {
    self.column_gap = Some(Source::Derived(Rc::new(column_gap)));

    self
  }

  /// Sets a static width-to-height ratio the unsized axis follows.
  #[inline(always)]
  pub fn set_aspect_ratio(mut self, aspect_ratio: f32) -> Self {
    self.aspect_ratio = Some(Source::Static(aspect_ratio));

    self
  }

  /// Sets a derived width-to-height ratio the unsized axis follows.
  #[inline(always)]
  pub fn derive_aspect_ratio(mut self, aspect_ratio: impl Fn() -> f32 + 'static) -> Self {
    self.aspect_ratio = Some(Source::Derived(Rc::new(aspect_ratio)));

    self
  }

  /// Sets a static margin on each edge.
  #[inline(always)]
  pub fn set_margin(mut self, margin: skene_core::Inset<Dimension<f32>>) -> Self {
    self.margin = Some(Source::Static(margin));

    self
  }

  /// Sets a derived margin on each edge.
  #[inline(always)]
  pub fn derive_margin(
    mut self,
    margin: impl Fn() -> skene_core::Inset<Dimension<f32>> + 'static,
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
  pub fn set_inset(mut self, inset: skene_core::Inset<Dimension<f32>>) -> Self {
    self.inset = Some(Source::Static(inset));

    self
  }

  /// Sets a derived offset from each edge, for a positioned box.
  #[inline(always)]
  pub fn derive_inset(
    mut self,
    inset: impl Fn() -> skene_core::Inset<Dimension<f32>> + 'static,
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
  pub fn set_flex_basis(mut self, flex_basis: impl Into<Dimension<f32>>) -> Self {
    self.flex_basis = Some(Source::Static(flex_basis.into()));

    self
  }

  /// Sets a derived initial main-axis size as a flex item.
  #[inline(always)]
  pub fn derive_flex_basis<D: Into<Dimension<f32>>>(
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
    tracks: impl IntoIterator<Item = GridTrack<f32>>,
  ) -> Self {
    self.grid_template_columns = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived explicit grid column tracks.
  #[inline(always)]
  pub fn derive_grid_template_columns(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<f32>> + 'static,
  ) -> Self {
    self.grid_template_columns = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets static explicit grid row tracks.
  #[inline(always)]
  pub fn set_grid_template_rows(
    mut self,
    tracks: impl IntoIterator<Item = GridTrack<f32>>,
  ) -> Self {
    self.grid_template_rows = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived explicit grid row tracks.
  #[inline(always)]
  pub fn derive_grid_template_rows(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<f32>> + 'static,
  ) -> Self {
    self.grid_template_rows = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets static sizes for implicitly created grid columns, cycled.
  #[inline(always)]
  pub fn set_grid_auto_columns(mut self, tracks: impl IntoIterator<Item = GridTrack<f32>>) -> Self {
    self.grid_auto_columns = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived sizes for implicitly created grid columns, cycled.
  #[inline(always)]
  pub fn derive_grid_auto_columns(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<f32>> + 'static,
  ) -> Self {
    self.grid_auto_columns = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets static sizes for implicitly created grid rows, cycled.
  #[inline(always)]
  pub fn set_grid_auto_rows(mut self, tracks: impl IntoIterator<Item = GridTrack<f32>>) -> Self {
    self.grid_auto_rows = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets derived sizes for implicitly created grid rows, cycled.
  #[inline(always)]
  pub fn derive_grid_auto_rows(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<f32>> + 'static,
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
  ///
  /// Declarative: it compiles to a stylesheet rule, so it works in static output with no
  /// script. Later variants win over earlier ones.
  #[inline(always)]
  pub fn set_when(mut self, condition: Condition, style: BoxStyle) -> Self {
    self.variants.push((condition, style));

    self
  }

  /// Adds a property transition, interpolating that property whenever its value changes.
  #[inline(always)]
  pub fn set_transition(mut self, transition: Transition) -> Self {
    self.transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(mut self, animation: Animation<BoxStyle>) -> Self {
    self.animations.push(animation);

    self
  }

  /// Sets the click handler.
  #[inline(always)]
  pub fn handle_click<R: Into<Prevent>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self.click_handler = Some(Rc::new(move || action().into()));

    self
  }

  /// Sets the pointer-enter handler.
  #[inline(always)]
  pub fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self.pointer_enter_handler = Some(Rc::new(action));

    self
  }

  /// Sets the pointer-leave handler.
  #[inline(always)]
  pub fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self.pointer_leave_handler = Some(Rc::new(action));

    self
  }

  /// Sets the focus handler, run when the box or a descendant gains focus.
  ///
  /// A box never takes focus itself—only controls do; this observes focus within it.
  #[inline(always)]
  pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.focus_handler = Some(Rc::new(action));

    self
  }

  /// Sets the blur handler, run when the box or a descendant loses focus.
  #[inline(always)]
  pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.blur_handler = Some(Rc::new(action));

    self
  }

  /// Sets the key-press handler, run for keys pressed on the box or within it.
  #[inline(always)]
  pub fn handle_key_down<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&KeyEvent) -> R + 'static,
  ) -> Self {
    self.key_down_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-down handler, run for presses on the box or within it.
  #[inline(always)]
  pub fn handle_pointer_down<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.pointer_down_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-up handler, run for releases on the box or within it.
  #[inline(always)]
  pub fn handle_pointer_up<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.pointer_up_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-move handler, run for moves over the box or within it.
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

  /// Sets a static overflow behavior.
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

impl HasChildren<crate::GuiOps> for Container {
  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl skene_core::View for Container {
  // One `split_prop` call per property; length is the property count, not complexity.
  #[expect(clippy::too_many_lines)]
  fn build(self) -> skene_core::NodeId {
    use skene_core::{BoxProp, BoxProps};

    use crate::{
      driver,
      realize::{Binders, split_prop},
    };

    let mut style = BoxStyle::default();
    let mut binders = Binders::new();

    split_prop(
      self.width,
      &mut style.width,
      &mut binders,
      BoxProp::Width,
      driver::update_box,
    );

    split_prop(
      self.height,
      &mut style.height,
      &mut binders,
      BoxProp::Height,
      driver::update_box,
    );

    split_prop(
      self.min_width,
      &mut style.min_width,
      &mut binders,
      BoxProp::MinWidth,
      driver::update_box,
    );

    split_prop(
      self.min_height,
      &mut style.min_height,
      &mut binders,
      BoxProp::MinHeight,
      driver::update_box,
    );

    split_prop(
      self.max_width,
      &mut style.max_width,
      &mut binders,
      BoxProp::MaxWidth,
      driver::update_box,
    );

    split_prop(
      self.max_height,
      &mut style.max_height,
      &mut binders,
      BoxProp::MaxHeight,
      driver::update_box,
    );

    split_prop(
      self.aspect_ratio,
      &mut style.aspect_ratio,
      &mut binders,
      BoxProp::AspectRatio,
      driver::update_box,
    );

    split_prop(
      self.margin,
      &mut style.margin,
      &mut binders,
      BoxProp::Margin,
      driver::update_box,
    );

    split_prop(
      self.padding,
      &mut style.padding,
      &mut binders,
      BoxProp::Padding,
      driver::update_box,
    );

    split_prop(
      self.position,
      &mut style.position,
      &mut binders,
      BoxProp::Position,
      driver::update_box,
    );

    split_prop(
      self.inset,
      &mut style.inset,
      &mut binders,
      BoxProp::Inset,
      driver::update_box,
    );

    split_prop(
      self.color,
      &mut style.color,
      &mut binders,
      BoxProp::Color,
      driver::update_box,
    );

    split_prop(
      self.border,
      &mut style.border,
      &mut binders,
      BoxProp::Border,
      driver::update_box,
    );

    split_prop(
      self.border_radius,
      &mut style.border_radius,
      &mut binders,
      BoxProp::BorderRadius,
      driver::update_box,
    );

    split_prop(
      self.cursor,
      &mut style.cursor,
      &mut binders,
      BoxProp::Cursor,
      driver::update_box,
    );

    split_prop(
      self.row_gap,
      &mut style.row_gap,
      &mut binders,
      BoxProp::RowGap,
      driver::update_box,
    );

    split_prop(
      self.column_gap,
      &mut style.column_gap,
      &mut binders,
      BoxProp::ColumnGap,
      driver::update_box,
    );

    split_prop(
      self.flex_wrap,
      &mut style.flex_wrap,
      &mut binders,
      BoxProp::FlexWrap,
      driver::update_box,
    );

    split_prop(
      self.flex_grow,
      &mut style.flex_grow,
      &mut binders,
      BoxProp::FlexGrow,
      driver::update_box,
    );

    split_prop(
      self.flex_shrink,
      &mut style.flex_shrink,
      &mut binders,
      BoxProp::FlexShrink,
      driver::update_box,
    );

    split_prop(
      self.flex_basis,
      &mut style.flex_basis,
      &mut binders,
      BoxProp::FlexBasis,
      driver::update_box,
    );

    split_prop(
      self.justify_content,
      &mut style.justify_content,
      &mut binders,
      BoxProp::JustifyContent,
      driver::update_box,
    );

    split_prop(
      self.align_items,
      &mut style.align_items,
      &mut binders,
      BoxProp::AlignItems,
      driver::update_box,
    );

    split_prop(
      self.align_content,
      &mut style.align_content,
      &mut binders,
      BoxProp::AlignContent,
      driver::update_box,
    );

    split_prop(
      self.justify_items,
      &mut style.justify_items,
      &mut binders,
      BoxProp::JustifyItems,
      driver::update_box,
    );

    split_prop(
      self.align_self,
      &mut style.align_self,
      &mut binders,
      BoxProp::AlignSelf,
      driver::update_box,
    );

    split_prop(
      self.justify_self,
      &mut style.justify_self,
      &mut binders,
      BoxProp::JustifySelf,
      driver::update_box,
    );

    split_prop(
      self.grid_template_columns,
      &mut style.grid_template_columns,
      &mut binders,
      BoxProp::GridTemplateColumns,
      driver::update_box,
    );

    split_prop(
      self.grid_template_rows,
      &mut style.grid_template_rows,
      &mut binders,
      BoxProp::GridTemplateRows,
      driver::update_box,
    );

    split_prop(
      self.grid_auto_columns,
      &mut style.grid_auto_columns,
      &mut binders,
      BoxProp::GridAutoColumns,
      driver::update_box,
    );

    split_prop(
      self.grid_auto_rows,
      &mut style.grid_auto_rows,
      &mut binders,
      BoxProp::GridAutoRows,
      driver::update_box,
    );

    split_prop(
      self.grid_auto_flow,
      &mut style.grid_auto_flow,
      &mut binders,
      BoxProp::GridAutoFlow,
      driver::update_box,
    );

    split_prop(
      self.grid_row,
      &mut style.grid_row,
      &mut binders,
      BoxProp::GridRow,
      driver::update_box,
    );

    split_prop(
      self.grid_column,
      &mut style.grid_column,
      &mut binders,
      BoxProp::GridColumn,
      driver::update_box,
    );

    split_prop(
      self.overflow,
      &mut style.overflow,
      &mut binders,
      BoxProp::Overflow,
      driver::update_box,
    );

    let node = driver::create_box(&BoxProps {
      layout: self.layout,
      role: self.role,
      style,
      variants: self.variants,
      transitions: self.transitions,
      animations: self.animations,
    });

    for binder in binders {
      binder(node);
    }

    if let Some(action) = self.click_handler {
      driver::handle_click(node, action);
    }

    if let Some(action) = self.pointer_enter_handler {
      driver::handle_pointer_enter(node, action);
    }

    if let Some(action) = self.pointer_leave_handler {
      driver::handle_pointer_leave(node, action);
    }

    if let Some(action) = self.focus_handler {
      driver::handle_focus(node, action);
    }

    if let Some(action) = self.blur_handler {
      driver::handle_blur(node, action);
    }

    if let Some(action) = self.key_down_handler {
      driver::handle_key_down(node, action);
    }

    if let Some(action) = self.pointer_down_handler {
      driver::handle_pointer_down(node, action);
    }

    if let Some(action) = self.pointer_up_handler {
      driver::handle_pointer_up(node, action);
    }

    if let Some(action) = self.pointer_move_handler {
      driver::handle_pointer_move(node, action);
    }

    if let Some(action) = self.scroll_handler {
      driver::handle_scroll(node, action);
    }

    if let Some(anchor) = &self.anchor {
      anchor.assign(node);
    }

    match self.layout {
      // Overlap the children in one grid cell.
      Layout::Stack => self.children.mount_with(node, |child| {
        driver::place_in_stack(child);

        child
      }),
      _ => self.children.mount(node),
    }

    node
  }
}
