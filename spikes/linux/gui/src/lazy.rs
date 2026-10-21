//! Lazy lists: scroll boxes that realize only the items in view and a few beyond, so a list of
//! a hundred thousand items costs what its viewport shows.

use skene_foundation::{
  lazy::GridCells,
  view::{self as base, ItemHeightBase, ItemWidthBase, LazyBase, Wraps},
  wraps,
};

use crate::{Active, Axis};

/// Items one below another, only those in view realized: Compose's `LazyColumn`, SwiftUI's
/// `LazyVStack` in a `ScrollView`.
///
/// It scrolls itself, so it takes a height, a greatest height or a `Flexible` weight to stay
/// within the window, as a `Scroll` does. Each item is built once per key while it is in view
/// or holds focus, measured as it is realized (or fixed, [`ItemHeightBase::set_item_height`]),
/// and the content never moves under the reader as items above measure or change. A reader
/// finds a list counting every item, each realized row its place in it.
///
/// ```
/// use skene_gui::prelude::*;
///
/// let names = Signal::new((0..10_000).map(|n| format!("Contact {n}")).collect::<Vec<_>>());
///
/// LazyColumn::new()
///   .set_item_height(ItemExtent::estimated(40))
///   .set_spacing(1)
///   .set_label("Contacts")
///   .derive_items(move || names.get(), Clone::clone, |name: String| Text::new(name));
/// ```
#[must_use]
pub struct LazyColumn(base::Lazy<Active>);

wraps!([] LazyColumn => base::Lazy<Active>, Active);

impl LazyBase<Active> for LazyColumn {}

impl ItemHeightBase<Active> for LazyColumn {}

impl LazyColumn {
  /// An empty list scrolling up and down.
  pub fn new() -> Self {
    Self(base::Lazy::new(Axis::Vertical, None))
  }
}

impl Default for LazyColumn {
  fn default() -> Self {
    Self::new()
  }
}

/// Items side by side, only those in view realized: Compose's `LazyRow`, SwiftUI's `LazyHStack`.
///
/// Its members are a [`LazyColumn`]'s along the horizontal axis, its items' widths
/// ([`ItemWidthBase::set_item_width`]) in place of their heights.
#[must_use]
pub struct LazyRow(base::Lazy<Active>);

wraps!([] LazyRow => base::Lazy<Active>, Active);

impl LazyBase<Active> for LazyRow {}

impl ItemWidthBase<Active> for LazyRow {}

impl LazyRow {
  /// An empty list scrolling side to side.
  pub fn new() -> Self {
    Self(base::Lazy::new(Axis::Horizontal, None))
  }
}

impl Default for LazyRow {
  fn default() -> Self {
    Self::new()
  }
}

/// Items in rows of cells, scrolling up and down, only the rows in view realized: Compose's
/// `LazyVerticalGrid`, SwiftUI's `LazyVGrid`.
///
/// A header or a single item takes a row of its own; a row is as tall as its tallest cell.
///
/// ```
/// use skene_gui::prelude::*;
///
/// LazyVerticalGrid::new()
///   .set_columns(GridCells::adaptive(160))
///   .set_spacing(8)
///   .add_header(Text::new("Photos"))
///   .derive_items(|| (0..500).collect::<Vec<u32>>(), |n| *n, |n: u32| Text::new(format!("{n}")));
/// ```
#[must_use]
pub struct LazyVerticalGrid(base::Lazy<Active>);

wraps!([] LazyVerticalGrid => base::Lazy<Active>, Active);

impl LazyBase<Active> for LazyVerticalGrid {}

impl ItemHeightBase<Active> for LazyVerticalGrid {}

impl LazyVerticalGrid {
  /// An empty grid scrolling up and down, two columns until [`LazyVerticalGrid::set_columns`]
  /// says otherwise.
  pub fn new() -> Self {
    Self(base::Lazy::new(Axis::Vertical, Some(GridCells::Count(2))))
  }

  /// How many columns its rows hold.
  pub fn set_columns(self, columns: GridCells) -> Self {
    self.map_inner(|lazy| lazy.with_cells(columns))
  }
}

impl Default for LazyVerticalGrid {
  fn default() -> Self {
    Self::new()
  }
}

/// Items in columns of cells, scrolling side to side, only the columns in view realized:
/// Compose's `LazyHorizontalGrid`, SwiftUI's `LazyHGrid`.
#[must_use]
pub struct LazyHorizontalGrid(base::Lazy<Active>);

wraps!([] LazyHorizontalGrid => base::Lazy<Active>, Active);

impl LazyBase<Active> for LazyHorizontalGrid {}

impl ItemWidthBase<Active> for LazyHorizontalGrid {}

impl LazyHorizontalGrid {
  /// An empty grid scrolling side to side, two rows until [`LazyHorizontalGrid::set_rows`] says
  /// otherwise.
  pub fn new() -> Self {
    Self(base::Lazy::new(Axis::Horizontal, Some(GridCells::Count(2))))
  }

  /// How many rows its columns hold.
  pub fn set_rows(self, rows: GridCells) -> Self {
    self.map_inner(|lazy| lazy.with_cells(rows))
  }
}

impl Default for LazyHorizontalGrid {
  fn default() -> Self {
    Self::new()
  }
}
