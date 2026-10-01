//! The grid layout.

use skene_foundation::{GridBase, View, Wraps};

use crate::ActivePlatform;

/// Children placed into tracks.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`GridCell`](crate::GridCell)
/// wrapper as a child.
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct Grid(skene_foundation::Grid<ActivePlatform>);

impl GridBase<ActivePlatform> for Grid {}
