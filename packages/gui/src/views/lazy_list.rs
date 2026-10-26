//! Lazy lists: a keyed list building only the slice in view.

use skene_foundation::{LazyColumnBase, LazyRowBase, View, Wraps};

use crate::Active;

/// A vertically scrolling column of items, only the slice in view plus an overscan
/// margin built at any moment; the rest is empty space.
///
/// Scrolls on its own overflow; a size comes from the box it is wrapped in. Items are
/// keyed, so scrolling reuses the views whose keys stay in the window, and an item
/// expresses its own reactivity as usual. The item height is fixed or estimated
/// ([`ItemExtent`](skene_foundation::ItemExtent)); until one is set every item builds.
///
/// ```
/// use skene_gui::prelude::*;
///
/// let rows = LazyColumn::new().set_item_height(24).derive_items(
///   || 0..10_000,
///   |index| *index,
///   |index| Text::new(format!("Row {index}")),
/// );
/// ```
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct LazyColumn(skene_foundation::LazyColumn<Active>);

impl LazyColumnBase<Active> for LazyColumn {}

/// A horizontally scrolling row of items, only the slice in view plus an overscan
/// margin built at any moment; the rest is empty space.
///
/// Scrolls on its own overflow; a size comes from the box it is wrapped in. Items are
/// keyed, so scrolling reuses the views whose keys stay in the window, and an item
/// expresses its own reactivity as usual. The item width is fixed or estimated
/// ([`ItemExtent`](skene_foundation::ItemExtent)); until one is set every item builds.
///
/// ```
/// use skene_gui::prelude::*;
///
/// let strip = LazyRow::new().set_item_width(64).derive_items(
///   || 0..1_000,
///   |index| *index,
///   |index| Text::new(index.to_string()),
/// );
/// ```
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct LazyRow(skene_foundation::LazyRow<Active>);

impl LazyRowBase<Active> for LazyRow {}
