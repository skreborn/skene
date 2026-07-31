# Roadmap

The phased migration to the driver-vocabulary architecture (`DESIGN.md`), then the platform build-out.
Phases are ordered; later phases assume the earlier ones. **Current phase: 3.**

## Phase 1 — Reactive engine internals ✅

API kept as-is (`RwSignal`/`Memo`/`Effect`/`Scope`/`Context`, sync/local storage split).

- **1a, correctness — done.** Eager staleness marking (a write marks dependent memos
  stale transitively; reads pull freshness — fixing the stale memo-read inside
  `Effect::batch`), memo change-filtering preserved, non-reentrant flushing (a mid-flush
  write enqueues; the running flush drains it), sync-runtime drains mark before running.
- **1b, performance — done.** The five `HashMap<Id, _>` runtime maps collapsed into one
  `FxHashMap<Id, Entity>`; all hashing on rustc-hash; dispose unlinks from the parent's
  children set (fixed unbounded growth under child churn). Criterion benches: creation
  −40%, reads −44%, write-through-effects −30%, memo chains −24%.
- **Remaining:** a true generational arena (needs an `Id` allocation redesign across the
  local and sync runtimes); implicit batch around event dispatch (lands with phase 4).

## Phase 2 — The typed vocabulary, the crate split, realize-once views ✅

The structural core of the redesign; see `DESIGN.md` for the full shape (refined during
implementation: a typed driver vocabulary rather than a persistent core scene — static
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

## Phase 3 — Declarative style: conditions, transitions, animations

- `StyleSet` grows `variants: Vec<(Condition, Props)>` with
  `Condition = Media(width/height ranges, color scheme, reduced motion) | State(hover,
  focus, focus-visible, active, disabled)`, plus `transitions` and `animations`
  (keyframes).
- Layout props adopt taffy's style types as the vocabulary.
- Web renders the whole set into the interned class: `@media` blocks, pseudo-class rules,
  `transition:`, `@keyframes`. **Gate:** an SSG page is responsive, hoverable and animated
  with zero JS.
- Streaming style deltas: a `Deferred` chunk whose content interns a brand-new class
  carries that rule in its chunk.
- Hydration adopts the server's `<style>` sheet instead of rebuilding a duplicate.

## Phase 4 — Events and first controls

- Portable event vocabulary: pointer (down/up/move/enter/leave, click stays), keyboard,
  focus/blur, input/change, scroll. Root-level delegation on the web (one listener per
  event type, dispatch by node id) replaces the per-node closures.
- Implicit reactive batch around driver event dispatch.
- First form controls (web: native `<input>`/`<textarea>` realizations), `Role` expanded.
- Focus management primitives.

## Phase 5 — Router, runner, SSG

- `skene-router`: a typed route as a signal; web csr binds the History API, ssr takes the
  route as render input, native/tui keep it in memory. Depends on `skene-reactive` only.
- `skene-runner`: a host-side library embedding wasmtime around the app's `wasm32-wasip2`
  module—`render(route) -> Rendered`/`Stream` for any server, plus a dev server. The SSR
  reactor's `register` seam goes public so third-party clients (`wasi:http`) block the
  render like the built-in `sleep` does.
- SSG: enumerate the route table, render each route through the runner, write files.
- The build tool emits the CSR page from the same shell contract SSR uses, so the two
  cannot drift.

## Phase 6 — Native driver family

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

## Phase 7 — TUI driver

- taffy with `1.0 = 1 cell`; buffer renderer; no Painted/Embed realization.
- Deliberately minimal—its role is to prove the vocabulary is platform-neutral.

## Phase 8 — CLI

Explicitly last, after the stretch goals above.

- One tool to build/run/serve per platform: wasm bundling + the generated CSR page, SSG
  output, the dev server (subsuming `cargo xtask web`), native builds.
