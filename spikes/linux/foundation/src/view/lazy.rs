//! Lazy lists: a scroll box whose content realizes only the items in view and a few beyond,
//! windowed by the foundation's engine ([`crate::lazy::engine::Engine`]) on every platform.
//!
//! A lazy list is a `Scroll` node around a content grid: the realized lines are its tracks, the
//! unrealized ones before the first realized line its leading padding, those after the last its
//! trailing padding, and those between two runs of realized lines (a row holding focus
//! scrolled far away) a track of their extent. Each item sits in a box of its own (the list's
//! `Item` part, [`LazyBase::set_item_builder`]), a reader's list item reporting its place in
//! the whole list, the scroll box the list reporting the whole count.

use std::{cell::RefCell, hash::Hash, rc::Rc};

use crate::{
  Alignment, AxisAlignment, Effect, GridPlacement, GridTrack, Inset, NodeId, Owner, Signal,
  Spacing, State, StateSet, Tracks, Value,
  layout::Axis,
  lazy::{
    GridCells, ItemExtent, Key, KeyMap, KeySet, ScrollPosition,
    engine::{Anchor, Engine},
  },
  seam::{
    Realization,
    access::{AccessFacts, Role},
    content::{ScrollOffset, contents},
    node::{LayoutKind, NodeKind, NodeSpec},
    record::{Record, keys},
    tree,
  },
  untrack,
  value::GridLine,
  view::{
    AnyKind, AnyView, ChildOf, Container, ContainerBase, Cx, HasRecord, Node, NodeHandle, Scroll,
    ScrollAxes, ScrollBase, View, Wraps, trait_properties, wraps_itself,
  },
};

/// The items of one of a list's segments, by their place in it.
trait Items<R: Realization> {
  /// How many, as last read.
  fn len(&self) -> usize;

  /// The key of the item at `index`.
  fn key(&self, index: usize) -> Key;

  /// The view of the item at `index`, where it can be made.
  fn view(&self, index: usize) -> Option<AnyView<R>>;

  /// Whether each takes a line of its own in a grid.
  fn whole(&self) -> bool;

  /// Whether each counts among the list's items for a reader (a header does not).
  fn counted(&self) -> bool;

  /// Whether a built item is kept realized from then on, its view made once.
  fn kept(&self) -> bool;
}

/// A segment of one item: a single item or a header, its view made once.
struct One<R: Realization> {
  view: RefCell<Option<AnyView<R>>>,
  key: Key,
  header: bool,
}

impl<R: Realization> Items<R> for One<R> {
  fn len(&self) -> usize {
    1
  }

  fn key(&self, _: usize) -> Key {
    self.key
  }

  fn view(&self, _: usize) -> Option<AnyView<R>> {
    self.view.borrow_mut().take()
  }

  fn whole(&self) -> bool {
    true
  }

  fn counted(&self) -> bool {
    !self.header
  }

  fn kept(&self) -> bool {
    true
  }
}

/// What a segment's closure last returned, with the items' keys.
type Kept<T> = (Rc<[T]>, Rc<[Key]>);

/// A segment of items read from the app's data: what its closure last returned, with their
/// keys, read once per change of what it reads.
struct Derived<T, C> {
  kept: RefCell<Kept<T>>,
  view: Rc<dyn Fn(T) -> C>,
}

impl<R: Realization, T: Clone + 'static, C: View<R>> Items<R> for Derived<T, C> {
  fn len(&self) -> usize {
    self.kept.borrow().0.len()
  }

  fn key(&self, index: usize) -> Key {
    self.kept.borrow().1[index]
  }

  fn view(&self, index: usize) -> Option<AnyView<R>> {
    let item = self.kept.borrow().0.get(index)?.clone();

    Some((self.view)(item).into_any())
  }

  fn whole(&self) -> bool {
    false
  }

  fn counted(&self) -> bool {
    true
  }

  fn kept(&self) -> bool {
    false
  }
}

/// What starts a derived segment's effect, given the segment's place and the signal a change
/// bumps: the items it keeps.
type Start<R> = Box<dyn FnOnce(usize, Signal<u64>) -> Rc<dyn Items<R>>>;

/// What a list's segment is before it is built.
enum Segment<R: Realization> {
  /// One item, or a header.
  One { view: AnyView<R>, header: bool },
  /// Items from the app's data: what starts the effect keeping them, given the segment's
  /// place and the signal a change bumps.
  Derived(Start<R>),
}

/// What makes the box each item sits in.
type ItemBuilder<R> = Rc<dyn Fn() -> Container<R>>;

/// What realizes a built list's item by its place, or keeps one realized (or none).
type Realize = Rc<dyn Fn(Option<usize>, bool)>;

/// What reaches a lazy list from outside it once built: an item realized by its place where
/// it is not, as a list box moving focus to an option it has not built asks, or kept realized
/// wherever the list scrolls, as a list box keeps its tab stop ([`LazyBase::set_lazy_handle`]).
#[derive(Clone, Default)]
pub struct LazyHandle {
  realize: Rc<RefCell<Option<Realize>>>,
}

impl LazyHandle {
  /// A handle no list holds yet.
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Realizes the item at `index` (its place among the list's items and headers) where it is
  /// not realized, wherever the list is scrolled, keeping it realized until focus lands in
  /// it or another is realized this way: nothing before the list is built.
  pub fn realize(&self, index: usize) {
    let realize = self.realize.borrow().clone();

    if let Some(realize) = realize {
      realize(Some(index), false);
    }
  }

  /// Keeps the item at `index` realized wherever the list is scrolled, in place of the one
  /// kept before; none where `None`. Nothing before the list is built.
  pub fn keep(&self, index: Option<usize>) {
    let realize = self.realize.borrow().clone();

    if let Some(realize) = realize {
      realize(index, true);
    }
  }
}

/// A lazy list: a scroll box realizing only the items in view, along `axis`, its lines one item
/// each or, in a grid, as many as its [`GridCells`] fit. The facades' `LazyColumn`, `LazyRow`,
/// `LazyVerticalGrid` and `LazyHorizontalGrid` are this view.
#[must_use]
pub struct Lazy<R: Realization> {
  axis: Axis,
  cells: Option<GridCells>,
  extent: Rc<dyn Fn() -> ItemExtent>,
  overscan: usize,
  spacing: Rc<dyn Fn() -> Spacing>,
  padding: Rc<dyn Fn() -> Inset>,
  item_alignment: AxisAlignment,
  label: Option<Rc<dyn Fn() -> Rc<str>>>,
  labelled_by: Vec<NodeHandle>,
  position: Option<Rc<dyn Fn() -> ScrollPosition>>,
  on_scroll: Option<Rc<dyn Fn(ScrollPosition)>>,
  item_builder: Option<ItemBuilder<R>>,
  bare: bool,
  scroll: Scroll<R>,
  handle: Option<NodeHandle>,
  lazy_handle: Option<LazyHandle>,
  segments: Vec<Segment<R>>,
}

impl<R: Realization> Lazy<R> {
  /// An empty list along `axis`, one item a line, or as many as `cells` fit in a grid.
  pub fn new(axis: Axis, cells: Option<GridCells>) -> Self {
    Self {
      axis,
      cells,
      extent: Rc::new(|| ItemExtent::Estimated(48.)),
      overscan: 2,
      spacing: Rc::new(Spacing::default),
      padding: Rc::new(Inset::default),
      item_alignment: AxisAlignment::Stretch,
      label: None,
      labelled_by: Vec::new(),
      position: None,
      on_scroll: None,
      item_builder: None,
      bare: false,
      scroll: Scroll::new(match axis {
        Axis::Vertical => ScrollAxes::Vertical,
        Axis::Horizontal => ScrollAxes::Horizontal,
      }),
      handle: None,
      lazy_handle: None,
      segments: Vec::new(),
    }
  }

  /// This list as a control's: each item's own node is its row (a list box's option), with no
  /// box around it, and the list and its scroll box stand for their rows to a reader, the
  /// control reporting the count.
  pub fn bare(mut self) -> Self {
    self.bare = true;

    self
  }

  /// The axis it scrolls along.
  #[must_use]
  pub const fn axis(&self) -> Axis {
    self.axis
  }

  /// This list with its lines holding as many items as `cells` fit: a grid.
  pub const fn with_cells(mut self, cells: GridCells) -> Self {
    self.cells = Some(cells);

    self
  }
}

wraps_itself!(Lazy<R> where R: Realization);

impl<R: Realization> HasRecord for Lazy<R> {
  #[inline(always)]
  fn record_mut(&mut self) -> &mut Record {
    self.scroll.record_mut()
  }
}

/// What every lazy list carries: its extents, its spacing and padding, how many lines it
/// realizes beyond the viewport, its name, its scroll position, the box each item sits in, its
/// sizing as a scroll box, and its items, last.
pub trait LazyBase<R: Realization>: Wraps<Inner = Lazy<R>> {
  trait_properties! {
    /// The width of the list's box.
    set_width, derive_width: WIDTH, crate::Dimension;
    /// The height of the list's box.
    set_height, derive_height: HEIGHT, crate::Dimension;
    /// The least width of the list's box.
    set_min_width, derive_min_width: MIN_WIDTH, crate::Dimension;
    /// The least height of the list's box.
    set_min_height, derive_min_height: MIN_HEIGHT, crate::Dimension;
    /// The greatest width of the list's box.
    set_max_width, derive_max_width: MAX_WIDTH, crate::Dimension;
    /// The greatest height of the list's box.
    set_max_height, derive_max_height: MAX_HEIGHT, crate::Dimension;
  }

  /// The space between neighboring items, in pixels: between lines along the list's axis,
  /// between a grid's cells across it.
  fn set_spacing(self, spacing: impl Into<Spacing>) -> Self {
    let spacing = spacing.into();

    self.derive_spacing(move || spacing)
  }

  /// The space between neighboring items, as `spacing` returns it.
  ///
  /// Derived: recomputed whenever a signal the closure reads changes.
  fn derive_spacing<S: Into<Spacing>>(mut self, spacing: impl Fn() -> S + 'static) -> Self {
    self.inner_mut().spacing = Rc::new(move || spacing().into());

    self
  }

  /// The space inside the scrolled content, around its items, in pixels (Compose's
  /// `contentPadding`).
  fn set_padding(self, padding: impl Into<Inset>) -> Self {
    let padding = padding.into();

    self.derive_padding(move || padding)
  }

  /// The space inside the scrolled content, as `padding` returns it.
  ///
  /// Derived: recomputed whenever a signal the closure reads changes.
  fn derive_padding<I: Into<Inset>>(mut self, padding: impl Fn() -> I + 'static) -> Self {
    self.inner_mut().padding = Rc::new(move || padding().into());

    self
  }

  /// Where the items sit across the list's axis; stretched across it where unset.
  fn set_item_alignment(mut self, alignment: AxisAlignment) -> Self {
    self.inner_mut().item_alignment = alignment;

    self
  }

  /// How many lines it realizes beyond each edge of its viewport: one at least, which a
  /// reader walking past an edge needs; two where unset.
  fn set_overscan(mut self, lines: usize) -> Self {
    self.inner_mut().overscan = lines.max(1);

    self
  }

  /// Names the list for a reader.
  fn set_label(self, label: impl Into<Rc<str>>) -> Self {
    let label = label.into();

    self.derive_label(move || Rc::clone(&label))
  }

  /// Names the list for a reader, kept current.
  fn derive_label<S: Into<Rc<str>>>(mut self, label: impl Fn() -> S + 'static) -> Self {
    self.inner_mut().label = Some(Rc::new(move || label().into()));

    self
  }

  /// Names the list for a reader by the text of the view `caption` is written into, placed
  /// anywhere.
  fn set_labelled_by(mut self, caption: &NodeHandle) -> Self {
    self.inner_mut().labelled_by.push(*caption);

    self
  }

  /// Scrolls the list to `position` once it is laid out.
  fn set_scroll_position(self, position: ScrollPosition) -> Self {
    self.derive_scroll_position(move || position)
  }

  /// Scrolls the list to the position `position` returns, kept current: each change scrolls
  /// it there.
  fn derive_scroll_position(mut self, position: impl Fn() -> ScrollPosition + 'static) -> Self {
    self.inner_mut().position = Some(Rc::new(position));

    self
  }

  /// Runs `scroll` with the list's position each time it changes, as a scroll or a change of
  /// the items moves it: its first item in view at the viewport's start.
  fn handle_scroll(mut self, scroll: impl Fn(ScrollPosition) + 'static) -> Self {
    self.inner_mut().on_scroll = Some(Rc::new(scroll));

    self
  }

  /// Keeps the list's position in `position` both ways: the list scrolls to what it holds,
  /// and each scroll writes it.
  fn bind_scroll_position(self, position: Signal<ScrollPosition>) -> Self {
    self
      .derive_scroll_position(move || position.get())
      .handle_scroll(move |now| {
        if position.get_untracked() != now {
          position.set(now);
        }
      })
  }

  /// Makes the box each item sits in (the list's `Item` part) with `builder`: a library's rows
  /// padded and separated; a box that draws nothing where unset.
  fn set_item_builder<C: Wraps<Inner = Container<R>>>(
    mut self,
    builder: impl Fn() -> C + 'static,
  ) -> Self {
    self.inner_mut().item_builder = Some(Rc::new(move || builder().into_inner()));

    self
  }

  /// Writes the list's scroll box into `handle` once built.
  fn set_handle(mut self, handle: &NodeHandle) -> Self {
    self.inner_mut().handle = Some(*handle);

    self
  }

  /// Lets `handle` reach the list once built: realize an item by its place.
  fn set_lazy_handle(mut self, handle: &LazyHandle) -> Self {
    self.inner_mut().lazy_handle = Some(handle.clone());

    self
  }

  /// Appends one item, keyed by its place among the list's segments: built when it first comes
  /// into view, and kept from then on.
  fn add_item(mut self, item: impl View<R>) -> Self {
    self.inner_mut().segments.push(Segment::One {
      view: item.into_any(),
      header: false,
    });

    self
  }

  /// Appends a header: an item that takes a line of its own in a grid and does not count among
  /// the list's items for a reader.
  fn add_header(mut self, header: impl View<R>) -> Self {
    self.inner_mut().segments.push(Segment::One {
      view: header.into_any(),
      header: true,
    });

    self
  }

  /// Appends one item per item `items` returns, each keyed by `key` and shown by `view`: the
  /// list runs `items` once per change of what it reads and keeps what it returned, so a
  /// scroll reads nothing of it; a row is built once per key while it is realized.
  ///
  /// # Panics
  ///
  /// In a debug build, where two items share a key.
  fn derive_items<T: Clone + 'static, K: Hash + 'static, C: ChildOf<Lazy<R>, R> + View<R>>(
    mut self,
    items: impl Fn() -> Vec<T> + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> C + 'static,
  ) -> Self {
    let view: Rc<dyn Fn(T) -> C> = Rc::new(view);

    self
      .inner_mut()
      .segments
      .push(Segment::Derived(Box::new(move |segment, changed| {
        let derived = Rc::new(Derived {
          kept: RefCell::new((Rc::from([]), Rc::from([]))),
          view,
        });
        let kept = Rc::clone(&derived);

        Effect::new(move || {
          let items: Rc<[T]> = items().into();

          untrack(|| {
            let keys: Rc<[Key]> = items
              .iter()
              .map(|item| Key::of(segment, &key(item)))
              .collect();

            *kept.kept.borrow_mut() = (items, keys);
            changed.update(|count| *count = count.wrapping_add(1));
          });
        });

        derived as Rc<dyn Items<R>>
      })));

    self
  }
}

impl<R: Realization> LazyBase<R> for Lazy<R> {}

/// What a lazy list carries beside its layout members: how long its items are along its axis.
pub trait LazyListBase<R: Realization>: LazyBase<R> {
  /// How long each item is along the list's axis (Flutter's `itemExtent`): fixed (`48`), or
  /// estimated and measured as it is realized (`ItemExtent::Estimated(56)`, the default at 48).
  fn set_item_extent(self, extent: impl Into<ItemExtent>) -> Self {
    let extent = extent.into();

    self.derive_item_extent(move || extent)
  }

  /// How long each item is along the list's axis, as `extent` returns it.
  ///
  /// Derived: recomputed whenever a signal the closure reads changes.
  fn derive_item_extent<E: Into<ItemExtent>>(mut self, extent: impl Fn() -> E + 'static) -> Self {
    self.inner_mut().extent = Rc::new(move || extent().into());

    self
  }
}

impl<R: Realization> LazyListBase<R> for Lazy<R> {}

impl<R: Realization> AnyKind for Lazy<R> {}

/// A realized item.
struct Row {
  owner: Owner,
  node: NodeId,
  index: Signal<usize>,
  position: Signal<Option<usize>>,
  cell: Signal<GridPlacement>,
}

/// What a list keeps between its windowings.
struct List<R: Realization> {
  engine: Engine,
  axis: Axis,
  cells: Option<GridCells>,
  overscan: usize,
  bare: bool,
  segments: Vec<Rc<dyn Items<R>>>,
  firsts: Vec<usize>,
  counted: Vec<usize>,
  rows: KeyMap<Row>,
  order: Vec<Key>,
  owner: Owner,
  inner: Cx<R>,
  item_builder: Option<ItemBuilder<R>>,
  viewport: Option<(f32, f32)>,
  shown: f32,
  anchor: Option<Anchor>,
  cross_spacing: f32,
  pins: KeyMap<StateSet>,
  kept: KeySet,
  transient: Option<Key>,
  stop: Option<Key>,
  no_stop: KeySet,
  items_seen: Option<u64>,
  pending_position: Option<ScrollPosition>,
  reported: Option<ScrollPosition>,
  content: Option<NodeId>,
}

/// The signals a list's windowing reads and writes.
#[derive(Clone, Copy)]
struct Wires {
  items: Signal<u64>,
  changed: Signal<u64>,
  viewport: Signal<Option<(f32, f32)>>,
  offset: Signal<ScrollOffset>,
  shown: Signal<f32>,
  main_tracks: Signal<Tracks>,
  cross_tracks: Signal<Tracks>,
  padding: Signal<Inset>,
  set_size: Signal<usize>,
}

impl<R: Realization> List<R> {
  /// The segment holding the global item `index`, and its place in it.
  fn locate(&self, index: usize) -> Option<(usize, usize)> {
    let segment = self
      .firsts
      .partition_point(|first| *first <= index)
      .checked_sub(1)?;

    (index - self.firsts[segment] < self.segments[segment].len())
      .then(|| (segment, index - self.firsts[segment]))
  }

  /// The place among the counted items of the global item `index`, where it counts.
  fn position(&self, index: usize) -> Option<usize> {
    let (segment, at) = self.locate(index)?;

    self.segments[segment]
      .counted()
      .then(|| self.counted[segment] + at)
  }

  /// Reads the segments' items again: their keys, the lines they make, and the counted ones.
  fn read_items(&mut self, per_line: usize) -> usize {
    let mut keys = Vec::new();
    let mut whole = Vec::new();
    let mut counted = 0;

    self.firsts.clear();
    self.counted.clear();

    for segment in &self.segments {
      self.firsts.push(keys.len());
      self.counted.push(counted);

      let length = segment.len();

      if segment.whole() {
        whole.extend(keys.len()..keys.len() + length);
      }

      if segment.counted() {
        counted += length;
      }

      keys.extend((0..length).map(|index| segment.key(index)));
    }

    self.engine.set_items(keys, whole, per_line);
    self.no_stop.clear();

    counted
  }

  /// The items a page of the viewport holds: what `Tab` looks ahead by.
  fn page(&self) -> usize {
    let lines = self.viewport.map_or(1, |(width, height)| {
      let main = match self.axis {
        Axis::Vertical => height,
        Axis::Horizontal => width,
      };

      ((f64::from(main) / self.engine.estimate().max(1.)).ceil() as usize).max(1)
    });

    lines * self.per_line()
  }

  /// How many items a line holds now.
  fn per_line(&self) -> usize {
    match (self.cells, self.viewport) {
      (None, _) => 1,
      (Some(GridCells::Count(count)), _) => usize::from(count.max(1)),
      (Some(cells), Some((width, height))) => {
        let spacing = self.engine.spacing() as f32;
        let across = match self.axis {
          Axis::Vertical => width,
          Axis::Horizontal => height,
        };

        cells.per_line(across, spacing)
      }
      (Some(_), None) => 1,
    }
  }
}

/// The content of a lazy list: the grid its realized items sit in, windowing them as its
/// scroll box's viewport and offset say.
struct Content<R: Realization> {
  list: Rc<RefCell<Option<List<R>>>>,
  segments: Vec<Segment<R>>,
  extent: Rc<dyn Fn() -> ItemExtent>,
  spacing: Rc<dyn Fn() -> Spacing>,
  padding: Rc<dyn Fn() -> Inset>,
  scroll: NodeHandle,
  wires: Wires,
  axis: Axis,
  item_alignment: AxisAlignment,
  label: Option<Rc<dyn Fn() -> Rc<str>>>,
  labelled_by: Vec<NodeHandle>,
  bare: bool,
  cells: Option<GridCells>,
  overscan: usize,
  item_builder: Option<ItemBuilder<R>>,
}

impl<R: Realization> AnyKind for Content<R> {}

impl<R: Realization> View<R> for Content<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let Self {
      list,
      segments,
      extent,
      spacing,
      padding,
      scroll,
      wires,
      axis,
      item_alignment,
      label,
      labelled_by,
      bare,
      cells,
      overscan,
      item_builder,
    } = self;
    let id = cx.create(NodeSpec::new(NodeKind::Layout(LayoutKind::Grid)).with_role(Role::Generic));
    let mut record = Record::new();
    let (main, cross) = (wires.main_tracks, wires.cross_tracks);
    let (rows, columns) = match axis {
      Axis::Vertical => (main, cross),
      Axis::Horizontal => (cross, main),
    };
    let alignment = match axis {
      Axis::Vertical => Alignment {
        horizontal: item_alignment,
        vertical: AxisAlignment::Stretch,
      },
      Axis::Horizontal => Alignment {
        horizontal: AxisAlignment::Stretch,
        vertical: item_alignment,
      },
    };
    let gaps = Rc::clone(&spacing);

    record.set(keys::ROWS, Value::derived(move || rows.get()));
    record.set(keys::COLUMNS, Value::derived(move || columns.get()));
    record.set(keys::PADDING, Value::derived(move || wires.padding.get()));
    record.set(keys::SPACING, Value::derived(move || gaps()));
    record.set(keys::ALIGNMENT, alignment);
    cx.finish(id, record, None);

    // The scroll box is the list a reader finds: its viewport its bounds, its count the
    // whole list's.
    if !bare && let Some(scroll) = untrack(|| scroll.node()) {
      tree::add_access::<R>(scroll, move || AccessFacts {
        set_size: Some(wires.set_size.get()),
        name: label.as_ref().map(|label| label()),
        labelled_by: labelled_by.iter().filter_map(NodeHandle::node).collect(),
        ..AccessFacts::default()
      });
    }

    let owner = Owner::new();
    let inner = cx.child();
    let initial = extent();
    let built: Vec<Rc<dyn Items<R>>> = owner.run(|| {
      segments
        .into_iter()
        .enumerate()
        .map(|(at, segment)| match segment {
          Segment::One { view, header } => Rc::new(One {
            view: RefCell::new(Some(view)),
            key: Key::place(at),
            header,
          }) as Rc<dyn Items<R>>,
          Segment::Derived(start) => start(at, wires.items),
        })
        .collect()
    });

    *list.borrow_mut() = Some(List {
      engine: Engine::new(initial, 0., (0., 0.)),
      axis,
      cells,
      overscan,
      bare,
      segments: built,
      firsts: Vec::new(),
      counted: Vec::new(),
      rows: KeyMap::default(),
      order: Vec::new(),
      owner,
      inner,
      item_builder,
      viewport: None,
      shown: 0.,
      anchor: None,
      cross_spacing: 0.,
      pins: KeyMap::default(),
      kept: KeySet::default(),
      transient: None,
      stop: None,
      no_stop: KeySet::default(),
      items_seen: None,
      pending_position: None,
      reported: None,
      content: Some(id),
    });

    // The scroll box's viewport and offset, as the realization reports them.
    if let Some(scroll) = untrack(|| scroll.node()) {
      tree::handle_size::<R>(scroll, move |size| {
        let next = Some((size.width, size.height));

        if wires.viewport.get_untracked() != next {
          wires.viewport.set(next);
        }
      });
      tree::control::<R, _>(
        scroll,
        contents::SCROLL_OFFSET,
        move || wires.offset.get(),
        Some(Box::new(move |offset: ScrollOffset| {
          if wires.offset.get_untracked() != offset {
            wires.offset.set(offset);
          }
        })),
      );

      let sequential = Rc::clone(&list);

      tree::handle_sequential::<R>(scroll, move |from, backward| {
        look_ahead(&sequential, wires, from, backward)
      });
    }

    let window = Rc::clone(&list);

    Effect::new(move || {
      let extent = extent();
      let spacing = spacing();
      let padding = padding();
      let _ = (wires.items.get(), wires.changed.get());
      let viewport = wires.viewport.get();
      let offset = wires.offset.get();

      untrack(|| {
        reconcile(&window, wires, extent, spacing, padding, viewport, offset);
      });
    });

    Node::new(id)
  }
}

impl<R: Realization> View<R> for Lazy<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let Self {
      axis,
      cells,
      extent,
      overscan,
      spacing,
      padding,
      item_alignment,
      label,
      labelled_by,
      position,
      on_scroll,
      item_builder,
      bare,
      scroll,
      handle,
      lazy_handle,
      segments,
    } = self;
    let scroll_handle = handle.unwrap_or_default();
    let list: Rc<RefCell<Option<List<R>>>> = Rc::new(RefCell::new(None));
    let wires = Wires {
      items: Signal::new(0),
      changed: Signal::new(0),
      viewport: Signal::new(None),
      offset: Signal::new(ScrollOffset::default()),
      shown: Signal::new(0.),
      main_tracks: Signal::new(Tracks::default()),
      cross_tracks: Signal::new(Tracks::from([GridTrack::Fraction(1.)])),
      padding: Signal::new(Inset::default()),
      set_size: Signal::new(0),
    };
    let content = Content {
      list: Rc::clone(&list),
      segments,
      extent,
      spacing,
      padding,
      scroll: scroll_handle,
      wires,
      axis,
      item_alignment,
      label,
      labelled_by,
      bare,
      cells,
      overscan,
      item_builder,
    };

    cx.set_role(if bare { Role::Generic } else { Role::List });

    let node = scroll
      .set_handle(&scroll_handle)
      .set_child(content)
      .build(cx);

    wires.changed.update(|count| *count = count.wrapping_add(1));

    if let Some(lazy_handle) = lazy_handle {
      let list = Rc::clone(&list);

      *lazy_handle.realize.borrow_mut() = Some(Rc::new(move |index, keep| {
        let asked = list.borrow_mut().as_mut().is_some_and(|state| {
          let key = index.and_then(|index| state.engine.key(index));

          if keep {
            let moved = state.stop != key;

            state.stop = key;

            return moved && key.is_some_and(|key| !state.rows.contains_key(&key));
          }

          match key {
            Some(key) if !state.rows.contains_key(&key) => {
              state.transient = Some(key);

              true
            }
            _ => false,
          }
        });

        if asked {
          wires.changed.update(|count| *count = count.wrapping_add(1));
        }
      }));
    }

    if let Some(position) = position {
      let list = Rc::clone(&list);

      Effect::new(move || {
        let position = position();

        untrack(|| {
          let applied = list.borrow_mut().as_mut().is_some_and(|state| {
            if state.reported == Some(position) {
              return false;
            }

            state.pending_position = Some(position);

            true
          });

          if applied {
            wires.changed.update(|count| *count = count.wrapping_add(1));
          }
        });
      });
    }

    if let Some(on_scroll) = on_scroll {
      let list = Rc::clone(&list);

      Effect::new(move || {
        let _ = (wires.shown.get(), wires.changed.get(), wires.viewport.get());

        untrack(|| {
          let now = list.borrow_mut().as_mut().and_then(|state| {
            let viewport = state.viewport?;
            let main = along(state.axis, viewport);
            let anchor = state
              .engine
              .anchor(f64::from(state.shown), f64::from(main), None)?;
            let now = ScrollPosition {
              index: anchor.index,
              alignment: AxisAlignment::Start,
              offset: anchor.distance as f32,
            };

            (state.reported != Some(now)).then(|| {
              state.reported = Some(now);

              now
            })
          });

          if let Some(now) = now {
            on_scroll(now);
          }
        });
      });
    }

    node
  }
}

/// `size`'s length along `axis`.
const fn along(axis: Axis, size: (f32, f32)) -> f32 {
  match axis {
    Axis::Horizontal => size.0,
    Axis::Vertical => size.1,
  }
}

/// The states that keep a row realized wherever the list scrolls: focus inside it, and a press
/// held inside it.
const PINNING: StateSet = StateSet::EMPTY
  .with(State::FocusWithin)
  .with(State::Pressed);

/// Builds the row of the global item `index`, detached, under an owner of the list's: its box
/// (or the item's own node, in a bare list) placed in its cell, measured where its extent is
/// estimated, pinned while it holds focus or a press, and telling a reader its place.
fn build_row<R: Realization>(
  list: &Rc<RefCell<Option<List<R>>>>,
  wires: Wires,
  index: usize,
) -> Option<(Key, Row)> {
  let (key, items, at, owner, mut cx, builder, bare, axis, position) = {
    let mut borrowed = list.borrow_mut();
    let state = borrowed.as_mut()?;
    let (segment, at) = state.locate(index)?;
    let items = Rc::clone(&state.segments[segment]);
    let key = items.key(at);

    if items.kept() {
      state.kept.insert(key);
    }

    (
      key,
      items,
      at,
      state.owner,
      state.inner.child(),
      state.item_builder.clone(),
      state.bare,
      state.axis,
      state.position(index),
    )
  };
  // The row's view is made under its own owner too, so what making it creates (a control's
  // state) lives as long as the row.
  let row_owner = owner.run(Owner::new);
  let view = row_owner.run(|| items.view(at));
  let Some(view) = view else {
    row_owner.dispose();

    return None;
  };
  let row = {
    let owner = row_owner;

    owner.run(|| {
      let index_signal = Signal::new(index);
      let position_signal = Signal::new(position);
      let cell = Signal::new(GridPlacement::at(1, 1));
      let mut placement = Record::new();

      placement.set(keys::CELL, Value::derived(move || cell.get()));
      cx.fold_placement(&placement);

      let node = if bare {
        view.build(&mut cx).id()
      } else {
        cx.set_role(Role::ListItem);

        let item = builder
          .as_ref()
          .map_or_else(Container::new, |builder| builder());

        item.set_child(view).build(&mut cx).id()
      };

      // Its measure along the list's axis, while its extent is estimated.
      let measure = Rc::clone(list);

      tree::handle_size::<R>(node, move |size| {
        let length = along(axis, (size.width, size.height));
        let changed = measure.borrow_mut().as_mut().is_some_and(|state| {
          let index = state
            .engine
            .index_of(key, Some(index_signal.get_untracked()));

          index.is_some_and(|index| state.engine.measure(index, length))
        });

        if changed {
          wires.changed.update(|count| *count = count.wrapping_add(1));
        }
      });

      // Pinned while focus or a press is inside it.
      tree::watch_states::<R>(node, PINNING);

      let pins = Rc::clone(list);

      Effect::new(move || {
        let held = tree::reported_states::<R>(node).intersection(PINNING);

        untrack(|| {
          let moved = pins.borrow_mut().as_mut().is_some_and(|state| {
            let was = state.pins.get(&key).copied().unwrap_or(StateSet::EMPTY);

            if held.contains(State::FocusWithin) && state.transient == Some(key) {
              state.transient = None;
            }

            if was == held {
              return false;
            }

            if held.is_empty() {
              state.pins.remove(&key);
            } else {
              state.pins.insert(key, held);
            }

            // A row that loses its pins may leave; one that gains them stays.
            !held.is_empty() != !was.is_empty()
          });

          if moved {
            wires.changed.update(|count| *count = count.wrapping_add(1));
          }
        });
      });

      if !bare {
        tree::add_access::<R>(node, move || AccessFacts {
          position_in_set: position_signal.get(),
          ..AccessFacts::default()
        });
      }

      Row {
        owner,
        node,
        index: index_signal,
        position: position_signal,
        cell,
      }
    })
  };

  Some((key, row))
}

/// Removes a row for good: its owner disposed, its nodes removed.
fn drop_row<R: Realization>(row: Row) {
  row.owner.dispose();
  tree::remove::<R>(row.node);
}

/// Brings the realized rows to the window the viewport and the offset show, with every pinned
/// row; corrects the offset so the anchor stays where it was in the viewport after a change of
/// the items or their extents; and lays the realized lines out in the content's grid.
fn reconcile<R: Realization>(
  list: &Rc<RefCell<Option<List<R>>>>,
  wires: Wires,
  extent: ItemExtent,
  spacing: Spacing,
  padding: Inset,
  viewport: Option<(f32, f32)>,
  offset: ScrollOffset,
) {
  // What the window needs, from the state as it stands.
  let (needed, stale, corrected) = {
    let mut borrowed = list.borrow_mut();
    let Some(state) = borrowed.as_mut() else {
      return;
    };
    let axis = state.axis;
    let (main_spacing, cross_spacing) = match axis {
      Axis::Vertical => (spacing.vertical, spacing.horizontal),
      Axis::Horizontal => (spacing.horizontal, spacing.vertical),
    };
    let (start, end) = match axis {
      Axis::Vertical => (padding.top, padding.bottom),
      Axis::Horizontal => (padding.leading, padding.trailing),
    };

    state.viewport = viewport;
    state.cross_spacing = cross_spacing;

    let per_line = state.per_line();

    state.engine.set_extent(extent);
    state
      .engine
      .set_layout(main_spacing, (start, end), per_line);

    let items = wires.items.get_untracked();

    if state.items_seen != Some(items) {
      state.items_seen = Some(items);

      let counted = state.read_items(per_line);

      if wires.set_size.get_untracked() != counted {
        wires.set_size.set(counted);
      }
    }

    let main = viewport.map(|size| along(axis, size));
    let moved_by_reader = offset.along(axis) != state.shown;
    let mut at = f64::from(offset.along(axis));

    // A position the app asked for, once the viewport is known.
    if let Some(main) = main
      && let Some(position) = state.pending_position.take()
    {
      at = state.engine.offset_of(
        position.index,
        position.fraction(),
        f64::from(position.offset),
        f64::from(main),
      );
      state.anchor = None;
    } else if !moved_by_reader
      && state.shown > 0.
      && let Some(anchor) = state.anchor
      && let Some(anchored) = state.engine.anchored(anchor)
    {
      at = anchored;
    }

    if let Some(main) = main {
      at = at.clamp(0., (state.engine.total() - f64::from(main)).max(0.));
    } else {
      at = at.max(0.);
    }

    let lines = match main {
      Some(main) => state.engine.window(at, f64::from(main), state.overscan),
      None => 0..state.overscan.min(state.engine.line_count()),
    };
    let mut needed: Vec<usize> = state.engine.items_of_lines(lines).collect();

    for key in state
      .pins
      .keys()
      .chain(state.kept.iter())
      .chain(state.transient.iter())
      .chain(state.stop.iter())
    {
      let hint = state.rows.get(key).map(|row| row.index.get_untracked());

      if let Some(index) = state.engine.index_of(*key, hint) {
        // A pinned item's whole line stays.
        needed.extend(state.engine.items_of(state.engine.line_of(index)));
      }
    }

    needed.sort_unstable();
    needed.dedup();

    let wanted: KeySet = needed
      .iter()
      .filter_map(|index| state.engine.key(*index))
      .collect();
    let stale: Vec<Key> = state
      .rows
      .keys()
      .filter(|key| !wanted.contains(key))
      .copied()
      .collect();
    let stale: Vec<Row> = stale
      .into_iter()
      .filter_map(|key| {
        state.pins.remove(&key);
        state.rows.remove(&key)
      })
      .collect();
    let corrected = (at as f32 != offset.along(axis)).then(|| offset.with(axis, at as f32));

    state.shown = at as f32;

    (needed, stale, corrected)
  };

  for row in stale {
    drop_row::<R>(row);
  }

  // The rows to build, outside the borrow: building runs their effects.
  let missing: Vec<usize> = {
    let borrowed = list.borrow();
    let Some(state) = borrowed.as_ref() else {
      return;
    };

    needed
      .iter()
      .copied()
      .filter(|index| {
        state
          .engine
          .key(*index)
          .is_some_and(|key| !state.rows.contains_key(&key))
      })
      .collect()
  };
  let built: Vec<(Key, Row)> = missing
    .into_iter()
    .filter_map(|index| build_row(list, wires, index))
    .collect();

  // The realized lines laid out in the grid, and the rows attached in order.
  let (attach, tracks, cross, inset, content) = {
    let mut borrowed = list.borrow_mut();
    let Some(state) = borrowed.as_mut() else {
      return;
    };

    state.rows.extend(built);

    let axis = state.axis;
    let mut realized: Vec<(usize, Key)> = state
      .rows
      .iter()
      .filter_map(|(key, row)| {
        let index = state
          .engine
          .index_of(*key, Some(row.index.get_untracked()))?;

        Some((index, *key))
      })
      .collect();

    realized.sort_unstable();

    let fixed = match state.engine.extent() {
      ItemExtent::Fixed(length) => Some(length),
      ItemExtent::Estimated(_) => None,
    };
    let mut tracks: Vec<GridTrack> = Vec::new();
    let mut last_line: Option<usize> = None;
    let mut placements = Vec::with_capacity(realized.len());
    let per_line = state.per_line();
    let spacing = state.engine.spacing();

    for (index, key) in &realized {
      let line = state.engine.line_of(*index);

      if last_line != Some(line) {
        if let Some(previous) = last_line
          && line > previous + 1
        {
          let gap = state.engine.line_start(line) - state.engine.line_end(previous) - 2. * spacing;

          tracks.push(GridTrack::Length(gap.max(0.) as f32));
        }

        tracks.push(fixed.map_or(GridTrack::Auto, GridTrack::Length));
        last_line = Some(line);
      }

      let items = state.engine.items_of(line);
      let whole = items.len() == 1 && state.engine.line_count() > 0 && per_line > 1 && {
        state
          .locate(*index)
          .is_some_and(|(segment, _)| state.segments[segment].whole())
      };
      let track = i16::try_from(tracks.len()).unwrap_or(i16::MAX);
      let column = i16::try_from(index - items.start + 1).unwrap_or(i16::MAX);
      let (main_line, cross_line) = (
        GridLine {
          start: Some(track),
          span: 1,
        },
        GridLine {
          start: Some(if whole { 1 } else { column }),
          span: if whole {
            u16::try_from(per_line).unwrap_or(u16::MAX)
          } else {
            1
          },
        },
      );
      let placement = match axis {
        Axis::Vertical => GridPlacement {
          row: main_line,
          column: cross_line,
        },
        Axis::Horizontal => GridPlacement {
          row: cross_line,
          column: main_line,
        },
      };

      placements.push((*key, *index, placement));
    }

    let leading = realized.first().map_or(state.engine.total(), |(index, _)| {
      state.engine.line_start(state.engine.line_of(*index))
    });
    let trailing = realized.last().map_or(0., |(index, _)| {
      state.engine.total() - state.engine.line_end(state.engine.line_of(*index))
    });
    let inset = match axis {
      Axis::Vertical => Inset {
        top: leading as f32,
        bottom: trailing as f32,
        ..padding
      },
      Axis::Horizontal => Inset {
        leading: leading as f32,
        trailing: trailing as f32,
        ..padding
      },
    };
    let cross_length = state.viewport.map(|(width, height)| match axis {
      Axis::Vertical => width - padding.leading - padding.trailing,
      Axis::Horizontal => height - padding.top - padding.bottom,
    });
    let cross_spacing = state.cross_spacing;
    let cross = match (per_line, cross_length) {
      (1, _) => Tracks::from([GridTrack::Fraction(1.)]),
      (count, Some(length)) => {
        let cell = ((length - cross_spacing * (count - 1) as f32) / count as f32).max(0.);

        Tracks::from(vec![GridTrack::Length(cell); count])
      }
      (count, None) => Tracks::from(vec![GridTrack::Flex(1.); count]),
    };
    let mut attach = Vec::with_capacity(placements.len());

    for (key, index, placement) in placements {
      if let Some(row) = state.rows.get(&key) {
        if row.index.get_untracked() != index {
          row.index.set(index);
        }

        let position = state.position(index);

        if row.position.get_untracked() != position {
          row.position.set(position);
        }

        if row.cell.get_untracked() != placement {
          row.cell.set(placement);
        }

        attach.push(row.node);
      }
    }

    state.order = realized.iter().map(|(_, key)| *key).collect();

    let main = state.viewport.map(|size| along(axis, size));

    state.anchor = main.and_then(|main| {
      state
        .engine
        .anchor(f64::from(state.shown), f64::from(main), None)
    });

    (attach, Tracks::from(tracks), cross, inset, state.content)
  };

  if wires.main_tracks.get_untracked() != tracks {
    wires.main_tracks.set(tracks);
  }

  if wires.cross_tracks.get_untracked() != cross {
    wires.cross_tracks.set(cross);
  }

  if wires.padding.get_untracked() != inset {
    wires.padding.set(inset);
  }

  if let Some(content) = content {
    let present = tree::children::<R>(content);

    for (at, node) in attach.iter().enumerate() {
      if present.get(at) != Some(node) {
        tree::attach::<R>(content, *node, at);
      }
    }
  }

  if let Some(corrected) = corrected
    && wires.offset.get_untracked() != corrected
  {
    wires.offset.set(corrected);
  }

  let shown = list.borrow().as_ref().map(|state| state.shown);

  if let Some(shown) = shown
    && wires.shown.get_untracked() != shown
  {
    wires.shown.set(shown);
  }
}

/// The stop the sequential focus navigation goes to from `from` past the list's realized
/// rows, looking a page of items beyond them: each item without a stop built detached,
/// found wanting and dropped, and remembered so the next search skips it; the first with a
/// stop realized, pinned until focus lands in it.
fn look_ahead<R: Realization>(
  list: &Rc<RefCell<Option<List<R>>>>,
  wires: Wires,
  from: NodeId,
  backward: bool,
) -> Option<NodeId> {
  let candidates: Vec<usize> = {
    let borrowed = list.borrow();
    let state = borrowed.as_ref()?;
    let count = state.engine.count();
    let realized: Vec<usize> = state
      .rows
      .values()
      .map(|row| row.index.get_untracked())
      .collect();
    let first = realized.iter().copied().min()?;
    let last = realized.iter().copied().max()?;
    let holder = state
      .rows
      .values()
      .find(|row| tree::is_within::<R>(from, row.node))
      .map(|row| row.index.get_untracked());
    let page = state.page();

    match (holder, backward) {
      (Some(index), false) => ((index + 1).max(last + 1)..(last + 1 + page).min(count)).collect(),
      (Some(index), true) => (first.saturating_sub(page)..index.min(first))
        .rev()
        .collect(),
      (None, false) => ((last + 1)..(last + 1 + page).min(count)).collect(),
      (None, true) => Vec::new(),
    }
  };

  for index in candidates {
    let skip = list.borrow().as_ref().is_some_and(|state| {
      state
        .engine
        .key(index)
        .is_none_or(|key| state.no_stop.contains(&key))
    });

    if skip {
      continue;
    }

    let (key, row) = build_row(list, wires, index)?;
    let stop = tree::first_stop::<R>(row.node);

    match stop {
      Some(stop) => {
        if let Some(state) = list.borrow_mut().as_mut() {
          state.transient = Some(key);
          state.rows.insert(key, row);
        }

        wires.changed.update(|count| *count = count.wrapping_add(1));

        return Some(stop);
      }
      None => {
        drop_row::<R>(row);

        if let Some(state) = list.borrow_mut().as_mut() {
          state.no_stop.insert(key);
        }
      }
    }
  }

  None
}
