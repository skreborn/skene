//! The flex layouts: a row and a column.

use skene_foundation::{ColumnBase, FlexBase, RowBase, View, Wraps};

use crate::ActivePlatform;

/// Children laid out left to right.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Flexible`](crate::Flexible)
/// wrapper as a child.
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct Row(skene_foundation::Row<ActivePlatform>);

impl RowBase<ActivePlatform> for Row {}

impl FlexBase<ActivePlatform> for Row {}

/// Children laid out top to bottom.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Flexible`](crate::Flexible)
/// wrapper as a child.
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct Column(skene_foundation::Column<ActivePlatform>);

impl ColumnBase<ActivePlatform> for Column {}

impl FlexBase<ActivePlatform> for Column {}
