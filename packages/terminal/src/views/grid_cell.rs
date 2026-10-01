//! The wrapper placing a grid child in the grid's tracks.

use skene_foundation::{AnyView, GridChild, IntoChildOf, NoChild, View};

use crate::ActivePlatform;

/// Places the child it wraps in the tracks of a [`Grid`](crate::Grid).
///
/// Creates no node: the placements and the alignment become properties of the child's
/// own style, so they compile, condition and derive like any property. The child is
/// auto-placed on an axis with no placement set. Accepted only where a grid's children
/// are; holds [`NoChild`] until `set_child` supplies the child.
#[must_use]
pub struct GridCell<C = NoChild>(skene_foundation::GridCell<ActivePlatform, C>);

impl<C> std::fmt::Debug for GridCell<C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(formatter)
  }
}

impl Default for GridCell {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl GridCell {
  /// A wrapper placing its child automatically, with no child yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::GridCell::new())
  }

  skene_foundation::grid_cell_surface! { 0, ActivePlatform }

  /// Sets the child.
  pub fn set_child(self, child: impl View<ActivePlatform>) -> GridCell<AnyView<ActivePlatform>> {
    GridCell(self.0.set_child(child))
  }
}

impl IntoChildOf<ActivePlatform, GridChild> for GridCell<AnyView<ActivePlatform>> {
  #[inline(always)]
  fn into_child(self) -> AnyView<ActivePlatform> {
    self.0.into_child()
  }
}
