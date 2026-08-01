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

- **Done.** `BoxProps`/`RunProps` carry `variants: Vec<(Condition, Style)>` (media
  features and interaction states, composable to any depth with `Condition::and`/
  `Condition::or`), `transitions`, and `animations` (keyframes over the same style
  type). Builders: `set_when`, `set_transition`, `set_animation` on
  `Container`/`Text`/`Span`;
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
  (`MinWidth`/`MaxWidth`/`MinHeight`/`MaxHeight` in cells, `Focused`, `Hovered`)—so nothing a
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
- **Mouse—done.** Motion drives `Condition::Hovered` variants and pointer enter/leave
  handlers; left clicks hit-test against the taffy layout, focus the target, and run its
  click handlers; a hovered node's `cursor` sets the pointer shape via OSC 22. Headless
  `trigger_mouse_move`/`trigger_mouse_click` seams drive it in tests.
- **Remaining:** text wrapping; scroll (incl. mouse wheel); caret positioning in inputs
  (edits append/pop only); a diffed redraw instead of full frames; selectable border
  character sets (rounded `╭╮╰╯`—the terminal's border radius—double, heavy, ASCII,
  custom).

## Backlog—unphased features

Wanted, not yet scheduled into a phase. (Confirmed already true, recorded here so it
stays true: **URI paths on all platforms**—`Route::parse`/`to_path` speak URL paths
everywhere; the browser History API, the server request path, and the TUI's in-memory
sink are just different sinks for the same paths.)

- **HTTP client** as a separate crate (`skene-http`), built on the public seams
  (`spawn`, `Resource`) exactly as a third-party client would be—same footing is the
  point. A WebSocket sibling follows the same rule.
- **Overlays**: portals, popups, modals, tooltips—content mounted outside its parent's
  clip/z-order. Web: top-layer (`<dialog>`/popover) or a portal to `<body>`; native: the
  platform compositor's layers; TUI: a second paint pass over the cell buffer.
- **Container queries** next to media queries: `Condition` leaves resolved against an
  ancestor container's size, not the viewport. Web compiles to `@container` (the driver
  stamps `container-type` on the queried ancestor); TUI and native resolve them against
  the taffy results they already have.
- **Animation in TUI**: run the frame ticker against tui style data—color and attribute
  interpolation foremost, but sizes and positions are just as animatable in cells.
- **Panic hook for TUI**: a hook that restores the terminal (cooked mode, main screen,
  mouse capture off, cursor shown) before the panic message prints, so a crash never
  leaves the terminal wedged; restore also runs on normal unwind today via the raw-mode
  guard.
- **Titles and head management**: `set_title`/`derive_title` everywhere—web
  `document.title`, TUI OSC 0/2, native window title—and on web the full head
  vocabulary (title, meta, link, stylesheet, script) rendered into SSR/SSG heads and
  kept live under CSR.
- **Light/dark detection in TUI**: query the terminal background via OSC 11 and expose
  `DarkMode`/`LightMode` condition leaves, mirroring the GUI vocabulary.
- **Truecolor detection in TUI**: detect support (`COLORTERM`/terminfo) and approximate
  to 256- or 16-color palettes when absent, instead of assuming truecolor.
- **Asset system**: declare assets (fonts, images, stylesheets, scripts) in code; the
  build tool bundles them with the output (hashed URLs for web/SSG, embedded or
  alongside for native/TUI).
- **Component libraries**: third-party crates shipping components—views plus
  facade-specific code plus assets—consumable on equal footing with built-ins. Depends
  on the asset system; the realize-once vocabulary already permits the code side.
- **Serializable reactive state**: the signal graph snapshots to a byte stream and
  restores on startup. Strictly **opt-in**—serializability is never a bound forced on
  users' types; the likely shape is an optional unique tag on a signal, with only
  tagged state restored (exact logic open). The snapshot also carries the extras that
  live outside the graph—the focused node, scroll positions, and similar platform
  particularities—which the drivers fully own and can replay. Powers state-preserving
  dev reloads (full restart, no hot-patching) and stands on its own (session restore,
  crash recovery). Stable identities across runs are the hard prerequisite; the
  phase-1 arena redesign must be shaped with this in mind.
- **Accessibility**: AccessKit on native (already in phase 6's shared machinery) and in
  the TUI where it can reach; on the web, `Role` expansion and semantic output audited
  against ARIA. A stated commitment across facades, not an afterthought.
- **Reactive stores**: fine-grained nested reactivity over structs and collections
  (derive-based; field- and key-level subscriptions), generally available on the same
  signal substrate.
- **Error/suspense audit**: name what `Boundary` + `Resource` + `Deferred` actually
  guarantee against the error-boundary/suspense/transition triple, and fill what falls
  short.
- **All basic controls**: checkbox, select, textarea, radio, slider, and friends—
  unstyled by default with full keyboard and ARIA behavior; a styled set may ship as a
  separate optional package.
- **List virtualization**: windowed rendering for large lists, on every facade (the TUI
  scroll view and native lists need it alike).
- **Internationalization**: first-class eventually—message catalogs, locale-aware
  formatting, reactive locale switching.

Decided against, for the record: hot-patching (state-preserving reloads come from
serializable reactive state instead); a custom devtools/inspector (native realizes into
the platform view hierarchy, so the platform's own inspectors already work; platforms
without one may grow minimal introspection later); islands architecture (compiled no-JS
styling already removes most reasons a static page ships wasm); LiveView-style server
rendering (adjacent to the server-side non-goal); a webview desktop shell.

## Phase 8—CLI

Explicitly last, after the stretch goals above.

- One tool to build/run/serve per platform: wasm bundling + the generated CSR page, SSG
  output, the dev server (subsuming `cargo xtask web`), native builds.
- The dev loop is watch + rebuild + full reload; state survives via the serializable
  reactive state in the backlog—the CLI holds the snapshot across the restart.
- Route-based wasm code splitting with lazy loading, where the splitting tooling allows.
