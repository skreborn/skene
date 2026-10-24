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
they draw, on those two, the Simple core on the foundation, each platform's Simple crate
on its facade, and the macros, the transport and the CLI beside them.

**A facade per platform, one spine.** The web and the terminal are different media, not
one medium with fallbacks: an app is written for each on purpose, and neither
cross-compiles into the other. Each facade therefore owns its builder vocabulary outright, typed to
exactly its capabilities—and everything beneath it is written once in the spine. The
facade's own contribution is small by design: a scalar, the medium's condition leaves,
the properties only that medium has, the platform it selects, and a type of its own per
view, a newtype over the foundation's, that gives app code a flat, medium-typed surface.
The newtype derives `Wraps`, which gives it `Apply`: `.apply(option, Type::set_<prop>)`
sets a property only where a value is given. What a medium lacks is absent at compile
time because the platform's associated types say so.

Dependency arrows only point down; a platform depends on the foundation, never on another
platform; each facade names only its own platforms, and publishes the selected one as
`Active`. A crate reaching neither facade names no platform and builds in every build,
plain cargo at the root among them; one reaching a facade builds in that facade's
platforms' builds and compiles empty in the other's.

## The scene and the platform

**The spine owns the tree; a platform keeps a record per node.** `Scene<P: Realization>` is a
generational arena of node records—parent, children as a list of linked siblings (a move
relinks a few records and shifts nothing), the annotation a wrapper asked for the child,
the token definitions a theme placed on the node, the query-container name, the
handlers. The scene mints every `NodeId` a node holds (`NodeId::new` builds one from its
index and generation, as the layout engine's tests key their canned trees, and one no
live node holds resolves to nothing in the scene), and the platform keys its own record of the
node by that id (a `NodeMap`: an element, a string-tree index, a taffy leaf), realizing
it once the scene is released and before the node takes part in anything else. Every
structural mutation (`attach`, `detach`, `destroy`, reorder) is implemented once in the spine;
the platform is called only to mirror it into its records, after the spine has released its
own borrow. The scene also says where each node is placed (`Scene::placed_children`,
`Scene::placed_parent`): where it stands, except the caption a control keeps beside it
(`Scene::keep_caption_beside`), placed before the control among its siblings on every
platform. The terminal reads its structure there rather than keeping a copy, through
`Scene::read`, the shared borrow reads take, which nests and mirrors nothing, and the
server's string tree serializes the scene's structure through its records, a control's
carrier and its caption's `<label>` the only elements it adds; the browser's tree is the
DOM. The scene tells the platform what happens to each
child: it is attached (new, or returning from being hidden), relocated (a move, which
the web makes with `moveBefore` where the engine has it, keeping focus and animations,
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
(`Scene::tab_order`): positioned stops first, then document order, the modal opened
last containing it with the overlays opened after it. A platform drawing its own
controls traverses by it, stepping from the focused node by `Scene::tab_from` (from a
node that is no stop, the nearest stop beside it in document order), as the terminal
does. The scene answers which overlays a press dismisses
(`Scene::dismissed_by_press`) for every platform: from the top down, each overlay the
press lands outside of, on neither it nor the anchor it opened from, until one it lands
inside of, a modal ending the descent and dismissed itself; the web leaves a modal's
backdrop to its click. It answers what a key its handlers leave to the platform does
(`Scene::key_default`: `Tab` traverses, `Escape` dismisses the topmost overlay, which
the web leaves to a modal's own cancel, and the menu key or `Shift-F10` asks for a
context menu), which popups focus leaves as it moves to another node
(`Scene::popups_left_by`, dismissed by `notify_focus_moved` on every platform), and where
a modal takes focus as it opens (`Scene::modal_focus`: its first tab stop, else itself,
as a dialog's focusing steps choose). The scene holds the interaction every platform reports: the
focused node with whether focus shows (a key but a modifier alone sets it, a pointer
press clears it, a text entry's shows however reached), the pressed node and the hovered
one, dropping each with its node, and resolves every node's interaction from them
(`Scene::interaction`), so every platform that draws resolves states alike; it decides
whether an entry takes edits (`Scene::is_editable`: neither disabled nor read-only) from
the control state and reports it holds, which the platforms read rather than copy.

**`Realization` is a zero-sized trait with associated types**, monomorphized, hidden behind
each facade's views: `Scalar` (`f32` or `i32`), `StyleExt` (the medium's own
properties), `TokenValue` (the erased definition the medium accepts). Its methods are the
portable vocabulary over node ids: realize boxes, controls, overlays and runs, write a
derived slot's erased leaf into it (`apply_slot`), realize anchors and scopes, mirror
tree mutations, define tokens on a `TokenScope` (one call, a node's or the root's, the
root a scope like any other), register event kinds, focus, set values, observe sizes,
report media.
`Painted` and embeds are deliberately outside it—their payloads never cross a crate
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
  expressions arrive together in the create call—the web folds them into one
  content-addressed class—while each derived leaf binds a render effect that writes one
  leaf, erased over its property (`SlotLeaf`), into its per-node slot through
  `apply_slot`. The split is written once against the property visitor.
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
- **Embeds are platform-specific views**, written against a platform's escape trait: raw DOM
  through `WebDom`, markup through `WebMarkup`, cells through `CellPainter`, later a
  platform view through `PlatformView`. `LowLevel::realize` mints a scene node the view
  realizes through its escape trait, so the view composes with wrappers, themes and
  events. There is no
  `Any` and no downcast.

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
  `Deferred`, `AsyncView`, `empty`—mounts between two anchors (a comment pair on the web, a
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
  A run carries it as its layout, its derived leaves slots numbered after the run's
  own, and a low-level node hands it to its platform through `Realization::apply_layout`,
  which the web compiles into the element's class. A wrapper holds its child boxed,
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
which parts it exposes. `Toggle`,
`Slider`, `Popup`, `Activate`, `TextEntry` and `Collection` (with `Member`) ship; a
collection's `CollectionPolicy` says what it and its members are, which keys move
between the members and what a move and a choice do, and `RadioGroup` (with
`Radio`), `Listbox` (with `ListboxOption`) and `Menu` (with `MenuOption`) are
collections under the radio, listbox and menu policies, whose members are one tab stop
and whose moves pass over the disabled members in one place; `BasicButton`,
`BasicCheckbox`, `BasicSlider`, `BasicRadioGroup`,
`BasicRadio`, `BasicListbox`, `BasicListboxOption`, `BasicTextField` and `BasicTextEditor` are aliases over
them, and `BasicPicker` composes a `Popup` face with a `Listbox` in an `Overlay`. Both
have type-ahead: `TypeAhead` holds the characters typed in quick succession, reset by
a `Realization::schedule` timer, and `Roster::typed` matches them against the members'
names (`Label::set_name`, the value standing in), one character cycling through the
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
the keyboard, through `Realization::set_overlay_position`) and answers `Prevent::All`, so
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
`SimpleContextMenu` (`skene-simple`) is the palette-decorated one on both facades. In
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
`Simple*` components (`skene-simple`) are the ready-made library over the same builder:
one palette, one shape language, shared across the web and the terminal wherever the
medium allows, and deliberately nothing a third-party library could not build on the
same footing—including how an app reaches it: an explicit dependency, never a facade
re-export.

- **A control decides its verdict.** The behavior runs first and its verdict is the
  whole verdict; a user's handler observes. A button's click never reaches the row it
  sits in, on any facade.
- **A control is controlled.** It reports; the bound signal is the truth. Two-way
  binding is a derived read plus a handler write, on every facade, held once in a
  `Controlled<T>` that every shipped behavior, and a third party's, keeps its value in:
  `set`, `derive`, `handle` and `bind` for the behavior's own setters, `read` and `ask`
  for its logic.
- **A drawn entry reports an edit whole and applies its verdict.** A text entry whose
  platform edits it with Skene's editing core (the terminal's today) reports a reader's
  edit as one `Edit`, its text and its selection together, with whether its toolkit
  shows it already, through `report_edit::<P>(node, &edit, applied)`, and is answered
  with a `Verdict`: `Accept`, or `Replace` with the truth, which it shows. A truth that
  gives no selection keeps the proposed one, mapped through the smallest edit from the
  proposal to the truth by one rule in the shared half (`Edit::map_through`, counted in
  grapheme clusters by `TextIndex::map_through` whatever unit the platform's protocol
  counts in): an offset before the replaced range stays, one after it moves with it,
  and one strictly inside it stays as many clusters into the replacement as it was,
  clamped to the replacement's end, so a transform that rewrites the whole text, as
  uppercasing does, keeps the caret where the reader typed. The control asks the
  behavior (`Behavior::on_edit`; while it is disabled the behavior's held truth,
  `Behavior::held_edit`, answers), refreshes its state and does not reconcile, so the
  verdict is the only write-back and a vetoed or transformed edit leaves no step in the
  core's undo history; the bound value's own write arrives later through `set_value`,
  which the core takes as nothing where it equals what is shown. `TextEntry` accepts an
  edit the truth already holds (`Edit::holds`) without asking, asks the value and then
  the selection otherwise, and accepts any edit of an entry whose value nothing derives.
  The web's `<input>` and `<textarea>` report the same way, as a toolkit that applies
  first: each `input` event is one applied `Edit`, its selection read in UTF-16 code
  units and counted in clusters, and a `Replace` is repaired by the smallest splice
  (`setRangeText`) with the selection mapped by the same rule, inside the event, so the
  turn's write of the bound value finds it shown. While an input method composes
  (`InputEvent.isComposing`) the repair waits, and a signal's write of the value or the
  selection leaves the entry alone, until `compositionend` reports the entry's text
  again and applies the verdict (an adopted entry holding text typed before the client
  arrived is left alone the same way until hydration replays it as one more report of
  its value and selection); a cut or a paste from the context menu splices the entry's
  selection, the caret after the text it inserts, and dispatches the bubbling `input`
  (`deleteByCut`, `insertFromPaste`) a browser's own would, so it reports the same way;
  `Edit::holds` makes the report idempotent whichever of the two events an engine sends
  last. A `TextSelection` counts grapheme clusters; where a platform's protocol counts
  in code points or UTF-16 code units it names its unit (`Unit::CodePoint`,
  `Unit::Utf16`) and converts through one index per text (`TextIndex`), as the web does
  for its `<input>`.
- **Parts and control state.** A part is a named child the styling targets (`Track`,
  `Fill`, `Thumb`, `Mark`, `Face`, `List`, `Item`, `Label`); in the terminal a part maps
  to a glyph token. A `State` a box cannot have on its own (`Checked`, `Indeterminate`,
  `Disabled`, `Invalid`, `Open`) resolves against the enclosing control; a node's own
  `Hovered` and `Focused` stay its own. On the web a control that owns a native input
  compiles control state to a selector anchored on that input; natively it is the
  control's own state.
- **The scriptless form is the control.** A text field or editor is its native
  `<input>` or `<textarea>` itself, decorated as the box; a checkbox, radio or slider
  keeps its native input inside its label, covering the label's box unseen and
  focusable, beside the parts that draw it, which take the press it lets through, as a
  run inside any control leaves a press to the box it falls in. So keyboard, form
  participation and accessibility are the browser's before and after hydration and
  nothing is swapped. A radio group's radios share the group's element id as their
  `name` (a member names its collection through `Behavior::group`, carried to the
  platform as `ControlProps::group`), so before the bundle the browser keeps one of
  them chosen and submits them as one field, and a choice made there reaches the
  signal as any adopted toggle does; since the browser unchecks the others with the
  one it checks, a toggle reported on one member re-reads every member's state. The
  picker alone is drawn (no engine but Chromium restyles an open `<select>` yet), so it
  is an `Enhance` of its own: its scriptless form is a `NativePicker` control, a
  `<select>` on the web, listing the options the option views name, its caption a
  `<label for>` beside it and the chosen value selected; the option views build once,
  under the group both forms answer through, and attach nowhere until the drawn form
  lists them, and the caption builds once and each form mounts it with its name and
  the disabled setting, so a swap rebuilds nothing but the face; a choice the reader
  made in the `<select>` before the bundle arrived reaches the signal as any adopted
  live value does, and the drawn face that takes the select's place shows it.
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
  whether focus shows and a node holds a press. A page is a view built
  on the platform, so a driver builds it in its own process or serves it to another:
  the web's, `skene-conformance-web`, serves every page from one app and acts each
  scenario out over a WebDriver session in Firefox and Chromium by trusted input, an
  event a page dispatches to itself running no browser default, finding what it acts
  on by the role and the name the browser computes, and holds every page to a
  screenshot golden that a pinned Firefox draws in a pinned font, pixel for pixel. A
  behavior every platform must share is a scenario, and a divergence the suite finds is
  fixed in the platform, never excused in the suite. The
  rules those scenarios exercise that a platform realizing its own overlays would
  otherwise copy live in the spine: an overlay's role
  (`OverlayProps::role`: a hint's tooltip, a modal's dialog) and the focus a closing
  overlay returns, to the node focused when it opened or its anchor's nearest
  focusable, as a browser's top layer does.
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
  `ResolvedStyle`; transitions and the animations' timing cross as data at both. Its
  `address` is the 128-bit xxh3 of its static part (the level, the entries and the
  motion), a cache key alone: equal static parts share one, and a realization names
  nothing by it, the web's class names staying the hash of the template. A coverage
  test holds it to the whole model for each medium (none, `WebStyle` in pixels,
  `TerminalStyle` in cells): a style setting every property, each under variants of
  every `Media`, container query, `State` and `Condition` kind with every leaf and
  expression node kind among them, crosses whole, motion included, crosses compiled at
  `Keep::Everything` whole, every slot bound where it stands, and compiled at
  `Keep::Nothing` resolves as the typed style does under readings where every condition
  both holds and fails. The seam does not carry it yet: a realization receives the
  typed styles in `BoxProps` and `RunProps`.
- **Raw CSS** reaches a node only through the web escape trait: `WebMarkup` takes
  `Declarations`, the sorted, content-addressed declaration set the compiler itself
  renders into. The typed model carries no passthrough of its own.
- **Web** renders the whole set into one interned rule template: base declarations,
  `@media`, `@container` and pseudo-class rules, `transition:`, content-addressed
  `@keyframes`, `var()` for tokens and slots. State-, media- and theme-driven styling
  therefore works in a static document with zero script. A template is compiled from
  the node's typed styles and, in debug builds on both leaves, from its record compiled
  at `Keep::Everything` as well (its entries through the same `Rules`, its frames'
  leaves rendered as the typed keyframes', a container's `size` containment read from
  whether it has a height, its slot bindings the shared half's), and the two compiles
  are asserted equal, template and slot bindings.
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
  browser leaf, its read node a low-level view keeping the props the web realizes it
  with, compiles the node's record at `Keep::Nothing` with the resolver of a
  `ResolutionContext` over the web's own scene (its themes, scopes, containers and the
  enclosing control's state), runs its motion through the foundation's `Motion`, and
  holds the value to the one the engine computes, so a divergence it finds is one every
  platform resolving through the shared half shares. In debug builds the terminal
  resolves every box, run, text entry and keyframe style through the node's record
  compiled at `Keep::Nothing` as well, with the same resolver and before its own
  fill-ins (rounded border glyphs, the typography's size and weight, `bold`), and
  asserts the two resolutions equal by their `Fingerprint` and alike in whether they
  read the node's interaction, the flag kept as the typed resolution alone sets it, as
  it asserts the record's motion equal to the dynamics it was built from; a wrapper's
  annotation resolves typed alone.

### Tokens and themes

A design token is a named, inherited slot. `TokenRef<T>` names it (a `const`; the name is
an implementation detail on every platform), `Token<T>` pairs the name with the
definition a palette gives it, and a consumer emits only the name, so two boxes under two
palettes intern one class and a theme change never touches a consumer. A token's own
conditions live in its definition, once. A length token serves every property measured
in lengths, a box's sizes, padding, spacing and margins, its corner radius and a run's
font size and letter spacing alike, and a property that is a plain number reads a
number token alone.

`Theme<P, S: TokenSet>` is the one theme view. It realizes one layout-invisible scope node
(`display: contents` on the web, a scope record in the terminal), provides `Memo<P>`
through context, and runs one effect that writes `P::definitions()` onto the scope, so
nothing rebuilds. On the web a palette change therefore costs the number of tokens, never
the number of nodes; the terminal still re-resolves every node beneath the scope until its
token caches take per-scope epochs. Nesting a theme redefines beneath it. On the web the static definitions
are an interned class on the scope element (with `@media` blocks and the scheme double
emission where a definition conditions on the scheme) and each token kind registers its
custom property with `@property`, so it interpolates and inherits as a typed value; a
token set used with no theme above registers its default under `:root` once
(`ThemedIn::current`/`watch`). Any node defining tokens, a scope or a control defining
its own look as a Simple button does, takes them as such a class, and a redefinition
whose every token is unconditional writes them inline over it; a node adopted from the
server's markup gives up the definitions the server wrote inline on it at its first
definition. The root's definitions reach the realization through the same
`define_tokens` call as a scope's, their scope `TokenScope::Root`: the web writes them as
the root rule's class, and the terminal re-resolves its styles as it does for a scope's.
`Typography` (all text inheritance on both facades) and the Simple library's
`SimpleGlyphs` (its terminal marks, a set only a platform with text tokens over cells can
theme) are token sets, so the framework has one inheritance mechanism.
Nothing below a component library knows what a palette is: `skene-simple` owns
`SimplePalette`, and a third party's palette gets its `Theme`, `current()` and
`watch()` from the same blanket.

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
order is identical, which hydration relies on. A class cache keyed by the hash of the
typed props renders the template text on a miss only. The interner, the cache and the
token and slot registrations are one `StyleBook` both web leaves share, telling each
leaf's `StyleSink` what to add and remove (the browser: one `<style>` element per
class; the server: each rule collected as it is interned and emitted with the next
flush, the document's `<style>` or a streamed chunk's delta).

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
  word the whole text) the other, which no entry is yet.
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
  over a truth and a handler, or a platform's adapter onto `report_edit`), an edit it
  refuses leaves no step, and a truth it answers instead is shown with the proposed
  selection mapped through it by `TextIndex::map_through`, cluster by cluster.
  `Binding::for_key` is the key table where Ctrl is the command key (Linux, Windows and
  the terminal): select all; undo and redo (`Ctrl-Shift-Z`, `Ctrl-Y`); copy, cut and
  paste (`Ctrl-Shift-V`, `Ctrl-Insert`, `Shift-Insert` and `Shift-Delete` among them);
  deletion by cluster and by word; Home and End, the whole text's under Ctrl; and the
  arrows, up and down in an editor alone, while a composing key and `Ctrl-Shift-A` bind
  nothing and a key typing one cluster inserts it.

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
empty. The `WebDom` escape trait exists on the browser leaf alone, so the server leaf
*cannot* touch the DOM; each leaf's dependencies are keyed on the same cfgs.
What both realize is written once, in a `Realizer` over the `Sink` the DOM and the
string tree implement: the records, the element choices, attributes, classes, slots,
token definitions, control state and accessibility, so the two cannot drift; each leaf
adds only what is its own:

- **browser** (`not(skene_server)`)—the browser. DOM sink, the stylesheet
  sink, root-level event delegation with a stamped node id (one listener per event type,
  `scroll` and enter/leave included, an event a handler causes, as the `focusin` of a
  focus it moves, delivered at once inside it), one shared `ResizeObserver`, hydration
  (creation-order `data-h` markers, loud mismatch, the server's stylesheet adopted
  rather than duplicated, text typed in an entry before the client arrived replayed as
  an applied edit and repaired in place, any other live input value that differs from
  the serialized one written into its signal, and convergence once the tree is built:
  every server node the client built differently or never built removed, every adopted
  element given exactly the client's classes, and the order-keyed resource values no
  resource adopted dropped), scroll and focus restoration keyed by a segment's end
  anchor, the `WebDom` escape trait, the History binding and the head.
- **server** (`skene_server`)—the serializer. Renders its string tree to HTML plus
  the interned stylesheet; drives async to a fixpoint on the host executor, parked until
  a waker fires, before serializing, under a render deadline; streams in document order around `Deferred`
  boundaries with a style delta per chunk; serializes resource state for the client to
  adopt; takes client hints as render input. The raw handles do not exist here, painted
  content renders empty, and handlers are inert.

The crate's `platform` module holds each pixel platform's own vocabulary, a module per
platform: `platform::web` is the web platform itself (`Web`, the `WebMarkup` and
`WebDom` escape traits, `Declarations`, `RenderHints`, the re-exported `Document`, and
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
first client: its `Editor` over the cell engine (a field one unwrapped line, an editor
wrapped at its content width) is held apart from the records, so an operation on it—a
key, a press or a drag, a paste, a write from the app—runs with none of them borrowed
and what it changed (damage, restyle, the window, a moved selection to report) is
settled after, an operation reaching an editor another holds (a handler that moves
focus, say) waiting for that one to end. Its box its style leaves unsized lays out at
its native size, eight cells or its value's widest line and the caret's cell past it,
and its lines tall, as an `<input>`'s intrinsic size: its content size, so the basis it
grows from beside a growing sibling, and the least a layout stretching or shrinking it
leaves it, held to its own maximum and raised to its own minimum (its size
outright where either is a fraction, which then bounds it). The value draws on the box's
first row, where a browser centers it in a taller `<input>` (`ROADMAP.md`). Its edits
are reported through `report_edit`, never applied ahead of the verdict, which it shows;
it takes its read-only state from the scene before each operation, and a key command
lays the scene out first (`lay_out`).
Its selection is the core's—an anchor beside the caret over the value,
reported to the app as a `TextSelection` in clusters after every move and written from
its bound signal, each index clamped into the value—so keyboard focus selects a field's
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
and focus order, exactly as on the web, and to link runs, which sit in the tab order as
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
  `skene-foundation`, writes its views generic over `P: Realization` exactly as the built-ins
  are written, states what it needs from a medium as a bound on the platform's associated
  types (`P::StyleExt: CornerRadius`), and keeps each view's members in a trait; each
  platform's crate makes them concrete as views of its own, as each facade does the
  built-ins. Nothing on the path a built-in takes is crate-private: the setter macros the
  built-ins generate their `set_`/`derive_` pairs with (`properties!`,
  `trait_properties!`) and the surfaces they share (`DecorationBase`, `RunBase`) are
  public, so a third-party layout view built on `LayoutBox` spells its surface the way
  `Row` does. Simple is the standing proof, `skene-simple` its core and
  `skene-simple-web` and `skene-simple-terminal` its platforms' crates, and
  `skene new lib` creates the same shape: a core holding a palette, a unit trait and a
  view, and a crate per platform named.
- **A low-level view, against the platform, like a built-in.** It is written against a
  platform's escape trait and realized through the same scene and node ids as a
  `Container`, so it composes with wrappers, themes, conditions and events. A view
  library covers every platform of its medium through a trait of its
  own that each platform implements (`impl<P: MapHost> View<P> for Map<P>`), so an app
  names no platform and writes no `cfg`; a blanket impl over one escape trait would allow
  one per view type and push the cfgs into the app. Selecting the platform is what
  selecting the facade already does. A platform's escape traits live in its facade's
  `platform::<p>` module: `skene_web::platform::web::{WebDom, WebMarkup}` and
  `skene_terminal::platform::terminal::CellPainter`. `acme-map` (a canvas on the browser
  leaf through `WebDom`, a placeholder through `WebMarkup` before it and on the server)
  and `acme-sparkline` (bars through `CellPainter`) are the standing proofs. The
  portable low-level path for pixel media is `Painted`.

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
module, its data types in every build and its controls in its own: `platform::web` and
`platform::terminal` hold each leaf's realization and escape traits, and
`platform::{android, apple, ios, linux, macos, windows}` are empty until their
platforms land. `#[main]` and `#[lazy]` expand per leaf on the same cfgs, so a server
build never emits the browser entry or splits a lazy body, the terminal's `#[main]`
emits its `main` in the terminal's build alone, and an entry taking arguments,
parameters or generics is refused.

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
patches and starts from the checkout's lock.

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
`spawn`, `Deferred`, `client_only`, `media`, and the interactive-only wrappers. App code
names no platform.

## Async, data, and third parties

HTTP, WebSockets and timers are **userland**, deliberately (the browser owns the loop on
the web; owning an HTTP stack is permanent debt). The framework ships only the glue no
external crate can provide, and ships it as *public seams* so third-party integrations
sit on exactly the footing first-party ones would:

- `spawn`—resume on the UI thread, scope-bound cancellation.
- `Realization::schedule`—a one-shot action on the platform's clock, cancelled by the
  `Timer` it returns: the window's timeout on the web, the loop's wait in the terminal,
  fired under a test from the hand-stepped clock the test advances, and never on the
  server.
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
