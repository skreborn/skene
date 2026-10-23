//! The wrapper placing a grid child in the grid's tracks.

use skene_foundation::{AnyView, GridCellBase, GridChild, IntoChildOf, Wraps};

use crate::Active;

/// Places the child it wraps in the tracks of a [`Grid`](crate::Grid).
///
/// Creates no node: the placements and the alignment become properties of the child's
/// own style, so they compile, condition and derive like any property. The child is
/// auto-placed on an axis with no placement set. Accepted only where a grid's children
/// are; a wrapper without its child renders nothing and takes no room.
///
/// ```
/// use skene_web::prelude::*;
///
/// let layout = Grid::uniform(3).set_children((GridCell::new()
///   .set_column(Line::between(1, 3))
///   .set_child(Text::new("Wide")),));
/// ```
#[derive(Debug, Wraps)]
#[must_use]
pub struct GridCell(skene_foundation::GridCell<Active>);

impl Default for GridCell {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl GridCellBase<Active> for GridCell {}

impl IntoChildOf<Active, GridChild> for GridCell {
  #[inline(always)]
  fn into_child(self) -> AnyView<Active> {
    self.0.into_child()
  }
}
