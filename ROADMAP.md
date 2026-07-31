# Roadmap

The phased migration to the driver-vocabulary architecture (`DESIGN.md`), then the platform build-out.
Phases are ordered; later phases assume the earlier ones. **Current phase: 6 (TUI done first as the vocabulary proof).**

## Phase 1—Reactive engine internals ✅

API kept as-is (`RwSignal`/`Memo`/`Effect`/`Scope`/`Context`, sync/local storage split).

- **1a, correctness—done.** Eager staleness marking (a write marks dependent memos
  stale transitively; reads pull freshness—fixing the stale memo-read inside
  `Effect::batch`), memo change-filtering preserved, non-reentrant flushing (a mid-flush
  write enqueues; the running flush drains it), sync-runtime drains mark before running.
- **1b, performance—done.** The five `HashMap<Id, _>` runtime maps collapsed into one
  `FxHashMap<Id, Entity>`; all hashing on rustc-hash; dispose unlinks from the parent's
  children set (fixed unbounded growth under child churn). Criterion benches: creation
  −40%, reads −44%, write-through-effects −30%, memo chains −24%.
- **Remaining:** a true generational arena (needs an `Id` allocation redesign across the
  local and sync runtimes); implicit batch around event dispatch (lands with phase 4).

## Phase 2—The typed vocabulary, the crate split, realize-once views ✅

The structural core of the redesign; see `DESIGN.md` for the full shape (refined during
implementation: a typed driver vocabulary rather than a persistent core scene—static
dispatch requires the realization to live where the driver is nameable).

- **Done.** The typed property vocabulary in `skene-core` (`BoxProps`/`BoxProp`,
  `RunProps`/`RunProp`); `Border`/`Layout`/`Role` and `Boundary` hoisted into core;
  `skene-web` split out as a driver crate (csr/ssr leaves keep the `target_os` split,
  interner/Declarations/hydration channel inside); views realized once in the umbrella
  `skene` against `driver::*`; `skene-gui` dissolved (crate renamed `skene`,
  `skene-gui-macros` → `skene-macros`, entry `__skene_main`).
- **Gate met:** the full web/ssr/hydration/resource/streaming suites pass, and a
  byte-exact SSR snapshot (interned class names, `data-h` markers, markup) is identical
  before and after the migration.
- **Remaining:** a host-compilable recording driver for headless view tests; driver
  selection by feature when a second same-triple driver lands.

Carried over from the pre-split code and preserved through the migration:
`render_to_string` and `Rendered::page`; in-order streaming (`render_to_stream`,
`Deferred`, resumable cursor); the WASI-`poll` reactor and `stabilize` fixpoint;
marker-based hydration with loud mismatch; serialize-by-default resources
(`Resource::client` opt-out); `Span`/`RichText`; scope-bound `spawn`.

## Phase 3—Declarative style: conditions, transitions, animations ✅ (core)

- **Done.** `BoxProps`/`RunProps` carry `variants: Vec<(Vec<Condition>, Style)>`
  (conjunction; media features and interaction states), `transitions`, and
  `animations` (keyframes over the same style type). Builders: `set_when`,
  `set_when_all`, `set_transition`, `set_animation` on `Container`/`Text`/`Span`;
  `BoxStyle`/`RunStyle` gained chainable const setters. The web driver renders the whole
  set into one interned **rule template** (`@media` blocks, pseudo-class rules,
  `transition:`, content-addressed `@keyframes`); the interner keys on templates; the
  browser sink is one `<style>` element per class. **Gate met:** SSR output is
  responsive, hoverable and animated with zero JS (`web_style.rs`), and the snapshot
  pins it.
- **Remaining:**
  - Layout props adopt taffy's style types as the vocabulary (with the wider flex/grid
    property set).
  - Streaming style deltas: a `Deferred` chunk whose content interns a brand-new class
    carries that rule in its chunk.
  - Hydration adopts the server's `<style>` rules instead of rebuilding duplicates.
  - Variants for the raw-`Declarations` escape hatch (currently base-only).

## Phase 4—Events and first controls ✅ (core)

- **Done.** Portable event vocabulary: click, pointer enter/leave, focus/blur, key down
  (`KeyEvent`: W3C key names + modifiers), input. On the web the bubbling events are
  **delegated**—one listener per event type on the mount root, dispatch walks the
  target's ancestors by a stamped node id—and the non-bubbling pointer enter/leave
  attach per node. Every dispatch runs inside an implicit reactive batch. First control:
  `Input` (web `<input>`, a void element in SSR output) with
  `bind_value`/`derive_value`/`handle_input`/`handle_key_down`, placeholder, and the full
  declarative styling surface (`set_style`, `set_when`, transitions, animations).
- **Remaining:** pointer down/up/move, scroll, change (committed value), `<textarea>`,
  focus management primitives (programmatic focus, tab order), `Role` expansion.

## Phase 5—Router, runner, SSG ✅ (core)

- **Done.** `skene-router`: a typed `Route` (parse/to_path) in a signal-backed `Router`
  with a platform sink seam, provided through the reactive `Context`; the umbrella's
  `install_router` binds it to the browser History API (pushState + popstate) or, on the
  server leaf, to the requested path. `render_to_string_at(path, app)` renders a route;
  `#[main]` now also expands to the server entry, making the app's `wasm32-wasip2` build
  a plain command (`wasmtime app.wasm /path` prints the page). `skene-runner` embeds
  wasmtime around exactly that contract (`Runner::load` + `render(path)`), host-only, for
  any server to call. `cargo xtask ssg --path / --path /about` builds the component and
  writes `<out>/<route>/index.html` through the runner.
- **Remaining:** route enumeration for SSG (`StaticRoutes::all` surfaced through the
  artifact); streaming through the runner (`render_to_stream` over incremental stdout);
  the CSR page emitted from the SSR shell contract (build tool); typed link helpers
  (an `<a>` that navigates without a reload).

## Phase 6—Native driver family

The largest phase by far; do not start before phase 4 proves the event model.

- Shared machinery: taffy layout, vello paint, parley text, AccessKit, the frame ticker
  running style transitions/animations, `Condition` resolution against window/pointer
  signals.
- Compositing premise: **the platform composites.** On iOS/Android/macOS the driver
  realizes the tree into the native view hierarchy—painted content in vello-backed
  layers that are views, embeds as ordinary native views—so custom paint and real
  platform controls interleave in z-order for free. The Linux leaf is
  winit + single vello surface (no native control set exists to embed); Windows chooses
  per its composition APIs.
- `Embed` constructors per leaf (`ios::native_view`, …): real text fields, maps, web
  views.
- Off-thread delivery: wire `set_sync_effect_waker` + `drain_pending_work` to the event
  loop proxy—the machinery exists and is tested; only the wiring is missing.
- A loop-integrated timer (the one sanctioned exception to timers-are-userland).
- Per-OS glue in leaves: IME, clipboard, menus, window management.

## Phase 7—TUI facade ✅ (core)

- **Done, twice-shaped.** First landed as a driver of the shared GUI vocabulary; then
  reshaped into a **first-class, equal facade**: `skene-tui` owns its vocabulary
  outright—`u16` cells, `bool` bold, one-cell colored borders, terminal conditions
  (`MinWidth`/`MaxWidth`/`MinHeight`/`MaxHeight` in cells, `Focused`)—so nothing a
  terminal cannot render exists to call. Its `Container`/`Text`/`Input` builders and
  realization live in-crate; layout is taffy at `1.0 = 1 cell`; frames paint via
  crossterm (truecolor, bold, reverse, `Tab`/`Enter` focus, input editing, `Ctrl-C`) or
  headlessly through `render_to_text`, with `trigger_key`/`focused_node` test seams.
- Alongside it: the umbrella returned to its `skene-gui` name (GUI is not the blessed
  way), its `tui` feature is gone (selection is by facade dependency), and the
  structural machinery (`Children`/`reconcile`/`dynamic`/`DynamicView`/`empty`) hoisted
  into core as zero-cost `TreeOps` generics both facades alias. The reactive UI-thread
  designation became resettable under the `test` feature for multi-threaded host
  harnesses.
- **Remaining:** text wrapping; mouse events; scroll; cursor positioning in inputs
  (edits append/pop only); a diffed redraw instead of full frames.

## Phase 8—CLI

Explicitly last, after the stretch goals above.

- One tool to build/run/serve per platform: wasm bundling + the generated CSR page, SSG
  output, the dev server (subsuming `cargo xtask web`), native builds.
