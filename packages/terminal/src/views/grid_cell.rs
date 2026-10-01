//! The wrapper placing a grid child in the grid's tracks.

use skene_foundation::{AnyView, GridCellBase, GridChild, IntoChildOf, NoChild, View, Wraps};

use crate::ActivePlatform;

/// Places the child it wraps in the tracks of a [`Grid`](crate::Grid).
///
/// Creates no node: the placements and the alignment become properties of the child's
/// own style, so they compile, condition and derive like any property. The child is
/// auto-placed on an axis with no placement set. Accepted only where a grid's children
/// are; holds [`NoChild`] until `set_child` supplies the child.
///
/// ```
/// use skene_terminal::prelude::*;
///
/// let layout = Grid::uniform(3).set_children((GridCell::new()
///   .set_column(Line::between(1, 3))
///   .set_child(Text::new("Wide")),));
/// ```
#[derive(Debug, Wraps)]
#[must_use]
pub struct GridCell<C = NoChild>(skene_foundation::GridCell<ActivePlatform, C>);

impl Default for GridCell {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl GridCellBase<ActivePlatform> for GridCell {}

impl GridCell {
  /// Sets the child.
  #[inline(always)]
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
