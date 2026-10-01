//! The one decorated box.

use skene_foundation::{AnyKind, Children, HasChild, NodeId, PlainChild, Seal, View};

use crate::ActivePlatform;

/// Decoration and sizing around exactly one child.
///
/// The freely stylable box: decoration, sizing, events, alignment and a single child,
/// set through [`HasChild::set_child`] or [`HasChild::derive_child`]. The child fills
/// the container when the container is sized and the container hugs the child when it is
/// not; [`set_alignment`](Self::set_alignment) places a child that does neither. Several
/// children take a layout view, wrapped in a container to decorate it.
#[derive(Debug, Default)]
#[must_use]
pub struct Container(skene_foundation::Container<ActivePlatform>);

impl Container {
  /// An undecorated container.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::Container::new())
  }

  skene_foundation::container_surface! { 0, ActivePlatform }
}

impl HasChild<ActivePlatform> for Container {
  type Kind = PlainChild;

  #[inline(always)]
  fn child_mut(&mut self, seal: Seal) -> &mut Children<ActivePlatform, PlainChild> {
    self.0.child_mut(seal)
  }
}

impl AnyKind for Container {}

impl View<ActivePlatform> for Container {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
