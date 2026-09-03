//! The one decorated box.

use std::rc::Rc;

use skene_core::{
  Bubbling,
  Color,
  Dimension,
  Inset,
  Layout,
  Role,
  next::{
    AnyKind,
    Children,
    Condition,
    ContainerName,
    Driver,
    HasChildren,
    Leaf,
    NodeId,
    PlainChild,
    Seal,
    Value,
    View,
  },
};

use crate::LayoutBox;

/// A property value under the driver's conditions.
type Of<D, T> = Value<T, Condition<<D as Driver>::Scalar>>;

/// Decoration and sizing around exactly one child.
///
/// The freely stylable box: decoration, sizing, events and a single child, set through
/// [`HasChildren::set_child`] or [`HasChildren::derive_child`].
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

  /// Sets the background color.
  #[inline(always)]
  pub fn set_color(mut self, color: impl Into<Of<D, Color>>) -> Self {
    self.inner.style_mut().color = Some(color.into());

    self
  }

  /// Derives the background color from the signals `color` reads.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.inner.style_mut().color = Some(Value::new(Leaf::Derived(Rc::new(color))));

    self
  }

  /// Sets the padding on each edge.
  #[inline(always)]
  pub fn set_padding(mut self, padding: impl Into<Of<D, Inset<D::Scalar>>>) -> Self {
    self.inner.style_mut().padding = Some(padding.into());

    self
  }

  /// Derives the padding from the signals `padding` reads.
  #[inline(always)]
  pub fn derive_padding(mut self, padding: impl Fn() -> Inset<D::Scalar> + 'static) -> Self {
    self.inner.style_mut().padding = Some(Value::new(Leaf::Derived(Rc::new(padding))));

    self
  }

  /// Sets the width.
  #[inline(always)]
  pub fn set_width(mut self, width: impl Into<Of<D, Dimension<D::Scalar>>>) -> Self {
    self.inner.style_mut().width = Some(width.into());

    self
  }

  /// Derives the width from the signals `width` reads.
  #[inline(always)]
  pub fn derive_width(mut self, width: impl Fn() -> Dimension<D::Scalar> + 'static) -> Self {
    self.inner.style_mut().width = Some(Value::new(Leaf::Derived(Rc::new(width))));

    self
  }

  /// Sets the height.
  #[inline(always)]
  pub fn set_height(mut self, height: impl Into<Of<D, Dimension<D::Scalar>>>) -> Self {
    self.inner.style_mut().height = Some(height.into());

    self
  }

  /// Derives the height from the signals `height` reads.
  #[inline(always)]
  pub fn derive_height(mut self, height: impl Fn() -> Dimension<D::Scalar> + 'static) -> Self {
    self.inner.style_mut().height = Some(Value::new(Leaf::Derived(Rc::new(height))));

    self
  }

  /// Names the container for container queries against its size.
  #[inline(always)]
  pub fn set_container_name(mut self, name: ContainerName) -> Self {
    self.inner = self.inner.set_container_name(name);

    self
  }

  /// Sets the semantic role.
  #[inline(always)]
  pub fn set_role(mut self, role: Role) -> Self {
    self.inner = self.inner.set_role(role);

    self
  }

  /// Runs `action` on a click; return [`Bubbling::Stop`] to keep the click from the
  /// ancestors.
  #[inline(always)]
  pub fn handle_click<R: Into<Bubbling>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self.inner = self.inner.handle_click(action);

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
