//! Lazy lists: a keyed list building only the slice in view.

use crate::ActivePlatform;

/// A vertically scrolling column of items, only the slice in view plus an overscan
/// margin built at any moment; the rest is empty space.
///
/// Scrolls on its own overflow; a size comes from the box it is wrapped in. Items are
/// keyed, so scrolling reuses the views whose keys stay in the window, and an item
/// expresses its own reactivity as usual. The item height is fixed or estimated
/// ([`ItemExtent`](skene_foundation::ItemExtent)); until one is set every item builds.
#[must_use]
pub struct LazyColumn(skene_foundation::LazyColumn<ActivePlatform>);

skene_foundation::impl_lazy_column!(LazyColumn, ActivePlatform);

/// A horizontally scrolling row of items, only the slice in view plus an overscan
/// margin built at any moment; the rest is empty space.
///
/// Scrolls on its own overflow; a size comes from the box it is wrapped in. Items are
/// keyed, so scrolling reuses the views whose keys stay in the window, and an item
/// expresses its own reactivity as usual. The item width is fixed or estimated
/// ([`ItemExtent`](skene_foundation::ItemExtent)); until one is set every item builds.
#[must_use]
pub struct LazyRow(skene_foundation::LazyRow<ActivePlatform>);

skene_foundation::impl_lazy_row!(LazyRow, ActivePlatform);
