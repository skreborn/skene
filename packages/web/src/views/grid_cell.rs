//! The wrapper placing a grid child in the grid's tracks.

use skene_foundation::NoChild;

use crate::ActivePlatform;

/// Places the child it wraps in the tracks of a [`Grid`](crate::Grid).
///
/// Creates no node: the placements and the alignment become properties of the child's
/// own style, so they compile, condition and derive like any property. The child is
/// auto-placed on an axis with no placement set. Accepted only where a grid's children
/// are; holds [`NoChild`] until `set_child` supplies the child.
#[must_use]
pub struct GridCell<C = NoChild>(skene_foundation::GridCell<ActivePlatform, C>);

skene_foundation::impl_grid_cell!(GridCell, ActivePlatform);
