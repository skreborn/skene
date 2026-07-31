# Roadmap

The phased migration to the scene architecture (`DESIGN.md`), then the platform build-out.
Phases are ordered; later phases assume the earlier ones. **Current phase: 1.**

## Phase 1 — Reactive engine internals

API stays as-is (`RwSignal`/`Memo`/`Effect`/`Scope`/`Context`, sync/local storage split).

- **1a, correctness:**
  - Eager staleness marking: a write marks dependent memos stale transitively at write
    time; reads pull freshness. Fixes the proven bug where a memo read inside
    `Effect::batch` after a write returned the stale value. Memos keep change-filtering
    (an unchanged recompute notifies no one).
  - Non-reentrant flushing: a write during a flush enqueues instead of recursing; the
    running flush drains it. Sync-runtime drains mark before running.
- **1b, performance:** replace the five `HashMap<Id, _>` runtime maps with arena storage
  and small-vector subscription edges with back-pointers. Measure before/after.
- Later, with drivers: implicit batch around event dispatch.

## Phase 2 — The scene, the crate split, the web driver retargeted

The structural core of the redesign; see `DESIGN.md` for the full shape.

- The scene in `skene-core`: node kinds (Box, Text, Group, Painted, Embed), properties as
  `Source`s, persistent structural tree, generic driver-ops algorithms (`reconcile`,
  `dynamic`, destroy walks) hoisted from the facade.
- Built-in views realized once against the scene, in `skene-views`.
- `skene-web` split out as a driver crate; csr/ssr leaves keep the `target_os` split. The
  ssr serializer walks the scene (its private string tree dissolves into it); csr keeps
  only its NodeId→Element registry and sinks.
- The umbrella `skene`: driver selection by feature (`compile_error!` on zero or two),
  prelude, `launch`, `#[main]`, `spawn`, `Resource`, `Deferred`, `client_only`.
- Headless scene tests for the built-in views; the browser/wasmtime suites keep covering
  the drivers.
- **Gate:** every existing web/ssr/hydration/resource/streaming test passes; interned
  class names and the hydration marker scheme are byte-compatible before/after.

Carried over, already done in the pre-scene code and preserved through the migration:
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

The largest phase by far; do not start before phase 4 proves the event/scene model.

- Shared machinery: taffy layout, vello paint, parley text, AccessKit, the frame ticker
  running style transitions/animations, `Condition` resolution against window/pointer
  signals.
- Compositing premise: **the platform composites.** On iOS/Android/macOS the driver
  realizes the scene into the native view hierarchy—painted content in vello-backed
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
- Deliberately minimal—its role is to prove the scene is platform-neutral.

## Phase 8 — CLI

Explicitly last, after the stretch goals above.

- One tool to build/run/serve per platform: wasm bundling + the generated CSR page, SSG
  output, the dev server (subsuming `cargo xtask web`), native builds.
