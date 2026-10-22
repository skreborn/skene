# API design: the author's answers of 2026-10-17

For the drawn Linux prototype (`spikes/linux`), before the answers land. Written 2026-10-19
against `claude/platform-architecture-tradeoffs-y9q57l` at `a31b959`, from the prototype's
`foundation`, `gui`, `gui-linux`, `conformance`, `simple` and `third-party`, and from
`packages/` (`foundation`, `web`, `terminal`, `reactive`) for the shapes that exist there; then
reviewed the same day against `AGENTS.md`, the port to `packages/` and to every platform, and the
cost at 10,000 and 100,000 rows. The list box it types is windowed by the engine of
`lists-design.md`, beside this file.

**Labels.** VERIFIED (ran): executed on this host. VERIFIED (read): checked in the source at
the place cited, without running it. JUDGMENT: a design choice, or an expectation not run here.
Every proposal below is a design and so JUDGMENT unless a line says otherwise. No Chromium or
Firefox is installed on this host, so every statement about a browser's behavior is JUDGMENT
from the HTML and CSS specifications and Chromium's known behavior, for the web tier's tests to
hold when the web is ported.

**The user's request of 2026-10-19** (a native control's toolkit names as doc aliases, so a
search by the native name finds it) is in `AGENTS.md` since `a31b959`; section 7 applies it to
the surface designed here, and `lists-design.md` applies it to the lists.

**What the review changed.**

- Options are content: a chain ends with them, its choice bound before them, and the gate's
  children-last lint gains the option setters.
- A choice group reads its options through one source (their count, the value at a place, the
  place of a value, a name), its built members found by value in a map. The same source windows
  a list box's options, so a 10,000-option picker opens in a screen's worth of builds.
- `Listbox<C>` joins the picker and the radio group, typed by the choice it holds (`Option<T>` or
  a set), so a handler of another shape than its binding does not compile (VERIFIED (ran)).
- The resolver: a control's own node takes its beside caption's hover and its behavior's press
  as its parts do; only a box that acts keeps its own.
- Hit testing prunes by each subtree's bounds, since controls no longer cut the walk; the
  compositor platforms override their toolkit's hit test, and the web has the rule from CSS.
- The scroll bar per platform: the web and the compositor platforms keep their own bars and
  indicators; on Linux a track press follows the desktop's setting, as Firefox's does on Linux,
  rather than Chromium's paging.
- Autofocus: the modality's starting value said plainly; iOS and Android; lazy rows.
- `Painted` on every platform; `ContextMenu`'s entries stay data, which keeps `ROADMAP.md` item
  3's native-menu candidate open.
- The aliases' scope agrees with `lists-design.md`, and every member that carries a toolkit
  member's meaning carries its alias (`set_`, `derive_`, `bind_` and `handle_` alike).
- The probes are named relative to this file, never by a scratch path.

## 0. Summary

- **A choice's value** is the app's own type: `Picker<T>` and `RadioGroup<T>` hold a `T`, every
  option tagged with a `T: Choice` (`Clone + Eq + Hash + 'static`, as SwiftUI's tag is
  `Hashable`); `Listbox<C>` holds an `Option<T>` or a set of `T`. `set_chosen`, `derive_chosen`,
  `handle_choose`, `bind_chosen`, then the options, last: `add_option`, `set_options`,
  `derive_options`, keyed by the tag. Nothing chosen in a picker is the app's own `Option<V>`.
  The web names each option by its tag's 128-bit key, hashed so the server leaf and the browser
  leaf agree.
- **Part states.** Inside a control every state but `FocusWithin` is the control's, its own node
  included; a box that acts keeps its own hover, press and focus. `Disabled` composes outward
  through nested controls. A caption beside its control hovers and presses it.
- **`set_autofocus(bool)`** on every control. A window focuses its first autofocus control or
  nothing; a popup its first autofocus control or nothing; a modal its first autofocus control,
  else its first focusable, else itself; closing returns focus to what held it.
- **Hit testing.** A control no longer clips hits. A clipping box, a scroll box's viewport, an
  overlay's surface and the window clip them, each to its rounded shape; a subtree whose bounds
  miss the point is skipped.
- **The scroll bar** is `Scroll`'s parts: `set_track_builder` and `set_thumb_builder` over
  `Scroll::default_track(axis)` and `Scroll::default_thumb(axis)`, today's look. The scroll box
  is its parts' control. The drawn platforms draw them; the web and the compositor platforms
  color their own bar from them.
- **`Painted`** records in device pixels: `Canvas::size()` (device pixels), `Canvas::scale()`
  (per axis, a cell's pixels in the terminal), `Canvas::to_device()`, `Canvas::layout_size()`.
  It records again when its device size, its scale, a signal or a state it read changes.
  `Painted` and `ContextMenu` move into the foundation together with the view layer they
  compose.

## 1. A choice's typed value

### 1.1 What an app writes

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Size {
  Small,
  Medium,
  Large,
}

fn sizes() -> impl View {
  let size = Signal::new(Size::Medium);

  Column::new()
    .set_spacing(12)
    .add_child(
      Picker::new("Size")
        .bind_chosen(size)
        .add_option(Size::Small, "Small")
        .add_option(Size::Medium, "Medium")
        .add_option(Size::Large, "Large"),
    )
    .add_child(
      RadioGroup::new("Size")
        .bind_chosen(size)
        .set_options([
          (Size::Small, "Small"),
          (Size::Medium, "Medium"),
          (Size::Large, "Large"),
        ]),
    )
}
```

Options from data, one fixed option first, nothing chosen at the start:

```rust
let fruit = Signal::new(None::<FruitId>);

Picker::new("Fruit")
  .bind_chosen(fruit)
  .add_option(None, "None")
  .derive_options(
    move || basket.get().iter().map(|fruit| Some(fruit.id)).collect(),
    move |id: &Option<FruitId>| name_of(*id),
  )
```

- **Content last.** The options are the picker's content, so they end the chain as children do
  (`AGENTS.md`'s children last), and the choice is bound before them. The gate's lint knows no
  option setter today (VERIFIED (read): `xtask/src/lint.rs`, `CHILD_SETTERS`); it gains
  `add_option`, `set_options` and `derive_options`, and the list setters of `lists-design.md`.
- **Inference.** An option's value is the tag itself, not `impl Into<T>`, so `T` is fixed by the
  binding or the first option with no annotation, and an `Option` tag is written `Some(..)` as
  SwiftUI's `Optional` tag is. VERIFIED (ran): `choiceprobe/` (beside this file, rustc 1.99.0)
  compiles `Picker::new("Size").bind_chosen(Signal::new(Size::Medium)).add_option(Size::Small,
  "Small")` with no annotation.

### 1.2 The bound: `Choice`

```rust
/// What a choice group's options carry and what it has chosen: a value of the app's own
/// type, compared to find the chosen option and hashed to name it.
#[diagnostic::on_unimplemented(
  message = "`{Self}` cannot be a choice's value",
  note = "a choice's value is `Clone`, `Eq` and `Hash`, as SwiftUI's tag is `Hashable`: derive them, or choose by an id"
)]
pub trait Choice: Clone + Eq + Hash + 'static {}

#[diagnostic::do_not_recommend]
impl<T: Clone + Eq + Hash + 'static> Choice for T {}
```

- `Eq`: the chosen option is the one whose tag equals the chosen value.
- `Hash`: the tag is the option's identity. Derived options are keyed by it, so an option keeps
  its control, focus and hover while its tag stays, a group finds a built member by its tag in a
  map, and the web names the option by its hash (1.6). A float is no choice: choose by an
  integer or an id. VERIFIED (ran): the probe's picker bound to an `f32` fails with "`f32`
  cannot be a choice's value" and the note above.
- `Clone`: each option keeps its tag, and a choice hands one to `handle_choose`.
- Its `Hash` agrees with its `Eq`, as std asks, and must not depend on the build (no pointer, no
  `TypeId`), since the server and the browser hash it apart. A derived `Hash` qualifies once the
  hasher widens (1.6, section 10), with both leaves built by one toolchain, since std promises no
  `Hash` stable across Rust releases; one `skene build` builds both.

The house rule for a purpose-built trait (an `on_unimplemented` message, the blanket impl
`do_not_recommend`) holds it. The bound is the same in every build, since app code compiles for
every platform alike.

### 1.3 The facade's surface

`Picker<T>` (the styled picker), `RadioGroup<T>` (the styled group) and `Listbox<C>` (the styled
list box) take the same option members; `PickerItem`, `Radio` and `ListboxItem` (the styled
members) stay non-generic, the group giving each its tag as it builds it, as it gives each its
behavior today.

```rust
impl<T: Choice> Picker<T> {
  /// A picker labelled `label`, with no options.
  pub fn new(label: impl Into<Label>) -> Self;

  /// The chosen value.
  pub fn set_chosen(self, chosen: T) -> Self;

  /// The chosen value, kept current.
  pub fn derive_chosen(self, chosen: impl Fn() -> T + 'static) -> Self;

  /// Runs `choose` with each value the reader chooses.
  pub fn handle_choose(self, choose: impl Fn(T) + 'static) -> Self;

  /// Shows `chosen` and writes each choice into it.
  pub fn bind_chosen(self, chosen: Signal<T>) -> Self;

  /// Makes each option's item with `builder`, from its value and its caption;
  /// `PickerItem::new(label)` where unset.
  pub fn set_item_builder(self, builder: impl Fn(&T, Label) -> PickerItem + 'static) -> Self;

  // Unchanged: set_mark, default_mark, set_list, default_list, every control member.

  /// Appends an option carrying `value`, captioned by `label` (a text names it, a view
  /// names it through `Label::set_name`).
  pub fn add_option(self, value: T, label: impl Into<Label>) -> Self;

  /// Its options, in place of those it has.
  pub fn set_options<L: Into<Label>>(self, options: impl IntoIterator<Item = (T, L)>) -> Self;

  /// Appends the options `values` returns, kept current: each value's option is built once by
  /// `label` and kept, wherever it moves, while the value stays.
  pub fn derive_options<L: Into<Label>>(
    self,
    values: impl Fn() -> Vec<T> + 'static,
    label: impl Fn(&T) -> L + 'static,
  ) -> Self;
}

impl<T: Choice> RadioGroup<T> {
  // The same choice and option members as Picker<T>, and, unchanged beside them:
  pub fn set_option_spacing(self, spacing: impl Into<Value<Spacing>>) -> Self;
  pub fn derive_option_spacing<V: Into<Value<Spacing>>>(self, spacing: impl Fn() -> V + 'static) -> Self;

  /// Makes each option's radio with `builder`, from its value and its caption;
  /// `Radio::new(label)` where unset.
  pub fn set_option_builder(self, builder: impl Fn(&T, Label) -> Radio + 'static) -> Self;
}

/// What a list box has chosen: one value or none, or a set of values.
#[diagnostic::on_unimplemented(
  message = "a list box chooses an `Option` or a set of its options' values, not `{Self}`",
  note = "bind an `Option<T>` for one choice, a `HashSet<T>` or a `BTreeSet<T>` for several"
)]
pub trait Chosen: Clone + 'static {
  /// What each option carries.
  type Value: Choice;
}

impl<T: Choice> Chosen for Option<T> { type Value = T; }
impl<T: Choice> Chosen for HashSet<T> { type Value = T; }
impl<T: Choice + Ord> Chosen for BTreeSet<T> { type Value = T; }

impl<C: Chosen> Listbox<C> {
  /// A list box labelled `label`, with no options.
  pub fn new(label: impl Into<Label>) -> Self;
  // set_chosen(C), derive_chosen(Fn() -> C), handle_choose(Fn(C)), bind_chosen(Signal<C>);
  // set_item_builder(Fn(&C::Value, Label) -> ListboxItem); the windowing members of
  // `LazyBase` (lists-design.md section 3); every control member; then the option members of
  // Picker<C::Value> and `add_header`, last.
}

impl PickerItem { pub fn new(label: impl Into<Label>) -> Self; /* rest unchanged */ }
impl Radio { pub fn new(label: impl Into<Label>) -> Self; /* rest unchanged */ }
impl ListboxItem { pub fn new(label: impl Into<Label>) -> Self; }

pub type BasicPicker<T> = Control<behavior::Popup<T>>;
pub type BasicRadioGroup<T> = Control<behavior::RadioGroup<T>>;
pub type BasicRadio<T> = Control<behavior::Radio<T>>;
pub type BasicListbox<C> = Control<behavior::Listbox<C>>;
pub type BasicListboxOption<C> = Control<behavior::ListboxOption<C>>;
```

- **Segments**, as a layout view's children are: `add_option` appends one, `set_options`
  replaces every option with a fixed list, `derive_options` appends a derived segment, and
  `add_header` (a list box's) starts a group. The options are the segments in order, so a fixed
  "None" can lead a derived list. JUDGMENT.
- **One tag per option**: a debug build panics where two options carry one value, as a token
  defined twice does; the check is the map the group builds anyway. JUDGMENT.
- **A list box** holds what it has chosen, so its one type parameter is that: `Option<T>`, or a
  set of `T`, as SwiftUI's `List(selection:)` takes. A list starts with nothing chosen as a
  matter of course, where a picker always shows a value, so its single choice is `Option<T>`
  where a picker's is `T`. Typed by its choice, a list box refuses a handler of another shape
  than its binding. VERIFIED (ran): `choiceprobe/` infers `Listbox<Option<MessageId>>` and
  `Listbox<HashSet<MessageId>>` from the binding, its options' closures inferring `MessageId`;
  a bare `u64` fails with the `Chosen` message, and a `HashSet` handler beside an `Option`
  binding fails as a closure mismatch.
- **The face** shows the chosen option's name, type-ahead finds options by their names, and the
  face reserves the widest name's width, as today (VERIFIED (read):
  `gui/src/controls/picker.rs`, `Text::derive(shown).derive_reserved_texts(names)`). A name is
  read from the option's `Label`, which is a description until it is built (VERIFIED (read):
  `gui/src/control.rs`, `Label { view: Child, name }`), so the face and type-ahead read every
  name without building an option. Reserving the widest name shapes every name once, cached by
  name: a picker of 10,000 options pays that once, as a native `<select>` measuring its options
  does; a longer choice is a list box (JUDGMENT).
- **The open list** is windowed by `lists-design.md`'s engine, its behavior a list box holding
  the picker's one value, so opening builds about a screen of options (today's builds every
  option: VERIFIED (read), `Column::new().derive_children(..)` in `picker.rs`).
- **A label that changes** is a derived text with a derived name
  (`Label::new(Text::derive(..)).derive_name(..)`), since an option is kept while its tag
  stays. Today a radio whose text changes is built again (VERIFIED (read):
  `gui/src/controls/radio.rs`, keyed by `(index, name)`); keyed by tag it is kept. JUDGMENT.

`Picker::new` keeps taking a caption, so its type parameter is the only new thing an app meets,
as `AGENTS.md`'s "a view's one type parameter being the type of a value it holds" says.

### 1.4 Nothing chosen

The chosen value may match no option. Then the face's value part is empty, no option reports
itself selected or checked, a radio group's one tab stop is its first enabled radio, and the web
renders an empty hidden option selected (`<option value="" hidden selected>`) so the `<select>`
shows nothing, as the drawn face does. A reader only ever chooses an option that is there, so
`handle_choose` never hears a value no option carries. A picker or a radio group that starts
with nothing chosen makes `Option<V>` its `T` and tags its options `Some(v)`; a list box chooses
`Option<T>` or a set already. JUDGMENT.

Alternative left out: `bind_chosen(Signal<Option<T>>)` on every choice, which makes the common
always-chosen picker carry an `Option` and departs from SwiftUI's tag.

### 1.5 The behaviors (foundation)

`Roster`, `Collection`, `Member` and `Popup` take what the collection holds where today they take
an index (VERIFIED (read): `foundation/src/behavior/collection.rs`,
`Controlled<Option<usize>>`, `members: Vec<(usize, ControlHandle)>` searched linearly, `count()`
one past the greatest index registered).

```rust
/// What a collection holds: a value always chosen (a radio group's, a picker's list's), or a
/// list box's choice.
pub trait Held: Clone + 'static {
  /// What each member carries.
  type Option: Choice;
  /// What its choice is bound as: `T` for `One<T>`, the choice itself for a list box's.
  type Bound: Clone + 'static;
}

/// A value always chosen.
#[derive(Clone)]
pub struct One<T>(pub T);

impl<T: Choice> Held for One<T> { type Option = T; type Bound = T; }
impl<C: Chosen> Held for C { type Option = C::Value; type Bound = C; }

/// The options a collection moves through, built or not: what its keys, its type-ahead and a
/// closed face read.
pub trait Options<T> {
  fn count(&self) -> usize;
  fn value(&self, index: usize) -> T;
  fn index_of(&self, value: &T) -> Option<usize>;
  /// The option's name where its label states one; none where its content names it.
  fn name(&self, index: usize) -> Option<Rc<str>>;
}

impl<H: Held> Roster<H> {
  pub fn new(chosen: Controlled<H::Bound>) -> Rc<Self>;
  /// The options it moves through: a styled group's segments, or, where a third party places
  /// members by hand, its registered members in the scene's order.
  pub fn set_options(&self, options: Rc<dyn Options<H::Option>>);
  pub fn is_chosen(&self, value: &H::Option) -> bool;
  /// Asks for `value` to be chosen (one value, or a list box's single choice), unless it is.
  pub fn choose(&self, value: &H::Option);
  /// Chooses `value` and runs what a choice commits.
  pub fn commit(&self, value: &H::Option);
  pub fn set_commit(&self, commit: impl Fn(&H::Option) + 'static);
  pub fn set_dismiss(&self, dismiss: impl Fn() + 'static);
  /// Registers `value`'s built member until the current owner is disposed.
  pub fn register(self: &Rc<Self>, value: H::Option, member: ControlHandle);
  pub fn member(&self, value: &H::Option) -> Option<ControlHandle>;
  pub fn focus(&self, value: &H::Option);
  /// The member the group's one tab stop is on: the chosen one where it is enabled, else the
  /// first enabled.
  pub fn stop(&self) -> Option<H::Option>;
}

impl<P: CollectionPolicy, H: Held> Collection<P, H> {
  pub fn new() -> Self;
  pub const fn with_roster(roster: Rc<Roster<H>>) -> Self;
  pub fn roster(&self) -> Rc<Roster<H>>;
  pub fn set_chosen(self, chosen: H::Bound) -> Self;
  pub fn derive_chosen(self, chosen: impl Fn() -> H::Bound + 'static) -> Self;
  pub fn handle_choose(self, choose: impl Fn(H::Bound) + 'static) -> Self;
  pub fn bind_chosen(self, chosen: Signal<H::Bound>) -> Self;
  /// The behavior of the member carrying `value`.
  pub fn member(&self, value: H::Option) -> Member<P, H>;
}

impl<P: CollectionPolicy, H: Held> Member<P, H> {
  pub const fn value(&self) -> &H::Option;
}

pub type RadioGroup<T> = Collection<RadioPolicy, One<T>>;
pub type Radio<T> = Member<RadioPolicy, One<T>>;
pub type Listbox<C> = Collection<ListboxPolicy, C>;
pub type ListboxOption<C> = Member<ListboxPolicy, C>;
/// A menu's commands, by their place among its entries.
pub type Menu = Collection<MenuPolicy, Option<usize>>;
pub type MenuOption = Member<MenuPolicy, Option<usize>>;

impl<T: Choice> Popup<T> {
  /// A closed popup choosing through `roster`, the list in its popup holding the same one
  /// value: its arrows and type-ahead move through the roster's options before the list is
  /// built.
  pub fn new(roster: Rc<Roster<One<T>>>) -> Self;
}
```

- **One source.** A group's keys, its type-ahead, a closed face and a lazy list box all read the
  roster's `Options`: a styled group's segments (kept once per change, `lists-design.md` section
  4), or a hand-placed group's registered members in the scene's order, so radios a third party
  lays out in a grid move in the scene's order, as a browser's radio group moves in tree order.
  The built members are found by value in a map, where today's are found by a linear search
  (`stop()` over n members is O(n²) today, O(n) then). The key tables (`CollectionKeys`,
  `PopupKeys`) keep working on places; the behavior maps a place to a value through the source.
  JUDGMENT.
- **`Held`** keeps one `Collection<P, H>` for the three shapes. VERIFIED (ran): the probe's
  `Held`, implemented for `One<T>` and blanket for every `Chosen`, compiles without overlapping.
- **Menus** keep places: a menu's entries are commands, not values. JUDGMENT.

### 1.6 The web's scriptless form

- **The key.** Each option's key is `Key::of(&tag)`, the 128-bit key `reconcile` and
  `View::set_key` already use in `packages/reactive`, written as 32 lowercase hex digits as the
  `value` of its `<option>`, and as the `value` of each radio's `<input type=radio>`, the radios
  of one group sharing a `name` taken from the group's node. JUDGMENT.
- **The seam.** A choice crosses as one atomic content, "a choice with its options" as
  `AGENTS.md` puts it, for the web's native form (`NativePicker`, a list box's `<select>`) and a
  native door's `ControlSink` alone: the drawn controls compose views and send no such content
  (the controls stage took the drawn contents out of the seam). `Content::Choice` holds
  `{ options: Rc<[(Key, Rc<str>)]>, chosen: Rc<[Key]> }`, none, one, or several for a list box's
  `<select multiple>`. The reader's change is reported as the keys now chosen
  (`Report::Change { content: Choice, applied: true, .. }`); the shared half maps each key back
  to its tag through the option source and asks `handle_choose`, and a refusal repairs the
  `<select>` to the truth, the smallest edit. A realization never sees `T`. JUDGMENT.
- **The server** renders `selected` on each option whose tag is chosen, and the hidden empty
  option where a picker's chosen value matches none (1.4).
- **Across the swap.** Adopting the server's `<select>`, the browser leaf compares its chosen
  keys with those it rendered. A difference is a choice the reader made before the bundle
  arrived; it is reported as applied, so the bound signal takes it before `Enhance` swaps the
  `<select>` for the drawn control, which then shows it. An unknown key is repaired. JUDGMENT.
- **Leaf parity.** The key must come out the same on the server leaf (64-bit) and the browser
  leaf (wasm32). VERIFIED (ran): xxh3 over std's `Hash`, as `packages/reactive`'s `Key::of`
  hashes (VERIFIED (read): `reactive/src/hash.rs`, `wide_of`), gives different values on x86-64
  and wasm32 for an enum, a `usize` and a `Vec<u8>`, and the same for a `&str`; xxhash-rust's
  `Xxh3` implements `Hasher::write` alone, so a derived enum's `isize` discriminant and a slice's
  length are hashed at the target's pointer width. A hasher that writes every integer
  little-endian and widens `usize` and `isize` to 64 bits gave identical values on both, equal
  to today's x86-64 ones (the probe is `keyprobe/`, beside this file, built with 1.99.0 natively
  and for `wasm32-unknown-unknown`, the wasm run under Node 22). So the key's hasher widens;
  section 10.
- **Collisions** of distinct tags are a 2^-128 event; a debug build panics where two tags of one
  choice share a key.
- **Size.** A `<select>` of 100,000 options is the scriptless form's cost; a list that long is a
  list box, whose scriptless form is `lists-design.md`'s choice 14 (JUDGMENT).

### 1.7 The terminal and the conformance suite

- **The terminal** (`skene-tui`) takes the same `Picker<T>`, `RadioGroup<T>` and `Listbox<C>`,
  name for name. It has no hydration and no keys; its reader-facing tree (`access_tree`,
  `access_outline`) shows each option by its name and its checked or selected state, never `T`.
  JUDGMENT.
- **The conformance probes** stay strings a page publishes. Each choice page declares its own
  enum and publishes `format!("{:?}", fruit.get_untracked())`, so scenarios read `"Cherry"`
  where they read `"Some(2)"` today (VERIFIED (read): `conformance/src/lib.rs:869-884`). One page
  keeps `Option<Fruit>` with no option for `None`, holding 1.4 on every platform. The web's key
  round trip is a browser test of its own (a choice made in the `<select>` before the bundle
  arrives survives the swap), not a conformance scenario, since only the web hydrates.
  JUDGMENT.

### 1.8 What changes in the prototype

`foundation/src/behavior/{collection,popup,tests}.rs` (the `Choice`, `Chosen` and `Held` traits,
`One`, the `Options` source, `Roster<H>` with its member map, `Collection`, `Member`, `Popup`);
`gui/src/controls/{picker,radio}.rs` and a new `listbox.rs` (`Listbox<C>`, `ListboxItem`, the
picker's open list windowed once `lists-design.md`'s engine lands), `gui/src/control.rs`
(aliases), `gui/src/controls/context_menu.rs` (`Menu` over places); the call sites in
`hello/src/lib.rs`, `conformance/src/pages.rs` (`picker`, `fruit`),
`gui-linux/tests/it/{contents,scale,roles}.rs` and `shell-probes/src/bin/{controls,rows}.rs`
(VERIFIED (read): the `grep` of `set_options`, `bind_chosen`, `derive_chosen`, `handle_choose`,
`set_item_builder`), each chain reordered so its options come last.

## 2. A part takes every state from its control

### 2.1 The rule

| State | A node no control encloses | A part, or a control's own node | A box that acts inside a control |
|---|---|---|---|
| `Checked`, `Indeterminate`, `Invalid`, `Open` | never holds | the control's | its enclosing control's |
| `Disabled` | never holds | the control's, composed outward (2.3) | its enclosing control's |
| `Hovered`, `Pressed`, `Focused`, `FocusVisible` | its own | the control's (2.2) | its own |
| `FocusWithin` | its own | its own | its own |

- **The control's interaction states.** `Hovered`: the pointer is over the control's node, any
  node built inside it (an overflowing part, its open popup's content: 2.6), or its caption
  placed beside it (2.5). `Pressed`: a press the control owns (its beside caption's included),
  or its behavior holding it pressed from the keyboard (`ControlState::pressed`). `Focused` and
  `FocusVisible`: focus is on the control's node.
- **A control's own node** takes them like its parts, so a picker's face, which is the control's
  own node, highlights while its beside caption is hovered.
- **`FocusWithin` stays a node's own**: it says where focus is beneath that node, which only the
  node can answer.
- **The change from today**: `Hovered` joins the states a part takes from its control
  (VERIFIED (read): `foundation/src/condition.rs`, `follows_control` is the control states and
  `Focused`, `FocusVisible`, `Pressed`; "Hovered stays the node's own"). So hovering a checkbox's
  caption highlights its face, which `Checkbox::default_face` already asks for with
  `.when(State::Hovered, SystemColors::CONTROL_HOVER)` (VERIFIED (read):
  `gui/src/controls/checkbox.rs:64`).
- **`State`'s grouping** becomes three kinds with one method each: `is_control_state`
  (`Checked`, `Indeterminate`, `Disabled`, `Invalid`, `Open`), `is_interaction` (`Hovered`,
  `Pressed`, `Focused`, `FocusVisible`), and `FocusWithin` neither. `follows_control` goes,
  being their union. JUDGMENT.

### 2.2 How the resolver finds the control

The control is found once, when a node is created, never by walking up. `Cx::control` is set
when a control builds what is inside it (VERIFIED (read): `gui/src/control.rs`,
`inner.control = Some(id)`), carried by `Cx::child` and `Cx::inherit` to every node built
beneath, segments, derived rebuilds and overlays included, and stored as `Entry::control` at
creation; a control's own entry names itself (VERIFIED (read): `foundation/src/seam/tree.rs`,
`insert_entry`). A node is created once, inside one control, so it never changes control.
Resolution reads the node's reported states, its control's reported states, the states its
control's beside captions add (2.5) and the control's `ControlState`, all signals:

```rust
fn holds(&self, state: State) -> bool {
  match (&self.control, state) {
    (_, State::FocusWithin) => self.own(state),
    (None, state) if state.is_control_state() => false,
    (None, state) => self.own(state),
    (Some(control), state) if state.is_control_state() => control.holds(state),
    (Some(_), state) if self.acting => self.own(state),
    (Some(control), state) => control.interaction(state),
  }
}
```

`acting` is a box that acts (`Interaction::Acting`: an activatable `Container`, a link), never
a control's own node, which falls to the last arm with its parts; `control.holds` reads the
`ControlState` (its `disabled` already composed, 2.3), and `control.interaction` is the
control's reported state, its beside captions' and, for `Pressed`, its behavior's press. Today's
resolver gives a control's own node its own reported states alone (VERIFIED (read):
`Resolution::holds`, the `interactive` arm covering `Acting` and `Control`), which would miss
the caption's hover and a keyboard press. The realization is asked for each interaction state at
the control rather than at the part, as `set_record` already does for focus and press (VERIFIED
(read): `tree.rs`, `watch_states::<R>(control, at_control)`), now with `Hovered` among them.

A scroll box's bar parts name the scroll box as their control (section 5); `Part`'s doc
becomes "a node built as a part is that part of the nearest enclosing control, or of the scroll
box whose bar it is".

### 2.3 A control inside a control

- **Its parts follow the inner control**, the nearest: a radio's face follows the radio, not the
  group; an open picker's options follow themselves, not the face. The outer control's
  `Checked`, `Open` or hover never reach them.
- **`Disabled` composes**: a control is disabled when it or any control enclosing it is, as a
  `<fieldset disabled>` disables the controls inside it. A disabled radio group disables its
  radios (dimmed, out of the Tab order, swallowing input), and a disabled picker's open list,
  should one be open, disables its options. Today a group's `disabled` reaches no member
  (VERIFIED (read): `ControlHandle::is_disabled` reads the member's own `members.disabled`).
  Each control's entry records the control enclosing it (`Entry::outer`), and the control's
  state effect ORs the outer control's `disabled` into its own. On the web each native input
  takes its own `disabled` attribute, so the form enforces it without script. JUDGMENT; section
  9, choice 4.

### 2.4 A box no control encloses, and a box that acts

- **A box no control encloses keeps its own states**, as the answer says: a text inside a
  clickable row is hovered when the pointer is over the text, as CSS's `:hover` on it is.
- **A box that acts** (an activatable `Container`, a link) takes focus and presses itself, so it
  keeps its own interaction states inside a control too, and its own children keep theirs: it is
  not a control, so it does not lend its states downward. Inside a control it takes the
  control's `Checked`, `Disabled` and the rest like any part. JUDGMENT; this matches today for
  focus and press (VERIFIED (read): `Resolution::holds`) and extends it to hover.

### 2.5 A caption beside its control

A caption placed beside its control (`LabelPlacement::Beside`, a sibling in a segment,
`Interaction::Answering`) is built inside the control (VERIFIED (read): `gui/src/control.rs`,
the label built with `inner`, `cx.answering = beside`), so its own states are the control's.
HTML makes a labeled control match `:hover` and `:active` while its label does, and Chromium
forwards both from a `<label>` to its control, so the control is hovered while the pointer is
over its caption and pressed while its caption is pressed (JUDGMENT: HTML's `:hover` and
`:active` definitions; not run). The spine does it, once for every realization: an `Answering`
node's reported `Hovered` and `Pressed` fold into its control's `Entry::answered:
Signal<StateSet>`, which resolution reads beside the control's own reported states, and the
spine asks the realization for `Hovered` and `Pressed` at a control's beside captions whenever
it asks for them at the control. A press on the caption still focuses the control without
clicking it (a default that stands).

### 2.6 An open popup

The drawn hover chain stops at an open overlay's root today (VERIFIED (read):
`gui-linux/src/shell.rs:930`, `hit` returns `hit_in(&open.node, ..)` alone). On the web a
popover is promoted to the top layer in place, so it stays a descendant of its anchor's element,
and `:hover` holds on every flat-tree ancestor of the hovered element. For parity, the drawn
chain continues from the overlay's root to its scene ancestors, so a picker's face is hovered
while the pointer is over its open list (its default draws `Open` over `Hovered`, so nothing
changes on screen). JUDGMENT.

### 2.7 Each platform

- **The web keeps states**, so the rule becomes selectors. A node built inside a control
  compiles its state variants into one `@scope ([data-skene-control]) to ([data-skene-control])`
  block, each state on the scope's root (`:where(:scope:hover) .skene-…`), the `to` limit keeping
  a nested control's parts out of the outer control's states. Every variant of that node sits in
  the block, its media ones included: CSS ranks a scoped rule above an unscoped one of equal
  specificity by scope proximity, before order of appearance, which would break "rule order
  alone decides between variants" (JUDGMENT: CSS Cascade 6's scope proximity). The node's class
  address includes "inside a control", since the same record compiles differently there.
- **The web without script**: `Checked` and `Indeterminate` read the control's native input
  where it has one (`:scope:has(> input:checked)`), so a checkbox toggled before the bundle
  arrives restyles its face. A beside caption is a `<label for>` before the control, so the
  control's parts follow the caption's hover through HTML's labeled-control rule, and the
  caption's own variants use `:where(:has(+ [data-skene-control]:hover))`. JUDGMENT.
- **`@scope`** is in Chromium since 118, Safari since 17.4 and Firefox since 146 (JUDGMENT: from
  memory, not checked here), inside the doctrine's "latest CSS standards".
- **Every other platform** (Linux, Windows, the compositor platforms and the terminal) resolves
  every state through the spine, its `Keep` holding no states, so it takes the rule with no
  code of its own; the realization only reports hover, press and focus where the spine asks.
  JUDGMENT for the platforms not yet built.

### 2.8 What changes in the prototype

`foundation/src/condition.rs` (the three kinds), `foundation/src/seam/tree.rs` (`Resolution`
with `acting` in place of `interactive`, `Entry::answered`, `Entry::outer`, `set_record`'s
watch, the `Answering` fold, `set_control_state` composing `disabled`, the unit test
`a_part_resolves_focus_press_and_control_states_against_its_control` rewritten to the table),
`gui/src/control.rs` (record the outer control), `gui-linux/src/shell.rs` (the hover chain
through an open overlay). `Canvas::states()` already returns the resolved states, so a painted
mark follows the rule with no change (VERIFIED (read): `gui/src/painted.rs`,
`tree::resolved_states`).

## 3. `set_autofocus`, and what opening focuses

### 3.1 The surface

```rust
TextField::new("Name").set_autofocus(true)
```

Every control carries `set_autofocus(bool)` through `control_members!`; a third party's
`Control<B>` has it with no code of its own. It is read once, when the control mounts, so it has
no `derive_` twin, as `set_shortcut` and `set_handle` have none: it asks for focus at a moment,
which no later value can change. JUDGMENT; `AGENTS.md` names controls, so an activatable
`Container` does not take it until asked.

The spine keeps the candidates: `tree::set_autofocus::<R>(id)` marks a node, and
`tree::autofocus_in::<R>(root) -> Option<NodeId>` answers the first marked node beneath `root`
in the scene's order that is shown, not inside a closed overlay, not disabled, and takes focus
by its facts. The marked nodes are kept as a list, so the answer costs the candidates, never a
walk of the tree. The realization calls it when something opens.

### 3.2 What each opening focuses

| What opens | Focus goes to | When it closes |
|---|---|---|
| A window | its first autofocus control; else nothing, `Tab` then starting at the first stop (today) | |
| A popup (`Overlay::popup()`) | its first autofocus control; else focus stays where it is, unless the popup's own behavior moves it (a picker's list to its chosen option, a context menu to its first operable item: today) | focus inside it returns to what held focus when it opened, where that still takes focus; else to the nearest node before it that does (today's rule) |
| A modal (item 3) | its first autofocus control; else its first control that takes focus; else the modal's own box | focus returns to what held it when it opened |
| A subtree mounted later (a derived view, a lazy list's row) | its autofocus control, only while nothing in its window is focused and no autofocus has acted there yet | |

These follow HTML's autofocus processing (the first candidate, once per document), its popover
focusing steps (an autofocus descendant only) and its dialog focusing steps (an autofocus
descendant, else the first focusable, else the dialog, a fallback WHATWG has debated narrowing),
and the restoring of focus when a popover hides. JUDGMENT: from the HTML specification; not run
here. A lazy list's row built by scrolling therefore never takes focus from what holds it.

- **Shown?** An autofocused control shows its focus until a pointer press comes first, as
  Chromium shows a focus a page gives before any interaction. The shell's `keyboard_modality`
  starts `false` today (VERIFIED (read): `shell.rs:512`), which hides it; it becomes three-way,
  starting at "no input yet", which shows it, a pointer press then hiding it and a key showing
  it. JUDGMENT: Chromium's `:focus-visible` heuristic, for the web tier to confirm.
- **A reader** hears the focus at open, so Orca on mutter announces an autofocused control as it
  announces the control `GtkWindow` focuses first. Without autofocus it announces the window
  alone: the GNOME Wayland difference `ROADMAP.md` item 4 lists, now chosen rather than open.
  JUDGMENT.
- **The web** renders `autofocus` on the control's focusable element (its `<input>`, `<select>`
  or the box carrying `tabindex`), so a page with no script focuses it, and the browser leaf keeps
  that focus as it adopts the markup. The terminal follows the table as Linux does.
- **iOS and Android** focus an autofocused text field as their own field focused when a screen
  appears: iOS makes it first responder, which raises the keyboard; Android focuses it and
  leaves the keyboard to the window's soft-input mode. A control that is not a text field takes
  no focus there without a hardware keyboard, as the platforms' own controls take none (JUDGMENT,
  for item 6 and item 7 to confirm on hardware).

### 3.3 What changes in the prototype

`gui/src/control.rs` (`Members::autofocus`, the setter, the mark at build),
`foundation/src/seam/tree.rs` (the candidates), `gui-linux/src/shell.rs` (focus the candidate on
the first frame after the root attaches and on `sync_overlays`' open; record the opener in `Open`
and restore it in `close_overlay`; the three-way modality). The notes example gives its editor
`set_autofocus(true)`, its one field; hello stays without. JUDGMENT.

## 4. Hit testing: overflow reaches the control

### 4.1 The rule

A hit is the deepest shown node, in reverse paint order, whose hit area holds the point, cut
only by the boxes that clip.

| Clips the hits of what it holds, to its own shape | Does not clip hits |
|---|---|
| a box with `set_clip(true)`, to its edges and rounded corners, as it clips paint | a control (changed) |
| a scroll box, to its viewport | every other box and layout view |
| an overlay's surface | a segment, a scope |
| the window | |

- **A node's own hit area** is its box rounded by its corner radii, moved by its `offset`, as
  CSS's `border-radius` shapes hit testing and a transform moves it. A ring and a shadow take no
  hit, as an outline and a `box-shadow` take none; a node at opacity 0 still takes hits, as in
  Chromium; a hidden node takes none. JUDGMENT for the CSS and Chromium facts.
- **So a press on content visibly overflowing its control reaches the control**: the
  overflowing part is the control's descendant, the press's owner is the deepest control or
  acting box on the chain (VERIFIED (read): `shell.rs`, `owner`), and nothing between cuts it.
  Hit areas follow what is painted, since only what clips paint clips hits.
- **Pruning.** A node keeps the bounds of its subtree's hit areas, refreshed with layout and
  with each `offset` change, and the walk skips a subtree whose bounds miss the point, as
  Chromium skips a box whose overflow rectangle misses it (JUDGMENT). The walk today descends into
  every node that does not clip, whether or not the point is inside it (VERIFIED (read):
  `shell.rs:945-990`, `go` returns early only where `!inside && clips`); with controls clipping no
  more, a pointer move over a long non-lazy column would otherwise visit every row's subtree.
- **Today** a control takes a point inside its own box alone (VERIFIED (read):
  `gui-linux/src/shell.rs:944-968`, `clips = look.clip || is_control() || Scroll`), and a
  rounded corner takes hits.

### 4.2 Each platform

- **The web** has the rule from CSS: an overflowing part is a DOM descendant of its control, so
  the press reaches the control's element, and `border-radius` and `overflow: clip` shape and
  cut hits as above.
- **The terminal** takes it through `skene-layout`'s runtime (`ROADMAP.md` item 2), in cells.
- **iOS, macOS and Android** stop a hit at a view's own bounds by default: `UIView.hitTest`
  asks `point(inside:with:)` of each view before its subviews, `NSView.hitTest` likewise, and an
  Android `ViewGroup` dispatches a touch only to children whose bounds hold it. Skene's own view
  classes answer by the subtree's bounds (the same as the pruning bounds) and by the rounded
  shape, so the rule holds; a native view adopted through the door keeps its toolkit's own hit
  testing (JUDGMENT, for items 6 and 7).
- **Windows** draws everything, so it takes the drawn rule as Linux does.

### 4.3 What changes in the prototype

`shell.rs` `hit_in`: `clips` loses `is_control()`; the inside test becomes a rounded
rectangle's (the same `Corners` the painter's `push_clip` cuts with); each node keeps its
subtree's hit bounds and the walk prunes by them. Driver tests: a control whose part overflows
its box by an `offset` takes a press on the overflow (its probe changes); the same control inside
a `set_clip(true)` box does not; a press in the corner of an 18-pixel acting box rounded by 9,
outside the circle, reaches what lies beneath; the frame benchmark's hover frames at 1000 rows
do not grow.

## 5. The scroll bar as `Scroll`'s parts

### 5.1 The surface

```rust
Scroll::new(ScrollAxes::Vertical)
  .set_thumb_builder(|axis| Scroll::default_thumb(axis).set_corner_radius(0))
  .set_child(rows)
```

```rust
impl Scroll {
  /// Makes the bar's track along `axis` with `builder`; `Scroll::default_track(axis)` where
  /// unset. The thumb is placed in it, so a child it holds is replaced.
  pub fn set_track_builder(self, builder: impl Fn(Axis) -> Container + 'static) -> Self;

  /// Makes the bar's thumb along `axis` with `builder`; `Scroll::default_thumb(axis)` where
  /// unset. Its length and its place along the track are the scroll's, its least length its
  /// least size along `axis`.
  pub fn set_thumb_builder(self, builder: impl Fn(Axis) -> Container + 'static) -> Self;

  pub fn default_track(axis: Axis) -> Container;
  pub fn default_thumb(axis: Axis) -> Container;
}
```

A part repeated per axis comes from a builder, as one repeated per option does
(`set_option_builder`, `set_item_builder`); its defaults are public, as every part's are. One
builder serves both axes because a thumb's least length is a size along its axis, which one
`Container` cannot state for both (a vertical thumb's `min_width` would be its thickness).
JUDGMENT.

### 5.2 The defaults: today's look

Today the realization draws a vertical bar alone: a thumb 4 pixels wide whose right edge is 3
from the box's, inset 2 at each end, at least 24 long before the insets, its corners rounded by
2, in the inherited ink at alpha 70 of 255, 110 while the scroll box is hovered (VERIFIED (read):
`gui-linux/src/paint.rs:1072-1095`). As parts (`Inset::new(top, trailing, bottom, leading)`,
VERIFIED (read): `foundation/src/value.rs`):

```rust
pub fn default_track(axis: Axis) -> Container {
  match axis {
    Axis::Vertical => Container::new().set_width(10).set_padding(Inset::new(2, 3, 2, 3)),
    Axis::Horizontal => Container::new().set_height(10).set_padding(Inset::new(3, 2, 3, 2)),
  }
}

pub fn default_thumb(axis: Axis) -> Container {
  let thumb = Container::new().set_corner_radius(2).set_background(
    Expr::ink()
      .with_alpha(70. / 255.)
      .when(State::Hovered, Expr::ink().with_alpha(110. / 255.)),
  );

  match axis {
    Axis::Vertical => thumb.set_width(4).set_min_height(20),
    Axis::Horizontal => thumb.set_height(4).set_min_width(20),
  }
}
```

The thumb's length becomes the track's inner length times the viewport over the extent, at
least 20, where today it is the box's length times that ratio, at least 24, less 4: within 4
pixels of today, the same at no overflow. JUDGMENT; the screenshot goldens move by that.

**The color needs the inherited ink as a value**: CSS's `currentColor`, the `Paint::Ink` a
`Painted` box already draws with. `Expr<Color>` in `packages/` has `with_alpha` and `mix` but no
current color (VERIFIED (read): `packages/foundation/src/spine/expr.rs:636-680`), and the
prototype has no `Expr` (VERIFIED (read): `foundation/src/value.rs`). The proposal is
`Expr::<Color>::ink()`, the inherited ink in force, a leaf of the same kind as
`Expr::<Length>::font_size(multiple)`, which already reads an inherited value (VERIFIED (read):
`expr.rs`). The web compiles it to `currentColor`, and `packages/`' compiler writes `with_alpha`
as `oklch(from <color> l c h / <alpha>)` (VERIFIED (read): `packages/web/src/expr.rs:62-63`),
here `oklch(from currentColor l c h / 0.2745)`; where an engine resolves no relative color from
`currentColor`, `color-mix(in oklab, currentColor 27.45%, transparent)` is the floor (JUDGMENT).
The prototype gains the minimal `Expr<Color>` it needs. The alternative is two `SystemColors`
tokens (`SCROLL_THUMB`, `SCROLL_THUMB_HOVER`), which lose today's following of a themed region's
ink. JUDGMENT; section 9, choice 7.

### 5.3 States

The scroll box is its bar parts' control (2.2): `Hovered` while the pointer is over the scroll
box, its content included (today's brighter thumb); `Pressed` while its bar is pressed (the
thumb dragged, the track held); `Focused` and `FocusVisible` while the scroll box has focus,
where it takes focus (the web's tab stop; Linux has none today: VERIFIED (read): `shell.rs`,
`focusable` admits controls and acting boxes alone; `lists-design.md` section 5 makes a scroll
box with no focusable content a stop). The control states never hold. A bar that thickens only
under the pointer, as Chromium's overlay bars do, is not expressible: the bar would have to be a
control of its own, and its parts would then lose the scroll box's hover that today's look
needs. JUDGMENT.

### 5.4 Interaction and accessibility

- **Dragging the thumb** follows the pointer, which the bar keeps until the release.
- **A press on the track** follows the platform: "match the native, then follow Chromium". On
  Linux the native rule is the desktop's (GTK's `gtk-primary-button-warps-slider`, which Firefox
  also follows on Linux, read where XSETTINGS or the settings files state it, `true` by GTK's
  default): a primary press jumps the thumb to the point, and a middle or `Shift` press pages;
  `false` swaps them. Windows pages toward the press and repeats while held. The terminal, which
  has no native rule, pages as Chromium does. JUDGMENT; section 9, choice 8.
- **A press on the bar** never moves focus; the wheel over the bar scrolls the box.
- **A reader** skips the bar, and the scroll box reports its position as it does now.
- **Placement**: the bar shows on an axis while the content overflows it, over the content and
  taking no room (today); with both bars, the vertical track stops short of the horizontal one's
  thickness; in a right-to-left context the vertical bar sits at the leading edge, as Chromium
  places it. JUDGMENT.

### 5.5 The scene and the seam

A track is a node of the scroll box (`Interaction::ScrollBar { axis }`), out of the scrolling
content's flow; the thumb is its one child (`Part::Thumb`). The realization lays the track along
the viewport's edge, unscrolled and above the content, and lays the thumb: its length from the
viewport over the extent, floored by its least size along the axis, and its place along the
track as a paint-only translation updated with every scroll. Nothing crosses the reactive graph
per scroll frame, as the doctrine asks of motion. A realization says whether it draws bars
(`Realization::DRAWS_SCROLL_BARS`, a constant with no default, since drawing and not drawing are
both behaviors): Linux, Windows and the terminal do, and the web, iOS, macOS and Android do not
realize the bar's nodes. JUDGMENT.

### 5.6 Each platform

| Platform | The bar | From the parts |
|---|---|---|
| Linux, Windows | the parts, drawn | everything |
| The terminal | the parts, boxes one cell thick, the thumb's ends in eighth-cell glyphs | their fills; one `TerminalStyle` glyph property of the thumb |
| The web | the engine's own bar | `scrollbar-color` (the thumb's fill, then the track's, their `Hovered` variants under the scroll box's `:hover`) and `scrollbar-width`, compiled into the scroll box's own class: no node is realized |
| iOS | `UIScrollView`'s indicators, kept with their behavior (the draggable indicator) | the thumb's fill picks `indicatorStyle` (black or white) where it is dark or light |
| macOS | `NSScrollView`'s scrollers, overlay or legacy as the user set them | the thumb's fill picks the scroller's `knobStyle` (dark or light) |
| Android | the platform's scroll bars | the thumb's and track's fills as drawables (`setVerticalScrollbarThumbDrawable`, API 29, the floor) |

The compositor platforms keep their own indicators because they carry functionality the
governing rule keeps (the user's scroller settings, the draggable indicator, the reader's
scroll gestures). A drawn bar on the web over `scrollbar-width: none`, its thumb placed by a
scroll-driven animation (`animation-timeline: scroll()`), is an upgrade a signal could prove
later. The `TerminalStyle` properties `scrollbar_thumb`, `scrollbar_track` and
`scrollbar_glyphs` (VERIFIED (read): `packages/foundation/src/terminal/ext.rs:179-185`) become
the parts' fills and the thumb's glyph property, for item 1. JUDGMENT for every row.

### 5.7 What changes in the prototype

`gui/src/views.rs` (`Scroll` builds its bars), `foundation/src/seam/node.rs`
(`Interaction::ScrollBar`), `foundation/src/seam/realization.rs` (the constant), and in
`gui-linux`: `layout.rs` (the bar's nodes), `paint.rs` (`paint_scroll_bar` goes), `shell.rs` (the
bar's hits, the thumb's drag, the track's press under the desktop's setting), `x11.rs` and the
settings reader (`Gtk/PrimaryButtonWarpsSlider`), `access.rs` (the bar skipped). Driver tests:
the screenshots within the thumb's 4 pixels; a drag on the thumb scrolls (the reader's scroll
position); a primary press on the track jumps, and a `Shift` press pages; a horizontal bar
appears for `ScrollAxes::Horizontal`; a `set_thumb_builder` restyle shows; at scale 1.5 on the
scaled legs.

## 6. `Painted` and `ContextMenu` in the foundation

### 6.1 Where they move, and what moves with them

`AGENTS.md`: "`Painted`, `Canvas` and `Painting` live in `skene-foundation`", and "`ContextMenu`
in `skene-foundation`". In the prototype the view layer is the facade's: `View`, `Cx`, `Node`,
`NodeHandle`, `AnyView` and every view live in `gui` over `Active` (VERIFIED (read):
`gui/src/view.rs`, `gui/src/lib.rs`), and the foundation has no view. `ContextMenu` composes
`Overlay`, `Column`, `Container`, `Control<Menu>`, `AsPart`, `Label` and `Text` (VERIFIED (read):
`gui/src/controls/context_menu.rs`), so it cannot move alone.

The proposal: the view layer moves into `skene_foundation::view`, generic over the realization
(`View<R: Realization>`, `Cx<R>`, `AnyView<R>`), and every built-in view moves with it, as in
`packages/` (VERIFIED (read): `packages/foundation/src/facade.rs`, the facade's `pub trait View:
$crate::View<$platform>` with its blanket impl). `skene-gui` declares a type of its own per view
(`pub struct Painted(skene_foundation::Painted<Active>)`, `impl PaintedBase for Painted {}`), its
documentation its own, and app code keeps writing `impl View` with no type parameter. Moving
only `Painted` and `ContextMenu` would leave two standards, which the house rules forbid. It is
mechanical (`Active` becomes `R`) but large: about 4,900 lines move (`view.rs`, `views.rs`,
`wrappers.rs`, `overlay.rs`, `control.rs`, `controls/`, `painted.rs`: VERIFIED (read), their line
counts), and the `*Base` traits and newtypes add several hundred. JUDGMENT; section 9, choice
11.

### 6.2 `Painted` and `Canvas`

```rust
Painted::new(|canvas: &mut Canvas| {
  canvas.set_transform(canvas.to_device());

  let size = canvas.layout_size();

  canvas.fill(&Circle::new((size.width / 2., size.height / 2.), size.height / 2.), Paint::Ink);
})
.set_width(48)
.set_height(48)
.set_description("Level")
```

```rust
impl Canvas<'_> {
  /// The box's size in device pixels, its edges on the device's pixel grid: whole numbers.
  pub const fn size(&self) -> kurbo::Size;
  /// Device pixels per layout unit along each axis: the window's scale on a pixel platform,
  /// a cell's pixel width and height in the terminal.
  pub const fn scale(&self) -> kurbo::Vec2;
  /// The transform from layout units (pixels, or cells) to device pixels.
  pub fn to_device(&self) -> kurbo::Affine;
  /// The box's size in layout units, as laid out.
  pub const fn layout_size(&self) -> kurbo::Size;
  /// The box's states, its control's where it is a part.
  pub fn states(&self) -> StateSet;
  pub fn fill(&mut self, shape: &impl Shape, paint: impl Into<Paint>);
  pub fn stroke(&mut self, shape: &impl Shape, stroke: &Stroke, paint: impl Into<Paint>);
  pub fn push_clip(&mut self, shape: &impl Shape);
  pub fn push_opacity(&mut self, opacity: f32);
  pub fn pop(&mut self);
  /// The transform what follows is drawn under, from device pixels.
  pub fn set_transform(&mut self, transform: kurbo::Affine);
}

pub enum Paint {
  /// The box's ink, as it inherits it, resolved when the picture is drawn.
  Ink,
  Color(Color),
  Brush(peniko::Brush),
}
```

- **Device pixels.** The recording's origin is the box's top leading corner on the device's
  grid, one unit one device pixel: a one-pixel line at `x = 0.5` covers exactly one device
  column at any scale, which a picture in layout pixels cannot promise at 1.5. JUDGMENT.
- **The scale is per axis**: the terminal's cell is not square, and the web's device size rounds
  per axis (`packages/` already scales by `device / css` per axis: VERIFIED (read):
  `packages/web/src/browser/canvas.rs:354`). JUDGMENT.
- **`stroke` takes kurbo's `Stroke`**, its joins, caps and dashes, where the prototype takes a
  width (VERIFIED (read): `gui/src/painted.rs`); `packages/` already does. JUDGMENT.
- **`Paint::Ink` stays**, resolved when the picture is replayed, so a change of ink (a hover, a
  theme) replays without recording again. `packages/`' `Canvas` lacks it. JUDGMENT.
- **The fallback** stays `set_fallback_builder`, an `Enhance` on `Painting::available()`.

### 6.3 When it records again

When a signal its closure read changes, when a state it read through `Canvas::states()` changes,
and when its device size or its scale changes: laid out at another size, moved to a window,
popup or output at another scale, a terminal's cell changing its pixel size. A move that keeps
its device size, and a change of ink, replay the same recording. The prototype already records
again on a scale change (VERIFIED (read): `gui/src/painted.rs`, the `laid` memo keys on the
reported scale); what changes is that the key becomes the device size and the per-axis scale.

### 6.4 The recording and the renderers

`Picture` stays Skene's recording of kurbo's shapes and peniko's brushes (fill, stroke, clip,
opacity, pop, transform), `Paint::Ink` symbolic in it, replayed through a `Renderer` trait each
realization implements over the renderer it owns (vello_cpu's `RenderContext`, vello_hybrid's
`Scene`), as `packages/`' `Renderer` does. vello offers no public recording both renderers
replay: vello_common 0.3's `record` module is the renderers' internal command graph, and the
prototype's lockfile pairs vello_cpu 0.3 (vello_common 0.3) with vello_hybrid 0.2 (vello_common
0.2) (VERIFIED (read): the crates' sources in the cargo registry, and `spikes/linux/Cargo.lock`).
So "a vello scene", in `AGENTS.md` and the author's answer, is realized as this recording in
vello's vocabulary, replayed into vello; the doctrine's wording is the author's to adjust.
Nothing here needs vello to change, so no upstream report.

### 6.5 `Painting::available()`

A signal each realization answers through a seam method with no default, since "never paints"
and "always paints" are both behaviors, not an absence: Linux, Windows and the compositor
platforms true from the start, the server false for good, the browser true once live, the
terminal once its graphics probe is answered. The prototype's `const fn available() -> bool {
true }` (VERIFIED (read)) becomes the tracked read. JUDGMENT.

### 6.6 Each realization

- **Linux** snaps the box's window rectangle to the device grid of the surface it is drawn in
  (the window's scale, or a popup's), records at that size and scale, and replays translated by
  the snapped origin under the box's clip, in the scene's order, on the CPU or the GPU path
  alike. **Windows** does the same at the monitor's scale, recording again on `WM_DPICHANGED`.
  JUDGMENT.
- **iOS, macOS, Android** draw the recording into the box's own platform view at its
  `contentScaleFactor`, `backingScaleFactor` or display density, recording again when the scale
  changes (a trait collection change, `viewDidChangeBackingProperties`, a configuration change).
  JUDGMENT, for items 6 and 7.
- **The web** sizes its `<canvas>` from the `device-pixel-content-box` the engine reports, or the
  content box times the pixel ratio where it reports none, takes the scale per axis as `device /
  css`, and records again when either changes, as `packages/` does today. `Ink` is the canvas
  element's computed `color`, read when it paints; the box records nothing new when the color
  changes but must replay, which needs an event for a computed change (a `transitionrun` on
  `color` is the one a page hears). JUDGMENT, unproven; an open point for item 1.
- **The terminal** records at the box's cells times the cell's pixels, scale `(cell width, cell
  height)`, shows it through the kitty graphics protocol where proven, and records again when the
  cell's pixel size changes (a font change the terminal reports through mode 2048's pixel size).
  `Ink` is the box's resolved foreground, through the colors queried at launch on a 16-color
  terminal. JUDGMENT.

### 6.7 `ContextMenu` and `MenuEntry`

The surface stays the prototype's, with `MenuEntry` brought to the doctrine's names
(`handle_activate`) and `packages/`' constructors:

```rust
ContextMenu::new()
  .add_entry(MenuEntry::new("Refresh").handle_activate(move || refresh()))
  .add_entry(MenuEntry::copy())
  .set_child(Text::new("Inbox"))
```

```rust
impl MenuEntry {
  /// A command labelled `label`, always offered.
  pub fn new(label: impl Into<Rc<str>>) -> Self;
  /// The standard entries, offered as the realization's `clipboard_availability` says.
  pub fn cut() -> Self;
  pub fn copy() -> Self;
  pub fn paste() -> Self;
  pub fn delete() -> Self;
  pub fn select_all() -> Self;
  pub fn handle_activate(self, activate: impl Fn() + 'static) -> Self;
  pub fn set_label(self, label: impl Into<Rc<str>>) -> Self;
  pub fn derive_label<S: Into<Rc<str>>>(self, label: impl Fn() -> S + 'static) -> Self;
  pub fn set_disabled(self, disabled: bool) -> Self;
  pub fn derive_disabled(self, disabled: impl Fn() -> bool + 'static) -> Self;
  pub const fn set_shortcut(self, shortcut: Shortcut) -> Self;
}

impl ContextMenu {
  pub fn new() -> Self;
  pub fn standard() -> Self;
  pub fn default_list() -> Container;
  pub fn set_list(self, list: Container) -> Self;
  /// Makes each entry's item with `builder`; `MenuItem::new(entry.label())` where unset.
  pub fn set_item_builder(self, builder: impl Fn(&MenuEntry) -> MenuItem + 'static) -> Self;
  pub const fn set_target(self, target: &NodeHandle) -> Self;
  pub fn add_entry(self, entry: MenuEntry) -> Self;
  pub fn set_entries(self, entries: impl IntoIterator<Item = MenuEntry>) -> Self;
  pub fn set_child(self, child: impl View) -> Self;
}
```

- `MenuEntry::new(label, run)` and `MenuEntry::clipboard(op)` go (VERIFIED (read):
  `gui/src/controls/context_menu.rs`; `ClipboardOp` already holds `Delete` and `SelectAll`); the
  item builder takes the entry so an item can show its shortcut.
- **An entry is data**: a label, a shortcut, whether it is offered, what it runs. That keeps
  `ROADMAP.md` item 3's candidate open, in which `NSMenu`, `UIMenu` and `PopupMenu` realize the
  entries where the platform composites views (a drawn Paste triggers iOS's paste prompt). Where
  a platform realizes the menu natively, the list and item builders are the drawn form's and do
  not apply, as a picker's parts do not apply to its `<select>`. JUDGMENT.
- **A standard entry's `handle_activate`** runs after the platform's own action, as an observed
  click runs beside a native toggle's flip, so an app hears a copy without redoing it. JUDGMENT;
  section 9, choice 15.
- **Content last**: the entries and the wrapped child are the menu's content, so the chain ends
  with them, the child last.
- A separator (`MenuEntry::separator()`, in `packages/`) is left for item 3's menus. The server
  renders nothing for a menu, the web opens it as a popover promoted in place, and the terminal
  serves its standard menu from the root where no wrapper does, as `AGENTS.md` says. JUDGMENT.

## 7. Toolkit names as doc aliases

`AGENTS.md` (since `a31b959`): a platform's own control takes Skene's names where the meaning
matches, "the toolkit's own names in its doc line and as `#[doc(alias)]`es of the type and its
members". Applied:

```rust
/// A switch: UIKit's `UISwitch`.
#[doc(alias = "UISwitch")]
pub struct Switch { /* .. */ }

impl Switch {
  /// Whether it is on: `isOn`, set through `setOn(_:animated:)`.
  #[doc(alias = "isOn")]
  #[doc(alias = "setOn")]
  pub fn set_on(self, on: bool) -> Self;

  /// Whether it is on, kept current: `isOn`, set through `setOn(_:animated:)`.
  #[doc(alias = "isOn")]
  #[doc(alias = "setOn")]
  pub fn derive_on(self, on: impl Fn() -> bool + 'static) -> Self;

  /// Runs `change` when the reader flips it: `.valueChanged`.
  #[doc(alias = "valueChanged")]
  pub fn handle_change(self, change: impl Fn(bool) + 'static) -> Self;
}
```

- **Every member that carries a toolkit member's meaning carries its alias**: the `set_`, the
  `derive_` and the `bind_` of one property alike, and the `handle_` of its event, so a search
  for `isOn` lists them all. A member with no toolkit counterpart carries none.
- **The bare name a developer types**, with the full selector at most beside it. VERIFIED (ran):
  rustdoc 1.99 accepts `UISwitch`, `android.widget.Switch`, `isOn` and `setOn(_:animated:)` as
  aliases with no warning and puts them in the search index (`aliasprobe/`, beside this file,
  `cargo +1.99.0 doc`). JUDGMENT: rustdoc matches an alias only when the search is the whole
  alias.
- **A prefixed setter** is a toolkit's member surfaced on a shared view, so it carries the
  member's name too (`set_ios_scrolls_to_top`, `#[doc(alias = "scrollsToTop")]`). JUDGMENT; the
  landed rule names a platform's own control, and this reads it as covering the platform's own
  properties as well (section 9, choice 14).
- **Documentation per build.** A platform control exists in its own platform's build alone, so
  its aliases are found in that platform's documentation (each build's `rustdocflags` name its
  platform); the Linux documentation has no `UISwitch` to find.
- **In this prototype** there is no platform control to alias: Linux has no toolkit, and
  `platform::linux` holds the clipboard alone (VERIFIED (read): `gui/src/platform/linux.rs`).
  `gui/src/platform/mod.rs`'s doc line ("the toolkit's own in their documentation") gains "and as
  doc aliases". On the web, the picker's scriptless form (`NativePicker`) carries `select` and
  `HTMLSelectElement`, its `handle_choose` `change`. JUDGMENT.
- **The gate** can hold it once platform controls exist: every public view type in a
  `platform::<p>` module carries at least one alias, a house rule beside the others; that a member
  carries its toolkit's name stays a review's to check, since no lint knows which members have
  counterparts. JUDGMENT.
- **The shared views** do not carry the toolkits' names under the rule as landed and asked for
  native controls. Carrying them (`Picker`: `select`, `UIPickerView`, `NSPopUpButton`,
  `Spinner`, `GtkDropDown`; `Toggle`: `UISwitch`, `NSSwitch`, `GtkSwitch`; `LazyColumn`:
  `RecyclerView`, `UICollectionView`, `UITableView`, `NSTableView`, `GtkListView`), on the type
  alone, would make a search from any toolkit land on Skene's view in every build's
  documentation. Offered, not taken: section 9, choice 14, the same choice as
  `lists-design.md`'s choice 1.

## 8. Landing order and tests

1. **The spine's rules**: part states (2), autofocus (3), hit testing (4). Mostly
   `foundation/src/seam/tree.rs`, `condition.rs`, `gui/src/control.rs` and
   `gui-linux/src/shell.rs`. Driver tests: hovering a checkbox's caption fills its face with
   `CONTROL_HOVER` (a pixel of the presented surface); hovering a picker's beside caption fills
   its face, the control's own node; pressing a caption shows `CONTROL_PRESSED`; hovering a radio
   group's caption leaves its radios' faces alone; a box outside every control keeps its own
   hover; a disabled group's radios read disabled and take no `Tab`; a window opens with nothing
   focused, and with its autofocus control focused where one is set (the reader's focused state,
   typed text arriving there); a popup's autofocus control takes focus and closing returns it to
   the opener; the overflow presses of 4.3; the hover frames at 1000 rows hold. Conformance: "a
   caption's hover is its control's", "autofocus", "overflow reaches the control".
2. **Typed choices** (1): the foundation's behaviors with their unit tests (a tag carried twice
   panics in a debug build; the members move in the source's order, a hand-placed group's in the
   scene's; a member is found by value), the controls with their chains reordered, the call
   sites and the conformance pages, an `Option<Fruit>` page among them, and a derived list that
   reorders while open, focus staying on its option. `Listbox<C>` lands here over the eager list
   it has, its windowing with `lists-design.md`'s engine.
3. **The scroll bar's parts** (5), with `Expr::<Color>::ink()` or the tokens (section 9), and the
   desktop's track-press setting.
4. **The view layer into the foundation** (6), with `Painted` in device pixels and
   `ContextMenu`: a one-device-pixel line covers one device column on the 1.5 legs; a scale
   change records again (sway's output scale changed at run time); `menu.rs`'s tests pass
   unchanged.

Every step: fmt, clippy `-D warnings`, rustdoc with no warning, the affected tests through the
runner on x11 and wayland (mutter), and on sway, weston and KWin where the shell changes (steps
1, 3 and 4), and the scaled legs for steps 3 and 4. JUDGMENT, from the working rules.

## 9. Choices for the author

Each has the default first; the design above takes it.

1. **`T: Hash`** (SwiftUI's `Hashable`), which keys derived options, finds built members and
   names the web's options, at the cost of float tags. Alternative: `T: Clone + Eq`, the web
   keyed by place, derived options keyed by a separate key closure.
2. **Nothing chosen in a picker or a radio group as the app's `Option<V>`** (SwiftUI's
   `Picker`). Alternative: `Signal<Option<T>>` on every choice.
3. **A list box typed by its choice**, `Listbox<C>` with `C` an `Option<T>` or a set (SwiftUI's
   `List(selection:)`), so a mismatched handler does not compile. Alternative: `Listbox<T>` with
   each choice member generic over its own shape.
4. **`Disabled` composing through nested controls** (`<fieldset disabled>`). Alternative: each
   control's own alone, as today.
5. **A box that acts keeps its own hover, press and focus inside a control**, lending nothing to
   its children; a control's own node takes its caption's hover and its behavior's press.
   Alternative: an acting box takes the control's like any part.
6. **The scroll box as its bar's control**, keeping today's hover-brightened thumb, losing a
   bar-only hover. Alternative: the bar a control of its own.
7. **`Expr::<Color>::ink()`** (CSS's `currentColor`) for the thumb's default. Alternative: two
   `SystemColors` tokens.
8. **A track press follows the platform**: on Linux the desktop's
   `gtk-primary-button-warps-slider` (a primary press jumps by GTK's default, as Firefox does on
   Linux), Windows paging, the terminal paging as Chromium does. Alternative: paging everywhere
   (Chromium), the original proposal.
9. **A closing popup returns focus to what held it at open** (a popover's hiding). Alternative:
   to its anchor, as today.
10. **An autofocused control shows its focus** until a pointer press (Chromium's heuristic).
    Alternative: hidden until a key.
11. **The whole view layer moves into the foundation** with `Painted` and `ContextMenu`.
    Alternative for the prototype: `Canvas`, `Paint`, `Picture`, `Renderer`, `Painting`,
    `MenuEntry` and the menu's wiring move now, the two views staying in `gui` until item 1
    ports the view layer.
12. **The web keeps the engine's scroll bar**, colored by `scrollbar-color`, and the compositor
    platforms keep their indicators, the drawn bar an upgrade later. Alternative: a drawn bar on
    the web now.
13. **Options are content**, ending the chain, the choice bound before them, the gate's lint
    knowing them. Alternative: options as properties, in any order.
14. **The aliases' scope**: a platform's own controls and their members, and prefixed setters, as
    the landed rule reads; the shared views carry none. Alternative: the shared views carry the
    toolkits' names on their type alone (`lists-design.md`'s choice 1, the same choice).
15. **A standard menu entry's handler runs after the platform's action.** Alternative: it
    replaces the action, the app doing the copy itself.
16. **The web names an option by its tag's hash** (`Key::of`, widened). Alternative: an
    app-supplied key per option, stable across toolchains, at the price of a second identity.
17. **The list box's styled member is `ListboxItem`**, as `PickerItem` and `MenuItem`, leaving
    `ListboxOption` to the behavior. Alternative: `ListboxOption` for the styled member too.

## 10. Found on the way, outside the prototype

- **`packages/reactive`'s `Key::of` is not the same on the server leaf and the browser leaf**
  for a value holding a `usize`, a derived enum or a slice. VERIFIED (ran), 1.6; an `isize` takes
  the same path (`write_isize` defaults to `write_usize`), JUDGMENT. Today's uses (owner paths
  for reconcile and `set_key`, read back by the same binary's snapshot) do not cross leaves, and
  the hydration channel matches resources by creation order and by tag (VERIFIED (read): the
  `grep` of `Key::of` in `packages/`, and `packages/web/src/resource_state.rs`), so nothing breaks
  yet; the web's option keys would be the first crossing. The fix: a `Hasher` around `Xxh3`
  writing integers little-endian and widening `usize` and `isize` to 64 bits, which keeps every
  value's x86-64 key unchanged (VERIFIED (ran)) and a big-endian host's equal to a
  little-endian one's (JUDGMENT). Both leaves come from one toolchain, since std's `Hash` is not
  promised stable across releases. A `ROADMAP.md` item for the author's session to place, since
  `packages/` is outside this stage's reach; the lazy lists' keys (`lists-design.md` section 4)
  take the same hasher.
