//! The one decorated box.

use crate::{
  AccessProp,
  Alignment,
  AnyKind,
  Bubbling,
  Children,
  Condition,
  ContainerName,
  DeclaredBoxStyle,
  DecorationBase,
  DragAxes,
  DragData,
  FocusOrder,
  HasBoxStyle,
  HasChild,
  HasChildren,
  HasLayoutBox,
  KeyEvent,
  Layout,
  LayoutBox,
  MotionBase,
  NodeHandle,
  NodeId,
  PlainChild,
  PointerEvent,
  Realization,
  Role,
  ScrollEvent,
  Seal,
  StaticLeaf,
  Value,
  View,
  WheelEvent,
  Wraps,
};

/// Decoration and sizing around exactly one child.
///
/// The freely stylable box: decoration, sizing, events, alignment and a single child,
/// set through [`HasChild::set_child`] or [`HasChild::derive_child`]. The child
/// fills the container when the container is sized and the container hugs the child
/// when it is not; [`set_alignment`](Self::set_alignment) places a child that does
/// neither. Several children take a layout view, wrapped in a container to decorate it.
#[derive(Debug)]
#[must_use]
pub struct Container<P: Realization> {
  inner: LayoutBox<P, PlainChild>,
}

impl<P: Realization> Default for Container<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Container<P> {
  /// An undecorated container.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      inner: LayoutBox::new(Layout::Single),
    }
  }
}

impl<P: Realization> HasChild<P> for Container<P> {
  type Kind = PlainChild;

  #[inline(always)]
  fn child_mut(&mut self, seal: Seal) -> &mut Children<P, PlainChild> {
    self.inner.children_mut(seal)
  }
}

impl<P: Realization> AnyKind for Container<P> {}

impl<P: Realization> View<P> for Container<P> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.inner.build()
  }
}

impl<P: Realization> HasBoxStyle<P> for Container<P> {
  #[inline(always)]
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.inner.style_mut()
  }
}

impl<P: Realization> HasLayoutBox<P> for Container<P> {
  type Kind = PlainChild;

  #[inline(always)]
  fn map_layout_box(
    self,
    map: impl FnOnce(LayoutBox<P, PlainChild>) -> LayoutBox<P, PlainChild>,
  ) -> Self {
    Self {
      inner: map(self.inner),
    }
  }
}

impl<P: Realization> DecorationBase<P> for Container<P> {}

impl<P: Realization> MotionBase<P> for Container<P> {}

impl<P: Realization> ContainerBase<P> for Container<P> {}

/// What a [`Container`] carries beyond its decoration and its motion.
///
/// Its constructor, where its child sits, what names it, its place in focus traversal,
/// what it reports to assistive technology, its handle and its handlers.
pub trait ContainerBase<P: Realization>: HasLayoutBox<P, Kind = PlainChild> {
  /// An undecorated container.
  #[must_use]
  fn new() -> Self
  where
    Self: Wraps<Inner = Container<P>>,
  {
    Self::wrap(Container::new())
  }

  /// Sets where the child sits when it fills neither axis.
  #[must_use]
  fn set_alignment(self, alignment: impl Into<Value<Alignment, Condition<P::Scalar>>>) -> Self {
    self.map_layout_box(|layout| layout.set_alignment(alignment))
  }

  /// Derives where the child sits from the signals `alignment` reads.
  #[must_use]
  fn derive_alignment<R: Into<StaticLeaf<Alignment>>>(
    self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.map_layout_box(|layout| layout.derive_alignment(alignment))
  }

  /// Names the container for container queries against its size.
  #[must_use]
  fn set_container_name(self, name: ContainerName) -> Self {
    self.map_layout_box(|layout| layout.set_container_name(name))
  }

  /// Declares the axes a press's pointer drags the box along: the press's moves and
  /// its release reach the box wherever the pointer goes, and a touch pans only along
  /// the others.
  #[must_use]
  fn set_drag_axes(self, axes: DragAxes) -> Self {
    self.map_layout_box(|layout| layout.set_drag_axes(axes))
  }

  /// Sets the box's accessible name, for a region, a dialog or a picture that no visible
  /// text names.
  #[must_use]
  fn set_label(self, label: impl Into<String>) -> Self {
    self.map_layout_box(|layout| layout.set_label(label))
  }

  /// Sets the node whose text names the box: a region's heading.
  #[must_use]
  fn set_labelled_by(self, node: NodeId) -> Self {
    self.map_layout_box(|layout| layout.set_labelled_by(node))
  }

  /// Sets the semantic role, guiding the platform's choice of element.
  #[must_use]
  fn set_role(self, role: Role) -> Self {
    self.map_layout_box(|layout| layout.set_role(role))
  }

  /// Sets the box's place in sequential focus traversal; a box without one takes no
  /// focus.
  #[must_use]
  fn set_focus_order(self, focus_order: FocusOrder) -> Self {
    self.map_layout_box(|layout| layout.set_focus_order(focus_order))
  }

  /// Derives the box's place in sequential focus traversal from the signals
  /// `focus_order` reads.
  #[must_use]
  fn derive_focus_order(self, focus_order: impl Fn() -> FocusOrder + 'static) -> Self {
    self.map_layout_box(|layout| layout.derive_focus_order(focus_order))
  }

  /// Sets what the box reports to assistive technology, beside its role.
  #[must_use]
  fn set_accessibility(self, access: impl IntoIterator<Item = AccessProp>) -> Self {
    self.map_layout_box(|layout| layout.set_accessibility(access))
  }

  /// Derives what the box reports to assistive technology from the signals `access`
  /// reads.
  #[must_use]
  fn derive_accessibility(self, access: impl Fn() -> Vec<AccessProp> + 'static) -> Self {
    self.map_layout_box(|layout| layout.derive_accessibility(access))
  }

  /// Binds `handle` to the box once built: an overlay is placed against it, focus
  /// moves to it, which the box takes only with a focus order, and its measured size
  /// is reported through the handle.
  #[must_use]
  fn bind_handle(self, handle: NodeHandle<P>) -> Self {
    self.map_layout_box(|layout| layout.bind_handle(handle))
  }

  /// Runs `action` on a click; return [`Bubbling::Stop`](Bubbling::Stop) to keep the click
  /// from the ancestors.
  ///
  /// A box that handles a click is an activatable row, not a button: a tab stop unless
  /// its focus order says otherwise, activated by `Enter` on every press and by `Space`
  /// on its release, as a click activates it, and it may hold controls, which own their
  /// presses and keys. It carries no role but the one [`set_role`](Self::set_role)
  /// gives it.
  #[must_use]
  fn handle_click<R: Into<Bubbling>>(self, action: impl Fn() -> R + 'static) -> Self {
    self.map_layout_box(|layout| layout.handle_click(action))
  }

  /// Runs `action` on the second click of a double click, after the click itself;
  /// return [`Bubbling::Stop`](Bubbling::Stop) to keep the double click from the ancestors.
  #[must_use]
  fn handle_double_click<R: Into<Bubbling>>(self, action: impl Fn() -> R + 'static) -> Self {
    self.map_layout_box(|layout| layout.handle_double_click(action))
  }

  /// Runs `action` on a key pressed while the box or a descendant holds focus; return
  /// [`Bubbling::Stop`](Bubbling::Stop) to keep the key from the ancestors.
  #[must_use]
  fn handle_key_down<R: Into<Bubbling>>(self, action: impl Fn(&KeyEvent) -> R + 'static) -> Self {
    self.map_layout_box(|layout| layout.handle_key_down(action))
  }

  /// Runs `action` on a key released while the box or a descendant holds focus;
  /// return [`Bubbling::Stop`](Bubbling::Stop) to keep the release from the ancestors.
  #[must_use]
  fn handle_key_up<R: Into<Bubbling>>(self, action: impl Fn(&KeyEvent) -> R + 'static) -> Self {
    self.map_layout_box(|layout| layout.handle_key_up(action))
  }

  /// Runs `action` on a pointer button pressed over the box; return
  /// [`Bubbling::Stop`](Bubbling::Stop) to keep the press from the ancestors.
  #[must_use]
  fn handle_pointer_down<R: Into<Bubbling>>(
    self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.map_layout_box(|layout| layout.handle_pointer_down(action))
  }

  /// Runs `action` on a pointer button released over the box; return
  /// [`Bubbling::Stop`](Bubbling::Stop) to keep the release from the ancestors.
  #[must_use]
  fn handle_pointer_up<R: Into<Bubbling>>(
    self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.map_layout_box(|layout| layout.handle_pointer_up(action))
  }

  /// Runs `action` on the pointer moving over the box; return
  /// [`Bubbling::Stop`](Bubbling::Stop) to keep the move from the ancestors.
  #[must_use]
  fn handle_pointer_move<R: Into<Bubbling>>(
    self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.map_layout_box(|layout| layout.handle_pointer_move(action))
  }

  /// Offers what a drag from the box carries, `data` building it as the drag starts;
  /// the box holds [`State::Dragged`](crate::State::Dragged) while the drag lasts.
  #[must_use]
  fn set_drag_data(self, data: impl Fn() -> DragData + 'static) -> Self {
    self.map_layout_box(|layout| layout.set_drag_data(data))
  }

  /// Takes a drag carrying a `T` dropped on the box, running `action` with the value;
  /// the box holds [`State::Targeted`](crate::State::Targeted) while such a drag
  /// is over it.
  #[must_use]
  fn handle_drop<T: 'static>(self, action: impl Fn(&T) + 'static) -> Self {
    self.map_layout_box(|layout| layout.handle_drop(action))
  }

  /// Runs `action` on a wheel turned over the box, before the platform scrolls by it;
  /// return [`Bubbling::Stop`](Bubbling::Stop) to keep the turn from the ancestors.
  #[must_use]
  fn handle_wheel<R: Into<Bubbling>>(self, action: impl Fn(&WheelEvent) -> R + 'static) -> Self {
    self.map_layout_box(|layout| layout.handle_wheel(action))
  }

  /// Runs `action` when the platform takes a pointer pressed over the box for itself,
  /// ending the press without a release; return [`Bubbling::Stop`](Bubbling::Stop) to keep
  /// the cancel from the ancestors.
  #[must_use]
  fn handle_pointer_cancel<R: Into<Bubbling>>(
    self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.map_layout_box(|layout| layout.handle_pointer_cancel(action))
  }

  /// Runs `action` when the pointer enters the box.
  #[must_use]
  fn handle_pointer_enter(self, action: impl Fn() + 'static) -> Self {
    self.map_layout_box(|layout| layout.handle_pointer_enter(action))
  }

  /// Runs `action` when the pointer leaves the box.
  #[must_use]
  fn handle_pointer_leave(self, action: impl Fn() + 'static) -> Self {
    self.map_layout_box(|layout| layout.handle_pointer_leave(action))
  }

  /// Runs `action` when focus enters the box or a node inside it from outside.
  #[must_use]
  fn handle_focus(self, action: impl Fn() + 'static) -> Self {
    self.map_layout_box(|layout| layout.handle_focus(action))
  }

  /// Runs `action` when focus leaves the box and every node inside it.
  #[must_use]
  fn handle_blur(self, action: impl Fn() + 'static) -> Self {
    self.map_layout_box(|layout| layout.handle_blur(action))
  }

  /// Runs `action` with the box's new offsets when it scrolls; the box scrolls only
  /// with an overflow that allows it.
  #[must_use]
  fn handle_scroll(self, action: impl Fn(&ScrollEvent) + 'static) -> Self {
    self.map_layout_box(|layout| layout.handle_scroll(action))
  }
}
