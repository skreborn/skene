//! The one decorated box.

use crate::{
  AnyKind,
  Children,
  HasChild,
  HasChildren,
  Layout,
  LayoutBox,
  NodeId,
  PlainChild,
  Platform,
  Seal,
  View,
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
pub struct Container<P: Platform> {
  inner: LayoutBox<P, PlainChild>,
}

impl<P: Platform> Default for Container<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

/// Generates a [`Container`]'s builder surface on a type holding a box in `$field`.
///
/// The surface is the declared style, the decoration and sizing, the alignment, the
/// accessibility, the motion and the handlers, on the platform `$platform`. The
/// foundation's container expands it over its box, and a facade's newtype over the
/// container it wraps.
#[macro_export]
macro_rules! container_surface {
  ($field:tt, $platform:ty) => {
    /// The declared style, for a facade's medium-specific setters to write into.
    #[inline(always)]
    pub const fn style_mut(&mut self) -> &mut $crate::DeclaredBoxStyle<$platform> {
      self.$field.style_mut()
    }

    $crate::decoration! { style_mut, $platform }

    $crate::forward! { $field;
      /// Sets where the child sits when it fills neither axis.
      fn set_alignment(alignment: impl Into<$crate::Value<$crate::Alignment, $crate::Condition<<$platform as $crate::Platform>::Scalar>>>);
      /// Derives where the child sits from the signals `alignment` reads.
      fn derive_alignment[R: Into<$crate::StaticLeaf<$crate::Alignment>>](alignment: impl Fn() -> R + 'static);
      /// Names the container for container queries against its size.
      fn set_container_name(name: $crate::ContainerName);
      /// Declares the axes a press's pointer drags the box along: the press's moves and
      /// its release reach the box wherever the pointer goes, and a touch pans only along
      /// the others.
      fn set_drag_axes(axes: $crate::DragAxes);
      /// Sets the box's accessible name, for a region, a dialog or a picture that no visible
      /// text names.
      fn set_label(label: impl Into<String>);
      /// Sets the node whose text names the box: a region's heading.
      fn set_labelled_by(node: $crate::NodeId);
      /// Sets the semantic role, guiding the platform's choice of element.
      fn set_role(role: $crate::Role);
      /// Sets the box's place in sequential focus traversal; a box without one takes no
      /// focus.
      fn set_focus_order(focus_order: $crate::FocusOrder);
      /// Derives the box's place in sequential focus traversal from the signals
      /// `focus_order` reads.
      fn derive_focus_order(focus_order: impl Fn() -> $crate::FocusOrder + 'static);
      /// Sets what the box reports to assistive technology, beside its role.
      fn set_accessibility(access: impl IntoIterator<Item = $crate::AccessProp>);
      /// Derives what the box reports to assistive technology from the signals `access`
      /// reads.
      fn derive_accessibility(access: impl Fn() -> Vec<$crate::AccessProp> + 'static);
      /// Binds `handle` to the box once built: an overlay is placed against it, focus
      /// moves to it, which the box takes only with a focus order, and its measured size
      /// is reported through the handle.
      fn bind_handle(handle: $crate::NodeHandle<$platform>);
      /// Adds a property transition, interpolating that property whenever its value
      /// changes.
      fn set_transition(transition: $crate::Transition<<<$platform as $crate::Platform>::StyleExt as $crate::StyleExtension>::Property>);
      /// Adds a keyframe animation.
      fn set_animation(animation: $crate::Animation<$crate::BoxKeyframe<$platform>>);
      /// Names the box for view transitions: it is paired with the box of the same name
      /// across a navigation, where the platform animates between them.
      fn set_transition_name(name: impl Into<String>);
      /// Runs `action` on a click; return [`Bubbling::Stop`](skene_foundation::Bubbling::Stop) to keep the click from the
      /// ancestors.
      fn handle_click[R: Into<$crate::Bubbling>](action: impl Fn() -> R + 'static);
      /// Runs `action` on the second click of a double click, after the click itself;
      /// return [`Bubbling::Stop`](skene_foundation::Bubbling::Stop) to keep the double click from the ancestors.
      fn handle_double_click[R: Into<$crate::Bubbling>](action: impl Fn() -> R + 'static);
      /// Runs `action` on a key pressed while the box or a descendant holds focus; return
      /// [`Bubbling::Stop`](skene_foundation::Bubbling::Stop) to keep the key from the ancestors.
      fn handle_key_down[R: Into<$crate::Bubbling>](action: impl Fn(&$crate::KeyEvent) -> R + 'static);
      /// Runs `action` on a key released while the box or a descendant holds focus;
      /// return [`Bubbling::Stop`](skene_foundation::Bubbling::Stop) to keep the release from the ancestors.
      fn handle_key_up[R: Into<$crate::Bubbling>](action: impl Fn(&$crate::KeyEvent) -> R + 'static);
      /// Runs `action` on a pointer button pressed over the box; return
      /// [`Bubbling::Stop`](skene_foundation::Bubbling::Stop) to keep the press from the ancestors.
      fn handle_pointer_down[R: Into<$crate::Bubbling>](action: impl Fn(&$crate::PointerEvent) -> R + 'static);
      /// Runs `action` on a pointer button released over the box; return
      /// [`Bubbling::Stop`](skene_foundation::Bubbling::Stop) to keep the release from the ancestors.
      fn handle_pointer_up[R: Into<$crate::Bubbling>](action: impl Fn(&$crate::PointerEvent) -> R + 'static);
      /// Runs `action` on the pointer moving over the box; return [`Bubbling::Stop`](skene_foundation::Bubbling::Stop) to
      /// keep the move from the ancestors.
      fn handle_pointer_move[R: Into<$crate::Bubbling>](action: impl Fn(&$crate::PointerEvent) -> R + 'static);
      /// Offers what a drag from the box carries, `data` building it as the drag starts;
      /// the box holds [`State::Dragged`](skene_foundation::State::Dragged) while the drag lasts.
      fn set_drag_data(data: impl Fn() -> $crate::DragData + 'static);
      /// Takes a drag carrying a `T` dropped on the box, running `action` with the value;
      /// the box holds [`State::Targeted`](skene_foundation::State::Targeted) while such a drag
      /// is over it.
      fn handle_drop[T: 'static](action: impl Fn(&T) + 'static);
      /// Runs `action` on a wheel turned over the box, before the platform scrolls by it;
      /// return [`Bubbling::Stop`](skene_foundation::Bubbling::Stop) to keep the turn from the ancestors.
      fn handle_wheel[R: Into<$crate::Bubbling>](action: impl Fn(&$crate::WheelEvent) -> R + 'static);
      /// Runs `action` when the platform takes a pointer pressed over the box for itself,
      /// ending the press without a release; return [`Bubbling::Stop`](skene_foundation::Bubbling::Stop) to keep the cancel
      /// from the ancestors.
      fn handle_pointer_cancel[R: Into<$crate::Bubbling>](action: impl Fn(&$crate::PointerEvent) -> R + 'static);
      /// Runs `action` when the pointer enters the box.
      fn handle_pointer_enter(action: impl Fn() + 'static);
      /// Runs `action` when the pointer leaves the box.
      fn handle_pointer_leave(action: impl Fn() + 'static);
      /// Runs `action` when focus enters the box or a node inside it from outside.
      fn handle_focus(action: impl Fn() + 'static);
      /// Runs `action` when focus leaves the box and every node inside it.
      fn handle_blur(action: impl Fn() + 'static);
      /// Runs `action` with the box's new offsets when it scrolls; the box scrolls only
      /// with an overflow that allows it.
      fn handle_scroll(action: impl Fn(&$crate::ScrollEvent) + 'static);
    }
  };
}

impl<P: Platform> Container<P> {
  /// An undecorated container.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      inner: LayoutBox::new(Layout::Single),
    }
  }

  crate::container_surface! { inner, P }
}

impl<P: Platform> HasChild<P> for Container<P> {
  type Kind = PlainChild;

  #[inline(always)]
  fn child_mut(&mut self, seal: Seal) -> &mut Children<P, PlainChild> {
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
