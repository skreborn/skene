# Roadmap

Known future work. Not part of the delivered redesign; no commitment to order.

## Platforms

- **Native platforms** (iOS, Android, desktop) — each a `platform/<name>` module wrapping
  `taffy` for CSS-style layout plus native views, with its own `tracing` subscriber
  (android → logcat, ios → `os_log`, desktop → stderr). Adding one means a new module plus
  its `tree` functions; nothing shared has to change.
- **TUI** — the `skene-tui` branch: a parallel facade with its own vocabulary (integer
  cells, no fractional insets / font size), reusing only the spine and universal values. It
  gets `Reorder`, `IntoChildren` and the view spine for free, and writes its own `Children`
  and reconcile driver.
- **Server-side rendering for the web platform.** Render a view tree to HTML plus a
  stylesheet off the browser, hydrating on the client. **Rendering is done** (`skene_gui::
  render_to_string`): the `ssr` leaf backs the shared vocabulary with a string tree and a
  serialize-to-text `StyleStore` (content-addressed, so class names match CSR), builds under a
  disposable per-render scope, drives resources to a fixpoint, and serializes a `<style>` block
  plus the settled tree. Escaping, class dedup, and the resolved-resource path are tested under
  `wasmtime` on `wasm32-wasip2`. **Hydration is done** too — structure (`web::hydrate` adopts the
  server DOM by per-node markers, catching divergence) and resource state (a resource's value
  serializes by default and the client adopts it without re-loading). Async loads settle too — a
  WASI-`poll` reactor blocks the render until they resolve. What remains is narrower: wiring a
  concrete HTTP client to the reactor, and stylesheet reuse — see below.

  **The app still compiles to wasm, not native.** SSR runs the *same* wasm module inside a
  runner — a host, bound to its own platform, embedding a wasm runtime — which supplies the
  host imports the module needs (the app's HTTP library among them, e.g. bound to `wasi:http`).
  So the app is written once and compiled once per wasm target; the server does not get a
  native build of it.

  **Running until it stabilizes.** SSR must serialize the *settled* tree, not the loading
  placeholder, so the runner has to drive the app to a fixpoint first:
  - **`spawn_local` does not exist off a JS host — done.** It rides the browser's microtask
    queue, which a wasmtime/WASI runner has none of, so the `ssr` leaf drives futures itself: a
    `std::task::Wake`-based single-threaded executor fills the `drive` seam (`spawn`'s
    scope-bound wrapper is unchanged), and `stabilize` polls every ready task to quiescence.
    (If the runner is a JS runtime on `wasm32-unknown-unknown`, `spawn_local` works and the
    runner pumps its own queue instead — another thing the target choice below decides.)
  - **SSR async runner — done** (`ssr/task.rs`, a WASI-`poll` reactor). A future that parks on a
    host operation registers its `Pollable` with the reactor via `register`; when `stabilize`
    finds nothing synchronously ready but the reactor has outstanding waiters, it blocks the
    runner on `wasi:io/poll.poll` until one is ready, wakes that task, and re-polls — so a
    genuinely-async load resolves and the render settles on its *loaded* tree. `sleep(Duration)`,
    the runner's one timing primitive, is built on `wasi:clocks` and both drives and tests it.
    Verified under `wasmtime`: a resource that `await`s a `sleep` serializes `Ada`, not the
    placeholder. It is **not** the CSR executor-ownership question (Tasks → "Owning an executor"),
    which is a separate concern about frame scheduling and worker offload on the client.
    - **Remaining: real HTTP.** The reactor awaits anything that registers a `Pollable` with it —
      our `sleep` does. A user's HTTP library must register its `wasi:http` pollables the same
      way (the `register` seam) to be awaited; wiring a concrete client to it is the open piece.
  - **Blocking is automatic for reactor-backed loads.** Because `stabilize` blocks while the
    reactor has any outstanding waiter, every load that registers a pollable *does* block the
    render (the "load everything" default, as Dioxus's `StreamingMode::Disabled` does). A
    *non-blocking* load that ships something and streams the rest later is the `Deferred` boundary
    (below). We have no per-resource "critical/blocking" flag, and Dioxus doesn't either — it, like
    us, streams per boundary (`SuspenseBoundary`), not per resource.
  - **Streaming, in-order — done** (`Deferred`, `render_to_stream` → `Stream`). Wrap slow content
    in `Deferred::new(|| …)`: the render flushes everything up to that boundary, blocks until the
    boundary's async work settles, then emits its content **in place** and continues — so
    `render_to_stream` yields an ordered sequence of HTML chunks a server writes as they are
    produced. It is a *boundary*, not a resource flag: async work is counted at `spawn`, against
    the nearest enclosing boundary threaded through the reactive `Context` (the whole render is the
    root/shell boundary), so *any* async inside a `Deferred` defers, not only a `Resource`. The
    counting is a `Boundary` (an `RwSignal<usize>` of pending tasks, zero = settled); the streaming
    walk is resumable (an explicit cursor over the arena) so it can pause before an unsettled
    boundary with ancestors left open — exactly what a progressively-streamed document expects —
    then resume in document order. This is the **no-JavaScript** path: the chunks concatenate, in
    order, into the same markup `render_to_string` would produce; nothing on the client is required
    to place the deferred content. Tested under `wasmtime`: a page whose shell is ready but whose
    `Deferred` awaits a `sleep` flushes the shell first (no placeholder, no loaded value), then
    streams the settled content in a later chunk, shell-before-content. On the client a `Deferred`
    is just a grouping node that builds its content directly (tested in the browser); hydration
    adopts the server's settled markup.
    - **Remaining: out-of-order streaming** (a JS shim: ship the shell with a fallback, stream each
      boundary as it settles, swap it in on arrival) — faster first paint but *requires*
      JavaScript, so it is the complement to in-order, not a replacement. And **style deltas**: the
      streamed head carries the shell's styles, so a `Deferred` whose *loaded* branch interns a
      brand-new class (one absent at build time) needs that rule streamed in its chunk — the walk
      already segments the body, the style diff is the missing piece.
  - **Stabilising is a fixpoint, not one await — done.** A landed resource re-runs effects, which
    may build a view that starts *another* async load — a waterfall. `stabilize` handles it: poll
    every ready task, and when none are ready block the reactor on the outstanding pollables,
    repeat until nothing is ready and nothing is outstanding. The reactor's waiter list *is* the
    outstanding-load set, so no separate count is needed; the round bound (`MAX_ROUNDS`) still
    caps a self-waking task so it cannot hang the render.
  - **"Settled" already means "finished, not succeeded."** Because failure lives in the loaded
    type (`Resource<Result<T, E>>`), `Some(Err(..))` is settled like any other value. Nothing
    extra to design — the `Option<T>` model gives this for free.
  - **Hydration, structure — done, marker-based** (`skene_gui::web::hydrate`). The server writes
    a `data-h="{id}"` marker (the node's creation-order id) on every element; the client indexes
    those markers and, as it builds, adopts the server element whose id matches the node it is
    creating — taking over the existing DOM rather than rebuilding it, with `attach` skipping the
    insert for an adopted node. Event listeners and reactive bindings wire onto the adopted nodes
    — no rebuild, no flash. A lookup that misses, or finds a different tag, is a **mismatch**:
    it is logged and a fresh node is created in the divergent one's place, so a build/DOM
    divergence surfaces loudly instead of silently mis-adopting (the gap Dioxus closes with the
    same discovery-order markers). Tested in the browser: adoption by element identity, a hydrated
    click firing on the adopted node, a serde value adopted without its loader running, and a
    deliberate structural mismatch caught rather than mis-adopted.
    - **Deferred:** stylesheet reuse (the client rebuilds its rules into a fresh sheet — benign
      duplication with the server's `<style>`; later it should adopt the server sheet — now
      unblocked, since page assembly gives the styles a defined place in `<head>`). Unlike Dioxus
      we mark *every* node, not only dynamic ones, since our imperative build has no
      static-template concept to skip — a modest HTML-size cost for robust matching.
  - **Page assembly — done** (`Rendered::page`, `skene_gui::hydrate_page`). `render_to_string`
    returns the parts split — `styles()`, `body()`, `state()` — and `page()` assembles a complete
    `<!DOCTYPE html>` document: styles in `<head>`, the tree in `<body>`, and the state in a
    `<script id="skene-state">` (a `<` in the state is JSON-escaped so a value can't close the
    script). The
    client boot `hydrate_page` is the counterpart to `launch`: it reads that script and adopts
    `<body>`, so a server-rendered app takes over without a rebuild. Tested: `page()` produces a
    well-formed document with the embedded state; a runner can assemble custom pages from the
    parts instead.
  - **Hydration, resource state — done, serialize-by-default** (`Resource::new` / `Resource::client`).
    A resource's settled value is serialized into the render (`render_to_string` returns the
    `Rendered` parts); the client, given that state, adopts each value instead of re-loading. This
    is the **default**: `new` serializes (bounded `Serialize + DeserializeOwned`), which avoids the
    hazard where a server-settled resource left unserialized makes the client build a different
    tree and mis-hydrate. `Resource::client` is the opt-out — a client-only load (a browser API, an
    unserializable value) that takes no part in hydration, does not shift the creation-order
    matching, and **does not run during the server render** (guarded by an `is_server` seam, so a
    synchronous client load can't settle server-side and mismatch). Serialization is serde
    (`serde_json`); state is a JSON array of per-resource blobs, `null` for one that did not
    settle. The channel is a platform seam (`adopt_next`/`register_source`/`is_server`), so
    `resource.rs` stays platform-agnostic. Resources are **retryable**: the loader is `Fn` and
    retained, so `refetch` clears the value and re-runs it. Tested: the server serializes two
    resources by creation order, a client resource is absent from the state (and its loader never
    runs on the server), the client adopts a value without loading, and `refetch` re-runs.
    - **Async loads settle — done.** A resource backed by an async load that registers a pollable
      with the **SSR async runner** (see "Running until it stabilizes" above) now blocks the
      render until it resolves, rather than shipping its placeholder. A concrete HTTP client still
      needs wiring to the reactor's `register` seam; a non-blocking/streaming flavour is a later
      refinement. (We have no per-resource "critical/blocking" flag; Dioxus doesn't either — it
      blocks per `SuspenseBoundary`.)

  **Decided: SSR is its own platform, not a DOM shim.** Running the CSR build in a runner
  with a simulated DOM cannot be made correct, because the web escape hatch
  (`web::element(node) -> web_sys::Element`) lets a view do *anything* — so a shim would
  have to reimplement all of web-sys faithfully, and any divergence is a *silent* CSR/SSR
  mismatch. That is runtime hope standing in for compile-time knowledge, the same thing the
  platform model exists to avoid. Rejected. A runtime flag is the same regression and is
  rejected too.

  Instead, model a **web family**: CSR and SSR are two platforms that emit the *same* web
  semantics (`Container` → `<div>` with `display:flex`) to different sinks (a live DOM vs. an
  HTML string). Shape (the seam and the `ssr` leaf are **done** — see below; hydration remains):
  - **A cfg-selected web vocabulary** ✅ — `platform::web::{create_element, set_attribute,
    set_style, override_style, set_text, handle_click}`, re-exported from the active web leaf
    (the structural `platform::tree`/`drive` sit one level up, shared by every GUI platform).
    CSR backs it with the DOM; SSR will back it with a string tree.
  - **The built-in views live in a shared `platform/web/views/`** ✅, in the web *family*
    module (`platform/web/`, gated `target_arch = "wasm32"`, which covers both CSR-`unknown`
    and SSR-`wasi`), written once against the vocabulary. `Styler` and
    `Declarations`/`render_color` (pure CSS text) sit beside them. Every built-in will SSR for
    free. `handle_click` becomes a no-op on the SSR leaf — server HTML is inert; interactivity
    returns at hydration.
  - **The escape hatch is the compile-time seam** ✅. The vocabulary is portable across both
    web platforms; the raw `web::element(node) -> Element` handle is **CSR-only** and is
    deliberately not in `platform`'s re-exports, so shared code cannot reach for it. Code that
    keeps to the vocabulary works on both; code that reaches for a raw `Element` (canvas, JS
    interop) will not compile for SSR and wraps in `client_only(…)` ✅ (→ `empty()` on the
    server; the client fills it post-hydration). SSR output is well-defined by construction,
    not by shim fidelity.
  - **The `ssr` leaf** ✅ — under `platform/web/` (gated `target_os = "wasi"`), it implements the
    vocabulary against a string tree plus a serialize-to-text `StyleStore`, with the
    `target_os = "wasi"` arm of `client_only` (→ `empty()`) and its own executor. `handle_click`
    is inert, `element`/`document` are absent so raw-DOM code fails to compile. **Remaining:
    hydration** (below), plus the critical-resource opt-in noted above.
  - **Selection should be the target, not a feature** — so an app author declares nothing.
    Since SSR is now a *separate* realization (not a DOM shim), the only reason to keep it on
    the browser's target is gone. Run the SSR build under a WASI runtime → target
    `wasm32-wasip2` (`target_os = "wasi"`) vs. the browser's `wasm32-unknown-unknown`
    (`target_os = "unknown"`), and plain target-gating separates them: no feature, no flag,
    no author config, `--target wasm32-wasip2` to build the server. It is also the honest
    description of the environment and matches "the runner provides the imports" (the WASI
    component model, `wasi:http` etc.), and `web-sys` physically does not exist there, so the
    SSR platform *cannot* touch the DOM — the boundary is enforced by the target, not policed
    by a flag.
    - Fallback, if the runner must be a JS host on `wasm32-unknown-unknown`: a selector is
      then unavoidable, but keep it off the app's manifest — a skene build tool sets
      `RUSTFLAGS="--cfg skene_ssr --check-cfg=cfg(skene_ssr)"` for the server build. Cost: a
      `RUSTFLAGS` change busts the build cache, so CSR/SSR share no artifacts. Strictly worse
      than the WASI target, which pays nothing.
    - Either way most app code needs no branch (built-ins SSR via the shared vocabulary);
      raw-`Element` interop is wrapped in a framework helper (`client_only(…)` → `empty()` on
      the server), so the author never writes the cfg by hand.
  - **Sharing follows semantics, not platform count.** CSR + SSR share `views/` because both
    emit web semantics; a native (`taffy`) platform emits *different* semantics and keeps its
    own view realization. The module layout must reflect "web family" vs. "native", not one
    `views/` for everyone.
  - **The framework generates the CSR page too — do this right after streaming.** SSR assembles
    its own document (`Rendered::page`); CSR should not force the app author to hand-write an
    `index.html` that boots the wasm and can *drift* from the SSR shell. The build tool should
    emit the CSR page from the same shell contract SSR uses (same `<head>`, same root, same wasm
    boot), so the two can never diverge. A build-tool concern (`xtask`), not runtime.

## Tasks & off-thread work

Nothing today can run work off the UI thread, on a timer, or await a result. Every item
below solves one problem: get a value produced *elsewhere* back onto the UI thread and into
a signal. The pieces that exist: `SyncRwSignal` (`Send + Sync` storage on its own
`SYNC_RUNTIME`) already reads and writes across threads, and `DynamicView` already swaps a
loading tree for a loaded one. The pieces that don't: everything that gets a value from a
thread or a future to a signal, and everything that runs the resulting effects.

- **Delivery already works** (`packages/reactive/tests/sync_effect.rs`). An off-thread
  `SyncRwSignal` write queues its subscribed effect on `SYNC_RUNTIME`, does *not* run it on
  the writer, and `Runtime::drain_pending_work()` runs it on the UI thread with the new
  value. `Runtime::set_sync_effect_waker` exists to nudge the UI loop when that queueing
  happens. So the mechanism is designed and now proven; what is missing is the *integration*:
  **nothing installs a waker and nothing drains.** A platform must install a waker (web → a
  microtask/`setTimeout` that calls `drain_pending_work`) and own the drain, or a delivered
  value sits in the queue forever.
- **Hazard, silent-loss without `init_on_ui_thread` — now loud.** Without a designated UI thread,
  `Runtime::is_ui_thread()` returns `true`, so a background `Sync`-signal write believes it *is*
  the UI thread, posts its effects into the writer thread's own runtime, and they vanish — no
  panic, no log. `run_effects` now warns (once) when a `Sync` signal *with subscribed effects*
  runs while no UI thread was designated, pointing at `init_on_ui_thread`. Narrow on purpose:
  `SyncStorage` is also used internally (`SyncDerivedRwSignal`) and a subscriber-less cross-thread
  `Sync` signal loses nothing, so the warning gates on both `Sync` and non-empty subscribers, and
  is guidance rather than a panic (single-threaded `Sync` use still works). `web::install` calls
  `init_on_ui_thread`, so a web app never trips it. **Still to do:** actually *routing* an
  off-thread write correctly is the separate off-thread-delivery work (the `SyncRwSignal` waker +
  `drain_pending_work`), which lands with a native platform.
- **Spawning — done, and reshaped into a seam.** `skene_gui::spawn` is now
  platform-independent (`src/task.rs`): it wraps the future for scope-bound cancellation
  (`Scope::on_cleanup`, a flag checked *before* each poll so a parked task stops rather than
  resuming into a torn-down tree) and hands it to the leaf's one executor primitive,
  `platform::drive` (web → `spawn_local`). A leaf implements only `drive`; the cancellation
  logic is written once and shared.
  - **Still to do: the same for a native platform**, where the primitive is real threads
    rather than an event loop — but it only supplies `drive`, and inherits `spawn`. Note the
    signature that fell out here — `Future<Output = ()> + 'static`, deliberately **not**
    `Send`, because the browser resumes on the UI thread. A native platform can honour that
    with a UI-thread executor; genuinely off-thread work needs `Send` and so wants a
    *separate* entry point, not a widened `spawn`.
- **The marshalling machinery is for native, not the web.** The browser build is
  single-threaded (no atomics/shared memory), so `is_ui_thread()` is always true there, an
  awaited `spawn_local` future resumes on the UI thread, and a plain `RwSignal` write runs
  its effects immediately — nothing ever queues. `SyncRwSignal`, the waker and
  `drain_pending_work` only start earning their keep when a platform has real threads.
  Installing a waker on the web today would be dead code.
- **Both sync and async, deliberately.** The networking ecosystem is async, so async has to
  be first-class; but we can't force it — or an executor — on anyone, so a plain blocking
  closure must work too. The asymmetry to resolve rather than stumble into: blocking-off-
  thread is natural on native and near-impossible on the web (no threads; only a Web Worker,
  which cannot share memory). The sync flavour may end up native-only.
- **No forced executor.** The framework must not depend on tokio. Either each platform
  supplies a minimal one (web → `spawn_local` is nearly free) or the app injects one.
- **Awaiting initial data — done** (`Resource`). Load starts on creation, scope-bound; reads
  track, so a reading view re-runs when the value lands; pair with `DynamicView` to swap a
  placeholder for the loaded tree. Failure lives in the loaded type (`Resource<Result<..>>`),
  so `Some(Err)` is settled like any value. The load is an arbitrary future — the user's HTTP
  library goes straight in: `Resource::new(|| async { reqwest::get(url).await })`. (Hydration
  for SSR still open — see the SSR item.)
- **Timers and HTTP are userland — deleted, deliberately.** Both were briefly shipped
  (`skene_gui::sleep`/`timeout`/`interval`, `skene_gui::http`) and removed. Nothing about a
  socket or a timer is reactive: users bring their own HTTP (`reqwest`, `gloo-net`, `ehttp`)
  and their own timing (`tokio::time`, `gloo-timers`, `futures`), matched to the platform they
  build for. The framework ships only the reactive glue no external crate can provide — resume
  on the UI thread, scope-bound cancellation, write back into a signal — which is exactly
  `spawn` + `Resource`. Owning an HTTP stack would mean owing users streaming, multipart,
  cookies, redirects, HTTP/2 and a TLS decision forever; owning a timer stack duplicates the
  ecosystem's combinators. Both are pure tech debt and lock-in, so we ship neither.
  - **Exception to revisit once we own a native event loop: a loop-integrated timer.** On the
    web a timer is just `setTimeout`/`gloo-timers` — genuinely userland, since the browser owns
    the loop. But a framework that owns its own native loop can offer a `sleep`/`exec_after` as
    a cheap *scheduling primitive on that loop* (winit `ControlFlow::WaitUntil`), which is
    closer to `drive` than to an HTTP library — Floem (`exec_after`/`debounce_action`), Slint
    (`Timer`), and Yew (`platform::time`) all do this. So a native-only, loop-backed timer is
    fair game when the native platform lands; the blanket "timers are userland" holds only
    while the browser owns the loop. HTTP has no such exception — it stays userland everywhere.
- **Owning an executor — deliberately not yet.** The `drive` seam is executor-agnostic on
  purpose. Today it *delegates*: `spawn_local` on the web, a `tokio` `LocalSet` on an eventual
  native platform. We are not forced onto `spawn_local` even on the web — the browser is the
  *reactor* (fetch/timer completions arrive as host callbacks, unavoidably), but the
  *executor* (the task queue, poll ordering, integration with the reactive flush) is ours to
  write if we want it, driven by a microtask / `MessageChannel` / `requestAnimationFrame`.
  Owning one would buy frame-aligned DOM writes and batched effect flushing, and it
  generalizes to multi-threaded WASM (SharedArrayBuffer + workers), whose shape — `Send`
  compute on a pool, `!Send` UI tasks on the main thread — is identical to native's. But that
  is exactly the specialized surface to avoid until a concrete need (frame scheduling, worker
  offload) forces it. The seam is shaped so swapping the fill later touches neither `spawn`,
  `Resource`, nor any call site. This is the *client* executor; the server's `wasi:http`-driven
  **SSR async runner** (Platforms → SSR) is a separate item, not this one.

## Views

- **Inline text — done** (`Span` / `RichText`). A `Span` is an inline run of text, a nested
  group styled as a unit, or an arbitrary embedded view (inline-level box); `RichText` flows a
  sequence of spans as one wrapping line. Ported from the pre-redesign design against the current
  text model: per-run style overrides, links, a group whose resolved style cascades to its
  children (via the shared `provide_group_base`, mirroring `TextTheme`), and views-within-runs.
  The web realization shares an `open_run` helper with `Text` (the only difference is
  block-vs-inline display), and `Styler::finish` now assigns no class when a run is unstyled.
  Tested under SSR: an inline run is classless (no `display:block`, unlike `Text`), groups nest
  and views embed inline, and a group's color cascades to a colorless child.
