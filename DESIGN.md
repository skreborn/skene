# Design

The architecture of skene: facts and the reasons behind the load-bearing decisions.
Conventions and invariants live in `AGENTS.md`; what is open and what is planned in
`ROADMAP.md`.

Skene is a **client-side** GUI framework. One app, written against one dependency, runs on
whatever platform the build selects: the web (client-rendered, server-rendered, or
statically generated), the terminal, and painted-native desktop and mobile when that
platform lands. The reactive engine is signal-based and fine-grained; there is no virtual
DOM and no tree diffing. The framework ships no server-side features—no server functions,
no backend integrations. What it ships instead is a render the app's own server binary
performs as a plain command, so any server can serve a skene frontend without the
framework becoming a server framework.

The building block is a **view** (the `View` trait). "View" is the canonical term
throughout—there is no separate "widget" concept.

## Crates

```
skene-reactive   the engine: signals, memos, effects, owners, contexts, stores, Resource,
      │          the snapshot, the scheduler and spawner seams
      │
skene-core       the spine: Scene<P> (parents, children, annotations, token definitions,
      │          generational node ids), the Platform trait, event dispatch over the
      │          scene, the property model (BoxStyle<N, S, X>, RunStyle<S, X>, visitors),
      │          Value/Leaf/Expr, Media/State/Condition, Token/TokenSet, Boundary, motion
      │          and interpolation, Painting, generic geometry, universal values. Knows
      │          nothing of platforms.
      │
skene-common      every built-in view ONCE, generic over P: Platform—Container, Row/Column/
      │          Grid/Stack/LayoutBox, Flexible/Pinned/GridCell, Text/Span/RichText,
      │          Control<B: Behavior> and the shipped behaviors, RawPicker, Overlay,
      │          LazyColumn/LazyRow, Show/Switch/Suspense/ErrorBoundary, Theme, typography,
      │          LowLevel; gui (GuiExt, the pixel Typography set, Painting/Canvas/
      │          Painted through vello), tui (TuiExt, BorderGlyphs, the terminal
      │          Typography set), routing (a typed route in a signal)
      │
      ├── skene-web       Platform for the web: DOM sink (csr) + HTML/CSS serializer
      │                   (ssr) over one node record, the compiler, the interner,
      │                   hydration, streaming, the server executor, client hints;
      │                   escape traits WebMarkup (both leaves), WebDom (browser)
      ├── skene-headless  Platform for the host: realizes into an inspectable record
      │                   store so view logic tests run without a platform; the
      │                   whole trait, trigger_ seams, the pumped executor
      └── skene-tui       the terminal facade AND its platform: taffy in cells, a
                          cell buffer, motion on its frame clock; escape trait
                          CellPainter

skene-gui        the GUI facade: f32, ActivePlatform (the web on wasm32, the headless
                 platform elsewhere), aliases for every view, Typography, media::*,
                 Resource across the server boundary, Deferred, Lazy, Link, head,
                 view_transitions, launch and the server entries, #[main]
skene-tui        the terminal facade, EQUAL in standing: u32 cells, Tui, the same
                 aliases, Typography, BorderGlyphs, media::*, launch, render_to_text
skene-simple     the Simple design language over skene-common, generic over P: the palette
                 (a TokenSet), the looks, the shape per unit, the terminal marks
                 (SimpleGlyphs) and the Simple* views; an app's explicit dependency;
                 gui and tui alias the types to each facade's ActivePlatform, with a prelude
skene-macros     #[main], #[lazy], #[derive(Store)]
skene-cli        the skene command: build and run per target, the dev server, static
                 generation, skene.toml assets
```

**Two facades, one spine.** GUI and TUI are different media, not one medium with
fallbacks: a GUI app targets any combination of web and native and genuinely converges;
a terminal app is written for the terminal on purpose, and never cross-compiles into the
other family. Each facade therefore owns its builder vocabulary outright, typed to
exactly its capabilities—and everything beneath it is written once in the spine. The
facade's own contribution is small by design: a scalar, the medium's condition leaves,
the properties only that medium has, the platform it selects, and the aliases that give
app code a flat, medium-typed surface. What a medium lacks is absent at compile time
because the platform's associated types say so.

Dependency arrows only point down; platforms depend on core (never on each other); each
facade names only its own platforms and publishes the selected one as `ActivePlatform`.

## The scene and the platform

**Core owns the tree; a platform owns a sink.** `Scene<P: Platform>` is a generational arena
of node records—parent, children, the annotation a wrapper asked for the child, the
token definitions a theme placed on the node, the query-container name, the handlers,
and the platform's sink handle (an element index, a record handle, a taffy id). Every
structural mutation (`attach`, `detach`, `destroy`, reorder) is implemented once in core;
the platform is called only to mirror it into the sink, after core has released its own
borrow. `NodeId` carries a generation, so a stale id resolves to nothing rather than to a
recycled node. Token resolution and event dispatch walk this tree; nothing reads
structure back from a platform. The scene also records each node's place in focus
traversal and each overlay's state, the open ones in the order they opened, as a
browser's top layer stacks them, and computes the tab order from them
(`Scene::tab_order`): positioned stops first, then document order, the modal opened
last containing it with the overlays opened after it. A platform drawing its own
controls traverses by it, stepping from the focused node by `Scene::tab_from` (from a
node that is no stop, the nearest stop beside it in document order), the terminal and
headless alike: headless applies a `Tab` its handlers leave to the platform. The scene answers which overlays a press dismisses
(`Scene::dismissed_by_press`) for every platform: from the top down, each overlay the
press lands outside of, on neither it nor the anchor it opened from, until one it lands
inside of, a modal ending the descent and dismissed itself; the web leaves a modal's
backdrop to its click. The scene holds the interaction every platform reports: the
focused node with whether focus shows (a key sets it, a pointer press clears it, a text
entry's shows however reached), the pressed node and the hovered one, dropping each
with its node, and resolves every node's interaction from them (`Scene::interaction`),
so the terminal and headless resolve states alike.

**`Platform` is a zero-sized trait with associated types**, monomorphized, hidden behind
each facade's aliases: `Sink`, `Scalar` (`f32` or `u32`), `StyleExt` (the medium's own
properties), `TokenValue` (the erased definition the medium accepts). Its methods are the
portable sink vocabulary: create and update boxes, controls, overlays and runs, create
anchors and scopes, mirror tree mutations, define tokens on a scope and at the root,
register event kinds, focus, set values, observe sizes, report media. `Painted` and
embeds are deliberately outside it—their payloads never cross a crate boundary—and
arrive through extension traits (`Painting`; each platform's escape trait). The leaf-only
surface (`document`, `element`, `render_to_string`, `launch`) stays free functions in the
platform crate, since it is not a shared contract.

- **Why a trait and not name-resolved free functions.** A missing function is only an
  error on a path that names it, and those paths are target-gated; a trait makes a gap a
  hard error. That is what makes the host platform complete, so every view and `Resource`
  are testable headlessly.
- **Why the scene is in core.** The scene *calls* nothing: core mutates it and then
  calls the platform through the generic parameter. Every platform needs the structure; one
  implementation is smaller and has one reparent semantics.
- **Dispatch is core's.** A platform reports an event at a node; core walks the
  parents the node had when the dispatch began, as the DOM fixes an event's path, runs
  handlers deepest first, folds `Prevented`, and tells the platform what the platform
  may still do. The browser's root delegation, the terminal's hit path and the
  headless platform share one bubbling implementation, so the rule that a view owns the
  interactions it handles is enforced, and testable, in one place. A click on a native
  web toggle is dispatched *observed* (`dispatch_click_observed`): every handler on the
  path runs except the acting one the behavior registered, since the platform's own flip
  arrives as `change`, and the climb ends at the control, as the acting one's verdict
  ends it on every other platform. A focus move notifies each node whose subtree it
  enters or leaves (`notify_focus_moved`), as `:focus-within` changes, so a box hears
  focus reach any control inside it and never hears it move between them; a pointer
  move does the same for enter and leave (`notify_pointer_moved`) over the hovered node
  the scene holds, which every platform moves.
- **Statics batch, deriveds bind.** A builder's fixed properties, token references and
  expressions arrive together in the create call—the web folds them into one
  content-addressed class—while each derived leaf binds a render effect that writes its
  per-node slot. The split is written once against the property visitor.
- **`Role` guides the platform's concrete choice** (web: `Role::Navigation` → `<nav>`); the
  vocabulary stays semantic, never platform-worded.
- **Painted**—a vello paint callback plus a layout size, portable across every
  *interactive* pixel target through the `Painting` extension. A `Canvas` is a vello
  scene in device pixels (kurbo shapes, peniko brushes, the framework's `Color`
  converting to a brush), recorded once and replayed into the platform's `Renderer`. The
  browser leaf renders it into a `<canvas>` placed over the box and out of its flow, so
  it never sizes the box, observed in the device pixels the engine reports (the
  content box times the pixel ratio, followed through a resolution query, where it
  reports none) and repainted when either changes: through vello's hybrid renderer
  over wgpu where the page has WebGPU, one device shared by every painted box with a
  surface per canvas, asked for afresh when lost, and through vello's CPU renderer
  otherwise, in a fresh canvas where a canvas's GPU path failed; the headless platform renders it to pixels
  a test reads (`headless::paint`); the server leaf renders it empty; the terminal
  renders it on the CPU and shows it through the kitty graphics protocol where the
  terminal proved one, its fallback standing elsewhere. The native layers sit behind
  the same `Canvas`. `Painted`, `Canvas` and `Painting` live in `skene-common`, since
  both medium cores realize them. Vello is the one renderer, everywhere.
- **Embeds are platform-specific views**, written against a platform's escape trait: raw DOM
  through `WebDom`, markup through `WebMarkup`, cells through `CellPainter`, later a
  platform view through `PlatformView`. `LowLevel::realize` registers a platform-made sink
  as a scene node, so the view composes with wrappers, themes and events. There is no
  `Any` and no downcast.

  **Why a typed vocabulary and not per-platform realization.** The orphan rule: with
  views in one crate and platforms in others, `impl View for Container` can live in neither
  platform, so separate platform crates are impossible under per-platform realization. Typing
  the vocabulary moves the medium decision (CSS text, native styles) into the platform crate
  while realization is written once above it.

  **No mock platform—but headless tests.** Because the vocabulary carries typed data, the
  host platform (`skene-headless`) realizes exactly what any real platform receives into an
  inspectable record store, resolves a property through core's `Resolver` (the scene's
  tokens, the expression evaluator and the environment), and runs tasks when a test
  pumps them, giving
  view logic fast headless tests without pretending to be a platform. Platforms test against
  their real sinks (web: a browser DOM via `wasm-bindgen-test`; ssr: the server leaf
  natively under a byte-exact snapshot; native: its toolkit).

## Composition

`View::build(self) -> NodeId` is the whole trait: a one-shot, side-effecting descent that
mints scene nodes, binds effects, attaches children and returns its root. There is no
update path on a view; updates are the effects a build left behind. This is the Solid
model, and it is kept because fine-grained signals make a diffing view tree unnecessary
and its cost unjustified.

- **Anchors, not groups.** A reactive subtree—`derived`, `Suspense`, `ErrorBoundary`,
  `Deferred`, `Lazy`, `empty`—mounts between two anchors (a comment pair on the web, a
  non-layout record in the terminal) rather than inside a box of its own. Its nodes are
  therefore its parent's children for layout, and a wrapper around it annotates the
  content, every rebuild's and each form of a two-form view's: a boundary's fallback,
  a switch's branches. A group node that is a real `<div>` and a real taffy node makes every
  derived subtree one flex item, which is why there is none.
- **`Children` is a list of segments**, static or keyed, each between its own anchors;
  `set_children` and `derive_children` append. A header beside a reconciled list in one
  `Row` needs no nested container. The child kind is carried on `Children<P, Kind>` and
  the setter that would bypass it is sealed.
- **Keyed reconcile** keeps its LIS-minimal reorder, its reused scratch buffers, and
  builds items untracked; a rebuild closure (`derived`) tracks. The two names say which.
- **Structural change is a last resort with a cheap alternative.** `derived` destroys
  and rebuilds, disposing the subtree's scope with its state; `Show` and `Switch` build
  branches once and attach or detach them, preserving state. `Suspense` and
  `ErrorBoundary` are built on them and destroy the losing branch on disposal.
- **A view declares its baseline and its enhancement.** `Enhance` in `skene-common` mounts one of two builders between its anchors:
  `set_fallback(|| baseline)` is the form that works before the environment is ready,
  and `set_child_builder(|| enhancement)` the form built once it is. Which is built
  follows a signal the enhancement is given (`Enhance::new(live)`), and the signals
  the built-ins wait on belong to the things that can lack them, never to a shared
  capability vocabulary. **Hydration** is the web's: `Platform::hydration()` returns
  the signal that turns true once a render made without script has been hydrated,
  and `None` by default, which every platform whose tree is live from the start keeps,
  the terminal and the native platforms among them, so nothing on the terminal facade
  can name it. The GUI facade reads it as `media::hydrated()`, true from the start on
  native. The server leaf answers false forever, so a server render and a static page
  carry the baseline as markup; the browser leaf answers false while a hydration
  adopts the server's markup and flips to true once it has finished and the adopted
  live values have reached their signals, so the baseline is built once more on the
  client, adopted element by element with no mismatch, and then rebuilt into the
  enhancement in place, as a `derived` rebuild is, under a replacement owner, so a
  signal bound beneath keeps its value and its snapshot identity; a mount that
  hydrates nothing answers true from the start and never builds the baseline. The
  headless platform answers hydrated until a test says otherwise (`trigger_hydrated`),
  so a generic view's two forms are tested on the host. **Pixels** are `Painting`'s:
  `Painting::available()` is the signal a painted box waits on, false on the server
  leaf, false on the browser leaf until it is live, false in the terminal until the
  graphics probe is answered, and the headless platform's from `trigger_graphics`. Only
  the branch in force is ever built, so an enhancement may touch the browser and a
  baseline may be the whole view where the signal never turns. The seam is the
  built-ins' own: `Painted` takes `set_fallback`, the view drawn where no pixels
  show, so a painted box has a no-JS form on the web and a cell form in the terminal
  through the one mechanism, and the picker's prerendered `<select>` is the same swap
  over `hydration()`, drawn outright on a platform that has none. What the
  seam does not do: read the baseline's platform state into the enhancement by
  itself. A value the reader typed into a baseline entry before the bundle arrived
  reaches the enhancement through the signal the entry is bound to, as any live value
  does, and the focus and scroll inside the baseline are lost by the rebuild, as they
  are by any.
- **Wrappers annotate before creation.** `Flexible`, `Pinned` and `GridCell` write their
  annotation into the scene slot the next box, control, run or low-level node built
  under them consumes, so per-child layout is an ordinary property of the child's
  style: in the class on the web, conditional, token-valued or derived like any other.
  A run carries it as its layout, its derived leaves slots numbered after the run's
  own, and a low-level node hands it to its platform through `Platform::apply_layout`,
  which the web compiles into the element's class.
- **Owners** are created per reconciled item, per rebuild, per keyed view and where
  context is provided; everything else inherits the ambient owner. `derived` rebuilds
  into `Owner::replacement()`, the same parent and step, so snapshot identity survives
  a branch flip. A reconciled item's owner is `Owner::keyed_child` under the item's key,
  and `View::set_key` builds any view under one, so state beneath either restores by
  key wherever it sits.

## Controls

A control is `Control<P, B: Behavior>`: one builder carrying focus, events, decoration,
label, parts and accessibility once, over a public `Behavior` that says what the
interaction *is*—its role, its keys, its pointer handling, what it reports to assistive
technology, where it sits in the tab order, and which parts it exposes. `Toggle`,
`Range`, `Roving` (with `RadioMember`), `Popup`, `Activate`, `TextEntry` and `Listbox`
(with `ListItem`) ship; `RawButton`, `RawCheckbox`, `RawSlider`, `RawRadioGroup`,
`RawRadio`, `RawListbox`, `RawOption`, `RawTextField` and `RawTextEditor` are aliases over
them, and `RawPicker` composes a `Popup` face with a `Listbox` in an `Overlay`. Both
have type-ahead: `TypeAhead` holds the characters typed in quick succession, reset by
a `Platform::schedule` timer, and `Roster::typed` matches them against the members'
names (`Label::set_name`, the value standing in), one character cycling through the
names starting with it and more matching a prefix; the listbox moves focus to the
match, and the closed face chooses it outright through `Popup::handle_typed`, the
behavior's hook for the keys it does not act on itself. A behavior that follows a
press's pointer until its release (`Behavior::drags`, a slider's) keeps the pointer
with its control: the web captures the pointer on a primary press and takes a touch as
a drag (`touch-action: none`), and the terminal, which routes every held press to the
node it landed on, shows a grab cursor over it. An anchored popup on the web follows its anchor through CSS anchor positioning (the anchor
named after its sink, the popup placed by `position-area`) where the engine has it, and
sits at the anchor's measured rectangle as it opens elsewhere. A `Tooltip` wraps the one
view it describes and mounts that view and a hint overlay, the `Bubble`, in a segment of
its own: it opens on the platform's clock once the pointer has rested for its delay and
at once on focus that shows (`Platform::focus_visible`), closes on leave, blur and a
dismiss, and reports the bubble as the description (`DescribedBy`, and `Description` for
text) of the node that takes focus within the view (`Scene::focus_holder_within`), the
report merged in the scene with that node's own; the web's server leaf renders the text
as `title` and hydration removes it.

A **context menu is a view of the same shape**. `ContextMenu` in `skene-common` wraps
the one view it serves and mounts that view and a popup overlay, the `List`, in a
segment of its own, exactly as `Tooltip` does; `serve(node)` attaches the list under a
node that already exists instead, for an app that wants a menu over its root. Wrapping
is the opt-in: a secondary press on the wrapped view, or `Shift-F10` and the
`ContextMenu` key while focus is inside it, dispatches `EventKind::ContextMenu` at the
node through the ordinary bubbling dispatch, and the deepest wrapper along the path
opens its menu pinned at the pointer (at the focused node's bottom-start corner from
the keyboard, through `Platform::set_overlay_position`) and answers `Prevent::All`, so
the browser's own menu stays wherever no wrapper is, and an app that wants its own
everywhere wraps its root. The terminal facade serves the standard menu from the root
at install, because the terminal has no menu of its own; a deeper wrapper wins over it
as it wins over the browser's, so replacing the default is the same act on both
facades. The items are `MenuItem`s: `MenuItem::new(label)` with `handle_select`,
`set_disabled`/`derive_disabled` and `set_shortcut` (a hint the item shows, never a
binding), `MenuItem::separator()`, and the standard entries `MenuItem::copy()`, `cut()`
and `paste()`. A standard item asks the platform for its `Availability` each time the
menu is requested: `Absent` hides it, `Disabled` lists it disabled, `Enabled` lists it
live—outside a text entry only `Copy` exists, and only with a selection; inside one all
three are listed, `Copy` and `Cut` disabled without a selection—and running it calls
`Platform::clipboard` (OSC 52 in the terminal, the async Clipboard API on the web, where
the items are absent without it and a paste is permission-gated and stays best effort;
the headless platform records the action). A menu with nothing usable does not open but
still prevents the platform's, which is what a right press on an empty spot did before.
Extending the standard menu is listing the standard items beside one's own; nothing
merges implicitly. The list is a `Menu` control (`Role::Menu`) over `MenuOption`
controls (`Role::MenuItem`), the behaviors a third party's menu bar reuses: the arrows
walk the enabled items with wrapping, `Home`/`End` reach the ends, typing runs the
roster's type-ahead, `Enter`, `Space` or a click runs the focused item and the menu
closes, and `Escape` and a press outside close it through the overlay's light dismiss.
The first usable item takes focus from an ordinary effect rather than a render effect,
so it runs after the items' disabled states have settled. The menu is decorated per
property like every control—the list's box properties, `set_item_padding`,
`set_item_color`, `set_item_corner_radius`, `set_item_cursor`, `set_item_text_color`,
`set_shortcut_color`, `set_separator_color`, the `Part::List`/`Part::Item` targets; the
terminal's standard menu is the raw view decorated with the platform's own values, and
`SimpleContextMenu` (`skene-simple`) is the palette-decorated one on both facades. In
the terminal focus moves into the open list while the entry beneath keeps its
selection, so the standard items act on it, and closing returns focus without
reselecting the value; `Escape` closes an open menu before it clears a selection. The
web platform listens to `contextmenu` only on a node whose wrapper registered for it,
positions the popover fixed at the pointer and shifts it back into the viewport, and
the server leaf renders nothing for the menu: with no script the browser's menu is the
menu, as progressive enhancement asks.
The
`Simple*` components (`skene-simple`) are the ready-made library over the same builder:
one palette, one shape language, shared across the GUI and the terminal wherever the
medium allows, and deliberately nothing a third-party library could not build on the
same footing—including how an app reaches it: an explicit dependency, never a facade
re-export.

- **A control decides its verdict.** The behavior runs first and its verdict is the
  whole verdict; a user's handler observes. A button's click never reaches the row it
  sits in, on any facade.
- **A control is controlled.** It reports; the bound signal is the truth. Two-way
  binding is a derived read plus a handler write, on every facade.
- **Parts and control state.** A part is a named child the styling targets (`Track`,
  `Fill`, `Thumb`, `Mark`, `Face`, `List`, `Item`, `Label`); in the terminal a part maps
  to a glyph token. A `State` a box cannot have on its own (`Checked`, `Indeterminate`,
  `Disabled`, `Invalid`, `Open`) resolves against the enclosing control; a node's own
  `Hovered` and `Focused` stay its own. On the web a control that owns a native input
  compiles control state to a selector anchored on that input; natively it is the
  control's own state.
- **The scriptless form is the control.** A text field or editor is its native
  `<input>` or `<textarea>` itself, decorated as the box; a checkbox, radio or slider
  keeps its native input inside its label, visually hidden but focusable, beside the
  parts that draw it. So keyboard, form participation and accessibility are the
  browser's before and after hydration and nothing is swapped. The picker alone is
  drawn (no engine but Chromium restyles an open `<select>` yet), so it is an
  `Enhance` of its own: its scriptless form is a `NativePicker` control, a `<select>`
  on the web, listing the options the option views name, its caption a `<label for>`
  beside it and the chosen value selected; the option views build once, under the
  group both forms answer through, and attach nowhere until the drawn form lists
  them, and the caption builds once and each form mounts it with its name and the
  disabled setting, so a swap rebuilds nothing but the face; a choice the reader made in the
  `<select>` before the bundle arrived reaches the signal as any adopted live value
  does, and the drawn face that takes the select's place shows it.
- **Accessibility is reported, never spelled.** A behavior reports what it is
  (`Checked`, `Value`, `Expanded`, `HasPopup`, `Orientation`, `Selected`) and a control's
  user adds what any control may carry (`Required`, `ReadOnly`, `Invalid`,
  `DescribedBy`, `Disabled`); the platform maps each to its layer. On the web a form
  element takes the native attribute where one exists (`required`, `readonly`, and a
  slider's `value`, `min`, `max` and `step`), so the form enforces it without script,
  and ARIA otherwise; a property naming another node
  (`Controls`, `LabelledBy`, `DescribedBy`) gives that node an id derived from its node
  id, identical on both leaves. A name comes from wherever it is: a string caption is
  the control's own (`aria-label`, or the native input's), and a caption drawn by a
  view names the control by reference (`LabelledBy` the caption part), so a combobox,
  a listbox or a radio group, which ARIA forbids naming from content, is named all the
  same. A region, a dialog and a painted picture name themselves through `set_label`
  and `set_labelled_by` on `Container`, `Overlay` and `Painted::set_description`, the
  latter making the box a named image; without a description a painted box is
  decoration. `Role::Status` and `Role::Alert` are the live regions, `Role::Dialog` a
  popup's role, and a tooltip's bubble carries `role="tooltip"` from the platform. A box
  that scrolls takes `tabindex="0"` unless a focus order is set, as Chromium makes a
  scroller focusable, so the keyboard reaches its content. The gallery runs under
  axe-core in the browser tier (`web_axe`, the library vendored beside the test), so
  every built-in is checked against the ARIA rules as it is drawn. Disabled controls
  leave the focus order, a deliberate reading of the practices' allowance, recorded
  in `AGENTS.md`.
- **Membership is read from the tree.** A grouping control's members live in a `Roster`
  ordered by walking the subtree under its root, so a reconciling set stays in document
  order however it changes and a destroyed member is absent because the walk cannot
  reach it. A radio group wraps, a listbox clamps, because the platforms' do.
- **The terminal's controls are composed trees**—a `Container`, a `Text` mark from a
  glyph token—painted by the same painters as everything else, so they carry roles,
  accessibility properties and focus order, and the keyboard tables live in the
  behaviors rather than in a match per medium.

## The style model

Styling is **data**, not closures, because two requirements demand it: server-rendered and
statically-generated pages must be responsive, themed and animated with **no JS/WASM**,
and every platform speaks the same layout semantics. A closure is opaque—a server can
only serialize its current value; style data compiles to real CSS and evaluates natively.

**One property model, spelled once.** `BoxStyle<N: Scalar, S: Shape, X: StyleExtension>`
and `RunStyle<S, X>` name every shared property once; the leaf shape is a type parameter
(`Declared` for a builder, `Static` for a platform, `Plain` for a keyframe or a resolved
style) and the medium's own properties come through the extension (`GuiExt`: the corner
radius and the type metrics; `TuiExt`: border glyphs, the corner radius, a shadow, bold,
size and weight). A corner radius, a font size and a letter spacing are lengths
(`Length`) in the medium's unit, fractions kept, so one length token serves them as it
serves padding and sizes. The
shared run properties are the text color, the selection's ink and surface, and the
decoration: the line (`TextDecoration`: beneath, above or through the text, a link
underlined unless set), its style (`TextDecorationStyle`: solid, double, dotted,
dashed, wavy) and its color, each a property of its own so the color takes a token.
The web compiles them to the `text-decoration` longhands; the terminal writes the
underline styles (SGR `4:3`, `4:4`, `4:5`) and color (SGR `58`) where a DECRQSS reply
to a curly-underline probe proves the terminal keeps them, a plain underline
elsewhere, and the overline and the line through as SGR `53` and `9`. A
visitor written once gives overlay, the static/derived split, CSS rendering and taffy
application their iteration; what remains per property is the field, its CSS name and
its taffy mapping. `CornerRadius` is the pattern for an optional capability: a style
extension says whether it rounds, a library that merely prefers a radius degrades on a
medium without one, and a library that requires one bounds on it and does not exist
elsewhere.

**A property value is a base and its conditions, and each leaf is one of five kinds.**

```
StaticLeaf<T> = Fixed(T) | Token(TokenName) | Expr(Expr<T>)
Leaf<T>       = Static(StaticLeaf<T>) | Derived(Rc<dyn Fn() -> StaticLeaf<T>>)
Value<T>      = Conditional<Leaf<T>, Condition>          // what a setter takes
Definition<T> = Conditional<StaticLeaf<T>, Media>        // what a token is; no closures
Media         = MinWidth | MaxWidth | MinHeight | MaxHeight | Dark | Light | ReducedMotion
              | ReducedTransparency | HighContrast | Container { name, query } | And | Or
State         = Hovered | Focused | FocusVisible | Pressed              // the node's own
              | Checked | Indeterminate | Disabled | Invalid | Open      // the control's
Condition     = Media | State | And | Or
```

- **Fixed** leaves render as literals. **Tokens** render as `var(--name)` on the web and
  resolve through the scene elsewhere. A leaf holds a token by its name: a `TokenRef<T>`
  is of a type a token holds (`Tokenable`: a color, a number, a length, a dimension or
  a text) and converts into the leaves of every property type reading it, a length
  token into a padding's or a radius's, so no reference is ever retyped. **Expressions**
  are typed arithmetic that compiles to `calc()`, `clamp()`, `color-mix()` and relative
  color and evaluates natively: lengths and numbers have the ring operations CSS
  type-checks, colors have only the named functions, and there is no boolean
  expression, so an ill-formed expression does not compile. **Derived** leaves are the script-only kind: on the web a per-node custom
  property slot the class references (registered `inherits: false`), written by a render
  effect, so a derived base never outranks a variant; on the server, evaluated once. The
  closure returns any static leaf, so a state chooses between tokens: the slot holds the
  token's reference, and the theme in force answers it where the slot is read. A
  definition admits no closure, which is what keeps a palette plain data that
  serializes, hashes and renders without script.
- Conditions ride the value: `value.when(condition, other)`. The composition machinery
  (`Composable`: `holds`, DNF expansion) is one generic in the spine; the web emits one
  rule per conjunction and groups a node's properties by canonical condition, the
  terminal and native evaluate against live state. Container queries are
  `Media::Container` leaves against a box that named itself; the web stamps
  `container-type` and emits `@container`, the terminal resolves them in a second pass
  with the container's measured axis fixed, which is sound for the reason CSS makes it
  sound. `Hovered` holds on the node under the pointer and on its ancestors in the
  tree, an open overlay's among them, as `:hover` does, and `Pressed` on the node a
  primary press landed on and on its ancestors until the release, as `:active` does, a
  control's parts pressed with it.
- **Layout** runs on taffy at full capability, statically constrained per layout: each
  mode is its own container view carrying layout properties alone, `Container` is the one
  decorated box, and per-child properties ride in wrappers. `Dimension` is a fixed length,
  a `Fraction` of the parent, or auto. Overflow is per axis (`Axes<Overflow>`, one
  overflow setting both) and computed as CSS computes it, an axis left visible beside
  one that is not scrolling where needed; the terminal scrolls, and draws a scrollbar,
  only along an axis that scrolls. An adaptive track (`GridTrack::Adaptive`,
  `Grid::adaptive`) fits as many tracks as the container holds, as SwiftUI's adaptive
  grid item does, compiled to CSS's auto-filled repetition. A grid names areas over its
  cells (`GridAreas`, a row of cell names per grid row) and the lines between its
  tracks (`GridTemplate`, a track list with names added where lines stand, a plain
  list naming none), and a child is placed by an area's or a line's name (`GridName`,
  `Line::area`), a template breaking CSS's rules dropped whole on every platform as CSS
  drops it. The framework invents no layout
  system.
- **Raw CSS** reaches a node only through the web escape trait: `WebMarkup` takes
  `Declarations`, the sorted, content-addressed declaration set the compiler itself
  renders into. The typed model carries no passthrough of its own.
- **Web** renders the whole set into one interned rule template: base declarations,
  `@media`, `@container` and pseudo-class rules, `transition:`, content-addressed
  `@keyframes`, `var()` for tokens and slots. State-, media- and theme-driven styling
  therefore works in a static document with zero script.
- **Terminal** (and native) resolve every property at one point through core's
  `Resolver`—variant overlay against the environment and the node's `Interaction`, token
  lookup through the scene, expression evaluation, slot reads—handing it the medium's
  unit, its token and slot lookups and its `em`. The terminal caches the result per
  node under a generation covering environment, interaction and definitions, and
  reports whether the resolved value changed; that report is where a transition
  starts. The headless platform resolves through the same `Resolver`, so a test's answer
  and the terminal's are one computation.

### Tokens and themes

A design token is a named, inherited slot. `TokenRef<T>` names it (a `const`; the name is
an implementation detail on every platform), `Token<T>` pairs the name with the
definition a palette gives it, and a consumer emits only the name, so two boxes under two
palettes intern one class and a theme change never touches a consumer. A token's own
conditions live in its definition, once. A length token serves every property measured
in lengths, a box's sizes, padding, spacing and margins, its corner radius and a run's
font size and letter spacing alike, and a property that is a plain number reads a
number token alone.

`Theme<P, P: TokenSet>` is the one theme view. It realizes one layout-invisible scope node
(`display: contents` on the web, a scope record in the terminal), provides `Memo<P>`
through context, and runs one effect that writes `P::definitions()` onto the scope. A
palette change therefore costs the number of tokens, never the number of nodes, and
nothing rebuilds; nesting a theme redefines beneath it. On the web the static definitions
are an interned class on the scope element (with `@media` blocks and the scheme double
emission where a definition conditions on the scheme) and each token kind registers its
custom property with `@property`, so it interpolates and inherits as a typed value; a
token set used with no theme above registers its default under `:root` once
(`ThemedIn::current`/`watch`). `Typography` (all text inheritance on both facades) and
the Simple library's `SimpleGlyphs` (its terminal marks, a set only a platform with text
tokens over cells can theme) are token sets, so the framework has one inheritance
mechanism.
Nothing below a component library knows what a palette is: `skene-simple` owns
`SimplePalette`, and a third party's `AcmePalette` gets `Theme<P, AcmePalette>`,
`current()` and `watch()` from the same blanket.

The color scheme is one `Media` leaf, answered per property in a definition or a value.
The scheme *in force* is the document's: `media::set_scheme` writes `data-skene-scheme`
on the root (with `color-scheme`), every scheme-conditional rule is emitted twice—the
preference query guarded by `:where(:root:not([data-skene-scheme="light"]))` and the
choice as `:where(:root[data-skene-scheme="dark"])`, both in `:where()` so they add no
specificity—and the terminal keeps the choice beside its detected background. A
server-rendered page that offers a choice emits one inline blocking script that applies
the stored choice before first paint, carrying the render's script nonce
(`set_script_nonce`) so a strict Content Security Policy admits it. Scoping a scheme to
a subtree is not wanted.

### Environment

`media::viewport()`, `media::scheme()` and `media::reduced_motion()` are signals on every
facade, backed by the platform: `MediaQueryList` and `resize` in the browser, the
terminal's size and background, the headless store's settings. `media::watch(&media)`
turns any declared condition into a memo rendered from the same query text the class
compiler renders, so the signal and the stylesheet cannot disagree. A box's measured size
reaches a view through `Platform::observe_size` (the lazy column's viewport, the slider's
track). On the server these hold the client hints the render was given
(`Sec-CH-Prefers-Color-Scheme`, `Sec-CH-Viewport-Width`, `Sec-CH-Prefers-Reduced-Motion`;
Chromium only, opt-in through `Accept-CH`/`Critical-CH`), else documented defaults.

### The interner

The content-addressed class interner is load-bearing: a node's complete rule template is
hashed with a wide hash to a deterministic name, interned once, reference-counted by live
node, collected when unused; naming never depends on intern order or liveness. Identical
styles collapse to one class; both leaves emit rules in interning order, which is build
order, so the server's stylesheet is byte-compatible with the client's and the cascade
order is identical, which hydration relies on. A class cache keyed by the hash of the
typed props renders the template text on a miss only. The interner is pure and shared by
the web leaves; each layers its sink (csr: one `<style>` element per class; ssr: the live
templates rendered once at the end, plus a delta `<style>` per streamed chunk for classes
interned since the previous flush).

## Reactive engine

The public API speaks the ecosystem's standard reactive vocabulary:
`Signal`/`ReadSignal`/`WriteSignal` (Copy handles), `Memo`, `Effect` with an
`EffectHandle`, `Trigger`, `Selector`, `Owner`, `provide_context`/`use_context`,
`batch`/`untrack`, the access traits, `Store` with its derive, `Resource`, and the
`set_`/`derive_`/`handle_` builder convention above it. The internals contract:

- **Split arenas, intrusive edges.** Signals, computations and owners live in separate
  generational arenas selected by id bits; a signal costs one allocation. Dependency
  edges are a pooled doubly-linked list (the alien-signals shape): subscribe and
  unsubscribe are O(1), and a re-run walks its previous edges and reuses each unchanged
  one, so a steady-state effect run allocates nothing.
- **Two-color propagation.** A write marks direct subscribers dirty and transitive
  computations pending-check; only effects enter the run queue, already in dependency
  order. Memos are lazy: an unobserved memo never runs, a memo read once in a batch
  computes once, and a memo whose recomputed value is unchanged (per its comparator)
  notifies no one. A memo is never stale to a reader, including inside a batch.
- **Ownership is ordered.** An owner disposes its children newest first, then runs its
  cleanups in reverse registration order; a re-run first tears down what its last run
  made, and is a batch, so what it invalidates runs once it ends. A moved subtree takes
  the depths of its new place. A pull walks its own stack, so a memo chain of any
  length settles without deepening the call stack. A `Selector` answers from its pulled
  current key, inside a batch too, and a per-key trigger, like a store location's, goes
  once nothing tracks it.
- **Effects have handles and flavors.** `Effect::render` effects flush before ordinary
  effects in owner-depth order and are what property binders and the structural effects
  (`derived`, `reconcile`, `Show`, `Switch`) are, so a parent's structure settles before
  its descendants' properties and a disposed descendant never runs; `Effect::watch` is
  the fine-grained binder; a handle disposes or pauses.
- **A pluggable scheduler, flushed by turns.** Synchronous at write end by default;
  every platform holds the flush until its turn ends, so what a build notifies is
  delivered once the build is whole. The terminal's turn is an event, a run of due
  timers, a task pump or a render's settling round; the browser's is an event, a
  mount, a hydration or a navigation, and the next microtask checkpoint for anything
  else; the server's is its build and each round of task polls, as a batch. On the
  headless platform and the terminal a test's trigger is a turn and a query ends the
  pending one first, so a test sees what the loop does; `end_turn` on the headless
  platform and the browser settles a direct write where a test observes it otherwise,
  and `flush_sync` settles mid-turn where a turn needs it, as a key's default does
  after its handlers.
- **Panic and borrow discipline.** `batch` and `Owner::enter` are guarded; no user code
  runs under a runtime borrow; an entity is moved out of its arena before its cleanups
  run. A read of a disposed signal panics and a write to one is dropped; a debug build
  names the late access, where the signal was created and the call that disposed it,
  and warns of the write.
- **Threading.** Each thread's runtime owns what that thread made, and every handle
  stays on its thread; work on another thread reaches it through a task that thread's
  executor wakes, as the transport's clients do. An id carries its runtime's nonce, so
  a handle that outlives its runtime resolves to nothing.
- **The store is a trie.** One node per location ever addressed, interned to a
  `StoreLocation`; each node carries a `below` trigger (a write here or beneath) and a
  `here` trigger (a write exactly here), a reader tracks its own `below` and every
  ancestor's `here`, a write notifies the reverse, so a row edit over ten thousand
  tracked cells costs the same as over a hundred. Field keys are a hash of the field
  name (or its `#[store(tag)]`), stable under reordering; the value stays a plain struct.
  A location derives a memo (`derive`), and a keyed location's `entries` subscribe to
  its shape alone—its own `here` and its ancestors'—so a keyed list follows the
  collection while each item follows its entry; `HasChildren::derive_entries` feeds
  them to `reconcile` keyed by the entry's key, and a `Vec` is keyed by a key its
  elements carry (`keyed_by`), read by scan.
- **Snapshot identity is positional, and a key anchors it.** A signal registers by the
  path of steps from the runtime's root owner, its index among its owner's snapshot
  signals and the hash of its type name. A step is an owner's position among its
  siblings, or the `Key` of a keyed owner: a keyed step takes no position, so keying
  a sibling shifts none of the positional ones, and a key is the hash of the value it
  was taken from, so `reconcile`'s item key and a view's `set_key` are one concept.
  `Signal::opaque` occupies no position, `Signal::tagged` is an anchor the diff aligns
  around within one owner. Restore aligns per owner as the live side is created, and
  every mismatch defaults, so a stale snapshot reaching an edited program is the
  ordinary case and never fails. A node takes a `Position` from the same sequence when
  the scene creates it, and a platform carries what a node holds outside the graph—focus,
  a text control's selection or caret, scroll offsets—through `provide`/`staged` by that
  position, so a rebuilt page resumes on the same node through the same alignment,
  keys included.
- **Tasks belong to their owners.** `set_spawner` installs the thread's executor,
  which hands back a guard that cancels a task when dropped; `spawn` files it with the
  current owner, so a task ends when it finishes, when its owner is disposed or runs
  again, or through the `TaskHandle` it returns, and is never polled again once it
  ends. A load ends with the view that asked for it on every platform, and a reload
  cancels the load it overtakes. The executors run on `async-task`, a task queued once
  however often it is woken; the host one tells a notifier of each wake, which the
  terminal wires to its wake pipe, so a task woken from another thread runs at once. A
  debug build panics on entering an owner already disposed.

## Motion

`Transition` and keyframe `Animation` are data. The web compiles them to CSS; every
platform running motion on its own clock runs it through one runner,
`skene_core::interpolate::Motion`: the terminal on its frame clock (quantized to cells and
truecolor), the headless platform under a clock a test advances, native later. A
platform hands the runner each property from the resolution point that reports a
changed value—never through the reactive graph per frame—and it interpolates per value
type, starting, reversing and cancelling transitions as CSS does: a change back to where
a running transition began reverses it over the share it had played. A property a
keyframe animation leaves out at `0%` or `100%` moves from and to the value beneath it,
as CSS's neutral keyframes do. Colors mix in Oklab, as CSS Color 4 specifies for the
`oklch()` values the web writes and Chromium does; Firefox still mixes them in sRGB.
An aspect ratio moves through its logarithm, as CSS's `<ratio>` does, and one of zero or
infinity is discrete. Every change source animates: a condition flipping, a token redefined, an expression
input moving, a derived write. Reduced motion is respected by default. Keyframes may
hold tokens. `set_transition_name` maps to `view-transition-name`; the router wraps a
same-document navigation in `startViewTransition` when the app opts in
(`view_transitions::enable`), the stylesheet emits `@view-transition` for cross-document
navigations, and both degrade to a cut. A native platform will snapshot the outgoing tree
into a layer and interpolate named pairs.

## Platforms

### Web (`skene-web`)

Two leaves over one node record, selected by the build and enforced by it: the browser
leaf compiles on `wasm32-unknown-unknown` alone and the server leaf where the build names
the server platform (`--cfg skene_platform="server"`, on any host), and the `WebDom`
escape trait exists on the browser leaf alone, so the server leaf *cannot* touch the DOM:

- **csr** (`skene_leaf = "browser"`)—the browser. DOM sink, the stylesheet sink,
  root-level event delegation with a stamped node id (one listener per event type,
  `scroll` and enter/leave included), one shared `ResizeObserver`, hydration
  (creation-order `data-h` markers, loud mismatch, the server's stylesheet adopted rather
  than duplicated, a live input value that differs from the serialized one written into
  its signal), scroll and focus restoration keyed by a segment's end anchor, the
  `WebDom` escape trait, the History binding and the head.
- **ssr** (`skene_leaf = "server"`)—the serializer. Renders its string tree to HTML plus
  the interned stylesheet; drives async to a fixpoint on the host executor, parked until
  a waker fires, before serializing, under a render deadline; streams in document order around `Deferred`
  boundaries with a style delta per chunk; serializes resource state for the client to
  adopt; takes client hints as render input. The raw handles do not exist here, painted
  content renders empty, and handlers are inert.

The app compiles to wasm for the browser and natively for the server: the server binary
renders as a plain command and, as a render worker over its stdin, serves the CLI's dev
server and generator. The two builds must agree on every byte they both produce, since
hydration adopts the server's class names: a `Color` converts through the pure-Rust
`libm` on every target rather than the host's math library, which differs by an ulp in
`powf`, `cbrt` and `atan2`, and a probe over thousands of colors rendered on the host
and on wasm proved the bits equal.

### Headless (`skene-headless`)

The host platform: every `Platform` method mirrors into a record store, nothing lays out and
no platform response fires. Beside the records it keeps what a test asks: the outline,
the props a node was created with, the slot a render effect last wrote, a property
resolved through core's `Resolver` under the environment a test set (viewport, scheme,
reduced motion, container sizes, hover and focus), control state and
parts, overlay openness, focus order, accessible names, bound painters. Events go through
`trigger_*` into core's dispatch; a frame clock advances on request; an executor holds
every spawned task until `pump_tasks`. It is what `skene-gui` selects on a host target,
so an app's `#[main]` compiles everywhere.

### Native (`skene-native`, planned)

One family; the design premise is that **the platform composites**. Where the OS has a
real view hierarchy (iOS, Android, macOS), the platform realizes the tree *into* it: taffy
computes frames, painted content renders through vello into layers that are themselves
views in that hierarchy, and an embed is an ordinary native view. Sibling order is
z-order, so custom-painted content and real platform controls interleave for free. The
platform decides its own realization granularity internally; the invariant is only that
tree order maps to compositor order. The resolution cache tells it whether a
layout-affecting value moved, so a palette write restyles taffy only when it must.

The Linux desktop leaf has no native control set to embed, so it is the plain
winit + single-vello-surface case; Windows sits between and its leaf chooses. Text is
parley; accessibility is AccessKit; per-OS glue (IME, clipboard, menus) lives in each
leaf. Off-thread work uses the engine's scheduler wired to the event-loop proxy, and a
loop-integrated timer is fair game here (the loop is ours).

### TUI (`skene-tui`)

A facade in its own right, not a platform of the GUI vocabulary. Its views are the shared
ones typed to the terminal: lengths are `u32` cells, text is a color plus a `bool` bold,
a border is one cell of box-drawing characters on each edge its width names, in its own
color, its conditions are cells and
its own background. Nothing the medium lacks exists to call, so a terminal app is
first-class at compile time rather than best-effort at run time.

The mouse is in-capability: motion drives `Hovered` variants and pointer handlers, a
left click hits the topmost node and runs its handlers through core's dispatch, a
hovered node's `cursor` sets the pointer shape via OSC 22, with the shapes the GUI's
pointer takes implied where none is set (the I-beam over selectable text and a text
entry, the hand over a control or a link, the open hand over a slider and the closed
one through a slider, scrollbar or caret drag), written once per change after the
frame so that what changes under a still pointer changes the shape, and handed back to
the terminal on exit—terminals without mouse reporting or pointer shapes degrade
quietly. A pointer position is its cell's center
unless the terminal reports pixels: where the platform reports the window's pixel size,
the loop asks for SGR-Pixels (DECSET 1016) and queries the mode (a DECRPM reply the
loop's parser reads for any mode), and from the reply that confirms it
every report is pixels over the cell size, so a pointer sits within its cell and a
slider or scrollbar drag lands where the pointer is; the request repeats on resize, and
hit-testing stays by cell. The window's size is event-driven: the platform's at launch,
then every resize notification's, and the loop sets in-band resize (mode 2048) and
queries it, so a terminal that confirms it reports every resize in the input stream
(`CSI 48 ; rows ; cols ; height ; width t`) with the pixel size the pixel mouse needs where the platform's own
notification carries none, and a notification without pixels keeps the known ones
while the mode is confirmed. Launch queries synchronized output (DECRQM 2026) the same
way, and from a reply that says the mode can be toggled every frame is written between
`CSI ? 2026 h` and `l`, so a redraw never tears; a terminal that does not know the mode
gets plain frames. Grapheme clustering (mode 2027) is set and queried at launch the
same way: text is measured in extended grapheme clusters (`unicode-segmentation`),
each as wide as the terminal draws it (`unicode-width`)—the cluster's width where the
terminal confirmed the mode, the sum of its code points' widths where it did not, so
the layout's measure agrees with the terminal's either way—and a cell holds a cluster
(`Glyph`: one code point, an interned cluster, or the continuation of a wide one, which
paints nothing) while a field's caret, window and selection index clusters, the window
of a single-line field settling by cells. A confirmation arriving after the first
frame measures every entry again and lays the frame out afresh. Text sizing (kitty's
OSC 66) is the same kind of enhancement: the terminal's `Typography` has a `size`
token, `1.` by default, and `set_font_size` on a run is the shared type metric in
the medium's unit, so `2.` is a heading two cells tall. A size rounds to a whole
scale of one to seven, and a size
below one is a fraction of a cell (`n/d`, `d` at most fifteen) drawn at the cell's
foot. A scaled cluster takes a block of `scale` rows by `scale` times its columns:
wrapping counts the scaled columns, a line is as tall as its tallest cluster and the
smaller clusters sit on its bottom rows as on a baseline, a run measures to its
lines' widths and the sum of their heights, and a hit at a cell finds the cluster
whose block holds it, so selection, links and inline boxes follow. In the cell
buffer the block's top-left cell holds the glyph and its scale and the rest are
continuations, and the writer emits the top-left cell as
`OSC 66 ; s=<scale> ; <cluster> ST` (`n=;d=;v=1` for a fraction), skips the cells
the terminal's own advance covers, and moves the cursor past a block's lower rows
before the next glyph. Launch probes it once the queries are written: the cursor at
home, a space at scale two, and a cursor position report (`CSI 6 n`); a reply at the
third column proves the protocol and re-lays the frame out with every size honored,
while a reply at the first, or none, leaves every run at one cell, so a size costs
nothing where the terminal cannot draw it. The loop handles every event already queued before it paints
the next frame, and a run of pointer motion collapses to where it ends, as a browser
coalesces `pointermove`, so a fast drag never falls behind the pointer. Selection is anchored in source space so it
survives reflow; wheel gestures latch their target. A text entry's selection is its
own—an anchor beside the caret over the value, reported to the app as a
`TextSelection` in clusters after every move and written from its bound signal, each
index clamped into the value—so keyboard focus selects a field's
value, `Shift` with the caret keys and the mouse extend it (a drag held past the edge
scrolling the window on the frame clock, as a browser's does), an edit replaces it,
`Ctrl-Z` and `Ctrl-Shift-Z`/`Ctrl-Y` undo and redo through a bounded ring of (value,
caret, selection) states per entry, as an `<input>` keeps (a run of typed or of removed
characters is one step, a caret move ends a run, and a value written from the signal is
a step of its own, the initial one aside), and
`Ctrl-C`/`X`/`V` and the standard context menu copy, cut and paste through the loop's OSC 52
write, the terminal's bracketed paste, and an OSC 52 read fired on demand. OSC 52 has no
capability query, so which of those exist is the app's launch choice: `Launch`'s
`Clipboard` names the operations offered (all by default; without `Copy` nothing in the
document is selectable and the standard menu lists no `Copy`)—unless the terminal's
clipboard answers: launch queries mode 5522 (kitty's clipboard protocol) with the other
DECRQMs, and a terminal that knows it has every operation offered from that reply, a
copy written as an OSC 5522 write in plain-text chunks, and a paste read as an OSC
5522 read whose answer arrives as `OK`, `DATA` chunks and `DONE`, pasted whole once
done and not at all when refused (`EPERM`, `EBUSY`), which OSC 52 could never
say—`set_mouse(false)` leaves the mouse to
the terminal, and the mouse key (`Alt-M`, or what `set_mouse_key` names) hands it over
at run time: the hover path empties, the loop resets the mouse modes and the pixel
reports, and the next key—the mouse key or any other, which then acts as usual—takes
the mouse back and asks for them afresh. The terminal's own selection—a drag with `Shift` held, `Option` on
iTerm2—bypasses the app on every terminal that has it, so a terminal without OSC 52
still copies; with the mouse off a plain drag does. `Ctrl-Q` is the one unconditional
key. Focus belongs to controls, which are composed trees with roles
and focus order, exactly as on the GUI, and to link runs, which sit in the tab order as
an anchor does: `Enter` is a focused link's click, staged for the system handler unless
a handler prevented it, a press on its cell focuses it, and while its focus shows it
paints in the typography's focused-link ink and surface, reverse video where those are
transparent, as the default leaves them. Focus shows as `:focus-visible` does: after a
key, or in a text entry, and not since a pointer press. A key's default runs after the handlers' writes
have taken effect, as a browser's does after its microtask checkpoint. A key release
dispatches as `KeyUp` (`handle_key_up` on a box and a control) with no default, a held
key repeats its `KeyDown` with `repeat` set, and a modifier pressed alone arrives under
its W3C name (`Shift`, `Control`, `Alt`, `Meta`) without showing focus; the terminal
asks the kitty keyboard protocol for `REPORT_EVENT_TYPES`, `REPORT_ALTERNATE_KEYS` and
`REPORT_ALL_KEYS_AS_ESCAPE_CODES` beside the disambiguated codes, and a terminal
without them reports presses alone, as the legacy encoding does. Beside `key`, a
`KeyEvent` carries `code`, the physical key under the W3C names (`KeyA`, `Digit1`,
`ShiftLeft`, `Numpad7`), so a shortcut on it holds across keyboard layouts: the
browser's `KeyboardEvent.code`, in the terminal the kitty protocol's base-layout
alternate where it reports one and the key's own position on the US layout otherwise
(`KeyEvent::code_of`, which the headless platform's key specs use too), and a shifted
alternate the terminal applied restores `Shift` for any key, so `Shift-1` reaches an
app as `!` with `Shift` held. A modal takes
focus to its first tab stop as it opens and returns it to the node that held focus as
it closes, a popup closing with focus inside returns it the same way, and a press on
the anchor a popup opened from is the anchor's click rather than a dismissal. An editor is a
scroll target like a box: the wheel, its scrollbar and the page keys move its window
over its wrapped rows without moving the caret, a caret scrolled out of view hides until
focus or an edit reveals it, and the horizontal wheel moves a field's window along its
value. A release ends a drag on the node the press captured wherever the pointer is. A
childless, borderless box paints to eighth cells from its parent's origin, unrounded:
whole cells take its color, and a cell one of its edges crosses takes a block glyph
carrying its color on the box's side and the color already there on the other, so a
slider's fill and thumb move by eighths, meet where the layout put them whatever the
parent's own offset, and a seam between siblings is exact.

The terminal itself is spoken to through three modules of `skene-tui`, with no
terminal crate beneath them: `tty` opens the device (standard input and output, or
`/dev/tty` where those are not terminals), puts it in raw mode with `rustix`, polls
it with a timeout, reads what arrived and restores its mode on drop and in a panic
hook; the window's size is read by `ioctl` after every wait and a change is a resize
event, beside the in-band notification; `parse` decodes the bytes incrementally into
the loop's own vocabulary—keys in the legacy encodings and the kitty protocol with
its event kinds, alternate keys and lock state, SGR and X10 mouse reports as
`PointerButton`s, bracketed pastes, and exactly the replies the loop asks for
(DECRPM for any mode, the theme report, the cursor position, the keyboard flags, the
in-band resize, OSC 11 and 52, DECRPSS with its SGR list in either separator)—and
drops any other string or CSI whole rather than reading it as keys; `escape` writes
the sequences the loop emits through `Display`, so a frame allocates nothing. Every
enhancement that needs a new reply adds it to `parse` in the same change. Windows
has no console layer yet; it is the tty module's to grow.

Pictures come through the kitty graphics protocol. Launch sends the protocol's query
(`a=q`, one pixel, an id of its own) among the other queries; a terminal that answers
`OK` (an APC reply the parser reads) proves it, `Painting::available` turns true, and
every `Painted` whose fallback stood rebuilds into its painted box. A painted box's
painter is kept on its record; before a frame paints, every painted box whose image
is missing or of another size than its content in pixels—its cells times the cell's
pixel size, the window's where the terminal reports it and ten by twenty else—runs
its painter on a `Canvas` of that size and keeps the RGBA pixels under the next
generation. The paint pass then places each picture over the visible part of its
box: the cells in view under the clip, with the source rectangle cropped to them, so
a picture scrolled half away shows its lower half, and a picture an open overlay
covers in part is left out, since the terminal draws images over the overlay's
cells. The writer transmits an image when its generation moved on (`a=t`, RGBA,
direct, in chunks, replies suppressed), places it where its cells changed or the
frame is written whole (`a=p` at the cursor with `c`, `r` and the crop, the cursor
kept), frees it when no picture shows it (`a=d,d=I`), and frees every image at
exit. Shared memory and file transmission, unicode placeholders through tmux, sixel
and iTerm2's protocol are on the roadmap.

The terminal's own colors are read back too. Launch queries the foreground (OSC 10),
the background (OSC 11, which also decides the scheme) and the sixteen base colors
(OSC 4, one query for all), and each reply lands as it comes: at sixteen colors a
painted RGB is written as the nearest base color the terminal reported rather than
the nearest xterm default, and a paint that mixes into an unset cell—a shadow, a
scrollbar's track—mixes against the reported foreground and background where the
scheme in force is the detected one, against white on black or black on white where
the app chose the other scheme, so the contrast is the terminal's real one.

Decoration is painted in pixels beneath the text where the terminal shows graphics. A
box whose decoration asks for more than cells draw—a corner radius, a border, a
shadow—gets a **backdrop**: an image of its cells and the shadow's band at the cell's
pixels, the shadow as three softening layers offset half a cell, the background a
rounded rectangle at the radius in rows, the border a line a quarter cell wide inside
each edge it is drawn on, rendered through the same `Canvas` a painted box uses and afresh only when
the size, the cell's pixels or a part of the decoration changes. It is placed on the
layer beneath every colored cell (`z` below the protocol's threshold, the box's depth
added so a nested backdrop draws over its parent's), and the box's own cells are left
uncolored and unbordered, so the image shows through them while a colored cell
inside—a plain box, a selection, a caret—still paints over it and every glyph stays a
glyph, so selection, copy, the caret, the font and accessibility are untouched.
Layout stays in cells: the border still takes its cell, drawn thin inside it. The
shape was measured on the gallery at 100 by 40 cells: seven backdrops of 452,400
pixels in all, the largest 201,400, against 800,000 for one image of the frame; a
change to one box re-sends that box alone, where a frame image would re-send
everything on any change, so the backdrop is per box.

Layout is taffy at `1.0 = 1 cell`, with the effective style cached per node and
`set_style` called only on change, so a keystroke relays out the changed subtree and a
hover change repaints without relayout. A frame paints over the previous one, rewriting
only the rows the changed nodes covered and cover—a node whose resolution, text, caret,
scroll or rectangle changed—and every row for a whole-frame change; the terminal loop
then diffs rows again on output. A paragraph lays its inline runs out as one wrapped
text, and a view inside it flows as an inline box: laid out on its own, standing in the
text as a placeholder of its width, painted and hit at the cells it flowed into. An
edge whose border width is zero takes no cells and draws none, and a border whose color
is unset draws in the terminal's own ink, as CSS's `currentColor` does. Frames paint into a cell buffer—to the terminal
through `launch`, or to plain text
through `render_to_text` for headless tests (`trigger_key`/`trigger_mouse_move`/
`trigger_mouse_click`/`trigger_frame` drive it without a terminal). Terminal detection is
asynchronous; the loop starts immediately and the answers arrive as signal writes. The
dev loop carries the snapshot across a swap through the launcher's control pipe. Terminal
apps never cross-compile into the GUI family; the write-once promise belongs to
web+native.

## Third parties and platforms

Two guarantees, both properties of what is public rather than of any one API.

- **A component library, for every platform, from one crate.** It depends on
  `skene-common`, writes generic over `P: Platform` exactly as the built-ins are written, and
  states what it needs from a medium as a bound on the platform's associated types
  (`P::StyleExt: CornerRadius`). Each facade's aliases make it concrete for the app
  that uses it. Nothing on the path a built-in takes is crate-private: the setter
  macros the built-ins generate their `set_`/`derive_` pairs with (`properties!`,
  `decoration!`, `type_metrics!`) are exported, so a third-party layout view built on
  `LayoutBox` spells its surface the way `Row` does.
  `examples/third-party/acme-ui` is the standing proof: a palette and a badge, used by
  both galleries.
- **A low-level view, against the platform, like a built-in.** It is written against a
  platform's escape trait (`impl<P: Platform + WebDom> View for Map<P>`) and realized through
  the same scene and node ids as a `Container`, so it composes with wrappers, themes,
  conditions and events. Selecting the platform is what selecting the facade already does.
  `acme-map` (a canvas on the browser leaf, a placeholder through `WebMarkup` on the
  server) and `acme-sparkline` (bars through `CellPainter`) are the standing proofs. The
  portable low-level path for pixel media is `Painted`.

A platform need not be first-party: a crate that implements `Platform`, provides its scalar,
leaves, extension, a `launch` and aliases over `skene-common` is a facade. `Platform` is not
sealed, and the shared machinery bounds only on it and the public extension traits; the
headless platform is exactly such a crate in all but ownership.

## Selection and entry

Selection is by **facade dependency** first—a GUI app depends on `skene-gui`, a
terminal app on `skene-tui`—and by target within a facade: `skene-gui` resolves
`wasm32-unknown-unknown` to the web's browser leaf, a build naming the server platform
(`--cfg skene_platform="server"`) to its server leaf on any host, every other host to the
headless platform, and will resolve native triples to the native platform, with no
feature flag anywhere. Where one triple hosts several platforms the build names the
platform by a cfg, chosen once and unifying nothing, as `getrandom` names its backend.
Target-gating stays the enforcement layer where it applies. A cargo feature may only add items; never a bound, a
type, a variant or a platform; the workspace has none.

A facade is the one place allowed to name its platforms, which is why the shared
platform-adjacent surface lives there: `launch`, `#[main]` (per-platform entry glue),
`spawn`, `Deferred`, `client_only`, `media`, and the interactive-only wrappers. App code
names no platform.

## Async, data, and third parties

HTTP, WebSockets and timers are **userland**, deliberately (the browser owns the loop on
the web; owning an HTTP stack is permanent debt). The framework ships only the glue no
external crate can provide, and ships it as *public seams* so third-party integrations
sit on exactly the footing first-party ones would:

- `spawn`—resume on the UI thread, scope-bound cancellation.
- `Platform::schedule`—a one-shot action on the platform's clock, cancelled by the `Timer`
  it returns: the window's timeout on the web, the loop's wait in the terminal, the
  hand-stepped clock under the headless platform, and never on the server.
- `Resource`—in the engine: load into a signal with a `Loading`/`Ready`/`Refetching`
  state that keeps the last value; the GUI facade's newtype adds hydration across the
  SSR boundary (`new` by order, `tagged` by tag, `client` opting out).
- The server leaf's executor—`sleep`, and a render deadline (`set_render_deadline`) that
  bounds every render's settling.
- `Boundary`/`Deferred`—any async inside a boundary defers its streamed chunk.

`skene-transport` is the standing proof that the seams suffice: an HTTP client (`http`) in a crate
of its own that nothing in the framework knows about. A `Request` is a method, a URL,
headers and a body; `send` is a future resolving to a `Response` with its status, its
headers and its whole body, any status a response and only a failure on the way an
error. The transport is the platform's, chosen by target: the browser's `fetch` on
the web, and on every other host, a server render included, a thread around a plain
client (`ureq`) whose completion wakes the platform's executor through the future's
waker, so a terminal app's loop, a headless test's pump or a parked server render polls
it on. Dropping the future
abandons the exchange. An app drives it through the seams it already has:
`Resource::client` for data a view shows, `spawn` for a task of its own. `skene_transport::ws`
is the WebSocket sibling on the same seams: `connect` is a future resolving to a
`WebSocket` once the connection stands, `send` writes a message at once, and
`receive` is a future a task awaits in a loop, each message as it lands and `None`
once the socket has closed; the browser's `WebSocket` on the web and a thread around
`tungstenite` on every other host reading into the same receiver, a server render
included, so a page whose data arrives over a socket renders with it. A socket opened
during a render lives for that render, as a fetch does. Dropping the socket closes it.

## Router (`skene_common::routing`)

Routing is state: a typed route as a signal, shared by every platform. Each platform binds
it thinly—web csr to the History API (wrapping a navigation in a view transition when
the app opts in, through `Router::set_commit`), ssr takes the route as render input,
the terminal keeps it in memory. SSG enumerates the route table (`StaticRoutes`) to know
what to render. The router depends on the engine alone.

## SSR, SSG, and the server binary

Server-side rendering is a *capability*, not a server: `render_to_string` /
`render_to_stream` produce the parts (styles, body, state) or ordered chunks, and
`Rendered::page` assembles a document. One template, `skene_web::Document`, writes every
page, the CLI's client-rendered shell among them: the head preloads the client bundle,
the body ends in the hydration state and the module script booting the bundle, a page
in the dev loop carries the dev marker and the dev script, and every inline script
carries the render's nonce. A head registration (`head::set_title`, `add_meta` and
their kin) holds while the owner that made it lives, in one registry the web leaves
and the headless platform share, `skene_common::gui::Head`: the latest live title is in
force, and so is the latest live element of what it stands for, so a route's head
leaves with the route; the server serializes what is in force, and the browser applies
it to the live head, adopting the elements a server render left. The app's server binary is built natively with the
server platform named (`skene build web --static` and `skene run web --ssr` do it, for
the host target alone so build scripts and proc macros never see the cfg; by hand,
`RUSTFLAGS='--cfg skene_platform="server"' cargo build --bin app-server`), and
`#[main]` makes it a plain command booting its own crate's bundle: `app-server /path`
prints the page, `--routes` the enumerable routes, `--stream /path` the page as it
flushes, and `--serve` renders one length-framed request after another read from
stdin—the path, then the hint headers and the nonce—so a user's server runs it as a
command or embeds `render_to_string` directly, hands each request's headers to the
render as `RenderHints` and names the response headers that request them. A render occupies the thread it runs on, since the runtime and the scene are
that thread's, and starts from a clean one every time. SSG is the same binary run at build
time by `skene build web --static`; the CLI's dev server (`skene run web`, `--ssr` to
render per request through a `--serve` worker) is where the development conveniences
live. The terminal splits the same way: `skene run tui` keeps the build loop and its
logs in the invoking terminal and publishes each built binary and each failure on a
socket in a directory only its user may enter, and `skene attach <id>` launches the app
in a second terminal and forwards both over a pipe the app inherits.

**A dev reload snapshots once, at the swap, under a cooperative stop.** Nothing
checkpoints per change and no server holds state: the terminal app hears `swap <path>`
on its launcher's pipe, records its snapshot to the unnamed file whose descriptor the
launcher passed in `SKENE_DEV_STATE`, restores the terminal and becomes the new binary
in place through `execve`, so the process, the terminal, the file and the pipe carry
over and the successor restores the file at boot; where the process cannot be replaced it exits with status
75 and the launcher spawns the binary. The page keeps its snapshot in the origin
private file system when the dev server says reload, awaiting the write before it
reloads, with session storage as the best effort on a reload the reader made, and
takes it back at boot through the dev script's `skene_state()`. A rebuild failure
reaches the app as `notice <text>` on the same pipe. Static output is fully functional without script (styling, media and container
queries, hover, themes, transitions, animations—all CSS); hydration upgrades it to
interactive when the wasm loads, and `#[lazy]` bodies split into modules fetched on
first use while the server leaf renders them inline. The CLI owns the loader glue: a
load in flight is shared by every caller, each caller hears once whether it succeeded,
and a failed load is asked for again after a wait that doubles to thirty seconds, the
boundary's fallback standing until the module arrives.

## Logging

Framework and app code log through `tracing`. Each platform installs its subscriber at
launch. The spine carries no subscriber.

## Superseded decisions

Recorded because they were load-bearing and the reasons they fell matter:

- **Per-platform view realization** fell to the orphan rule the moment platforms became
  separate crates, and to the duplication it forced. The typed vocabulary replaces it.
- **"No framework style model"** existed so no platform would be forced onto a foreign
  vocabulary. Taffy-everywhere plus no-JS responsive SSG *is* a shared vocabulary by
  requirement; the model is framework-defined, and a medium extends it through its
  typed extension.
- **"Selection is the target, only"** cannot distinguish desktop from TUI on one triple.
  Target-gating is demoted from selector to enforcement.
- **A `Backend` trait object** cost `unsafe`, `Box::leak`, and a fallible downcast. Still
  rejected: platforms are statically selected and the platform parameter monomorphizes.
- **"The built-in views live in the umbrella."** True with one facade; with two it
  produced two umbrellas and two copies, sixty-five percent of them character-identical.
  The generic parameter already used for `Children` is applied to every view in
  `skene-common`.
- **"No persistent scene tree in core."** The objection stood for a tree that calls
  platforms. The scene calls none; every platform kept the structure anyway, four ways.
- **"The vocabulary is free functions resolved by name."** A missing function was only an
  error on a target-gated path, which let the host platform lack a third of it. A
  zero-sized trait makes the contract complete.
- **A recording platform (`skene-record`) beside the real ones.** It ran only the target's
  handlers, did not unlink on reparent, and could not be complete because nothing forced
  it to be. The headless platform implements the whole trait and is selected by the GUI
  facade on the host, so it cannot drift.
- **Group nodes for reactive subtrees.** A real `<div>` and a real taffy node made every
  derived subtree one flex item. Subtrees mount between anchors.
- **A style blob per control (`BoxStyle` handed to `set_style`), the `set_<prop>_when`
  families and a whole-style `set_when`.** A whole-style variant could set properties
  the view does not honor; a per-property setter exists only where the property does,
  and a condition rides the value.
- **`Adaptive<T>`: a token's conditions landing beside the property as ordinary
  variants.** They were copied into every use site, doubled by the scheme guard, and a
  palette could change only by a rebuild that lost state. A token is a named inherited
  slot; its conditions live in its definition; a theme redefines it by one effect.
- **`SimpleTheme::adaptive(light, dark)`, a light/dark palette pair, and a whole-palette
  `when(condition, overlay)`.** Each made light and dark the one condition with machinery
  of its own; the per-property form removes the separation, and the overlay is the same
  privileging one level down. `Media::Dark` in a definition is the whole mechanism.
- **`theme_<prop>` setters taking `Fn(&Palette) -> Color`.** They welded the framework's
  box view to one library's palette type. A token is a value the ordinary setters accept.
- **`TextTheme`, `ControlTheme`, `SimpleTheme`.** Three inheritance mechanisms for one
  need; `Typography`, `SimpleGlyphs` and `SimplePalette` are token sets under the one
  `Theme`.
- **A `derived`-based `derive_scheme`.** A rebuild disposes the subtree's scope with its
  state—measured: a signal inside went back to its initial value. The platform is told the
  scheme and one write re-inks the page.
- **A scheme override scoped to a subtree.** The choice half scopes for free; the guard
  half (the preference rule applying only where no ancestor chose) has no scriptless
  answer Firefox supports. An app has one scheme, the way the platform does.
- **A handler-side override (`Override`/`ControlFlow`) letting a user take an event from
  a control.** No first-party code wanted it and it taxed every ordinary handler, because
  the orphan rule blocks `From<()>` for a foreign type. A control decides its verdict.
- **`Enhanced`: a native `Web*` control rendered without script and swapped for a drawn
  `Raw*` after hydration.** The scriptless form is the control: a text entry is its
  native element and a toggle or slider keeps its input inside the label. A view that
  genuinely has two forms declares them through `Enhance`, the picker included.
- **A derived value as an inline override.** Inline outranks every variant on the web
  and not in the terminal. A derived leaf is a per-node custom-property slot the class
  references.
- **The `snapshot`, `test` and `simple-views` cargo features.** Each gated a trait bound,
  a type or a platform, which an additive feature cannot do soundly. Snapshot is on by
  default and opt-out, the test seams always compile, and the Simple views are a crate.
- **Transitional `next` namespaces** (`skene_core::next`, `skene_web::next`,
  `skene_simple::next`) carried the new model beside the old during the redesign and
  are flattened into their crate roots.
- **A WASI server leaf (`wasm32-wasip2`) under a wasmtime runner.** A render sandboxed per
  instance could reach nothing but `wasi:*`, so every client a page needed was a second
  transport, the runner a second host, and a crate a server reaches for was out of
  reach. The server leaf compiles natively, selected by a build-set cfg, every crate on
  the host reaches a render, and wasm hosting is a runner someone may write.
