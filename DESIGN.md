# Design

The architecture of skene: facts and the reasons behind the load-bearing decisions.
Conventions and invariants live in `AGENTS.md`; the phased plan lives in `ROADMAP.md`.

Skene is a **client-side** GUI framework. One app, written against one dependency, runs on
whatever platform the build selects: the web (client-rendered, server-rendered, or
statically generated), painted-native desktop and mobile, and the terminal. The reactive
engine is signal-based and fine-grained; there is no virtual DOM and no tree diffing. The
framework ships no server-side features—no server functions, no backend integrations. What
it ships instead is a renderable artifact and an embeddable runner, so any server can serve
a skene frontend without the framework becoming a server framework.

The building block is a **view** (the `View` trait). "View" is the canonical term
throughout—there is no separate "widget" concept.

## Crates

```
skene-reactive   the engine: signals, memos, effects, owners, contexts, the scheduler seam
      │
skene-core       the spine: Scene<D> (parents, children, annotations, token definitions,
      │          generational node ids), the Driver trait, event dispatch over the
      │          scene, the property model (BoxStyle<N, S>, RunStyle<S>, visitors),
      │          Value/Leaf/Expr, Media/State/Condition, Token/TokenSet, Boundary,
      │          generic geometry, universal values. Knows nothing of platforms.
      │
skene-views      every built-in view ONCE, generic over D: Driver—Container, Row/Column/
      │          Grid/Stack/LayoutBox, Flexible/Pinned/GridCell, Text/Span/RichText,
      │          Control<B: Behavior> and the shipped behaviors, Overlay, LazyColumn,
      │          Suspense/ErrorBoundary/Deferred, Theme
      │
      ├── skene-web       Driver for the web: DOM sink (csr) + HTML/CSS serializer (ssr)
      │                   over one node record, the interner, hydration, the WASI reactor;
      │                   escape traits WebMarkup (both leaves) and WebDom (browser only)
      ├── skene-native    Driver for the painted-native family: taffy, vello, parley,
      │                   AccessKit; per-OS leaves; escape trait PlatformView
      ├── skene-headless  Driver for the host: realizes into an inspectable tree so view
      │                   logic tests run without a platform; implements the whole trait
      └── skene-tui       the terminal facade AND its driver: taffy in cells, a cell
                          buffer; escape trait CellPainter

skene-gui        the GUI facade and a GUI app's single dependency: f32, the GUI Media and
                 State leaves, the GUI style extension (radius, transitions), Typography,
                 media::*, launch, #[main], driver selection by target, and aliases
                 (type Container = skene_views::Container<ActiveDriver>)
skene-tui        the terminal facade, EQUAL in standing: u32 cells, the terminal's leaves,
                 its extension (glyphs, shadow), Typography, Glyphs, media::*, launch
skene-simple     the Simple design language over skene-views, generic over D: the palette
                 (a TokenSet) and the Simple* views over Control<B>
skene-router     routing over signals; a small per-facade URL binding seam
skene-runner     host-side library embedding a wasm runtime; exposes the app's render
                 functions to a user's server, seeds media from client hints, powers SSG
skene-macros     the #[skene_gui::main] entry proc-macro, #[lazy], the Store derive
```

**Two facades, one spine.** GUI and TUI are different media, not one medium with
fallbacks: a GUI app targets any combination of web and native and genuinely converges;
a terminal app is written for the terminal on purpose, and never cross-compiles into the
other family. Each facade therefore owns its builder vocabulary outright, typed to
exactly its capabilities—and everything beneath it is written once in the spine. The
facade's own contribution is small by design: a scalar, the medium's condition leaves,
the properties only that medium has, the driver it selects, and the aliases that give
app code a flat, medium-typed surface. What a medium lacks is absent at compile time
because the driver's associated types say so.

Dependency arrows only point down; drivers depend on core (never on each other); each
facade names only its own drivers and publishes the selected one as `ActiveDriver`.

## The scene and the driver

**Core owns the tree; a driver owns a sink.** `Scene<D: Driver>` is a generational arena
of node records—parent, children, the annotation a wrapper asked for the child, the
token definitions a theme placed on the node, the query-container name, and the driver's
sink handle (`Element`, a taffy id, a string node, a record). Every structural mutation
(`attach`, `detach`, `destroy`, reorder) is implemented once in core; the driver is
called only to mirror it into the sink. `NodeId` carries a generation, so a stale id
resolves to nothing rather than to a recycled node. Token resolution and event dispatch
walk this tree; nothing reads structure back from a platform.

**`Driver` is a zero-sized trait with associated types**, monomorphized, hidden behind
each facade's aliases: `Sink`, `Scalar` (`f32` or `u32`), `Media` and `State` (the
medium's condition leaves), `StyleExt` (the medium's own properties), `TokenValue` (the
erased definition the medium accepts). Its methods are the portable sink vocabulary:
create and update boxes and runs, create anchors, mirror tree mutations, define tokens on
a scope, register event kinds, focus, set values, observe sizes, report media. `Painted`
and embeds are deliberately outside it—their payloads never cross a crate boundary—and
arrive through extension traits (`Painting`; each driver's escape trait). The leaf-only
surface (`document`, `element`, `render_to_string`, `launch`) stays free functions in the
driver crate, since it is not a shared contract.

- **Why a trait and not name-resolved free functions.** The previous vocabulary was
  sixty-nine functions the host driver silently lacked a third of, because a missing
  name is only an error on a path that names it, and those paths were target-gated. A
  trait makes the gap a hard error, which is what let the host driver implement all of
  it and made `Resource`, `Deferred`, `Lazy`, `Link`, the head and the router testable
  headlessly.
- **Why the scene is in core when a core-owned tree was once rejected.** The rejection
  was of a tree that *calls* drivers, which by-name dispatch made impossible. The scene
  calls nothing; core mutates it and then calls the driver through the generic
  parameter, exactly as `reconcile` always called `TreeOps`. Every driver already kept
  this structure privately, with four subtly different reparent semantics; one
  implementation is smaller and correct.
- **Dispatch is core's.** A driver reports an event at a node; core walks the scene's
  parents, runs handlers deepest first, folds `Prevented`, and tells the driver what the
  platform may still do. The browser's root delegation, the terminal's hit path and the
  headless driver share one bubbling implementation, so the rule that a view owns the
  interactions it handles is enforced, and testable, in one place.
- **Statics batch, deriveds bind.** A builder's fixed properties, token references and
  expressions arrive together in the create call—the web folds them into one
  content-addressed class—while each derived leaf binds a render effect that writes its
  per-node slot. The split is written once against the property visitor.
- **`Role` guides the driver's concrete choice** (web: `Role::Navigation` → `<nav>`); the
  vocabulary stays semantic, never platform-worded.
- **Painted**—a vello paint callback plus a layout size, portable across every
  *interactive* pixel target through the `Painting` extension. Native drivers paint it
  into a layer; the web driver paints it into a `<canvas>` via wgpu (the vello CPU/hybrid
  path is the fallback where WebGPU is absent). SSR/SSG render it empty and the terminal
  does not realize it. Vello is the one renderer, everywhere.
- **Embeds are driver-specific views**, written against a driver's escape trait: raw DOM
  through `WebDom`, later a `UITextField` or a map through `PlatformView`. There is no
  `Any` and no downcast.

  **Why a typed vocabulary and not per-platform realization.** The orphan rule: with
  views in one crate and drivers in others, `impl View for Container` can live in neither
  driver, so separate driver crates are impossible under per-platform realization. Typing
  the vocabulary moves the platform decision (CSS text, native styles) into the driver
  while realization is written once above it.

  **No mock driver—but headless tests.** Because the vocabulary carries typed data, the
  host driver (`skene-headless`) realizes exactly what any real driver receives into an
  inspectable tree, giving view logic fast headless tests without pretending to be a
  platform. Drivers test against their real sinks (web: a browser DOM via
  `wasm-bindgen-test`; ssr: wasmtime under a byte-exact snapshot; native: its toolkit).

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
  content. A group node that was a real `<div>` and a real taffy node made every
  derived subtree one flex item.
- **`Children` is a list of segments**, static or keyed, each between its own anchors;
  `set_children` and `derive_children` append. A header beside a reconciled list in one
  `Row` needs no nested container. The child kind is carried on `Children<D, Kind>` and
  the setter that bypassed it is sealed.
- **Keyed reconcile** keeps its LIS-minimal reorder, its reused scratch buffers, and
  builds items untracked; a rebuild closure (`derived`) tracks. The two names say which.
- **Structural change is a last resort with a cheap alternative.** `derived` destroys
  and rebuilds, disposing the subtree's scope with its state; `Show` and `Switch` build
  branches once and attach or detach them, preserving state. `Suspense` and
  `ErrorBoundary` are built on them and destroy the losing branch on disposal.
- **Wrappers annotate before creation.** `Flexible`, `Pinned` and `GridCell` write their
  annotation into the scene slot the next box built under them consumes into its own
  props, so per-child layout is an ordinary property of the child's style: in the class
  on the web, conditional, token-valued or derived like any other.
- **Owners** are created per reconciled item, per rebuild, and where context is
  provided; everything else inherits the ambient owner. `on_cleanup` returns a handle so
  a finished task removes its entry.

## Controls

A control is `Control<D, B: Behavior>`: one builder carrying focus, events, decoration,
label, parts and accessibility once, over a public `Behavior` that says what the
interaction *is*—its role, its keys, its pointer handling, what it reports to assistive
technology, where it sits in the tab order, and which parts it exposes. `Toggle`,
`Range`, `Roving`, `Popup`, `Activate` and `TextEntry` ship; `RawCheckbox`,
`RawSlider`, `RawRadioGroup`, `RawPicker`, `RawButton` and the text fields are aliases
over them. The `Simple*` components (`skene-simple`) are the ready-made library over the
same builder: one palette, one shape language, shared across the GUI and the terminal
wherever the medium allows, and deliberately nothing a third-party library could not
build on the same footing.

- **A control decides its verdict.** The behavior runs first and its verdict is the
  whole verdict; a user's handler observes. A button's click never reaches the row it
  sits in, on any facade.
- **A control is controlled.** It reports; the bound signal is the truth. Two-way
  binding is a derived read plus a handler write, on every facade, and the terminal no
  longer mutates its own bit.
- **Parts and control state.** A part is a named child the styling targets (`Track`,
  `Fill`, `Thumb`, `Mark`, `Face`, `List`, `Item`, `Label`); in the terminal a part maps
  to a glyph token. A `State` a box cannot have on its own (`Checked`, `Indeterminate`,
  `Disabled`, `Invalid`, `Open`) resolves against the enclosing control; a node's own
  `Hovered` and `Focused` stay its own. On the web a control that owns a native input
  compiles control state to a selector anchored on that input; natively it is the
  control's own state.
- **The scriptless form is the control.** Checkbox, radio, slider and the text fields
  keep their native input inside the label, visually hidden but focusable, so keyboard,
  form participation and accessibility are the browser's before and after hydration and
  nothing is swapped. The picker keeps a swap (no engine but Chromium restyles an open
  `<select>` yet) and the swap carries the reader's value.
- **The terminal's controls are composed trees**—a `Container`, a `Text` mark from a
  glyph token—painted by the same painters as everything else, so they carry roles,
  accessibility properties and focus order, and the keyboard tables live in the
  behaviors rather than in a closed match per medium.

## The style model

Styling is **data**, not closures, because two requirements demand it: server-rendered and
statically-generated pages must be responsive, themed and animated with **no JS/WASM**,
and every platform speaks the same layout semantics. A closure is opaque—a server can
only serialize its current value; style data compiles to real CSS and evaluates natively.

**One property model, spelled once.** `BoxStyle<N: Scalar, S: Shape, X: StyleExtension>`
and `RunStyle<S, X>` name every shared property once; the leaf shape is a type parameter
(`Declared` for a builder, `Static` for a driver, `Plain` for a keyframe or a resolved
style) and the medium's own properties come through the extension. A visitor written once
gives overlay, the static/derived split, CSS rendering and taffy application their
iteration; what remains per property is the field, its CSS name and its taffy mapping.

**A property value is a base and its conditions, and each leaf is one of five kinds.**

```
StaticLeaf<T> = Fixed(T) | Token(TokenRef<T>) | Expr(Expr<T>)
Leaf<T>       = Static(StaticLeaf<T>) | Derived(Rc<dyn Fn() -> T>)
Value<T>      = Conditional<Leaf<T>, Condition>          // what a setter takes
Definition<T> = Conditional<StaticLeaf<T>, Media>        // what a token is; no closures
Media         = MinWidth | MaxWidth | MinHeight | MaxHeight | Dark | Light | ReducedMotion
              | ReducedTransparency | HighContrast | Container { name, query } | And | Or
State         = Hovered | Focused | FocusVisible | Pressed              // the node's own
              | Checked | Indeterminate | Disabled | Invalid | Open      // the control's
Condition     = Media | State | And | Or
```

- **Fixed** leaves render as literals. **Tokens** render as `var(--name)` on the web and
  resolve through the scene elsewhere. **Expressions** are typed arithmetic that compiles
  to `calc()`, `clamp()`, `color-mix()` and relative color and evaluates natively:
  lengths and numbers have the ring operations CSS type-checks, colors have only the named
  functions, and there is no boolean expression, so an ill-formed expression does not
  compile. **Derived** leaves are the script-only kind: on the web a per-node custom
  property slot the class references (registered `inherits: false`), written by a render
  effect, so a derived base never outranks a variant; on the server, evaluated once. A
  definition admits no closure, which is what keeps a palette plain data that
  serializes, hashes and renders without script.
- Conditions ride the value: `value.when(condition, other)`. Each facade owns its
  `Media` and `State` leaves, and the composition machinery (`Composable`: `holds`,
  DNF expansion) is one generic in the spine; the web emits one rule per conjunction and
  groups a node's properties by canonical condition, the terminal and native evaluate
  against live state. Container queries are `Media::Container` leaves against a box that
  named itself; the web stamps `container-type` and emits `@container`, the terminal and
  native resolve them in a second pass with the container's measured axis fixed, which is
  sound for the reason CSS makes it sound.
- **Layout** runs on taffy at full capability, statically constrained per layout: each
  mode is its own container view carrying layout properties alone, `Container` is the one
  decorated box, and per-child properties ride in wrappers. `Dimension` is a fixed length,
  a `Fraction` of the parent, or auto. The framework invents no layout system.
- **Passthrough** keeps the vocabulary open: a per-family extension (web: raw CSS
  declarations) for everything the typed core does not model.
- **Web** renders the whole set into one interned rule template: base declarations,
  `@media`, `@container` and pseudo-class rules, `transition:`, content-addressed
  `@keyframes`, `var()` for tokens and slots. State-, media- and theme-driven styling
  therefore works in a static document with zero script.
- **Native/TUI** resolve every property at one point—token lookup, expression
  evaluation, variant overlay—cached per node under a generation covering environment,
  interaction and definitions, and reporting whether the resolved value changed. That
  report is where a transition starts.

### Tokens and themes

A design token is a named, inherited slot. `TokenRef<T>` names it (a `const`; the name is
an implementation detail on every platform), `Token<T>` pairs the name with the
definition a palette gives it, and a consumer emits only the name, so two boxes under two
palettes intern one class and a theme change never touches a consumer. A token's own
conditions live in its definition, once.

`Theme<D, P: TokenSet>` is the one theme view. It realizes one layout-invisible scope node
(`display: contents` on the web, a group record in the terminal), provides `Memo<P>`
through context, and runs one effect that writes `P::definitions()` onto the scope. A
palette change therefore costs the number of tokens, never the number of nodes, and
nothing rebuilds; nesting a theme redefines beneath it. On the web the static definitions
are an interned class on the scope element (with `@media` blocks and the scheme double
emission where a definition conditions on the scheme), an unconditional redefinition is
written inline, a conditional one re-interns and swaps the class; a token set used with
no theme above registers its default under `:root` once. `Typography` (all text
inheritance on both facades) and the terminal's `Glyphs` are token sets, so the framework
has one inheritance mechanism. Nothing below a component library knows what a palette
is: `skene-simple` owns `SimplePalette`, and a third party's `AcmePalette` gets
`Theme<D, AcmePalette>`, `current()` and `watch()` from the same blanket.

The color scheme is one `Media` leaf, answered per property in a definition or a value.
The scheme *in force* is the document's: `media::set_scheme` writes `data-skene-scheme`
on the root (with `color-scheme`), every scheme-conditional rule is emitted twice—the
preference query guarded by `:where(:root:not([data-skene-scheme="light"]))` and the
choice as `:where(:root[data-skene-scheme="dark"])`, both in `:where()` so they add no
specificity—and the terminal keeps the choice beside its detected background. A
server-rendered page that offers a choice emits one inline blocking script that applies
the stored choice before first paint. Scoping a scheme to a subtree is not wanted.

### Environment

`media::viewport()`, `media::scheme()` and `media::reduced_motion()` are signals on every
facade, backed by the driver: `MediaQueryList` and `resize` in the browser, the
terminal's size and background, the native window. `Media::watch()` turns any declared
condition into a memo rendered from the same query text the class compiler renders, so
the signal and the stylesheet cannot disagree. A `SizeHandle` bound to a box reports its
measured size. On the server these hold the client hints the runner was given
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
interned since the previous flush). On native the same interning shares resolved style
objects across nodes.

## Reactive engine

The public API speaks the ecosystem's standard reactive vocabulary:
`Signal`/`ReadSignal`/`WriteSignal` (Copy handles), `Memo`, `Effect` with an
`EffectHandle`, `Trigger`, `Selector`, `Owner`, `provide_context`/`use_context`,
`batch`/`untrack`, the access traits, the `Sync`/local storage split, and the
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
- **Effects have handles and flavors.** `RenderEffect`s flush before ordinary effects in
  owner-depth order and are what property binders are; `watch` is the fine-grained
  binder; a handle disposes or pauses.
- **A pluggable scheduler.** Synchronous at write end by default; a driver installs a
  frame-tied flush with `flush_sync` for the cases that need it. The sync waker routes
  through the same scheduler. The animation clock on native and the terminal lives here.
- **Panic and borrow discipline.** `batch` and `Owner::enter` are guarded; no user code
  runs under a runtime borrow; an entity is moved out of its arena before its cleanups
  run.
- **Threading.** The UI thread owns the local runtime. `Sync` signals live in a global
  registry; an off-thread write enqueues and pokes the scheduler. The browser build is
  single-threaded and never queues; the machinery earns its keep on native.

## Motion

`Transition` and keyframe `Animation` are data. The web compiles them to CSS; native and
the terminal run them on the driver's frame clock, per value type (colors in Oklab,
lengths linearly, the terminal quantizing to cells and truecolor), from the resolution
point that reports a changed value—never through the reactive graph per frame. Every
change source animates: a condition flipping, a token redefined, an expression input
moving, a derived write. Reduced motion is respected by default. Keyframes may hold
tokens. `set_transition_name` maps to `view-transition-name`; the router wraps a
same-document navigation in `startViewTransition` when the app opts in, the stylesheet
emits `@view-transition` for cross-document navigations, and native snapshots the
outgoing tree into a layer and interpolates named pairs; all degrade to a cut.

## Drivers

### Web (`skene-web`)

Two leaves over one node record, selected by target and enforced by it—`web-sys` does not
exist on `wasm32-wasip2`, so the server leaf *cannot* touch the DOM:

- **csr** (`target_os = "unknown"`)—the browser. DOM sink, the CSSOM stylesheet sink,
  root-level event delegation with a stamped node id (one listener per event type,
  `scroll` and enter/leave included), one shared `ResizeObserver`, hydration
  (creation-order `data-h` markers, loud mismatch, the server's stylesheet adopted rather
  than duplicated, a live input value that differs from the serialized one written into
  its signal), scroll and focus restoration by node id, the `WebDom` escape trait, later
  `Painted` via `<canvas>` + wgpu.
- **ssr** (`target_os = "wasi"`)—the serializer. Renders its string tree to HTML plus the
  interned stylesheet; drives async to a fixpoint on a WASI-`poll` reactor before
  serializing; streams in document order around `Deferred` boundaries with a style delta
  per chunk; serializes resource state for the client to adopt; takes client hints as
  render input. The raw handles do not exist here, painted/embedded content renders
  empty, and handlers are inert.

The app compiles to wasm for both leaves; the same module serves CSR in the browser and
SSR/SSG in a runner.

### Native (`skene-native`)

One family; the design premise is that **the platform composites**. Where the OS has a
real view hierarchy (iOS, Android, macOS), the driver realizes the tree *into* it: taffy
computes frames, painted content renders through vello into layers that are themselves
views in that hierarchy, and an embed is an ordinary native view. Sibling order is
z-order, so custom-painted content and real platform controls interleave for free. The
driver decides its own realization granularity internally; the invariant is only that
tree order maps to compositor order. The resolution cache tells it whether a
layout-affecting value moved, so a palette write restyles taffy only when it must.

The Linux desktop leaf has no native control set to embed, so it is the plain
winit + single-vello-surface case; Windows sits between and its leaf chooses. Text is
parley; accessibility is AccessKit; per-OS glue (IME, clipboard, menus) lives in each
leaf. Off-thread work uses the engine's scheduler wired to the event-loop proxy, and a
loop-integrated timer is fair game here (the loop is ours).

### TUI (`skene-tui`)

A facade in its own right, not a driver of the GUI vocabulary. Its views are the shared
ones typed to the terminal: lengths are `u32` cells, text is a color plus a `bool` bold,
a border is one cell of box-drawing characters in a color, its conditions are cells and
its own background. Nothing the medium lacks exists to call, so a terminal app is
first-class at compile time rather than best-effort at run time.

The mouse is in-capability: motion drives `Hovered` variants and pointer handlers, a
left click hits the topmost node and runs its handlers through core's dispatch, a
hovered node's `cursor` sets the pointer shape via OSC 22—terminals without mouse
reporting or pointer shapes degrade quietly. Selection is anchored in source space so it
survives reflow; wheel gestures latch their target. Focus belongs to controls, which are
composed trees with roles and focus order, exactly as on the GUI.

Layout is taffy at `1.0 = 1 cell`, with the effective style cached per node and
`set_style` called only on change, so a keystroke relays out the changed subtree and a
hover change repaints without relayout. A frame paints over the previous one, rewriting
only the rows the changed nodes covered and cover—a node whose resolution, text, caret,
scroll or rectangle changed—and every row for a whole-frame change; the terminal loop
then diffs rows again on output. A paragraph lays its inline runs out as one wrapped
text, and a view inside it flows as an inline box: laid out on its own, standing in the
text as a placeholder of its width, painted and hit at the cells it flowed into. A
border of zero width takes no cells. Frames paint into a cell buffer—to the terminal
through `launch` (crossterm: raw mode, truecolor, mouse capture), or to plain text
through `render_to_text` for headless tests (`trigger_key`/`trigger_mouse_move`/
`trigger_mouse_click` drive it without a terminal). Terminal detection is asynchronous;
the loop starts immediately and the answers arrive as signal writes. Terminal apps never
cross-compile into the GUI family; the write-once promise belongs to web+native.

## Third parties and platforms

Two guarantees, both properties of what is public rather than of any one API.

- **A component library, for every platform, from one crate.** It depends on
  `skene-views`, writes generic over `D: Driver` exactly as the built-ins are written, and
  states what it needs from a medium as a bound on the driver's associated types
  (`D::StyleExt: HasCornerRadius`). Each facade's aliases make it concrete for the app
  that uses it. Nothing on the path a built-in takes is crate-private.
- **A low-level view, against the platform, like a built-in.** It is written against a
  driver's escape trait (`impl<D: Driver + WebDom> View for Map<D>`) and realized through
  the same scene and node ids as a `Container`, so it composes with wrappers, themes,
  conditions and events. Selecting the driver is what selecting the facade already does.
  The portable low-level path for pixel media is `Painted`.

A driver need not be first-party: a crate that implements `Driver`, provides its scalar,
leaves, extension, a `launch` and aliases over `skene-views` is a facade. `Driver` is not
sealed, and the shared machinery bounds only on it and the public extension traits; the
headless driver is exactly such a crate in all but ownership.

## Selection and entry

Selection is by **facade dependency** first—a GUI app depends on `skene-gui`, a
terminal app on `skene-tui`—and by target within a facade: `skene-gui` resolves wasm32
to the web driver and will resolve native triples to the native driver, with no feature
flag anywhere. Target-gating stays the enforcement layer where it applies. A cargo
feature may only add items; never a bound, a type, a variant or a driver.

A facade is the one place allowed to name its drivers, which is why the shared
driver-adjacent surface lives there: `launch`, `#[main]` (per-driver entry glue),
`spawn`, `Deferred`, `client_only`, `media`, and the interactive-only wrappers. App code
names no platform.

## Async, data, and third parties

HTTP, WebSockets and timers are **userland**, deliberately (the browser owns the loop on
the web; owning an HTTP stack is permanent debt). The framework ships only the glue no
external crate can provide, and ships it as *public seams* so third-party integrations sit
on exactly the footing first-party ones would:

- `spawn`—resume on the UI thread, scope-bound cancellation.
- `Resource`—in the engine: load into a signal with a `Loading`/`Ready`/`Refetching`
  state that keeps the last value; the web leaf adds hydration across the SSR boundary.
- The SSR reactor's `register(pollable, waker)`—public in `skene-web`, so any client
  library that speaks `wasi:*` pollables blocks the render until its load settles.
- `Boundary`/`Deferred`—any async inside a boundary defers its streamed chunk.

## Router (`skene-router`)

Routing is state: a typed route as a signal, shared by every platform. Each driver binds
it thinly—web csr to the History API (wrapping a navigation in a view transition when
the app opts in), ssr takes the route as render input, native/tui keep it in memory. SSG
enumerates the route table to know what to render. The router depends on
`skene-reactive` only.

## SSR, SSG, and the runner (`skene-runner`)

Server-side rendering is a *capability*, not a server: `render_to_string` /
`render_to_stream` produce the parts (styles, body, state) or ordered chunks, and
`Rendered::page` assembles a document. The app compiles once to `wasm32-wasip2`; the
runner—a plain Rust library embedding wasmtime—loads that module and exposes the render
calls to whatever server the user already has, parses client hints into `RenderHints`
and emits the response headers that request them, plus a dev server. SSG is the same
runner run at build time. Static output is fully functional without script (styling,
media and container queries, hover, themes, transitions, animations—all CSS); hydration
upgrades it to interactive when the wasm loads.

## Logging

Framework and app code log through `tracing`. Each driver installs its subscriber at
launch. The spine carries no subscriber.

## Superseded decisions

Recorded because they were load-bearing and the reasons they fell matter:

- **Per-platform view realization** fell to the orphan rule the moment drivers became
  separate crates, and to the duplication it forced. The typed vocabulary replaces it.
- **"No framework style model"** existed so no platform would be forced onto a foreign
  vocabulary. Taffy-everywhere plus no-JS responsive SSG *is* a shared vocabulary by
  requirement; the model is framework-defined but open (passthrough).
- **"Selection is the target, only"** cannot distinguish desktop from TUI on one triple.
  Target-gating is demoted from selector to enforcement.
- **A `Backend` trait object** cost `unsafe`, `Box::leak`, and a fallible downcast. Still
  rejected: drivers are statically selected and the driver parameter monomorphizes.
- **"The built-in views live in the umbrella."** True with one facade; with two it
  produced two umbrellas and two copies, sixty-five percent of them character-identical.
  The generic parameter already used for `Children` is applied to every view in
  `skene-views`.
- **"No persistent scene tree in core."** The objection stood for a tree that calls
  drivers. The scene calls none; every driver kept the structure anyway, four ways.
- **"The vocabulary is free functions resolved by name."** A missing function was only an
  error on a target-gated path, which let the host driver lack a third of it. A
  zero-sized trait makes the contract complete.
- **"A token's conditions land beside the property as ordinary variants."** They were
  copied into every use site, doubled by the scheme guard, and a palette could change
  only by a rebuild that lost state. A token is a named inherited slot; its conditions
  live in its definition; a theme redefines it by one effect.
- **"A derived value is an inline override."** Inline outranks every variant on the web
  and not in the terminal. A derived leaf is a per-node custom-property slot the class
  references.
- **`set_<prop>_when`, `SimpleTheme`, `TextTheme`, `ControlTheme`, the `Web*` toggles and
  range, the per-control `Raw*` structs, group nodes, `skene-record`.** Each is replaced
  by a construct named above; the reasons are recorded in `ROADMAP.md`'s redesign entry.
