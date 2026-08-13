//! Decoration and sizing around exactly one child, in cells.

use std::rc::Rc;

use skene_core::{
  Alignment,
  Color,
  Dimension,
  IntoChildOf,
  KeyEvent,
  Layout,
  MouseCursor,
  NodeId,
  Overflow,
  PlainChild,
  Prevent,
  Source,
  View,
};

use crate::{
  Children,
  box_node::BoxNode,
  events::{PointerEvent, ScrollEvent},
  props::{BoxStyle, Condition},
};

/// Decoration and sizing around exactly one child.
///
/// The vocabulary's plain box: it lays nothing out—the child fills the content area
/// when the container is sized, and the container hugs the child when it is not, with
/// [`set_alignment`](Self::set_alignment) placing a child that does neither. Several
/// children take a [`Row`](crate::Row), [`Column`](crate::Column), or another layout
/// container, which carries no decoration of its own: wrap it in one of these.
#[must_use]
pub struct Container {
  node: BoxNode,
}

impl Container {
  /// Creates an empty container.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      node: BoxNode::mode(Layout::Single),
    }
  }

  /// Sets the child.
  #[inline(always)]
  pub fn set_child(mut self, child: impl IntoChildOf<PlainChild>) -> Self {
    self.node.children = Children::new::<PlainChild>(vec![child.into_child()]);

    self
  }

  /// Sets a derived child, rebuilt whenever its reactive sources change.
  #[inline(always)]
  pub fn derive_child<V: IntoChildOf<PlainChild>>(
    mut self,
    child: impl Fn() -> V + 'static,
  ) -> Self {
    self.node.children =
      Children::derive::<PlainChild, _, _, _, _>(|| vec![()], |()| (), move |()| child());

    self
  }

  /// Sets how the child is placed when it does not fill the container.
  #[inline(always)]
  pub fn set_alignment(mut self, alignment: Alignment) -> Self {
    self.node.item_horizontal_alignment = Some(Source::Static(alignment.horizontal));
    self.node.item_alignment = Some(Source::Static(alignment.vertical));

    self
  }

  /// Sets a derived placement of the child.
  #[inline(always)]
  pub fn derive_alignment(mut self, alignment: impl Fn() -> Alignment + 'static) -> Self {
    let alignment = Rc::new(alignment);
    let horizontal = Rc::clone(&alignment);

    self.node.item_horizontal_alignment =
      Some(Source::Derived(Rc::new(move || horizontal().horizontal)));
    self.node.item_alignment = Some(Source::Derived(Rc::new(move || alignment().vertical)));

    self
  }

  /// Sets a static width: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_width(mut self, width: impl Into<Dimension<u32>>) -> Self {
    self.node.width = Some(Source::Static(width.into()));

    self
  }

  /// Sets a derived width.
  #[inline(always)]
  pub fn derive_width<D: Into<Dimension<u32>>>(mut self, width: impl Fn() -> D + 'static) -> Self {
    self.node.width = Some(Source::Derived(Rc::new(move || width().into())));

    self
  }

  /// Sets the width applied while `condition` holds.
  #[inline(always)]
  pub fn set_width_when(mut self, condition: Condition, width: impl Into<Dimension<u32>>) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_width(width)));

    self
  }

  /// Sets a static height: cells, a percentage, or auto.
  #[inline(always)]
  pub fn set_height(mut self, height: impl Into<Dimension<u32>>) -> Self {
    self.node.height = Some(Source::Static(height.into()));

    self
  }

  /// Sets a derived height.
  #[inline(always)]
  pub fn derive_height<D: Into<Dimension<u32>>>(
    mut self,
    height: impl Fn() -> D + 'static,
  ) -> Self {
    self.node.height = Some(Source::Derived(Rc::new(move || height().into())));

    self
  }

  /// Sets the height applied while `condition` holds.
  #[inline(always)]
  pub fn set_height_when(
    mut self,
    condition: Condition,
    height: impl Into<Dimension<u32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_height(height)));

    self
  }

  /// Sets a static minimum width.
  #[inline(always)]
  pub fn set_min_width(mut self, min_width: impl Into<Dimension<u32>>) -> Self {
    self.node.min_width = Some(Source::Static(min_width.into()));

    self
  }

  /// Sets a derived minimum width.
  #[inline(always)]
  pub fn derive_min_width<D: Into<Dimension<u32>>>(
    mut self,
    min_width: impl Fn() -> D + 'static,
  ) -> Self {
    self.node.min_width = Some(Source::Derived(Rc::new(move || min_width().into())));

    self
  }

  /// Sets the minimum width applied while `condition` holds.
  #[inline(always)]
  pub fn set_min_width_when(
    mut self,
    condition: Condition,
    min_width: impl Into<Dimension<u32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_min_width(min_width)));

    self
  }

  /// Sets a static minimum height.
  #[inline(always)]
  pub fn set_min_height(mut self, min_height: impl Into<Dimension<u32>>) -> Self {
    self.node.min_height = Some(Source::Static(min_height.into()));

    self
  }

  /// Sets a derived minimum height.
  #[inline(always)]
  pub fn derive_min_height<D: Into<Dimension<u32>>>(
    mut self,
    min_height: impl Fn() -> D + 'static,
  ) -> Self {
    self.node.min_height = Some(Source::Derived(Rc::new(move || min_height().into())));

    self
  }

  /// Sets the minimum height applied while `condition` holds.
  #[inline(always)]
  pub fn set_min_height_when(
    mut self,
    condition: Condition,
    min_height: impl Into<Dimension<u32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_min_height(min_height)));

    self
  }

  /// Sets a static maximum width.
  #[inline(always)]
  pub fn set_max_width(mut self, max_width: impl Into<Dimension<u32>>) -> Self {
    self.node.max_width = Some(Source::Static(max_width.into()));

    self
  }

  /// Sets a derived maximum width.
  #[inline(always)]
  pub fn derive_max_width<D: Into<Dimension<u32>>>(
    mut self,
    max_width: impl Fn() -> D + 'static,
  ) -> Self {
    self.node.max_width = Some(Source::Derived(Rc::new(move || max_width().into())));

    self
  }

  /// Sets the maximum width applied while `condition` holds.
  #[inline(always)]
  pub fn set_max_width_when(
    mut self,
    condition: Condition,
    max_width: impl Into<Dimension<u32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_max_width(max_width)));

    self
  }

  /// Sets a static maximum height.
  #[inline(always)]
  pub fn set_max_height(mut self, max_height: impl Into<Dimension<u32>>) -> Self {
    self.node.max_height = Some(Source::Static(max_height.into()));

    self
  }

  /// Sets a derived maximum height.
  #[inline(always)]
  pub fn derive_max_height<D: Into<Dimension<u32>>>(
    mut self,
    max_height: impl Fn() -> D + 'static,
  ) -> Self {
    self.node.max_height = Some(Source::Derived(Rc::new(move || max_height().into())));

    self
  }

  /// Sets the maximum height applied while `condition` holds.
  #[inline(always)]
  pub fn set_max_height_when(
    mut self,
    condition: Condition,
    max_height: impl Into<Dimension<u32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_max_height(max_height)));

    self
  }

  /// Sets a static width-to-height ratio the unsized axis follows.
  #[inline(always)]
  pub fn set_aspect_ratio(mut self, aspect_ratio: f32) -> Self {
    self.node.aspect_ratio = Some(Source::Static(aspect_ratio));

    self
  }

  /// Sets a derived width-to-height ratio.
  #[inline(always)]
  pub fn derive_aspect_ratio(mut self, aspect_ratio: impl Fn() -> f32 + 'static) -> Self {
    self.node.aspect_ratio = Some(Source::Derived(Rc::new(aspect_ratio)));

    self
  }

  /// Sets the ratio applied while `condition` holds.
  #[inline(always)]
  pub fn set_aspect_ratio_when(mut self, condition: Condition, aspect_ratio: f32) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_aspect_ratio(aspect_ratio)));

    self
  }

  /// Sets a static padding, in cells.
  #[inline(always)]
  pub fn set_padding(mut self, padding: crate::Inset) -> Self {
    self.node.padding = Some(Source::Static(padding));

    self
  }

  /// Sets a derived padding.
  #[inline(always)]
  pub fn derive_padding(mut self, padding: impl Fn() -> crate::Inset + 'static) -> Self {
    self.node.padding = Some(Source::Derived(Rc::new(padding)));

    self
  }

  /// Sets the padding applied while `condition` holds.
  #[inline(always)]
  pub fn set_padding_when(mut self, condition: Condition, padding: crate::Inset) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_padding(padding)));

    self
  }

  /// Sets a static margin on each edge.
  #[inline(always)]
  pub fn set_margin(mut self, margin: skene_core::Inset<Dimension<u32>>) -> Self {
    self.node.margin = Some(Source::Static(margin));

    self
  }

  /// Sets a derived margin.
  #[inline(always)]
  pub fn derive_margin(
    mut self,
    margin: impl Fn() -> skene_core::Inset<Dimension<u32>> + 'static,
  ) -> Self {
    self.node.margin = Some(Source::Derived(Rc::new(margin)));

    self
  }

  /// Sets the margin applied while `condition` holds.
  #[inline(always)]
  pub fn set_margin_when(
    mut self,
    condition: Condition,
    margin: skene_core::Inset<Dimension<u32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_margin(margin)));

    self
  }

  /// Sets a static in-flow shift from each edge.
  #[inline(always)]
  pub fn set_offset(mut self, offset: skene_core::Inset<Dimension<u32>>) -> Self {
    self.node.offset = Some(Source::Static(offset));

    self
  }

  /// Sets a derived in-flow shift.
  #[inline(always)]
  pub fn derive_offset(
    mut self,
    offset: impl Fn() -> skene_core::Inset<Dimension<u32>> + 'static,
  ) -> Self {
    self.node.offset = Some(Source::Derived(Rc::new(offset)));

    self
  }

  /// Sets the shift applied while `condition` holds.
  #[inline(always)]
  pub fn set_offset_when(
    mut self,
    condition: Condition,
    offset: skene_core::Inset<Dimension<u32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_offset(offset)));

    self
  }

  /// Sets a static background color.
  #[inline(always)]
  pub fn set_color(mut self, color: Color) -> Self {
    self.node.color = Some(Source::Static(color));

    self
  }

  /// Sets a derived background color.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.node.color = Some(Source::Derived(Rc::new(color)));

    self
  }

  /// Sets the background color applied while `condition` holds.
  #[inline(always)]
  pub fn set_color_when(mut self, condition: Condition, color: Color) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_color(color)));

    self
  }

  /// Sets a static one-cell border in the given color.
  #[inline(always)]
  pub fn set_border(mut self, border: Color) -> Self {
    self.node.border = Some(Source::Static(border));

    self
  }

  /// Sets a derived border color.
  #[inline(always)]
  pub fn derive_border(mut self, border: impl Fn() -> Color + 'static) -> Self {
    self.node.border = Some(Source::Derived(Rc::new(border)));

    self
  }

  /// Sets the border color applied while `condition` holds.
  #[inline(always)]
  pub fn set_border_when(mut self, condition: Condition, border: Color) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_border(border)));

    self
  }

  /// Sets a static border character set.
  #[inline(always)]
  pub fn set_border_glyphs(mut self, border_glyphs: crate::BorderGlyphs) -> Self {
    self.node.border_glyphs = Some(Source::Static(border_glyphs));

    self
  }

  /// Sets a derived border character set.
  #[inline(always)]
  pub fn derive_border_glyphs(
    mut self,
    border_glyphs: impl Fn() -> crate::BorderGlyphs + 'static,
  ) -> Self {
    self.node.border_glyphs = Some(Source::Derived(Rc::new(border_glyphs)));

    self
  }

  /// Sets the border characters applied while `condition` holds.
  #[inline(always)]
  pub fn set_border_glyphs_when(
    mut self,
    condition: Condition,
    border_glyphs: crate::BorderGlyphs,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_border_glyphs(border_glyphs)));

    self
  }

  /// Sets the shadow color, cast one cell down and right.
  #[inline(always)]
  pub fn set_shadow(mut self, shadow: Color) -> Self {
    self.node.shadow = Some(Source::Static(shadow));

    self
  }

  /// Sets a derived shadow color.
  #[inline(always)]
  pub fn derive_shadow(mut self, shadow: impl Fn() -> Color + 'static) -> Self {
    self.node.shadow = Some(Source::Derived(Rc::new(shadow)));

    self
  }

  /// Sets the shadow applied while `condition` holds.
  #[inline(always)]
  pub fn set_shadow_when(mut self, condition: Condition, shadow: Color) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_shadow(shadow)));

    self
  }

  /// Sets a static pointer shape, shown over the box in terminals that honor it.
  #[inline(always)]
  pub fn set_cursor(mut self, cursor: MouseCursor) -> Self {
    self.node.cursor = Some(Source::Static(cursor));

    self
  }

  /// Sets a derived pointer shape.
  #[inline(always)]
  pub fn derive_cursor(mut self, cursor: impl Fn() -> MouseCursor + 'static) -> Self {
    self.node.cursor = Some(Source::Derived(Rc::new(cursor)));

    self
  }

  /// Sets the pointer shape applied while `condition` holds.
  #[inline(always)]
  pub fn set_cursor_when(mut self, condition: Condition, cursor: MouseCursor) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_cursor(cursor)));

    self
  }

  /// Sets how content overflowing the box behaves; `Scroll` and `Auto` make it scrollable.
  #[inline(always)]
  pub fn set_overflow(mut self, overflow: Overflow) -> Self {
    self.node.overflow = Some(Source::Static(overflow));

    self
  }

  /// Sets a derived overflow behavior.
  #[inline(always)]
  pub fn derive_overflow(mut self, overflow: impl Fn() -> Overflow + 'static) -> Self {
    self.node.overflow = Some(Source::Derived(Rc::new(overflow)));

    self
  }

  /// Sets the overflow applied while `condition` holds.
  #[inline(always)]
  pub fn set_overflow_when(mut self, condition: Condition, overflow: Overflow) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_overflow(overflow)));

    self
  }

  /// Sets the click handler (`Enter` while focused, or a mouse click); its [`Prevent`]
  /// verdict is honored per the dispatch contract.
  #[inline(always)]
  pub fn handle_click<R: Into<Prevent>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self.node.click_handler = Some(Rc::new(move || action().into()));

    self
  }

  /// Sets the pointer-enter handler, run when the mouse moves onto the box.
  #[inline(always)]
  pub fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self.node.pointer_enter_handler = Some(Rc::new(action));

    self
  }

  /// Sets the pointer-leave handler, run when the mouse moves off the box.
  #[inline(always)]
  pub fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self.node.pointer_leave_handler = Some(Rc::new(action));

    self
  }

  /// Sets the focus handler, run when the box or a descendant gains focus.
  #[inline(always)]
  pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.node.focus_handler = Some(Rc::new(action));

    self
  }

  /// Sets the blur handler, run when the box or a descendant loses focus.
  #[inline(always)]
  pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.node.blur_handler = Some(Rc::new(action));

    self
  }

  /// Sets the key-press handler, run for keys pressed while the box or a descendant is
  /// focused; its [`Prevent`] verdict is honored per the dispatch contract.
  #[inline(always)]
  pub fn handle_key_down<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&KeyEvent) -> R + 'static,
  ) -> Self {
    self.node.key_down_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-down handler; its [`Prevent`] verdict is honored per the
  /// dispatch contract.
  #[inline(always)]
  pub fn handle_pointer_down<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.node.pointer_down_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-up handler; its [`Prevent`] verdict is honored per the dispatch
  /// contract.
  #[inline(always)]
  pub fn handle_pointer_up<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.node.pointer_up_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-move handler; its [`Prevent`] verdict is honored per the
  /// dispatch contract.
  #[inline(always)]
  pub fn handle_pointer_move<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.node.pointer_move_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the scroll handler, run with the new offsets when the box scrolls.
  #[inline(always)]
  pub fn handle_scroll(mut self, action: impl Fn(&ScrollEvent) + 'static) -> Self {
    self.node.scroll_handler = Some(Rc::new(action));

    self
  }

  /// Binds `anchor` to the box, so an overlay's
  /// [`set_anchor`](crate::Overlay::set_anchor) positions against it once built.
  #[inline(always)]
  pub fn bind_anchor(mut self, anchor: &crate::Anchor) -> Self {
    self.node.anchor = Some(anchor.clone());

    self
  }
}

impl Default for Container {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl View for Container {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.node.build()
  }
}
