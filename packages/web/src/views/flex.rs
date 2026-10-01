//! The flex layouts: a row and a column.

use crate::ActivePlatform;

/// Children laid out left to right.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Flexible`](crate::Flexible)
/// wrapper as a child.
#[must_use]
pub struct Row(skene_foundation::Row<ActivePlatform>);

skene_foundation::impl_row!(Row, ActivePlatform);

/// Children laid out top to bottom.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Flexible`](crate::Flexible)
/// wrapper as a child.
#[must_use]
pub struct Column(skene_foundation::Column<ActivePlatform>);

skene_foundation::impl_column!(Column, ActivePlatform);
