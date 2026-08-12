use std::rc::Rc;

use skene_core::{
  Animation,
  Arrangement,
  BoxStyle,
  Color,
  Condition,
  Dimension,
  Flexibility,
  GridFlow,
  GridPlacement,
  GridTrack,
  ItemAlignment,
  KeyEvent,
  Line,
  MouseCursor,
  Overflow,
  PointerEvent,
  Prevent,
  ScrollEvent,
  Source,
  Spacing,
  Transition,
  Wrap,
};

use crate::{Border, Children, HasChildren, Inset, Layout, Role};

/// A box that decorates itself and flows its children in document order.
///
/// The shared box surface: the typed containers—[`Row`](crate::Row),
/// [`Column`](crate::Column), [`Grid`](crate::Grid), [`Stack`](crate::Stack)—each wrap
/// one and layer their layout over it, and a container view of your own does the same.
/// Children are set with [`set_children`](HasChildren::set_children) or
/// `derive_children`; decoration and sizing with the chainable `set_`/`derive_` methods.
/// Each platform realizes it its own way.
#[must_use]
pub struct StyledBox {
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
  pub(crate) offset: Option<Source<skene_core::Inset<Dimension<f32>>>>,
  pub(crate) pinned: Option<Source<skene_core::Inset<Dimension<f32>>>>,
  pub(crate) color: Option<Source<Color>>,
  pub(crate) border: Option<Source<Border>>,
  pub(crate) border_radius: Option<Source<f32>>,
  pub(crate) cursor: Option<Source<MouseCursor>>,
  pub(crate) spacing: Option<Source<Spacing<f32>>>,
  pub(crate) wrap: Option<Source<Wrap>>,
  pub(crate) flex: Option<Source<Flexibility<f32>>>,
  pub(crate) arrangement: Option<Source<Arrangement>>,
  pub(crate) item_alignment: Option<Source<ItemAlignment>>,
  pub(crate) line_arrangement: Option<Source<Arrangement>>,
  pub(crate) item_horizontal_alignment: Option<Source<ItemAlignment>>,
  pub(crate) self_alignment: Option<Source<ItemAlignment>>,
  pub(crate) self_horizontal_alignment: Option<Source<ItemAlignment>>,
  pub(crate) grid_columns: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_rows: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_implicit_columns: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_implicit_rows: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_flow: Option<Source<GridFlow>>,
  pub(crate) grid_row_placement: Option<Source<Line<GridPlacement>>>,
  pub(crate) grid_column_placement: Option<Source<Line<GridPlacement>>>,
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

impl StyledBox {
  #[inline(always)]
  pub(crate) fn mode(layout: Layout) -> Self {
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
      offset: None,
      pinned: None,
      color: None,
      border: None,
      border_radius: None,
      cursor: None,
      spacing: None,
      wrap: None,
      flex: None,
      arrangement: None,
      item_alignment: None,
      line_arrangement: None,
      item_horizontal_alignment: None,
      self_alignment: None,
      self_horizontal_alignment: None,
      grid_columns: None,
      grid_rows: None,
      grid_implicit_columns: None,
      grid_implicit_rows: None,
      grid_flow: None,
      grid_row_placement: None,
      grid_column_placement: None,
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
    StyledBox::mode(Layout::Flow)
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

  /// Sets a static in-flow shift from each edge.
  #[inline(always)]
  pub fn set_offset(mut self, offset: skene_core::Inset<Dimension<f32>>) -> Self {
    self.offset = Some(Source::Static(offset));

    self
  }

  /// Sets a derived in-flow shift.
  #[inline(always)]
  pub fn derive_offset(
    mut self,
    offset: impl Fn() -> skene_core::Inset<Dimension<f32>> + 'static,
  ) -> Self {
    self.offset = Some(Source::Derived(Rc::new(offset)));

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

impl Default for StyledBox {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl HasChildren<crate::GuiOps> for StyledBox {
  type Kind = skene_core::PlainChild;

  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl skene_core::View for StyledBox {
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
      self.offset,
      &mut style.offset,
      &mut binders,
      BoxProp::Offset,
      driver::update_box,
    );

    split_prop(
      self.pinned,
      &mut style.pinned,
      &mut binders,
      BoxProp::Pinned,
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
      self.spacing,
      &mut style.spacing,
      &mut binders,
      BoxProp::Spacing,
      driver::update_box,
    );

    split_prop(
      self.wrap,
      &mut style.wrap,
      &mut binders,
      BoxProp::Wrap,
      driver::update_box,
    );

    split_prop(
      self.flex,
      &mut style.flex,
      &mut binders,
      BoxProp::Flex,
      driver::update_box,
    );

    split_prop(
      self.arrangement,
      &mut style.arrangement,
      &mut binders,
      BoxProp::Arrangement,
      driver::update_box,
    );

    split_prop(
      self.item_alignment,
      &mut style.item_alignment,
      &mut binders,
      BoxProp::ItemAlignment,
      driver::update_box,
    );

    split_prop(
      self.line_arrangement,
      &mut style.line_arrangement,
      &mut binders,
      BoxProp::LineArrangement,
      driver::update_box,
    );

    split_prop(
      self.item_horizontal_alignment,
      &mut style.item_horizontal_alignment,
      &mut binders,
      BoxProp::ItemHorizontalAlignment,
      driver::update_box,
    );

    split_prop(
      self.self_alignment,
      &mut style.self_alignment,
      &mut binders,
      BoxProp::SelfAlignment,
      driver::update_box,
    );

    split_prop(
      self.self_horizontal_alignment,
      &mut style.self_horizontal_alignment,
      &mut binders,
      BoxProp::SelfHorizontalAlignment,
      driver::update_box,
    );

    split_prop(
      self.grid_columns,
      &mut style.grid_columns,
      &mut binders,
      BoxProp::GridColumns,
      driver::update_box,
    );

    split_prop(
      self.grid_rows,
      &mut style.grid_rows,
      &mut binders,
      BoxProp::GridRows,
      driver::update_box,
    );

    split_prop(
      self.grid_implicit_columns,
      &mut style.grid_implicit_columns,
      &mut binders,
      BoxProp::GridImplicitColumns,
      driver::update_box,
    );

    split_prop(
      self.grid_implicit_rows,
      &mut style.grid_implicit_rows,
      &mut binders,
      BoxProp::GridImplicitRows,
      driver::update_box,
    );

    split_prop(
      self.grid_flow,
      &mut style.grid_flow,
      &mut binders,
      BoxProp::GridFlow,
      driver::update_box,
    );

    split_prop(
      self.grid_row_placement,
      &mut style.grid_row_placement,
      &mut binders,
      BoxProp::GridRowPlacement,
      driver::update_box,
    );

    split_prop(
      self.grid_column_placement,
      &mut style.grid_column_placement,
      &mut binders,
      BoxProp::GridColumnPlacement,
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
