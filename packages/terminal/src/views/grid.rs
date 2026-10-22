//! The grid layout.

use skene_foundation::{GridBase, View, Wraps};

use crate::Active;

/// Children placed into tracks.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`GridCell`](crate::GridCell)
/// wrapper as a child.
///
/// ```
/// use skene_terminal::prelude::*;
///
/// let tiles = Grid::uniform(3).set_spacing(1).set_children((
///   Text::new("One"),
///   Text::new("Two"),
///   Text::new("Three"),
/// ));
/// ```
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct Grid(skene_foundation::Grid<Active>);

impl GridBase<Active> for Grid {}
