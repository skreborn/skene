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
skene-reactive   reactive engine: signals, effects, memos, scopes, contexts
      │
skene-core       the spine: view spine, the typed property vocabulary (BoxProps,
      │          RunProps, later StyleSet), Source, Boundary, Reorder, generic
      │          geometry, universal values. Knows nothing of platforms.
      │
      ├── skene-web       web driver: DOM sink (csr) + HTML/CSS serializer (ssr), the
      │                   CSSOM stylesheet and interner, hydration, the WASI reactor
      ├── skene-native    painted-native driver family: taffy layout, vello paint,
      │                   parley text, AccessKit; per-OS leaves
      ├── skene-tui       terminal driver: taffy layout in cells, buffer renderer
      │
skene            the umbrella and the single dependency an app pulls in: the built-in
                 views realized ONCE against the active driver, the composition helpers
                 (Children, reconcile, dynamic), driver selection, prelude, launch,
                 #[main], spawn, Resource, Deferred, client_only

skene-router     routing over signals; a small per-driver URL binding seam
skene-runner     host-side library embedding a wasm runtime; exposes the app's render
                 functions to a user's server, and powers SSG
skene-macros     the #[skene::main] entry proc-macro
```

Dependency arrows only point down; drivers depend on core (never on each other); the
umbrella depends on everything and is the **only** crate that names a driver.

## The typed driver vocabulary

Views are realized **once**, in the umbrella, against a driver-neutral vocabulary of
typed functions every driver crate exports under the same names—`create_box(&BoxProps)`,
`update_box(node, &BoxProp)`, `create_run(&RunProps)`, `update_run`, `set_text`,
`create_group`, `attach`/`detach`/`destroy`, `handle_click`, plus the async and hydration
seams (`drive`, `is_server`, `adopt_next`, `register_source`). The umbrella resolves
`driver::` to the enabled driver at compile time and writes each `impl View` exactly one
time; a driver maps the typed data to its own sink and keeps its own retained state.

- **Statics batch, deriveds bind.** A builder's fixed properties arrive together in the
  create call—the web driver folds them into one content-addressed class—while each
  `derive_` property binds an effect that re-applies through the per-property update call
  (web: a single-declaration inline override). The split is written once, in the umbrella.
- **`Role` guides the driver's concrete choice** (web: `Role::Button` → `<button>`); the
  vocabulary stays semantic, never platform-worded.
- **Painted**—a future vocabulary entry: a vello paint callback plus a layout size,
  portable across every *interactive* target. Native drivers paint it into a layer; the
  web driver paints it into a `<canvas>` via wgpu (WebGPU; the vello CPU/hybrid path is
  the fallback where WebGPU is absent). SSR/SSG render it empty—the client fills it after
  hydration—and the TUI driver does not realize it. Vello is the one renderer, everywhere.
- **Embeds are driver-specific views**, not vocabulary: a driver crate exposes its own
  constructors (`web::element` and `client_only` for raw DOM today; later
  `ios::native_view`, an Android map view). The payload never crosses a crate boundary,
  so there is no `Any` and no downcast.

  **Why a typed vocabulary and not per-platform realization.** The orphan rule: with
  views in one crate and drivers in others, `impl View for Container` can live in neither
  driver, so separate driver crates are impossible under per-platform realization. Typing
  the vocabulary moves the platform decision (CSS text, native styles) into the driver
  while realization is written once above it.

  **Why not a persistent scene tree in core.** A core-owned retained tree was considered
  and rejected: compile-time by-name dispatch requires the caller to sit above the callee
  in the crate graph, so core could never call driver functions—every workaround was a
  `dyn` driver or a function-pointer table in disguise—and the tree would double the
  bookkeeping every driver already keeps for its sink. The umbrella naming the driver is
  the one place static dispatch works, so that is where realization lives.

  **No mock driver—but headless tests.** Because the vocabulary carries typed data, a
  future host-compilable *recording* driver can capture exactly what any real driver
  receives, giving view logic fast headless tests without pretending to be a platform.
  Drivers test against their real sinks (web: a browser DOM via `wasm-bindgen-test`; ssr:
  wasmtime under a byte-exact snapshot; native: its toolkit).

## The style model

Styling is **data**, not closures, because two requirements demand it: server-rendered and
statically-generated pages must be responsive and animated with **no JS/WASM**, and every
platform speaks the same layout semantics. A closure (`derive_color(move || …)`) is opaque
—a server can only serialize its current value; style data compiles to real CSS.

The typed props carry the full static style (implemented for boxes and text runs):

```
BoxProps/RunProps {
  style:       BoxStyle/RunStyle,             // the fixed base properties
  variants:    Vec<(Vec<Condition>, Style)>,  // all conditions must hold; later wins
  transitions: Vec<Transition>,               // property, duration, delay, easing
  animations:  Vec<Animation<Style>>,         // keyframes over the same style type
}
Condition = MinWidth | MaxWidth | MinHeight | MaxHeight | DarkMode | LightMode
          | ReducedMotion | Hovered | Focused | FocusVisible | Pressed | Disabled
```

- **Layout** will adopt taffy's style model as the vocabulary (re-exported/aliased):
  flexbox, grid, block—defined once by the library that implements it everywhere,
  rendered to CSS text on the web. The framework invents no layout system. (Pending; the
  current `Layout` enum is the seed.)
- **Decoration and text** are skene's typed properties (background, border, radius,
  cursor, text color/size/weight, …), growing as views need them.
- **Passthrough** keeps the vocabulary open: a per-family extension (web: raw CSS
  declarations) for everything the typed core doesn't model. Portable code sticks to the
  core; web-only code can reach all of CSS.
- **Web** renders the whole set into one interned **rule template**: base declarations,
  `@media` blocks, `:hover`/`:focus`/`:active` rules, `transition:`, content-addressed
  `@keyframes`. State- and media-driven styling therefore works in a static document with
  zero script.
- **Native/TUI** resolve `Condition` against driver signals (window size, pointer state,
  focus) and run transitions/animations on the driver's frame ticker—not through the
  reactive graph per frame. `derive_` properties remain for genuinely dynamic values; they
  are the escape hatch, not the mechanism.

### The interner

The content-addressed class interner is load-bearing: a node's complete rule template
(base rule, variants, `@media` blocks, `@keyframes`, with the class as a placeholder) is
hashed to a deterministic name (`skene-{hash:x}`, forward probe on collision), interned
once, reference-counted by live node, collected when unused. Identical styles collapse to
one class; determinism makes the server's stylesheet byte-compatible with the client's,
which hydration relies on. The interner is pure and shared by the web leaves; each layers
its sink on the outcomes (csr: one `<style>` element per class, appended on intern and
removed on collect—one operation either way, however many rules the template holds; ssr:
render live templates once at the end, sorted by class). On native the same interning
shares resolved style objects across nodes—the computed-style sharing browsers get from
classes.

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

- **csr** (`target_os = "unknown"`)—the browser. DOM artifacts, the CSSOM stylesheet
  sink, hydration (creation-order `data-h` markers, loud mismatch), the raw
  `element`/`document` escape hatch; later root-level event delegation and `Painted` via
  `<canvas>` + wgpu.
- **ssr** (`target_os = "wasi"`)—the serializer. Renders its string tree to HTML plus the
  interned stylesheet; drives async to a fixpoint on a WASI-`poll` reactor before
  serializing; streams in document order around `Deferred` boundaries; serializes
  resource state for the client to adopt. The raw handles do not exist here (`web-sys` is
  absent on wasi), painted/embedded content renders empty, and handlers are inert.

The app compiles to wasm for both leaves; the same module serves CSR in the browser and
SSR/SSG in a runner.

### Native (`skene-native`)

One family; the design premise is that **the platform composites**. Where the OS has a
real view hierarchy (iOS, Android, macOS), the driver realizes the tree *into* it: taffy
computes frames, painted content renders through vello into layers that are themselves
views in that hierarchy, and an `Embed` is an ordinary native view (a `UITextField`, a
map). Sibling order is z-order, so custom-painted content and real platform controls
interleave for free—compositing is the platform's duty, not the framework's. The driver
decides its own realization granularity internally (view-per-node versus flattened painted
layers); the invariant is only that tree order maps to compositor order.

The Linux desktop leaf has no native control set to embed, so it is the plain
winit + single-vello-surface case; Windows sits between (composition visuals) and its leaf
chooses. Text is parley; accessibility is AccessKit; per-OS glue (IME, clipboard, menus)
lives in each leaf. Off-thread work uses the reactive engine's waker/drain pair wired to
the event-loop proxy, and a loop-integrated timer is fair game here (the loop is ours).

### TUI (`skene-tui`)

Layout, no painting: taffy with `1.0 = 1 cell`, rendered to a terminal buffer. `Painted`
and embeds have no realization. TUI is deliberately the cheapest driver—it doubles as
the proof that the vocabulary is platform-neutral.

## Selection and entry

With one driver today, selection is by target: the umbrella pulls `skene-web` under
`cfg(target_arch = "wasm32")` and no feature is needed. When a second same-triple driver
lands (desktop and TUI share a target triple), selection becomes **exactly one driver
feature** (`web`, `desktop`, `tui`, …), with more than one a `compile_error!`.
Target-gating stays the enforcement layer where it applies (`target_os` splits csr/ssr;
`web-sys` cannot exist off the browser target).

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
  it forced (each leaf re-derived the same realization decisions). The typed vocabulary
  replaces it: the decision moves to data, the mapping to the driver.
- **"No framework style model"** existed so no platform would be forced onto a foreign
  vocabulary. Taffy-everywhere plus no-JS responsive SSG *is* a shared vocabulary by
  requirement; the model is now framework-defined but open (passthrough), and each driver
  still owns its sink.
- **"Selection is the target, only"** cannot distinguish desktop from TUI on one triple.
  Target-gating is demoted from selector to enforcement.
- **"No generic platform to test against"** was true when the realization interface was
  per-platform text. The typed vocabulary is real data; a recording driver can capture it
  faithfully for headless view tests.
- **A `Backend` trait object** (pre-redesign) cost `unsafe`, `Box::leak`, and a fallible
  downcast to re-discover what the compiler knew. Still rejected: drivers are statically
  selected, and the generic driver-ops parameter monomorphizes—there is no `dyn` driver
  anywhere.
