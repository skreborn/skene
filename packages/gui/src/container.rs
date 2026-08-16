//! Decoration and sizing around exactly one child.

use std::rc::Rc;

use skene_core::{
  Alignment,
  Animation,
  Border,
  BoxStyle,
  Bubbling,
  Color,
  Condition,
  Dimension,
  IntoChildOf,
  KeyEvent,
  Layout,
  MouseCursor,
  NodeId,
  Overflow,
  PlainChild,
  PointerEvent,
  Role,
  ScrollEvent,
  Source,
  Transition,
  View,
};

use crate::{Children, Inset, box_node::BoxNode};

/// Decoration and sizing around exactly one child.
///
/// The vocabulary's plain box: it lays nothing out—the child fills the content area
/// when the container is sized, and the container hugs the child when it is not, with
/// [`set_alignment`](Self::set_alignment) placing a child that does neither. Several
/// children take a [`Row`](crate::Row), [`Column`](crate::Column), or another layout
/// container, which carries no decoration of its own: wrap it in one of these.
#[must_use]
pub struct Container {
  pub(crate) node: BoxNode,
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

  /// Sets a static width: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_width(mut self, width: impl Into<Dimension<f32>>) -> Self {
    self.node.width = Some(Source::Static(width.into()));

    self
  }

  /// Sets a derived width.
  #[inline(always)]
  pub fn derive_width<D: Into<Dimension<f32>>>(mut self, width: impl Fn() -> D + 'static) -> Self {
    self.node.width = Some(Source::Derived(Rc::new(move || width().into())));

    self
  }

  /// Sets the width applied while `condition` holds.
  #[inline(always)]
  pub fn set_width_when(mut self, condition: Condition, width: impl Into<Dimension<f32>>) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_width(width)));

    self
  }

  /// Sets a static height: logical pixels, a percentage, or auto.
  #[inline(always)]
  pub fn set_height(mut self, height: impl Into<Dimension<f32>>) -> Self {
    self.node.height = Some(Source::Static(height.into()));

    self
  }

  /// Sets a derived height.
  #[inline(always)]
  pub fn derive_height<D: Into<Dimension<f32>>>(
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
    height: impl Into<Dimension<f32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_height(height)));

    self
  }

  /// Sets a static minimum width.
  #[inline(always)]
  pub fn set_min_width(mut self, min_width: impl Into<Dimension<f32>>) -> Self {
    self.node.min_width = Some(Source::Static(min_width.into()));

    self
  }

  /// Sets a derived minimum width.
  #[inline(always)]
  pub fn derive_min_width<D: Into<Dimension<f32>>>(
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
    min_width: impl Into<Dimension<f32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_min_width(min_width)));

    self
  }

  /// Sets a static minimum height.
  #[inline(always)]
  pub fn set_min_height(mut self, min_height: impl Into<Dimension<f32>>) -> Self {
    self.node.min_height = Some(Source::Static(min_height.into()));

    self
  }

  /// Sets a derived minimum height.
  #[inline(always)]
  pub fn derive_min_height<D: Into<Dimension<f32>>>(
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
    min_height: impl Into<Dimension<f32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_min_height(min_height)));

    self
  }

  /// Sets a static maximum width.
  #[inline(always)]
  pub fn set_max_width(mut self, max_width: impl Into<Dimension<f32>>) -> Self {
    self.node.max_width = Some(Source::Static(max_width.into()));

    self
  }

  /// Sets a derived maximum width.
  #[inline(always)]
  pub fn derive_max_width<D: Into<Dimension<f32>>>(
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
    max_width: impl Into<Dimension<f32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_max_width(max_width)));

    self
  }

  /// Sets a static maximum height.
  #[inline(always)]
  pub fn set_max_height(mut self, max_height: impl Into<Dimension<f32>>) -> Self {
    self.node.max_height = Some(Source::Static(max_height.into()));

    self
  }

  /// Sets a derived maximum height.
  #[inline(always)]
  pub fn derive_max_height<D: Into<Dimension<f32>>>(
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
    max_height: impl Into<Dimension<f32>>,
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

  /// Sets a static padding.
  #[inline(always)]
  pub fn set_padding(mut self, padding: Inset) -> Self {
    self.node.padding = Some(Source::Static(padding));

    self
  }

  /// Sets a derived padding.
  #[inline(always)]
  pub fn derive_padding(mut self, padding: impl Fn() -> Inset + 'static) -> Self {
    self.node.padding = Some(Source::Derived(Rc::new(padding)));

    self
  }

  /// Sets the padding applied while `condition` holds.
  #[inline(always)]
  pub fn set_padding_when(mut self, condition: Condition, padding: Inset) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_padding(padding)));

    self
  }

  /// Sets a static margin on each edge.
  #[inline(always)]
  pub fn set_margin(mut self, margin: skene_core::Inset<Dimension<f32>>) -> Self {
    self.node.margin = Some(Source::Static(margin));

    self
  }

  /// Sets a derived margin.
  #[inline(always)]
  pub fn derive_margin(
    mut self,
    margin: impl Fn() -> skene_core::Inset<Dimension<f32>> + 'static,
  ) -> Self {
    self.node.margin = Some(Source::Derived(Rc::new(margin)));

    self
  }

  /// Sets the margin applied while `condition` holds.
  #[inline(always)]
  pub fn set_margin_when(
    mut self,
    condition: Condition,
    margin: skene_core::Inset<Dimension<f32>>,
  ) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_margin(margin)));

    self
  }

  /// Sets a static in-flow shift from each edge.
  #[inline(always)]
  pub fn set_offset(mut self, offset: skene_core::Inset<Dimension<f32>>) -> Self {
    self.node.offset = Some(Source::Static(offset));

    self
  }

  /// Sets a derived in-flow shift.
  #[inline(always)]
  pub fn derive_offset(
    mut self,
    offset: impl Fn() -> skene_core::Inset<Dimension<f32>> + 'static,
  ) -> Self {
    self.node.offset = Some(Source::Derived(Rc::new(offset)));

    self
  }

  /// Sets the shift applied while `condition` holds.
  #[inline(always)]
  pub fn set_offset_when(
    mut self,
    condition: Condition,
    offset: skene_core::Inset<Dimension<f32>>,
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

  /// Sets a static border.
  #[inline(always)]
  pub fn set_border(mut self, border: Border) -> Self {
    self.node.border = Some(Source::Static(border));

    self
  }

  /// Sets a derived border.
  #[inline(always)]
  pub fn derive_border(mut self, border: impl Fn() -> Border + 'static) -> Self {
    self.node.border = Some(Source::Derived(Rc::new(border)));

    self
  }

  /// Sets the border applied while `condition` holds.
  #[inline(always)]
  pub fn set_border_when(mut self, condition: Condition, border: Border) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_border(border)));

    self
  }

  /// Sets a static border radius.
  #[inline(always)]
  pub fn set_border_radius(mut self, border_radius: f32) -> Self {
    self.node.border_radius = Some(Source::Static(border_radius));

    self
  }

  /// Sets a derived border radius.
  #[inline(always)]
  pub fn derive_border_radius(mut self, border_radius: impl Fn() -> f32 + 'static) -> Self {
    self.node.border_radius = Some(Source::Derived(Rc::new(border_radius)));

    self
  }

  /// Sets the border radius applied while `condition` holds.
  #[inline(always)]
  pub fn set_border_radius_when(mut self, condition: Condition, border_radius: f32) -> Self {
    self
      .node
      .variants
      .push((condition, BoxStyle::new().set_border_radius(border_radius)));

    self
  }

  /// Sets a static mouse cursor.
  #[inline(always)]
  pub fn set_cursor(mut self, cursor: MouseCursor) -> Self {
    self.node.cursor = Some(Source::Static(cursor));

    self
  }

  /// Sets a derived mouse cursor.
  #[inline(always)]
  pub fn derive_cursor(mut self, cursor: impl Fn() -> MouseCursor + 'static) -> Self {
    self.node.cursor = Some(Source::Derived(Rc::new(cursor)));

    self
  }

  /// Sets the mouse cursor applied while `condition` holds.
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

  /// Sets what this widget reports to assistive technology.
  ///
  /// Pairs with a widget [`Role`]: the role says what the control is, these say what it
  /// currently reports. A driver maps them to its own accessibility layer.
  #[inline(always)]
  pub fn set_accessibility(
    mut self,
    state: impl IntoIterator<Item = skene_core::AccessProp>,
  ) -> Self {
    self.node.accessibility = Some(Source::Static(state.into_iter().collect()));

    self
  }

  /// Derives what this widget reports, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_accessibility(
    mut self,
    state: impl Fn() -> Vec<skene_core::AccessProp> + 'static,
  ) -> Self {
    self.node.accessibility = Some(Source::Derived(Rc::new(state)));

    self
  }

  /// Makes the box take focus, at the given place in sequential traversal.
  ///
  /// A plain box does not take focus—only controls do. A view drawing its own control
  /// sets this to join the tab order the way a native control would, and pairs it with
  /// a widget [`Role`] so what lands there is announced.
  #[inline(always)]
  pub fn set_focus_order(mut self, focus_order: skene_core::FocusOrder) -> Self {
    self.node.focus_order = Some(Source::Static(focus_order));

    self
  }

  /// Derives where the box sits in sequential focus traversal.
  ///
  /// A group of controls sharing one tab stop moves it to whichever member is current,
  /// the way a native radio group does.
  #[inline(always)]
  pub fn derive_focus_order(
    mut self,
    focus_order: impl Fn() -> skene_core::FocusOrder + 'static,
  ) -> Self {
    self.node.focus_order = Some(Source::Derived(Rc::new(focus_order)));

    self
  }

  /// Sets the semantic role.
  #[inline(always)]
  pub fn set_role(mut self, role: Role) -> Self {
    self.node.role = Some(role);

    self
  }

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  pub fn set_transition(mut self, transition: Transition) -> Self {
    self.node.transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(mut self, animation: Animation<BoxStyle>) -> Self {
    self.node.animations.push(animation);

    self
  }

  /// Sets the click handler; its [`Bubbling`] verdict is honored per the dispatch
  /// contract.
  #[inline(always)]
  pub fn handle_click<R: Into<Bubbling>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self.node.click_handler = Some(Rc::new(move || action().into().into()));

    self
  }

  /// Sets the pointer-enter handler.
  #[inline(always)]
  pub fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self.node.pointer_enter_handler = Some(Rc::new(action));

    self
  }

  /// Sets the pointer-leave handler.
  #[inline(always)]
  pub fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self.node.pointer_leave_handler = Some(Rc::new(action));

    self
  }

  /// Sets the focus handler, run when the box or a descendant gains focus.
  ///
  /// A box never takes focus itself—only controls do; this observes focus within it.
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

  /// Sets the key-press handler, run for keys pressed on the box or within it; its
  /// [`Bubbling`] verdict is honored per the dispatch contract.
  #[inline(always)]
  pub fn handle_key_down<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&KeyEvent) -> R + 'static,
  ) -> Self {
    self.node.key_down_handler = Some(Rc::new(move |event| action(event).into().into()));

    self
  }

  /// Sets the pointer-down handler; its [`Bubbling`] verdict is honored per the dispatch
  /// contract.
  #[inline(always)]
  pub fn handle_pointer_down<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.node.pointer_down_handler = Some(Rc::new(move |event| action(event).into().into()));

    self
  }

  /// Sets the pointer-up handler; its [`Bubbling`] verdict is honored per the dispatch
  /// contract.
  #[inline(always)]
  pub fn handle_pointer_up<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.node.pointer_up_handler = Some(Rc::new(move |event| action(event).into().into()));

    self
  }

  /// Sets the pointer-move handler; its [`Bubbling`] verdict is honored per the dispatch
  /// contract.
  #[inline(always)]
  pub fn handle_pointer_move<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.node.pointer_move_handler = Some(Rc::new(move |event| action(event).into().into()));

    self
  }

  /// Sets the scroll handler, run with the new offsets when the box scrolls.
  ///
  /// The box scrolls only with an overflow that allows it; see
  /// [`set_overflow`](Self::set_overflow).
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

  /// Binds `handle` to the box, so [`FocusHandle::focus`](crate::FocusHandle::focus)
  /// targets it once built.
  ///
  /// Only a box that takes focus—one carrying a widget [role](Self::set_role) and a
  /// [focus order](Self::set_focus_order)—can receive it.
  #[inline(always)]
  pub fn bind_focus(mut self, handle: &crate::FocusHandle) -> Self {
    self.node.focus_handle = Some(handle.clone());

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
