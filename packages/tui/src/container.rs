//! A box that decorates itself and lays out its children, in cells.

use std::rc::Rc;

use skene_core::{
  Axis,
  Color,
  CrossAxisAlignment,
  KeyEvent,
  Layout,
  MainAxisAlignment,
  MouseCursor,
  NodeId,
  Overflow,
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
  width: Option<Source<u32>>,
  height: Option<Source<u32>>,
  min_width: Option<Source<u32>>,
  min_height: Option<Source<u32>>,
  max_width: Option<Source<u32>>,
  max_height: Option<Source<u32>>,
  padding: Option<Source<Inset>>,
  color: Option<Source<Color>>,
  border: Option<Source<Color>>,
  spacing: Option<Source<u32>>,
  cursor: Option<Source<MouseCursor>>,
  overflow: Option<Source<Overflow>>,
  shadow: Option<Source<Color>>,
  main_axis_alignment: Option<Source<MainAxisAlignment>>,
  cross_axis_alignment: Option<Source<CrossAxisAlignment>>,
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
      padding: None,
      color: None,
      border: None,
      spacing: None,
      cursor: None,
      overflow: None,
      shadow: None,
      main_axis_alignment: None,
      cross_axis_alignment: None,
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
    Container::mode(Layout::Flex {
      direction: Axis::Horizontal,
    })
  }

  /// A flex container laying children out top to bottom.
  #[inline(always)]
  pub fn column() -> Self {
    Container::mode(Layout::Flex {
      direction: Axis::Vertical,
    })
  }

  /// A grid container with `columns` equal-width columns.
  #[inline(always)]
  pub fn grid(columns: u16) -> Self {
    Container::mode(Layout::Grid { columns })
  }

  /// A container overlapping its children in one cell, stacked in order (last on top).
  #[inline(always)]
  pub fn stack() -> Self {
    Container::mode(Layout::Stack)
  }

  /// Sets a static width, in cells.
  #[inline(always)]
  pub fn set_width(mut self, width: u32) -> Self {
    self.width = Some(Source::Static(width));

    self
  }

  /// Sets a derived width, in cells.
  #[inline(always)]
  pub fn derive_width(mut self, width: impl Fn() -> u32 + 'static) -> Self {
    self.width = Some(Source::Derived(Rc::new(width)));

    self
  }

  /// Sets a static height, in cells.
  #[inline(always)]
  pub fn set_height(mut self, height: u32) -> Self {
    self.height = Some(Source::Static(height));

    self
  }

  /// Sets a derived height, in cells.
  #[inline(always)]
  pub fn derive_height(mut self, height: impl Fn() -> u32 + 'static) -> Self {
    self.height = Some(Source::Derived(Rc::new(height)));

    self
  }

  /// Sets a static minimum width, in cells.
  #[inline(always)]
  pub fn set_min_width(mut self, min_width: u32) -> Self {
    self.min_width = Some(Source::Static(min_width));

    self
  }

  /// Sets a derived minimum width, in cells.
  #[inline(always)]
  pub fn derive_min_width(mut self, min_width: impl Fn() -> u32 + 'static) -> Self {
    self.min_width = Some(Source::Derived(Rc::new(min_width)));

    self
  }

  /// Sets a static minimum height, in cells.
  #[inline(always)]
  pub fn set_min_height(mut self, min_height: u32) -> Self {
    self.min_height = Some(Source::Static(min_height));

    self
  }

  /// Sets a derived minimum height, in cells.
  #[inline(always)]
  pub fn derive_min_height(mut self, min_height: impl Fn() -> u32 + 'static) -> Self {
    self.min_height = Some(Source::Derived(Rc::new(min_height)));

    self
  }

  /// Sets a static maximum width, in cells.
  #[inline(always)]
  pub fn set_max_width(mut self, max_width: u32) -> Self {
    self.max_width = Some(Source::Static(max_width));

    self
  }

  /// Sets a derived maximum width, in cells.
  #[inline(always)]
  pub fn derive_max_width(mut self, max_width: impl Fn() -> u32 + 'static) -> Self {
    self.max_width = Some(Source::Derived(Rc::new(max_width)));

    self
  }

  /// Sets a static maximum height, in cells.
  #[inline(always)]
  pub fn set_max_height(mut self, max_height: u32) -> Self {
    self.max_height = Some(Source::Static(max_height));

    self
  }

  /// Sets a derived maximum height, in cells.
  #[inline(always)]
  pub fn derive_max_height(mut self, max_height: impl Fn() -> u32 + 'static) -> Self {
    self.max_height = Some(Source::Derived(Rc::new(max_height)));

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

  /// Sets a static gap between children, in cells.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: u32) -> Self {
    self.spacing = Some(Source::Static(spacing));

    self
  }

  /// Sets a derived gap between children, in cells.
  #[inline(always)]
  pub fn derive_spacing(mut self, spacing: impl Fn() -> u32 + 'static) -> Self {
    self.spacing = Some(Source::Derived(Rc::new(spacing)));

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

  /// Sets a static main axis alignment.
  #[inline(always)]
  pub fn set_main_axis_alignment(mut self, alignment: MainAxisAlignment) -> Self {
    self.main_axis_alignment = Some(Source::Static(alignment));

    self
  }

  /// Sets a derived main axis alignment.
  #[inline(always)]
  pub fn derive_main_axis_alignment(
    mut self,
    alignment: impl Fn() -> MainAxisAlignment + 'static,
  ) -> Self {
    self.main_axis_alignment = Some(Source::Derived(Rc::new(alignment)));

    self
  }

  /// Sets a static cross axis alignment.
  #[inline(always)]
  pub fn set_cross_axis_alignment(mut self, alignment: CrossAxisAlignment) -> Self {
    self.cross_axis_alignment = Some(Source::Static(alignment));

    self
  }

  /// Sets a derived cross axis alignment.
  #[inline(always)]
  pub fn derive_cross_axis_alignment(
    mut self,
    alignment: impl Fn() -> CrossAxisAlignment + 'static,
  ) -> Self {
    self.cross_axis_alignment = Some(Source::Derived(Rc::new(alignment)));

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
      self.padding,
      &mut style.padding,
      &mut binders,
      BoxProp::Padding,
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
      self.main_axis_alignment,
      &mut style.main_axis_alignment,
      &mut binders,
      BoxProp::MainAxisAlignment,
      update_box,
    );

    split_prop(
      self.cross_axis_alignment,
      &mut style.cross_axis_alignment,
      &mut binders,
      BoxProp::CrossAxisAlignment,
      update_box,
    );

    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Box {
          layout: self.layout,
          style,
          variants: self.variants,
        },
        box_taffy_style(self.layout, &style, false),
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
