//! The grid layout.

use crate::ActivePlatform;

/// Children placed into tracks.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`GridCell`](crate::GridCell)
/// wrapper as a child.
#[must_use]
pub struct Grid(skene_foundation::Grid<ActivePlatform>);

skene_foundation::impl_grid!(Grid, ActivePlatform);
