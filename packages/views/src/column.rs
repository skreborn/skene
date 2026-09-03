//! A vertical flex layout.

use std::rc::Rc;

use skene_core::{
  FlexDirection,
  Layout,
  Spacing,
  next::{
    AnyKind,
    Children,
    Condition,
    Driver,
    FlexChild,
    HasChildren,
    Leaf,
    NodeId,
    Seal,
    Value,
    View,
  },
};

use crate::LayoutBox;

/// Children laid out top to bottom.
///
/// Carries layout properties and nothing else; decorating a column means wrapping it
/// in a [`Container`](crate::Container). Accepts any view or a
/// [`Flexible`](crate::Flexible) wrapper as a child.
#[must_use]
pub struct Column<D: Driver> {
  inner: LayoutBox<D, FlexChild>,
}

impl<D: Driver> Default for Column<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> Column<D> {
  /// An empty column.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      inner: LayoutBox::new(Layout::Flex {
        direction: FlexDirection::Column,
      }),
    }
  }

  /// Sets the spacing between children.
  #[inline(always)]
  pub fn set_spacing(
    mut self,
    spacing: impl Into<Value<Spacing<D::Scalar>, Condition<D::Scalar>>>,
  ) -> Self {
    self.inner.style_mut().spacing = Some(spacing.into());

    self
  }

  /// Derives the spacing between children from the signals `spacing` reads.
  #[inline(always)]
  pub fn derive_spacing(mut self, spacing: impl Fn() -> Spacing<D::Scalar> + 'static) -> Self {
    self.inner.style_mut().spacing = Some(Value::new(Leaf::Derived(Rc::new(spacing))));

    self
  }
}

impl<D: Driver> HasChildren<D> for Column<D> {
  type Kind = FlexChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<D, FlexChild> {
    self.inner.children_mut(seal)
  }
}

impl<D: Driver> AnyKind for Column<D> {}

impl<D: Driver> View<D> for Column<D> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.inner.build()
  }
}
