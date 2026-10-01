//! The flex layouts: a row and a column.

use skene_foundation::{AnyKind, Children, FlexChild, HasChildren, NodeId, Seal, View};

use crate::ActivePlatform;

/// Children laid out left to right.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Flexible`](crate::Flexible)
/// wrapper as a child.
#[derive(Debug, Default)]
#[must_use]
pub struct Row(skene_foundation::Row<ActivePlatform>);

impl Row {
  /// An empty layout running left to right.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::Row::new())
  }

  /// An empty layout running left to right, reversed.
  #[inline(always)]
  pub fn reversed() -> Self {
    Self(skene_foundation::Row::reversed())
  }

  skene_foundation::flex_surface! { 0, ActivePlatform }
}

impl HasChildren<ActivePlatform> for Row {
  type Kind = FlexChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<ActivePlatform, FlexChild> {
    self.0.children_mut(seal)
  }
}

impl AnyKind for Row {}

impl View<ActivePlatform> for Row {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}

/// Children laid out top to bottom.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Flexible`](crate::Flexible)
/// wrapper as a child.
#[derive(Debug, Default)]
#[must_use]
pub struct Column(skene_foundation::Column<ActivePlatform>);

impl Column {
  /// An empty layout running top to bottom.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::Column::new())
  }

  /// An empty layout running top to bottom, reversed.
  #[inline(always)]
  pub fn reversed() -> Self {
    Self(skene_foundation::Column::reversed())
  }

  skene_foundation::flex_surface! { 0, ActivePlatform }
}

impl HasChildren<ActivePlatform> for Column {
  type Kind = FlexChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<ActivePlatform, FlexChild> {
    self.0.children_mut(seal)
  }
}

impl AnyKind for Column {}

impl View<ActivePlatform> for Column {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
