//! Scrolling lists building only their visible slice: a column and a row.

use std::{cell::RefCell, collections::HashMap, hash::Hash, rc::Rc};

use skene_reactive::{Get, Memo, Set, Signal, Update};

use crate::{
  AnyKind,
  Container,
  Dimension,
  FlexDirection,
  Flexibility,
  HasChild,
  HasChildren,
  ItemWindow,
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
  item_window,
  item_window_over,
};

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
enum Axis {
  Horizontal,
  Vertical,
}

impl Axis {
  #[inline(always)]
  const fn direction(self) -> FlexDirection {
    match self {
      Self::Horizontal => FlexDirection::Row,
      Self::Vertical => FlexDirection::Column,
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
  axis: Axis,
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
  axis: Axis,
  extent: ItemExtent,
  overscan: usize,
  items: Option<Items<P>>,
}

impl<P: Platform> LazyList<P> {
  const fn new(axis: Axis) -> Self {
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
    let mut list = LayoutBox::<P, PlainChild>::new(Layout::Flex {
      direction: axis.direction(),
    })
    .handle_scroll(move |event: &ScrollEvent| offset.set(axis.offset(*event)))
    .add_child(rigid(
      axis.derive_sized(Container::<P>::new(), move || slice.get().leading),
    ));

    list.style_mut().overflow = Some(Overflow::Auto.into());

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
  container.style_mut().flex = Some(
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
    Self(LazyList::new(Axis::Vertical))
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
    Self(LazyList::new(Axis::Horizontal))
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
