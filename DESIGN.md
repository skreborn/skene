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

> **Transition status.** This document describes the target architecture. The codebase is
> mid-migration: `packages/gui` still realizes views per platform against the pre-scene
> model. `ROADMAP.md` tracks which phase the code is in.

## Crates

```
skene-reactive   reactive engine: signals, effects, memos, scopes, contexts
      │
skene-core       the spine: view spine, the SCENE (platform-neutral node tree), the
      │          style model (StyleSet + interner core), Source, Reorder, generic
      │          geometry, universal values. Knows nothing of platforms.
      │
skene-views      the built-in views (Container, Text, RichText, TextTheme, controls as
      │          they land), realized ONCE against the scene. Knows nothing of platforms.
      │
      ├── skene-web       web driver: DOM sink (csr) + HTML/CSS serializer (ssr), CSSOM
      │                   stylesheet, hydration, event delegation, painted-node canvas
      ├── skene-native    painted-native driver family: taffy layout, vello paint,
      │                   parley text, AccessKit; per-OS leaves
      ├── skene-tui       terminal driver: taffy layout in cells, buffer renderer
      │
skene            the umbrella and the single dependency an app pulls in: driver selection,
                 prelude, launch, #[main], and the driver-adjacent shared features that
                 must name the active driver (spawn, Resource, Deferred, client_only)

skene-router     routing over signals; a small per-driver URL binding seam
skene-runner     host-side library embedding a wasm runtime; exposes the app's render
                 functions to a user's server, and powers SSG
skene-macros     the #[skene::main] entry proc-macro
```

Dependency arrows only point down the first column; drivers depend on core (never on
views or on each other); the umbrella depends on everything and is the only crate that
names a driver.

## The scene

The scene is the platform-neutral realization target: views build **scene nodes** instead
of calling platform functions. It has two aspects:

- **A build-time description.** A node carries its kind, its typed properties as
  `Source<T>` (static or derived, unbound), its `StyleSet`, and its event handlers. A
  built-in view's `build` writes this data once—there is no per-platform `impl View`.
- **A persistent structural tree.** Parent/child order and node identity live in the scene
  for the app's lifetime. Structural algorithms (attach, detach, destroy walks,
  reconciliation) run here, written once.

  **Why a scene and not per-platform realization.** Three forcing functions. (1) The
  orphan rule: with views in one crate and platforms in another, `impl View for Container`
  cannot live in the platform crate, so separate driver crates are impossible under
  per-platform realization. (2) The old model wrote the same node bookkeeping twice—the
  csr and ssr leaves each kept a parallel arena with children/parents mirrors; the scene
  is that structure, hoisted and written once. (3) With taffy layout and the shared style
  model on every platform, realization decisions genuinely are shared; per-platform
  realization would copy them N times.

### Node kinds

- **Box**—a container: layout, decoration, handlers, children. `Role` guides the driver's
  concrete choice (web: `Role::Button` → `<button>`).
- **Text**—a block or inline text run with resolved text style; rich text nests runs.
- **Group**—a structural anchor with no semantics of its own (`dynamic`, `Deferred`).
- **Painted**—a custom-painted node: a vello paint callback plus a layout size. Portable
  across every *interactive* target: native drivers paint it into a layer; the web driver
  paints it into a `<canvas>` via wgpu (WebGPU; the vello CPU/hybrid path is the fallback
  where WebGPU is absent). SSR/SSG render it empty—the client fills it after hydration—and
  the TUI driver does not realize it. The paint signature is shared vocabulary; vello is
  the one renderer, everywhere.
- **Embed**—a driver-managed node. The scene records only that the node belongs to the
  driver; the payload lives in the driver's own side table, and the constructor is
  exposed by the driver crate alone. This is how a web view takes the raw `Element`
  (`web::raw_element`, csr-only), and how a native leaf hosts a real platform control
  (`ios::native_view`, an Android map, a web iframe). No `Any`, no downcast: the payload
  never crosses a crate boundary.

### How drivers realize the scene

A driver **mounts** a scene subtree: for each node it creates its artifact (DOM element,
native view, taffy node + layer, cell region) and **binds** the node's `Source`s—the
driver owns the effects, so a derived property re-applies through driver code directly.
Shared structural algorithms (`reconcile`, `dynamic`, destroy walks) live in core as
functions generic over a small driver-ops parameter, monomorphized into each driver: no
trait object, no `dyn`, no runtime dispatch. After mount, a reactive update flows
signal → effect (owned by the driver) → driver sink, without touching a diff.

  **Why drivers bind, rather than the scene notifying.** Compile-time by-name dispatch
  requires the caller to sit above the callee in the crate graph. Views sit below drivers,
  so views cannot call driver functions; drivers sit above core, so driver code can walk
  scene data and instantiate core's generic algorithms freely. Binding at mount puts every
  platform-facing call inside the driver crate, which is what keeps the whole system
  statically dispatched with separate driver crates.

  **No mock driver—but headless tests.** The scene is a real artifact, not a fiction: view
  logic (properties, structure, style resolution, reconcile behavior) is tested on the
  host against the scene, in milliseconds. Drivers test against their real sinks (web: a
  browser DOM via `wasm-bindgen-test`; ssr: wasmtime; native: its toolkit).

## The style model

Styling is **data**, not closures, because two requirements demand it: server-rendered and
statically-generated pages must be responsive and animated with **no JS/WASM**, and every
platform speaks the same layout semantics. A closure (`derive_color(move || …)`) is opaque
—a server can only serialize its current value; a `StyleSet` compiles to real CSS.

```
StyleSet {
  base:        Props,                    // the typed portable core + open passthrough
  variants:    Vec<(Condition, Props)>,  // Condition = Media(…) | State(Hover|Focus|Active|…)
  transitions: Vec<Transition>,          // property set, duration, delay, easing
  animations:  Vec<Keyframes>,
}
```

- **Layout** adopts taffy's style model as the vocabulary (re-exported/aliased): flexbox,
  grid, block—defined once by the library that implements it everywhere, rendered to CSS
  text on the web. The framework invents no layout system.
- **Decoration and text** are skene's typed properties (background, border, radius,
  cursor, text color/size/weight, …), growing as views need them.
- **Passthrough** keeps the vocabulary open: a per-family extension (web: raw CSS
  declarations) for everything the typed core doesn't model. Portable code sticks to the
  core; web-only code can reach all of CSS.
- **Web** renders a `StyleSet` into one interned class: base declarations, `@media`
  blocks, `:hover`/`:focus`/`:active` rules, `transition:`, `@keyframes`. State- and
  media-driven styling therefore works in a static document with zero script.
- **Native/TUI** resolve `Condition` against driver signals (window size, pointer state,
  focus) and run transitions/animations on the driver's frame ticker—not through the
  reactive graph per frame. `derive_` properties remain for genuinely dynamic values; they
  are the escape hatch, not the mechanism.

### The interner

The content-addressed class interner is load-bearing and its contract is preserved
verbatim from the pre-scene design: a style set is hashed to a deterministic name
(`skene-{hash:x}`, forward probe on collision), interned once, reference-counted by live
node, collected when unused. Identical styles collapse to one class; determinism makes the
server's stylesheet byte-compatible with the client's, which hydration relies on. The
interner core is pure and lives in `skene-core`; each driver layers its sink on the
outcomes (web csr: a live `CssStyleSheet`, rule in on intern, rule out on collect; ssr:
render live rules once at the end). On native the same interning shares resolved style
objects across nodes—the computed-style sharing browsers get from classes.

## Reactive engine

The public API is stable: `RwSignal`/`ReadSignal`/`WriteSignal` (Copy handles),
`Memo`, `Effect`, `Trigger`, `Scope`, `Context`, the `Sync`/local storage split, and the
`set_`/`derive_`/`handle_` builder convention above it. The internals contract:

- **Eager staleness, lazy recomputation.** A write marks dependent memos stale
  transitively at write time; a memo read pulls freshness through the chain. A memo is
  therefore never stale to a reader—including inside `batch`—and a memo whose recomputed
  value is unchanged still notifies no one (memos filter change).
- **Non-reentrant flushing.** Effects triggered while a flush is running are enqueued and
  drained by the same flush, not run recursively. Driver event dispatch wraps in an
  implicit batch, so N writes in one handler run each dependent effect once.
- **Arena storage.** Signals, effects, scopes and subscription edges live in slotmap-style
  arenas with small-vector edge lists, not per-entity `Rc<dyn Any>` in hash maps. (The
  five-map layout predates this; the swap is API-invisible.)
- **Threading.** The UI thread owns the local runtime. `Sync` signals live in a global
  runtime; an off-thread write enqueues its subscribed effects and pokes the installed
  waker; the driver drains on its loop (`drain_pending_work`). The browser build is
  single-threaded and never queues; the machinery earns its keep on native.

## Drivers

### Web (`skene-web`)

Two leaves, selected by target and enforced by it—`web-sys` does not exist on
`wasm32-wasip2`, so the server leaf *cannot* touch the DOM:

- **csr** (`target_os = "unknown"`)—the browser. DOM artifacts, the CSSOM stylesheet sink,
  root-level event delegation (one listener per event type, dispatch by node id),
  hydration (creation-order `data-h` markers, loud mismatch), `Painted` via
  `<canvas>` + wgpu, `Embed` for raw-element views.
- **ssr** (`target_os = "wasi"`)—the serializer. Walks the scene to HTML plus the interned
  stylesheet; drives async to a fixpoint on a WASI-`poll` reactor before serializing;
  streams in document order around `Deferred` boundaries; serializes resource state for
  the client to adopt. `Painted` and `Embed` render empty; handlers are inert.

The app compiles to wasm for both leaves; the same module serves CSR in the browser and
SSR/SSG in a runner.

### Native (`skene-native`)

One family; the design premise is that **the platform composites**. Where the OS has a
real view hierarchy (iOS, Android, macOS), the driver realizes the scene *into* it: taffy
computes frames, painted content renders through vello into layers that are themselves
views in that hierarchy, and an `Embed` is an ordinary native view (a `UITextField`, a
map). Sibling order is z-order, so custom-painted content and real platform controls
interleave for free—compositing is the platform's duty, not the framework's. The driver
decides its own realization granularity internally (view-per-node versus flattened painted
layers); the invariant is only that scene order maps to compositor order.

The Linux desktop leaf has no native control set to embed, so it is the plain
winit + single-vello-surface case; Windows sits between (composition visuals) and its leaf
chooses. Text is parley; accessibility is AccessKit; per-OS glue (IME, clipboard, menus)
lives in each leaf. Off-thread work uses the reactive engine's waker/drain pair wired to
the event-loop proxy, and a loop-integrated timer is fair game here (the loop is ours).

### TUI (`skene-tui`)

Layout, no painting: taffy with `1.0 = 1 cell`, rendered to a terminal buffer. `Painted`
and `Embed` have no realization. TUI is deliberately the cheapest driver—it doubles as
the proof that the scene is platform-neutral.

## Selection and entry

An app depends on `skene` with **exactly one driver feature** (`web`, `desktop`, `tui`,
…); zero or two is a `compile_error!`. Target-gating remains the enforcement layer where
it applies (wasm32 → web; `target_os` splits csr/ssr), but it is no longer the selector:
desktop and TUI share a target triple, so selection by dependency/feature is the primary
mechanism.

The umbrella is the one crate allowed to name the active driver, which is why the shared
driver-adjacent surface lives there: `launch`, `#[main]` (per-driver entry glue),
`spawn` (scope-bound cancellation over the driver's executor seam), `Resource`
(serialize-by-default across the SSR boundary, `Resource::client` opt-out), `Deferred`,
`client_only`, and the interactive-only wrappers. App code names no platform.

## Async, data, and third parties

HTTP, WebSockets and timers are **userland**, deliberately (the browser owns the loop on
the web; owning an HTTP stack is permanent debt). The framework ships only the glue no
external crate can provide, and ships it as *public seams* so third-party integrations sit
on exactly the footing first-party ones would:

- `spawn`—resume on the UI thread, scope-bound cancellation.
- `Resource`—load into a signal, hydrate across the SSR boundary, refetch.
- The SSR reactor's `register(pollable, waker)`—public in `skene-web`, so any client
  library that speaks `wasi:*` pollables (an HTTP client on `wasi:http`) blocks the render
  until its load settles, exactly as the built-in `sleep` does.
- `Boundary`/`Deferred`—any async inside a boundary defers its streamed chunk.

## Router (`skene-router`)

Routing is state: a typed route as a signal, shared by every platform. Each driver binds
it thinly—web csr to the History API, ssr takes the route as render input, native/tui keep
it in memory (deep links later). SSG enumerates the route table to know what to render.
The router depends on `skene-reactive` only.

## SSR, SSG, and the runner (`skene-runner`)

Server-side rendering is a *capability*, not a server: `render_to_string` /
`render_to_stream` produce the parts (styles, body, state) or ordered chunks, and
`Rendered::page` assembles a document. The app compiles once to `wasm32-wasip2`; the
runner—a plain Rust library embedding wasmtime—loads that module and exposes the render
calls to whatever server the user already has, plus a dev server. SSG is the same runner
run at build time: enumerate routes, render each, write files. Static output is fully
functional without script (styling, media queries, hover, transitions, animations—all
CSS); hydration upgrades it to interactive when the wasm loads.

## Logging

Framework and app code log through `tracing`. Each driver installs its subscriber at
launch (web → console; native → logcat / `os_log` / stderr). The spine carries no
subscriber.

## Superseded decisions

Recorded because they were load-bearing and the reasons they fell matter:

- **Per-platform view realization** (shared builder, per-platform `impl View`) fell to the
  orphan rule the moment drivers became separate crates, and to the duplication it forced
  (the csr/ssr leaves each maintained the same node bookkeeping). The scene replaces it.
- **"No framework style model"** existed so no platform would be forced onto a foreign
  vocabulary. Taffy-everywhere plus no-JS responsive SSG *is* a shared vocabulary by
  requirement; the model is now framework-defined but open (passthrough), and each driver
  still owns its sink.
- **"Selection is the target, only"** cannot distinguish desktop from TUI on one triple.
  Target-gating is demoted from selector to enforcement.
- **"No generic platform to test against"** was true when there was nothing real between
  views and platforms. The scene is real; headless view tests assert on it.
- **A `Backend` trait object** (pre-redesign) cost `unsafe`, `Box::leak`, and a fallible
  downcast to re-discover what the compiler knew. Still rejected: drivers are statically
  selected, and the generic driver-ops parameter monomorphizes—there is no `dyn` driver
  anywhere.
