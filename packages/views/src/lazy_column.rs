//! A vertically scrolling list building only its visible slice.

use std::{hash::Hash, rc::Rc};

use skene_core::{
  Dimension,
  FlexDirection,
  ItemWindow,
  Layout,
  Overflow,
  ScrollEvent,
  item_window,
  next::{AnyKind, Driver, HasChildren, IntoView, NodeId, PlainChild, Scalar, View},
};
use skene_reactive::{Get, Memo, Set, Signal, batch};

use crate::{Container, LayoutBox, realize::with_sink};

/// The window over the items, and their count.
type Window = Memo<(ItemWindow, usize)>;

/// Installs the keyed segment over the window, given the item height, onto the list.
type Install<D> =
  Box<dyn FnOnce(Window, f32, LayoutBox<D, PlainChild>) -> LayoutBox<D, PlainChild>>;

/// What [`derive_items`](LazyColumn::derive_items) leaves for the build: the item
/// count, tracked, and the keyed segment installed over the window.
struct Items<D: Driver> {
  count: Rc<dyn Fn() -> usize>,
  install: Install<D>,
}

/// A scrolling column of fixed-height items, only the slice in view plus an overscan
/// margin built at any moment; the rest is empty space.
///
/// Scrolls on its own overflow; a size comes from the box it is wrapped in. Items are
/// keyed, so scrolling reuses the views whose keys stay in the window, and an item
/// expresses its own reactivity as usual. Until an item height is set every item
/// builds.
#[must_use]
pub struct LazyColumn<D: Driver> {
  item_height: f32,
  overscan: usize,
  items: Option<Items<D>>,
}

impl<D: Driver> LazyColumn<D> {
  /// An empty column with an overscan of three items.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      item_height: 0.,
      overscan: 3,
      items: None,
    }
  }

  /// Sets the height of every item, in the medium's unit.
  #[inline(always)]
  pub const fn set_item_height(mut self, item_height: f32) -> Self {
    self.item_height = item_height;

    self
  }

  /// Sets how many items build beyond each edge of the slice in view.
  #[inline(always)]
  pub const fn set_overscan(mut self, overscan: usize) -> Self {
    self.overscan = overscan;

    self
  }

  /// Derives the items from a keyed collection: `items` yields them all, `key` gives
  /// each a stable identity, and `view` draws one; only those in the window build.
  pub fn derive_items<T, I, K, V>(
    mut self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    T: 'static,
    I: IntoIterator<Item = T>,
    I::IntoIter: ExactSizeIterator,
    K: Eq + Hash + 'static,
    V: IntoView<D>,
  {
    let items = Rc::new(items);
    let count = {
      let items = Rc::clone(&items);

      Rc::new(move || items().into_iter().len())
    };
    let install = Box::new(
      move |window: Window, item_height: f32, list: LayoutBox<D, PlainChild>| {
        list.derive_children(
          move || {
            let (window, _) = window.get();

            items()
              .into_iter()
              .skip(window.start)
              .take(window.len())
              .collect::<Vec<T>>()
          },
          key,
          move |item| {
            Container::<D>::new()
              .set_height(Dimension::Length(D::Scalar::from_f32(item_height)))
              .set_child(view(item).into_view())
          },
        )
      },
    );

    self.items = Some(Items { count, install });

    self
  }
}

impl<D: Driver> Default for LazyColumn<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> AnyKind for LazyColumn<D> {}

impl<D: Driver> View<D> for LazyColumn<D> {
  fn build(self) -> NodeId {
    let Self {
      item_height,
      overscan,
      items,
    } = self;
    let offset = Signal::new(0.);
    let viewport = Signal::new(0.);
    let (count, install) = match items {
      Some(Items { count, install }) => (count, Some(install)),
      None => (Rc::new(|| 0_usize) as Rc<dyn Fn() -> usize>, None),
    };
    let window: Window = Memo::new(move |_| {
      let count = count();

      (
        item_window(offset.get(), viewport.get(), item_height, count, overscan),
        count,
      )
    });
    #[expect(clippy::cast_precision_loss)]
    let extent =
      move |items: usize| Dimension::Length(D::Scalar::from_f32(items as f32 * item_height));
    let mut list = LayoutBox::<D, PlainChild>::new(Layout::Flex {
      direction: FlexDirection::Column,
    })
    .handle_scroll(move |event: &ScrollEvent| offset.set(event.y))
    .set_child(Container::<D>::new().derive_height(move || extent(window.get().0.start)));

    list.style_mut().overflow = Some(Overflow::Auto.into());

    if let Some(install) = install {
      list = install(window, item_height, list);
    }

    let node = list
      .set_child(Container::<D>::new().derive_height(move || {
        let (window, count) = window.get();

        extent(count - window.end)
      }))
      .build();

    batch(|| {
      with_sink::<D, _>(node, |sink| {
        D::default().observe_size(
          sink,
          Rc::new(move |size| viewport.set(size.height.to_f32())),
        );
      });
    });

    node
  }
}
