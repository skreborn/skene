# Design

The architecture of skene: facts and the reasons behind the load-bearing decisions.
Conventions and invariants live in `AGENTS.md`; what is open and what is planned in
`ROADMAP.md`.

Skene is a **client-side** UI framework. An app's shell for each platform depends on that
platform's facade, over one foundation: the web (client-rendered, server-rendered, or
statically generated) and the terminal, with a shell per desktop and mobile OS to come. The reactive engine is signal-based and fine-grained; there is no virtual
DOM and no tree diffing. The framework ships no server-side features—no server functions,
no backend integrations. What it ships instead is a render the app's own server binary
performs as a plain command, so any server can serve a skene frontend without the
framework becoming a server framework.

The building block is a **view** (the `View` trait). "View" is the canonical term
throughout—there is no separate "widget" concept.

## Crates

What each crate holds is the README's crate table. They stack: `skene-reactive` at the
bottom, `skene-foundation` on it, the layout every platform but the web computes
(`skene-layout`) and the text engine over Parley (`skene-parley`) on the foundation, the
platforms (`skene-web`, `skene-terminal`) on the foundation and, where
they draw, on those two, the Simple core (its palette, variants and sizes, naming no
realization) on the foundation, each platform's Simple crate (its presets) on its
facade, and the macros, the transport and the CLI beside them.

**A facade per platform, one spine.** The web and the terminal are different media, not
one medium with fallbacks: an app is written for each on purpose, and neither
cross-compiles into the other. Each facade therefore owns its builder vocabulary outright, typed to
exactly its capabilities—and everything beneath it is written once in the spine. The
facade's own contribution is small by design: a scalar, the medium's condition leaves,
the properties only that medium has, the platform it selects, and a type of its own per
view, a newtype over the foundation's, that gives app code a flat, medium-typed surface.
The newtype derives `Wraps`, which gives it `Apply`: `.apply(option, Type::set_<prop>)`
sets a property only where a value is given. It derives `View` too, which builds it for
its facade's `Active` and no other: a derived view belongs to one facade, and its build
is a function the facade's crate compiles once, a view generic over the app's types
building through one that is not wherever its field names none of them, so an app's
rebuild compiles the app's code and not the built-ins'. A view whose field is generic in
the app's types is `Thin` over a core that is not, and its derive names the core
(`#[view(core = ControlCore<Active>)]`), building it through a function generic in
nothing: a control over any behavior builds a `ControlCore`, a theme of any palette a
`ThemeCore`, a switch over any key a `OneOfCore`, a link to any route a `LinkCore` and a
picker of any value a `BasicPickerCore`, each compiled once, in the facade, the app
compiling only the erasure into it. What a medium lacks is absent at compile time
because the platform's associated types say so.

Dependency arrows only point down; a platform depends on the foundation, never on another
platform; each facade names only its own platforms, and publishes the selected one as
`Active`. A crate reaching neither facade names no platform and builds in every build,
plain cargo at the root among them; one reaching a facade builds in that facade's
platforms' builds and compiles empty in the other's.

## The scene and the platform

**The spine owns the tree; a platform keeps a record per node it makes.**
`Scene<P: Realization>` is a generational arena of node records—parent, children as a
list of linked siblings (a move relinks a few records and shifts nothing), the token
definitions a theme placed on the node, the query-container name, the handlers; beside
them, the root's definitions every node falls back to, the platform's colors
(`SystemColors`) among them, which the realization defines there as it is installed. The scene mints every `NodeId` a node holds (`NodeId::new` builds one from its
index and generation, as the layout engine's tests key their canned trees, and one no
live node holds resolves to nothing in the scene), and the platform keys its own record of the
node by that id (a `NodeMap`: an element, a string-tree index, a taffy leaf), realizing
it once the scene is released and before the node takes part in anything else. Its
`create` answers whether it made a node, and the scene records the answer: the web
makes one of every kind, the terminal none for an anchor or a scope, whose content joins
its parent's layout in its place, and a node a low-level view mints counts as made. Every
structural mutation (`attach`, `detach`, `destroy`, reorder) is implemented once in the spine;
the platform is called only to mirror it into its records, after the spine has released its
own borrow. The scene also says where each node is placed (`Scene::placed_children`,
`Scene::placed_parent`): where it stands, except the caption a control keeps beside it
(`Scene::keep_caption_beside`), placed before the control among its siblings on every
platform. The terminal reads its structure there rather than keeping a copy, through
`Scene::read`, the shared borrow reads take, which nests and mirrors nothing, and the
server's string tree serializes the scene's structure through its records, a control's
carrier and its caption's `<label>` the only elements it adds; the browser's tree is the
DOM. The scene tells the platform what happens to each child, naming only the nodes it
made: the parent is the nearest made ancestor, `before` the next node made after the
child under it, a caption kept beside its control passed over, as the control stands for
it among its siblings (the web inserting before the caption's `<label>`), and a node not
made stands for the made nodes beneath it, each told in its place (the rule is
`spine::scene::placement`'s, over plain data). A child is attached (new, or returning
from being hidden), relocated (a move, which the web makes with `moveBefore` where the engine has it, keeping focus and animations,
and with one `insertBefore` elsewhere), hidden (taken out live, to return) or removed
(for good, destroyed next), and re-attaching a node where it already stands tells the
platform nothing. Only a hidden child keeps what leaving loses: the scene keeps the
focus inside it and gives it back on its return where focus has gone nowhere since, and
the web keeps the focus and the scroll offsets its elements lose with the document; a
removal keeps nothing, so clearing a list reads no layout. A hidden node is no root, so
a reader finds nothing inside it. `NodeId` carries a generation, so a stale id resolves to nothing rather than
to a recycled node, in the scene and in a platform's records alike. Token resolution and event dispatch walk this tree; nothing reads
structure back from a platform. The scene also records each node's place in focus
traversal and each overlay's state, the open ones in the order they opened, as a
browser's top layer stacks them, and computes the tab order from them
(`Scene::tab_order`): document order, the modal opened last containing it with the
overlays opened after it. A platform drawing its own
controls traverses by it, stepping from the focused node by `Scene::tab_from` (from a
node that is no stop, the nearest stop beside it in document order), as the terminal
does. The scene answers which overlays a press dismisses
(`Scene::dismissed_by_press`) for every platform: from the top down, each overlay the
press lands outside of, on neither it nor the anchor it opened from, until one it lands
inside of, a modal ending the descent and dismissed itself; the web leaves a modal's
backdrop to its click. It answers what a key its handlers leave to the platform does
(`Scene::key_default`: the shortcut of a control or an activatable row activates it
before any other default, `Tab` traverses, `Escape` dismisses the topmost overlay, which
the web leaves to a modal's own cancel, `Enter` activates the activatable row holding
focus on every press and `Space` arms it, which keeps the press from scrolling, and the
menu key or `Shift-F10` asks for a context menu) and what a release does
(`Scene::key_up_default`: the release of the `Space` that armed a row activates it
where it kept focus since the press), each activation a click dispatched at the row or
control (`dispatch_activation`) its handlers and their verdicts take as a pointer's.
The scene keeps the window's shortcuts, a registry from node to `Shortcut` a control or
a row notes as it is built (`Scene::note_shortcut`) and a destroyed node leaves: a key
pressing one activates the first node in document order holding it that is a control
not disabled or an activatable row, attached under the window's root outside every
closed overlay and every subtree the platform reports hidden, and that the topmost open
modal does not exclude, as the tab order reaches its stops; a shortcut holding no
modifier but `Shift` does not act while focus is in a text entry, whose key it is, none
acts within a composition, and a view-scoped binding (a `Keymap`'s, a
`MenuEntry`'s), which a handler runs as the key climbs, prevents the key inside its
view first. A row is a box acting on a click of its own (`BoxFacts::acting`: it
handles one and is no link), which the scene records as it creates the node, and a tab
stop unless its focus order says otherwise. It answers which popups focus leaves as it moves to another node
(`Scene::popups_left_by`, dismissed by `notify_focus_moved` on every platform), and where
a modal takes focus as it opens (`Scene::modal_focus`: its first tab stop, else itself,
as a dialog's focusing steps choose). The tab order is the scene's child order alone
(`Scene::tab_order`; a `FocusOrder` is `Auto` or `Skip`, never a position), and a window
opens with nothing focused but a control set to autofocus: the scene notes each request
(`Scene::note_autofocus`) and settles the opening focus once, as the mount that opens the
window ends (`Scene::take_autofocus`: the first node in document order that takes focus
for a request, no subtree the platform's style hides holding it and no open modal
blocking it, so a disabled one is passed over, its focus not showing; a node with no
place in focus traversal of its own, a collection, has the member holding its stop
take it), every later request ignored. The scene holds the interaction every platform reports: the
focused node with whether focus shows (a key but a modifier alone sets it, a pointer
press clears it, a text entry's shows however reached), the pressed node and the hovered
one, dropping each with its node, and resolves every node's interaction from them
(`Scene::interaction`), so every platform that draws resolves states alike; it decides
whether an entry takes edits (`Scene::is_editable`: neither disabled nor read-only) from
the control state and reports it holds, which the platforms read rather than copy.

**`Realization` is a zero-sized trait with associated types**, monomorphized, hidden behind
each facade's views: `Scalar` (`f32` or `i32`), `StyleExt` (the medium's own
properties), `TokenValue` (the erased definition the medium accepts, which converts
from the foundation's `TokenValue<Scalar>` by `TryFrom`, refusing a kind it holds none
of, and lists the tokens its definitions name through `TokenReferences`, so a palette's
default reaches it through a reference's link), `Native` (the
platform's own view the native door adopts), `Entry` (the editing state the scene keeps
for each text entry the platform draws), with the constants
`FONT_SIZE` (the size in force where no typography defines one) and `KEEP` (the `Keep`
level of a style its toolkit keeps unresolved). Its methods are the portable vocabulary
over node ids:

- **Create** every node, a box, a control, an overlay, a run, an anchor or a scope, from
  one `NodeSpec` (`create`: what fixes its element, its role, its heading level, its
  drag axes and its kind's facts, a fact of another kind unrepresentable: a box's layout
  and whether it is a link, a control's native element, lines and caption placement, an
  overlay's modality and backdrop, a run's placement and whether it is a link),
  answering whether it made a node. What can change once the node exists crosses as a
  content instead. The scene creates it through `Scene::realize` and records the answer.
- **Adopt** the platform's own view through the native door (`adopt`, from the node's
  `NodeSpec` and the view, answering as `create` does): an ordinary node from then on,
  placed, styled by the layout a wrapper gives it and removed as any other. `native`
  hands back the view realizing a node, `None` where the platform has none.
- **Name an entry's editing state** (`entry`, from the node and whether it is
  multiline): the shared entry (`text::editing::Entry`) over the platform's text
  engine, its text and paragraph style and the mark a secure entry draws, which the
  scene asks for as `Scene::realize` realizes a text entry, before `create`, keeps on the
  node and drops with it (after its borrow is released, as a handler is), handing it out
  shared (`Scene::entry`, `Scene::entry_of` where the scene may be borrowed) so an
  operation on it runs with the scene released. The spine names nothing of the text
  module, so the item is the entry itself rather than the engine: the terminal's is
  `Entry<CellEngine>`, and the web, whose entries are its form elements, edited by the
  browser, names the uninhabited `platform::web::Undrawn` and keeps the default `None`,
  as its door would with an uninhabited `Native`.
- **Style** each box, control, overlay and run once, right after its creation and before
  any token definition on it: `apply_style` hands it the node's record compiled at
  `Keep::Everything` (a style that sets nothing included; a low-level node receives the
  per-child layout a wrapper annotated it with, and only then). The web keeps everything
  and compiles the style as it arrives; a realization keeping nothing keeps it and
  resolves it at every restyle through the one `Keep::Nothing` entry,
  `ResolutionContext::resolve` (each keyframe through `resolve_frame`), which is `KEEP`'s
  one reader: it asserts at compile time that its realization keeps nothing. Each
  derived leaf's effect writes one erased leaf into its slot (`apply_slot`). Both are
  reached through the shared half alone (the foundation's `apply_style` and `bind_slot`),
  which first lays over the root the default of every palette a token of the style or
  the leaf links to, so the root's definitions reach `define_tokens` before the style
  naming them. A node's
  focus order and accessibility are not part of its spec: the shared half writes them
  once its style is applied (`update_focus_order`, `update_access`).
- **Define tokens** on a `TokenScope` (`define_tokens`, one call, a node's or the root's,
  the root a scope like any other), which reaches the realization for a scope it did not
  make as well.
- **Mirror structure** through four calls, `attach`, `relocate`, `hide` and `remove`,
  each naming the nearest made ancestor as the parent and the next node made after the
  child under it as `before`, a caption kept beside its control passed over, and
  `destroy`, releasing a made node's record.
- **Write contents** through one call, `write`, which reports nothing: what a node
  shows that a view writes is one typed value (`Content`: a run's `Text`, an entry's
  `Edit`, a toggle's `Checked`, a range's `Range`, a choice's `Choices`, a group's
  `Chosen` key, a member's `Membership` of its group, a link's `Destination`, an
  entry's `Placeholder`, its `Secure` flag and its `Keyboard`, an overlay's
  `Placement`), written through its key in `contents`
  (`contents::RANGE.write::<P>(node, &range)`), the key typed by its value so a value
  of another type does not compile, and erased into a borrowed
  `ContentValue` the realization dispatches on. A value constrained by another crosses
  with it whole, a range with its bounds and step, a choice's chosen key with its
  options, an entry's selection with its text, an overlay's pinned point with its
  placement (`OverlayPlacement`, a pinned overlay ignoring its placement), so a
  realization never shows half of a change. Each kind takes its own contents
  (`NodeKind::contents`), which the write asserts in a debug build. A link's first
  destination and an overlay's first placement are written between its `create` and its
  `apply_style`, so the web writes a link's `href` where it stood and compiles an
  overlay's margins against the viewport from its first placement. A box's
  `set_destination`, a run's `set_link` and an entry's `set_placeholder`, `set_secure`
  and `set_keyboard` each have their `derive_` twin. What an entry shows is read back
  through `read` (its `Edit`, where the platform keeps one), and `composing` says
  whether what a control shows is still the reader's, as while an input method composes
  or before the client adopted a changed control; a controlled content's controller
  reads both.
- **The rest**: register event kinds, focus, observe sizes, report media, schedule on
  the clock, route, and reach the clipboard.

A method with a default body is one a platform lacking what it reaches keeps (`native`,
`listen`, `read` and `composing`, the focus order and accessibility where the scene is
read for them, `focus_visible`, `lay_out`, `hydration`, the window title, routing, static
routes, the clipboard): the default does nothing, or answers that nothing is there. Every behavior is required,
each platform realizing it in its own form: a navigation commits under a view
transition in the browser and at once on the server leaf and in the terminal, and a
pointer event is localized against its node's laid-out box, except on the server leaf,
which lays nothing out and hands it back unchanged.

**The native door's seam** is the trait's own: `Realization::Native` is the platform's own
view (the browser leaf's `web_sys::Element`, the server leaf's `NativeMarkup`, the
placeholder it renders as markup), and a platform whose door adopts nothing names an
uninhabited type of its own (`Unadoptable` in the terminal), so its door cannot be
reached. `LowLevel::adopt` mints the node and has the
platform adopt the view as it, and `LowLevel::register_sink` registers the library's
`ControlSink` on it: `read`, `write` and `composing` (`false` by default) of the view's
values by the door's own names. A door's content is `Content::Door(name)`, keyed by
`ContentKey::door(name)` and typed like every key, its value crossing erased as a
`ContentValue::Door` (`DoorValue`, compared and downcast as the value it holds); the key
writes, reads and asks whether it composes through the node's sink rather than the
realization, so a `Controller` of a door's content holds the built-ins' contract (the
echo rule, the equal-write skip, the refusal's repair, the composition hold) with no
code of its own, and the library reports the reader's change through `report` with
whether the view shows it already, as a realization does. `NodeKind::contents` lists no
door's content, which a node takes through its sink alone.

`Painted` and a low-level view's payload are outside the trait—they never cross a crate
boundary—and arrive through extension traits (`Painting`; each platform's escape trait).
The leaf-only surface (`document`, `element`, `render_to_string`, `launch`) stays free
functions in the platform crate, since it is not a shared contract. One entry is
synchronous by contract: `lay_out::<P>()` ends the turn (`end_turn`) and lays the scene
out through `Realization::lay_out`, without painting, so a platform's text-input protocol,
which asks its questions synchronously, reads geometry that is current; the terminal
lays out through the settle loop its frame runs, and a platform whose layout is its
toolkit's keeps the default, which does nothing.

- **Why a trait and not name-resolved free functions.** A missing function is only an
  error on a path that names it, and those paths are gated on the build's platform; a
  trait makes a gap a hard error. That is what makes every platform complete: a build
  that compiles has every method a view can reach.
- **Why the scene is in the spine.** The scene *calls* nothing: the spine mutates it and then
  calls the platform through the generic parameter. Every platform needs the structure; one
  implementation is smaller and has one reparent semantics.
- **Dispatch is the spine's.** A platform reports an event at a node; the spine walks the
  parents the node had when the dispatch began, as the DOM fixes an event's path, runs
  handlers deepest first, folds `Prevented`, and tells the platform what the platform
  may still do. The browser's root delegation and the terminal's hit path share one
  bubbling implementation, so the rule that a view owns the
  interactions it handles is enforced, and testable, in one place. The path climbs
  through the start of every segment the node is mounted in before its parent, so a
  `Tooltip` or a `ContextMenu` around `derived`, `Visibility` or `Enhance` hears its content;
  an overlay anchored on a segment or a scope is placed against the first node inside
  it with a box (`Scene::placement`), and a tooltip describes the node holding focus
  within its view, moving the description when focus reaches a rebuilt one. A node carries one
  list of `Handler`s, each variant naming its event kind, added through `Scene::handle`;
  a control adds its behavior's handler for each event the behavior acts on before its
  observers, and gates both alike while disabled. A click on a native
  web toggle is dispatched *observed* (`dispatch_click_observed`): every handler on the
  path runs except the acting one the behavior registered, since the platform's own flip
  arrives as `change`, and the climb ends at the control, as the acting one's verdict
  ends it on every other platform. A focus move notifies each node whose subtree it
  enters or leaves (`notify_focus_moved`), as `:focus-within` changes, so a box hears
  focus reach any control inside it and never hears it move between them; a pointer
  move does the same for enter and leave (`notify_pointer_moved`) over the hovered node
  the scene holds, which every platform moves.
- **Reports are the spine's too.** A reader's change of a content a node controls
  (`report`, a `Report::Change` saying whether the toolkit applied it) is answered by
  the node's own controller alone, with no climb: one controller per content, a
  `Controller<T>` for a content held whole and an `EditController` for an entry's
  edit, holding the echo rule, the skip of an equal write, the composition hold and the
  disabled answer once for every platform, which reports and applies the verdict. A
  content held whole is repaired by its controller's own write of the truth, answered
  `Verdict::Refuse`, so a platform reporting a checked state, a range's value or a
  choice decides nothing.
- **Events are the W3C's, typed.** A key event carries `keyboard-types`' `Key` and
  `Code`, and a pointer event `ui-events`' pointer vocabulary beside the platform's own
  positions: the pointer (`PointerInfo`: its id, `PointerId::PRIMARY` for the primary
  pointer and none for an event no pointing device raised, and its `PointerType`), the
  button that changed, none for a move, the `PointerButtons` held once the event took
  effect, and the pressure, `0.5` while a button is held on a device reporting none. A
  platform that takes a pointer for itself—the browser starting a pan, the terminal's
  mouse key handing the mouse over mid-press—dispatches `PointerCancel` at the node the
  press reached, ending the press without a release. A wheel's turn dispatches `Wheel`
  at the node under the pointer before the platform scrolls by it, its delta per axis
  in the unit the device counts (`WheelUnit`: lengths, lines or pages; rows in the
  terminal), and a behavior that takes the turn keeps the scroll from happening; the web
  listens for it, as for every event, without `passive` and only once a node does. A
  click carries its modifiers and its place in a run of quick clicks at one spot
  (`ClickEvent`, `0` for a key's), and the second click of a double dispatches
  `DoubleClick` after itself, as `dblclick` follows `click`.
- **Drag and drop is the spine's too.** A box offers what a drag from it carries
  (`Container::set_drag_data`, a `DragData` of the app's value and any text another
  app may read, built as the drag starts), and the platform recognizes the gesture—the
  browser's own drag and drop, the terminal once a press's pointer leaves its cell,
  cancelling the press—while the spine carries the drag over the scene (`start_drag`,
  `move_drag`, `drop_drag`, `end_drag`): the nearest node at or above the pointer that
  takes what it carries (`Container::handle_drop`, taking a `T`) is its target, the
  source holding `Dragged` and the target `Targeted` while it lasts, and a drop runs
  the target's action. The web hands the browser the drag's text, and `Escape` abandons
  a terminal drag.
- **Statics batch, deriveds bind.** A builder's fixed properties, token references and
  expressions arrive together in the node's one style, its record compiled at
  `Keep::Everything` and handed over by `apply_style` right after the node is
  created—the web folds it into one content-addressed class—while each derived leaf
  binds a render effect that writes one leaf, erased over its property (`SlotLeaf`),
  into its per-node slot through `apply_slot`. The split is written once against the
  property visitor.
- **`Role` guides the platform's concrete choice** (web: `Role::Navigation` → `<nav>`); the
  vocabulary stays semantic, never platform-worded. It is AccessKit's, open to every
  role an accessibility layer knows, so a third party's switch, tab or tree is named as
  a shipped control is: the web realizes a landmark as its element and a heading at the
  `Level` it reports, and spells every other role ARIA has a word for on a generic
  element.
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
  otherwise, in a fresh canvas where a canvas's GPU path failed or the browser lost
  its 2D context; the server leaf renders it empty; the terminal
  renders it on the CPU and shows it through the kitty graphics protocol where the
  terminal proved one, its fallback standing elsewhere. The native layers sit behind
  the same `Canvas`. `Painted`, `Canvas` and `Painting` live in `skene-foundation`, since
  both medium cores realize them. Vello is the one renderer, everywhere.
- **Low-level views are platform-specific views**, written against a platform's escape
  trait (cells through `CellPainter`) or adopted through the native door
  (`LowLevel::adopt`; on the web `platform::web::NativeView`, an element built from its
  markup). `LowLevel::realize` mints a scene node the view realizes through its escape
  trait, and `LowLevel::adopt` one the platform realizes as the view itself, so either
  composes with wrappers, themes and events. A door's value crosses erased and is
  downcast by the door's own key alone.

  **Why a typed vocabulary and not per-platform realization.** The orphan rule: with
  views in one crate and platforms in others, `impl View for Container` can live in neither
  platform, so separate platform crates are impossible under per-platform realization. Typing
  the vocabulary moves the medium decision (CSS text, native styles) into the platform crate
  while realization is written once above it.

  **Real platforms, not a mock platform.** Because the vocabulary carries typed data, a
  real platform receives exactly what the views declare, so the foundation's views test
  where a platform realizes them, and no host platform stands in for one. The controls,
  the overlays and the reactive structure test on the platforms that realize them: their
  behaviors, names and keys, a modal's and a popup's dismissal, a tooltip's timing, a context menu's items, a
  lazy list's window, a boundary's, a switch's, a show's and an enhancement's swap, a
  wrapper's annotation and a key's restoration in the terminal's in-process tier, through
  real input, the hand-stepped clock and what a reader finds, and their web form (the
  native element, a reported edit's verdict, the hydrated swap, a popup's anchoring, a
  tooltip's `aria-describedby`, a childless view's absence from the DOM) in the browser;
  what a behavior declares it listens to is a unit test of its own. So do the styles: a
  transition's and a keyframe's motion, a token set's root defaults, a run's inks, a box's
  handlers, hover, tab order and drag in the cells the terminal paints, and the layout
  properties, a media condition against the window's width, a token border and a
  scheme-conditioned run in the styles the browser computes; a view's size on the stack is
  a compile-time assertion in each facade. So does the geometry: the layout engine's boxes
  and text flows (a paragraph's spans, its wrapping, an inline box, the guaranteed core)
  are `skene-layout`'s unit tests over canned trees and the real Parley engine, and what
  needs a live platform (a hit, a frame's measured size, a change resolving only what it
  touched, a text entry's native size) is the terminal's, in cells; the drawing is held by
  the pinned browser's goldens and the cells the terminal paints. So do the rest: a
  derived child, text and list, nested and derived themes, expressions over tokens and the
  environment, a container condition and a derived slot under a hover variant in the
  terminal's cells; the environment's answers in the media signals and conditions the
  browser reports through its own `matchMedia`, and in the terminal's pointer, which is
  the mouse the launch takes; and the rule that a platform's call runs with the scene
  released through the synchronous responses real platforms make (a scripted focus's
  events, a focused element moved or removed, a handler destroying the entry it edits). A
  facade's views test on their platform alone, against its real sinks: the web's in a
  browser DOM via `wasm-bindgen-test` and on the server leaf natively under a byte-exact
  snapshot; native ones against their toolkit. A component library tests each facade's
  crate on its platform the same way: Simple's in the browser through the styles the
  engine computes, and on the terminal through the cells it paints, where the pointer can
  rest on a control. The spine's algorithms are functions over
  plain data, unit-tested there with no scene: the tab order (`TabStop`), the open
  overlays a press dismisses and a modal keeps reachable (`OverlayStack`), the nearest
  definer of a token and its caches, the interaction held (`Held`), the keyed
  reconcile's plan (`KeyedItems`), the climb that folds a dispatch's verdicts, the
  subtrees a move crosses (`Crossing`), the arena's slots and an entry's accessibility
  node. What needs a live scene runs over the terminal's, its effect read where a reader
  and the frame find it: attaching, detaching, relocating and destroying nodes and
  segments (a property test holding the links whole and the cells and the reader's tree
  to the order the scene places after every change), a hidden subtree's kept focus, a
  caption kept beside, a segment's placement and focus holder, a derived subtree's anchors
  and annotation, and dispatch from real input (the fixed path, the climb through a
  segment's start, the pointer entering a segment, a handler's owner, a notification and
  a reported edit reaching the node's own handlers alone). No realization double stands
  in for a platform anywhere, the spine's own tests included.

## Composition

`View::build(self) -> NodeId` is the whole trait: a one-shot, side-effecting descent that
mints scene nodes, binds effects, attaches children and returns its root. There is no
update path on a view; updates are the effects a build left behind. This is the Solid
model, and it is kept because fine-grained signals make a diffing view tree unnecessary
and its cost unjustified.

- **Anchors, not groups.** A reactive subtree—`derived`, `Suspense`, `ErrorBoundary`,
  `Deferred`, `AsyncView`, `empty`—mounts between two anchors (a comment pair on the web,
  in the scene alone in the terminal, which makes no node for them) rather than inside a
  box of its own. Its nodes are therefore its parent's children for layout, and a wrapper around it annotates the
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
  and rebuilds, disposing the subtree's scope with its state; `Visibility` and `OneOf` build
  a branch the first time it shows and hide or attach it after, preserving its
  state, the focus inside it and its scroll offsets among it, and a branch standing
  apart pauses its effects (`Owner::pause`) until it shows again, catching up once. Each is a `Segment` of `Piece`s, every piece built under a
  scope of its own whose disposal destroys its nodes, as `Suspense` and
  `ErrorBoundary` are; those two keep their content live while it stands apart, since
  its work is what they wait on.
- **A view declares its baseline and its enhancement.** `Enhance` in `skene-foundation` mounts one of two builders between its anchors:
  `set_fallback_builder(|| baseline)` is the form that works before the environment is
  ready, and `set_child_builder(|| enhancement)` the form built once it is. Which is built
  follows a signal the enhancement is given (`Enhance::new(live)`), and the signals
  the built-ins wait on belong to the things that can lack them, never to a shared
  capability vocabulary. **Hydration** is the web's: `Realization::hydration()` returns
  the signal that turns true once a render made without script has been hydrated,
  and `None` by default, which every platform whose tree is live from the start keeps,
  the terminal and the native platforms among them, so nothing on the terminal facade
  can name it. The web facade reads it as `media::hydrated()`. The server leaf answers false forever, so a server render and a static page
  carry the baseline as markup; the browser leaf answers false while a hydration
  adopts the server's markup and flips to true once it has finished and the adopted
  live values have reached their signals, so the baseline is built once more on the
  client, adopted element by element with no mismatch, and then rebuilt into the
  enhancement in place, as a `derived` rebuild is, under a replacement owner, so a
  signal bound beneath keeps its value and its snapshot identity; a mount that
  hydrates nothing answers true from the start and never builds the baseline. An
  enhancement's two forms are tested on the terminal over a signal of the test's own,
  and over the hydration signal in the browser. **Pixels** are `Painting`'s:
  `Painting::available()` is the signal a painted box waits on, false on the server
  leaf, false on the browser leaf until it is live, and false in the terminal until the
  graphics probe is answered. Only the branch in force is ever built, so an enhancement
  may touch the browser and a baseline may be the whole view where the signal never
  turns. The seam is the
  built-ins' own: `Painted` takes `set_fallback_builder`, the view drawn where no pixels
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
  Taking it empties the slot, and the scene records nothing of it per node: the
  layout crosses in the child's style alone.
  A run's record carries it after the run's own properties, its derived leaves slots
  numbered after the run's own, and a low-level node hands it to its platform as a
  style holding that layout alone, through `Realization::apply_style`, which the web
  compiles into the element's class. A wrapper holds its child boxed,
  its setters before and after it alike; one without a child builds an empty segment,
  which takes no room and annotates nothing. Every other view around one child holds it
  the same way, boxed (`Overlay`, `Tooltip`, `ContextMenu`, `Visibility`, `Keymap`,
  `Link`) or as a builder closure (`Theme`, `Enhance`, `Suspense`, `ErrorBoundary`,
  `Deferred`, and `AsyncView`'s loader), and without it builds the same empty segment:
  a theme realizes no scope, an overlay no overlay, a boundary not even its fallback,
  and an enhancement keeps its fallback until it turns live, empty after. Children
  last is a convention, not a type: the gate's lint reports a property, a handler or
  an `apply` that follows a child setter in a chain, an applied child setter
  (`.apply(child, Container::set_child)`) counting as one.
- **A box that handles a click is an activatable row**, not a button:
  `Container::handle_click` makes the box a tab stop unless its focus order says
  otherwise, `Enter` and `Space` activate it as a click does through the scene's key
  defaults, and the controls it holds own their presses and their keys, a control's
  verdict ending the climb and focus on it keeping the row's key default out of reach.
  It carries no role but the one `set_role` gives it.
- **Owners** are created per reconciled item, per rebuild, per keyed view and where
  context is provided; everything else inherits the ambient owner. `derived` rebuilds
  into `Owner::replacement()`, the same parent and step, so snapshot identity survives
  a branch flip. A reconciled item's owner is `Owner::keyed_child` under the item's key,
  and `View::set_key` builds any view under one, each facade's `View` carrying it
  beside `build` and `into_any`, so state beneath either restores by key wherever it
  sits.

## Controls

A control is `Control<P, B: Behavior>`: one builder carrying focus, events, decoration,
label, parts and accessibility once, over a public `Behavior` that says what the
interaction *is*—its role, the native element it stands in where a platform has one
(`NativeElement`: a button, a checkbox, a radio, a range, a text entry or a picker; a
control with none is drawn by its view and named by its role), its keys, its pointer
handling, what it reports to assistive technology, where it sits in the tab order, and
which parts it exposes. `Behavior` has no type parameter: its hooks take a plain
`ControlHandle`, the control's node with its name, its disabled read and a
`ControlOps`, the operations of the build's one realization a behavior reaches (focus,
a click dispatched through the scene, a part's node, a walk of a subtree in tree order,
the platform's clock for `schedule` and `cancel`, a size observation and a pointer's
local position in `f32`, a length rounded to the realization's unit, and the
`ContentOps` beneath the controllers) as plain functions, filled once per build by
`ControlOps::of::<P>()` in the control's build, never a `dyn` realization; so a
third party's behavior is written exactly as the shipped ones are, naming no
realization. Each facade's `behavior` module (`skene_web::behavior`,
`skene_terminal::behavior`, parallel member for member) re-exports the shipped
behaviors (`Collection` and `Member` under a `CollectionPolicy` among them, with the
collections' contexts `CollectionContext`, `RadioGroupContext`, `ListboxContext` and
`MenuContext`, and the slider's `SliderContext`), `Behavior` with what its hooks name
beyond the views' vocabulary (`EventKind`, `LabelPlacement`, `PopupKind`),
`ControlHandle`, `ControlOps`, `Roster` and `TypeAhead`, so a library writes a behavior
through its facade alone and the root
leaves a behavior's name free for the drawn view of the same name; `Control`, the
`Basic*` views, `Choice`, `Chosen`, `TextSelection` and `Keyboard` with its kin stay at
the root, beside the views they are used with. A
`Control<P, B>` is `Thin` over a `ControlCore<P>`, the control with its behavior held as
an `Rc<dyn Behavior>`, and `ControlCore`'s build is the one control build, its
registration and every acting handler running over the erased behavior, so the build is
compiled once per realization whatever the behaviors an app writes.
`Toggle`, `Slider`, `Popup`, `Activate`, `TextEntry` and `Collection` (with `Member`)
ship; a collection's `CollectionPolicy` says what it and its members are, which keys
move between the members and what a move and a choice do, and `RadioGroup` (with
`Radio`), `Listbox` (with `ListboxOption`) and `Menu` (with `MenuOption`) are
collections under the radio, listbox and menu policies, whose members are one tab stop
and whose moves pass over the disabled members in one place; `BasicButton`,
`BasicCheckbox`, `BasicSlider`, `BasicRadio`, `BasicListboxOption`, `BasicTextField` and
`BasicTextEditor` are aliases over them, `BasicRadioGroup<T>` and `BasicListbox<C>` are
a facade's views of a control over a `RadioGroup` or a `Listbox`, typed by what it
chooses, and `BasicPicker<T>` composes a `Popup` face with a `Listbox` in an `Overlay`.
Both have type-ahead: `TypeAhead` holds the characters typed in quick succession, reset
by a timer on the realization's clock (`ControlOps::schedule`), and `Roster::typed`
matches them, in the scene order its root's `ControlOps` walks, against the names
the members' labels supply as they read now (`Label::set_name`, `Label::derive_name`),
a member with none passed over, one character cycling through the
names starting with it and more matching a prefix; the listbox moves focus to the
match, and the closed face chooses it outright through `Popup::set_key_fallback`, the
behavior's hook for the keys it does not act on itself. A control's drawn body
(`set_child`, `set_child_builder`) is optional and its setters come before or after it
alike: one with a label or parts alone still realizes its node, while a picker without
its face (`set_face`) renders nothing. A box or a control a press's pointer drags
declares the axes it drags along (`Container::set_drag_axes`, `Behavior::drag_axes`, a
slider's its own axis): a primary press keeps the pointer with
the nearest such node until the release—the web captures the pointer, the terminal
routes the press's moves and its release to the node—and a touch pans only along the
other axis (`touch-action: pan-y pinch-zoom` for a horizontal drag), while a press
anywhere else leaves each move and the release to the node under the pointer, as a
browser does; the terminal shows a grab cursor over a control that drags. An anchored popup on the web follows its anchor through CSS anchor positioning (the anchor
named after its node, the popup placed by `position-area`) where the engine has it, and
sits at the anchor's measured rectangle as it opens elsewhere. A `Tooltip` wraps the one
view it describes and mounts that view and a hint overlay, the `Bubble`, in a segment of
its own: it opens on the platform's clock once the pointer has rested for its delay and
at once on focus that shows (`Realization::focus_visible`), closes on leave, blur and a
dismiss, and reports the bubble as the description (`DescribedBy`, and `Description` for
text) of the node that takes focus within the view (`Scene::focus_holder_within`), the
report merged in the scene with that node's own; the web's server leaf renders the text
as `title` and hydration removes it.

A **context menu is a view of the same shape**. `ContextMenu` in `skene-foundation` wraps
the one view it serves and mounts that view and a popup overlay, the `List`, in a
segment of its own, exactly as `Tooltip` does; `serve(node)` attaches the list under a
node that already exists instead, for an app that wants a menu over its root. Wrapping
is the opt-in: a secondary press on the wrapped view, or `Shift-F10` and the
`ContextMenu` key while focus is inside it, dispatches `EventKind::ContextMenu` at the
node through the ordinary bubbling dispatch, and the deepest wrapper along the path
opens its menu pinned at the pointer (at the focused node's bottom-start corner from
the keyboard, through the overlay's `Placement` content) and answers `Prevent::All`, so
the browser's own menu stays wherever no wrapper is, and an app that wants its own
everywhere wraps its root. The terminal facade serves the standard menu from the root
at install, because the terminal has no menu of its own; a deeper wrapper wins over it
as it wins over the browser's, so replacing the default is the same act on both
facades. The items are `MenuEntry`s: `MenuEntry::new(label)` with `handle_activate`,
`set_disabled`/`derive_disabled` and `set_shortcut` (a `Shortcut` the item shows
beside its label, as its `Display` reads, and binds on the view the menu serves, so
the key runs a usable item from anywhere focus is inside that view, in place of the
platform's response), `MenuEntry::separator()`, and the standard entries
`MenuEntry::copy()`, `cut()` and `paste()`; a `Keymap` binds shortcuts the same way over
any view, no menu needed. A standard item asks the platform for its `Availability` each time the
menu is requested: `Absent` hides it, `Disabled` lists it disabled, `Enabled` lists it
live—outside a text entry only `Copy` exists, and only with a selection; inside one all
three are listed, `Copy` and `Cut` disabled without a selection—and running it calls
`Realization::clipboard` (OSC 52 in the terminal, the async Clipboard API on the web, where
the items are absent without it and a paste is permission-gated and stays best effort).
A menu with nothing usable does not open but still prevents the platform's, which is
what a right press on an empty spot did before.
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
`SimpleContextMenu`, a preset in each facade's Simple crate, the palette-decorated one. In
the terminal focus moves into the open list while the entry beneath keeps its
selection, so the standard items act on it, and closing returns focus without
reselecting the value; `Escape` closes an open menu before it clears a selection. The
web platform listens to `contextmenu` only on a node whose wrapper registered for it,
applies the menu key and `Shift-F10` itself as the key's default, preventing the key
and with it the browser's own menu where a wrapper serves the request, positions the
popover fixed at the pointer and shifts it back into the viewport, and
the server leaf renders nothing for the menu: with no script the browser's menu is the
menu, as progressive enhancement asks.
The
`Simple*` components are the ready-made library over the same builder: one palette, one
shape language, shared across the web and the terminal wherever the medium allows, and
deliberately nothing a third-party library could not build on the same footing—including
how an app reaches it: an explicit dependency, never a facade re-export. Its button,
card and tooltip are presets of the facade's built-ins in each facade's Simple crate:
`SimpleButton::new(label)` is the facade's `Button` styled in the primary look,
`secondary`, `outline`, `ghost` and `destructive` its siblings, and
`SimpleButton::derive_variant(label, || variant)` the button whose fill, ink and border
are derived leaves naming the variant's tokens, so a variant change re-inks it in place;
`SimpleCard::new()` is a `Container` on the card surface around the one child the app
gives it, and `SimpleTooltip::new` a `Tooltip` whose bubble is in the foreground color.
Each preset's pieces are public (`SimpleButton::styled`, `derive_styled`,
`SimpleCard::styled`, `SimpleTooltip::styled`), and a preset references the palette's
tokens without defining them: a `Theme` of the palette above it defines them, or a read
of the palette defines its default at the root. The palette defines every variant's
border token, transparent for a filled look, so a derived border needs the variant
alone. Its checkbox, radio group and radio, and its picker with its option
(`SimpleCheckbox::new()`, `SimpleRadioGroup::new()`, `SimpleRadio::new(value)`,
`SimplePicker::new()`, `SimpleListboxOption::new(value, caption)`) are presets too, each
replacing the built-in's mark and chrome by property and part. A mark follows the
control's state, `State::Checked` resolving against the enclosing control, rather than
reading the value the app binds after the preset is made: the web draws a box or a ring
on the card surface that fills with the primary surface while checked, and the terminal
stacks the checked and the unchecked glyph of `SimpleGlyphs` in one place, each hidden
unless the state matches. The picker's face reads the chosen option's name from the
roster (`Roster::name_of`), its indicator the face's `Part::Face`, its list surface the
`Part::List`, and an option's caption, which the preset takes so it inks in the primary
ink while chosen, also names it for type-ahead. Its slider, text field, text editor and
context menu (`SimpleSlider::new()`, `SimpleTextField::new()`,
`SimpleTextEditor::new()`, `SimpleContextMenu::new()`) are presets as well: the slider's
`Part::Track` holds its rail, its fill (`Part::Fill`) and its thumb (`Part::Thumb`),
placed by the value's fraction of the range the slider shares with its parts through
context (`SliderContext`), so the track follows a value bound after the preset is made;
the field and the editor take the field chrome and select their value in the palette's
colors through the entry's selection setters; the menu is styled through its list's
properties and its `set_item_*` setters. No Simple view remains: `skene-simple` holds
the palette, the looks, the variants and the sizes per unit, naming no realization.

- **A control decides its verdict.** The behavior runs first and its verdict is the
  whole verdict; a user's handler observes. A button's click never reaches the row it
  sits in, on any facade.
- **A choice's value is the app's own type.** A radio group, a listbox, a menu and a
  picker choose among values of any type that is a `Choice` (`Clone`, `Eq` and `Hash`,
  as SwiftUI's tag is `Hashable`), each member tagged with one (`Member::new(tag)`:
  `Radio`, `ListboxOption`, `MenuOption`). The cores hold them erased: a member its
  `AnyChoice`, which compares and hashes as its value, and a collection, the picker's
  core and the context they share what is chosen as an `AnyChosen`, a `Chosen` erased
  with the rules it chooses by (`Option<T>` for one choice, its `contains` and
  `choosing`), in a `ControlledChoice` its typed setters fill. So `Collection`,
  `NativePicker`, `Roster` and `BasicPickerCore` are each one type whatever the app's,
  and the typed views are thin over them: `BasicPicker<P, T>` holds a `BasicPickerCore`
  and builds it (`Thin`, a view over the app's types whose core is not, as a control is
  over its `ControlCore`), its face given
  a `Memo<Option<T>>`; a facade's `BasicRadioGroup<T>` and `BasicListbox<C: Chosen>`
  hold the control over the collection beside a `PhantomData` marker, the derives
  taking such markers beside the one field, and take their typed `set_chosen`,
  `derive_chosen`, `handle_choose` and `bind_chosen` from `BasicRadioGroupBase` and
  `BasicListboxBase`, which erase into the collection's own `K: Chosen` setters. A radio
  group or a picker holds a `T`, one starting with nothing chosen an `Option`, its
  members tagged `Some(..)`; a listbox holds its `Chosen`, `Option<T>` alone until a
  multi-select listbox lands. The option views stay untyped, and a member tagged with a
  value of another type than its collection's options carry is a debug assertion, the
  type known from a typed view's construction (`Collection::typed`,
  `ControlledChoice::typed`) or else from the first typed setter. Each
  member goes by a key in its `Roster`, its ordinal there (the order it joined, which
  the server leaf and the browser build alike and which never changes while it stays),
  so the scriptless form names an option by it and maps a key it reports back to the
  member's value (`Roster::key_of`, `Roster::value_of`); a key no member goes by asks
  nothing, and the controller's write of the truth repairs it.
- **A control is controlled.** It reports; the bound signal is the truth. Two-way
  binding is a derived read plus a handler write, on every facade, held once in a
  `Controlled<T>` that every shipped behavior, and a third party's, keeps its value in:
  `set`, `derive`, `handle` and `bind` for the behavior's own setters, `read` and `ask`
  for its logic. A content a behavior controls is registered in its `attach` through
  `ControlHandle::register` as a controller (`Controls`): a `Controller<T>` for a
  content held whole (`Controllable`: the checkbox's `Checked`, the slider's `Range`,
  the scriptless picker's `Choices`, a radio group's `Chosen` key) and an
  `EditController` for the text entry's `Edit`. Either holds both paths of the contract
  once, in the spine: the truth it reads, tracked by its render effect, is written as
  the node's content whenever it changes, and a reader's change of the content is
  reported through `report::<P>(node, Report::Change { content, proposed, applied })`
  and answered by that controller alone, whatever handlers the node or its ancestors
  carry. It reads, writes and installs its answer through a `ContentOps`, a table of
  plain functions (the realization's read, write and composing, the scene's install of
  the reporter, a door's sink lookup) filled once from the build's one realization by
  `ContentOps::of::<P>()`, never a `dyn` realization, so a caller holding no realization
  type registers a controller (`register_with`) and writes a content
  (`ContentKey::write_with`); `register::<P>` and `write::<P>` are the realization's and
  a door library's one-line forms of the same calls, and `ControlHandle::register`
  passes the `ContentOps` its `ControlOps` carries. The controller keeps what the node
  shows (read once through `Realization::read`, then what it wrote and what was
  reported) and decides over it in plain functions the spine unit-tests: a report the
  truth already holds asks nothing and stands, and so does one made while it writes, its
  echo, but for a selection the write moved, which it asks for as the reader's move
  would (the terminal reports the caret its write collapses inside the write); a write
  of what is shown is skipped; while `Realization::composing` says what the node shows
  is still the reader's, a write waits and a refused change stands with its repair held
  back, until the report that ends it is answered with the truth; and a disabled control
  asks nothing and answers with its truth. A content held whole is the reader's in the
  parts `Controllable::holds` names (a range's value, not its bounds; a choice's key,
  not its options): a change the truth then refuses is repaired by the controller
  writing the truth whole, answered `Verdict::Refuse`, where an edit's verdict is the
  platform's to apply. Registering a content listens for its reports
  (`EventKind::Report`), so a platform installs what it needs from the registration
  alone: the web its `input`, `change`, `compositionend` and `selectionchange`.
- **A drawn entry reports an edit whole and applies its verdict.** A text entry
  reports a reader's edit as one `Edit`, its text and its selection together, with
  whether its toolkit shows it already, and a moved caret as the same change of the
  text it shows, and is answered with a `Verdict`: `Accept`, or `Replace` with the
  truth, which it shows. A truth that gives no selection keeps the proposed one, mapped
  through the smallest edit from the proposal to the truth by one rule in the shared
  half (`Edit::map_through`, counted in grapheme clusters by `TextIndex::map_through`
  whatever unit the platform's protocol counts in): an offset before the replaced range
  stays, one after it moves with it, and one strictly inside it stays as many clusters
  into the replacement as it was, clamped to the replacement's end, so a transform that
  rewrites the whole text, as uppercasing does, keeps the caret where the reader typed.
  The verdict is the only write-back, so a vetoed or transformed edit leaves no step in
  the core's undo history, and the truth's own write after it finds the truth shown.
  `TextEntry`'s truth (`EditTruth`) is the text and the selection it derives, each
  `None` where nothing derives it: a change asks for the text where it differs from the
  truth's, then for the selection where it differs from the truth's, or from the one
  shown where nothing derives it; a selection derived beside no derived text is placed
  in the text shown; and an entry deriving neither keeps every change. The terminal's
  entries, which edit with Skene's editing core, report before applying, the core's
  `Arbiter` adapted onto `report`. The web's `<input>` and `<textarea>` report as a
  toolkit that applies first: each `input` event and each `selectionchange` of the
  focused entry is one applied `Edit`, its selection read in UTF-16 code units and
  counted in clusters, and a `Replace` is repaired by the smallest splice
  (`setRangeText`) with the selection mapped by the same rule, inside the event. While
  an input method composes (`InputEvent.isComposing`), or an adopted entry holds text
  typed before the client arrived, the web answers `composing` for the entry, so the
  controller holds its writes and repairs back until `compositionend`, or hydration's
  replay of that text, reports the entry again; a cut or a paste from the context menu
  splices the entry's selection, the caret after the text it inserts, and dispatches
  the bubbling `input` (`deleteByCut`, `insertFromPaste`) a browser's own would, so it
  reports the same way; `Edit::holds` makes the report idempotent whichever of the two
  events an engine sends last. A `TextSelection` counts grapheme clusters; where a
  platform's protocol counts in code points or UTF-16 code units it names its unit
  (`Unit::CodePoint`, `Unit::Utf16`) and converts through one index per text
  (`TextIndex`), as the web does for its `<input>`.
- **A secure flag and a keyboard cross as contents.** `TextEntry::set_secure` writes a
  single-line field's `Secure` content (an editor is never secure), and the field masks
  its text and keeps it from the clipboard: `Copy` and `Cut` are disabled in it, through
  `clipboard_availability`, so the standard context menu lists them disabled, as
  Chromium's password field does, while `Paste` stays. `TextEntry::set_keyboard` writes
  its `Keyboard` content, one value naming what the entry takes (`KeyboardType`: text,
  an email address, a whole or a decimal number, a telephone number, a URL, a query),
  what the platform may fill it from (`Autofill`, the purposes the web's `autocomplete`
  and text-input-v3's content purpose share, with the username and the new password
  beside them), whether the input method corrects what is typed, how it capitalizes
  (`Capitalization`) and what its `Enter` key says (`SubmitLabel`), each part a
  platform's protocol has no word for left to the platform. The web realizes both on the
  field's `<input>`: the secure flag is `type="password"`, and the keyboard is
  `inputmode`, `autocomplete`, `autocorrect` with `spellcheck`, `autocapitalize` and
  `enterkeyhint`, changing the `type` only to one that keeps the selection API (`tel`,
  `url`, `search`; `email` and `number` throw on the `selectionStart` an edit's report
  reads), a change of `type` keeping the value and the selection shown, and the server
  rendering the same attributes. The terminal draws a secure field through the core's
  `Masked` engine and has no keyboard to offer.
- **Parts and control state.** A part is a named child the styling targets (`Track`,
  `Fill`, `Thumb`, `Mark`, `Face`, `List`, `Item`, `Label`); in the terminal a part maps
  to a glyph token. A `State` a box cannot have on its own (`Checked`, `Indeterminate`,
  `Disabled`, `Invalid`, `Open`) resolves against the enclosing control; a node's own
  `Hovered` and `Focused` stay its own. On the web a control that owns a native input
  compiles control state to a selector anchored on that input; natively it is the
  control's own state.
- **The scriptless form is the control.** A text field or editor is its native `<input>`
  or `<textarea>` itself, decorated as the box; a checkbox, radio or slider keeps its
  native input inside its label, covering the label's box unseen and focusable, beside
  the parts that draw it, which take the press it lets through, as a run inside any
  control leaves a press to the box it falls in. So keyboard, form participation and
  accessibility are the browser's before and after hydration and nothing is swapped. A
  radio group's radios share the group's element id as their `name` and carry their key
  there as their `value`, which each radio takes from its `Membership` content (its
  group's node and its key), so before the bundle the browser keeps one of them chosen
  and submits them as one field, and a choice made there reaches the typed signal as any
  adopted control's does, its key mapped back to the radio's value. The group writes
  its chosen key as its `Chosen` content, which checks the member whose key it is and
  unchecks the rest; a radio's `change` is reported at its group's node as a choice of
  its key, applied, and a choice the truth refuses is repaired by one write of the
  group's chosen key, so the web keeps no decision of its own and no member is re-read.
  A checkbox's `change` reports its `Checked` content applied, and a range's or a
  select's `input` its `Range` or `Choices`. A click on a native toggle that something
  cancels has its flip restored by the browser once the dispatch ends, over whatever
  its handlers wrote, so the toggles it touched are given the scene's checked state
  again after it. The picker alone is drawn (no engine but Chromium restyles an open
  `<select>` yet), so it is an `Enhance` of its own: its scriptless form is a
  `NativePicker` control, a `<select>` on the web, listing the options the option views
  name, each `<option value>` its key, its caption a `<label for>` beside it and the
  chosen option selected, or an empty hidden option first and selected where nothing
  chosen is among the options, so the select never shows its first option as a choice
  the truth did not make; the options and the chosen key are derived as one `Choices`
  content its controller writes whenever either changes; the option views build once,
  under the group both forms answer through, and attach nowhere until the drawn form
  lists them, and the caption builds once and each form mounts it with its name and the
  disabled setting, so a swap rebuilds nothing but the face; a choice the reader made in
  the `<select>` before the bundle arrived reaches the signal as any adopted live value
  does, and the drawn face that takes the select's place shows it.
- **Accessibility is reported, never spelled.** A behavior reports what it is
  (`Checked`, `Value`, `Expanded`, `HasPopup`, `Orientation`, `Selected`) and a control's
  user adds what any control may carry (`Required`, `ReadOnly`, `Invalid`,
  `DescribedBy`, `Disabled`); the platform maps each to its layer. On the web a form
  element takes the native attribute where one exists (`required`, `readonly`), so the
  form enforces it without script, and ARIA otherwise, while what its content shows
  stays the content's: a slider's input takes its native `value`, `min`, `max` and
  `step` from its `Range` content, its bounds before its value, never from its `Value`
  report, which ARIA on a drawn slider and AccessKit read. A property naming another
  node (`Controls`, `LabelledBy`, `DescribedBy`) gives that node an id derived from its
  node id, identical on both leaves. A name comes from wherever it is: a string caption is
  the control's own (`aria-label`, or the native input's), and a caption drawn by a
  view names the control by reference (`LabelledBy` the caption part), so a combobox,
  a listbox or a radio group, which ARIA forbids naming from content, is named all the
  same. A region, a dialog and a painted picture name themselves through `set_label`
  and `set_labelled_by` on `Container`, `Overlay` and `Painted::set_description`, the
  latter making the box a named image; without a description a painted box is
  decoration. `Role::Status` and `Role::Alert` are the live regions, `Role::Dialog` a
  popup's role, and a tooltip's bubble carries `role="tooltip"` from the platform. A box
  that scrolls takes `tabindex="0"` unless a focus order is set, as Chromium makes a
  scroller focusable, so the keyboard reaches its content, and so does an activatable
  row on both leaves, with no role but the one it is given: ARIA's `row` needs a grid
  around it and `button` may hold no interactive content. The browser leaf listens for
  the keys wherever a box handles a click and maps the scene's key defaults, a row's
  `Enter` dispatched as its click, its `Space` press kept from scrolling the page and
  its release dispatched as its click. A focus order is `tabindex="0"` or `"-1"`, never a
  positive index. The control set to autofocus that opens the page focused carries the
  `autofocus` attribute on the element carrying its focus, its input for a native
  control and its box otherwise, on both leaves, so the scriptless page opens focused
  on it; the browser leaf's first mount focuses it once its turn ends, passing over a
  subtree the computed style hides, and a hydration keeps the focus the browser gave
  the served element and moves none. The server computes no style, so it marks the
  first that asks even where its box's style hides it. A shortcut is
  served as `aria-keyshortcuts` on both leaves; the browser leaf hears a key pressed
  while nothing holds focus, on the `<body>`, through one `keydown` listener on the
  document, which dispatches it at the root so the window's shortcuts reach it, a key
  pressed on the page's own element outside the root left the page's, and a key a
  shortcut takes has its browser default prevented. A subtree the web passes over is
  one whose style lays it out as none or makes it invisible. The gallery runs under
  axe-core in the browser tier (`browser_axe`, the library vendored beside the test), so
  every built-in is checked against the ARIA rules as it is drawn and with each of the
  gallery's overlays open: a tooltip, the picker's popup and a context menu, once under
  every rule and once under the naming rules alone, each proven to have run, a rule of
  the test's own (`composite-name`) requiring the name ARIA requires of a radio group,
  a tree and a grid, which axe has no rule for. Disabled controls leave the focus
  order, a deliberate reading of the practices' allowance, recorded in `AGENTS.md`.
- **Parity holds by construction.** `skene-conformance` is one suite of scenarios, each
  opening a page of the suite, acting on it as a user does and checking what a reader
  finds—the probes the page publishes of the signals it binds, the node holding focus,
  what a reader finds by role and name and reads of a node—through a `Driver` each
  platform implements over its own seams: open, probe, find by role and name or by
  text, read, hover, click, hold and release, press, type, focus, resize, advance, and
  whether focus shows and a node holds a press. The suite holds the scenarios, the
  driver, the probes and the names of its pages (`PAGES`, what each holds stated beside
  it), and no view: each facade writes the pages in its own vocabulary, as an app on
  both media writes two UIs, the web's in `skene-conformance-web`'s `pages` and the
  terminal's in a `pages` module beside its driver in its tests, the two files shaped
  alike, item for item, and each held to `PAGES` (the web's harness refusing to run
  otherwise, the terminal by a test of its own), while the suite's own test holds
  `PAGES` to the pages its scenarios open. A page is a view built on the platform, so a
  driver builds it in its own process or serves it to another: the web's,
  `skene-conformance-web`, serves every page from one app and acts each
  scenario out over a WebDriver session in Firefox and Chromium by trusted input, an
  event a page dispatches to itself running no browser default, finding what it acts
  on by the role and the name the browser computes, and holds every page to a
  screenshot golden that a pinned Firefox draws in a pinned font, pixel for pixel. A
  behavior every platform must share is a scenario, and a divergence the suite finds is
  fixed in the platform, never excused in the suite. The
  rules those scenarios exercise that a platform realizing its own overlays would
  otherwise copy live in the spine: an overlay's role, which `NodeSpec::new` computes
  (a hint's tooltip, a modal's dialog), and the focus a closing overlay returns, to
  the node focused when it opened or its anchor's nearest focusable, as a browser's
  top layer does.
- **A platform that names its nodes itself names them once.** The browser computes
  accessible names; everywhere else the spine does (`Scene::accessible_name`), over the
  scene's structure and parts and the facts a platform records of each node
  (`NameFacts`: role, reports, text, placeholder, value, whether it is a link, a block or
  hidden). It follows the accessible-name computation: the nodes a node is labelled by,
  its label, a control's caption, a text entry's placeholder, then its content where its
  role or its link takes the name from content, a block's text set apart, an embedded
  entry, picker or range giving its value and hidden content giving nothing. A native
  element's role stands where nothing states another (`NativeElement::role`: a text
  field is a text input). The queries a test reads go through the same walk:
  `find_by_role`, `find_by_text`, and `find_all_unnamed`, every link and every node of a
  role ARIA requires a name for that has none, which the terminal's gallery asserts
  empty as the web gallery's `browser_axe` holds axe's naming rules in the browser. The
  same walk builds the accessibility tree as AccessKit models it (`Scene::access_tree`):
  each node's role, name, value, states and relations under a window, focus on the
  focused node—what a native platform hands its screen reader, and, as indented text
  (`access_outline`), what a test asserts.
- **A drawn entry reports its text as AccessKit reads an editable one.** A platform
  whose entry runs on the editing core hands the scene its text runs
  (`NameFacts::text_runs`, an `AccessText` built once above the engine from the entry's
  editor by `AccessText::of`), and the tree gives the entry one `Role::TextRun` child
  per laid line, ahead of its scene children, under an id `NodeId::access_run_id`
  makes apart from every node's: the line's shown text (one `•` a cluster for a secure
  entry, its word starts read from the marks), each character's byte length (a cluster
  longer than 255 bytes reported as several characters), position, width and word
  start, the line's bounds and its direction, in the units and coordinates of the
  editor's layout with no window or scroll offset. The entry carries its text
  selection over the runs and the actions its state allows: `SetValue`,
  `SetTextSelection` and `ReplaceSelectedText`, a read-only entry the selection alone
  and a disabled one none. The outline leaves the runs out, and an entry's value stays
  its real text.
- **Focus follows the scene's child order.** A control's place in the tab order is its
  behavior's `focus_order` or the one its user sets (`set_focus_order`): reached in the
  scene's child order (`FocusOrder::Auto`) or passed over (`FocusOrder::Skip`), never at
  a position of its own, so the order a reader tabs through is the order a reader
  reads. `ControlBase::set_autofocus` asks for focus as the window opens, a fact read
  once at build: the first enabled control that asks, in the scene's child order, takes
  focus through `Realization::focus` once the mount that opens the window has ended,
  not showing it until a key does, as Chromium's autofocus shows none (Firefox's shows
  it, and the web leaves that to the engine), and a window where none asks opens with
  nothing focused, as a Chromium page does.
- **A shortcut is window-scoped.** `ControlBase::set_shortcut` and
  `ContainerBase::set_shortcut` give a control or an activatable row the key that
  activates it from anywhere in the window, a fixed `Shortcut` read once at build, which
  the scene's key default resolves after every handler has had the key
  (`KeyDefault::Shortcut`) and dispatches as a click at the node, focus left where it
  is; a collection, which takes no focus of its own, has its shortcut move focus to the
  member holding its stop (`KeyDefault::FocusShortcut`), as `aria-keyshortcuts` allows,
  one whose members hold no stop passing it over, and a box handling no click ignores
  its shortcut. The node reports it
  (`AccessProp::Shortcut`, in the W3C form `Shortcut::reported` writes, `Control+S`).
  A key holding no modifier but `Shift` is a text entry's own while one holds focus, a
  character it types or a key it edits by, so such a shortcut waits there, and the
  platform's own editable element the scene keeps no entry for, a native view's input,
  a `<select>` or editable content on the web, keeps it the same way (`key_default`'s
  `typing`); a view-scoped
  binding of the same key, a `Keymap`'s or a menu entry's, acts first inside its
  view.
- **Membership is read from the tree.** A grouping control's members live in a `Roster`
  ordered by walking the subtree under its root, so a reconciling set stays in document
  order however it changes and a destroyed member is absent because the walk cannot
  reach it; the key a member goes by is its ordinal, not its place, so a member moved
  keeps its key. A radio group wraps, a listbox clamps, because the platforms' do.
- **The terminal's controls are composed trees**—a `Container`, a `Text` mark from a
  glyph token—painted by the same painters as everything else, so they carry roles,
  accessibility properties and focus order, and the keyboard tables live in the
  behaviors rather than in a match per medium.
- **The styled built-ins are each facade's own.** `Button`, `Checkbox`, `Toggle` (a
  switch), `RadioGroup<T>`, `Radio`, `Slider`, `TextField`, `TextEditor`, `Picker<T>`,
  `Listbox<C>` and `ListboxOption` are drawn, default-styled controls over `Activate`,
  `Toggle`, `Toggle::switch` (which reports `Role::Switch`), the `RadioGroup`
  collection, its `Radio` member, `Slider`, `TextEntry` (single-line and multi-line),
  `BasicPickerCore`, the `Listbox` collection and its `ListboxOption` member, written in
  parallel files per facade (`views/button.rs`, `views/checkbox.rs`, `views/toggle.rs`,
  `views/radio.rs`, `views/slider.rs`, `views/text_field.rs`, `views/picker.rs`,
  `views/listbox.rs`), since the trees the media
  compose differ; every default reads `SystemColors`, so a theme of the set restyles
  them all. Each builds its defaults as a control of its own at build and lays the
  control its user configured over it (`Control::over`: the user's box style overlaid
  on the defaults' field by field, `BoxStyle::overlay` over any shape, the caption,
  flags and body replacing the defaults' where set, a part replacing the default part
  of its kind), so every `set_`/`derive_` its user calls wins whatever its order, and a
  default changes no layout property with a state. `HasControl` is how a view carries
  every control's members (`ControlBase`) whether it holds a control as its whole
  content or beside what its defaults read, as `Button` holds its label, its prominence
  and its label's color. `Button::new(label)`/`derive(label)` makes the label its
  content and its name; `set_prominent`/`derive_prominent` is the semantic option
  showing it as its screen's primary action, each leaf it picks a token where fixed and
  a derived leaf where it follows a signal, so a prominence that changes re-inks the
  button in place. In pixels the button is at least 32 tall, padded 4 by 12 inside a
  border of 2 rounded by 6, filled with `CONTROL` and its hover and pressed variants
  (the accent's darkened by a tenth of Oklch lightness where prominent, and `CONTROL`
  while disabled, prominent or not), its label in `INK` (`ON_ACCENT` where prominent,
  `MUTED_INK` disabled), and its border a ring in the
  accent (`INK` where prominent) while focus shows; in cells it is padded a cell at
  either side, and focus shows as accent cells under `ON_ACCENT`, reversed where
  prominent. `Checkbox` and `Toggle` carry `ToggleBase` (the checked state, controlled,
  through `HasControl`) and `Slider` carries `SliderBase` (its range, its value,
  controlled, and its thumb extent), as `TextField` and `TextEditor` carry
  `TextEntryBase`. The checkbox's default `Part::Mark` is an 18-pixel box in `FIELD` and
  `BORDER` turning to the accent with a check in `ON_ACCENT` while checked, and the
  glyphs `[ ]` and `[x]` in cells, `set_part(Part::Mark, ..)` replacing either. The
  switch's default `Part::Track` holds its `Part::Thumb`: a 40 by 24 pill in `TRACK`,
  the accent while on, with a 16-pixel `KNOB` disc, or three cells with a `●` thumb, the
  thumb shifted to the end by offset rather than layout while on; on the web its input
  is `<input type=checkbox switch role=switch>`, kept as the scriptless form as a
  checkbox's is. `RadioGroup<T>` and `Radio` are drawn over the `RadioGroup` collection
  and its `Radio` member, the group keeping `BasicRadioGroupBase`'s typed
  `set_chosen`/`derive_chosen`/`handle_choose`/`bind_chosen` (the trait, as
  `BasicListboxBase`, rides `HasControl`, so a view holding its control beside its own
  state carries it): the group holds its children (`add_child`, `set_children` and their
  kin, through `HasChildren` with a flex child's kind) in a `Column` spaced 8 pixels, or
  on rows of their own in cells, beneath its caption, its control's axis vertical and its
  items at the start, a body `set_child` supplies taking the column's place. A radio's
  default `Part::Mark` is an 18-pixel ring in `FIELD` and `BORDER` turning to the accent,
  an 8-pixel accent dot showing at its center while chosen, or the glyphs `○` and `◉` in
  cells, read from the group's context. `Slider` is drawn over the `Slider` behavior,
  which reports where its value sits along its range (`Slider::reads_fraction`) for the
  defaults to follow, and shares it with the parts it holds as a `SliderContext` it
  provides as it attaches, so a track set in the default's place reads it there: 160
  pixels or 20 cells wide, its `Part::Track` a flexible child taking what its caption
  leaves and holding its `Part::Fill` and its `Part::Thumb` (marked after build, as the
  switch's thumb is), a rail in `TRACK` clipping a fill in
  the accent that runs to the thumb, beneath a lane the thumb travels less its own width,
  the fill offset back by the unfilled fraction and the thumb forward by the filled one,
  so the value moves them by offset rather than layout; the thumb is an 18-pixel `KNOB`
  disc ringed in the accent (`INK` while focus shows) or a `KNOB` cell (`ACCENT_PRESSED`
  while focus shows), and the behavior's thumb extent is the default thumb's until
  `set_thumb_extent` says otherwise. On the web the slider keeps its range input covering
  its label, the scriptless form. `TextField` and `TextEditor` carry `TextEntryBase`
  (the text, the hint, the keyboard and the selection, all controlled, through
  `HasControl`) and the colors their value is selected in, a `SelectionColors` the view
  defines at its node as it builds (`SelectionColors::define`, redefining the typography
  selection tokens the entry's value is selected in there, a variant testing the
  environment kept and one testing the entry's state not); `TextField` adds the secure
  flag and `TextEditor` its lines, three by default and two at least. Their defaults: a
  `FIELD` fill (`CONTROL` while disabled) inside a border of 1 in `BORDER`, the accent
  while focus shows, padded 4 by 8 and rounded by 6 in pixels and a cell at either side
  in cells, the value selected in `INK` on `SELECTION`. On the web each is its `<input>`
  or `<textarea>` styled by CSS; in the terminal its window over the entry the scene
  keeps. `Picker<T>` is a `BasicPicker<T>` laid over defaults (`BasicPickerCore::over`,
  as `Control::over` does for a control): a face filled with `CONTROL` and its hover and
  pressed variants, bordered in `BORDER` and ringed in the accent while focus shows or
  the list is open in pixels, accent cells while focus shows in the terminal; a face
  showing the chosen option's name, read from the roster through the picker's
  `ListboxContext` (`Roster::name_of`), and the indicator `▾`, a glyph in both media; a
  list box (`BasicPickerCore::set_list`, a `Container` the options are placed in,
  recorded as the `Part::List`) in `FIELD` inside a border in `BORDER`. On the web it
  stays an `Enhance` over its `NativePicker` `<select>`, which takes the face's
  decoration, served and swapped as the basic picker's is. `Listbox<C>` holds its
  options as `RadioGroup<T>` holds its radios, in a `Column` inside a list box of the
  same look (`set_list` replacing it), beneath its caption; a `ListboxOption`, the
  option of both, fills with `CONTROL_HOVER` while hovered and `CONTROL` while focused
  (`CONTROL_PRESSED` in cells), rings in the accent while focus shows in pixels, and its
  default `Part::Mark` is a check in `INK` showing while chosen and keeping its place
  otherwise. The conformance pages keep the `Basic*` controls.

## The style model

Styling is **data**, not closures, because two requirements demand it: server-rendered and
statically-generated pages must be responsive, themed and animated with **no JS/WASM**,
and every platform speaks the same layout semantics. A closure is opaque—a server can
only serialize its current value; style data compiles to real CSS and evaluates natively.

**One property model, spelled once.** `BoxStyle<N: Scalar, S: Shape, X: StyleExtension>`
and `RunStyle<S, X>` name every shared property once; the leaf shape is a type parameter
(`Declared` for a builder, `Static` for a platform, `Plain` for a keyframe or a resolved
style) and the medium's own properties come through the extension (`WebStyle`: the corner
radius and the type metrics; `TerminalStyle`: border glyphs, the corner radius, a shadow, bold,
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
elsewhere, and the overline and the line through as SGR `53` and `9`. A platform that
draws a decoration's lines itself takes them from one computation above its engine,
over the metrics the engine reports per stretch of text (`RunExtent::decoration_lines`):
none for an undecorated stretch, two solid lines for a double one, the second above
an overline's first, and one otherwise, each at least a unit thick. A
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
              | ReducedTransparency | HighContrast | Pointer(PointerPrecision) | Hover
              | Container { name, query } | And | Or
State         = Hovered | Focused | FocusVisible | Pressed              // the node's own
              | Dragged | Targeted
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
- **A length is a plain number**, an `i32` or an `f32`, wherever one is taken. Every
  length-valued property's value, leaf and definition converts from both, so
  `set_width(16)`, `set_width(16.5)`, `when(.., 16)`, a token defined as `8`, a lazy
  list's `set_item_height(32)` and a slider's `set_thumb_extent(16)` are the ordinary
  spelling, and every constructor measured in lengths takes `impl IntoScalar<N>`:
  `Spacing::new(8, -4)`, `Inset::new(0, 8, 0, 8)` in its fields' order (start, top,
  end, bottom), `Inset::all`, `Grid::adaptive(120)`, `Media::min_width(900)` and
  `ContainerQuery::max_width(40)`, so code generic over the platform's scalar writes
  `Inset::all(4)` as an app does. `IntoScalar` lands a number in the unit through
  `Scalar::from_i32` and `Scalar::from_f32`: a pixel holds either exactly, and a cell
  rounds an `f32` to the nearest at the setter, so a fixed length is a whole cell, as is
  a lazy list's item extent and a slider's thumb extent; an expression's result and a
  motion sample land on one where they are evaluated, and layout still rounds what a
  fraction or a flexible share distributes. A `Length` keeps its fraction in every
  unit: a font size, a letter spacing, and the terminal's corner radius and shadow
  offset and softening, which its backdrop paints in pixels (`DECISIONS.md`). A grid
  track takes a plain number through its constructors (`GridTrack::length`,
  `fit_content`, `between`, `adaptive`, a limit's number a fixed length) and its own
  `From`. An inset of dimensions (a margin, an offset, a pinned edge) takes a plain
  number as a fixed length beside a `Dimension`. Any other type is no length, and
  `IntoScalar`'s diagnostic states the rule.
- Conditions ride the value: `value.when(condition, other)`. The composition machinery
  (`ConditionTree`: `holds`, DNF expansion) is one generic in the spine; the web emits one
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
  decorated box, and per-child properties ride in wrappers. Every platform but the web
  maps a resolved style onto taffy through `skene-layout` once (`BoxLayout`, generic
  over the scalar, a border taking a unit per edge drawn in cells and its width in
  pixels) and lays out on its `Tree`, keyed by the scene's nodes and kept across frames,
  each node's style, text and children written where they changed, revisiting the nodes
  the shared `Restyle` says moved (those queued, in motion or reading an interaction
  that moved, each with the leaf its text flows in, or every node once the definitions,
  the environment or the structure moved): by the terminal's records as they change, or
  by a walk of the scene where the structure moved or a node's part changed its shape
  and of the revisited nodes alone otherwise (`Tree::lay_out_revisited`), at the values
  its motion presents. There a box whose children are all inline runs lays them out as
  one text, a `TextFlow` of their runs and inline boxes over the platform's text engine
  (Text, below), keeping their blanks and newlines as the reset's `white-space:
  pre-wrap` does and wrapping where the Unicode line breaking algorithm allows across
  them and around an inline box, laid out at its content's size first, a word too wide
  for a line of its own broken where the line fills as `overflow-wrap: break-word`
  breaks it, the blanks a line ends in hanging where it wraps and taking room up to its
  limit before a newline or at the end, each run's line box on its line's baseline, a
  box's bottom on it, a run placed in a rectangle per line it is on, clipped off any box
  beside it and off the blanks hanging past the line's end, and, where its platform
  trims it (`TextFlow::trimmed`), the text trimmed to its first line's cap height and
  its last line's baseline as the reset's `text-box-trim` trims a text box. The layout's
  own tests lay their text out through `skene-parley` in the pinned face the conformance
  suite draws in, Inter's regular, kept in `packages/parley/fonts`, its metrics
  unrounded (kerned, its letter spacing past each cluster, a line its font size times
  its line height tall, each as the typography in force sets them), and where Parley
  reads text as an editor does its reading stands: a tab advances to the next stop
  eight blanks apart, as a browser's `tab-size` sets them, and a newline ending the
  text opens a line. `Dimension` is a fixed length, a `Fraction` of the
  parent, or auto. Overflow is per axis (`Axes<Overflow>`, one
  overflow setting both) and computed as CSS computes it, an axis left visible beside
  one that is not scrolling where needed; the terminal scrolls, and draws a scrollbar,
  only along an axis that scrolls. An adaptive track (`GridTrack::Adaptive`,
  `Grid::adaptive`) fits as many tracks as the container holds, as SwiftUI's adaptive
  grid item does, compiled to CSS's auto-filled repetition. A grid names areas over its
  cells (`GridAreas`, a row of cell names per grid row) and the lines between its
  tracks (`GridTemplate`, a track list with names added where lines stand, a plain
  list naming none), and a child is placed by an area's or a line's name (`GridName`,
  `Line::area`), a template breaking CSS's rules dropped whole on every platform as CSS
  drops it. A length below zero is laid out as CSS takes it: a size (a width, a height,
  a limit, a flexible basis) as auto, the declaration dropped, a track list holding one
  dropped whole, a padding, a border or a spacing as none, and a margin or an inset
  with its sign. A variant is no exception: a negative length under a condition that
  holds lays out as the same length in the base, whatever its leaf, so the web renders
  a negative literal as the drawing platforms lay it out (`initial` for a size and a
  rejected track list, none for a padding's, a border's or a spacing's edge, `auto` for
  a basis) rather than as a declaration CSS drops for the base's, a divergence from
  CSS's parsing recorded in `DECISIONS.md`. An expression and a motion sample are held
  to the property's range where they are evaluated instead (`Property::is_non_negative`,
  `BoxValue::in_range_of`), as CSS holds a `calc()` and an interpolated value: a size,
  a padding, a spacing, a flexibility or a track list below zero is zero there, never
  dropped. The framework invents no layout
  system.
- **The record** (`Record<N, X>`) is a node's style as one list: every property its box
  and run styles set, keyed by its `PropertyKey` (each naming one box or run property)
  in the order written, each style's in the order its visitor gives (a run's own before
  its layout's, as the web compiles a run), each value a `RecordValue` erased over the
  property (a `StaticValue` of `SlotValue`s: a fixed value erased, a token kept by its
  name, an expression by its tree, a derived leaf as its `KeptLeaf::Slot`), with its
  transitions as data and each animation's keyframes as `Frame`s. It is written typed,
  a property at a time (`set_box::<F>`, `set_run::<F>`, keyed by a `BoxField` or a
  `RunField`, so a value of another property's kind does not compile) or a style at a
  time (`add_box`, `add_run`, `add_box_dynamics`, `add_run_dynamics`), and read erased
  (`entries`, `get`, `has`). `box_style` and `run_style` read the typed styles back.
  `Record::compile` compiles it in the shared half to a `CompiledStyle` at a `Keep`
  level, named with what resolves the rest (`Keeping`): at `Keep::Everything` every
  entry and frame stays as the record holds it (a token by its name, which a
  realization keys by the theme scope defining it and never by its definition's
  content; an expression by its tree; every condition kept), and each slot an entry
  references is bound to its property and the leaf's position (`SlotBinding`, `0` the
  base), so a derived write reaches the slot and never the static part; at
  `Keep::Nothing` each entry is resolved through the one `Resolver` as the typed style
  resolves it (`box_value`, `run_value`, each frame's leaves as `keyframe_box_value`
  and `keyframe_run_value` resolve them), a fixed leaf under no condition in the
  record's order, one resolving to nothing left out, and `resolved` reads it back as a
  `ResolvedStyle`; transitions and the animations' timing cross as data at both. A style
  kept at `Keep::Everything` also resolves at a node through the one `Keep::Nothing`
  entry, `CompiledStyle::resolve` (each frame through `Frame::resolve`): every entry
  resolved as `compile` at `Keep::Nothing` resolves it, by the same visitor over the
  entries, straight into the `ResolvedStyle` with no compiled style or address between,
  so a realization keeping nothing resolves a node's kept style at every restyle for what
  a typed resolution costs. Its
  `address` is the 128-bit xxh3 of its static part (the level, the entries and the
  motion), a cache key alone: equal static parts share one, and a realization names
  nothing by it, the web's class names staying the hash of the template. A coverage
  test holds it to the whole model for each medium (none, `WebStyle` in pixels,
  `TerminalStyle` in cells): a style setting every property, each under variants of
  every `Media`, container query, `State` and `Condition` kind with every leaf and
  expression node kind among them, crosses whole, motion included, crosses compiled at
  `Keep::Everything` whole, every slot bound where it stands, and compiled at
  `Keep::Nothing`, or kept and resolved through `resolve`, resolves as the typed style
  does under readings where every condition both holds and fails. It is how a style
  crosses the seam (`apply_style`, under 'The scene and the platform'): the shared half
  builds each box's, control's, overlay's and run's record (a box's style then its
  motion; a run's own style, then the per-child layout a wrapper annotated it with, then
  its motion) and compiles it at `Keep::Everything` by value (`into_kept`, the entries
  moved rather than copied), the `NodeSpec` the node was created from carrying the
  construction facts alone.
- **Raw CSS** reaches only a native view, through its own markup or element: a
  `NativeView`'s `style` attribute, or the element its adopt handler holds. The typed
  model carries no passthrough of its own.
- **Web** renders the whole set into one interned rule template: base declarations,
  `@media`, `@container` and pseudo-class rules, `transition:`, content-addressed
  `@keyframes`, `var()` for tokens and slots. State-, media- and theme-driven styling
  therefore works in a static document with zero script. A template is compiled from
  the node's record kept at `Keep::Everything` alone (its entries through `Rules`, its
  frames' leaves rendered as declarations, a container's `size` containment read from
  whether it has a height, its slot bindings the shared half's) beside the facts of the
  node's construction it reads, kept from the node's creation until `apply_style`, and
  both leaves cache it by the kept style's address and a hash of those facts. A box's
  focus order and accessibility follow its style, since whether a box scrolls, and so
  takes a place in the focus order, is its style's.
- **Terminal** (and native) resolve every property at one point through the spine's
  `Resolver`—variant overlay against the environment and the node's `Interaction`, token
  lookup through the scene, expression evaluation, slot reads—handing it the medium's
  unit, its token and slot lookups and its font size. The terminal caches the result per
  node under a generation covering environment, interaction and definitions, and
  reports whether the resolved value changed; that report is where a transition
  starts. It builds the resolver through one `ResolutionContext` (a node's environment,
  interaction and containers from the scene, a token resolved where it is defined, its
  definition converted into the shared `TokenValue`, the font size from the
  typography's size token, the platform's `EM` where text takes no sizes) over a
  `ResolutionSource` giving what the scene does not hold: the environment, container
  sizes, slots and whether text is drawn at other sizes. The browser is that
  resolution's oracle: the parity harness (`browser_parity`) builds each case on the
  browser leaf, its read node a low-level view keeping the style the shared half hands
  the web, resolves it through `CompiledStyle::resolve` with the resolver of a
  `ResolutionContext` over the web's own scene (its themes, scopes, containers and the
  enclosing control's state), runs its motion through the foundation's `Motion`, and
  holds the value to the one the engine computes, so a divergence it finds is one every
  platform resolving through the shared half shares. The terminal keeps the style
  `apply_style` hands it for each box, control, overlay and run, and resolves it at every
  restyle through the shared `Keep::Nothing` entry, the one resolution setting whether
  the node's interaction was read and giving a run its wrapper's layout beside its own
  properties, before its own fill-ins (rounded border glyphs, the typography's size and
  weight, `bold`); its transitions and animations are the kept style's. A text entry's
  value takes the typography's tokens in force at the entry.

### Tokens and themes

A design token is a named, inherited slot. `TokenRef<T>` names it (a `const`; the name is
an implementation detail on every platform), `Token<T>` pairs the name with the
definition a palette gives it (`Token::new` takes the reference whole), and a consumer
emits only the name, so two boxes under two palettes intern one class and a theme change
never touches a consumer. A token's own conditions live in its definition, once. A
length token serves every property measured in lengths, a box's sizes, padding, spacing
and margins, its corner radius and a run's font size and letter spacing alike, and a
property that is a plain number reads a number token alone.

A reference made by `TokenRef::of::<S, N>` carries a link to its palette's default: one
plain function per palette, naming the set's `TypeId` and yielding
`Definitions::of(&S::default())` in the foundation's `TokenValue<N>`, so a const in a
library core links without naming any realization's token value. `TokenRef::new` names a
slot with no link, for a set something always defines itself (`Typography`,
`SystemColors`). The link rides the `TokenName` a leaf holds, and equality, hashing,
ordering and formatting read the name alone, so a linked name interns the class an
unlinked one does.

The first reference defines the palette's default at the root, once per palette per
root: before a node's style reaches the realization, and before each write of a derived
slot, the shared half lists the tokens the style or the leaf names (`TokenReferences`,
over every entry's static leaves, expressions included, and every animation frame's),
and for each linked one whose set is not claimed yet `Scene::define_defaults` claims the
set by its `TypeId`, builds its default through the link, converts each definition into
the realization's token value, dropping a kind it holds none of (a text default on the
web), and lays them over the root through `Scene::define_root_set`, the claim a read
through `ThemedIn` makes too, so a read and a reference lay a set once between them; a
read still lays it on a miss. `Scene::define_tokens`, the one entry for every
definition, lists the linked tokens its definitions name in turn, so a default or a
theme aliasing another palette's token lays that palette as well, the claim ending the
recursion. The default is laid whether or not a theme above defines the palette, since
a style is applied before its node is attached and its ancestors are unknown; the
nearer definition wins, the theme's scope over the root on every platform. An unlinked
name costs one `Option` check, and the scene is borrowed only where a linked set is not
laid yet. Once per root is once per render on the server, whose every render starts a
fresh scene, so a palette first referenced in a streamed chunk carries its root rule in
that chunk's style delta, and once per mount on the terminal.

`Theme<P, S: TokenSet>` is the one theme view. It realizes one layout-invisible scope node
(`display: contents` on the web; the terminal makes no node for it, keeping no record,
and its definitions still reach it through `define_tokens`), provides `Memo<S>`
through context, and runs one effect in its `ThemeCore` that writes the palette's
definitions (`S::definitions()`, as they read now) onto the scope, so nothing rebuilds. On the web a palette change therefore costs the number of tokens, never
the number of nodes; the terminal still re-resolves every node beneath the scope until its
token caches take per-scope epochs. Nesting a theme redefines beneath it. On the web the static definitions
are an interned class on the scope element (with `@media` blocks and the scheme double
emission where a definition conditions on the scheme) and each token kind registers its
custom property with `@property`, so it interpolates and inherits as a typed value; a
palette's default laid over the root, by a reference to a linked token or by a read of
the set below no theme (`ThemedIn::current`/`watch`), is written under `:root` once,
the root rule and its registrations interned before the class of the node that
referenced it. Any node defining tokens takes them as such a class,
and a redefinition whose every token is unconditional writes them inline over it; a
node adopted from the server's markup gives up the definitions the server wrote inline on it at its first
definition. The root's definitions are defined in the scene and reach the realization
through the same `define_tokens` call as a scope's (`Scene::define_tokens` and the
seam's alike), their scope `TokenScope::Root`: the web writes them as
the root rule's class, and the terminal re-resolves its styles as it does for a scope's.
`Typography` (all text inheritance on both facades) and the Simple library's
`SimpleGlyphs` (its terminal marks, in `skene-simple-terminal`, the crate of the one
platform whose tokens hold text over cells) are token sets, so the framework has one
inheritance mechanism.
`SystemColors<N>` (`skene_foundation::SystemColors`, a facade's `SystemColors` in its
unit) is the platform's colors, as CSS's system colors are: sixteen color tokens (the
ink, a muted ink, the window, a control with its hover and pressed fills, a border, the
accent with its hover and pressed variants and what sits on it, a track, a knob, a field,
and a selection active and inactive) naming no look, which the built-ins' defaults read.
Each realization defines them at the root as it is installed, claiming the set's root
definitions (`Scene::define_root_set`) so a read of the set below no theme lays nothing
over the platform's, and a theme redefines any of them as it does any token, each name
a token of its own, so redefining a fill moves none of its hover and pressed variants. Its
`Default`, a neutral light look turning dark under `Media::Dark` built from public
constructors, is what `ThemedIn` hands out where no theme provides the set and what a
theme of the set itself defines; the platform's own colors are the realization's.
Nothing below a component library knows what a palette is: `skene-simple` owns
`SimplePalette`, and a third party's palette gets its `Theme`, `current()` and
`watch()` from the same blanket, and its default at the root by declaring its references
with `TokenRef::of`, with no definition code of its own.

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
terminal's size and background. `media::watch(&media)`
turns any declared condition into a memo rendered from the same query text the class
compiler renders, so the signal and the stylesheet cannot disagree. A box's measured size
reaches a view through `Realization::observe_size` (the lazy column's viewport, the slider's
track). On the server these hold the client hints the render was given
(`Sec-CH-Prefers-Color-Scheme`, `Sec-CH-Viewport-Width`, `Sec-CH-Prefers-Reduced-Motion`;
Chromium only, opt-in through `Accept-CH`/`Critical-CH`), else documented defaults.

### The interner

The content-addressed class interner is load-bearing: a node's complete rule template is
hashed with a wide hash to a deterministic name, interned once, reference-counted by live
node, collected when unused; naming never depends on intern order or liveness. Identical
styles collapse to one class; both leaves emit rules in interning order, which is build
order, so the server's stylesheet is byte-compatible with the client's and the cascade
order is identical, which hydration relies on. A class cache keyed by the kept style's
address and a hash of the construction facts its template reads renders the template
text on a miss only. The interner, the cache and the token and slot registrations are
one `StyleBook` both web leaves share, telling each leaf's `StyleSink` what to add and
remove (the browser: one `<style>` element per class; the server: each rule collected as
it is interned and emitted with the next flush, the document's `<style>` or a streamed
chunk's delta).

## Text

Text is laid out and drawn by each platform's own engine and edited by one core of
Skene's (`skene_foundation::text`). The foundation keeps what a text says and how it is
styled; the engine shapes, breaks, measures and draws it; what acts on text—selection,
editing, accessibility and the decorations an engine lacks—is written once above the
engine, over the geometry it reports.

- **The engine's contract.** A realization's engine implements `TextEngine`: it makes a
  `Paragraph` of an `AttributedText` (a value, a base `TextStyle`, ordered runs of
  other styles over byte ranges, and the `InlineBox`es standing in it, each a width and
  a height at a byte) under a `ParagraphStyle` (alignment, base direction, wrap, line
  height, a most-lines limit and its ellipsis), a batch at once where it can
  (`paragraphs`). A paragraph reports its intrinsic minimum and maximum, lays itself
  out at a width or unbounded as a `Laid`, repaints a range in place (`repaint`, false
  where it must be made again) and answers boundaries: a grapheme cluster, a word
  (blanks skipped, then one UAX #29 segment, a full stop between letters one of its own,
  as Chromium tailors the rules) and a paragraph. A `Laid` is immutable:
  no later layout or repaint moves its geometry, while a repaint shows in every layout
  of the paragraph already handed out. It reports its size, baselines, lines
  (`LineInfo`) and truncation, hit tests, carets (two at a direction boundary), range
  rectangles, the stretches painted alike with their decoration metrics (`RunExtent`)
  and its inline boxes' frames, and moves a caret visually and between lines, past the
  first or last line to the text's start or end, a line's end standing before the
  blanks hanging past it. Every position is a byte of the text and an affinity. A run's
  style is resolved from the style model (`TextStyle::resolved`): family, size, weight,
  slant, letter spacing, line height (a run with none takes the paragraph's) and paint,
  the color and the decoration, a change of paint alone repainting and any other
  laying out again (`TextStyle::shapes_like`).
- **The guaranteed core** is held by a test on every engine: blanks and newlines kept,
  a word too wide for a line of its own broken where the line fills, an empty text laid
  out as one line. The foundation holds it on a grid engine its tests alone build,
  `skene-parley` on Inter, the terminal on its cell engine, and the web in the browser
  tier (`text_keeps_the_guaranteed_core`), where a `Line` run with nothing in it takes
  one line's block size (`:where(:empty)`, `min-block-size: var(--skene-line)`, the
  reset's `--skene-line` being `1cap` where lines are trimmed and `1lh` elsewhere). A
  newline ending a text opens a line on every engine but the browser's, which keeps
  its own reading of it.
- **Measuring.** `TextLeaf` is the layout engine's leaf over any paragraph, measured
  through `MeasureText` as taffy asks: min-content its narrowest width, max-content its
  widest, a definite width laying it out within that width and reporting the width it
  uses, so a width under min-content breaks a word where the line fills. Its intrinsic
  widths are measured once a paragraph (an epoch, which `replace` moves), its last
  three layouts are kept by width, what measuring read at its last four widths is kept
  apart so measuring again there lays nothing out, and a width at or past the widest
  line shares the unbounded layout where only sizes and baselines are asked for or the
  lines start at the left edge whatever the width (`Paragraph::is_left_anchored`).
  `skene-layout`'s `TextFlow` is a box's runs and inline boxes as one leaf: it builds
  the attributed text, a box's placeholder (U+FFFC in a platform's records) becoming an
  `InlineBox` there alone, keeps its paragraph until a run's text or style or a box's
  size changes, a change of its runs' paint alone repainting the paragraph it keeps
  without the leaf measured again (`Leaf::repaint_as`), places each run and box from the layout's range rectangles and inline
  box frames, and answers which run a point is on (`run_at`) and which is nearest
  (`nearest`), with the byte a caret there stands before, for every client: hit
  testing, links and the document's selection.
- **Parley.** `skene-parley` is the engine over the patched Parley the workspace
  carries (`vendor/parley` and `vendor/parley_engine` through `[patch.crates-io]`, each
  change marked `Skene:` and listed in the crate's `SKENE.md`): a break never falls
  inside a grapheme cluster, a wrapped bidirectional line places its trailing blanks by
  UAX #9's rule L1, a soft hyphen's break draws a hyphen at its width, a tab advances
  to stops a set number of blanks apart (eight unless the engine sets another, as a
  browser's `tab-size`), stacked marks clear one another, a full stop between letters
  is a word of its own and words are shaped through a cache; ICU4X's dictionaries for Thai, Lao, Khmer and Burmese are compiled in. It
  loads the typefaces it is given and no system font (`ParleyEngine::with_typefaces`,
  a run's families ending with the first of them); a paragraph is shaped once and
  broken in place for its first layout, later layouts copying it; its paint is shared
  by every layout of it, so a repaint shows in all; a batch of `PARALLEL` paragraphs or
  more shapes on threads off the web; line metrics are rounded unless the engine is
  told otherwise (`set_quantize`); and a laid text draws through vello as outlines,
  with the decoration lines drawn above the engine (`ParleyLaid::paint_outlines`).
- **Index, segments and substitution.** `TextIndex` is one table per text between
  bytes, code points, UTF-16 code units and grapheme clusters, keeping none for ASCII
  text and rounding half a surrogate pair or the inside of a cluster as asked
  (`Round`); `Segments` answers words (UAX #29's, a full stop between letters a word of
  its own as in Chromium, as the carried Parley's analysis has it too) and paragraphs. `Substituted<E, S>` lays
  out a text in which a `Substitute` stands a character of its choosing in for some
  clusters and maps everything back, so every position it reports and takes is a byte
  of the real text and its inline boxes keep their places: the terminal's stand-ins
  are one use, and a secure entry's marks (`Masked<E>` over a shared `MaskState`, its
  word the whole text) the other, the terminal's field its client: its editor runs over
  `Masked<CellEngine>`, and the field draws each cluster of what the masked layout shows
  (`MaskState::shown`), mapped back to the value's bytes.
- **The editing core** (`text::editing`). `Editor<E>` runs Parley's cursor and
  selection model over any engine: a position a byte and an affinity, a selection an
  anchor and a focus, a vertical move keeping a horizontal goal. It performs
  `Command`s (moves by cluster, word, line or the whole text, visual or logical,
  deletion, insertion, transposition, select all, copy, cut and paste, undo and redo),
  takes a press of one, two or three clicks and a drag after it (by cluster, word or
  line), holds an input method's composition, and answers an input-client surface in
  a `Unit` or in bytes (`select_in` and `select_bytes`, `replace`, `set_marked`,
  `rect_for`, `index_at` and `byte_at`), its geometry laid out on demand, so a query
  right after an edit answers synchronously. It cleans every value it shows as the
  entry takes it (an editor's line breaks become LF, a field's are dropped), moves the
  caret past a cluster an insertion ends inside, and keeps a history of 100 steps: a
  run of typing or of removal is one, typing over a selection starting one, an input
  method's deletion joins the edit after it, a composition is one where it changed the
  text and none where it did not, a value written from outside is one of its own unless
  it is an empty entry's first, a write of what is shown is nothing, and a secure entry
  keeps none. A command that edits (an insertion, a deletion, a paste or a cut, a
  transposition, an undo or a redo) ends a composition first, keeping its text.
  Backspace removes the last code point of the cluster before the caret, as Chromium's
  does, an emoji sequence, a keycap or a flag whole; a forward deletion removes the
  cluster. A change to the text is proposed to the editor's `Arbiter` (`ControlledText`
  over a truth and a handler, or a platform's adapter onto `report`), an edit it
  refuses leaves no step, and a truth it answers instead is shown with the proposed
  selection mapped through it by `TextIndex::map_through`, cluster by cluster.
  `Binding::for_key` is the key table where Ctrl is the command key (Linux, Windows and
  the terminal): select all; undo and redo (`Ctrl-Shift-Z`, `Ctrl-Y`); copy, cut and
  paste (`Ctrl-Shift-V`, `Ctrl-Insert`, `Shift-Insert` and `Shift-Delete` among them);
  deletion by cluster and by word; Home and End, the whole text's under Ctrl; and the
  arrows, up and down in an editor alone, while a composing key and `Ctrl-Shift-A` bind
  nothing and a key typing one cluster inserts it.
- **A drawn entry's editing state** (`text::editing::Entry<E>`) is the shared half's,
  kept by the scene per node for a realization that draws its entries
  (`Realization::Entry`, `Realization::entry`) and dropped with the node: the editor
  over `Masked<E>` and its `MaskState`, the changes it noted since they were last taken
  (`take_changed`), the selection last reported, the operations deferred until the one
  holding the editor ends (`defer`, `take_deferred`), and the arbiter reporting each edit
  at the entry's node through `report`, its text and selection as one, the verdict
  deciding what it shows. A moved caret is reported by `report_selection` as a change the
  toolkit applied, answered with the truth to show where the verdict replaces it. The
  realization keeps only its presentation of the entry (the terminal its window over the
  value) and the keys it binds to the core's commands.

## State

An app's state lives in one of three places, chosen by who reads it and how long it
lives.

- **A `Store` for the app's data.** State that several views read and write and that
  outlives any one of them is a plain struct under `#[derive(Store)]`, made once near the
  root (`Store::tagged("todos", Todos::default())`) and handed down as the `Copy` handle
  it is. A view reads the field it shows through the accessor the derive generates
  (`todos.items()`, `todo.done().get()`), so a write re-runs exactly the reads of the
  location it touched, and a list built from a store's `Vec` follows its shape by key
  (`derive_entries(todos.items().keyed_by(|todo| todo.id), ..)`), building, moving or
  destroying one row per change. The value stays plain data, so a tagged or saved
  store snapshots it as a unit across a dev reload.
- **A local `Signal` for one view's state.** What one view owns and nothing else
  reads—a draft, a disclosure's openness—is a `Signal` the view makes as it builds,
  under its owner, so it lives exactly as long as the view and goes with it; made with
  `Signal::saved` or `Signal::tagged`, it also carries its value across a dev reload.
- **Context for what a subtree sits inside.** Values a whole subtree reads without
  being handed them—a theme's palette (`ThemedIn::current`), the router, an async
  boundary—are provided once and read by any descendant built under that owner.

There are no view models in context. A screen's data is a `Store`, or a lens onto one,
passed to the function that builds the screen, not an object a parent provides and a
descendant looks up: an argument states what a view needs where it is called, costs a
copy, and keeps the view testable with a store the test builds. Context stays for the
genuinely ambient, which every layer between would otherwise have to thread.

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
  (`derived`, `reconcile`, `Visibility`, `OneOf`) are, so a parent's structure settles before
  its descendants' properties and a disposed descendant never runs; a handle disposes or
  pauses.
- **A pluggable scheduler, flushed by turns.** Synchronous at write end by default;
  every platform holds the flush until its turn ends, through the spine's one routine
  (`hold_flushes`, `end_turn`, and `reset_thread` to leave a thread), so what a build
  notifies is delivered once the build is whole. The terminal's turn is an event, a run of due
  timers, a task pump or a render's settling round; the browser's is an event, a
  mount, a hydration or a navigation, and the next microtask checkpoint for anything
  else; the server's is its build and each round of task polls, as a batch. In the
  terminal a test's trigger is a turn and a query ends the pending one first, so a test
  sees what the loop does; `end_turn` in the browser settles a direct write where a test
  observes it otherwise, and `flush_sync` settles mid-turn where a turn needs it, as a
  key's default does after its handlers.
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
  `StoreLocation`; each node carries a `below` trigger (a write here or beneath), a
  `here` trigger (a write exactly here) and a `shape` trigger (the collection here
  gaining, losing or moving an element); a reader tracks its own `below` and every
  ancestor's `here`, a write notifies the reverse, so a row edit over ten thousand
  tracked cells costs the same as over a hundred. Field keys are a hash of the field
  name (or its `#[store(tag)]`), stable under reordering; the value stays a plain struct.
  A location computes a memo (`memo`), and a keyed location's `entries` and a vector's
  `len` subscribe to its shape alone—its own `shape` and its ancestors' `here`—so a
  keyed list follows the collection while each item follows its entry;
  `HasChildren::derive_entries` feeds them to `reconcile` keyed by the entry's key, and
  a `Vec` is keyed by a key its elements carry (`keyed_by`), read through a
  key-to-index map. The writes that change a shape say what moved: `push` wakes the
  shape and the whole-value readers and no element's, `insert` and `remove` the
  elements from their position on, and a keyed vector's `reconcile(value)` each element
  whose value changed while the keys keep their order, the shape and the moved
  positions where they do not—where a write through `update` wakes every element, since
  it could have moved any.
- **Reload state is opt-in, its identity positional, and a key anchors it.** A signal
  takes part in a snapshot only where its maker says so: `Signal::saved` and
  `Store::saved` by position, `Signal::tagged` and `Store::tagged` by name, each asking
  its type for `Snapshot`, while `Signal::new`, `signal`, `Store::new` and `Default`
  ask nothing of it and take no part. Nothing registers until something records: a
  platform calls `record_snapshots` at startup where a dev channel is present, and
  `restore` implies it, so a production build creates a saved or tagged signal as a
  plain one. A signal registers by the path of steps from the runtime's root owner,
  its index among its owner's saved and tagged signals and the hash of its type name. A
  step is an owner's position among its siblings, or the `Key` of a keyed owner: a keyed
  step takes no position, so keying a sibling shifts none of the positional ones, and a
  key is the hash of the value it was taken from (`wide_of`, which hashes a value alike
  on every target, an integer as its little-endian bytes and a `usize` as a `u64`, a
  slice of `usize` or `isize` alone hashing its elements at the target's width), so
  `reconcile`'s item key and a view's `set_key` are one concept and a key taken on the
  server leaf is the browser's. A plain signal occupies no position, and a tagged one
  is an anchor the diff aligns around within one owner. Restore aligns per owner as
  the live side is created, and every mismatch defaults, so a stale snapshot reaching
  an edited program is the ordinary case and never fails. A node
  takes a `SnapshotPlace` from the same sequence when the scene creates it, and a
  platform carries what a node holds outside the graph—focus, a text control's
  selection or caret, scroll offsets—through `provide_extra`/`take_extra` by that
  place, so a rebuilt page resumes on the same node through the same alignment, keys
  included.
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
`skene_foundation::interpolate::Motion`: the terminal on its frame clock (quantized to cells and
truecolor), which a test advances by hand, native later. A
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

Two leaves of one platform over one node record, selected by the build's cfg alone and
enforced by it: a build naming the web (`--cfg skene_platform="web"`) compiles the
browser leaf (`not(skene_server)`), on `wasm32-unknown-unknown` alone, and the server
leaf where `--cfg skene_server` stands beside it, on any target. Every item of the crate
reaching a leaf is gated on the web, so a build naming no platform, the web without
`skene_server` on another target, or a pixel platform the crate does not realize fails
with one `compile_error!` of its own and nothing else, and a terminal build compiles it
empty. The DOM's handles (`document`, `element`, the element a `NativeView` hands its
adopt handler) exist on the browser leaf alone, so the server leaf *cannot* touch the
DOM; each leaf's dependencies are keyed on the same cfgs.
What both realize is written once, in a `Realizer` over the `Sink` the DOM and the
string tree implement: the records, the element choices, attributes, classes, slots,
token definitions, control state and accessibility, so the two cannot drift; each leaf
adds only what is its own:

- **browser** (`not(skene_server)`)—the browser. DOM sink, the stylesheet
  sink, root-level event delegation with a stamped node id (one listener per event type,
  `scroll` and enter/leave included, an event a handler causes, as the `focusin` of a
  focus it moves, delivered at once inside it), one shared `ResizeObserver`, hydration
  (creation-order `data-h` markers, loud mismatch, the server's stylesheet adopted
  rather than duplicated, a control the reader changed before the client arrived held
  as the reader's, every write of its content waiting, until hydration replays what it
  shows as an applied report, an entry's edit repaired in place and any other
  content by its controller's write, a toggle's checked state read as the hydration
  begins, before the client writes any radio of its group, and convergence
  once the tree is built: every server node the client built differently or never built removed, every adopted
  element given exactly the client's classes, a native view keeping its own beside
  them and losing only the server's compiled ones, and the order-keyed resource values
  no resource adopted dropped, a native view's placeholder adopted in place where the
  door adopts that very element and removed where it adopts another), scroll and focus
  restoration keyed by a segment's end anchor, the live element behind a native view,
  the History binding and the head.
- **server** (`skene_server`)—the serializer. Renders its string tree to HTML plus
  the interned stylesheet; drives async to a fixpoint on the host executor, parked until
  a waker fires, before serializing, under a render deadline; streams in document order around `Deferred`
  boundaries with a style delta per chunk; serializes resource state for the client to
  adopt; takes client hints as render input. A native view renders its markup as its
  placeholder, taking a marker as any element does. The raw handles do not exist here,
  painted content renders empty, and handlers are inert.

A text field's secure flag and keyboard are the shared realizer's
(`FormElement::entry_type`, `keyboard_attributes`), so both leaves write the same `type`
and attributes and the server's markup is the browser's; the browser leaf takes an
`<input>` of every type they give a field (`text`, `password`, `tel`, `url`, `search`)
as a text entry, and its `clipboard_availability` disables copying and cutting in a
password input, whose selection its clipboard never reads.

Both leaves open their stylesheet with the same base (`base_stylesheet`): the reset, the
platform's colors at the root and the base slots' registrations. The platform's colors
(`SystemColors`) are each registered as a `<color>` and defined in the reset's layer on
`:where(:root)` as CSS's system colors (`CanvasText`, `GrayText`, `Canvas`,
`ButtonFace`, `ButtonBorder`, `AccentColor` and `AccentColorText`, `Field`,
`Highlight`), a hover and a pressed variant as a `color-mix(in oklab, ...)` of the
colors before it, and an engine without `AccentColor` takes a blue of its own under
`@supports not`. The root's `color-scheme` is `light`, and `dark` through the scheme
guard, the platform's preference unless the root's `data-skene-scheme` says light and
the app's choice where it says dark, so a scriptless page follows both and one write of
the attribute re-inks every system color. The rule is the realization's own text rather
than a `WebTokenValue`, which holds only a `Definition<Color>`; each leaf claims the
set's root definitions as it is installed, so nothing is laid over the rule, and a theme
redefining one defines it on its scope as any token, its registration already made.

A choice's scriptless form is keyed alike on both leaves: a radio's `name` is its
group's element id and its `value` its key in its group, and a `<select>`'s
`<option value>` is each option's key, both the member's ordinal in its `Roster`, which
the two leaves build in the same order. The server marks the truth's option `selected`,
or renders `<option value="" hidden selected>` first where nothing chosen is among the
options, and the browser's `set_choices` shows the same options, rebuilding them only
where they differ; a `<select>`'s report leaves that empty option out, so choosing it
chooses nothing. At hydration a key the reader chose is reported applied and mapped back
to its member's value; one naming no member asks nothing, and the controller's write
repairs it to the truth. The styled `Picker<T>` is served and swapped the same way: its
`<select>` takes the face's decoration, its defaults included, so the scriptless page
shows the face's fill and outline, and its drawn face and list arrive with the swap.
A styled field's selection colors are token definitions at its `<input>` or
`<textarea>`, which the entry's `::selection` rule references.

The crate's `platform` module holds each pixel platform's own vocabulary, a module per
platform: `platform::web` is the web platform itself (`Web`, its native door
`NativeView` with the `NativeMarkup` it is built from, `RenderHints`, the re-exported
`Document`, and
each leaf's entry points and seams: install, mount, hydrate, drive, the head and the
path), and `platform::{android, apple, ios, linux, macos, windows}` stand empty beside
it, a platform's data types to exist there in every build and its controls in its own
build alone, `apple` the one grouping, for what iOS and macOS share. `Platform`, at the
crate's root and in its prelude, is the build's platform as a const enum
(`Web`, `Ios`, `Android`, `Macos`, `Windows`, `Linux`): `Platform::CURRENT` answers
`Web` on both leaves, `Platform::IS_SERVER` answers whether the build is the server leaf
as `skene_server` does, `Platform::IS_APPLE` whether it is iOS or macOS, and `name` the
value the build names in `skene_platform`. The facade's entry points (`launch`,
`render_to_string`, `render_main`) stay at its root.

The app compiles to wasm for the browser and natively for the server: the server binary
renders as a plain command and, as a render worker over its stdin, serves the CLI's dev
server and generator. The two builds must agree on every byte they both produce, since
hydration adopts the server's class names: a `Color` converts through the pure-Rust
`libm` on every target rather than the host's math library, which differs by an ulp in
`powf`, `cbrt` and `atan2`, and a probe over thousands of colors rendered on the host
and on wasm proved the bits equal. One representative tree holds the two builds to it:
`server_snapshot` holds the server leaf's style block, body and state script for it byte
for byte, and `browser_hydrate`'s `the_server_leafs_bytes_hydrate_in_place` hydrates
those same bytes in Firefox and Chromium, every served element adopted with the class
and the inline declarations the server gave it and the server's block the one
stylesheet, holding every rule the browser interns.

### Native (planned)

Each OS joins the pixel facade as a platform under its cfg (`ios`, `android`, `macos`,
`windows` and `linux`), the facade `skene-web` today and `skene-gui` once item 1 splits
it: its realization a crate of its own compiled in that platform's build alone, its own
vocabulary for its controls, structure and navigation in the facade's `platform::<p>`
module, and an app's pixel shell one crate built once per platform it declares, where
`skene-web` refuses each of these builds today with a `compile_error!` naming the
platform. Where the OS composites views (iOS, Android, macOS) a shell realizes the tree
into the OS's own view hierarchy, a box a view with its layers, and where it composites
windows alone (Linux, Windows) it draws the whole window. Every text is laid out and
drawn by the OS's own engine behind `TextEngine` (TextKit 2, `android.text` over
Minikin, DirectWrite, and on Linux `skene-parley` over the system's fonts), and a text
entry is Skene's own field over the editing core, speaking the OS's text-input protocol
itself and reporting its text to AccessKit from the core; the OS's own controls stay
reachable as its own. No realization is a hybrid of drawn content around a toolkit's
controls inside a window Skene draws. Layout is taffy, the OS's measure its leaves; the
resolution cache tells a shell whether a layout-affecting value moved, so a palette
write relays out only when it must. Off-thread work uses the engine's scheduler wired to
the event-loop proxy, and a loop-integrated timer is fair game there, the loop being the
shell's. The plan, its prerequisites and its order are in `ROADMAP.md`.

### The terminal (`skene-terminal`)

A facade in its own right, not a platform of the web's vocabulary. Its views are the shared
ones typed to the terminal: lengths are `i32` whole cells, an `f32` rounding to the
nearest cell at the setter (`1.5` is two) and a margin or an inset keeping its sign,
laid out as layout takes any length below zero, while a run's font size keeps its
fraction of a cell, and so do a corner radius and a shadow's offset and softening,
which the backdrop paints in pixels and the cells take in whole cells; text is a color plus a `bool` bold, a border is one cell of
box-drawing characters on each edge its width names, in its own color, its conditions
are cells and its own background. Nothing the medium lacks exists to call, so a
terminal app is first-class at compile time rather than best-effort at run time.

The terminal's build names it, `--cfg skene_platform="terminal"` on the host's target:
the crate, its dependencies and `#[main]`'s `main` exist there alone, a build naming no
platform fails with the crate's `compile_error!`, and every other platform's build
compiles it empty, as it does `skene-simple-terminal`, `acme-sparkline` and the terminal
examples, each gated on the same cfg.

Its `platform` module holds the terminal's own vocabulary as the web's holds each pixel
platform's: `platform::terminal` is the terminal platform itself, `Terminal`, the
`CellPainter` escape trait and the `Cells` it paints through. `Platform`, at the crate's
root and in its prelude, is the build's platform as a const enum of one variant,
`Terminal`, with `CURRENT` and `name` alone, since the pixel facade's leaves and Apple's
grouping have no terminal counterpart. `Launch` and its options, `launch`, `install`,
`mount` and `uninstall` stay at the root.

The loop runs on a `Device`: bytes in and out, the window's size, a wake, raw mode, and
notes for what the device learns besides input (a request to end, a job control stop
and continue). The loop owns the decoding—the parser, the escape grace, the reply
windows—so a device carries bytes and nothing it must understand. `launch` runs on the
process's own terminal (the tty, with its signals); `Launch::run_on` runs on any other,
and `testing::Pty` is one: a pseudo-terminal whose far side a test types into and reads
the drawn screen from, so the real loop, raw mode and all, runs end to end under test.
The dev launcher's control pipe is the dev module's, read after each wait, never the
device's.

The mouse is in-capability: motion drives `Hovered` variants and pointer handlers, a
left click hits the topmost node drawn at its cell, content overflowing a box taking it
unless the box clips its overflow, at its padding box, and runs its handlers through
the spine's dispatch, a
hovered node's `cursor` sets the pointer shape via OSC 22, with the shapes a browser's
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
hit-testing stays by cell. The pointer is the primary mouse, its back and forward
buttons `X1` and `X2`, and a context menu the keyboard opens names none; the mouse key
cancels a press in progress. The window's size is event-driven: the platform's at launch,
then every resize notification's, and the loop sets in-band resize (mode 2048) and
queries it, so a terminal that confirms it reports every resize in the input stream
(`CSI 48 ; rows ; cols ; height ; width t`) with the pixel size the pixel mouse needs where the platform's own
notification carries none, and a notification without pixels keeps the known ones
while the mode is confirmed. Launch queries synchronized output (DECRQM 2026) the same
way, and from a reply that says the mode can be toggled every frame is written between
`CSI ? 2026 h` and `l`, so a redraw never tears; a terminal that does not know the mode
gets plain frames. Grapheme clustering (mode 2027) is set and queried at launch the
same way: text is measured in extended grapheme clusters, each as wide as the terminal
draws it (`unicode-width`)—the cluster's width where the terminal confirmed the mode,
the sum of its code points' widths where it did not, so the layout agrees with the
terminal either way. Text is laid out by the terminal's engine, `skene-parley` over a
typeface the build generates whose every code point is a glyph as many cells wide as it
draws on its own (a cell an em, its ascent the whole cell, the dotted circle unmapped
so no shaper inserts one), a tab's stops a cell apart and a tab laid out as the one
blank it shows as, at its run's scale; under the mode a cluster whose code points sum
to another width is laid out as a stand-in of its width (U+2060, U+FF71 or U+4E00)
through the foundation's substitution, so every position the engine reports is a byte
of the real text. Each text leaf is one paragraph of its runs and inline boxes (a
`TextFlow` over the engine), an inline box a box as many cells wide and rows tall as
it, kept between frames and measured, painted, hit and selected from its one layout, a
width floored to whole cells, at least one: a run wraps and keeps its blanks as the
guaranteed core says and draws cluster by cluster in the engine's visual order, and a point
on a cell is probed just past its left edge, so a press lands at the nearer edge of
the cluster drawn there, after a wide one on its second cell. So that a terminal
reordering bidirectional text itself does not reorder it twice, launch asks for the
BiDi mode (ECMA-48's BDSM, mode 8, through `CSI 8 $ p`) and switches it to explicit
(`CSI 8 l`), and the hand-back restores the mode the first reply reported, writing
nothing where none came: a reply while one is kept, as a take-over after a stop the
process did not hand back for brings, is of the mode launch set. A cell holds a cluster (`Glyph`: one code point, an interned cluster,
or the continuation of a wide one, which paints nothing) while a text entry's caret
and selection are reported in clusters and its window counts cells, settling by cells
around the caret. A confirmation arriving after the first frame lays every text and
entry out again and the frame afresh. Text sizing (kitty's
OSC 66) is the same kind of enhancement: the terminal's `Typography` has a `size`
token, `1.` by default, and `set_font_size` on a run is the shared type metric in
the medium's unit, so `2.` is a heading two cells tall. A size rounds to a whole
scale of one to seven, and a size
below one is a fraction of a cell (`n/d`, `d` at most fifteen) drawn at the cell's
foot. A scale is the run's size in the engine, a whole scale of `n` a run of `n` rows
a line and `n` cells a column and a fraction a run of one row, so a scaled cluster
takes a block of `scale` rows by `scale` times its columns: wrapping counts the scaled
columns, a line is as tall as its tallest cluster and the smaller clusters sit on its
bottom rows as on a baseline, a run measures to its lines' widths and the sum of their
heights, and a hit at a cell lands at the nearer edge of the block holding it, so
selection, links and inline boxes follow. In the cell
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
survives reflow; wheel gestures latch their target. A text entry is the editing core's
first client: its editing state is the shared entry the scene keeps for its node
(`Entry<CellEngine>`, its `Editor` over the cell engine, a field one unwrapped line and
an editor wrapped at its content width, a secure entry's mark `•`), and the terminal's
`Field` record keeps only its presentation over it—the window over the value and the
area it was settled in, the placeholder, the lines it asks for and its widest
line—reading the entry through the scene. So an operation on the editor—a key, a press or
a drag, a paste, a write from the app—runs with neither the records nor the scene
borrowed and what it changed (damage, restyle, the window, a moved selection to
report) is settled after, an operation reaching an editor another holds (a handler
that moves focus, say) deferred on the entry until that one ends. Its box its style leaves unsized lays out at
its native size, eight cells or its value's widest line and the caret's cell past it,
and its lines tall, as an `<input>`'s intrinsic size: its content size, so the basis it
grows from beside a growing sibling, and the least a layout stretching or shrinking it
leaves it, held to its own maximum and raised to its own minimum (its size
outright where either is a fraction, which then bounds it). The value draws on the box's
first row, where a browser centers it in a taller `<input>` (`ROADMAP.md`). Its edits
are reported through `report`, never applied ahead of the verdict, which it shows;
it takes its read-only state from the scene before each operation, and a key command
lays the scene out first (`lay_out`). A secure field's `Secure` content masks its
engine (`MaskState::set_masked`) and its editor (`Editor::set_secure`, which keeps no
history and offers no copy or cut), so it draws one `•` a cluster, a wide cluster's
mark one cell, its natural width its marks', its access runs and value read the marks
and its role is a password input; its selection copies nothing and its `Copy` and
`Cut` are disabled, and its `Keyboard` content is ignored.
Its selection is the core's—an anchor beside the caret over the value,
reported after every move as a change of the `Edit` it shows, applied, its selection a
`TextSelection` in clusters, and written from its bound signal, each index clamped into
the value—so keyboard focus selects a field's
value, `Shift` with the caret keys and the mouse extend it (a drag held past the edge
scrolling the window on the frame clock, as a browser's does, and a drag after a
double press extending by words), an edit replaces it, and it is drawn cluster by
cluster where the layout draws them. `Ctrl-Z` and `Ctrl-Shift-Z`/`Ctrl-Y` undo and redo
through the core's history, as an `<input>` keeps one (a run of typed or of removed
characters is one step, a caret move ends a run, a value written from the signal is a
step of its own, the initial one aside, and an edit the handler rejects is none). An
entry's keys go through the core's table (`Binding::for_key`) after the legacy `Ctrl-W`
and `Ctrl-H`, which delete the word and the cluster before the caret, and its clipboard
keys (`Ctrl-C`/`X`/`V`, `Ctrl-Insert`, `Shift-Insert`, `Shift-Delete`), outside an entry
the document's copy, act only where the launch offers the operation, `Shift-Delete`
deleting forward where cut is not offered; `Ctrl-Shift-C` and `Ctrl-Shift-X` do
nothing and `Ctrl-Shift-V` pastes, as in Chromium. Those keys and the standard context
menu copy, cut and paste through the loop's OSC 52
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
and focus order, exactly as on the web, to activatable rows, which the scene's key
defaults activate as the web's do, and to link runs, which sit in the tab order as
an anchor does: `Enter` is a focused link's click, staged for the system handler unless
a handler prevented it, a press on its cell focuses it, and while its focus shows it
paints in the typography's focused-link ink and surface, reverse video where those are
transparent, as the default leaves them. Focus shows as `:focus-visible` does: after a
key, or in a text entry, and not since a pointer press. A window opens with nothing
focused, and the control set to autofocus that comes first in the scene's child order
takes focus as the first mount ends, without showing it. A key pressed while nothing
is focused is dispatched at the root, so a control's shortcut acts from anywhere, and
the access tree reports the shortcut as the AccessKit node's keyboard shortcut. A key's
default runs after the handlers' writes have taken effect, as a browser's does after its microtask checkpoint. A key release
dispatches as `KeyUp` (`handle_key_up` on a box and a control) with no default but the
scene's (`Scene::key_up_default`, a row's `Space`), a held
key repeats its `KeyDown` with `repeat` set, and a modifier pressed alone arrives under
its W3C name (`Shift`, `Control`, `Alt`, `Meta`) without showing focus; the terminal
asks the kitty keyboard protocol for `REPORT_EVENT_TYPES`, `REPORT_ALTERNATE_KEYS` and
`REPORT_ALL_KEYS_AS_ESCAPE_CODES` beside the disambiguated codes, and a terminal
without them reports presses alone, as the legacy encoding does. A `KeyEvent`'s
`key` and `code` are `keyboard-types`' `Key` and `Code`, the W3C values typed: `key`
is `Key::Character` for the text a key types and `Key::Named` for every other key, and
`code` the physical key (`Code::KeyA`, `Code::Digit1`, `Code::ShiftLeft`,
`Code::Numpad7`), so a shortcut on it holds across keyboard layouts: the browser's
`KeyboardEvent.code`, a value the crate does not know arriving as `Unidentified`, in
the terminal the kitty protocol's base-layout alternate where it reports one and the
key's own position on the US layout otherwise (`KeyEvent::code_of`, over the crate's
WebDriver table), and a shifted
alternate the terminal applied restores `Shift` for any key, so `Shift-1` reaches an
app as `!` with `Shift` held. A modal takes
focus to its first tab stop as it opens and returns it to the node that held focus as
it closes, a popup closing with focus inside returns it the same way, and a press on
the anchor a popup opened from is the anchor's click rather than a dismissal. An editor is a
scroll target like a box: the wheel, its scrollbar and the page keys move its window
over its wrapped rows without moving the caret, a caret scrolled out of view hides until
focus or an edit reveals it, a caret at the end of a full row stands at the area's
edge, and the horizontal wheel moves a field's window along its value. A release ends a drag on the node the press captured wherever the pointer is. A
childless, borderless box paints to eighth cells from its parent's origin, unrounded:
whole cells take its color, and a cell one of its edges crosses takes a block glyph
carrying its color on the box's side and the color already there on the other, so a
slider's fill and thumb move by eighths, meet where the layout put them whatever the
parent's own offset, and a seam between siblings is exact. A box's scrollbars and
shadow are its own presentation, set on the box as the web's `scrollbar-color` and
`box-shadow` are: the thumb's and the track's colors (`set_scrollbar_thumb`,
`set_scrollbar_track`; the foreground and a quarter of it over the cells beneath where
unset), their glyphs (`ScrollbarGlyphs`: a full-block thumb moves by eighths, any other
by whole cells), and the shadow's offset and softening (`set_shadow_offset`,
`set_shadow_softening`, half a cell each where unset): the cells take the shadow at its
offset rounded up, and the pixels place it exactly and soften its edge.

The terminal itself is spoken to through three modules of `skene-terminal`, with no
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

The platform's colors (`SystemColors`) are defined at the root from the same replies as
the platform is installed, and defined afresh as a reply arrives or the app's scheme
choice moves (`define_system_colors`, definitions equal to those in force changing
nothing): the ink and the window are the colors an unset cell shows, the accent the
reported blue of the base colors under the lighter or the darker of the two, whichever
stands out on it, and the rest mixes of the ink into the window, white on black for the
dark scheme the terminal assumes until it reports its colors. A cell painted in the
terminal's own reported foreground or background, where the scheme in force is the
detected one, is written as the cell's default color (`SGR 39` and `49`), so a 16-color
terminal keeps its own; the cells themselves keep the color, so a frame reads the same,
and the frame is written afresh as the terminal's own colors move.

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
text through its `TextFlow`, and a view inside it flows as an inline box: laid out on its own, standing in the
text as a placeholder of its width, painted and hit at the cells it flowed into. An
edge whose border width is zero takes no cells and draws none, and a border whose color
is unset draws in the terminal's own ink, as CSS's `currentColor` does. Frames paint into a cell buffer—to the terminal
through `launch`, or to plain text
through `testing::render_to_text` for in-process tests (`trigger_key`/`trigger_mouse_move`/
`trigger_mouse_click`/`trigger_frame` in `testing` drive it without a terminal). Terminal detection is
asynchronous; the loop starts immediately and the answers arrive as signal writes. The
dev loop carries the snapshot across a swap through the launcher's control pipe. Terminal
apps never cross-compile into the web.

## Third parties and platforms

Two guarantees, both properties of what is public rather than of any one API.

- **A component library: one core, a crate per platform.** Its core depends on
  `skene-foundation` and holds what both media share—its colors as a token set, its
  named sizes per unit, its logic—and names no realization. Each platform's crate writes
  its presets on its facade's vocabulary, each a function returning the facade's
  built-in styled from the core (`SimpleButton::outline(label)` returns a `Button`),
  with the pieces it composes public (`SimpleButton::styled`), and, where a preset is not
  enough, concrete views of its own, as each facade writes its built-ins; no public
  signature in it names the platform's realization, nothing in it is generic over one,
  and where the facade's own surface still asks for the realization (a base trait's
  bound, a part recorded in the scene, a wrapper erased into a flex child) it names it
  as the facade's `Active` alone. Nothing on the path a
  built-in takes is crate-private: the setter macros the
  built-ins generate their `set_`/`derive_` pairs with (`properties!`,
  `trait_properties!`) and the surfaces they share (`DecorationBase`, `RunBase`) are
  public, so a third-party layout view built on `LayoutBox` spells its surface the way
  `Row` does. A line of either macro may start with `[cfg(<predicate>)]`, a platform's
  own property of a shared view: its pair is generated in every build with one
  signature, writing its field where the predicate holds and dropping the value in every
  other build, so the field need exist only there. `extension_fields!` takes the same
  `[cfg(..)]` on a property of a medium's extension, gating its field type and its
  visit, so with the field and the property's variant gated alike the record entry
  exists in that build alone. The macros' text names no platform; the predicate is the
  declaring crate's. Simple is the standing proof: `skene-simple` its core, naming no
  realization, and `skene-simple-web` and `skene-simple-terminal` its platforms' crates of
  presets, written exactly as a third party's. `skene new lib`
  creates the core-plus-presets shape: a core holding a palette and a unit trait and
  naming no realization, and a crate per platform named whose badge is a preset, an
  uninhabited namespace whose constructors return the facade's `Container` styled from
  the core's tokens and its unit's sizes, defining no default at the root.
- **A low-level view, against the platform, like a built-in.** It is written against a
  platform's escape trait or through its native door and realized through the same scene
  and node ids as a `Container`, so it composes with wrappers, themes, conditions and
  events. A view of a platform of its own is concrete, as a built-in is:
  `acme-sparkline`'s `Sparkline` is a `View<skene_terminal::Active>` with no parameter,
  building a sized `Container` and handing the platform its cell painter
  (`<Active as CellPainter>::paint`) and its pixel painter
  (`<Active as Painting>::paint`) for that one box, so an app writes `Sparkline::new(..)` and names no platform. `acme-map`'s
  `Map` is likewise a `View<skene_web::Active>`: a `NativeView` canvas inside a sized
  `Container`, its markup served as is and painted on the browser leaf once its element
  is in hand, so the web's two leaves are one view with a `cfg` only around the
  painting, inside the library. Selecting the platform is what selecting the facade
  already does. A platform's escape traits live in its facade's `platform::<p>` module:
  `skene_terminal::platform::terminal::CellPainter`, which has no `Realization`
  supertrait, so a failed bound states its one rule, that cells are painted on the
  terminal alone; the web has none, its low-level views going through its native door.
  `acme-map` (a canvas through the web's door) and `acme-sparkline` (bars through
  `CellPainter`, and as pixels where the terminal proves its graphics) are the standing
  proofs. The portable low-level path for pixel media is `Painted`.
- **A compositor platform's own view, through the native door, like a built-in
  control.** `LowLevel::adopt` makes the view an ordinary scene node, and where it holds
  a value the library registers a `ControlSink` of its own on the node, keys the value
  by `ContentKey::door(name)` with a value type of its own (`ContentType` and
  `Controllable`), controls it with the same `Controller` a toggle's checked state has,
  and reports what the reader did through `report` with whether the view applied it. The
  value is then controlled as a built-in's is: the truth written when it changes and not
  when the view shows it already, a reader's change asked of the app, and a refused one
  repaired to the truth by one write through the sink. The browser leaf adopts a live
  `Element`, the server leaf the `NativeMarkup` it renders as a placeholder, and the
  terminal nothing. The door's view on the web is `platform::web::NativeView`, built
  from a tag and its attributes: the server renders them as its placeholder, and the
  browser creates the element from them or, while hydrating, adopts the served
  placeholder in place, writing each attribute its markup gives another value, the door
  taking the next hydration marker as any element does (`Page::claim`), so the rest of
  the page hydrates with no mismatch. On the browser leaf `handle_adopt` hands it the
  element with its node, where a library draws, listens and registers its sink; the
  browser tests adopting a color input under the contract, as a `NativeView` and as an
  element of a test's own (`browser_views`), are the door's standing proofs.

A platform need not be first-party: a crate that implements `Realization`, provides its
scalar, leaves, extension, a `launch` and views over `skene-foundation` is a facade.
`Realization` is not sealed, and the shared machinery bounds only on it and the public
extension traits.

## Selection and entry

**A build names its platform by a cfg**, `--cfg skene_platform="<p>"` with `<p>` one
of `web`, `ios`, `android`, `macos`, `windows`, `linux` and `terminal`: a build fact
like the triple, chosen once and unifying nothing, as `getrandom` names its backend. No
build script infers a platform or a leaf and no cargo feature selects one. The web's
two leaves are one platform: the browser leaf is the `web` build on
`wasm32-unknown-unknown`, and the server leaf the `web` build with `--cfg skene_server`
beside it, on any target. Selection is by facade dependency first, a web app depending
on `skene-web` and a terminal app on `skene-terminal`, and the cfg selects within the
facade. `skene-web` compiles under `web`, compiles empty in a terminal build, and
refuses every other build with a `compile_error!` of its own: one naming no platform,
the web without `skene_server` off `wasm32-unknown-unknown`, and each pixel platform it
does not realize yet, named. `skene-terminal` compiles under `terminal` on the host's
target, refuses a build naming no platform, and compiles empty in every other
platform's. Every root item that reaches a leaf is gated on its leaf's cfgs, so each
refusal is its build's one error, which the gate's `selection` step holds. A crate
reaching a facade (its Simple crate, a low-level view's, an example) is gated on that
facade's platform, so a build of a whole workspace for one platform compiles the other
facade's crates empty, and one naming no platform fails with both facades' messages.
Target-gating stays the enforcement layer where it applies: each leaf's dependencies
are keyed on its cfgs, so the server leaf cannot reach the DOM. A cargo feature is an
opt-in upgrade that enables optional dependencies alone, never a bound, a type, a
variant or a platform, and none is on by default: `skene-web`'s `gpu` is the one there
is.

**`Platform` answers the build's platform as a const.** Each facade's `Platform`, at
its root and in its prelude, has `Platform::CURRENT`. The web's lists the pixel
platforms (`Web`, `Ios`, `Android`, `Macos`, `Windows`, `Linux`), `CURRENT` answering
`Web` on both leaves, `Platform::IS_SERVER` the server leaf and `Platform::IS_APPLE`
Apple's two; the terminal's is `Terminal` alone. An app that diverges per platform
matches on it, writing `#[cfg(skene_platform = "<p>")]` only around an arm that names
a platform's own control. A platform's own vocabulary lives in its `platform::<p>`
module, its data types in every build and its controls in its own: `platform::web`
holds each leaf's realization and the web's native door, `platform::terminal` the
terminal's realization and escape trait, and
`platform::{android, apple, ios, linux, macos, windows}` are empty until their
platforms land. `#[main]` and `#[lazy]` expand per leaf on the same cfgs, so a server
build never emits the browser entry or splits a lazy body, the terminal's `#[main]`
emits its `main` in the terminal's build alone, and an entry taking arguments,
parameters or generics is refused.

**A platform's own property of a shared view is a prefixed setter.** Its pair
(`set_linux_*` with `derive_linux_*`, `set_ios_*`, `set_web_*`, and `set_apple_*` for
what iOS and macOS share) comes from the same `properties!` or `trait_properties!`
block on the facade's type as the shared ones, its line prefixed by
`[cfg(skene_platform = "<p>")]`, or by
`[cfg(any(skene_platform = "ios", skene_platform = "macos"))]` for Apple's. The setter
compiles in every build and does nothing in another platform's, its documentation
naming the build it acts in; its backing field, its property's variant of the
extension's property enum and the record entry it feeds are gated to the platform, so a
realization reads the entry only where it exists. No facade ships one yet: the arm's
proofs are the foundation's unit test of a property under a predicate that holds and of
one under a predicate that never does, naming a field no build has, and the third-party
gauge on `properties!`, a doctest every build of the workspace compiles, with Linux's
setter inert in each.

**An app's configuration names one platform for plain cargo.** `skene new` writes one
`.cargo/config.toml` at the project's root naming the first platform it is given:
`[build] target` its triple (`wasm32-unknown-unknown`, or `host-tuple` for the
terminal), its cfg in `[target.'cfg(all())'] rustflags`, which reach what is compiled
for the target and never a build script or a proc macro, and in `build.rustdocflags`,
which doctests see, and for the web its test runner, `wasm-bindgen-test-runner`, which
`skene new` refuses to name where it is not installed. So `cargo check`, `cargo test`,
`cargo clippy`, `cargo doc` and rust-analyzer work at the project's root with no CLI.
Each shell's crate root is gated on its platform, so the whole project builds under
each platform, and the project's workspace declares both cfgs in its `check-cfg`, which
every crate of it inherits, so an app's code and its entry's expansion name them
without a warning. A project made from a checkout patches the crates the checkout
patches and starts from the checkout's lock. The gate's `scaffolds` step writes the app
and the library scaffolds from this checkout, both naming the terminal and the web, and
holds each clippy-clean with warnings denied in every build of the platforms they name,
the browser leaf, the server leaf and the terminal, their doctests passing in the two on
the host, so a scaffold cannot fall behind the surface it teaches. The terminal's build
is plain cargo, the project's configuration alone naming its platform, and each other
build names its platform in `RUSTFLAGS` and `RUSTDOCFLAGS`, which outrank the project's
configuration, since a target's flags would join the project's and name two platforms.
Each build has a target directory of its own, and once a project's lock has taken its
own crates every build runs locked and offline.

**The CLI builds any platform by substituting the cfg where cargo reads it**
(`packages/cli/src/flags.rs`). It reads the flags cargo would use for the build's
target in cargo's own order: `CARGO_ENCODED_RUSTFLAGS`, `RUSTFLAGS`,
`target.<triple>.rustflags` followed by every matching `target.'cfg(..)'.rustflags` in
the order of their keys, a table matched against the target's cfg as rustc prints it
under the flags found before it until they settle, then `build.rustflags`, and rustdoc's
the same with `rustdocflags` and no `cfg(..)` table, over each `.cargo/config.toml` from
the directory cargo runs in up to the root and `$CARGO_HOME/config.toml` beneath them,
merged as cargo merges them, `host-tuple` resolved to the host's triple. It replaces `skene_platform`'s value where
it stands, keeps, removes or adds `skene_server` beside it, appends both where the
flags name no platform, and hands the result to cargo as `CARGO_ENCODED_RUSTFLAGS` and
`CARGO_ENCODED_RUSTDOCFLAGS` with the target named. A build for the platform the
configuration names therefore keeps its flags byte for byte and recompiles nothing
plain cargo compiled. What it cannot read exactly is a refusal naming it, never a
guess: a configuration `include`, the `CARGO_BUILD_*` and `CARGO_TARGET_*` variables
cargo merges with the configuration, a `cfg(..)` table naming a platform cfg, one it
cannot evaluate and one whose flags never settle, an array merged with a string, and a
platform cfg in a form it does not know.

**This repository names no default platform.** Plain cargo at its root builds its
default members, the crates reaching neither facade, which the gate's `manifests` step
holds the list to. Its cargo aliases (`cargo check-web`, `cargo check-server`, `cargo
check-terminal` and their `clippy`, `doc` and `test` kin) carry each platform's cfgs and
triple, each over the whole workspace in the target directory the gate builds it in, and
the checked-in rust-analyzer settings (`.zed/settings.json`) name the browser leaf:
`cargo.target` its triple, and the cfg in `cargo.extraEnv`'s
`CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS`. The guide's contributing chapter
teaches them. docs.rs documents each leaf of `skene-web` and `skene-simple-web` from
their metadata's `cargo-args`, which name the browser in `wasm32-unknown-unknown`'s
rustflags and rustdocflags and the server leaf in `x86_64-unknown-linux-gnu`'s, and
`skene-terminal` and `skene-simple-terminal` from theirs, naming the terminal in
`x86_64-unknown-linux-gnu`'s.

A facade is the one place allowed to name its platforms, which is why the shared
platform-adjacent surface lives there: `launch`, `#[main]` (per-platform entry glue),
`spawn`, `schedule` and `cancel`, `Deferred`, `client_only`, `media`, and the
interactive-only wrappers. App code names no platform, and a facade re-exports no
`Realization`: a low-level library or a test that implements or calls the seam takes it
from `skene-foundation`.

## Async, data, and third parties

HTTP, WebSockets and timers are **userland**, deliberately (the browser owns the loop on
the web; owning an HTTP stack is permanent debt). The framework ships only the glue no
external crate can provide, and ships it as *public seams* so third-party integrations
sit on exactly the footing first-party ones would:

- `spawn`—resume on the UI thread, scope-bound cancellation.
- `schedule`—a one-shot action on the platform's clock, a free function of each facade
  beside `spawn` over the realization's own, cancelled by passing the `Timer` it returns
  to `cancel`: the window's timeout on the web, the loop's wait in the terminal, fired
  under a test from the hand-stepped clock the test advances, and never on the server. A
  `sleep` future is the server leaf's alone (`platform::web::sleep`).
- `Resource`—in the engine: load into a signal with a `Loading`/`Ready`/`Refetching`
  state that keeps the last value; the web facade's newtype adds hydration across the
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
waker, so a terminal app's loop or a parked server render polls it on. Dropping the
future abandons the exchange. An app drives it through the seams it already has:
`Resource::client` for data a view shows, `spawn` for a task of its own. `skene_transport::ws`
is the WebSocket sibling on the same seams: `connect` is a future resolving to a
`WebSocket` once the connection stands, `send` writes a message at once, and
`receive` is a future a task awaits in a loop, each message as it lands and `None`
once the socket has closed; the browser's `WebSocket` on the web and a thread around
`tungstenite` on every other host reading into the same receiver, a server render
included, so a page whose data arrives over a socket renders with it. A socket opened
during a render lives for that render, as a fetch does. Dropping the socket closes it.

## Router (`skene_foundation::routing`)

Routing is state: a typed route as a signal, shared by every platform. Each platform binds
it thinly—the browser leaf to the History API (wrapping a navigation in a view
transition when the app opts in, through `Router::set_commit`), the server leaf takes
the route as render input, the terminal keeps it in memory. SSG enumerates the route
table (`StaticRoutes`) to know what to render. The router depends on the engine alone.

## SSR, SSG, and the server binary

Server-side rendering is a *capability*, not a server: `render_to_string` /
`render_to_stream` produce the parts (styles, body, state) or ordered chunks, and
`Rendered::page` assembles a document. One template, `skene_dev::Document`, which
`platform::web` re-exports, writes every page, the CLI's client-rendered shell among
them, so the CLI depends on no facade: the head preloads the client bundle, the body
ends in the hydration state and the module script booting the bundle, a page in the dev
loop carries the dev marker and the dev script, and every inline script carries the
render's nonce. A head registration (`add_meta` and its kin) holds while the
owner that made it lives, in one registry the web leaves share,
`skene_foundation::web::Head`: the latest live element of what it stands for is in
force, so a route's head leaves with the route; the server serializes what is in force,
and the browser applies it to the live head, adopting the elements a server render
left. The title is the window's (`window::set_title`), shared with the terminal: the
latest live one is in force, and each platform shows it, the web in the head. The app's server binary is built natively with the
web and its server leaf named (`skene build web --static` and `skene run web --ssr` do
it, for the host target alone so build scripts and proc macros never see the cfgs; by
hand, `RUSTFLAGS='--cfg skene_platform="web" --cfg skene_server' cargo rustc --lib
--crate-type bin --target host-tuple`, since `RUSTFLAGS` stands above the target
rustflags an app's configuration sets and beside `--target` reaches no build script or
proc macro), and
`#[main]` makes it a plain command booting its own crate's bundle: `app /path`
prints the page, `--routes` the enumerable routes, `--stream /path` the page as it
flushes, and `--serve` renders one length-framed request after another read from
stdin—the path, then the hint headers and the nonce—so a user's server runs it as a
command or embeds `render_to_string` directly, hands each request's headers to the
render as `RenderHints` and names the response headers that request them. A render occupies the thread it runs on, since the runtime and the scene are
that thread's, and starts from a clean one every time. SSG is the same binary run at build
time by `skene build web --static`; the CLI's dev server (`skene run web`, `--ssr` to
render per request through a `--serve` worker) is where the development conveniences
live. The terminal splits the same way: `skene run terminal` keeps the build loop and its
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

Framework and app code log through `tracing`. The web leaves install a subscriber at
launch unless one is set, the browser's writing to the console and the server's to
standard error; the terminal installs none, its screen being the app's. The spine carries no subscriber.

## Glossary

The words this codebase uses in more than one sense, with the phrase that keeps each
sense apart where context does not.

- **Leaf.** A property's leaf (`Leaf`, `StaticLeaf`, `KeptLeaf`): the fixed value,
  token, expression or derived closure at the bottom of a `Value`, conditions choosing
  among leaves. A web leaf: one of the platforms the web facade compiles to, the browser
  leaf and the server leaf.
  A layout's leaf: a node taffy measures through its platform rather than from
  children (`skene_layout::Leaf`), a text's being a `TextFlow`, measured through the
  foundation's `TextLeaf`.
- **Paragraph.** A text engine's paragraph (`text::Paragraph`): an attributed text
  shaped under a paragraph style, laid out per width. A document's paragraph: a box
  whose children are all inline runs (`RichText`), laid out as one `TextFlow`, one
  engine paragraph.
- **Layout.** The layout: taffy's placing of boxes (`skene-layout`'s `Tree`), or the
  web's CSS. A text's layout (`text::Laid`): one paragraph laid out at one width, its
  lines, carets and rectangles, immutable once made.
- **Editing core.** Skene's one editor (`text::editing::Editor`) every drawn entry
  runs on, over its platform's engine; a platform's own text field, where one exists,
  is a platform control, and the web's entries are its `<input>` and `<textarea>`.
- **Anchor.** A segment's anchors: the start and end markers a reactive subtree
  (`derived`, `Suspense`, `ErrorBoundary`, `Deferred`, `AsyncView`, `empty`) mounts
  between, a comment pair on the web. An overlay's anchor: the node a popup is placed
  against and toggled from, placed by CSS anchor positioning where the engine has it.
- **Store.** A `Store`: the reactive state a `#[derive(Store)]` struct lives in, read and
  written through its fields' accessors. A snapshot's store and restore: what a dev
  reload records and reads back. A platform's record store (see record).
- **Record.** A platform's record of a node: what it keeps per `NodeId` beside its
  realization, the terminal's. A snapshot record: what a run writes for
  its successor at a dev reload. A node's style record (`Record`): its box and run
  styles and their motion as one erased list under Skene's property names (The style
  model).
- **Realize.** A platform turning a scene node into its own representation: an element
  in the browser, an entry in the server's string tree, a record and a taffy node in the
  terminal. `LowLevel::realize` mints a node a view realizes itself.
- **Build.** `View::build`: a view creating its scene nodes and returning its root's
  id. Compiling is never building, and `skene build` is named in full.
- **Floor and upgrade.** What a platform always has and what a signal proves it can
  add, the platform choosing: the terminal's backdrop in cells is the floor and in
  pixels the upgrade, sixteen colors the floor and truecolor the upgrade.
- **Fallback, baseline, enhancement and child.** Where a view has two forms the view
  chooses: its fallback (`set_fallback_builder`) is the baseline that works before the
  environment is ready, and its child (`set_child_builder`) the enhancement the proven
  environment gets. `Suspense` and `ErrorBoundary` call what they show in place of their
  content a fallback too.
