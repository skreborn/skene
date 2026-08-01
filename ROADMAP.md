# Roadmap

The phased migration to the driver-vocabulary architecture (`DESIGN.md`), then the platform build-out.
Phases are ordered; later phases assume the earlier ones. Native is deliberately
last—not a current priority. **Current focus: vocabulary completeness (events,
controls, head management) ahead of the CLI.**

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
  - Investigate double-classing: one shared class per element kind carrying its reset,
    one content-addressed class carrying its initial style (dynamic values stay inline
    overrides)—so the reset text stops being duplicated into, and hashed with, every
    generated class.

## Phase 4—Events and first controls ✅ (core)

- **Done.** Portable event vocabulary: click, pointer enter/leave, focus/blur, key down
  (`KeyEvent`: W3C key names + modifiers), input. On the web the bubbling events are
  **delegated**—one listener per event type on the mount root, dispatch walks the
  target's ancestors by a stamped node id—and the non-bubbling pointer enter/leave
  attach per node. Every dispatch runs inside an implicit reactive batch. First control:
  `Input` (web `<input>`, a void element in SSR output) with
  `bind_value`/`derive_value`/`handle_input`/`handle_key_down`, placeholder, and the full
  declarative styling surface (`set_style`, `set_when`, transitions, animations).
- **Events complete.** Pointer down/up/move (`PointerEvent`: viewport position, button,
  modifiers; delegated), scroll (`ScrollEvent` on a box with `set_overflow`), change
  (committed value), and focus management: `set_tab_index`/`set_autofocus` (serialized
  in SSR output) plus `FocusHandle` for programmatic focus. `KeyEvent` modifiers moved
  into a shared `Modifiers` struct.
- **Basic controls—done.** `Button` (the one focus-taking box; a `Container` never
  takes focus—controls do), `Checkbox`, `Radio` (grouped, arrow-key movement),
  `Select` (fixed options), `Textarea`, `Slider`—each realized as its semantic element
  (native keyboard and accessibility behavior for free), unstyled by default, sharing
  one builder surface (`ControlBase` + the `control_surface!` macro: styling, variants,
  transitions, animations, tab order, autofocus, focus binding, focus/blur/key
  handlers) and two-way signal binding (`bind_checked`/`bind_value`). `Role` expanded:
  navigation/main/header/footer/aside/section/article/heading(level)/list/list-item
  realize as their semantic elements. SSR serializes control state (`checked`,
  `selected` options, textarea text content, range attributes).
- **Remaining:** a styled companion package (optional dependency), once theming
  settles.

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
- **Route enumeration and the generated page—done.** `install_static_router` records
  `StaticRoutes::all`; the server binary's `--routes` mode prints them;
  `Runner::routes()` reads them; and `skene build web --static` renders every
  enumerated route when given no `--path`. `skene build web` also generates the CSR
  boot page into `dist/web/` (dev builds carry the dev marker and reload script at
  generation time; the server injects nothing), so no hand-written `index.html`
  remains.
- **Remaining:** streaming through the runner (`render_to_stream` over incremental
  stdout); typed link helpers (an `<a>` that navigates without a reload).

## Phase 6—TUI facade ✅ (core)

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
- **Parity—done.** Pointer down/up/move with the terminal's own `PointerEvent` (`u16`
  cells, terminal-absolute and view-relative, button, modifiers); `set_title`/
  `derive_title` via OSC 2; `FocusHandle` + `set_autofocus`; `Input::handle_change`
  matches the GUI facade's real-time naming. `Button` is the focus-taking box (`Tab`
  reaches it; `Enter` and `Space` activate it); focusability belongs to controls, never
  plain containers—a clickable box stays mouse-clickable but out of the tab order,
  exactly like the web.
- **Remaining (parity):** the rest of the control set in cells (checkbox `[x]`, radio
  `(•)`, slider track, select cycling)—textarea waits on caret positioning.
- **Remaining:** text wrapping; scroll (incl. mouse wheel); caret positioning in inputs
  (edits append/pop only); a diffed redraw instead of full frames; selectable border
  character sets (rounded `╭╮╰╯`—the terminal's border radius—double, heavy, ASCII,
  custom); box shadows (always a 1×1 offset to the bottom-right, color and opacity
  configurable, rendered by mixing the shadow color into whatever lies beneath—both the
  foreground and the background color of the shadowed cell); a compositor pass where
  every layer draws into one internal buffer first (so a later layer reads the colors
  already placed—the shadow mix depends on it—and the frame flushes at once).

## Backlog—unphased features

Wanted, not yet scheduled into a phase. (Confirmed already true, recorded here so it
stays true: **URI paths on all platforms**—`Route::parse`/`to_path` speak URL paths
everywhere; the browser History API, the server request path, and the TUI's in-memory
sink are just different sinks for the same paths.)

- **HTTP client** as a separate crate (`skene-http`), built on the public seams
  (`spawn`, `Resource`) exactly as a third-party client would be—same footing is the
  point. A WebSocket sibling follows the same rule.
- **Overlays—done (web + TUI).** `Overlay::modal()` / `Overlay::popup()` on both
  facades: open state signal-driven (`bind_open`), dismissal always a request
  (`handle_dismiss`—Escape, backdrop click, or a press outside a popup; the overlay
  never closes itself), placement against the viewport or an `Anchor`-bound view
  (`Top`/`Bottom` open above/below the anchor), and a backdrop for modals. Web realizes
  the top layer natively—`<dialog>`+`showModal` (focus trap, `::backdrop`) for modals,
  a manual popover for popups, both serialized closed by SSR; the TUI paints overlays
  in a second pass over the cell buffer, hit-tests them first (nothing beneath an open
  modal is reachable, and it contains the tab order), and re-roots their layout so they
  escape normal flow. Remaining: flip-when-out-of-space anchoring, CSS anchor
  positioning where supported, and the portal-to-arbitrary-plane case if it ever earns
  its keep. Native: the platform compositor's layers, with the same vocabulary.
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
- **Titles and head management**: **web done**—`head::set_title`/`derive_title` plus
  `add_meta`/`add_meta_property`/`add_link`/`add_stylesheet`/`add_script`, serialized
  into the SSR/SSG `<head>` (escaped, registration order) and applied live under CSR,
  where an identical element left by a server render is adopted, not duplicated.
  Remaining: the TUI title via OSC 0/2 and the native window title.
- **Light/dark detection in TUI**: query the terminal background via OSC 11 and expose
  `DarkMode`/`LightMode` condition leaves, mirroring the GUI vocabulary.
- **Truecolor detection in TUI**: detect support (`COLORTERM`/terminfo) and approximate
  to 256- or 16-color palettes when absent, instead of assuming truecolor.
- **Asset system**: declared in `skene.toml`—the framework manifest, which will grow
  further settings over time (Flutter's pubspec is the model)—never in code and never
  statically checked. Declared files and directories become reachable at predictable
  URIs; the CLI serves them in dev, copies them into web/SSG output, and bundles them
  for native. A library crate's own `skene.toml` (found beside its `Cargo.toml` via
  cargo metadata) contributes assets the same way an app's does.
- **View transitions (web)**: best-effort—`document.startViewTransition` wrapping SPA
  route changes and `@view-transition` in generated CSS for MPA/SSG navigations,
  degrading to instant swaps where the browser lacks support.
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
  crash recovery). **Core landed:** the opt-in `snapshot` cargo feature (forwarded by
  both facades) gives `RwSignal::tagged("tag", default)`—the tag is the identity across
  runs, so the arena redesign is decoupled—plus `snapshot()`/`restore()` over
  serde+postcard: staged values apply exactly once on creation, failed decodes degrade
  to the default, corrupt snapshots stage nothing, duplicate live tags panic.
  Remaining: driver extras riding along (focus, scroll), and the CLI holding the
  snapshot across dev reloads.
- **Accessibility**: AccessKit on native (already in phase 8's shared machinery) and in
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

## Phase 7—CLI

- One tool to build/run/serve per platform: wasm bundling + the generated CSR page, SSG
  output, the dev server, native builds.
- **First milestone landed.** The `skene` binary (`packages/cli`, `cargo skene` in the
  workspace) subsumed and deleted the xtask, structured by target: `skene build web`
  (wasm + bindings), `skene build web --static` (routes through the embedded runner,
  assets alongside), `skene build tui` (the terminal binary), `skene run web` (build,
  serve, watch, rebuild, live-reload over SSE—serving only the page, the bundle, and
  declared assets), and `skene run tui`. Every output lands under `dist/<target>`
  (`web`, `web-static`, `tui`; `ios`/`android` when the native family arrives).
  `skene.toml` is read and its `[assets]` paths are collected across the dependency
  graph via cargo metadata (app wins collisions), served at their declared URIs in dev
  and copied into static output.
- **State-preserving reloads—done (web).** One protocol for every target: the dev
  server holds the authoritative snapshot (`POST`/`GET /skene:state`), the injected
  script beacons `skene_snapshot()` before each reload (and on `pagehide`), and—behind
  the `snapshot` feature—`launch`/`hydrate_page` fetch and stage the held bytes before
  mounting, gated by the dev server's injected `skene-dev` meta marker so production
  pages boot fetch-free. Assets are scoped per package: every URI is
  `/<package>/<declared path>`, so app and library assets can never collide.
- **The terminal dev loop—done.** `skene run tui` runs the binary with `SKENE_DEV`
  naming an ephemeral state port, watches, rebuilds on change (the old process stays up
  through the compile, and through a failed one), then swaps in the new binary. The
  app-side handshake mirrors the web's over the same `/skene:state` protocol: restore
  before mount, then debounced checkpoints off the UI thread after every event batch,
  plus a final flush on quit—so a kill at any moment loses at most a fraction of a
  second of state. Verified end to end under a pseudo-terminal: three clicks, a source
  touch, a rebuild-restart, and the fresh process painted the preserved count.
- **Remaining here:** wasm-split lazy loading, and richer `skene.toml` settings as they
  earn their place.
- The dev loop is watch + rebuild + full reload; state survives via the serializable
  reactive state in the backlog—the CLI holds the snapshot across the restart.
- `skene.toml` is read here: the manifest-declared assets (see the backlog) are served
  in dev, copied into SSG output, and collected from library crates' manifests across
  the dependency graph.
- Route-based wasm code splitting with lazy loading, where the splitting tooling allows.

## Phase 8—Native driver family

The largest phase by far, and deliberately the last: it realizes an
already-complete vocabulary once, instead of chasing additions.

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

