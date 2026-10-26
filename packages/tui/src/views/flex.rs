//! The flex layouts: a row and a column.

use skene_foundation::{ColumnBase, FlexBase, RowBase, View, Wraps};

use crate::Active;

/// Children laid out left to right.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Flexible`](crate::Flexible)
/// wrapper as a child.
///
/// ```
/// use skene_tui::prelude::*;
///
/// let toolbar = Row::new()
///   .set_spacing(1)
///   .set_item_alignment(ItemAlignment::Center)
///   .set_children((Text::new("Title"), Text::new("Subtitle")));
/// ```
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct Row(skene_foundation::Row<Active>);

impl RowBase<Active> for Row {}

impl FlexBase<Active> for Row {}

/// Children laid out top to bottom.
///
/// Carries layout properties and nothing else; decorating it means wrapping it in a
/// [`Container`](crate::Container). Accepts any view or a [`Flexible`](crate::Flexible)
/// wrapper as a child.
///
/// ```
/// use skene_tui::prelude::*;
///
/// let list = Column::new()
///   .set_spacing(1)
///   .set_arrangement(Arrangement::Start)
///   .set_children((Text::new("First"), Text::new("Second")));
/// ```
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct Column(skene_foundation::Column<Active>);

// A layout view is at most a kibibyte on the stack, however many properties its style
// holds.
const _: () = assert!(size_of::<Column>() <= 1024);

impl ColumnBase<Active> for Column {}

impl FlexBase<Active> for Column {}
