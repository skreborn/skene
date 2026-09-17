//! The one decorated box.

use skene_core::{
  AccessProp,
  Alignment,
  Animation,
  AnyKind,
  BoxKeyframe,
  Bubbling,
  Children,
  Condition,
  ContainerName,
  DeclaredBoxStyle,
  FocusOrder,
  HasChildren,
  KeyEvent,
  Layout,
  NodeId,
  PlainChild,
  Platform,
  PointerEvent,
  Role,
  ScrollEvent,
  Seal,
  StaticLeaf,
  StyleExtension,
  Transition,
  Value,
  View,
};

use crate::{Anchor, FocusHandle, LayoutBox, SizeHandle, decoration};

/// Decoration and sizing around exactly one child.
///
/// The freely stylable box: decoration, sizing, events, alignment and a single child,
/// set through [`HasChildren::set_child`] or [`HasChildren::derive_child`]. The child
/// fills the container when the container is sized and the container hugs the child
/// when it is not; [`set_alignment`](Self::set_alignment) places a child that does
/// neither. Several children take a layout view, wrapped in a container to decorate it.
#[must_use]
pub struct Container<P: Platform> {
  inner: LayoutBox<P, PlainChild>,
}

impl<P: Platform> Default for Container<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Container<P> {
  /// An undecorated container.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      inner: LayoutBox::new(Layout::Single),
    }
  }

  /// The declared style, for a facade's medium-specific setters to write into.
  #[inline(always)]
  pub const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.inner.style_mut()
  }

  decoration! { style_mut }

  /// Sets where the child sits when it fills neither axis.
  #[inline(always)]
  pub fn set_alignment(
    mut self,
    alignment: impl Into<Value<Alignment, Condition<P::Scalar>>>,
  ) -> Self {
    self.inner = self.inner.set_alignment(alignment);

    self
  }

  /// Derives where the child sits from the signals `alignment` reads.
  #[inline(always)]
  pub fn derive_alignment<R: Into<StaticLeaf<Alignment>>>(
    mut self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.inner = self.inner.derive_alignment(alignment);

    self
  }

  /// Names the container for container queries against its size.
  #[inline(always)]
  pub fn set_container_name(mut self, name: ContainerName) -> Self {
    self.inner = self.inner.set_container_name(name);

    self
  }

  /// Sets the semantic role, guiding the platform's concrete choice of element.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.inner = self.inner.set_label(label);

    self
  }

  /// Sets the node whose text names the box: a region's heading.
  #[inline(always)]
  pub fn set_labelled_by(mut self, node: NodeId) -> Self {
    self.inner = self.inner.set_labelled_by(node);

    self
  }

  /// Sets the semantic role, guiding the platform's choice of element.
  #[inline(always)]
  pub fn set_role(mut self, role: Role) -> Self {
    self.inner = self.inner.set_role(role);

    self
  }

  /// Sets the box's place in sequential focus traversal; a box without one takes no
  /// focus.
  #[inline(always)]
  pub fn set_focus_order(mut self, focus_order: FocusOrder) -> Self {
    self.inner = self.inner.set_focus_order(focus_order);

    self
  }

  /// Derives the box's place in sequential focus traversal from the signals
  /// `focus_order` reads.
  #[inline(always)]
  pub fn derive_focus_order(mut self, focus_order: impl Fn() -> FocusOrder + 'static) -> Self {
    self.inner = self.inner.derive_focus_order(focus_order);

    self
  }

  /// Sets what the box reports to assistive technology, beside its role.
  #[inline(always)]
  pub fn set_accessibility(mut self, access: impl IntoIterator<Item = AccessProp>) -> Self {
    self.inner = self.inner.set_accessibility(access);

    self
  }

  /// Derives what the box reports to assistive technology from the signals `access`
  /// reads.
  #[inline(always)]
  pub fn derive_accessibility(mut self, access: impl Fn() -> Vec<AccessProp> + 'static) -> Self {
    self.inner = self.inner.derive_accessibility(access);

    self
  }

  /// Binds `anchor` to the box once built, so an overlay is placed against it.
  #[inline(always)]
  pub fn bind_anchor(mut self, anchor: &Anchor) -> Self {
    self.inner = self.inner.bind_anchor(anchor);

    self
  }

  /// Binds `focus` to the box once built, so focus moves to it programmatically; the
  /// box takes focus only with a focus order.
  #[inline(always)]
  pub fn bind_focus(mut self, focus: &FocusHandle<P>) -> Self {
    self.inner = self.inner.bind_focus(focus);

    self
  }

  /// Binds `size` to the box once built, so its measured size is reported through it.
  #[inline(always)]
  pub fn bind_size(mut self, size: &SizeHandle<P>) -> Self {
    self.inner = self.inner.bind_size(size);

    self
  }

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  pub fn set_transition(
    mut self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.inner = self.inner.set_transition(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(mut self, animation: Animation<BoxKeyframe<P>>) -> Self {
    self.inner = self.inner.set_animation(animation);

    self
  }

  /// Names the box for view transitions: it is paired with the box of the same name
  /// across a navigation, where the platform animates between them.
  #[inline(always)]
  pub fn set_transition_name(mut self, name: impl Into<String>) -> Self {
    self.inner = self.inner.set_transition_name(name);

    self
  }

  /// Runs `action` on a click; return [`Bubbling::Stop`] to keep the click from the
  /// ancestors.
  #[inline(always)]
  pub fn handle_click<R: Into<Bubbling>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self.inner = self.inner.handle_click(action);

    self
  }

  /// Runs `action` on a key pressed while the box or a descendant holds focus; return
  /// [`Bubbling::Stop`] to keep the key from the ancestors.
  #[inline(always)]
  pub fn handle_key_down<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&KeyEvent) -> R + 'static,
  ) -> Self {
    self.inner = self.inner.handle_key_down(action);

    self
  }

  /// Runs `action` on a key released while the box or a descendant holds focus;
  /// return [`Bubbling::Stop`] to keep the release from the ancestors.
  #[inline(always)]
  pub fn handle_key_up<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&KeyEvent) -> R + 'static,
  ) -> Self {
    self.inner = self.inner.handle_key_up(action);

    self
  }

  /// Runs `action` on a pointer button pressed over the box; return
  /// [`Bubbling::Stop`] to keep the press from the ancestors.
  #[inline(always)]
  pub fn handle_pointer_down<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.inner = self.inner.handle_pointer_down(action);

    self
  }

  /// Runs `action` on a pointer button released over the box; return
  /// [`Bubbling::Stop`] to keep the release from the ancestors.
  #[inline(always)]
  pub fn handle_pointer_up<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.inner = self.inner.handle_pointer_up(action);

    self
  }

  /// Runs `action` on the pointer moving over the box; return [`Bubbling::Stop`] to
  /// keep the move from the ancestors.
  #[inline(always)]
  pub fn handle_pointer_move<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.inner = self.inner.handle_pointer_move(action);

    self
  }

  /// Runs `action` when the pointer enters the box.
  #[inline(always)]
  pub fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self.inner = self.inner.handle_pointer_enter(action);

    self
  }

  /// Runs `action` when the pointer leaves the box.
  #[inline(always)]
  pub fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self.inner = self.inner.handle_pointer_leave(action);

    self
  }

  /// Runs `action` when the box gains focus.
  #[inline(always)]
  pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.inner = self.inner.handle_focus(action);

    self
  }

  /// Runs `action` when the box loses focus.
  #[inline(always)]
  pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.inner = self.inner.handle_blur(action);

    self
  }

  /// Runs `action` with the box's new offsets when it scrolls; the box scrolls only
  /// with an overflow that allows it.
  #[inline(always)]
  pub fn handle_scroll(mut self, action: impl Fn(&ScrollEvent) + 'static) -> Self {
    self.inner = self.inner.handle_scroll(action);

    self
  }
}

impl<P: Platform> HasChildren<P> for Container<P> {
  type Kind = PlainChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<P, PlainChild> {
    self.inner.children_mut(seal)
  }
}

impl<P: Platform> AnyKind for Container<P> {}

impl<P: Platform> View<P> for Container<P> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.inner.build()
  }
}
