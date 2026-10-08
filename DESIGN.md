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
bottom, `skene-foundation` on it, the platforms (`skene-web`, `skene-terminal`,
`skene-headless`) and the Simple core on the foundation, each platform's Simple crate on
its facade, and the macros, the transport and the CLI beside them.

**A facade per platform, one spine.** The web and the terminal are different media, not
one medium with fallbacks: an app is written for each on purpose, and neither
cross-compiles into the other. Each facade therefore owns its builder vocabulary outright, typed to
exactly its capabilities—and everything beneath it is written once in the spine. The
facade's own contribution is small by design: a scalar, the medium's condition leaves,
the properties only that medium has, the platform it selects, and a type of its own per
view, a newtype over the foundation's, that gives app code a flat, medium-typed surface. What a medium lacks is absent at compile time
because the platform's associated types say so.

Dependency arrows only point down; a platform depends on the foundation, never on another
platform; each facade names only its own platforms, the web's naming the headless one
it compiles against off its target, and publishes the selected one as `ActivePlatform`.

## The scene and the platform

**The spine owns the tree; a platform keeps a record per node.** `Scene<P: Platform>` is a
generational arena of node records—parent, children as a list of linked siblings (a move
relinks a few records and shifts nothing), the annotation a wrapper asked for the child,
the token definitions a theme placed on the node, the query-container name, the
handlers. The scene mints every `NodeId`, and the platform keys its own record of the
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
node that is no stop, the nearest stop beside it in document order), the terminal and
headless alike: headless applies a `Tab` its handlers leave to the platform. The scene answers which overlays a press dismisses
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
(`Scene::interaction`), so the terminal and headless resolve states alike; it decides
whether an entry takes edits (`Scene::is_editable`: neither disabled nor read-only) from
the control state and reports it holds, which the platforms read rather than copy.

**`Platform` is a zero-sized trait with associated types**, monomorphized, hidden behind
each facade's views: `Scalar` (`f32` or `u32`), `StyleExt` (the medium's own
properties), `TokenValue` (the erased definition the medium accepts). Its methods are the
portable vocabulary over node ids: realize and update boxes, controls, overlays and
runs, realize anchors and scopes, mirror tree mutations, define tokens on a scope and at
the root, register event kinds, focus, set values, observe sizes, report media.
`Painted` and embeds are deliberately outside it—their payloads never cross a crate
boundary—and arrive through extension traits (`Painting`; each platform's escape trait).
The leaf-only surface (`document`, `element`, `render_to_string`, `launch`) stays free
functions in the platform crate, since it is not a shared contract.

- **Why a trait and not name-resolved free functions.** A missing function is only an
  error on a path that names it, and those paths are target-gated; a trait makes a gap a
  hard error. That is what makes the host platform complete, so every view and `Resource`
  are testable headlessly.
- **Why the scene is in the spine.** The scene *calls* nothing: the spine mutates it and then
  calls the platform through the generic parameter. Every platform needs the structure; one
  implementation is smaller and has one reparent semantics.
- **Dispatch is the spine's.** A platform reports an event at a node; the spine walks the
  parents the node had when the dispatch began, as the DOM fixes an event's path, runs
  handlers deepest first, folds `Prevented`, and tells the platform what the platform
  may still do. The browser's root delegation, the terminal's hit path and the
  headless platform share one bubbling implementation, so the rule that a view owns the
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
  content-addressed class—while each derived leaf binds a render effect that writes its
  per-node slot. The split is written once against the property visitor.
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
  its 2D context; the headless platform renders it to pixels
  a test reads (`headless::render_to_pixels`); the server leaf renders it empty; the terminal
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

  **Headless tests, not a mock platform.** Because the vocabulary carries typed data, the
  host platform (`skene-headless`) realizes exactly what any real platform receives into an
  inspectable record store, resolves a property through the spine's `Resolver` (the scene's
  tokens, the expression evaluator and the environment), and runs tasks when a test
  pumps them, giving
  view logic fast headless tests without pretending to be a platform. Platforms test against
  their real sinks (web: a browser DOM via `wasm-bindgen-test`; the server leaf
  natively under a byte-exact snapshot; native: its toolkit). Only the spine's own unit tests
  run on a double, a `TestPlatform` recording the calls it receives.

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
  capability vocabulary. **Hydration** is the web's: `Platform::hydration()` returns
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
  hydrates nothing answers true from the start and never builds the baseline. The
  headless platform answers hydrated until a test says otherwise (`trigger_hydrated`),
  so a generic view's two forms are tested on the host. **Pixels** are `Painting`'s:
  `Painting::available()` is the signal a painted box waits on, false on the server
  leaf, false on the browser leaf until it is live, false in the terminal until the
  graphics probe is answered, and the headless platform's from `trigger_graphics`. Only
  the branch in force is ever built, so an enhancement may touch the browser and a
  baseline may be the whole view where the signal never turns. The seam is the
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
a `Platform::schedule` timer, and `Roster::typed` matches them against the members'
names (`Label::set_name`, the value standing in), one character cycling through the
names starting with it and more matching a prefix; the listbox moves focus to the
match, and the closed face chooses it outright through `Popup::set_key_fallback`, the
behavior's hook for the keys it does not act on itself. A box or a control a press's
pointer drags declares the axes it drags along (`Container::set_drag_axes`,
`Behavior::drag_axes`, a slider's its own axis): a primary press keeps the pointer with
the nearest such node until the release—the web captures the pointer, the terminal
routes the press's moves and its release to the node—and a touch pans only along the
other axis (`touch-action: pan-y pinch-zoom` for a horizontal drag), while a press
anywhere else leaves each move and the release to the node under the pointer, as a
browser does; the terminal shows a grab cursor over a control that drags. An anchored popup on the web follows its anchor through CSS anchor positioning (the anchor
named after its node, the popup placed by `position-area`) where the engine has it, and
sits at the anchor's measured rectangle as it opens elsewhere. A `Tooltip` wraps the one
view it describes and mounts that view and a hint overlay, the `Bubble`, in a segment of
its own: it opens on the platform's clock once the pointer has rested for its delay and
at once on focus that shows (`Platform::focus_visible`), closes on leave, blur and a
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
the keyboard, through `Platform::set_overlay_position`) and answers `Prevent::All`, so
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
  participation and accessibility are the
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
  axe-core in the browser tier (`browser_axe`, the library vendored beside the test), so
  every built-in is checked against the ARIA rules as it is drawn and with each of the
  gallery's overlays open: a tooltip, the picker's popup and a context menu. Disabled controls
  leave the focus order, a deliberate reading of the practices' allowance, recorded
  in `AGENTS.md`.
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
  screenshot golden that a pinned Firefox draws in a pinned font, to which headless's
  golden of the page is held in turn, both blurred by three pixels. A behavior every
  platform must share is a scenario, and a divergence the suite finds is fixed in the
  platform, never excused in the suite. The
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
  role ARIA requires a name for that has none, which both galleries assert empty on the
  host tier as axe does in the browser. The same walk builds the accessibility tree as
  AccessKit models it (`Scene::access_tree`): each node's role, name, value, states and
  relations under a window, focus on the focused node—what a native platform hands its
  screen reader, and, as indented text (`access_outline`), what a test asserts.
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
  the environment or the structure moved): by the terminal's records as they change, and
  by the headless platform at each layout, a walk of the scene where the structure moved
  or a node's part changed its shape and the revisited nodes alone otherwise
  (`Tree::lay_out_revisited`), at the values its motion presents, the page mounted in a
  body (`mount`) laying its boxes out along a row that fills the viewport, each growing
  and shrinking along it, as the web's reset lays out the document's, and a text entry
  measured at the size Chromium gives its native element (a field nineteen average
  characters and the width of the face's bounds wide, an editor twenty wide, each its
  lines tall), stretched where its layout stretches its items as an `<input>` is. There
  a box whose children are all inline runs lays them out as one `Paragraph`, keeping
  their blanks and newlines as the reset's `white-space: pre-wrap` does and wrapping
  where the Unicode line breaking algorithm allows across them and around an inline box,
  laid out at its content's size first, a word too wide for a line of its own broken
  where the line fills as `overflow-wrap: break-word` breaks it, a tab advancing to the
  next of stops eight spaces apart, the blanks a line ends in hanging where it wraps and
  taking room before a newline or at the end, and a newline ending the text opening no
  line, each run's line box on its line's baseline, a box's bottom on it, a span placed
  in a rectangle per line it is on, and the paragraph trimmed to its first line's cap
  height and its last line's baseline as the reset's `text-box-trim` trims a text box;
  every run is measured in the pinned face the conformance suite draws in, Inter's
  regular (a cluster as far as its glyphs advance at its font size, its letter spacing
  past it, a line its font size times its line height tall, each as the typography in
  force sets them). Headless reports each box's size to the observers bound to it and
  answers `layout_rect`, `visible_rect` and, through the frames' paint order, `node_at`,
  a box clipping its overflow along an axis clipping what lies beneath it to its padding
  box there and a box's scroll offsets, as `trigger_scroll` reports them, shifting it
  back, as in a browser. It draws the scene as a browser paints the page
  (`render_to_image`): each box's background and border, rounded by its corner radius,
  what a clipping box holds clipped, each run's glyphs in the pinned face at the places
  its paragraph gives them with its decoration, a text entry's value (a field's line in
  its middle, as a browser draws an `<input>`'s), each painted box's picture and the
  open overlays above the rest, a border solid whatever its style and a decoration one
  solid line; every page of the conformance suite is held to a golden of it, and the
  golden to the browser's. `Dimension` is a fixed length,
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
- **Terminal** (and native) resolve every property at one point through the spine's
  `Resolver`—variant overlay against the environment and the node's `Interaction`, token
  lookup through the scene, expression evaluation, slot reads—handing it the medium's
  unit, its token and slot lookups and its font size. The terminal caches the result per
  node under a generation covering environment, interaction and definitions, and
  reports whether the resolved value changed; that report is where a transition
  starts. The headless platform resolves through the same `Resolver`, so a test's answer
  and the terminal's are one computation, and both build it through one
  `ResolutionContext` (a node's environment, interaction and containers from the scene,
  a token resolved where it is defined, the font size from the typography's size
  token, the platform's `EM` where text takes no sizes) over a `ResolutionSource` giving
  what the scene does not hold: the environment, container sizes, slots and whether
  text is drawn at other sizes.

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
(`ThemedIn::current`/`watch`). `Typography` (all text inheritance on both facades) and
the Simple library's `SimpleGlyphs` (its terminal marks, a set only a platform with text
tokens over cells can theme) are token sets, so the framework has one inheritance
mechanism.
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
typed props renders the template text on a miss only. The interner, the cache and the
token and slot registrations are one `StyleBook` both web leaves share, telling each
leaf's `StyleSink` what to add and remove (the browser: one `<style>` element per
class; the server: each rule collected as it is interned and emitted with the next
flush, the document's `<style>` or a streamed chunk's delta).

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
  destroying one row per change. The value stays plain data, so it snapshots as a unit
  across a dev reload.
- **A local `Signal` for one view's state.** What one view owns and nothing else
  reads—a draft, a disclosure's openness—is a `Signal` the view makes as it builds,
  under its owner, so it lives exactly as long as the view and goes with it.
- **Context for what a subtree sits inside.** Values a whole subtree reads without
  being handed them—a theme's palette (`ThemedIn::current`), the router, an async
  boundary—are provided once and read by any descendant built under that owner.

There are no view models in context. A screen's data is a `Store`, or a lens onto one,
passed to the function that builds the screen, not an object a parent provides and a
descendant looks up: an argument states what a view needs where it is called, costs a
copy, and keeps the view testable headless with a store the test builds. Context stays
for the genuinely ambient, which every layer between would otherwise have to thread.

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
- **Snapshot identity is positional, and a key anchors it.** Nothing registers until
  something records: a platform calls `record_snapshots` at startup where a dev channel
  is present, and `restore` implies it, so a production build creates every signal as
  a transient one. A signal registers by the path of steps from the runtime's root
  owner, its index among its owner's snapshot signals and the hash of its type name. A step is an owner's position among its
  siblings, or the `Key` of a keyed owner: a keyed step takes no position, so keying
  a sibling shifts none of the positional ones, and a key is the hash of the value it
  was taken from, so `reconcile`'s item key and a view's `set_key` are one concept.
  `Signal::transient` occupies no position, `Signal::tagged` is an anchor the diff aligns
  around within one owner. Restore aligns per owner as the live side is created, and
  every mismatch defaults, so a stale snapshot reaching an edited program is the
  ordinary case and never fails. A node takes a `SnapshotPlace` from the same sequence
  when the scene creates it, and a platform carries what a node holds outside the
  graph—focus, a text control's selection or caret, scroll offsets—through
  `provide_extra`/`take_extra` by that place, so a rebuilt page resumes on the same
  node through the same alignment, keys included.
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
escape trait exists on the browser leaf alone, so the server leaf *cannot* touch the DOM.
What both realize is written once, in a `Realizer` over the `Sink` the DOM and the
string tree implement: the records, the element choices, attributes, classes, slots,
token definitions, control state and accessibility, so the two cannot drift; each leaf
adds only what is its own:

- **browser** (`skene_web_platform = "browser"`)—the browser. DOM sink, the stylesheet sink,
  root-level event delegation with a stamped node id (one listener per event type,
  `scroll` and enter/leave included), one shared `ResizeObserver`, hydration
  (creation-order `data-h` markers, loud mismatch, the server's stylesheet adopted rather
  than duplicated, a live input value that differs from the serialized one written into
  its signal, and convergence once the tree is built: every server node the client built
  differently or never built removed, every adopted element given exactly the client's
  classes, and the order-keyed resource values no resource adopted dropped), scroll and
  focus restoration keyed by a segment's end anchor, the
  `WebDom` escape trait, the History binding and the head.
- **server** (`skene_web_platform = "server"`)—the serializer. Renders its string tree to HTML plus
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

The host platform: every `Platform` method writes into a record per node, keyed by its
`NodeId`, while the structure stays the scene's alone; nothing lays out and no platform
response fires. It is the web medium (`StyleExt = WebStyle`), so a corner radius or a
run's font size resolves as set, as it would on the web. Beside the records it keeps what
a test asks: the outline, read from the scene, the props a node was created with, the
slot a render effect last wrote, a property resolved through the spine's `Resolver` under the
environment a test set (viewport, scheme, reduced motion, container sizes, hover and
focus), control state and parts, overlay openness, focus order, bound painters, and any
node's accessible name, with the queries over it. Events go through `trigger_*` into
the spine's dispatch, and `user` composes them into a user's gestures in a browser's order: a
click hovers, presses, moves focus to the nearest node taking it (or off the focused
one), releases and clicks, and a key where focus is carries a text entry's edit where no
handler suppressed it. A frame clock advances on request; an executor, installed by
`spawn` and `Resource` alike, holds every spawned task until `pump_tasks`. It is what
`skene-web` selects on a host target, so an app's `#[main]` compiles everywhere.

### Native (planned)

One shell per OS—`skene-ios`, `skene-android`, `skene-windows`, `skene-macos` and
`skene-linux`—each its own crate and facade, with a vocabulary for that OS's controls,
structure and navigation. An app's shell depends on its OS's crate, which builds for that
OS's triples alone, so no cfg names the platform. Every shell realizes the tree into the
OS's own view hierarchy and composites it there: a box is a view with its layers, a text
entry is the platform's own (`UITextField`, `EditText`, `GtkText`, a XAML `TextBox`), and
only `Painted` draws, through vello into a layer-backed view. Native text entries, menus
and clipboard, scroll views with their physics, and accessibility are the platform's on
every OS, and no realization is a hybrid of painted content around embedded native
controls, so no shell is drawn: Linux is GTK4. Layout is the platform's own containers
beside taffy, behind a small library the platform's layout primitives call; the resolution
cache tells a shell whether a layout-affecting value moved, so a palette write relays out
only when it must. Off-thread work uses the engine's scheduler wired to the event-loop
proxy, and a loop-integrated timer is fair game there, the loop being the shell's. The
plan, its prerequisites and its order are in `ROADMAP.md`.

### The terminal (`skene-terminal`)

A facade in its own right, not a platform of the web's vocabulary. Its views are the shared
ones typed to the terminal: lengths are `u32` cells, text is a color plus a `bool` bold,
a border is one cell of box-drawing characters on each edge its width names, in its own
color, its conditions are cells and
its own background. Nothing the medium lacks exists to call, so a terminal app is
first-class at compile time rather than best-effort at run time.

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
WebDriver table, which the headless platform's key specs use too), and a shifted
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
text, and a view inside it flows as an inline box: laid out on its own, standing in the
text as a placeholder of its width, painted and hit at the cells it flowed into. An
edge whose border width is zero takes no cells and draws none, and a border whose color
is unset draws in the terminal's own ink, as CSS's `currentColor` does. Frames paint into a cell buffer—to the terminal
through `launch`, or to plain text
through `testing::render_to_text` for headless tests (`trigger_key`/`trigger_mouse_move`/
`trigger_mouse_click`/`trigger_frame` in `testing` drive it without a terminal). Terminal detection is
asynchronous; the loop starts immediately and the answers arrive as signal writes. The
dev loop carries the snapshot across a swap through the launcher's control pipe. Terminal
apps never cross-compile into the web.

## Third parties and platforms

Two guarantees, both properties of what is public rather than of any one API.

- **A component library: one core, a crate per platform.** Its core depends on
  `skene-foundation`, writes its views generic over `P: Platform` exactly as the built-ins
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
  library covers every platform of its medium, headless included, through a trait of its
  own that each platform implements (`impl<P: MapHost> View<P> for Map<P>`), so an app
  names no platform and writes no `cfg`; a blanket impl over one escape trait would allow
  one per view type and push the cfgs into the app. Selecting the platform is what
  selecting the facade already does. `acme-map` (a canvas on the browser leaf through
  `WebDom`, a placeholder through `WebMarkup` before it and on the server, an image box
  on headless) and `acme-sparkline` (bars through `CellPainter`) are the standing
  proofs. The portable low-level path for pixel media is `Painted`.

A platform need not be first-party: a crate that implements `Platform`, provides its scalar,
leaves, extension, a `launch` and views over `skene-foundation` is a facade. `Platform` is not
sealed, and the shared machinery bounds only on it and the public extension traits; the
headless platform is exactly such a crate in all but ownership.

## Selection and entry

Selection is by **facade dependency** first—a web app depends on `skene-web`, a
terminal app on `skene-terminal`—and by target within a facade: `skene-web` resolves
`wasm32-unknown-unknown` to the web's browser leaf, a build naming the server platform
(`--cfg skene_platform="server"`) to its server leaf on any host, every other host to the
headless platform, and no cargo feature selects any of them. Where one triple hosts
several platforms the build names the platform by a cfg, chosen once and unifying
nothing, as `getrandom` names its backend. Target-gating stays the enforcement layer
where it applies. A cargo feature is an opt-in upgrade that enables optional
dependencies alone, never a bound, a type, a variant or a platform, and none is on by
default: `skene-web`'s `gpu` is the one there is.

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

## Router (`skene_foundation::routing`)

Routing is state: a typed route as a signal, shared by every platform. Each platform binds
it thinly—the browser leaf to the History API (wrapping a navigation in a view
transition when the app opts in, through `Router::set_commit`), the server leaf takes
the route as render input, the terminal keeps it in memory. SSG enumerates the route
table (`StaticRoutes`) to know what to render. The router depends on the engine alone.

## SSR, SSG, and the server binary

Server-side rendering is a *capability*, not a server: `render_to_string` /
`render_to_stream` produce the parts (styles, body, state) or ordered chunks, and
`Rendered::page` assembles a document. One template, `skene_web::Document`, writes every
page, the CLI's client-rendered shell among them: the head preloads the client bundle,
the body ends in the hydration state and the module script booting the bundle, a page
in the dev loop carries the dev marker and the dev script, and every inline script
carries the render's nonce. A head registration (`add_meta` and its kin) holds while the
owner that made it lives, in one registry the web leaves and the headless platform
share, `skene_foundation::web::Head`: the latest live element of what it stands for is in
force, so a route's head leaves with the route; the server serializes what is in force,
and the browser applies it to the live head, adopting the elements a server render
left. The title is the window's (`window::set_title`), shared with the terminal: the
latest live one is in force, and each platform shows it, the web in the head. The app's server binary is built natively with the
server platform named (`skene build web --static` and `skene run web --ssr` do it, for
the host target alone so build scripts and proc macros never see the cfg; by hand,
`RUSTFLAGS='--cfg skene_platform="server"' cargo rustc --lib --crate-type bin`), and
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
standard error; the terminal installs none, its screen being the app's, and headless
leaves the choice to the test. The spine carries no subscriber.

## Glossary

The words this codebase uses in more than one sense, with the phrase that keeps each
sense apart where context does not.

- **Leaf.** A property's leaf (`Leaf`, `StaticLeaf`, `PlatformLeaf`): the fixed value,
  token, expression or derived closure at the bottom of a `Value`, conditions choosing
  among leaves. A web leaf: one of the platforms the web facade compiles to, the browser
  leaf and the server leaf, the headless platform standing in off their targets.
- **Anchor.** A segment's anchors: the start and end markers a reactive subtree
  (`derived`, `Suspense`, `ErrorBoundary`, `Deferred`, `AsyncView`, `empty`) mounts
  between, a comment pair on the web. An overlay's anchor: the node a popup is placed
  against and toggled from, placed by CSS anchor positioning where the engine has it.
- **Store.** A `Store`: the reactive state a `#[derive(Store)]` struct lives in, read and
  written through its fields' accessors. A snapshot's store and restore: what a dev
  reload records and reads back. A platform's record store (see record).
- **Record.** A platform's record of a node: what it keeps per `NodeId` beside its
  realization, the terminal's and headless's. A snapshot record: what a run writes for
  its successor at a dev reload.
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
