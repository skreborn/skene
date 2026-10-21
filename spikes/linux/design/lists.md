# Lists that recycle their rows

The list half of `ROADMAP.md` item 4: lazy lists shaped once for both kinds of platform (the
compositor platforms, where Skene's views are the platform's views among native ones, and the
window platforms, where Skene draws everything), realized first on Linux in `spikes/linux`.
Written 2026-10-19 against `AGENTS.md` as of commit a31b959, then reviewed the same day against
`AGENTS.md`, the port to `packages/` and to every platform, and the cost at 10,000 and 100,000
rows. A list that chooses takes the typed choice `api-design.md` (beside this file) designs for
the picker and the radio group.

**Labels.** VERIFIED (ran): run in this stage, its transcript in appendix A or its probe named.
VERIFIED (read): read in a source at a stated version or commit. JUDGMENT: everything else,
every design decision among it; a decision is a proposal for the author until it lands.

**What the review changed.**

- Choosing moves from `LazyColumn` to `Listbox<C>`, a control whose options the same engine
  windows: a layout view stays a layout view, and a list box is typed by the value it holds, as
  a picker is.
- A list reads its source once per change and indexes it per frame, so a frame costs the window
  at 100,000 rows. Today's `derive_items` runs its closure on every scroll event, and again
  whenever the window moves.
- `Tab` past the realized rows looks one page ahead and then leaves the list, as it leaves a
  virtualized list on the web. The unbounded search built every row of a long text-only list on
  the first `Tab`: seconds at 100,000 rows.
- A header starts a section, which a reader finds as a set of its own, so the web's ARIA stays
  valid. Whether headers stick is a property of the list.
- `ScrollPosition` is by index, with no type parameter; the engine still anchors by key.
- An unmeasured item counts at an estimate re-based only while the list is at rest, and
  hydration reads the rows it removes as two blocks rather than one read per row.
- Android keeps the platform's physics inside a scroll container of Skene's own, as
  `RecyclerView` does. Compose and Flutter scroll themselves, so they are no precedent for
  keeping the platform's scroll view.
- Section 12 states the cost of every operation at 10,000 and 100,000 rows.

## 1. The decisions in brief

1. **Four lazy views, plain lists.** `LazyColumn`, `LazyRow`, `LazyVerticalGrid` and
   `LazyHorizontalGrid` on both facades, name for name, in pixels on `skene-gui` and in cells on
   `skene-tui`, with no type parameter. Items arrive last, as children do: `add_item`,
   `add_header`, `derive_items(items, key, view)` and `derive_entries(location, view)`. A lazy
   list reports itself to a reader as a list and chooses nothing.
2. **A list that chooses is a `Listbox<C>`.** It is a control (`Control<behavior::Listbox<C>>`,
   styled in each facade), its options windowed by the same engine. Its one type parameter is
   the value it holds, its choice: one value or none (`Option<T>`) or several (`HashSet<T>`,
   `BTreeSet<T>`), as SwiftUI's `List(selection:)` takes, each option carrying a `T` of the
   app's own type as a picker's options do. Its choice is a set of values, never of rows, so a
   range and select-all cover options that are not built.
3. **One windowing engine in the foundation, on every platform.** Which items are realized,
   their extents, the anchor, the pins, focus beyond the window and the reader's requests are
   written once in `skene-foundation`, over the seam. No platform's recycler realizes it:
   `RecyclerView`, `UICollectionView`, `NSTableView` and the browser are not the engine. Each
   platform keeps its own scrolling physics, its bars or indicators, its nested scrolling and the
   reader's scroll gestures.
4. **A source is read once per change and indexed per frame.** A list keeps what its source
   last gave (the items of `derive_items`, the keys of a store location), and a frame indexes
   what it kept, so moving the window costs the window whatever the count. A change of the items
   costs their count, once.
5. **A row is built once per key while it is realized, and disposed when it leaves.** State a row
   must keep while it is away is opted in, the way reload state is, and kept by key until the key
   returns, as Compose's `rememberSaveable` keeps it in a `SaveableStateHolder`. A row's scroll
   offsets and caret come back the same way. Recycling happens in the realizations, which pool
   toolkit objects per node kind. A row's reactive build is never handed to another item.
6. **Every row is measured or of a fixed extent.** `ItemExtent::Fixed(length)` gives every row
   that extent and measures nothing. The default, `ItemExtent::Estimated(length)`, measures each
   row as it is realized and remembers the measure by key. An unmeasured item counts at the
   estimate in force, which moves to the mean of the measured rows only while the list is at
   rest. Offsets are prefix sums over the measured differences: locating an item costs O(log m)
   in the m items ever measured.
7. **Content never moves under the reader.** After any change of extent or data above the
   viewport, the anchor (the focused editable row where one is visible, else the first visible
   item) keeps its distance to the viewport's leading edge, in the same frame, before paint, on
   every platform.
8. **Focus follows the realized rows, with a page of look-ahead.** A row holding focus, a pointer
   capture, a selection endpoint or an overlay's anchor is pinned: it stays realized wherever the
   list scrolls. `Tab` past the last realized stop builds at most a page of rows beyond, detached,
   to find the next stop; finding none, it leaves the list. A list box is one tab stop, and its
   keys (arrows, `Home`, `End`, the page keys, type-ahead) move by index through every option.
9. **A reader is told the whole and walks the window.** Each set (the list, or each section of
   it) reports its count, and each realized row its position (`SizeOfSet` and `PositionInSet` in
   AccessKit, `aria-setsize` and `aria-posinset` on the web). At least one row is always realized
   beyond each edge, and a reader moving onto it, asking a row to scroll into view or asking the
   list to scroll a page moves the window. On Linux this already reads as `posinset` 501 and
   `setsize` 1000 over AT-SPI (VERIFIED (ran)), which GTK 4.14's `GtkListView` does not report.
10. **The web renders every item on the server and virtualizes once hydrated.** The server render
    is the full list, each row under `content-visibility: auto`, as the doctrine's full content
    asks. Hydration adopts the rows in the window, removes the rest's markup in one removal per
    side and keeps the geometry exact by reading each removed run as one block. Today's
    `packages/` server render, which emits only the overscan slice, is corrected.
11. **A header starts a section.** A reader finds each section as a set of its own: a list named
    by its header, or in a list box a group of options named by it. Headers stick by default
    (`set_sticky_headers`), the next one pushing the last out. On the web each section is a box
    of its own, which both the push (`position: sticky`) and valid ARIA need.
12. **Grids fall out of the same engine.** The engine windows over lines rather than items: a
    line is one item in a column or row and N items in a grid. `GridCells::{Count, Adaptive,
    FixedSize}` sets N.
13. **The seam gains no node kind.** A lazy list is a `Scroll` node around a column, with two
    additions to the seam: a scroll offset content, reported as an applied change and written to
    scroll; and the reader's scroll requests and the list facts (`set_size`, `position_in_set`,
    `multiselectable`, the `List` and `ListItem` roles).
14. **The native names are searchable** under the rule landed in a31b959: a prefixed setter a
    later change adds, and any platform list control, carries the toolkit's names as
    `#[doc(alias)]`es. Whether the shared views carry them too is the author's call (section 15,
    choice 1); the default keeps the rule to the platform's own vocabulary.

## 2. What exists

**In `packages/`** (VERIFIED (read), at a31b959):

- `packages/foundation/src/lazy_list.rs` has `LazyColumn<P>` and `LazyRow<P>`, built over a
  generic platform parameter, with `LazyColumnBase` and `LazyRowBase`. The web and terminal
  facades declare them (`packages/web/src/views/lazy_list.rs`,
  `packages/terminal/src/views/lazy_list.rs`).
- Items come from `derive_items(items: Fn() -> I, key, view)` with `I::IntoIter:
  ExactSizeIterator`. The window is a keyed `derive_children` segment, each item wrapped in a
  rigid `Container`.
- The window is a memo over a scroll offset signal set on every scroll event, and it counts
  the items by calling `items()` on every evaluation; the keyed segment calls `items()` again
  whenever the window moves, skipping to it (`items().into_iter().skip(window.start)`). A
  closure that clones its items (`basket.get()`) therefore costs their count on every scroll
  event and again on every move of the window, and an iterator that cannot skip costs the
  window's start.
- `ItemExtent::Fixed` or `ItemExtent::Estimated` sets the extent; the default, `Fixed(0.)`,
  builds every item. Overscan is three items.
- A leading and a trailing spacer stand in for the unbuilt items. `observe_size` reports the
  viewport and each estimated item.
- Each new measure recomputes every item's leading edge by walking all items and hashing every
  key: O(n) per measure.
- Nothing anchors. Nothing reports a count or a position to a reader. Nothing moves focus beyond
  the window, and nothing chooses.
- The web's server render serializes the overscan slice and a spacer
  (`packages/web/tests/server_lazy_column.rs`), which the doctrine's full-content rule forbids.
- The foundation's tests run on the headless platform the doctrine retired. The windowing
  functions (`item_window`, `item_window_over`) and their property tests are sound and carry
  over.
- `packages/reactive`'s store keeps a key-to-index map per keyed vector, rebuilt only once it
  stops answering (`store.rs`, `StoreCore::keyed_index`). `KeyedField::entries` returns a lens
  per entry, allocating the count on every change of shape.

**In the prototype** (`spikes/linux`, VERIFIED (read)):

- There is no lazy list. The previous build's report names the lazy layout of off-screen rows
  as one of the levers left for the 1000-row first frame, 61–69 ms against a 50 ms target with
  layout 18–20 ms against 16. A relayout still costs 20–27 ms at 1000 rows, and 5000 rows' first
  frame is 192–217 ms.
- A `Scroll` node keeps its offset in the shell (`gui-linux/src/shell.rs`, `scroll_by`,
  `reveal_focus`, `scroll_key`). The seam neither reports nor writes that offset.
- The accessibility requests the seam knows are `Activate`, `Toggle`, `Increment`, `Decrement`,
  `Expand`, `Collapse`, `Focus` and `SetValue`, with no scrolling among them. `Role` has
  `ListBox` and `Option` but no `List` or `ListItem`.
- The AccessKit tree is rebuilt whole on every push (`gui-linux/src/access.rs`, `build`).
- The `Collection` behavior's key table is index-based (`CollectionKeys { count, current,
  enabled }`), which fits a list most of whose members are not built. Its `step` tries up to the
  whole count for an enabled member, and `Roster` holds its members as a `Vec<(usize,
  ControlHandle)>` searched linearly, its count one past the greatest index registered
  (`foundation/src/behavior/collection.rs`): both assume every member is built.
- Its reactive runtime has no store, no `Key` and no opt-in reload state
  (`foundation/src/reactive.rs`), so `derive_entries` and the per-key stash wait on their port.
  Its keyed lists key by the app's own `K: Eq + Hash + Clone` (`foundation/src/keyed.rs`), and
  `derive_children` takes `Fn() -> Vec<T>`.
- The gate's children-last lint (`xtask/src/lint.rs`, `CHILD_SETTERS`) knows `add_child`,
  `set_child`, `derive_children`, `derive_entries` and their kin, and no item or option setter.

**The references measured in this stage:**

- GTK 4.14.5's `GtkListView` with 1000 rows in a 300-pixel window exposes 14 list items to
  AT-SPI. None has `posinset` or `setsize`, each one's index in its parent counts the realized
  rows, and `Component.ScrollTo` on a row fails with an AT-SPI error. `GtkListBox` with 1000 rows
  exposes all 1000. VERIFIED (ran).
- AccessKit through Skene's vendored `accesskit_unix` 0.24.0 and `accesskit_atspi_common`
  0.21.0: a `List` with `size_of_set(1000)` holding 14 rows with `position_in_set(500..)` reads
  as 14 list items with `posinset` 501 to 514 and `setsize` 1000. `ScrollTo` arrives as
  `ScrollIntoView(TopEdge)`. VERIFIED (ran).
- Orca 46.1 (this host's, Ubuntu 24.04) counts siblings for a position outside web content and
  never reads `posinset` there (`script_utilities.py`, `getPositionAndSetSize`). Orca 47.0
  through `main` read `posinset` and `setsize` in every toolkit (`ax_utilities.py`,
  `get_position_in_set`, `get_set_size`). VERIFIED (read), from GNOME's GitLab at the tags
  `ORCA_47_0`, `48.0`, `49.0`, `50.0` and `main`. No Orca 47 or later is installed here, so no
  speech was heard.
- AccessKit `main` (a39329d, 2026-10-18): the AT-SPI and UIA adapters map `size_of_set` and
  `position_in_set`, and the Android, macOS and iOS adapters map neither. Android maps
  `ACTION_SCROLL_FORWARD`/`BACKWARD` to `ScrollDown`/`ScrollUp` by a page. No adapter has an
  action for an item outside the tree. VERIFIED (read). Section 16 has the upstream report.

## 3. The facade

### The views

Both facades declare five views, each a type of their own over the foundation's one
implementation (`impl LazyColumnBase for LazyColumn {}` and its kin), their own members
inherent beside it and the surface both facades share from `skene_foundation::facade!`, as every
view's is. What the four lazy views and `Listbox<C>` share (the item extent, the overscan, the
spacing and padding, the headers' stickiness, the scroll position, the default scroll anchor) is
one trait, `LazyBase`, as `DecorationBase` is for the views that decorate.

| Skene | Compose | SwiftUI | Android Views | UIKit and AppKit | GTK |
|---|---|---|---|---|---|
| `LazyColumn` | `LazyColumn` | `LazyVStack` in a `ScrollView` | `RecyclerView`, vertical | `UICollectionView` (list), `UITableView`, `NSTableView` | `GtkListView` |
| `LazyRow` | `LazyRow` | `LazyHStack` | `RecyclerView`, horizontal | `UICollectionView`, horizontal | `GtkListView`, horizontal |
| `LazyVerticalGrid` | `LazyVerticalGrid` | `LazyVGrid` | `GridLayoutManager` | `UICollectionViewCompositionalLayout`, `NSCollectionView` | `GtkGridView` |
| `LazyHorizontalGrid` | `LazyHorizontalGrid` | `LazyHGrid` | `GridLayoutManager`, horizontal | the same, horizontal | `GtkGridView`, horizontal |
| `Listbox<C>` | none | `List(selection:)` | `RecyclerView` with a `SelectionTracker` | a table or collection view's selection | `GtkListView` over a selection model |

The other frameworks' columns are JUDGMENT, from their documentation as I know it. A
`LazyVerticalStaggeredGrid` (masonry) is left out: its columns scroll independently of one
another's extents, so it does not fall out of the line windowing.

### A lazy list's members

The members of `LazyColumn` follow; `LazyRow` takes the same with widths for heights.

```rust
LazyColumn::new()
  // Properties first.
  .set_item_height(ItemExtent::Estimated(56))  // or `48` for ItemExtent::Fixed(48)
  .set_spacing(1)                               // between rows, as Column's
  .set_padding(Inset::new(8, 0, 8, 0))          // inside the scrolled content (Compose's contentPadding)
  .set_item_alignment(AxisAlignment::Stretch)   // across the axis, as Column's
  .set_overscan(2)                              // lines realized beyond each edge, at least 1
  .set_default_scroll_anchor(Edge::Start)       // SwiftUI's defaultScrollAnchor; End for a chat
  .set_sticky_headers(true)                     // a section's header sticks while it is in view
  .set_label("Messages")                        // the list's accessible name, as Container's
  .bind_scroll_position(position)               // Signal<ScrollPosition>
  .set_item_builder(|| Container::new().set_padding(Inset::new(0, 16, 0, 16)))
  // Items last.
  .add_header(Text::new("Today"))
  .derive_entries(store.today().keyed_by(|message| message.id), message_row)
  .add_header(Text::new("Earlier"))
  .derive_entries(store.earlier().keyed_by(|message| message.id), message_row)
  .add_item(Text::new("No more messages"))
```

- **Items.**
  - `add_item(view)` appends one item, keyed by its place among the list's segments.
  - `derive_items(items, key, view)` appends a keyed segment from a closure yielding the items.
    The list runs the closure once per change of what it reads, keeps the items it yields and
    their keys, and indexes them per frame; each row's view gets a clone of its item (`T:
    Clone`), so long data keeps its items behind `Rc` or comes from a store.
  - `derive_entries(location, view)` appends one item per entry of a keyed store location, the
    path for long data. The list keeps the location's keys, reading its length and the key at an
    index (two reads `KeyedField` gains, since `entries` allocates a lens per entry), and finds
    the index of a key through the store's own key-to-index map. Each row is a lens onto its
    entry, so an edit to one entry reaches only its row, and an insertion shifts indices and
    rebuilds nothing.
  - `add_header(view)` appends a header, which starts a section that runs to the next header
    (section 8).
  - `Column` takes the same names (`derive_children`, `derive_entries`), so moving a list from a
    `Column` to a `LazyColumn` changes the type and nothing else, apart from `derive_children`
    becoming `derive_items`.
  - The gate's children-last lint gains `add_item`, `derive_items` and `add_header` (and the
    options of `api-design.md`), so a property after them fails the gate.
- **`ItemExtent`** is a property value like any other. It takes conditions and tokens
  (`ItemExtent::Fixed(48).when(Media::min_width(900), 56)`, or a length token for `Fixed`). The
  default is `Estimated(48)` on `skene-gui` and `Estimated(1)` on `skene-tui`. Today's default,
  `Fixed(0)`, builds every item; that trap is removed.
- **`set_overscan(lines)`** sets the lines realized beyond each edge; less than one is taken as
  one, which the reader contract needs (section 7).
- **`ScrollPosition { index: usize, alignment: AxisAlignment, offset: f32 }`** places the item
  at `index` (its place among the list's items and headers, as Compose counts its sticky
  headers) with its point at `alignment` (`Start`, `Center`, `End`) `offset` from the viewport's
  matching point. It is one atomic value, as the seam's contents are.
  - `set_scroll_position` places the list at first, `derive_scroll_position` places it as the
    closure says, `handle_scroll` hears every position, and `bind_scroll_position` is both.
  - The list reports `Start` alignment for the first visible item, once per frame while it
    scrolls, and only where a handler or a binding asks: the engine itself never reads its
    position through the reactive graph.
  - A data change that moves the first visible item reports its new index, so a bound position
    follows the item.
  - A write jumps, unless the list has a transition for its scroll (`add_transition`), which
    then animates it under reduced motion's rule.
- **`set_label` and `set_labelled_by`** name the list for a reader, as `Container`'s name a
  region. They decorate nothing: a layout view carries layout properties, its padding and
  nothing else, and the list's name is accessibility.
- **`set_item_builder(|| Container)`** builds the box each row sits in (the `Item` part), so a
  library's preset styles rows (padding, separators) without the list fixing any of it, and the
  app's view stays the row's content. The default draws nothing.

On `skene-tui` the same chain is written in cells: `LazyColumn::new().set_item_height(1)`.

### `Listbox<C>`

```rust
let chosen = Signal::new(None::<MessageId>);

Listbox::new("Messages")
  // Properties first.
  .set_item_height(ItemExtent::Estimated(56))
  .bind_chosen(chosen)                          // Listbox<Option<MessageId>>; or a HashSet<MessageId>
  .set_item_builder(|_, label| ListboxItem::new(label))
  // Options last.
  .add_header(Text::new("Today"))
  .derive_options(move || store.today_ids(), move |id: &MessageId| message_label(*id))
  .add_header(Text::new("Earlier"))
  .derive_options(move || store.earlier_ids(), move |id: &MessageId| message_label(*id))
```

- **Its options** are the picker's (`api-design.md` 1.3): `add_option(value, label)`,
  `set_options`, `derive_options(values, label)`, each option carrying a value of `T` (the
  choice's `Chosen::Value`), its label
  a `Label` (a text, or a view named through `Label::set_name`). The list keeps the values
  `values` returns, once per change, and indexes them per frame.
- **Its choice** is `bind_chosen` with `set_chosen`, `derive_chosen` and `handle_choose`, all
  typed by `C`: an `Option<T>` for one choice or a `HashSet<T>`/`BTreeSet<T>` for several, the
  three implementing a `Chosen` trait (its `Value` the options' type) whose
  `#[diagnostic::on_unimplemented]` states the rule. `C` is fixed by the binding, with no
  annotation, and the options' closures infer `T` from it; a bare value is refused with the
  trait's message, and a handler of another shape than the binding does not compile. VERIFIED
  (ran): a probe crate (`choiceprobe/` beside this file), with rustc 1.99.0, compiles and runs
  both shapes, prints "a list box chooses an `Option` or a set of its options' values, not
  `u64`" for a bare value, and refuses a `HashSet` handler beside an `Option` binding.
- **Nothing chosen** is the list's ordinary state, so its single choice is `Option<T>`, where a
  picker's is `T` (a picker always shows a value): SwiftUI's `List(selection:)` and `Picker`
  differ the same way. A type parameter per choice (`Listbox<T>` with each member generic over
  its own shape) would let a `HashSet` handler sit beside an `Option` binding; typing the list
  box by its choice makes that a compile error.
- **Its member** is `ListboxItem`, as `PickerItem` is the picker's and `MenuItem` the context
  menu's, so the styled name does not collide with the behavior's `ListboxOption`.
- **The rest** is `LazyBase` (above) and the control's members from `Control<B>` (its label,
  `set_disabled`, `set_autofocus`, `set_labelled_by` and the accessibility a control's user
  adds), each written once.
- **The picker's open list** is windowed by the same engine (its behavior a list box holding
  the picker's one value), so opening a 10,000-option picker builds about a screen of options
  rather than all of them.

Why not `LazyColumn::bind_chosen`, as SwiftUI's `List(selection:)`: a choosing list is a control
(it takes focus, owns its keys, is disabled, required or invalid, and is named as a control is),
and `AGENTS.md` gives every control's members to `Control<B>` once and a layout view nothing but
layout and padding. A typed choice on `LazyColumn` would also need a type parameter, fixed by a
setter that changes the list's type, for a view that otherwise holds no value. JUDGMENT; section
15, choice 2.

### The grids

The grids take a lazy list's members, plus:

```rust
LazyVerticalGrid::new()
  .set_columns(GridCells::Adaptive(160))   // or GridCells::Count(3), GridCells::FixedSize(120)
  .set_spacing(Spacing::new(8, 8))
  .add_header(Text::new("Photos"))         // a header or a static item takes a whole line
  .derive_entries(store.photos().keyed_by(|photo| photo.id), thumbnail)
```

`GridCells` is Compose's type. Its count variant is named `Count` rather than Compose's `Fixed`,
since `Fixed` names a length in `ItemExtent` (JUDGMENT, section 15). `LazyHorizontalGrid` takes
`set_rows`. A grid that chooses (a photo picker) is a later item.

### Platform properties

No platform needs a property of its own for the core. Where one is added later, it is a prefixed
setter carrying the toolkit's name in its doc line and as a `#[doc(alias)]`, for example
`set_ios_scrolls_to_top` with `#[doc(alias = "scrollsToTop")]` for `UIScrollView`'s status-bar
tap. That property is an example and is not proposed now.

## 4. One engine in the foundation

### The item model

A list is a sequence of segments: static items, keyed segments from `derive_items`,
`derive_entries` or `derive_options`, and headers. An item's global index is its segment's offset
plus its index in the segment. Its identity is its key: the hash of the value it was keyed by
(`Key`, 128 bits, the same identity `reconcile` and `View::set_key` use, its hasher widened as
`api-design.md` section 10 requires), or for a static item or a header its place among the
segments.

A debug build panics when two items share a key, as it panics on a token redefined as another
kind. The check costs nothing more than the key-to-index map a change already builds.

### Sources

- **`derive_items`.** The closure runs once per change of what it read, the list keeping its
  items as an `Rc<[T]>` and their keys. The key-to-index map is built when a key must be found
  (the anchor or a pin after a change, a measure's remap, a range of choices): O(n) once per
  change. The anchor and the pins are looked for near their old indices first, so a change that
  inserts or removes a few items above them builds no map.
- **`derive_entries`.** The list keeps the location's keys through its length and the key at an
  index; the store finds a key's index through its own map, rebuilt O(n) after a change of shape.
- **`derive_options`.** As `derive_items`, the values being the keys.
- **A frame** indexes what the list kept: O(window), whatever the count.

### Extents

- `Fixed(length)`: every line is `length`; the item's box is given that extent and nothing is
  measured (Flutter's `itemExtent`).
- `Estimated(length)`: every realized row is measured through the realization's size report
  (`Interest::size`, `observe_size`) and the measure is kept by key, so it survives insertions
  and moves.
- **The estimate in force** is `length` until a row is measured, then the mean of the measured
  rows. It is re-based only while the list is at rest (no gesture, fling or animated scroll),
  the anchor absorbing the change in one frame, so the offsets of unmeasured rows do not move
  and the thumb does not jitter during a scroll. Compose's and `RecyclerView`'s scroll bars
  estimate from the measured rows too (JUDGMENT).
- **Offsets** live in an order-statistic structure over the measured items' differences from the
  estimate, with their prefix sums: `offset(i) = i × estimate + Σ(measure − estimate)` over the
  measured items before `i`. Finding the item at an offset and the offset of an item both cost
  O(log m) in the m items ever measured. A data change remaps the measured keys to their new
  indices: O(m) through the key-to-index map. Today's O(n) walk per measure goes.
- **A change across the axis** (the window resized, the text scale changed) keeps the measures
  as estimates, refreshed as rows are realized; the anchor keeps the view in place while they
  move.
- **Positions** are `(item, offset)` and extents `f64` in the engine. A realization whose scroll
  range has a limit maps the engine's extent into it, scaling only the unrealized part beyond
  the limit. The web is the known case: an element's height stops near 17.9 million pixels in
  Firefox and 33.5 million in Chromium and WebKit (JUDGMENT, from their layout units, unchecked
  here). 100,000 rows of 48 pixels are 4.8 million; rows taller than about 179 pixels at
  100,000 reach Firefox's limit.

### The window

The window is the lines intersecting the viewport, plus `overscan` lines beyond each edge (at
least one, which the reader contract in section 7 needs), plus every pinned row.

- **Pins.** A row stays realized, wherever it is, while:
  - it holds focus;
  - it holds the pointer capture or a press not yet released (so a click still lands on its
    target);
  - it holds an endpoint of the text selection;
  - it anchors an open overlay (a picker or a context menu opened in it);
  - its exit motion runs.

  Compose's pinnable items do this for focus (JUDGMENT). The pin API is public in the
  foundation, since nothing on a built-in's path is crate-private.
- **Runs.** The realized lines form runs: the window and each far pin. The content is one column
  with the realized rows in flow, the same tree on taffy and in CSS:
  - the content's leading padding is the list's padding plus the extent before the first run;
  - the first row of each later run has a leading margin equal to the gap from the previous run;
  - the trailing padding is the extent after the last run.

  Padding and margins are derived per node: a slot write on the web, a value in the record
  elsewhere, never a class change. Spacer nodes are not used. A far-pinned row is laid out at
  its true offset, so its bounds stay true for a reader and for revealing it again.
- **Rows leaving.** A row outside the window and unpinned is disposed: its owner and its nodes
  go, and its state with them, unless opted in (below). Keeping rows hidden in a cache to return
  to is not done, so leaving the window is predictable, as Compose's `remember` is (JUDGMENT).
- **Reordering.** Inside the window, rows are reconciled by key with today's LIS-minimal reorder,
  and a moved row is relocated, keeping its focus, caret and scroll offsets
  (`Realization::relocate`).

### State across leaving

A row's opted-in signals are kept in the list's per-key stash when the row is disposed, scoped
to the row's owner, whose path step is the item's `Key`. When the key is realized again, the
build adopts the stash through the same alignment a restore uses. The same binary keeps and
adopts, so the stash holds the values themselves and encodes nothing.

What a node holds outside the graph (a field's caret and selection, a nested scroll box's
offset, a nested lazy list's position) goes through the extras a restore uses (`packages/`'
`provide_extra` and `take_extra`), by the row's places. A carousel row of `LazyRow`s therefore
comes back where it was left, as Compose's `rememberLazyListState` does inside a `LazyColumn`
(JUDGMENT).

A stash entry goes when its key leaves the items. Nothing is stashed for a signal that is not
opted in, so a list whose rows opt nothing in pays nothing. This gives the opt-in a production
use on every platform, beside Android's process death (`ROADMAP.md` item 16): the one concept of
state worth keeping, as `rememberSaveable` is Compose's for both. Section 15, choice 13.

### Recycling

A realization keeps a pool per node kind of the toolkit objects of disposed rows, and `create`
takes from it. On iOS, macOS and Android that is a `UIView`, an `NSView` or a `View`, whose
creation and layer setup are the costly part (JUDGMENT, to be measured). On Linux and Windows it
is a `Drawn` record. On the web it is an element. A pool is bounded, and a realization keeps
none until a measurement asks for one.

The reactive build itself is never reused for another item. A row rebound to a new item would
carry the old item's state. Compose avoids that by re-executing a reused composition, and a
build in Skene is a one-shot descent with nothing to re-execute.

A native view adopted through the native door is rebuilt with its row. A reuse hook on
`NativeView`, like Compose's `AndroidView(onReset)`, is a later item (JUDGMENT).

### The frame

On every platform but the web, the foundation runs the list inside the frame contract of
`ROADMAP.md` item 3 ("layout settles before a frame commits"), the window computed by the
engine and not through the reactive graph:

1. Apply the scroll input.
2. Compute the window, and build and dispose rows.
3. Lay out the realized rows (O(window)).
4. Take the new measures.
5. If the window changed (rows smaller than estimated leave a gap), repeat from step 2, at most a
   few times, each pass building only the shortfall.
6. Correct the anchor, write the offset, and paint.

Only building and disposing rows (step 2) touches the reactive graph. Compose fills the
viewport in one measure pass the same way (JUDGMENT). On the web the loop runs in the
`ResizeObserver` callback, which the HTML event loop delivers after layout and before paint,
laying out again before the paint (JUDGMENT, from the HTML specification's update-the-rendering
steps). On a compositor platform it runs in the scroll view's layout pass (section 11).

Prefetching rows in the frame's idle time, in the scroll's direction (`RecyclerView`'s
`GapWorker`, Compose's prefetch), is an upgrade with overscan as its floor, left for after the
first measurements (JUDGMENT).

### Anchoring

- **The anchor** is the focused row when it holds a text entry and is visible, else the first
  visible item, as Chromium's scroll anchoring prefers a focused editable element (JUDGMENT).
  Scrolled to the end, the anchor is the end. A jump to `End`, or to an item, lands on the true
  end once the rows there are measured.
- **The rule:** after a frame's layout, the anchor's leading edge keeps its distance to the
  viewport's leading edge. A row above it that is measured, inserted, removed or resized changes
  the offset by the same amount, in the same frame.
- **At the very start** (offset zero), an insertion above shows. Chromium does not anchor at a
  zero scroll offset and Compose keeps the first item by key (both JUDGMENT, to be checked in
  Chromium's `ScrollAnchor` before landing). The rule follows Chromium where they differ.
- **A removed anchor** passes the anchor to the next item, else the previous.
- **During a fling** the drawn platforms shift the kinetic motion's origin by the correction,
  keeping its velocity (`gui-linux/src/kinetic.rs`). On the web, where a programmatic scroll may
  stop a momentum scroll in WebKit (JUDGMENT), a correction is absorbed in the leading padding
  while the padding is larger than it, until `scrollend` (a debounce where the engine lacks it),
  then settled in one frame; near the start, where the padding is smaller, it is written to the
  offset at once.
- **`set_default_scroll_anchor(Edge::End)`** (SwiftUI's `defaultScrollAnchor`) starts at the end
  and anchors the end while it is in view, so a chat stays at its newest message as messages
  arrive, its items in their order. Scrolled up, the anchor is the first visible item as usual.
  Compose's `reverseLayout`, which puts item 0 at the far end, is decided against in
  `DECISIONS.md`.

### The scroll bar

The bar is the scroll box's parts (`Track`, `Thumb`), today's look the default, per the author's
2026-10-17 answer and `api-design.md` section 5: the drawn platforms (Linux, Windows, the
terminal) draw the parts, and the web and the compositor platforms keep their own bar or
indicator, colored from the parts where they can be. The thumb is the viewport over the
estimated extent. Dragging it (or the platform's own scroller) maps the position to an offset,
then to an item through the prefix sums. The thumb drifts as measures arrive, as every lazy
list's does, but only while the list is at rest (JUDGMENT).

## 5. Focus and the keyboard

- **Order.** Focus order follows the scene's child order, as `AGENTS.md` says: the realized
  rows, in item order. Unrealized items are not in the scene, as unrendered rows are not in a
  virtualized list's document on the web.
- **`Tab` past the realized edge.** The sequential navigation, the foundation's walk over the
  scene, asks the list for the next stop when it reaches the end (or, backward, the start) of
  the realized runs. The list then:
  - builds the next rows in order, detached and not attached to the platform, at most a page of
    lines (the lines in view);
  - checks each for a tab stop, a scene fact needing no layout, and disposes the row if it has
    none;
  - remembers by key the rows that hold none, so the next search skips them unbuilt;
  - realizes the first row with a stop, focuses it and scrolls it into view;
  - finding none within the page, leaves the list, as `Tab` leaves a virtualized list on the web.

  A `Tab` therefore costs at most a page of detached builds. A list whose stops are at most a
  page apart is walked whole by `Tab`; a stop beyond a longer stretch without one is reached by
  scrolling first, as on the web. The unbounded search, which reaches every stop in item order,
  costs a build per row passed: at an assumed 20 µs a row, 0.2 s at 10,000 text-only rows and
  2 s at 100,000, on the first `Tab` out of the list (JUDGMENT). Section 15, choice 3.
- **Entering a list.** `Tab` from before a list goes to the first stop among its realized rows,
  `Shift-Tab` from after it to the last, each scrolled into view; a press on a row sets the
  sequential focus navigation starting point there, as Chromium's does.
- **A list with no stop** among its realized rows and the page beyond is itself a tab stop, as
  Chromium's keyboard-focusable scrollers are, so its keys can scroll it. This needs Linux's
  scroll box to take focus, which it does not today (`shell.rs`, `focusable`: VERIFIED (read)).
- **A plain list's own keys.** Without a choice, the page keys, the arrows and `Home`/`End`
  scroll the list as any scroll box does (`scroll_key`), from the list or from a row's control
  that leaves them unhandled. `End` jumps to the estimated end and settles on the true one.
- **A focused row scrolled away** stays realized and focused (pinned). Typing into it scrolls
  it back into view, as Chromium scrolls a focused field it types into (JUDGMENT).
- **Removing the focused item.** In a list box, focus moves to the next option, else the
  previous. In a plain list focus is dropped, as a browser drops it, and the next `Tab` starts
  from where the row was (JUDGMENT).
- **A disabled control takes no focus**, inside a list as anywhere.
- **An autofocus control in a row** acts under the window's one autofocus (`api-design.md`
  section 3): a row built by scrolling never takes focus from what holds it.

### A list box's keys

A list box is one tab stop. It enters at the chosen option, else the last active one, else the
first enabled, as the ARIA practices say. The `Collection` behavior's index-based table drives
its keys over every option, through the list's source rather than its registered members:

- the arrows step by one, passing over disabled options; the target is realized to learn
  whether it is disabled, and a run of disabled options longer than a page stops the step there
  (bounded as `Tab` is);
- `Home` and `End` go to the first and last enabled option;
- `PageUp` and `PageDown` move by the lines in view;
- **type-ahead** reads each option's name from its label (`Label::set_name`, or a text label),
  calling the label closure without building the option and caching the name by key. An option
  whose label names it by its content alone is found only while realized.

## 6. Choosing

These are `Listbox<C>`'s; a plain lazy list chooses nothing.

- **Choosing writes values.** A click chooses the clicked option's value. In a list that chooses
  several, the platform's primary modifier (`Ctrl`, `⌘` on Apple) with a click toggles a value,
  and `Shift` with a click chooses every value between the anchor and the clicked option: found
  by index, read from the list's kept source without building a row, at a cost of O(k) in the k
  values chosen. The primary modifier with `A` chooses every value the same way: O(n).
- **On touch**, choosing several starts with a long press and continues with taps, as Android's
  `SelectionTracker` and UIKit's multiple selection do (JUDGMENT; a later item, since the
  prototype's touch has no long press on a list yet).
- **A row's look.** An option shows `State::Checked` while chosen (the controls stage's states),
  from `chosen.contains(value)`, when it is realized. Nothing is written into an unrealized row.
- **A reader** finds `multiselectable` on the list box and `selected` on each realized option.
  How many are chosen beyond the window is not reportable through AccessKit. AT-SPI's
  `Selection` counts the realized ones only, as GTK's does (VERIFIED (ran): `GtkListView`'s
  selection interface reported the one realized chosen row). This is a known limit (JUDGMENT).
- **Text selection** across rows (the terminal's document selection, and the web's) pins the
  rows holding its anchor and its focus, in any lazy list. Dragging past the list's edge scrolls
  it (browser parity), which realizes rows as it goes.
  - A copy serializes the rows between the endpoints in item order, building the unrealized ones
    detached to read their text: a cost proportional to the selection, about 0.2 s at 10,000
    rows and 2 s at 100,000 at the assumed 20 µs a row (JUDGMENT). A virtualized web list copies
    only the rows in its document, leaving gaps; copying only the realized rows is section 15,
    choice 17's alternative.
  - After hydration, the web pins the rows holding `getSelection()`'s anchor and focus on
    `selectionchange`, since a browser collapses a selection whose node is removed, and serializes
    a copy itself in a `copy` handler (JUDGMENT).

## 7. What a reader finds

### Facts

- **A lazy list without headers:**
  - role `List`;
  - its name (`set_label`, `set_labelled_by`);
  - `set_size`: the count of its items;
  - the scroll actions `ScrollDown`/`ScrollUp`, or `ScrollRight`/`ScrollLeft` on a `LazyRow`,
    and the child action `ScrollIntoView`.
- **Each realized row:** role `ListItem`, on its item box, and `position_in_set`, its index among
  its set's items. Its name comes from its content, which is beneath it, as a list item's does.
- **A lazy list with headers** is a generic container of sections. Each section is a `List`
  labelled by its header (`LabelledBy`), its count and positions its own; the header is a
  `Heading` where the app made it one, else its content. On the web that is a section box
  holding the header and a `role="list"` box with `aria-labelledby`, since a `role="list"` owns
  only list items (ARIA 1.2, which `browser_axe` holds) (JUDGMENT).
- **A list box:** role `ListBox`, its name as a control's, `multiselectable` where it chooses
  several; each realized option role `Option`, `selected` where chosen, `position_in_set`; each
  section a `Group` labelled by its header, the header presentational, as the ARIA practices'
  grouped list box does.
- **A grid** reports as a list of items, not a table. A table's row and column counts would
  describe layout, which an adaptive grid changes with the width (JUDGMENT).

AccessKit carries all of these (`Node::set_size_of_set`, `set_position_in_set`, `set_selected`,
`set_multiselectable`, `add_action`, `add_child_action`, `Role::Group`, `LabelledBy`). VERIFIED
(read), `accesskit` 0.25.1.

### The reader contract

A reader learns the whole from the counts and the positions. It walks the realized rows, and
because at least one row is realized beyond each edge, its next step always finds a row. It
moves the window three ways:

1. **By focus**: keyboard focus moves the window (section 5) and the reader follows focus.
2. **By asking a realized row to scroll into view**: AT-SPI's `ScrollTo`, UIA's `ScrollItem`,
   and a mobile reader moving accessibility focus onto an off-screen row all reach the list as
   `ScrollIntoView`. The list scrolls the row to the hint's edge, or minimally, realizing beyond
   it.
3. **By asking the list to scroll a page**: TalkBack's scroll forward, VoiceOver's three-finger
   swipe. These reach the list as `ScrollDown`/`ScrollUp` with `ScrollUnit::Page` (AccessKit's
   Android and iOS mappings, VERIFIED (read)).

What no reader can do through AccessKit today is ask for an item by index. UIA's
`ItemContainerPattern`/`VirtualizedItemPattern` and Android's `ACTION_SCROLL_TO_POSITION` exist
for exactly that. The upstream report proposes an action for them.

### Per platform

- **Linux (AT-SPI).**
  - Today: `posinset` 501 and `setsize` 1000 on the 14 realized rows (VERIFIED (ran)). Orca 47
    and later read them (VERIFIED (read)) and say the item's position in the whole list
    (JUDGMENT; no speech heard).
  - Orca 46.1, this host's and Ubuntu 24.04's, counts the realized siblings instead (VERIFIED
    (read)). It says the same thing for GTK's `GtkListView`, which reports no count at all
    (VERIFIED (ran)), so Skene is at GTK's bar on Orca 46 and above it on 47.
  - A patch to report the set size as AT-SPI's child count was considered and set aside. It
    would hand every client null children for the unrealized indices, the test driver's
    `find_by_role` among them.
  - The AT-SPI adapter already delivers `ScrollTo` as `ScrollIntoView` (VERIFIED (ran)). The
    prototype's realization handles no scroll request yet; it gains them.
- **Windows (UIA).** `PositionInSet` and `SizeOfSet` are mapped, and `ScrollItemPattern` reaches
  `ScrollIntoView` (VERIFIED (read)). `ItemContainerPattern`/`VirtualizedItemPattern` are
  missing (upstream).
- **Android, macOS, iOS.**
  - AccessKit maps neither the count nor the position yet (VERIFIED (read)). Skene carries the
    Android mapping as a patch, marked and offered upstream like its AT-SPI changes, until it
    lands.
  - The Apple mapping waits on knowing what VoiceOver reads, which needs Apple hardware this
    stage does not have.
  - Mobile readers' paging works through the existing scroll actions (VERIFIED (read)), and
    TalkBack and VoiceOver scroll a container to reach the next element when it is in the tree,
    which the realized row beyond each edge provides (JUDGMENT).
- **The web.**
  - ARIA: `role="list"`/`listitem` (per section where there are headers), or
    `listbox`/`group`/`option`, `aria-setsize` and `aria-posinset` on every row, and
    `aria-multiselectable`.
  - A screen reader's browse mode walks into the overscan rows, scrolling them into view. The
    list realizes beyond them, so the walk continues as the tree grows (JUDGMENT; to be held by
    `browser_axe` and a scenario).
- **The terminal.** It has no reader of its own. Its tests read the same facts through
  `access_tree`.

### Tests find what a reader finds

The driver reads the platform's accessibility layer, so a test finds realized rows only. It
finds item 5000 the way a user does, by scrolling or pressing keys first. The list's facts make
that a plain assertion: `setsize` 10,000, `posinset` 5001.

## 8. Headers and sections

A header starts a section that runs to the next header. With `set_sticky_headers(true)`, the
default, a header sticks at the viewport's leading edge while any item of its section is in
view, and the next header pushes it out; the header is pinned while its section is in view, so
it stays realized. With `false`, a header scrolls with its section. Stickiness is presentation,
so it is a default the list's user overrides; that it is one property of the list rather than
one per header (Compose's `stickyHeader`) follows SwiftUI's `pinnedViews` and `UITableView`'s
plain style, whose headers all stick (JUDGMENT, section 15, choice 10).

| Platform | How | Cost |
|---|---|---|
| Linux, Windows, terminal | The pinned header's paint translation (the style model's `offset`, paint-only), computed from the scroll offset in the same frame; hit testing follows the translation (`api-design.md` section 4) | one translation per frame (JUDGMENT) |
| iOS, macOS, Android | The realization places the pinned header's view at the computed offset in the scroll view's layout pass, which runs every scrolled frame on the main thread (`layoutSubviews`, `layout`, `onLayout`), as `UITableView` pins its headers (JUDGMENT) | one placement per frame |
| Web | `position: sticky` on the header inside its section's box, the box's own leading and trailing padding standing in for its unrealized rows, so the runs nest one level (sections, then rows); the section box is also what valid ARIA needs (section 7) | a second level of windowing over sections |

Sticky headers land after the core, on every platform at once and with the push everywhere,
rather than with a weaker form on the web (JUDGMENT). Scroll-driven animations
(`animation-timeline: view()`) could push without section boxes, but they are not on every
engine, and the section boxes are needed anyway.

## 9. Grids

The engine windows over **lines**:

- In `LazyColumn`, `LazyRow` and `Listbox<C>` a line is one item.
- In a grid a line is N items, N coming from `GridCells`:
  - `Count(n)` gives N = n;
  - `FixedSize(s)` gives the cells that fit, each `s`;
  - `Adaptive(min)` gives the cells that fit at `min` or more, sharing the rest.
- The foundation computes N from the width and spacing, and hands each line to the layout as a
  grid row of N equal tracks: taffy's grid, or CSS's `repeat(N, 1fr)`. CSS never computes its
  own `auto-fill`, so the two cannot disagree.
- A static item and a header take a whole line.
- A line's extent is its tallest item. Measures are kept by item key, and the line extents are
  rebuilt from them when N changes: O(m). The anchor holds the first visible item through a
  resize, even as the lines it belongs to are recomposed.

Everything else (keys, pins, focus, the reader contract) is the list's.

## 10. The seam

The seam gains no node kind:

- A lazy list is a `Scroll` node (`LayoutKind::Scroll`) around a content column
  (`LayoutKind::Column`, its spacing and padding the list's), holding the realized rows. Each
  row is the list's item box (`LayoutKind::Box`, from `set_item_builder`) around the item's
  view.
- Sections add a box per section where they are realized as boxes (the web, and a reader's
  `List` or `Group` on every platform).

What is added:

1. **`Content::ScrollOffset`** on a `Scroll` node: a point in the medium's scalar (pixels, or
   cells).
   - The realization reports it as `Report::Change { applied: true }` once per frame while the
     user scrolls (wheel, touchpad, fling, bar, keys, the reader's own scrolling). The verdict is
     always to keep it.
   - The shared half writes it to scroll programmatically and to correct the anchor; a write
     reports nothing, as every write.
   - It replaces the realization-private offset the Linux shell keeps today, and
     `ScrollPosition` is built over it.
2. **`AccessFacts`** gain `set_size: Option<usize>`, `position_in_set: Option<usize>` and
   `multiselectable: bool`. `Role` gains `List` and `ListItem`.
3. **`AccessRequest`** gains:
   - `ScrollIntoView(Option<ScrollHint>)`;
   - `Scroll { forward: bool, axis: Axis, unit: ScrollUnit }`;
   - `ScrollToPoint(Point)`.

   A realization maps its layer's requests to these, and the shared half answers them through
   the list or any scroll box.
4. **A look-ahead hook for the sequential navigation.** The list registers, on its content node,
   a provider answering "the next stop after this node, forward or backward, within a page",
   which may realize rows (section 5). On the web, where the browser navigates natively, the
   list's own key handler intercepts `Tab` only at its realized edges while the page beyond holds
   a stop, and asks the same provider. It states `Prevent` there, which is the layer that owns
   behavior doing so, as the doctrine allows.
5. **Pins** come from reports the shared half already hears (`Focus`, pointer capture,
   selection), plus a public pin guard for a third party's view.

## 11. Per platform

### Linux, first

- **The scroll offset becomes the seam's.**
  - `Shell::scroll_by` reports `ScrollOffset` after moving.
  - A `ScrollOffset` write sets `node.scroll`, invalidates the paint, refreshes hover and shifts
    a running fling's origin (`kinetic.rs`).
  - `reveal_focus` stays as it is: once the focused row is realized, it is an ordinary node in a
    scroll box.
  - A scroll box with no focusable content takes focus (section 5).
- **The frame** runs the list's settle loop (section 4) before paint. Only the realized rows
  are laid out, so the relayout cost of 20–27 ms at 1000 rows becomes the window's share
  (JUDGMENT, to be measured).
- **The accessibility tree** is built from the realized scene, so its size and its push cost
  are the window's too. `access.rs` maps:
  - the new facts to `set_size_of_set`, `set_position_in_set`, `set_multiselectable` and the
    roles;
  - `ScrollIntoView`, `ScrollDown`/`ScrollUp` (with `ScrollUnit`) and `ScrollToPoint` to the new
    requests.

  The AT-SPI side needs no new patch (VERIFIED (ran)).
- **The rows probe gains `MODE=lazy`**, the same rows in a `LazyColumn`, and `ROWS=10000` and
  `ROWS=100000`. The frame benchmark compares the first frame, hover frames, wheel frames,
  relayout and a jump to the middle against the `Column` it has today. The expectation
  (JUDGMENT): at 1000 rows the first frame drops below the 50 ms target and its layout below
  16 ms, since shaping (33–36 ms) and layout (18–20 ms) are linear in the rows and the window
  holds about 3% of them; at 100,000 rows a frame stays the same and the first frame adds the
  source's one read (section 12).
- **The GTK reference** is the `GtkListView` probe of appendix A, held beside the Skene list's
  AT-SPI dump. Orca 46's transcripts of both are compared as the controls' were. Orca 47 or later
  is needed for the count to be spoken, and is not installable here (section 14).

### Windows

As Linux, a window platform drawing everything, its bars drawn as the parts. UIA's mappings
exist (section 7), and the `ItemContainerPattern` gap is upstream.

### The terminal

As Linux, in cells: the same engine through `skene-layout`'s runtime (`ROADMAP.md` item 3).

- `packages/terminal`'s `LazyColumn` is replaced by the engine's, keeping its tests' behavior
  (window, restore from a snapshot) with the restore now by `(item, offset)`.
- Wheel latching, the scroll bar's parts drawn in eighth-cell blocks and the selection drag past
  the edge follow browser parity as for any scroll box.
- A row's height is a whole number of cells: a fixed extent of 1, or a measure. Text sizing's
  scaled rows measure in scaled cells.
- A copy across unrealized rows builds them detached (section 6). Sticky headers are a paint
  translation by whole cells.

### The web

- **Server and static renders.**
  - Every item renders, each row with `content-visibility: auto` and `contain-intrinsic-size:
    auto <estimate>px`, or with its fixed `height` under `Fixed`. The scriptless page is a long
    scroll box of real rows: find-in-page, fragment links and a reader's browse mode see all of
    it, and the browser skips laying out and painting the rows out of view (JUDGMENT for the
    engines' support: Chromium 85, Firefox 125, Safari 18 as I recall them).
  - ARIA facts are on every row.
  - The cost is the count: about 1–2 MB of markup at 10,000 rows and 10–30 MB at 100,000, and a
    server build per row (JUDGMENT, at 100–300 bytes a row). A list that long is paged by the
    app with real links, which the doctrine's navigation rule already serves; whether a list may
    declare a server prefix instead is section 15, choice 4.
- **Hydration.** The list adopts its element and reads `scrollTop` and the viewport. Then it:
  1. finds the window by binary search over the server rows' `offsetTop`, a handful of reads of
     an already-laid-out page;
  2. adopts the rows in the window, building their views against their elements, and measures
     them;
  3. reads the extent before the window and the extent after it, one read each (the first kept
     row's `offsetTop`, the content's height less the last kept row's end);
  4. removes the rows before and after in one `Range` deletion each;
  5. sets the leading and trailing padding to those two extents, which the engine keeps as two
     blocks of known extent and count, their rows counted at the block's mean until measured.

  Nothing moves on screen, and the hydration costs the window, four reads and two removals;
  removing 100,000 rows' markup in one deletion is the part to measure (JUDGMENT). This is the
  list's own hydration. The list has one form, the server's being more of the same rows, so it
  needs no `Enhance`, which would also build the full list once more on the client.
- **Client renders** build the window only, from the first frame.
- **Anchoring.** The scroll box sets `overflow-anchor: none`, and the list corrects in its
  `ResizeObserver` callback before paint, with the momentum rule of section 4.
- **Focus.** `Tab` at the realized edges is intercepted while the page beyond holds a stop, and
  the focused row is pinned (section 5). A list box uses roving focus on real elements, not
  `aria-activedescendant`, which would point at a removed element when its row leaves.
- **A list box's scriptless form** follows the picker's: an `Enhance` over a `<select>` (`size`
  rows tall, `multiple` for a set) holding each option's name under its key (`api-design.md`
  1.6), the drawn list box once hydrated. At 100,000 options that `<select>` is the cost to
  measure (JUDGMENT; `ROADMAP.md` item 10, section 15, choice 14).
- **Limits** (JUDGMENT): after hydration, find-in-page finds realized rows only, as in every
  virtualized list on the web, and `hidden="until-found"` cannot help with rows that are not in
  the document. An app that needs the browser's find over all rows uses a `Column`.

### The compositor platforms: iOS, macOS, Android

**Skene's own windowing inside a scroll container with the platform's scrolling; no
`RecyclerView`, `UICollectionView` or `NSTableView`.** The reasons (JUDGMENT):

- **Layout.** Those recyclers own their children's layout (a `LayoutManager`, a
  `UICollectionViewLayout`). Skene's doctrine gives layout to taffy.
- **Building.** They rebind a recycled cell to a new item through an adapter. Skene builds once
  per key, reactively, with state per key; rebinding would carry state across items (section 4).
- **Accessibility.** They report their cells through their own layer (`UITableView`'s virtual
  elements for off-screen rows, `RecyclerView`'s `CollectionInfo`). Skene reports through
  AccessKit, and the two trees would collide on the same views.
- **The governing rule.** The platform's scrolling carries functionality (its physics, its
  indicators and the user's settings for them, its nested scrolling, the reader's gestures), so
  it is kept. Compose's `LazyColumn` and Flutter's lists scroll themselves rather than through
  the platform's scroll view, so they are no precedent here; SwiftUI's `LazyVStack` scrolls in a
  `ScrollView` backed by `UIScrollView` (JUDGMENT).

**What stays native**, per platform:

- **iOS**: a `UIScrollView`, its deceleration, bounce, status-bar scroll to top, keyboard
  avoidance, indicators, and VoiceOver's three-finger scroll through AccessKit's
  `accessibilityScroll:` (VERIFIED (read)). The anchor correction sets `contentOffset` in
  `layoutSubviews`, which leaves a deceleration running, as `UIScrollView`'s recentering
  technique relies on (JUDGMENT).
- **macOS**: an `NSScrollView`, its elastic overscroll, overlay or legacy scrollers as the user
  set them, and responsive scrolling with its prepared content rectangle, which the overscan
  fills; momentum arrives as scroll events, so a correction between them keeps it.
- **Android**: a scroll container of Skene's own (a `ViewGroup`) driving the platform's
  `OverScroller` for its fling, `EdgeEffect` for its stretch from API 31 and glow below it (a
  floor and an upgrade by API level), and implementing `NestedScrollingChild3`, so a
  `CoordinatorLayout`'s collapsing toolbar answers the list. A `ScrollView`'s fling sets
  absolute positions from a scroller it keeps private, so a correction during it would be undone
  the next frame; `RecyclerView` owns its `OverScroller` for the same reason (JUDGMENT).

The anchor correction lands in the scroll container's layout pass (`layoutSubviews`, `layout`,
`onLayout`), before the frame draws. Recycling is the realizations' pools of views (section 4).
On iOS and Android a scroll container's content size and coordinates hold 100,000 rows of any
ordinary height (JUDGMENT).

**Native lists stay reachable.** An app that needs a toolkit's own list (a library's
`RecyclerView` adapter, an `NSTableView` with sortable columns) adopts it through the native
door, `platform::<p>::NativeView`. A platform list control is not proposed. If one is added later
(a macOS table is the likely one), it takes Skene's names where the meaning matches, and the
toolkit's names in its doc line and as `#[doc(alias)]`es of the type and its members
(`NSTableView`, `usesAlternatingRowBackgroundColors`), as the rule landed in a31b959 asks.

**Accessibility gaps** are AccessKit's (section 7), carried as patches and reported upstream.

## 12. Cost at 10,000 and 100,000 rows

Every figure is JUDGMENT until the rows probe's `MODE=lazy` measures it; n is the count, w the
window (about 20–40 lines), m the items measured, k the values or rows an operation covers.

| Operation | Cost | At 10,000 | At 100,000 |
|---|---|---|---|
| A scrolled frame | O(w + log m), building the lines entering | the window's | the same |
| The first frame | O(w) builds, plus one read of the source: O(n) to keep `derive_items`' items, O(n) for a store location's keys | a window plus about 1 ms | a window plus about 10 ms |
| A change of `derive_items`' items | O(n) read and keys; O(n) key-to-index map only where a key is not near its old index | about 1 ms | about 10 ms |
| An edit to one store entry | its row alone | | |
| A measure arriving | O(log m) | | |
| A jump (thumb drag, `End`, a written position) | O(log m), then the window, then a few settle passes | | |
| `Tab` past the window | at most a page of detached builds | under 1 ms at 20 µs a row | the same |
| A list box's arrow | O(1), a disabled run bounded by a page | | |
| `Shift`-click across k options | O(k) values read, the set written whole | | 2 ms for 100,000 |
| Choose all | O(n) values and a set of n | | a set of 100,000 values |
| Type-ahead with no match | O(n) label calls once, names then cached | | about 50 ms once |
| A copy across k rows (terminal) | O(k) detached builds | 0.2 s | 2 s |
| The accessibility tree | O(w) per push | | |
| Memory | the kept items or keys, the key map (about 32 bytes an item), the measures (about 48 bytes each) | about 1 MB | about 10 MB |
| The web's server render | O(n) builds and markup | 1–2 MB | 10–30 MB |
| The web's hydration | O(w), four reads, two removals | | one removal of 100,000 rows |

## 13. Testing

Driver only, on the real platforms, as the doctrine says.

**The foundation's own algorithms**, over canned input, are unit- and property-tested:

- the window over prefix sums, blocks included;
- the anchor correction, at rest and during a gesture;
- the line composition of grids;
- the key-to-index remap, near the old index and through the map;
- the bounded look-ahead.

Today's `item_window_over` property tests carry over.

**`skene-conformance` scenarios**, each acting as a user and checking what a reader finds:

1. A 10,000-item list realizes about the window: the reader finds about 20 rows, `setsize`
   10,000 and positions from 1.
2. The wheel, the page keys and a drag of the thumb reach item 5000. Its row reports `posinset`
   5001 and its text is found.
3. Anchoring:
   - an estimated list whose rows above the viewport measure taller does not move: the
     screenshot of the viewport before and after is the same;
   - an insertion above the viewport does not move it;
   - an insertion at offset zero shows.
4. Focus:
   - in a list whose rows each hold a button, `Tab` from the last realized stop reaches the next
     row's button, realized, focused and in view, and keeps going; `Shift-Tab` returns, and the
     wheel away and back keeps focus on it;
   - in a list with a button 300 text-only rows beyond the realized edge, `Tab` leaves the list,
     and scrolling there first makes the button the next stop.
5. A field in a row keeps its caret and text when scrolled away and back (pinned while focused),
   and an opted-in signal survives when it was not focused.
6. A `Listbox<C>`:
   - `Down` from the last realized option realizes and focuses the next;
   - `End` reaches the last option;
   - `Shift`-click across 5000 options chooses them all, and choosing all chooses every one;
   - type-ahead finds an option beyond the window by its label's name;
   - the reader finds `multiselectable`, `selected` and the bound signal's values.
7. A reader's `ScrollTo` on the last realized row scrolls it to the top and realizes more beyond
   it.
8. A reordering inside the window keeps focus and the caret on the moved row.
9. Sections: the reader finds each section as a list named by its header with its own count;
   sticky headers push (screenshots on the drawn platforms; on the web, the section boxes'
   geometry).
10. A grid's `Adaptive` columns recompose on a resize while the first visible item stays.

**Per platform:**

- **Linux** runs these on x11, mutter, sway, weston and KWin, plus a scale-1.5 leg, with the
  frame benchmark's `MODE=lazy` at 1000, 10,000 and 100,000 rows.
- **The terminal** runs them through its driver, with `insta` frames for the scroll bar and the
  sticky header.
- **The web** runs them through WebDriver, plus browser tests:
  - the server render holds every row with `content-visibility`;
  - hydration leaves only the window in the document and keeps `scrollTop` and the screenshot;
  - the anchor corrects before paint;
  - `browser_axe` holds the list, the sectioned list and the list box.

The `packages/web` test asserting the overscan-only server render is replaced.

**Comparisons:**

- GTK's `GtkListView` probe as the AT-SPI reference (appendix A).
- Orca 46 transcripts on both.
- Orca 47 or later where a leg can install it (section 14).

## 14. The Linux prototype: the plan

Each step lands with fmt, clippy `-D warnings`, rustdoc at zero warnings and the affected tests
on the runner's legs.

1. **The engine** (`foundation/src/lazy`):
   - segments, keys (`Key` with the widened hasher of `api-design.md` section 10), kept sources,
     the extents structure with blocks, and the window with runs and pins;
   - the anchor and the frame loop's hooks;
   - the bounded look-ahead;
   - unit and property tests.
2. **The seam:**
   - `Content::ScrollOffset`;
   - the list facts and roles;
   - the scroll requests;
   - the look-ahead hook and pins.
3. **The facade:**
   - `LazyColumn` and `LazyRow` with `LazyBase`, `ItemExtent`, `ScrollPosition` and headers;
   - `Listbox<C>` and `ListboxItem` over `Collection<ListboxPolicy, C>`, with `Chosen`, after
     the typed choice of `api-design.md` lands;
   - `derive_items` and `derive_options`; `derive_entries` waits on the store's port, since the
     prototype has none.
4. **`gui-linux`:**
   - the offset as seam content, and the kinetic shift;
   - a scroll box with no focusable content as a tab stop;
   - the settle loop;
   - the AccessKit mapping and the reader's requests.
5. **The conformance scenarios, the rows probe's `MODE=lazy` and the frame benchmark.**
6. **The grids, sections and sticky headers.**
7. **Stashed state across leaving** (it needs the opt-in reload state of `ROADMAP.md` item 2,
   which the prototype has not landed; until then a row's extras alone return).

What cannot be checked here:

- **Orca 47 or later speaking the count.** The host's Orca is 46.1 and no newer one is packaged
  for it. It takes a leg on a newer distribution, Ubuntu 25.04 or later, or an Orca built from
  source with its Python dependencies (JUDGMENT for the versions).
- **The compositor platforms and Windows.** They need their hardware or their CI runners, as
  the text-engine spikes had.
- **WebKit's momentum behavior.** It needs Safari on Apple hardware.

## 15. Choices for the author

Each with the default the design above takes.

1. **The toolkits' names as aliases on the shared views.** Default: the rule as landed in
   a31b959 and as asked: a platform's own control and its members carry the toolkit's names, and
   so does a prefixed setter, which is a toolkit's member surfaced on a shared view
   (`scrollsToTop`). The shared views do not. Alternative: `LazyColumn` also carries
   `RecyclerView`, `UICollectionView`, `UITableView`, `NSTableView` and `GtkListView` on its type
   alone. In its favour, a platform control exists only in its own platform's documentation
   build, so a search for `UISwitch` in the Linux documentation finds nothing unless a shared
   view carries the name.
2. **Where choosing lives.** Default: `Listbox<C>`, a control windowed by the engine, typed by
   its choice.
   Alternative: `LazyColumn::bind_chosen` (SwiftUI's `List(selection:)`), which needs a type
   parameter fixed by a type-changing setter and makes a layout view a control.
3. **`Tab` beyond the window.** Default: a page of look-ahead, then out of the list (the web's
   behavior, improved). Alternative: every stop in item order, building each passed row once:
   exact, and seconds on the first `Tab` past 100,000 text-only rows.
4. **Server renders of very long lists.** Default: every item, the app paging with real links.
   Alternative: a declared prefix plus a link (`set_server_limit`), which caps the markup at the
   price of content the page does not carry.
5. **`ScrollPosition`.** Default: by index (Compose's), with no type parameter, the engine
   anchoring by key internally. Alternative: by the item's key (SwiftUI's `scrollPosition(id:)`),
   which needs the list typed by its key or the key erased. And: a written position animates
   where the list has a transition for its scroll.
6. **Unmeasured items.** Default: the declared estimate, re-based to the measured mean while the
   list is at rest. Alternatives: the declared estimate always (stable, a worse thumb), or the
   live mean (the original proposal: the best thumb, a correction every frame of a scroll).
7. **An insertion at offset zero.** Default: it shows (Chromium). Alternative: the first item
   stays anchored (Compose). Both still to be confirmed in their sources.
8. **`GridCells::Count`**, rather than Compose's `Fixed`, which would clash with
   `ItemExtent::Fixed`. Default: `Count`.
9. **Sections for a reader.** Default: a header starts a set of its own (a list named by it, or a
   group of options). Alternative: one set across the list, headers neither counted nor
   positioned, which on the web puts a non-item in a `role="list"`.
10. **Sticky headers.** Default: one list property, `set_sticky_headers`, on by default.
    Alternative: per header (`add_sticky_header` beside a plain `add_header`, Compose's).
11. **Type-ahead past the window.** Default: names from the options' labels, cached by key, an
    option named only by its content found while realized. Alternative: a
    `set_item_name(|item| ..)` closure, a second way to state a name.
12. **A lazy list with an unbounded main axis** (inside a `Scroll` on the same axis). Default: a
    debug build panics with the rule, as Compose throws.
13. **Row state across leaving.** Default: the reload opt-in keeps a row's state by key in
    production, on every platform. Alternative: rows keep nothing, and an app lifts what must
    survive into its own state.
14. **A list box's scriptless form.** Default: an `Enhance` over a `<select>`, as the picker's.
    Alternative: the rows themselves, each with a native radio or checkbox input, which keeps
    rich rows without script but nests an input in an option.
15. **A lazy list's name.** Default: `set_label` and `set_labelled_by` on the list, naming the
    `List` a reader finds. Alternative: the app wraps the list in a named `Container`, the list
    itself unnamed.
16. **Android's scroll container.** Default: Skene's own `ViewGroup` over `OverScroller` and
    `EdgeEffect`, as `RecyclerView`. Alternative: a platform `ScrollView`, corrections applied
    only at rest.
17. **A copy across unrealized rows.** Default: exact, building them detached. Alternative: the
    realized rows alone, as a virtualized web list copies.

## 16. Upstream

Ready to file, not filed:

- `upstream/accesskit-virtualized-collections.md` (beside this file):
  - the Android mapping of `size_of_set` and `position_in_set` to `CollectionInfo` and
    `CollectionItemInfo`;
  - the Apple question;
  - an action to reach an item outside the tree, backing UIA's `ItemContainerPattern` and
    `VirtualizedItemPattern` and Android's `ACTION_SCROLL_TO_POSITION`.

Nothing for vello, Parley or harfrust comes from this design.

## Appendix A: the probes

The probes are kept beside this file in `lists-probe/`. Each ran under `dbus-run-session` with
`at-spi-bus-launcher --launch-immediately`, the GTK one on Xvfb, read with `gi.repository.Atspi`
under Python 3.12. The typed choice's probe is `choiceprobe/`, beside it.

**GTK 4.14.5, `GtkListView` over a `GtkStringList` of 1000 rows in a 400×300 window**
(`gtk_list.py listview`, `read.py`):

```
list: list children: 14 attrs: {'toolkit': 'GTK'}
 child 0 role=list item name='' index=0 posinset=None setsize=None children=1 text='Row 0'
 child 1 role=list item name='' index=1 posinset=None setsize=None children=1 text='Row 1'
 child 13 role=list item name='' index=13 posinset=None setsize=None children=1 text='Row 13'
 child 500 None
 child 999 None
selected children: 1
scroll_to on child 13: GLib.GError: atspi_error: (1)
```

**GTK 4.14.5, `GtkListBox` of 1000 labels** (`gtk_list.py listbox`): `list children: 1000`,
child 500 found, no `posinset` or `setsize`, and `scroll_to` fails the same way.

**AccessKit, Skene's vendored adapter: a `List` with `size_of_set(1000)` holding 14 `ListItem`s
with `position_in_set(500..=513)`** (`akprobe`, `read_ak.py`):

```
action ScrollIntoView on #513 data Some(ScrollHint(TopEdge))
app: akprobe toolkit: probe
list: list children: 14 attrs: {}
 child 0: role=list item name='Row 500' index=0 posinset=501 setsize=1000
 child 1: role=list item name='Row 501' index=1 posinset=502 setsize=1000
 child 13: role=list item name='Row 513' index=13 posinset=514 setsize=1000
scroll_to: True
```

(`#513` is child 3, row 503; the action line is printed by the app before the reader's lines
are flushed.)

**The typed choice** (`choiceprobe/`, `cargo run --example ok`, `cargo build --example bare`
and `--example mismatch`, rustc 1.99.0): `Listbox::new("Messages").bind_chosen(Signal::new(
None::<MessageId>))` and the same with a `HashSet<MessageId>` infer the list box's type and its
options' `T` with no annotation and run; a bare `u64` fails with the `Chosen` trait's message;
a `HashSet` handler beside an `Option` binding fails with a closure mismatch. A picker bound to
an `f32` fails with the `Choice` trait's message.

**Orca's sources read:**

- 46.1 installed: `/usr/lib/python3/dist-packages/orca/script_utilities.py`,
  `getPositionAndSetSize`. The web script's `getPositionAndSetSize` reads the attributes.
- GNOME GitLab, `src/orca/ax_utilities.py` at `ORCA_47_0`, `48.0`, `49.0`, `50.0` and `main`
  (latest tag 51.1, 2026-10-14): `get_position_in_set` and `get_set_size` read `posinset` and
  `setsize` first.
