# Decisions

The alternatives set aside and the designs replaced, each with the date it was recorded
and the condition that reopens it, beside which new evidence that the alternative is
better reopens it too (`AGENTS.md`'s first rule). A reopened decision becomes an item in
`ROADMAP.md`.
The redesign recorded on 2026-10-15 is decided, and its first part has landed: the web
and the terminal stand on the reshaped foundation, one platform per build, as two
facades over realization crates of their own. The rest of it lands as `ROADMAP.md`
orders it, and until it does the code and `DESIGN.md` still describe the designs it
supersedes there.

## Decided against

- **A small taffy tree owned by each container (the prototype's `ContainerLayout`) with
  epochs** (2026-10-28), each container holding a tree of its own members and an epoch
  telling a stale measure from a fresh one: `skene-layout`'s `Tree` keeps one store
  keyed by the scene's nodes, each node's style, children, taffy cache and layouts in
  its slot, and each container's engine lays its members out in place over that store
  with taffy's own per-node cache, which does the epochs' work and lets a layout be
  held to one taffy tree's bit for bit. Revisit when a toolkit hosting the engine (item
  5) needs a container's tree apart from the store.
- **The prototype's single-child fast path and natural-height skip** (2026-10-28), a
  container of one child placed without taffy's grid and a container laid out at its
  natural height skipping its second pass: a path beside taffy's grid cannot lay out and
  measure each leaf as one taffy tree does, which the oracle holds `Tree` to, and taffy's
  cache already answers an input it has seen. Revisit when Callgrind shows a
  single-child box's grid or a repeated layout dominating a relayout.
- **A scene walk in `skene-layout` beside the terminal's (`Tree::lay_out`, `Part`,
  `Frames`)** (2026-10-27), a second walk bringing the layout tree in line with the
  scene from each node's part as its platform classified it, and indexing every node's
  rectangle, clip and paint order for hit testing: no platform called it, and the
  terminal's walks, held by its integration tests, become the runtime's. Revisit if a
  realization needs a layout of the scene without the runtime.
- **A scene walk revisiting the moved nodes alone (`Tree::lay_out_revisited`)**
  (2026-10-27), rewriting the revisited nodes and the leaves their text flows in on the
  tree the last walk left, and falling back to a whole walk where a node's part changed
  its shape: the per-container engine, one persistent taffy tree per decorated
  `Container` with its layout-only boxes folded in, and the shared `Restyle`'s revisit
  set do its work. Revisit when a structure change's relayout dominates a frame.
- **A text trimmed as `text-box-trim` trims it (`TextFlow::trimmed`)** (2026-10-27), a
  flow's first line cut to its highest cap height and its last to its baseline: beyond
  the guaranteed core a text reads as its platform's engine reads it, the web trims only
  where its engine has `text-box-trim` and never in a form element
  (`packages/gui-web/src/reset.css`), the prototype matched GTK, which does not trim,
  and no drawn platform called it. Revisit when item 3 decides a drawn pixel label trims
  its line box.
- **The spine's `Window`, an `Environment` per window and a scene per root waiting for a
  second window** (2026-10-27), one runtime per thread until item 3's popup surfaces or
  item 5's embedding gave a window or a root its second reader: the media API every new
  realization is written against would change under each of them. The spine's `Window`
  owning its `Environment`, `media::*` reading the window's and the scene's held state
  partitioned by root land with item 1, the web and the terminal having one window
  each. Revisit if the per-window lookup costs a shipped platform measurably while none
  has a second window.
- **Layout-only boxes as containers of their own in the drawn runtime** (2026-10-27),
  flattening only a compositor platform's view tree, a layout-only box realizing no
  platform view there while the drawn runtime keeps a taffy tree per layout box: one
  creation fact (`BoxFacts::decorated`) tells a decorated box from a layout-only one, and
  every platform but the web folds the layout-only boxes (`Row`, `Column`, `Grid`,
  `Stack`) into their nearest decorated `Container`'s tree by it. Revisit when the fold
  makes a relayout slower than per-box trees on the same case. Measured at the fold
  (2026-10-28) on the terminal's frame bench of 16,000 rows, Callgrind Ir per frame
  against per-box engines, the same leaf measures and paragraph layouts in each case:
  unchanged 525,292 to 524,954 (-0.064%), pointer_move 6,924,569 to 6,925,249
  (+0.010%), resized 237,779,549 to 237,781,175 (+0.0007%), keyed_row_toggled
  227,684,000 to 227,716,982 (+0.014%), mounted_first 1,919,324,389 to 1,919,453,321
  (+0.0067%). An engine is a cursor over the one store, so the fold moves no layout
  work: the layout pass costs the same, mount's record insertion about 7 Ir more a node
  as the function is built, and the rest is the allocator's heap (a keyed reconcile's
  vector copied as it grows in one frame of four where it grew in place).
- **Typed input for the drawn runtime alone** (2026-10-27), entry points on the runtime
  taking the foundation's event types, which the compositor platforms and the web would
  bypass by dispatching from their own views into the spine, or no input type beyond the
  events the handlers take: one typed input enum in the foundation beside the spine's
  events, which every platform produces before dispatch, the web's DOM listeners
  included, so dispatch has one input vocabulary and item 12's gesture arena one source.
  Revisit if translating the web's events into it delays a pointer move measurably.
- **A palette's root default defining another set's tokens** (2026-10-26), the default a
  reference or a read lays at the root carrying every definition of the palette, its
  aliases of the typography's tokens included: with Simple's typography aliases riding
  the default, the first preset on a page re-inked every plain text under no theme in
  Simple's foreground, a library inking the app's text. The default keeps the palette's
  own tokens, those its references link to it, and its definitions of another set's
  tokens apply beneath a theme of it alone. Revisit if a library must style text outside
  its own views without the app placing its theme.
- **A crate of the terminal's conformance pages** (2026-10-26), beside
  `skene-conformance-web`, so the terminal's pages would stand as the web's do: the
  pages are written in `skene-tui`'s vocabulary, so the crate would depend on
  `skene-tui` and be the dev-dependency of the tests in `skene-tui` that run them, a
  cycle in the crate graph that the realization's own crate, `skene-tui-terminal`,
  does not break, since a page names the facade's views and not the realization's. The
  terminal's pages stay a module beside its driver in `skene-tui`'s tests, shaped as
  the web's file. Revisit when the terminal's tests leave `skene-tui`, or when item 10
  gives each facade a crate of its pages.
- **The link policy's realization public on `LinkPolicy`** (2026-10-26),
  `LinkPolicy::realize(node)` made public once `Text` and `Span` call it from the
  facade's crate: a method an app reaches on a value of its own vocabulary, in the
  prelude, that writes into the realization. The seam is the leaf's own,
  `Web::set_link_policy`, which `Text` and `Span` call through `Active`. Revisit if a
  second realization implements link policies, which then takes a seam method.
- **The web's `gpu` feature declared on the facade** (2026-10-26), `skene-gui`'s
  `gpu = ["dep:vello_gpu", "dep:wgpu"]` over dependencies the facade's code never
  reaches: the GPU path is the realization's, so the feature and its dependencies live
  on `skene-gui-web` and the facade forwards to it (`gpu = ["skene-gui-web/gpu"]`), the
  gate's manifest rule admitting a member's feature held to the same rule. Revisit if
  an app depends on a realization crate directly.
- **The gate's web partition keyed on the facade** (2026-10-26), `skene-gui` as the
  crate whose reach marks a member as the web's: the realization beneath it reaches
  no facade and would be classed a host crate, built with no platform named. The
  partition keys on `skene-gui-web`, which every web member reaches, the facade
  included. Revisit if a web member reaches the web without its realization.
- **The realization's doctests written in the facade's vocabulary** (2026-10-26),
  `skene-gui-web` taking `skene-gui` as a dev-dependency so `NativeView`'s example could
  use the facade's `Container`: a cycle through the tests, building the realization a
  second time beneath the facade for its unit tests, for one example. The realization's
  example builds on the foundation's views at `Web`, and the facade's `platform::web`
  keeps the example in its own vocabulary. Revisit if the realization's own tests need
  the facade's views.
- **The surface traits re-declared without a parameter in each facade, now**
  (2026-10-26), the shape AGENTS states (`impl ContainerBase for Container {}`): 41
  traits (`ContainerBase`, `DecorationBase`, `MotionBase` and their kin), each a facade
  trait over the foundation's at `Active` that forwards every member, and the 120 impl
  lines of both facades that name them rewritten, to close a gap a library met in one
  bound alone, which `DecoratedTextEntry` closes. Revisit with ROADMAP item 8, which
  makes the two facades' surfaces parallel name for name.
- **A same-named subtrait without members for a library's bound** (2026-10-26), the
  facade's own `TextEntryBase` and `DecorationBase` over the foundation's at `Active`:
  a named import of either hides the foundation's trait of that name, so its members
  no longer resolve on a view (`entry.set_background(..)`) where the facade's was
  imported. Replaced by one bound trait of a name of its own, `DecoratedTextEntry`,
  which hides nothing. Revisit when the facade's surface traits forward every member
  (item 8).
- **A library's private function generic over the realization** (2026-10-26),
  `field_chrome<P: Realization, E: TextEntryBase<P> + DecorationBase<P>>`: it names no
  `Active`, but a crate on a facade is generic over no realization, its one build's
  realization being the facade's. Revisit if a library's facade crate must serve more
  than one realization in one build.
- **The field chrome duplicated per type** (2026-10-26), one chain for `TextField` and
  another for `TextEditor`: two copies of what one bound states. Revisit if the two
  entries' chrome diverges.
- **A preset inked only under a `Theme` of its palette or after a read of it**
  (2026-10-26), the interim after a library's view over `P: Realization` stopped
  defining its palette's default through `ThemedIn::define_root` as it built: a preset
  is a plain function returning a built-in, which may run before any platform is
  installed, so it cannot define at construction. Replaced by token references that
  carry their palette's default (`TokenRef::of`), which the shared half lays over the
  scene's root the first time a node's style, a derived slot's write or a definition
  names one, once per palette per root. Revisit if a palette's default must differ per
  subtree in a way one root definition cannot express.
- **A mount hook on every built-in a preset attaches to define its palette**
  (2026-10-26), a `handle_mount` the preset registers: it adds an app-facing member to
  every built-in and runs per node rather than once per palette. Revisit when built-ins
  gain a mount hook for a purpose of their own.
- **A registry of palettes a library populates** (2026-10-26), the library registering
  its palette with the framework so its default is defined at launch: it is registration
  code of the library's own, which a reference's link makes needless, and it defines
  palettes nothing references. Revisit if a palette's default must be defined before any
  node references it.
- **Defining a palette's default on a token miss at resolution time** (2026-10-26), in
  each realization as it resolves a token nothing defines: the web keeps every token for
  CSS to resolve and the server must emit the root rule with the render, so it would be
  written per realization, never once in the shared half. Revisit if every realization
  resolves its tokens in the shared half.
- **Skipping the root default under a theme that defines the palette** (2026-10-26): a
  node's style is applied before the node is attached, so its ancestors, and any theme
  above it, are unknown then, and the cost of defining it anyway is one root rule per
  palette, which the nearer theme outranks. Revisit when styles apply after attachment.
- **A palette's link producing the realization's own token value** (2026-10-26): a
  library core names no realization, so its consts cannot; the link yields the
  foundation's `TokenValue<N>`, which every realization's token value converts from by
  `TryFrom`. Revisit if a library core may name its realization.
- **A Simple option captioned by the app's `set_label`** (2026-10-26),
  `SimpleListboxOption::new(value)` styling a `ListboxOption` whose caption its user
  sets afterwards: a control's caption takes no run style from the control, so a
  caption set after the preset could not ink in the primary ink while chosen, and the
  preset takes its caption (`SimpleListboxOption::new(value, caption)`), built by its
  public `label`. Revisit when a control's caption inherits a run style the control
  sets.
- **Styled built-ins hosted once in the foundation with per-medium defaults**
  (2026-10-25), one `Button`, `Checkbox` and `Toggle` over `P: Realization` whose
  defaults each medium supplies as data: the trees the two media compose differ (a drawn
  box with a check and a pill with a disc against glyph runs and a track of cells, a
  border ring against reversed cells), so the code would only look alike, and each
  facade writes its built-ins in parallel files over the foundation's `Control::over`.
  Revisit when two facades' trees for a control coincide.
- **A collection's shortcut passed over as acting on no click** (2026-10-25), a
  `Behavior` hook saying whether a control acts on a click and the scene leaving such a
  control's shortcut unreported: a radio group's, a listbox's or a menu's shortcut moves
  focus to the member holding its stop (`KeyDefault::FocusShortcut`), as an access key
  on a control that cannot be activated focuses it and as `aria-keyshortcuts` allows,
  and one whose members hold no stop is passed over. Revisit when a collection must
  act on its shortcut beyond taking focus, such as choosing its next member.
- **A picker's and a listbox's options from `(value, caption)` pairs** (2026-10-25),
  as `SimplePicker::set_options` took them and the spike's `Picker::add_option` takes
  them: the styled `Picker<T>` and `Listbox<C>` take `ListboxOption` views (`set_options`,
  `set_children`), styled and wrapped like any control and at any depth beside other
  views, as a radio group takes its radios, and the picker's face reads the chosen
  option's name from its roster. Revisit when a list's options must be built from data
  the list alone holds, such as a list it filters.
- **A drawn chevron as the pixel picker's indicator** (2026-10-25), a `Painted` mark as
  the spike's `marks::chevron`: a canvas paints in colors fixed when it records and
  follows no token, so its ink would not follow `SystemColors` or the scheme, and the
  web paints it only once a renderer is chosen; the face's indicator is the glyph `▾`
  in both media, inked from `SystemColors`. Revisit when a painted mark can take its
  ink from a token, or a box can turn so a bordered corner draws the chevron.
- **A text entry's selection colors through a `Typography` theme** (2026-10-25), the
  field wrapping itself in a `Theme` redefining the typography selection tokens: a theme
  redefines every token of its set, overriding the typography an ancestor gave the
  field, so the field instead redefines the two selection tokens alone at its own node
  (`SelectionColors::define`), from `set_selection_color` and
  `set_selection_background`. Revisit when a theme can redefine a part of its set.
- **A radio group's options from an option builder** (2026-10-25), the spike's
  `RadioGroup::add_option(tag, caption)` building each radio from a tag and a caption:
  the group takes its radios as children (`add_child`, `set_children` and their kin)
  into its default column, each a `Radio` its user styles and wraps like any control and
  which may sit at any depth beside other views, as `BasicRadioGroup`'s do, and a
  `set_child` body replaces the column outright. Revisit when a group's radios must be
  built from data the group alone holds, such as a list it filters.
- **The accent itself as a prominent button's fill on the web** (2026-10-25): Chromium
  answers `AccentColor` with `#0075ff` and `AccentColorText` with white, 4.2:1, below
  WCAG AA's 4.5:1 for a label, so the web's prominent fill is the accent darkened by a
  tenth of Oklch lightness, which keeps a light accent's dark text legible too. Revisit
  when every engine's accent pair reaches 4.5:1, or `contrast-color()` can pick the
  label's ink on every engine.
- **A styled built-in holding its defaults in the control** (2026-10-25), a defaults
  layer inside `Control` that a built-in's own setters (`set_prominent`) rewrite as they
  are called: the built-in keeps what its defaults depend on beside its control and
  builds them at build, under every setter, through `Control::over`, so a setter's
  order never matters, and `HasControl` lets such a view carry every control's members.
  Revisit when a built-in's defaults must be known before it is built.
- **The web's system colors as a token value** (2026-10-25), a keyword kind of
  `WebTokenValue` (or a `Color` naming a system color) so the root's `SystemColors`
  would cross the seam as any root definition does: CSS's system colors are keywords no
  `Definition<Color>` holds, and a value of that kind would widen the public token value
  for the one root the realization defines, so the web writes the root rule as text of
  its own beside the reset and claims the set's root definitions instead. Revisit when
  another web definition needs a CSS color no `Color` expresses, or a drawing platform
  needs the web's root definitions in the scene.
- **The terminal's own colors marked in the token value** (2026-10-25), a root
  `SystemColors` token resolving to a marker the paint writes as the cell's default
  color: a resolved color carries no origin, so the writer instead writes as the
  default any cell color equal to the reported foreground or background, which shows
  the same in a terminal that draws its default in that color. Revisit if a color
  equal to the terminal's own must be written as RGB.
- **Styled built-ins under names other than their behaviors'** (2026-10-25), such as
  `Switch` for the toggle and `RadioButton` for the radio, so the shipped behaviors
  could stay at each facade's root beside the views: the drawn views take the
  platform-native frameworks' names (`Toggle`, `Slider`, `Radio`, `RadioGroup`,
  `Listbox`), as the Linux spike's facade did, so the behaviors move into each facade's
  `behavior` module and the root carries the views. Revisit if a facade's root must
  carry a behavior and a view of one name.
- **A `NativeView` built under `Enhance`** (2026-10-25), the server's placeholder its
  fallback and the adopted element its child, built once the page has hydrated: the
  native door takes the next hydration marker as any element does, so a hydrating
  browser adopts the served placeholder in place without reordering the hydration, the
  rest of the page hydrating with no mismatch, and the element is in hand from the
  build, with no rebuild after it. Revisit if adopting in place reorders the hydration
  for an element a library must make itself.
- **A `sleep` future on every facade** (2026-10-25), beside `spawn`, so an async task
  could wait on the platform's clock: timers are userland, and the framework ships only
  the seam no crate can provide, a one-shot action on the platform's clock, which each
  facade exports as `schedule` and `cancel` over the realization's own. A future that
  waits on it is a few lines of a library's. The exceptions stand where a platform owns
  the loop a timer runs in: the server leaf's executor keeps its `sleep`
  (`platform::web::sleep`), bounding a render, and a platform owning its event loop may
  ship a loop-integrated timer. Revisit with the loop-integrated timer of the event loop
  item (item 1).
- **Refusing a derived view in a crate reaching both facades** (2026-10-25), as a
  crate reaching neither is refused: the documents' example crates (`guide`,
  `readme`) depend on both facades, and the guide's libraries chapter derives `View`
  in a doctest built for one platform, so such a crate gets a build per facade, each
  under the `skene_platform` cfg naming its platform, and a crate on one facade gets
  its one build with no cfg. Revisit when the documents' examples are split into a crate per
  facade, or a crate on both facades derives a view by mistake.
- **A facade-concrete `ControlHandle`** (2026-10-25), each facade declaring a handle
  of its own over its `Active`, a behavior written against one facade's handle: a
  behavior is foundation code the facades share, so a handle per facade would fix the
  shipped behaviors to a realization again or write them twice, and a third party's
  behavior would name its facade. The handle is one type carrying `ControlOps`, the
  build's realization's operations as plain functions filled once by
  `ControlOps::of::<P>()`. Revisit if a behavior needs an operation a plain function
  cannot carry.
- **Behavior hooks generic over the realization** (2026-10-25), each hook a generic
  method, `fn on_click<R: Realization>(&self, control: &ControlHandle<R>)` and its kin:
  a generic method makes `Behavior` not dyn-compatible, so a control could not hold
  its behavior erased and build once whatever the behavior, and every hook would still
  name a realization. Revisit if a behavior needs an operation a plain function cannot
  carry.
- **A door's `ControlSink` handed over in `adopt`, and its `read` returning an owned
  value** (2026-10-25), the prototype's shape: `LowLevel::adopt` takes the view alone
  and `LowLevel::register_sink` the sink, since a view that holds no value has none to
  give and an `Option` of one has no type to infer, and `ControlSink::read` runs a
  closure with the value as `Realization::read` does, so a door's value crosses
  borrowed in both directions as a `ContentValue::Door` and is owned by its key alone.
  Revisit when a library's door must hand over its sink before the node exists, or a
  sink cannot lend what its view shows.
- **Naming a scriptless option by `Key::of(tag)`** (2026-10-25), a `<select>`'s
  `<option value>` and a radio's `value` the hash of the option's value as 32 hex
  digits, as the roadmap first had it: the server leaf and the browser must name an
  option alike, and the standard library hashes a slice of `usize` or `isize` as raw
  memory, bypassing the hasher's integer methods, so no hasher makes `Key::of` of such a
  value agree between the host and wasm32. An option is named by its key in its
  group's `Roster` instead, which both leaves build alike. Revisit when a hash of any
  `Hash` value stable across targets is taken, or `Choice` asks for `Snapshot`, whose
  encoding is.
- **Naming a scriptless option by its place among its group's options** (2026-10-25),
  the option's position in the roster's scene order, which both leaves build alike as
  well: a position renumbers the options after it as one is added, moved or removed, so
  every membership would be rewritten on each change, and a member's place is not
  known until the scene attaches it, after it joins. A member's key is its ordinal in
  its roster, the order it joined, which both leaves build alike too and which never
  changes while it stays. Revisit when the scene reports a member's place as it
  attaches it, or a platform needs its options keyed by place.
- **A picker's face given a `Memo<T>`** (2026-10-25), the face drawing the chosen value
  itself: a picker deriving nothing has chosen nothing, and a `Choice` has no value to
  stand for none, so the face is given a `Memo<Option<T>>`. Revisit when a picker
  derives its choice from its construction, as SwiftUI's takes its binding.

- **A secure editor** (2026-10-24), `set_secure` masking a multi-line entry as it masks
  a field: no platform's secure entry is more than one line, Chromium's password field
  and Apple's and Android's secure text fields among them, and the web's `<textarea>` has
  no password form, its masking (`-webkit-text-security`) nonstandard and unknown to
  form autofill, so the flag holds on a single-line field alone and an editor is never
  secure. Revisit when a platform's input protocol offers a secure entry of several
  lines, or CSS standardizes masking a text control's text.
- **A keyboard's `type="email"` and `type="number"` on the web** (2026-10-24), the
  field's `<input>` taking the type of the keyboard it asks for, as it takes `tel`,
  `url` and `search`: an email or number input has no selection API, its
  `selectionStart` throwing, and an edit's report reads it, so those keyboards are
  `inputmode="email"` and `inputmode="numeric"` (or `decimal`) over a text input, which
  brings the same keyboard without the type's validation. Revisit when the web's entry
  stops reading the selection from the element, as an `EditContext` client would, or
  the HTML standard gives those types the selection API.
- **A radio group's choice crossing as `Choices`** (2026-10-24), the group writing
  its members as the options of one `Choice` content beside the chosen key, as the
  `<select>` does: a group's options are its members, each a node of its own carrying
  its group and its key as its `Membership`, so the group's own content is the chosen
  key alone (`Chosen`), and a copy of the members' keys in the group's value would be
  a second path for one fact. Revisit when a platform realizes a group as one native
  control listing its options, as a `<select>` lists them.
- **The platform applying a refused content held whole** (2026-10-24), the verdict on
  a checked state, a range or a choice carrying the truth for the realization to write,
  as `Replace` carries an edit's: the repair of a value held whole is the write of the
  value, which the controller makes already on its write path, so it repairs through
  that path and answers `Verdict::Refuse`, and a platform reporting a toggle, a range
  or a choice applies nothing and decides nothing, a third party's native view included.
  Revisit when a toolkit's repair of a value held whole must be smaller than the write
  of it.
- **A caret move reported apart from its text** (2026-10-24), a `SelectionChange`
  report beside the edit, as the web's `selectionchange` and the terminal's moved
  selections were notified until then: a selection that meets a text it was not read
  with is the half-change the contents exist to prevent, and a second entry would split
  the controller's echo rule and its verdict in two. A moved caret is a change of the
  entry's `Edit` whose text is the one shown, asking only for the parts it changes.
  Revisit when a platform's protocol reports a caret move it cannot read the entry's
  text beside.
- **`Realization::read` handing back a borrowed `ContentValue`** (2026-10-24), the
  read returning the value as a write receives it: a realization keeps what a node
  shows behind its own borrow, from which no borrowed value outlives the call, so the
  read runs a closure with the value instead, and `ContentKey::read` owns it typed.
  Revisit when a realization keeps every content it reads as a value it can lend.
- **`ReadOnly` as a content beside its access report** (2026-10-24), a read-only
  entry's flag crossing the seam as a content of its own as its text does: it already
  crosses typed as an access report, which the web maps to the native `readonly` the
  form enforces without script and the scene reads to decide whether an entry takes
  edits (`Scene::is_editable`), so a content beside it would be a second path for one
  fact. Revisit when a platform's input protocol must hear read-only apart from its
  access layer.
- **The prototype's index-placing `insert` and in-place `set_hidden`** (2026-10-24),
  a realization told to place a child at an index among its parent's children and to
  hide a view where it stands: the scene's model takes a hidden child out and gives it
  back through `attach`, which an in-place flag cannot express, and neither realization
  reads an index, the web inserting before a reference node and the terminal reading its
  structure from the scene. `attach`, `relocate`, `hide` and `remove` name the nearest
  made ancestor and the next made sibling. Revisit when a realization's toolkit can
  place children by index alone or must hide a view in place.
- **Seam constants for scope and segment nodes** (2026-10-24), the prototype's
  `SCOPE_NODES` and `SEGMENT_NODES` declaring ahead of time whether a realization makes
  a node for a scope or a segment's anchors: a constant per kind splits one fact across
  the trait and the creation it governs, and holds nothing a realization cannot answer
  as it creates the node. `create` answers whether it made one, and the scene records
  it. Revisit when a realization must know before creating a node whether it will make
  one.
- **A per-node effect recompiling the whole record at `Keep::Nothing`** (2026-10-24), the
  prototype's shape: a render effect per node reading every signal its style reads and
  recompiling its record, resolved, whenever one changes, handing the realization a
  style with nothing left to resolve. A derived leaf already binds an effect of its own
  per slot, so a change to one would recompute every property of the node, and what a
  resolution reads beyond the signals (the node's interaction, the environment, the
  definitions in force) changes after creation and is tracked by the realization's
  retained runtime. The shared half hands every realization the record kept at
  `Keep::Everything`, and one keeping nothing resolves it at each restyle through
  `ResolutionContext::resolve`. Revisit when a realization keeps nothing and has no
  retained runtime resolving through the `Resolver`.
- **`apply_style` taking the style by reference** (2026-10-24), the realization borrowing
  the compiled record as it does a derived slot's leaf: a realization keeping nothing
  keeps the style for its restyles, so a borrow would copy every entry of every node
  once more. The style crosses by value, and a hook that reads it (`LayoutBox::realize`'s
  creation, `Run::add_realization`) borrows it before it is applied. Revisit when a
  realization must apply one style to several nodes.
- **A `.cargo/config.toml` per shell** (2026-10-24), `skene new` writing each shell's
  own configuration naming its platform, so plain cargo in a shell's directory builds
  that shell's platform: cargo reads its configuration from the directory it runs in, so
  cargo and rust-analyzer at the project's root would see no platform and fail on every
  crate reaching a facade. One configuration at the project's root names the first
  platform given, each shell is gated on its platform, and the CLI substitutes the cfg
  for the others. Revisit with a multi-root workspace, an editor opening each shell as a
  project of its own.
- **The facades' split and renames left to their name-for-name item** (2026-10-23),
  the web and the terminal ported onto the reshaped seam in place under the names they
  had, and split into `skene-gui` and `skene-tui` over realization crates of their own
  by the item that makes the two facades parallel: the doctrine's two facades over one
  foundation would have stood in no crate's name through every item between, each of
  them written against names it would then lose. The migration ported both in place,
  then split and renamed them, and the facades' item keeps only their vocabulary's
  parallelism and `EditContext`. Revisit if making the facades parallel name for name
  (item 8) needs an item of the split undone, a facade and its realization joined.
- **`skene-gui` born Linux-first beside a second seam** (2026-10-23), the pixel facade
  started on the Linux realization the spike prototypes, over the reshaped seam, while
  the web and the terminal kept the seam they stood on until a later port: two seams
  side by side, every mechanism written for both or for one, and the web, the pixel
  facade's most constrained platform, joining a vocabulary settled without it. The web
  and the terminal moved onto the reshaped seam first, and Linux joins `skene-gui` as
  its next realization (item 3). Revisit if Linux's realization (item 3) finds the seam
  unable to carry a window platform without a reshaping the web and the terminal would
  not survive.
- **A default platform at this repository's root** (2026-10-23), the root's
  `.cargo/config.toml` naming one platform so plain cargo and an editor work there
  unconfigured: plain cargo would build that platform alone while looking like the
  whole repository, its tests passing for platforms nothing ran, and the platform-free
  crates would build for it rather than natively. The root names none: plain cargo
  builds the crates reaching neither facade, a cargo alias per platform carries its cfgs
  and triple, and the checked-in rust-analyzer settings name one. Revisit if a
  contributor workflow cannot work through the aliases.
- **A `server` value of `skene_platform`** (2026-10-23), the server leaf named as a
  platform of its own (`--cfg skene_platform="server"`, a `Platform::Server` variant):
  the server leaf is the web platform's leaf, rendering the web's markup from the same
  views, so it is the `web` build with `--cfg skene_server` beside it, `Platform::CURRENT`
  answering `Web` on both leaves and the const `Platform::IS_SERVER` telling them apart.
  Revisit when the server leaf's vocabulary must diverge from the browser's in a way app
  code matches on.
- **A fixture realization for the spine's own tests** (2026-10-23), a `TestPlatform`
  logging the calls the scene mirrors, or a `#[cfg(test)]` realization realizing
  nothing, kept so the spine's structure, dispatch and reconcile tests run without a
  platform: it is a realization double by the letter of the 2026-10-15 entry, and a test
  asserting the calls a double received says nothing of what a platform does with them.
  The spine's algorithms are functions over plain data, unit-tested there, and what
  needs a live scene runs over the terminal's, asserted as the frame and the reader's
  tree show it. Revisit when a spine algorithm is one no plain-data function and no real
  platform can reach.
- **`Scene` storage generic over the medium rather than the realization** (2026-10-23),
  the scene keyed by a medium's scalar and token value so a test could hold one with no
  realization behind it: it redesigns the scene for testability alone, its mirrors and
  its `with` reaching the one realization a build has either way. Revisit when a spine
  algorithm is one no plain-data function and no real platform can reach.
- **`f32` cells end to end, rounded at layout and paint** (2026-10-23), the terminal
  keeping every length fractional through resolution and layout and rounding where a
  cell is drawn: a fixed length lands on a whole cell at the setter
  (`Scalar::from_f32`), and an expression's result and a motion sample land on one
  where they are evaluated, so the terminal's lengths are `i32` cells from the value
  onward. Layout still rounds what a fraction or a flexible share distributes and keeps
  the unrounded rectangles a childless box's eighth-cell painting reads, and a run's
  font size, a corner radius and a shadow keep their fractions of a cell. Revisit when
  a terminal draws whole boxes, their children with them, at sub-cell offsets.
- **Whole cells for every terminal length** (2026-10-23), the terminal's corner radius
  and its shadow's offset and softening rounded to whole cells at the setter as every
  other length is: the backdrop paints them in pixels where the terminal shows
  graphics, so a quarter-cell radius or a half-cell shadow draws as given there, and
  the cells take them in whole cells where they draw them, so they keep their
  fractions beside a run's font size. Revisit when the terminal paints its decoration
  in cells alone.
- **A negative literal dropped as CSS's parser drops it** (2026-10-23), a variant
  whose literal is a length below what its property takes falling back to the variant
  beneath or the base, as a browser that drops the declaration at parse time does:
  CSS answers the same value as initial when a token, a derived slot or the base
  carries it, so the drop would make one value lay out two ways by its leaf, and the
  web instead renders the literal as the drawing platforms lay it out (`initial`, or
  none for a padding's, a border's or a spacing's edge). Revisit when a browser's
  behavior for a negative variant is one an app is shown to need.
- **The prototype's record vocabulary** (2026-10-23), `spikes/linux`'s seam record: a
  `Prop` list of its own, its properties sorted into groups and its values into kinds
  of its own, compiled and resolved in its own `CompileCx`. The style record is the
  erased projection of the one property model (`BoxStyle`, `RunStyle` and their motion,
  keyed by `PropertyKey` in their visitors' order), and its compile at `Keep::Nothing`
  resolves each entry through the one `Resolver` as the typed style resolves it, so no
  second vocabulary or resolution can drift from the model. Revisit when a realization
  needs a property or a grouping the model's visitors cannot give.
- **A third `Keep` level, colors and states** (2026-10-23), the prototype's
  `ColorsAndStates`: color tokens kept as display-wide named colors tagged by their
  definition's content, state conditions kept, everything else resolved. `Keep` has the
  two levels a realization uses, `Everything` for the web and `Nothing` for every
  realization that draws, and no realization's toolkit keeps less than CSS and more
  than nothing. Revisit when one does (a style language short of CSS's); its tokens are
  then kept by name and keyed by their theme's scope, never tagged by content.
- **A builder opting a signal in, `Signal::new(v).saved()`** (2026-10-23), one
  constructor and a method that registers the signal after it is made: the opt-in is a
  constructor, `Signal::saved` and `Store::saved` beside `tagged`, so a signal's place
  is taken where it is created, its type is asked for `Snapshot` there and nowhere else,
  and no signal exists unsaved for a moment before it registers. Revisit when the
  constructors multiply past what one opt-in method would replace.
- **`tagged` as the only opt-in** (2026-10-23), every saved signal named: an untagged
  `Signal::saved` matches by its place among its owner's saved and tagged signals, so
  a view's local state opts in without a name unique in its owner, and `tagged` stays
  the anchor where an edit around it would move the place. Revisit when the positional
  opt-in misleads more than it helps, a saved signal restoring another's value after
  an edit often enough that apps tag every one.
- **`Unit::Utf8`** (2026-10-22), a third unit beside `Unit::CodePoint` and
  `Unit::Utf16` for the offsets a text crosses the seam in, since text-input-v3 and
  AccessKit count in bytes: a byte is the editing core's own index, and the core's
  input-client surface takes byte ranges directly (`select_bytes`, `replace_bytes`,
  `set_marked_bytes`, `rect_for_bytes`, `byte_at`), so a protocol counting in bytes
  reaches the core with no conversion and the seam names only the units that need one.
  Revisit when a seam protocol must cross in UTF-8 rather than through the core's byte
  surface.
- **`Verdict::Ignore`** (2026-10-22), the prototype's answer to a proposal the truth
  already holds, beside `Accept` and `Replace`: a truth holding the proposal accepts
  it, so the answer is `Accept`, the handler is not asked, and the client applies
  nothing it has not shown; a third answer meaning the same left every client a case
  to handle that changes nothing. Revisit when a client must tell a proposal the truth
  already held apart from one its handler accepted.
- **The terminal's text in logical order** (2026-10-22), each line's clusters written
  as the text stores them and left to a terminal that reorders bidirectional text
  itself: the editing core's carets, hits and selection rectangles are the engine's,
  in visual order, so a right-to-left entry drawn in logical order would place its
  caret where no cluster it names is drawn, and a terminal reordering nothing would
  show it backward. The terminal draws in the engine's visual order and switches BDSM
  to explicit at launch, restoring the mode it reported on exit. Revisit when the
  terminals in use reorder text written under explicit BDSM again, or a terminal
  protocol hands an app the visual order it draws.
- **A splice per changed run** (2026-10-22), repairing a toolkit that applied first by
  a diff of every separated run the truth changes, each caret mapped through the run
  it falls in, rather than by the one smallest edit from the common prefix to the
  common suffix: with an offset inside that edit keeping its distance into the
  replacement, one splice keeps the caret where the reader typed for every transform
  that changes the text in one place, and a diff would be a second algorithm the
  editing core and every realization's repair must agree on. Revisit when one
  transform must keep carets across several separated changes.
- **Keeping a repaired caret's distance in the realization's unit** (2026-10-22),
  mapping an offset inside the replaced range through `Edit::map_through` in bytes in
  the editing core and in UTF-16 code units on the web: a replacement whose characters
  are of other widths than those it replaces (an `a` made `À`, an `e` and a combining
  acute composed into `é`) would place the caret elsewhere than where the reader typed,
  even inside a character, and elsewhere on each platform, so the distance is counted
  in grapheme clusters, the unit a `TextSelection` counts. Revisit when a platform's
  native field keeps a repaired caret's distance in a unit of its own.
- **A generic medium** (2026-10-15), views generic over the medium they run on (`impl
  View<P>` with `P: Within<Pixel>`), `Adaptive` value chains and the platform sets
  (`Pixel`, `Cell`, `Apple`, `Mobile`, `Desktop`): checking a view tree grew
  superlinearly with its depth, a depth-5 tree in 7.8 s and a depth-6 one in 83 s where
  the concrete design takes 0.13 s and 0.26 s, and rust-analyzer needed four minutes and
  9.3 GB to answer a hover at depth 4 and was killed at 13.9 GB at depth 5, against
  1.8 s and 600 MB. App code is concrete: no type parameters, no generic medium.
  Revisit when rustc and rust-analyzer check a generic view tree in time and memory
  linear in its depth.
- **The `.on::<P>()` chain** (2026-10-15), a view's platform options as arms
  (`.on::<Ios>(|c| c.set_material(..))`, `.otherwise`) under coverage and ordering
  rules, a clone per arm's closure: it is the generic medium's road to a platform and
  pays its cost, and it asks a single-platform app for concepts it never needs. A
  platform's option is a prefixed setter (`set_ios_*`) and a difference in structure a
  `match` on `Platform::CURRENT`. Revisit with the generic medium.
- **Third-party platforms** (2026-10-15): one platform is chosen per build and
  `Platform::CURRENT` is a const enum, so a platform the framework does not realize has
  nowhere to stand, and a library reaches what a platform has through its native door
  instead. Revisit when someone asks to realize a platform outside the framework.
- **A deprecation warning on another platform's setter** (2026-10-15), `deprecated`
  raised under a cfg naming the platforms an app declares, so `set_android_ripple` in
  an app that never builds for Android warns: it borrows a lint for a meaning it does
  not have, its message still says "deprecated", and the facade needs the app's
  platform list as a second cfg. Another platform's setter compiles silently and does
  nothing. Revisit when a library can raise a diagnostic of its own conditionally.
- **A platform control under its toolkit's names** (2026-10-15): six toolkits'
  vocabularies came to 24 control types, 140 functions and 14 handler names for 4
  events. A platform control takes Skene's names where the meaning matches, the
  toolkit's names in its doc line and as doc aliases. Revisit when a toolkit's control means something
  Skene has no name for in most of its members.
- **Per-platform presets for a shared option** (2026-10-15): a shared option names its
  meaning once (`Button::set_prominent`) and each platform realizes it in its own idiom,
  where a preset per platform asks an app on several platforms to pick the idiom on
  each. Revisit when a meaning cannot be named once across the platforms that offer it.
- **Sharing a screen between the two media through `#[path]`** (2026-10-15), a guide
  paragraph teaching an app to include one source file in both its UIs: an app on both
  media writes two UIs, name-for-name parallel, and the framework neither ships a
  sharing feature nor teaches one. Revisit with one app serving both media.
- **A test double** (2026-10-15), a thin semantic recorder standing in for a platform
  off its target, as the investigation recommended at about 160 lines beyond the stub
  every host build needs: green on a
  double says nothing below the seam, and headless grew from bookkeeping into a browser
  emulator within five days. Each platform has one realization, the real one: `skene
  new` sets `[build] target` to the platform's triple beside a runner, so a plain
  `cargo test` runs on the platform, and CI cross-clippies every pixel target against
  the real bindings. A stand-in for an OS service (the settings portal) and the layout
  engine's unit tests over canned measurements double no realization and stay; a test's
  key press focuses its target by real input (`Tab` until the accessibility tree
  reports focus), never by a toolkit call. Revisit when a platform's tests cannot run
  on any machine its developers or CI reach.
- **Fully native text fields on iOS and macOS** (2026-10-15), `UITextField`,
  `UITextView`, `NSTextField` and `NSSecureTextField` for every field: Skene can draw
  the box around one, but the text inside loses word spacing, wavy and offset
  decorations, the caret's width and shape, per-run styling, Skene's editing behavior
  and its tests, and agreement with the Skene text beside it. Every field, the password
  field included, is Skene's own field view on TextKit 2 speaking `UITextInput` or
  `NSTextInputClient`, as on every other platform, at the possible cost of the
  credential extras tied to Apple's controls (the strong-password and save offers,
  macOS's inline AutoFill), with app-initiated saving as the fallback. Revisit when a
  device test finds those offers lost to Skene's field on iOS and app-initiated saving
  cannot stand in for them; the alternative is then every iOS field native, never the
  mix below.
- **Native password fields beside drawn regular fields** (2026-10-15), on iOS and
  macOS, where capture blanking, strong passwords and the save prompt are tied to the
  platform's control: two fields that differ in style and behavior on one form is the
  worst outcome of all. A password field is Skene's field, masked, under the platform's
  secure-entry flag. Not revisited: a platform whose password field goes native takes
  every field native with it.
- **A hidden native field as the input proxy** (2026-10-15), as Flutter and Compose
  Multiplatform feed their drawn fields: the view a reader focuses and the view the
  input method writes into would be two, held together by an identity trick
  (`isKindOfClass:`) no signal can prove. Skene's own field view speaks the platform's
  text-input protocol (`UITextInput` with `UITextInteraction`, `InputConnection`,
  `NSTextInputClient` with `NSTextCheckingClient`) and takes the system's selection
  chrome where the platform offers it. Revisit when a platform's protocol cannot be
  spoken by a view of Skene's own.
- **Replacing the web's form elements with drawn text input** (2026-10-15), through
  `EditContext` or rendering of Skene's own: `<input>`, `<textarea>` and the other form
  elements styled by CSS are Skene's drawn controls on the web, nearly unlimited in
  style and the only text input that works without script. Anything more elaborate is
  progressive enhancement over them, never a replacement. Not revisited while
  progressive enhancement is doctrine.
- **Landing AccessKit's missing text support upstream first** (2026-10-15): a drawn
  field regressed against `GtkEntry` for want of `EditableText` and several `Text`
  methods, its writes reading back stale for about 300 ms. AccessKit serves every
  platform but the web, and Skene carries its changes itself, as a fork or patches,
  contributing them upstream once they are proven in practice. Revisit when upstream
  AccessKit carries them.
- **Pango on Linux** (2026-10-15): Pango is GNOME's text engine, pixel-identical to
  `GtkLabel`, not the operating system's; Linux's own layers are fontconfig, FreeType
  and HarfBuzz under the desktop's settings. Parley lays Linux's text out over
  fontconfig's per-character fallback (color emoji included), FreeType rasterization
  under the user's antialias and hinting settings, and complex-script line breaking.
  Revisit when matching GNOME's applications glyph for glyph becomes a goal.
- **ClearType text on Windows** (2026-10-15): DirectWrite lays Windows's text out and
  its glyphs enter Skene's renderer in grayscale, vello outlines the floor and a
  DirectWrite-rasterized atlas the upgrade; subpixel coverage holds only over a
  background opaque when the glyph is drawn, and its gain is unmeasured. Revisit when
  opaque text measured on Windows shows a gain worth a second glyph path.
- **Drawing Linux inside a GTK window** (2026-10-15), Skene's drawn controls in a GTK
  shell that supplies IME, the AT-SPI relations and fractional scale: Linux only
  composites windows, as Windows does, so Skene owns the whole window there and speaks
  the protocols itself (Wayland, X11, IBus and Fcitx, AT-SPI, the portals), one family
  with Windows. GTK's third-party widgets are not needed, and on Ubuntu 24.04's GTK 4.14
  a custom widget gets no `EditableText`, no text extents and no assistive caret or
  selection writes. Revisit when an app needs to host GTK's own or third-party widgets.
- **A realization crate on its bindings' default features** (2026-10-15):
  `objc2-ui-kit` checks clean in 2.93 s with the features a realization uses and in
  41 s with its defaults. A realization crate enables only the binding features it uses
  and keeps target-side C build scripts out of its graph. Revisit when a binding's
  defaults cost nothing beside its minimal features.
- **Theming the drawing platforms through reactive values** (2026-10-15), every platform
  but the web reading a palette as signals throughout its tree in place of token sets:
  tokens stay as they are on every platform, a named inherited slot a theme defines and
  the foundation resolves where a realization keeps less than everything, the web
  keeping its custom properties so a scriptless page themes through CSS. Revisit when
  tokens are revisited, with whether a token must define a variant for each scheme.
- **A toolkit's own containers laying a platform out** (2026-10-14), GTK's boxes and
  grids for Skene's layout views: they express no grow weights, shrink, flex basis,
  fractions, maximum or exact sizes, wrap lines or grid tracks, and matched the
  reference in 26 of 45 scenarios where taffy matched 43. Taffy owns layout on every
  toolkit but the web's, one small tree per container, the toolkit measuring its
  leaves, its minimum a floor and the rectangles fractional. Revisit when a toolkit's
  containers express Skene's layout vocabulary.
- **A clickable `Container` as a button** (2026-10-14), a toolkit button that may hold
  no control: the rule was a runtime assert, so an app whose derived child later became
  a control would panic, and the pointer-only box offered instead took no focus and no
  key. A clickable `Container` is an activatable row, focusable, activated by `Enter`
  and `Space`, holding controls that own their presses; focus follows the scene's child
  order, never geometry, which had put a switch before the command to its left. Revisit
  when a platform cannot make a box holding controls focusable and activatable.
- **A role of its own for the activatable row on the web** (2026-10-25), `row` or
  `button` on the clickable box: ARIA's `row` belongs inside a grid or a table, which
  a free-standing box is not, and a `button` may hold no interactive content, which a
  row holding controls does. The row carries the role its app gives it through
  `set_role`, and none otherwise. Revisit when ARIA gains a role for an activatable
  container that may hold controls.
- **Positional focus order** (2026-10-25), `FocusOrder::Position`, a control reached at
  a position of its own before everything in document order, the web's positive
  `tabindex`: focus follows the scene's child order on every platform, so the order a
  reader tabs through is the order a reader reads, and an app that wants another order
  builds its views in it. `FocusOrder` keeps `Auto` and `Skip`. Revisit when a reader or
  a platform needs an order the child order cannot express.
- **A drawn entry's editing state opened by its behavior** (2026-10-25), the
  `TextEntry` behavior asking the realization for the entry through `ControlOps` as it
  attaches: the scene already knows, from the node's spec, that it realizes a text
  entry and how many lines it shows, and it is what must drop the state with the node,
  so it asks the realization (`Realization::entry`) as it realizes the node, before
  `create`, and a behavior's table gains no operation only one kind of control needs.
  Revisit when a control other than a text entry needs editing state the scene keeps.
- **The text engine as the seam's item** (2026-10-25), `Realization::Engine:
  TextEngine` with a constructor handing back an `Editor<Masked<Engine>>`: the seam is
  the spine's, which names nothing of the foundation beyond itself, and the engine's
  contract and the editor are the text module's, so the seam's item is the entry the
  scene keeps (`Realization::Entry`, the terminal's `Entry<CellEngine>`), naming the
  engine through it, and the web's an uninhabited type rather than an uninhabited
  engine with an uninhabited paragraph and layout. Revisit when code generic over the
  realization needs its engine to lay text out, which the drawn platforms' shared
  runtime (item 1) may.
- **Content-addressed token tags on display-wide colors** (2026-10-14), a toolkit's
  named colors tagged by their definition's content: a palette switch changed every tag
  and so restyled every node naming one. A palette switch touches no node: named
  colors are keyed by the theme's scope, and the web keeps class names of content
  alone, since `var()` scopes its tokens. Revisit when a platform's named colors are
  scoped as custom properties are.
- **Reading a settings portal's "no preference" as light** (2026-10-14): it overwrote
  a reader's own dark preference wherever the portal's backend always answers so. No
  preference falls back to the desktop's toolkit settings (`GtkSettings`). Revisit when
  every desktop's portal answers the scheme it shows.
- **A `skene` command passing over what it cannot do** (2026-10-14): `skene check`
  skipped a declared platform nothing realized and exited 0 having checked nothing, and
  `skene new` named a runner it had not found, so the first plain `cargo test` failed.
  A check fails on a declared platform it cannot realize, and `skene new` refuses to
  name a runner that does not exist. Not revisited.
- **Laying text out by hand** (2026-10-08), a paragraph of our own measuring each
  cluster by its platform's metrics: each platform's own engine lays its text out, and
  Parley lays it out on Linux and in the terminal, the terminal through a typeface the
  build generates whose glyphs are its cells. Revisit when neither a platform's engine
  nor Parley can carry its text, or the terminal needs a behavior no typeface expresses.
- **A newline ending a text laid out as a browser's block lays it out** (2026-10-08),
  with no line, where Parley lays text out: Parley opens a line after a final newline,
  as an editor does, and an app leaves out what it does not want laid out; an empty
  text is one line in the core every platform guarantees. The web keeps the browser's
  reading of a final newline, and every other engine its own. Revisit when the
  platforms Parley serves expect a browser's reading.
- **Collapsing a text's blanks and newlines** (2026-10-08), as a browser does by
  default: text keeps them, as SwiftUI's, Flutter's and Compose's text does, and a word
  too wide for a line of its own breaks where the line fills, so a newline in a run
  breaks its line on every platform. Revisit when a platform's own text view collapses
  them.
- **Signals shared across threads** (2026-09-29), as a registry each runtime drains
  through an inbox: a handle stays on its thread, and another thread's work reaches it
  through a task that thread's executor wakes. Revisit when the hop through the owning
  thread's executor dominates a real app's cross-thread update.
- **Hot-patching** (2026-09-16): state-preserving reloads come from the snapshot
  instead. Revisit when a reload through the snapshot is too slow to keep a developer's
  flow and a hot-patching toolchain serves both wasm and native.
- **One vocabulary across every platform** (2026-10-01), the terminal measuring in
  logical pixels: pixels unify the numbers but not the medium, since a border still
  costs a whole cell, small spacings round to nothing or to a whole row and a type
  hierarchy collapses into one row outside kitty, so writing once yields a correct
  structure and a poor look while every property owes a floor on every platform. Each
  medium keeps its own unit and its own vocabulary, `skene-gui`'s pixels and
  `skene-tui`'s cells, the two name-for-name parallel. Revisit when the terminals worth
  targeting draw borders, spacings and sizes below the cell as a matter of course.
- **Grouping platforms by family** (2026-10-01; the prefixes 2026-10-15), a shell for
  the web and the desktops and one for both mobile OSes, or `mobile_` and `desktop_`
  setter prefixes beside `apple_`: iOS and Android differ in representation, app
  structure and navigation, and each desktop OS in its layout and control conventions,
  so a family's option means something different on each member, and an iOS-only app
  would write `set_mobile_haptic`. `platform::apple` (`Platform::IS_APPLE`,
  `set_apple_*`) is the only grouping. Revisit when two platforms of one family
  converge on their representation, structure and conventions.
- **A capabilities model of typed slots on `Platform`** (2026-09-17) in place of
  progressive enhancement: shelved, since it puts bounds on every shared signature; only
  a low-level view names what a platform can do. Revisit when an ordinary view, not a
  low-level one, needs to name what a platform can do.
- **A hybrid native realization** (2026-09-28), painted content with native controls
  embedded in it: every embedded view is paid for, as Flutter's platform views show.
  It is also what the native door would be on a window platform, a toolkit's control
  inside the window Skene draws: Linux's door adopts no toolkit's view, having no
  toolkit, and whether Windows's hosts a Win32 control is open in `ROADMAP.md`. Revisit
  when an embedded native view costs a frame no more than one in the platform's own
  hierarchy.
- **A painted compositor platform** (2026-09-28; narrowed to these 2026-10-15),
  Android through Linebender's android-view, or iOS or macOS painted into one surface:
  their views carry real functionality (text selection and input, their own widgets,
  third-party native views), so Skene draws its views as the platform's views and
  interleaves them with native ones, where one surface meets those platforms only as
  the hybrid above. Revisit when a painted surface interleaves with the platform's own
  views at no cost in function or frame time.
- **A WASI server leaf** (2026-09-17), under a wasmtime runner or as a `wasi:http`
  handler: it needs a second transport for every client, and the demand for wasm
  rendering is on JavaScript isolates. The server leaf compiles natively, selected by
  `--cfg skene_platform="web"` with `--cfg skene_server`, so every crate on the host
  reaches a render; a wasm host is a runner someone may write, for pages that await
  nothing until the settle loop is async. Revisit when an app must render where only
  WASI runs, and WASI's clients cover what its pages fetch.
- **Islands architecture** (2026-09-16): compiled no-JS styling already removes most
  reasons a static page ships wasm. Revisit when static pages measured in practice ship
  wasm for a small interactive part of a large page.
- **LiveView-style server rendering** (2026-09-16): adjacent to the server-side
  non-goal. Not revisited while Skene is a client-side framework.
- **A webview desktop shell** (2026-09-16): a desktop gets a native shell of its own.
  Revisit when the native desktop shells are given up.
- **A lazy split of the WebGPU renderer** (2026-09-16), measured again 2026-10-02 on
  release builds after `wasm-bindgen`: the GPU path is the `gpu` feature an app opts
  into, so an app leaving it off carries none of it and no shader text, whether it
  paints nothing (`counter`, 1.27 MB) or paints on the CPU (`enhance`, 1.75 MB, 246 KB
  of it the CPU renderer); an app enabling it (`gallery`, 2.92 MB) carries 116 KB for
  the GPU path beside the 254 KB CPU renderer it needs either way. Nothing worth a
  split. Revisit when a painting app's GPU path outweighs its CPU renderer.
- **Rendering the whole terminal frame as one image** (2026-09-16): the text would then
  be ours to rasterize, and the terminal's font, selection, copy and accessibility would
  go with it, a poor GUI in place of a good terminal. Revisit when a terminal lets an
  image carry selectable, copyable and accessible text.
- **Any terminal enhancement gated by a terminal's name** (2026-09-16): a name
  (XTVERSION, `TERM`) is not a capability; a parsed reply proves one. Not revisited.
- **Inferring a terminal's depth** (2026-09-16): a terminal answering no DECRQSS paints
  at 16 colors, Terminal.app among them; the cost of inferring nothing, taken knowingly.
  Client hints are likewise Chromium-only (`Accept-CH`/`Critical-CH`), and other engines
  get the documented defaults. Revisit when a terminal that answers no DECRQSS shows
  truecolor to many of an app's users, or another engine sends client hints.
- **A vertical fractional edge in the terminal** (2026-09-16): only the lower block
  glyphs exist, so a top-anchored fill would need upper ones; a vertical fill ends on a
  whole cell. Revisit when the fonts terminals ship carry the upper eighth blocks of
  Symbols for Legacy Computing.
- **Moving an editor's caret off the cell past a full row** (2026-09-28), where a
  wrapping space was consumed: a browser's caret hangs past such a row into the
  textarea's padding, and the terminal's bar cursor in the cell past it, the padding or
  the border, shows the same position. Revisit when browsers stop hanging the caret past
  such a row.
- **Replacing `IntoChildren` and `IntoChildOf` with std traits** (2026-09-16), checked
  against the compiler: a blanket `impl<I: IntoIterator<Item = V>> IntoChildren for I`
  conflicts with the tuple impls, and the same conflict appears with `From`; tuples are
  not optional because children are heterogeneous. Revisit when coherence admits such a
  blanket impl beside the tuple impls on stable Rust.
- **A collision check on snapshot keys at every build** (2026-09-16): a key is matched
  by a 127-bit hash of its value, so two values hashing alike share one scope and their
  state crosses on a restore, a risk smaller than the check's cost. Revisit when two
  keys are seen to collide; the route is then a keyed step carrying the value's postcard
  bytes and a type hash, at a `Serialize + DeserializeOwned` bound on every key and a
  heap step per keyed scope.
- **A `&'static str` signal restoring** (2026-09-16): it never restores, since it cannot
  be deserialized owned. Revisit when apps keep state a reload should carry in `&'static
  str` signals.
- **A `HashMap` store field iterating in a stable order** (2026-09-16): its entries come
  in its iteration order; an ordered list wants a `BTreeMap` or a keyed `Vec`. Revisit
  when an app needs an order neither a `BTreeMap` nor a keyed `Vec` gives.
- **Another reactive engine** (2026-09-28): Leptos's `reactive_graph` schedules effects
  asynchronously behind `Arc` and `RwLock`, breaking the synchronous render-first flush
  the terminal's key defaults rely on; `sycamore-reactive` has eager memos and no render
  flavor; the Rust alien-signals port has no owners, contexts or cleanups. Its
  techniques are ported instead. Revisit when an engine offers a synchronous
  render-first flush with owners, contexts and cleanups.
- **Another terminal parser, or one beneath it** (2026-09-28): crossterm parses no
  replies, termina lacks APC and re-parses byte by byte, vte has no APC dispatch, and
  termwiz is heavy. Revisit when a parser handles the replies and APC strings this one
  does at no more weight.
- **The `color` crate for `Color`** (2026-09-28): byte-identical class names across the
  server and the browser need `libm` math, which its `libm` feature cannot promise once
  peniko and vello unify it with `std`; its CSS semantics are taken instead. Revisit
  when the crate's math is `libm` under every feature its dependents unify.
- **A view macro (`rsx!`) beside the builders** (2026-09-28): a macro costs rustfmt and
  rust-analyzer, and Freya left rsx for builders in 2026; the effort goes into
  diagnostics. Revisit when rustfmt and rust-analyzer format and complete inside a macro
  as they do outside it.
- **Collapsing the crates beyond `skene-foundation`** (2026-09-28): separate crates
  compile in parallel. Revisit when a measurement shows the crate boundaries costing
  more build time than their parallelism saves.
- **One app serving both media** (2026-09-28): the two galleries still differ on 431 of
  893 lines once numbers are stripped. Revisit when the galleries converge on most of
  their lines once numbers are stripped.
- **A stable plugin ABI** (2026-09-28): nobody asks for one. Revisit when someone asks
  for one.
- **A property table giving each property an interpolation class and a unit class**
  (2026-09-29): a value's `Interpolate` answers per pair, as CSS computes it per pair (a
  length and `Auto` are discrete, two lengths are not), and the value's variant is its
  unit. Revisit when a pair of values needs an interpolation its types cannot answer.

## Superseded

- **One `TaffyTree` for the whole scene (the terminal's and `skene-layout`'s `Tree`, to
  2026-10)** (2026-10-28), every node of the scene in one taffy tree keyed by its node,
  laid out and rounded by taffy's own `compute_layout_with_measure`: one label changed
  among 1,000 rows of three cost 34 measures on the changed row's 3 leaves, 12 flexbox
  runs and 1 grid run along its ancestors, 47 node layouts in all. `Tree` now lays each
  container out by a taffy engine of its own over one store of the scene's nodes, held
  to that one tree's layouts, marks and measures by a proptest. Revisit when a relayout
  under the per-container engine misses its frame budget where the whole-scene tree,
  measured on the same change, meets it.
- **A component library's views generic over the realization** (2026-10-26): Simple's
  core wrote its views over `P: Realization`, stating what it needed from a medium as a
  bound on the platform's associated types, their members in a `Simple*Base` trait each
  platform's crate implemented for a newtype of its own, so the core was the last code
  above the realizations to name one. A library's core now holds its colors, named
  sizes and logic, and each facade's crate writes presets returning the facade's
  built-ins styled from them and, where a preset is not enough, concrete views of its
  own. Not revisited, unless a library view's semantics are shared by both facades and
  can be hosted once in the foundation.
- **The conformance suite's pages generic over the realization** (2026-10-25):
  `skene-conformance` built every page itself as a `Page<P>` over the foundation's
  views, and each platform's driver built them for its own realization, so the one UI
  the framework shared between the media was its own test pages. The suite now holds
  the scenarios, the `Driver`, the `Probes` and the names of its pages (`PAGES`) alone,
  and each facade writes the pages in its own vocabulary, held to those names. Not
  revisited: a facade shares no UI with the other.
- **The server leaf's unreachable door** (2026-10-25), its native view an uninhabited
  `Unadoptable`, so a library's native view existed on the browser leaf alone and the
  server rendered nothing for it: the server leaf's native view is the `NativeMarkup` a
  `NativeView` is built from, rendered as the placeholder a hydrating browser adopts in
  place. Revisit if a server-rendered placeholder misleads a reader before the bundle
  arrives.
- **Raw CSS as a public declaration set** (2026-10-25), `platform::web::Declarations`
  taken by the `WebMarkup` escape trait: a native view's raw CSS is its own `style`
  attribute, or the element its adopt handler holds, and the declaration set is the
  compiler's own again. Revisit if a native view needs CSS its markup cannot carry.
- **Per-platform view realization** (2026-07-31) fell to the orphan rule the moment
  platforms became separate crates, and to the duplication it forced. The typed
  vocabulary replaces it. Not revisited while platforms are separate crates.
- **"No framework style model"** (2026-07-31) existed so no platform would be forced
  onto a foreign vocabulary. Taffy-everywhere plus no-JS responsive SSG *is* a shared
  vocabulary by requirement; the model is framework-defined, and a medium extends it
  through its typed extension. Revisit when a medium needs a property its typed
  extension cannot carry.
- **"Selection is the target, only"** (2026-07-31) cannot distinguish desktop from the
  terminal on one triple. Target-gating is demoted from selector to enforcement. Revisit
  when every platform has a target triple of its own.
- **A `Backend` trait object** (2026-07-31) cost `unsafe`, `Box::leak`, and a fallible
  downcast. Still rejected: platforms are statically selected, one per build. Revisit
  when a platform must be chosen at run time.
- **"The built-in views live in the umbrella"** (2026-09-03). True with one facade; with
  two it produced two umbrellas and two copies, sixty-five percent of them
  character-identical. The generic parameter already used for `Children` is applied to
  every view in `skene-foundation` (that generic parameter is itself superseded,
  2026-10-15: the views are written once over the seam). Revisit when a single facade
  remains.
- **"No persistent scene tree in core"** (2026-09-03). The objection stood for a tree
  that calls platforms. The scene calls none; every platform kept the structure anyway,
  four ways. Revisit when a platform keeps no structure of its own.
- **"The vocabulary is free functions resolved by name"** (2026-09-03). A missing
  function was only an error on a target-gated path, which let the host platform lack a
  third of it. A zero-sized trait makes the contract complete. Not revisited.
- **A recording platform (`skene-record`) beside the real ones** (2026-09-17). It ran
  only the target's handlers, did not unlink on reparent, and could not be complete
  because nothing forced it to be. The headless platform implements the whole trait and
  is selected by the web facade on the host, so it cannot drift; headless is itself
  superseded (2026-10-15). Revisit with the test double.
- **Group nodes for reactive subtrees** (2026-09-05). A real `<div>` and a real taffy
  node made every derived subtree one flex item. Subtrees mount between anchors. Revisit
  when a platform needs a node per reactive subtree.
- **A style blob per control (`BoxStyle` handed to `set_style`), the `set_<prop>_when`
  families and a whole-style `set_when`** (2026-09-05). A whole-style variant could set
  properties the view does not honor; a per-property setter exists only where the
  property does, and a condition rides the value. Revisit when styles arrive as data an
  app loads at run time, which no type checks.
- **`Adaptive<T>`: a token's conditions landing beside the property as ordinary
  variants** (2026-08-17). They were copied into every use site, doubled by the scheme
  guard, and a palette could change only by a rebuild that lost state. A token is a
  named inherited slot; its conditions live in its definition; a theme redefines it by
  one effect. Revisit when a token's conditions must differ per use site.
- **`SimpleTheme::adaptive(light, dark)`, a light/dark palette pair, and a whole-palette
  `when(condition, overlay)`** (2026-08-17). Each made light and dark the one condition
  with machinery of its own; the per-property form removes the separation, and the
  overlay is the same privileging one level down. `Media::Dark` in a definition is the
  whole mechanism. Revisit when a condition must switch a palette whole rather than
  token by token.
- **`theme_<prop>` setters taking `Fn(&Palette) -> Color`** (2026-08-17). They welded
  the framework's box view to one library's palette type. A token is a value the
  ordinary setters accept. Revisit when a themed value cannot be named by a token.
- **`TextTheme`, `ControlTheme`, `SimpleTheme`** (2026-09-03). Three inheritance
  mechanisms for one need; `Typography`, `SimpleGlyphs` and `SimplePalette` are token
  sets under the one `Theme`. Revisit when an inherited value cannot be a token.
- **A `derived`-based `derive_scheme`** (2026-09-05). A rebuild disposes the subtree's
  scope with its state—measured: a signal inside went back to its initial value. The
  platform is told the scheme and one write re-inks the page. Revisit when a rebuild
  keeps the state of the subtree it rebuilds.
- **A scheme override scoped to a subtree** (2026-09-05). The choice half scopes for
  free; the guard half (the preference rule applying only where no ancestor chose) has
  no scriptless answer Firefox supports. An app has one scheme, the way the platform
  does. Revisit when every engine the web targets answers the guard half without script.
- **A handler-side override (`Override`/`ControlFlow`) letting a user take an event from
  a control** (2026-09-05). No first-party code wanted it and it taxed every ordinary
  handler, because the orphan rule blocks `From<()>` for a foreign type. A control
  decides its verdict. Revisit when first-party code needs to take an event from a
  control.
- **`Enhanced`: a native `Web*` control rendered without script and swapped for a drawn
  `Basic*` after hydration** (2026-08-16). The scriptless form is the control: a text
  entry is its native element and a toggle or slider keeps its input inside the label. A
  view that genuinely has two forms declares them through `Enhance`, the picker
  included. Revisit when a control has two forms `Enhance` cannot declare.
- **A derived value as an inline override** (2026-09-05). Inline outranks every variant
  on the web and not in the terminal. A derived leaf is a per-node custom-property slot
  the class references. Revisit when a derived value should outrank every variant on
  every platform.
- **The `snapshot`, `test` and `simple-views` cargo features** (2026-08-12). Each gated
  a trait bound, a type or a platform, which an additive feature cannot do soundly.
  Restoring state is a bound a signal opts into, testing is a module of each facade,
  and the Simple views are a crate. Revisit when a cargo feature can gate a bound
  soundly.
- **Transitional `next` namespaces** (2026-09-05): `skene_core::next`, `skene_web::next`
  and `skene_simple::next` carried the new model beside the old during the redesign and
  are flattened into their crate roots. Not revisited: they were scaffolding for one
  redesign.
- **A facade per platform** (2026-10-15), `skene-web` and `skene-terminal`, later
  `skene-ios`, `skene-android`, `skene-windows`, `skene-macos` and `skene-linux`, each
  with a vocabulary typed to exactly its platform: an app on several pixel platforms
  would write a UI per platform where, measured, 65 of its 227 lines differ by
  platform. Two facades sit over the foundation, one per medium: `skene-gui` for every
  pixel platform and `skene-tui` for the terminal, name-for-name parallel, a
  platform's options its prefixed setters (`set_linux_*`, `set_ios_*`, `set_apple_*`,
  each with its `derive_` twin) and its own controls in its `platform::<p>` module,
  whose data types exist in every build; the bare name `skene` is the command. Revisit
  when a platform's vocabulary diverges from its medium's further than its prefixed
  setters and its own module carry.
- **The `Platform` trait open to third parties** (2026-10-15), zero-sized and unsealed,
  views and libraries generic over it, the web's escape traits for a low-level view
  (`WebMarkup`, `WebDom`), the planned `PlatformView` with an embed constructor per
  leaf, and a host trait per view library (`acme-map`'s `MapHost`): one platform is
  chosen per build, `Platform::CURRENT` is a const enum and app code names no type
  parameter, and third-party platforms are dropped. A library reaches a platform
  through its native door (`platform::<p>::NativeView` with a `ControlSink`),
  configured declaratively in its manifest with native code in conventional `native/`
  folders where it is unavoidable, and a library's drawn control is built from the
  behaviors and a painted canvas. The terminal's `CellPainter` stays its escape trait.
  Revisit with third-party platforms.
- **Selection by facade, then by target** (2026-10-15), the host target selecting the
  headless platform and `skene-build` deriving the `skene_web_platform` axis: a plain
  `cargo test` on the host was green for a platform nothing ran. One platform per build
  is named by `--cfg skene_platform="<p>"`; `skene new` writes the platform's triple to
  `[build] target`, the cfg to `[target.'cfg(all())'] rustflags` and
  `build.rustdocflags`, and a runner into `.cargo/config.toml`; a build naming no
  platform fails with a `compile_error!`; and the CLI substitutes the cfg by reading
  cargo's rustflags sources in cargo's order, refusing when it cannot tell. Revisit
  with the test double.
- **A reversed lazy list** (2026-10-21), Compose's `reverseLayout`, putting item 0 at the
  far end: a list that starts at its end and keeps it in view while it shows (a chat) is
  SwiftUI's `defaultScrollAnchor`, `set_default_scroll_anchor(ScrollAnchor::END)`, the items
  keeping their order. Revisit when an app needs the items themselves reversed and
  reversing its data does not serve.
- **A realization's unit tests over a windowless instance of itself** (2026-10-16), the
  drawn Linux prototype's tests that built its shell with no backend, fed it synthetic
  pointer and key events and read its pixmap and its own tree: they read what no reader
  reads, kept a mode of the realization alive for tests alone and let a damage bug past
  the screenshots, while the driver runs the same prototype's 174 tests in 10 to 20
  seconds. Every test of a realization goes through the driver, on the platform's
  display, reading the accessibility layer and sending real input. Revisit when a
  behavior the driver cannot reach needs a test, or the driver's tests grow too slow
  to run on every change.
- **The headless platform** (2026-10-15), `skene-headless`, the host realization the
  web compiled against off its target, with a layout, a pinned font, a renderer and
  goldens held to the browser's: it grew from bookkeeping into a browser emulator
  without precedent, 4,273 lines whose browser emulator absorbed nearly all of its
  recent effort, and a green run
  proved nothing below the seam. Tests run on the real platforms (Option B): on
  Linux, accessibility reads over AT-SPI and real input, `settle()`,
  `#[skene_gui::test]` and a display runner giving each test process a private display
  and buses. Revisit with the test double.
- **Taffy behind a small FFI library, one tree per root** (2026-10-15), called by each
  platform's own layout primitive and crossed once per layout pass beside the
  platform's own containers: one tree per window was as exact as one per container but
  recomputed the whole window on any change, 200,000 toolkit measures for one label's
  change at 1,000 rows where one tree per container issued 32,120 and left the
  toolkit's measure cache to decide what recomputes. Taffy owns layout on every toolkit
  but the web's, one small tree per container. Reaffirmed 2026-10-27, its earlier
  revisit condition having held: a whole-scene tree over the terminal's
  realization recomputed only what a change touched, one label changed among 1,000
  rows of three costing 34 measures on the changed row's 3 leaves, 12 flexbox runs and
  1 grid run along its ancestors, 47 node layouts in all, and the author kept one tree
  per container regardless, a container being a decorated `Container` whose
  layout-only boxes fold into its tree, so the drawn runtime and a toolkit hosting the
  engine lay out the same trees. Revisit when a relayout under the per-container engine
  misses its frame budget where the whole-scene tree, measured on the same change,
  meets it.
- **AccessKit only where a painted layer needs it** (2026-10-15), native views carrying
  their own accessibility everywhere else: Skene's views are drawn on every platform
  but the web, so AccessKit reports for every one of them, Skene carrying its changes
  to it. Revisit with the platform's own controls as the built-ins.
- **Text through Parley on every platform but the web** (2026-10-15), the shared
  `Paragraph` following CSS Text everywhere: text and text input are the hardest parts
  of a platform to get right, and its engine carries its fallback, emoji, hyphenation
  and line-breaking rules, where on Linux Parley's fallback drew color emoji as
  monochrome boxes and read none of fontconfig's antialias and hinting settings. Each
  platform's own engine lays out and draws Skene's text inside Skene's views under
  taffy, behind a `TextEngine` trait per realization: TextKit 2 on iOS and macOS,
  `android.text` over Minikin on Android, DirectWrite on Windows, the DOM on the web,
  and Parley over Linux's own layers on Linux and in the terminal. The guarantee
  shrinks to a core (blanks and newlines kept, a word too wide for its line broken,
  an empty text one line), and one app may wrap differently per platform. Measured
  against Parley on 2026-10-17 in both of Parley's forms (the next two entries) and
  kept; revisit with them.
- **Parley and vello drawing every glyph** (2026-10-17), one layout engine and one
  painter on every platform but the web, the platform supplying its fonts and fallback
  alone: measured over `spikes/text-parity`'s corpus on macOS, the iOS simulator,
  Android 10 to 15 and Windows, vello's glyphs at the platform's own positions are
  lighter and softer than the platform's rasterizer everywhere (ink 0.83 of TextKit's on
  macOS and 0.90 on iOS, SSIM 0.944 against Direct2D and 0.971 against Skia), having
  vertical hinting alone and no gamma, stem darkening or LCD; they draw no Chinese on
  macOS 15 and iOS 26 without Core Text's outlines, PingFang's being in Apple's `hvgl`
  table; and they lose OpenType-SVG colour glyphs and Android's high-contrast text,
  beside every residue of Parley's layout (the next entry). Selection, editing and the
  input and accessibility protocols are Skene's own under every engine and were never
  at stake. Revisit when vello's text at a platform's own glyph positions matches the
  platform's rasterizer within the spread of the platform's own two paths (Direct2D
  against the DirectWrite atlas, SSIM 0.991), or when a platform's engine costs a
  function Skene needs.
- **Parley's layout over the platform's fonts and rasterizer** (2026-10-17), Chromium's
  model on every platform but the web: Parley breaks lines as the platform's engine
  does in 125 of the corpus's 131 variants on macOS, 124 on iOS, 123 on Windows and 116
  to 122 on Android once Minikin's whole-pixel advances are applied, but each platform
  keeps a residue its users see and a dependency its engine does not have. On Apple
  CJK punctuation compression is lost (a 27-character Chinese line 7.7% wider), Core
  Text's rule being unpublished, the Indic fallback instances stay a few percent off
  after seven rounds of calibration that consults TextKit at run time, and at the
  largest Dynamic Type size no line height lands within half a point. On Android the
  break rules at URLs, hyphens and no-break spaces, phrase breaking and Lao and Khmer on
  Android 15 differ, high-contrast text is Skene's to draw, OEM font styles are
  unmeasured, and an app grows 1.1 MB per ABI. On Windows, where breaks agree in 123 of
  131 and fallback in every character, Parley misplaces trailing spaces on wrapped
  bidi lines and stacks marks badly. Each platform's own engine stays. Revisit, per
  platform, when Parley's residue against its engine in `spikes/text-parity` falls
  within the spread of the engine's own two paths, or when a platform's engine costs a
  function Skene needs.
- **vello's outlines in place of FreeType on Linux** (2026-10-17), one glyph path on
  every platform Parley lays text out on: it would drop the glyph mask cache and the
  desktop's font-rendering settings, and with them the antialiasing and hinting the
  user chose, which FreeType honours. Linux keeps FreeType's rasterization. Revisit
  with Parley and vello drawing every glyph.
- **The platform's own controls as the built-ins** (2026-10-15), `Toggle` wrapping the
  platform's switch and every native platform's built-in a control of its toolkit: the
  governing rule is self-drawn everything without losing native functionality. Skene's
  built-in controls are drawn everywhere, compositor platforms included, a compositor
  platform's own reachable through `platform::<p>` and the native door; on the web the
  drawn controls are the form elements styled by CSS. Revisit when a drawn control
  cannot keep what the platform's own control has on that platform.
- **Leaving text editing to the platform's own fields** (2026-10-15), recorded on
  2026-09-17 as draw-everything rejected for the native family, since mobile text
  selection, IME and edit menus are years of platform work: a field of Skene's own
  keeps them by speaking the platform's text-input protocol and taking its selection
  chrome, the system's caret, highlight and handles tinted by Skene's style on iOS. Text
  fields and editors are Skene-drawn everywhere and the default, the platform's own
  text views low-level platform controls, and the web keeps its form elements. Revisit
  when a platform's protocol cannot feed Skene's field what the platform's own field
  gets: input methods, selection UI, autofill, dictation, handwriting.
- **A window platform realized through its toolkit's widgets** (2026-10-15), recorded
  on 2026-09-28 as a painted desktop rejected for owning text editing: where a platform
  only composites windows (Linux, Windows), Skene owns the whole window, draws
  everything and implements the platform's text and accessibility protocols itself,
  while a platform with a view compositor (iOS, Android, macOS, the web) takes Skene's
  views as its own. Revisit when a window platform's toolkit carries views an app must
  interleave with.
- **GTK as Linux's realization** (2026-10-15), GTK 4 widgets under taffy, styled by
  content-addressed classes and named colors, read through `GtkAccessible`: Linux joins
  Windows in the family Skene draws whole, on Wayland, X11, IBus and Fcitx, AT-SPI and
  the portals. The drawn spike passed IME on both display servers, Orca on a drawn
  field, client-side decorations and the Wayland test driver, at an estimated 16,000
  to 22,000 lines against GTK's 4,550 production lines (7,300 with its harness), a
  cost taken knowingly; the GTK prototype stays
  a reference. Revisit when an app needs to host GTK's own or third-party widgets.
- **Children last in the type system** (2026-10-15), the `NoChild` and `Children<V>`
  typestates: a property after the child failed as `no method named set_background
  found for struct Children<V>`, a type a third party's view could not construct. A
  wrapper holds its child boxed, a missing child renders nothing, and children last is
  a convention; typed children stay, a `Flexible` accepted by a `Row` or a `Column`
  alone, a `GridCell` by a `Grid` and a `Pinned` by a `Stack`. Revisit when a
  typestate's error can state the rule and its type is open to third parties.
- **Every signal restoring unless it opts out** (2026-10-15), `Signal::new` carrying the
  `Snapshot` bound and `Signal::transient` the way out: reload state is opt-in, so
  only the state an app means to carry across a reload asks its type for `Snapshot`,
  and a plain signal asks nothing. Revisit when the state apps opt in is most of the
  state they keep.
- **Lengths in `f32` alone** (2026-10-15): an app wrote `16.` where it meant 16, and an
  unsigned integer type, tried beside it, made `-4` a `-4.`. Integer lengths are `i32`
  beside `f32` wherever a length is taken, value constructors included. Revisit when
  the integer type measurably slows checking real view trees.
- **`#[inline(always)]` wherever appropriate** (2026-10-15): the attribute goes on tiny
  functions alone (a field read, a constructor, a forwarding call), and every other
  inlining decision is the compiler's. Revisit when a measurement shows a function that
  is not tiny gaining from a forced inline across crates.
- **The caret inside a repaired range going to the replacement's end** (2026-10-22):
  a truth replacing a proposal moved every offset strictly inside the replaced range
  to the replacement's end, so a handler uppercasing the text threw the caret to the
  end of the change wherever the reader typed. Such an offset now stays as many
  grapheme clusters into the replacement as it was, clamped to its end
  (`Edit::map_through`, over clusters by `TextIndex::map_through`), for the editing
  core and every realization's repair alike. Revisit when a platform's native field
  moves its caret to the replacement's end.
- **The web's `<input>` rewritten whole from its signal after a change** (2026-10-22):
  each `input` event reported the new value through the change path, and the control's
  reconciliation wrote the bound value back over the whole element, so a transformed or
  refused character moved the caret wherever the whole rewrite left it and broke an
  input method's composition. A text `<input>` or `<textarea>` now reports each `input`
  as an applied `Edit` and is repaired to a `Replace` by the smallest splice
  (`setRangeText`), its selection mapped through it, the repair held while an input
  method composes; a value the server serialized and the reader changed before the
  bundle replays the same way. Revisit when a browser loses its undo history or a
  composition to `setRangeText` where it keeps them under a whole rewrite.
- **Unsigned cells (`u32`) as the terminal's unit** (2026-10-23): a cell count cannot
  hold `-4`, so a negative margin or inset had no length to be, and every expression
  landing below zero clamped to it. The terminal measures in `i32` whole cells: a
  margin and an inset keep their sign, and layout takes any other length below zero
  as CSS does, a size as auto, a track list as dropped, a padding, a border or a
  spacing as none. Not revisited while lengths are signed.
- **A typed seam function per content and per report** (2026-10-24): the realization
  took `set_value`, `set_selection`, `set_checked`, `set_options`, `set_text` and
  `set_overlay_position`, one function per content, and reported a user's change
  through `notify_change`, `notify_selection_change`, `notify_toggle` and
  `report_edit`, so a value and what constrains it crossed as separate calls and each
  report kept its own echo and repair rules. Every content now crosses whole through
  one typed `write` and `read` keyed by Skene's name, and every change through one
  `report` saying whether the toolkit applied it, answered by the node's `Controller`.
  Revisit when a toolkit cannot take a content's parts as one write.
- **A choice held as a `String`** (2026-10-25): a radio group, a listbox, a menu and the
  picker named each option by a string, which an app converted its own values to and
  parsed back from. A choice is now a value of the app's own `Choice` type, erased to
  `AnyChoice` and `AnyChosen` in the cores and keyed by its ordinal in the `Roster`
  where a scriptless form needs a name. Revisit when a choice must cross a boundary that
  cannot carry an erased value.
- **The web re-checking a radio group's chosen radio itself, set aside for a re-read
  of every member** (2026-10-24): since 2026-10-23 a toggle report on one member
  re-read and rewrote every member's checked state in the shared half, so the web
  kept no copy of who is chosen. The group's chosen key now crosses as its `Chosen`
  content and each radio's group and key as its `Membership`, so the web applies the one
  write of the chosen key that checks the truth's radio, a refused choice repaired by
  that write, keeping no decision of its own, and nothing is re-read. Revisit when a
  platform's grouping of its members cannot be told through those two contents.
- **A control's build monomorphized per behavior** (2026-10-25): `Control<P, B>` built
  its node, its reports, its acting handlers and its registration generic in `B`, so
  every behavior an app or a library wrote, and every built-in alias an app used,
  instantiated the whole control build again in the crate that named it. A control is
  now `Thin` over a `ControlCore<P>` holding its behavior as an `Rc<dyn Behavior>`, and
  the facade builds that core once, a theme, a switch and a link alike over their
  `ThemeCore`, `OneOfCore` and `LinkCore`. Revisit if dispatch through the erased
  behavior shows in a measured profile.
