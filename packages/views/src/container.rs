//! The one decorated box.

use skene_core::{
  AccessProp,
  Alignment,
  Animation,
  Bubbling,
  Color,
  Dimension,
  FocusOrder,
  Inset,
  KeyEvent,
  Layout,
  MouseCursor,
  Overflow,
  PointerEvent,
  Role,
  ScrollEvent,
  Transition,
  next::{
    AnyKind,
    Border,
    BoxStyle,
    Children,
    Condition,
    ContainerName,
    DeclaredBoxStyle,
    Driver,
    HasChildren,
    NodeId,
    Plain,
    PlainChild,
    Seal,
    Value,
    View,
  },
};

use crate::{LayoutBox, macros::properties};

/// Decoration and sizing around exactly one child.
///
/// The freely stylable box: decoration, sizing, events, alignment and a single child,
/// set through [`HasChildren::set_child`] or [`HasChildren::derive_child`]. The child
/// fills the container when the container is sized and the container hugs the child
/// when it is not; [`set_alignment`](Self::set_alignment) places a child that does
/// neither. Several children take a layout view, wrapped in a container to decorate it.
#[must_use]
pub struct Container<D: Driver> {
  inner: LayoutBox<D, PlainChild>,
}

impl<D: Driver> Default for Container<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> Container<D> {
  /// An undecorated container.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      inner: LayoutBox::new(Layout::Single),
    }
  }

  #[inline(always)]
  const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<D> {
    self.inner.style_mut()
  }

  properties! { style_mut;
    set_width / derive_width (width) => width: Dimension<D::Scalar>,
      "the width";
    set_height / derive_height (height) => height: Dimension<D::Scalar>,
      "the height";
    set_min_width / derive_min_width (min_width) => min_width: Dimension<D::Scalar>,
      "the minimum width";
    set_min_height / derive_min_height (min_height) => min_height: Dimension<D::Scalar>,
      "the minimum height";
    set_max_width / derive_max_width (max_width) => max_width: Dimension<D::Scalar>,
      "the maximum width";
    set_max_height / derive_max_height (max_height) => max_height: Dimension<D::Scalar>,
      "the maximum height";
    set_aspect_ratio / derive_aspect_ratio (aspect_ratio) => aspect_ratio: f32,
      "the width-to-height ratio the unsized axis follows";
    set_padding / derive_padding (padding) => padding: Inset<D::Scalar>,
      "the padding on each edge";
    set_margin / derive_margin (margin) => margin: Inset<Dimension<D::Scalar>>,
      "the margin on each edge";
    set_offset / derive_offset (offset) => offset: Inset<Dimension<D::Scalar>>,
      "the shift from each edge, keeping the box in flow";
    set_color / derive_color (color) => color: Color,
      "the background color";
    set_border / derive_border (border) => border: Border<D::Scalar>,
      "the border";
    set_cursor / derive_cursor (cursor) => cursor: MouseCursor,
      "the mouse cursor shown over the box";
    set_overflow / derive_overflow (overflow) => overflow: Overflow,
      "how content exceeding the box's bounds is treated; `Scroll` and `Auto` make it scrollable";
  }

  /// Sets where the child sits when it fills neither axis.
  #[inline(always)]
  pub fn set_alignment(
    mut self,
    alignment: impl Into<Value<Alignment, Condition<D::Scalar>>>,
  ) -> Self {
    self.inner = self.inner.set_alignment(alignment);

    self
  }

  /// Derives where the child sits from the signals `alignment` reads.
  #[inline(always)]
  pub fn derive_alignment(mut self, alignment: impl Fn() -> Alignment + 'static) -> Self {
    self.inner = self.inner.derive_alignment(alignment);

    self
  }

  /// Names the container for container queries against its size.
  #[inline(always)]
  pub fn set_container_name(mut self, name: ContainerName) -> Self {
    self.inner = self.inner.set_container_name(name);

    self
  }

  /// Sets the semantic role, guiding the driver's concrete choice of element.
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

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  pub fn set_transition(mut self, transition: Transition) -> Self {
    self.inner = self.inner.set_transition(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(
    mut self,
    animation: Animation<BoxStyle<D::Scalar, Plain, D::StyleExt>>,
  ) -> Self {
    self.inner = self.inner.set_animation(animation);

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

impl<D: Driver> HasChildren<D> for Container<D> {
  type Kind = PlainChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<D, PlainChild> {
    self.inner.children_mut(seal)
  }
}

impl<D: Driver> AnyKind for Container<D> {}

impl<D: Driver> View<D> for Container<D> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.inner.build()
  }
}
