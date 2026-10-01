//! Scrolling lists building only their visible slice: a column and a row.

use std::{cell::RefCell, collections::HashMap, hash::Hash, rc::Rc};

use skene_reactive::{Get, Memo, Set, Signal, Update};

use crate::{
  AnyKind,
  Axis,
  Container,
  DecorationBase,
  Dimension,
  Flexibility,
  HasBoxStyle,
  HasChild,
  HasChildren,
  Layout,
  LayoutBox,
  NodeId,
  Overflow,
  PlainChild,
  Platform,
  Scalar,
  ScrollEvent,
  Size,
  View,
  Wraps,
};

/// The slice of a virtual list worth rendering: item indices `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct ItemWindow {
  /// The first rendered item's index.
  pub start: usize,
  /// One past the last rendered item's index.
  pub end: usize,
}

impl ItemWindow {
  /// The number of rendered items.
  #[inline(always)]
  #[must_use]
  pub const fn len(&self) -> usize {
    self.end - self.start
  }

  /// Whether the window renders nothing.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.start == self.end
  }
}

/// The item window covering a viewport of `viewport` at scroll `offset`, over `count`
/// items each `item_extent` long, padded by `overscan` extra items on each side.
///
/// Extents are in the caller's unit (pixels, cells); a non-positive `item_extent`
/// yields the full list.
#[inline(always)]
pub fn item_window(
  offset: f32,
  viewport: f32,
  item_extent: f32,
  count: usize,
  overscan: usize,
) -> ItemWindow {
  if item_extent <= 0.0 {
    return ItemWindow {
      start: 0,
      end: count,
    };
  }

  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the offset is clamped non-negative, and an item index stays far below usize::MAX"
  )]
  let first = (offset.max(0.0) / item_extent).floor() as usize;
  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the offset is clamped non-negative, and an item index stays far below usize::MAX"
  )]
  let visible = ((offset.max(0.0) + viewport.max(0.0)) / item_extent).ceil() as usize;

  ItemWindow {
    start: first.saturating_sub(overscan).min(count),
    end: visible.saturating_add(overscan).min(count),
  }
}

/// The item window covering a viewport of `viewport` at scroll `offset`, over items
/// whose leading edges are `edges`, padded by `overscan` extra items on each side.
///
/// `edges` holds one entry per item, its distance from the start, and one more for the
/// whole extent, so an item's length is the difference of its neighbors; the entries
/// never decrease. Extents are in the caller's unit (pixels, cells).
pub fn item_window_over(offset: f32, viewport: f32, edges: &[f32], overscan: usize) -> ItemWindow {
  let count = edges.len().saturating_sub(1);
  let offset = offset.max(0.0);
  let first = edges
    .partition_point(|edge| *edge <= offset)
    .saturating_sub(1)
    .min(count);
  let visible = edges
    .partition_point(|edge| *edge < offset + viewport.max(0.0))
    .min(count);

  ItemWindow {
    start: first.saturating_sub(overscan),
    end: visible.saturating_add(overscan).min(count).max(first),
  }
}

/// The extent of every item of a lazy list along its axis, in the medium's unit.
///
/// `set_item_height(32.)` on a column and `set_item_width(32.)` on a row take a plain
/// number as the fixed form.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum ItemExtent {
  /// Every item is this long; a non-positive length builds every item.
  Fixed(f32),
  /// Items are measured as they build and remembered by key; one not yet measured is
  /// assumed this long. An estimate off from the measure moves what follows by the
  /// difference when the item measures.
  Estimated(f32),
}

impl From<f32> for ItemExtent {
  #[inline(always)]
  fn from(extent: f32) -> Self {
    Self::Fixed(extent)
  }
}

/// The axis a lazy list scrolls along.
#[derive(Clone, Copy)]
enum ScrollAxis {
  Horizontal,
  Vertical,
}

impl ScrollAxis {
  #[inline(always)]
  const fn layout(self) -> Layout {
    let axis = match self {
      Self::Horizontal => Axis::Horizontal,
      Self::Vertical => Axis::Vertical,
    };

    Layout::Flex {
      axis,
      reversed: false,
    }
  }

  #[inline(always)]
  const fn offset(self, event: ScrollEvent) -> f32 {
    match self {
      Self::Horizontal => event.x,
      Self::Vertical => event.y,
    }
  }

  fn extent<N: Scalar>(self, size: Size<N>) -> f32 {
    match self {
      Self::Horizontal => size.width.to_f32(),
      Self::Vertical => size.height.to_f32(),
    }
  }

  /// `container` sized to `extent` along the axis.
  fn sized<P: Platform>(
    self,
    container: Container<P>,
    extent: Dimension<P::Scalar>,
  ) -> Container<P> {
    match self {
      Self::Horizontal => container.set_width(extent),
      Self::Vertical => container.set_height(extent),
    }
  }

  /// `container` sized along the axis by `extent`, followed.
  fn derive_sized<P: Platform>(
    self,
    container: Container<P>,
    extent: impl Fn() -> f32 + 'static,
  ) -> Container<P> {
    let length = move || Dimension::Length(P::Scalar::from_f32(extent()));

    match self {
      Self::Horizontal => container.derive_width(length),
      Self::Vertical => container.derive_height(length),
    }
  }
}

/// The slice a list shows: the window over its items, their count, and the extents
/// of the items before and after the window.
#[derive(Clone, Copy, PartialEq)]
struct Slice {
  window: ItemWindow,
  count: usize,
  leading: f32,
  trailing: f32,
}

/// The measured extents of the items built so far, bumping `version` on a change.
struct Measures {
  version: Signal<u32>,
  axis: ScrollAxis,
}

/// Installs the keyed segment over the slice onto the list.
type Install<P> =
  Box<dyn FnOnce(Memo<Slice>, Rc<Measures>, LayoutBox<P, PlainChild>) -> LayoutBox<P, PlainChild>>;

/// The leading edges of the items, one per item and the whole extent last, from the
/// measures and the estimate.
type Edges = Rc<dyn Fn(&Measures, f32) -> Vec<f32>>;

/// What `derive_items` leaves for the build: the item count, tracked, the edges, and
/// the keyed segment installed over the slice.
struct Items<P: Platform> {
  count: Rc<dyn Fn() -> usize>,
  edges: Edges,
  install: Install<P>,
}

/// The list both public forms share: the axis, the item extent and the overscan.
struct LazyList<P: Platform> {
  axis: ScrollAxis,
  extent: ItemExtent,
  overscan: usize,
  items: Option<Items<P>>,
}

impl<P: Platform> LazyList<P> {
  const fn new(axis: ScrollAxis) -> Self {
    Self {
      axis,
      extent: ItemExtent::Fixed(0.),
      overscan: 3,
      items: None,
    }
  }

  fn derive_items<T, I, K, V>(
    mut self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    T: 'static,
    I: IntoIterator<Item = T>,
    I::IntoIter: ExactSizeIterator,
    K: Eq + Hash + Clone + 'static,
    V: View<P>,
  {
    let items = Rc::new(items);
    let key = Rc::new(key);
    let lengths: Rc<RefCell<HashMap<K, f32>>> = Rc::new(RefCell::new(HashMap::new()));
    let count = {
      let items = Rc::clone(&items);

      Rc::new(move || items().into_iter().len())
    };
    let edges: Edges = {
      let items = Rc::clone(&items);
      let key = Rc::clone(&key);
      let lengths = Rc::clone(&lengths);

      Rc::new(move |measures: &Measures, estimate: f32| {
        let _ = measures.version.get();

        let lengths = lengths.borrow();
        let mut edge = 0.;
        let mut edges = vec![0.];

        for item in items() {
          edge += lengths.get(&key(&item)).copied().unwrap_or(estimate);
          edges.push(edge);
        }

        edges
      })
    };
    let axis = self.axis;
    let extent = self.extent;
    let install = Box::new(
      move |slice: Memo<Slice>, measures: Rc<Measures>, list: LayoutBox<P, PlainChild>| {
        list.derive_children(
          move || {
            let Slice { window, .. } = slice.get();

            items()
              .into_iter()
              .skip(window.start)
              .take(window.len())
              .collect::<Vec<T>>()
          },
          {
            let key = Rc::clone(&key);

            move |item: &T| key(item)
          },
          move |item| {
            let item_key = key(&item);
            let container = rigid(Container::<P>::new().set_child(view(item).into_any()));
            let node = match extent {
              ItemExtent::Fixed(length) => axis
                .sized(container, Dimension::Length(P::Scalar::from_f32(length)))
                .build(),
              ItemExtent::Estimated(_) => container.build(),
            };

            if matches!(extent, ItemExtent::Estimated(_)) {
              let lengths = Rc::clone(&lengths);
              let measures = Rc::clone(&measures);

              let _observed = P::observe_size(
                node,
                Rc::new(move |size| {
                  let length = measures.axis.extent(size);
                  let changed =
                    lengths.borrow_mut().insert(item_key.clone(), length) != Some(length);

                  if changed {
                    measures.version.update(|version| *version += 1);
                  }
                }),
              );
            }

            node
          },
        )
      },
    );

    self.items = Some(Items {
      count,
      edges,
      install,
    });

    self
  }

  fn build(self) -> NodeId {
    let Self {
      axis,
      extent,
      overscan,
      items,
    } = self;
    let offset = Signal::transient(0.);
    let viewport = Signal::transient(0.);
    let measures = Rc::new(Measures {
      version: Signal::transient(0),
      axis,
    });
    let (count, edges, install) = match items {
      Some(Items {
        count,
        edges,
        install,
      }) => (count, Some(edges), Some(install)),
      None => (Rc::new(|| 0_usize) as Rc<dyn Fn() -> usize>, None, None),
    };
    let edges: Option<Memo<Rc<Vec<f32>>>> = match (extent, edges) {
      (ItemExtent::Estimated(estimate), Some(edges)) => {
        let measures = Rc::clone(&measures);

        Some(Memo::new(move |_| Rc::new(edges(&measures, estimate))))
      }
      _ => None,
    };
    let slice: Memo<Slice> = Memo::new(move |_| {
      let count = count();
      let (offset, viewport) = (offset.get(), viewport.get());

      match (extent, &edges) {
        (ItemExtent::Estimated(_), Some(edges)) => {
          let edges = edges.get();
          let window = item_window_over(offset, viewport, &edges, overscan);

          Slice {
            window,
            count,
            leading: edges[window.start],
            trailing: edges[count] - edges[window.end],
          }
        }
        (ItemExtent::Fixed(length) | ItemExtent::Estimated(length), _) => {
          let window = item_window(offset, viewport, length, count, overscan);
          #[expect(
            clippy::cast_precision_loss,
            reason = "an item count stays far below the integers f32 holds exactly"
          )]
          let extent_of = |items: usize| items as f32 * length;

          Slice {
            window,
            count,
            leading: extent_of(window.start),
            trailing: extent_of(count - window.end),
          }
        }
      }
    });
    let mut list = LayoutBox::<P, PlainChild>::new(axis.layout())
      .handle_scroll(move |event: &ScrollEvent| offset.set(axis.offset(*event)))
      .add_child(rigid(
        axis.derive_sized(Container::<P>::new(), move || slice.get().leading),
      ));

    list.style_mut().overflow = Some(Overflow::ScrollIfNeeded.into());

    if let Some(install) = install {
      list = install(slice, Rc::clone(&measures), list);
    }

    let node = list
      .add_child(rigid(
        axis.derive_sized(Container::<P>::new(), move || slice.get().trailing),
      ))
      .build();

    let _observed = P::observe_size(node, Rc::new(move |size| viewport.set(axis.extent(size))));

    node
  }
}

/// `container` kept at its own extent in the list: a flex item that neither grows nor
/// shrinks, so the spacers and the items measure the whole content.
fn rigid<P: Platform>(mut container: Container<P>) -> Container<P> {
  container.box_style_mut().flex = Some(
    Flexibility {
      initial: Dimension::Auto,
      grow_factor: 0.,
      shrink_factor: 0.,
    }
    .into(),
  );

  container
}

/// A vertically scrolling column of items, only the slice in view plus an overscan
/// margin built at any moment; the rest is empty space.
///
/// Scrolls on its own overflow; a size comes from the box it is wrapped in. Items are
/// keyed, so scrolling reuses the views whose keys stay in the window, and an item
/// expresses its own reactivity as usual. The item height is fixed or estimated
/// ([`ItemExtent`]); until one is set every item builds.
#[must_use]
pub struct LazyColumn<P: Platform>(LazyList<P>);

impl<P: Platform> std::fmt::Debug for LazyColumn<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("LazyColumn").finish_non_exhaustive()
  }
}

impl<P: Platform> LazyColumn<P> {
  /// An empty column with an overscan of three items.
  #[inline(always)]
  pub const fn new() -> Self {
    Self(LazyList::new(ScrollAxis::Vertical))
  }

  /// Sets the height of every item, fixed or estimated, in the medium's unit.
  pub fn set_item_height(mut self, item_height: impl Into<ItemExtent>) -> Self {
    self.0.extent = item_height.into();

    self
  }

  /// Sets how many items build beyond each edge of the slice in view.
  #[inline(always)]
  pub const fn set_overscan(mut self, overscan: usize) -> Self {
    self.0.overscan = overscan;

    self
  }

  /// Derives the items from a keyed collection: `items` yields them all, `key` gives
  /// each a stable identity, and `view` draws one; only those in the window build.
  pub fn derive_items<T, I, K, V>(
    self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    T: 'static,
    I: IntoIterator<Item = T>,
    I::IntoIter: ExactSizeIterator,
    K: Eq + Hash + Clone + 'static,
    V: View<P>,
  {
    Self(self.0.derive_items(items, key, view))
  }
}

impl<P: Platform> Default for LazyColumn<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> AnyKind for LazyColumn<P> {}

impl<P: Platform> View<P> for LazyColumn<P> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}

/// A horizontally scrolling row of items, only the slice in view plus an overscan
/// margin built at any moment; the rest is empty space.
///
/// Scrolls on its own overflow; a size comes from the box it is wrapped in. Items are
/// keyed, so scrolling reuses the views whose keys stay in the window, and an item
/// expresses its own reactivity as usual. The item width is fixed or estimated
/// ([`ItemExtent`]); until one is set every item builds.
#[must_use]
pub struct LazyRow<P: Platform>(LazyList<P>);

impl<P: Platform> std::fmt::Debug for LazyRow<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("LazyRow").finish_non_exhaustive()
  }
}

impl<P: Platform> LazyRow<P> {
  /// An empty row with an overscan of three items.
  #[inline(always)]
  pub const fn new() -> Self {
    Self(LazyList::new(ScrollAxis::Horizontal))
  }

  /// Sets the width of every item, fixed or estimated, in the medium's unit.
  pub fn set_item_width(mut self, item_width: impl Into<ItemExtent>) -> Self {
    self.0.extent = item_width.into();

    self
  }

  /// Sets how many items build beyond each edge of the slice in view.
  #[inline(always)]
  pub const fn set_overscan(mut self, overscan: usize) -> Self {
    self.0.overscan = overscan;

    self
  }

  /// Derives the items from a keyed collection: `items` yields them all, `key` gives
  /// each a stable identity, and `view` draws one; only those in the window build.
  pub fn derive_items<T, I, K, V>(
    self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    T: 'static,
    I: IntoIterator<Item = T>,
    I::IntoIter: ExactSizeIterator,
    K: Eq + Hash + Clone + 'static,
    V: View<P>,
  {
    Self(self.0.derive_items(items, key, view))
  }
}

impl<P: Platform> Default for LazyRow<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> AnyKind for LazyRow<P> {}

impl<P: Platform> View<P> for LazyRow<P> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}

/// What a facade's [`LazyColumn`] carries: its constructor, item height, overscan and items.
pub trait LazyColumnBase<P: Platform>: Wraps<Inner = LazyColumn<P>> {
  /// An empty column with an overscan of three items.
  #[must_use]
  fn new() -> Self {
    Self::wrap(LazyColumn::new())
  }

  /// Sets the height of every item, fixed or estimated, in the medium's unit.
  #[must_use]
  fn set_item_height(self, item_height: impl Into<ItemExtent>) -> Self {
    Self::wrap(self.into_inner().set_item_height(item_height))
  }

  /// Sets how many items build beyond each edge of the slice in view.
  #[must_use]
  fn set_overscan(self, overscan: usize) -> Self {
    Self::wrap(self.into_inner().set_overscan(overscan))
  }

  /// Derives the items from a keyed collection: `items` yields them all, `key` gives
  /// each a stable identity, and `view` draws one; only those in the window build.
  #[must_use]
  fn derive_items<T, I, K, V>(
    self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    T: 'static,
    I: IntoIterator<Item = T>,
    I::IntoIter: ExactSizeIterator,
    K: Eq + Hash + Clone + 'static,
    V: View<P>,
  {
    Self::wrap(self.into_inner().derive_items(items, key, view))
  }
}

/// What a facade's [`LazyRow`] carries: its constructor, item width, overscan and items.
pub trait LazyRowBase<P: Platform>: Wraps<Inner = LazyRow<P>> {
  /// An empty row with an overscan of three items.
  #[must_use]
  fn new() -> Self {
    Self::wrap(LazyRow::new())
  }

  /// Sets the width of every item, fixed or estimated, in the medium's unit.
  #[must_use]
  fn set_item_width(self, item_width: impl Into<ItemExtent>) -> Self {
    Self::wrap(self.into_inner().set_item_width(item_width))
  }

  /// Sets how many items build beyond each edge of the slice in view.
  #[must_use]
  fn set_overscan(self, overscan: usize) -> Self {
    Self::wrap(self.into_inner().set_overscan(overscan))
  }

  /// Derives the items from a keyed collection: `items` yields them all, `key` gives
  /// each a stable identity, and `view` draws one; only those in the window build.
  #[must_use]
  fn derive_items<T, I, K, V>(
    self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    T: 'static,
    I: IntoIterator<Item = T>,
    I::IntoIter: ExactSizeIterator,
    K: Eq + Hash + Clone + 'static,
    V: View<P>,
  {
    Self::wrap(self.into_inner().derive_items(items, key, view))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn windows_over_edges_follow_the_measured_items() {
    let edges = [0.0, 20.0, 40.0, 60.0, 80.0, 100.0, 110.0, 120.0];

    assert_eq!(
      item_window_over(0.0, 50.0, &edges, 0),
      ItemWindow { start: 0, end: 3 }
    );
    assert_eq!(
      item_window_over(40.0, 50.0, &edges, 0),
      ItemWindow { start: 2, end: 5 }
    );
    assert_eq!(
      item_window_over(40.0, 50.0, &edges, 1),
      ItemWindow { start: 1, end: 6 }
    );
    assert_eq!(
      item_window_over(500.0, 50.0, &edges, 2),
      ItemWindow { start: 5, end: 7 }
    );
    assert_eq!(
      item_window_over(0.0, 50.0, &[0.0], 3),
      ItemWindow { start: 0, end: 0 }
    );
  }

  #[test]
  fn windows_cover_the_viewport_with_overscan() {
    let window = item_window(100.0, 50.0, 10.0, 100, 2);

    assert_eq!(window.start, 8);
    assert_eq!(window.end, 17);

    let top = item_window(0.0, 50.0, 10.0, 100, 2);

    assert_eq!(top.start, 0);
    assert_eq!(top.end, 7);

    let clamped = item_window(990.0, 50.0, 10.0, 100, 2);

    assert_eq!(clamped.end, 100);
    assert_eq!(clamped.start, 97);
  }

  #[test]
  fn degenerate_inputs_stay_in_bounds() {
    assert_eq!(item_window(0.0, 0.0, 10.0, 5, 1).len(), 1);
    assert_eq!(item_window(0.0, 100.0, 0.0, 5, 1).len(), 5);
    assert_eq!(item_window(1000.0, 100.0, 10.0, 3, 1).start, 3);
    assert!(item_window(0.0, 10.0, 5.0, 0, 3).is_empty());
  }

  #[cfg(not(target_arch = "wasm32"))]
  use proptest::prelude::*;

  /// Whether the item `[from, to)` shows in a viewport of `viewport` at `offset`.
  #[cfg(not(target_arch = "wasm32"))]
  fn shows(from: f32, to: f32, offset: f32, viewport: f32) -> bool {
    to > from && viewport > 0.0 && from < offset + viewport && to > offset
  }

  #[cfg(not(target_arch = "wasm32"))]
  proptest! {
    /// Over any items of measured extents, the window stays in bounds and takes every
    /// item a positive viewport shows.
    #[test]
    fn a_window_over_edges_takes_every_item_shown(
      lengths in proptest::collection::vec(0..20_u8, 0..40),
      offset in 0..800_u16,
      viewport in 0..200_u16,
      overscan in 0..4_usize,
    ) {
      let mut edges = vec![0.0_f32];

      for length in &lengths {
        edges.push(edges.last().copied().unwrap_or(0.0) + f32::from(*length));
      }

      let (offset, viewport) = (f32::from(offset), f32::from(viewport));
      let window = item_window_over(offset, viewport, &edges, overscan);

      prop_assert!(window.start <= window.end && window.end <= lengths.len(), "{window:?}");

      for item in 0..lengths.len() {
        if shows(edges[item], edges[item + 1], offset, viewport) {
          prop_assert!(window.start <= item && item < window.end, "{item} outside {window:?}");
        }
      }
    }

    /// Over items of one whole extent, the window over their edges is the window by the
    /// extent, and takes every item a positive viewport shows.
    #[test]
    fn a_uniform_window_takes_every_item_shown(
      extent in 1..20_u8,
      count in 0..40_usize,
      offset in 0..800_u16,
      viewport in 0..200_u16,
      overscan in 0..4_usize,
    ) {
      let extent = f32::from(extent);
      #[expect(clippy::cast_precision_loss, reason = "a test's item count is small")]
      let edges: Vec<f32> = (0..=count).map(|item| item as f32 * extent).collect();
      let (offset, viewport) = (f32::from(offset), f32::from(viewport));
      let window = item_window(offset, viewport, extent, count, overscan);

      prop_assert!(window.start <= window.end && window.end <= count, "{window:?}");

      for item in 0..count {
        if shows(edges[item], edges[item + 1], offset, viewport) {
          prop_assert!(window.start <= item && item < window.end, "{item} outside {window:?}");
        }
      }

      if viewport > 0.0 && offset < edges[count] {
        prop_assert_eq!(window, item_window_over(offset, viewport, &edges, overscan));
      }
    }
  }
}
