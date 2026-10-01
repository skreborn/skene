//! The grid layout.

use skene_foundation::{AnyKind, Children, GridChild, HasChildren, NodeId, Platform, Seal, View};

use crate::ActivePlatform;

/// Children placed into tracks.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`GridCell`](crate::GridCell)
/// wrapper as a child.
#[derive(Debug, Default)]
#[must_use]
pub struct Grid(skene_foundation::Grid<ActivePlatform>);

impl Grid {
  /// A grid with no explicit tracks.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::Grid::new())
  }

  /// A grid with `count` columns of equal width.
  #[inline(always)]
  pub fn uniform(count: u16) -> Self {
    Self(skene_foundation::Grid::uniform(count))
  }

  /// A grid with as many columns as fit, each at least `minimum` wide, sharing what
  /// width remains equally.
  #[inline(always)]
  pub fn adaptive(minimum: <ActivePlatform as Platform>::Scalar) -> Self {
    Self(skene_foundation::Grid::adaptive(minimum))
  }

  skene_foundation::grid_surface! { 0, ActivePlatform }
}

impl HasChildren<ActivePlatform> for Grid {
  type Kind = GridChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<ActivePlatform, GridChild> {
    self.0.children_mut(seal)
  }
}

impl AnyKind for Grid {}

impl View<ActivePlatform> for Grid {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
