# Roadmap

What is open and what is planned. The architecture is in `DESIGN.md`, the doctrine in
`AGENTS.md`; this file records neither, only the work that remains and the shape it
lands in.

## The redesign shipped

The redesign to one spine and thin facades is complete: every phase landed and the
workspace is green on all three tiers. The shape it left is the one `DESIGN.md`
describes—core owns a retained scene and the zero-sized `Driver` trait; `skene-views`
hosts every built-in view once, generic over the driver; `skene-web`, `skene-headless`
and the terminal implement the whole trait; the facades are a scalar, a condition set, a
style extension and aliases; a control is `Control<B: Behavior>`; a token is a named
inherited slot a `Theme` defines; motion is data; the snapshot is positional and on by
default; the old vocabulary and the transitional namespaces are gone; and `README.md`,
`DESIGN.md`, `AGENTS.md` and this file describe the same code.

The gates it passes, and which stay part of every change: no deleted name compiles
anywhere; `cargo hack check --feature-powerset --workspace` on every tier; the host,
wasi and browser suites; every example on every target it supports; the third-party
crates under `examples/third-party` building and testing on their drivers; the design
doc's crate map matching `Cargo.toml`.

### The audit's defects, closed

Each is fixed where its mechanism now lives:

- `derive_child` never rebuilt (it built untracked through `reconcile`). A derived child
  is `derived`, which tracks; `reconcile` builds items untracked. `skene_core::derived`.
- A derived property was an inline style on the web and outranked every variant; the
  terminal overlaid variants over it. A derived leaf is a per-node custom-property slot
  the class references, registered `inherits: false`. `skene_web::compile`,
  `packages/gui/tests/theme.rs`.
- A GUI button with a plain handler bubbled to its row; the terminal's did not. A
  behavior's verdict is the whole verdict on every driver. `skene_views::control`,
  `packages/views/tests/controls.rs`.
- Every drawn Simple control styled `Focused` on a child of the focusable node, so no
  ring rendered. Control state resolves against the enclosing control.
  `skene_core::dispatch`, `packages/gui/tests/web_dom.rs`.
- Hydration injected the reset and one `<style>` per class beside the server's block. The
  server's stylesheet is adopted. `skene_web::hydration`, `packages/gui/tests/web_hydrate.rs`.
- Streaming rendered styles once at the shell, so a later boundary streamed unstyled.
  Every chunk opens with a `<style>` delta for the classes interned since the previous
  flush. `skene_web::ssr`, `packages/gui/tests/web_ssr.rs`.
- `Suspense` and `ErrorBoundary` never destroyed the branch they detached. Both are built
  on `Show`/`Switch` and destroy the losing branch on disposal.
  `packages/views/tests/boundaries.rs`.
- A memo was queued as an effect and recomputed unconditionally. Two-color propagation:
  memos are lazy and compute once per flush. `packages/reactive/tests/memo.rs`.
- `batch` and `Owner::enter` had no drop guard. Both are guarded.
  `packages/reactive/tests/borrow_panic.rs`.
- `set_sync_effect_waker` had no caller. The sync waker routes through the scheduler.
  `packages/reactive/tests/scheduler_sync.rs`.
- A radio group with nothing chosen had no tab stop. The roster is read from the tree
  and the tab stop follows the choice. `packages/gui/tests/headless.rs`.
- The recording driver ran only the target's click handlers, did not unlink on reparent
  and numbered nodes from 0. It is deleted; the headless driver implements the whole
  trait over core's scene and dispatch.
- Class naming depended on intern order and liveness. Names are content-addressed from a
  wide hash and emitted in interning order on both leaves. `skene_web::interner`.
- A group node was a real `<div>` and a real taffy node. Reactive subtrees mount between
  anchors. `packages/views/tests/headless.rs`.
- The terminal called `set_style` on every node every frame. The effective style is
  cached per node and written on change; damage rows bound the repaint.
  `packages/tui/tests/layout.rs`.
- Three control doc blocks described a retired handler override as shipped. The override
  is gone and so are the blocks.

## Open

Follow-ups the redesign noted and left; none blocks a release, each is scoped.

- **Spine and views**
  - The picker has no type-ahead. Type-ahead needs a timer to reset its buffer, timers
    are userland, and no driver exposes a clock to a behavior; the terminal's picker
    lacks it as well. `Tab` commits the focused option on both.
  - `Property::Border` transitions the width and the ink together with one timing; the
    terminal draws any width as one cell, so only the ink is visible there.
  - `ErrorBoundary` stays on probation: it exists only for errors with no render site
    of their own. If per-resource matching keeps covering real apps, delete it.
  - Optional keys on every view, so focus, caret, selection and open-popup state restore
    through the same alignment as signals. `set_key` is the name to weigh; it should be
    the concept `reconcile` keys lists by rather than a second one.
- **Web**
  - `Painting` has no web realization: `Painted` content through `<canvas>` + wgpu, with
    the vello CPU/hybrid path where WebGPU is absent. The headless driver records the
    binding; the server leaf renders it empty by design.
  - `WebMarkup::set_text_content` carries the DOM's name for a method that also exists on
    the server leaf; a driver-neutral name is wanted.
  - Client hints are Chromium-only (`Accept-CH`/`Critical-CH`); other engines get the
    documented defaults.
- **Terminal**
  - Keyframe token leaves resolve every frame rather than once per generation.
  - Selection and clipboard inside a text input: the source-anchored selection model
    extended over an input's value (drag, shift-arrows, double and triple click,
    insertion replacing the selection), with `Cut` and `Paste` in the menu. Paste is the
    hard half: reading the clipboard needs an OSC 52 query most terminals refuse, so the
    reachable design is bracketed paste as the source, with the menu item pulling from an
    OSC 52 read where XTVERSION says it works.
  - `Ctrl-Q` as the unconditional quit, freeing `Ctrl-C`/`X`/`V` for copy, cut and paste
    through the ordinary dispatch contract.
  - Explore per-input undo/redo (`Ctrl-Z`, `Ctrl-Shift-Z`/`Ctrl-Y`): a bounded ring of
    (value, caret, selection) per text control, matching what the DOM gives an `<input>`.
  - Live scheme change notifications (DECSET 2031) instead of the one launch query.
  - Explore splitting `skene run tui` like the web's: keep the invoking terminal for the
    build loop and logs, and print a runner command that attaches the app in a second
    terminal.
- **Snapshot and stores**
  - A positional identity shifts when an owner or effect is inserted before it; a tag
    anchors the diff, and a keyed list item's identity is still its creation position
    (the view-key exploration above would anchor it).
  - A `&'static str` signal is `opaque`, since it cannot be deserialized owned.
  - Stores: derived (memoized) lens reads and a keyed-iteration helper feeding
    `reconcile`.
  - On a hydrated page with several lazy views racing, order-keyed resource adoption
    inside them can cross; tags cover it, ordering the arrivals would too.
- **Lists and overlays**
  - `LazyColumn`: variable-extent items and horizontal lists.
  - Overlays: CSS anchor positioning where supported.

## Backlog

Wanted, not scheduled.

- **HTTP client** as a separate crate (`skene-http`), built on the public seams
  (`spawn`, `Resource`) exactly as a third-party client would be. A WebSocket sibling
  follows the same rule.
- **Asset system growth.** `skene.toml` declares assets today, served in dev, copied into
  static output and collected across the dependency graph; it grows further settings
  as they earn their place (Flutter's pubspec is the model). Font assets add generated
  metadata—per-face ascender, descender, line gap, cap height, read at build time—so
  the web can emulate text-box trimming where an engine lacks it and native and the
  terminal lay text out with the same numbers.
- **Accessibility**: AccessKit on native and in the terminal where it can reach; on the
  web, `Role` expansion and semantic output audited against ARIA. `aria-activedescendant`
  is deferred while roving focus reaches the same place; it needs a node-valued
  `AccessProp` and stable per-node ids in the web driver.
- **Internationalization**: message catalogs, locale-aware formatting, reactive locale
  switching.
- **The slider's standard parts.** The web slider is drawn, so nothing waits on them,
  but when engines ship `::slider-fill-*` (csswg-drafts#12419) the native input's track
  can be restyled through them instead.

Standing notes, attached to no step:

- **The children-last rule could ship as a lint.** Nothing enforces the ordering
  convention. Rust has no stable way to ship a lint that runs under plain
  `cargo clippy`—the ecosystem answer is a **dylint** crate at the cost of pinning an
  exact nightly. Worth remembering, not worth scheduling.
- **`IntoChildren`, `IntoChildOf`, and `IntoView` cannot be replaced by std traits**—
  checked against the compiler. A blanket `impl<I: IntoIterator<Item = V>> IntoChildren
  for I` conflicts with the tuple impls, and the same conflict appears with `From`;
  tuples are not optional because children are heterogeneous. `Into<AnyView>` cannot
  replace `IntoView` either, since `AnyView` is itself a view.

Decided against, for the record: hot-patching (state-preserving reloads come from the
snapshot instead); a custom devtools/inspector (native realizes into the platform view
hierarchy, so the platform's own inspectors already work); islands architecture
(compiled no-JS styling already removes most reasons a static page ships wasm);
LiveView-style server rendering (adjacent to the server-side non-goal); a webview
desktop shell.

## Planned: the native driver family

The largest remaining phase, and deliberately the last: it realizes an already-complete
vocabulary once, instead of chasing additions.

- Shared machinery: taffy layout, vello paint, parley text, AccessKit, the frame clock
  running `skene_core::interpolate` as the terminal does, `Condition` resolution against
  window and pointer signals through the one resolution point.
- Compositing premise: **the platform composites.** On iOS/Android/macOS the driver
  realizes the tree into the native view hierarchy—painted content in vello-backed
  layers that are views, embeds as ordinary native views—so custom paint and real
  platform controls interleave in z-order for free. The Linux leaf is
  winit + single vello surface (no native control set exists to embed); Windows chooses
  per its composition APIs.
- The `PlatformView` escape trait and per-leaf embed constructors (`ios::native_view`,
  …): real text fields, maps, web views.
- View transitions: snapshot the outgoing tree into a layer and interpolate named pairs.
- Off-thread delivery: the engine's scheduler wired to the event-loop proxy.
- A loop-integrated timer (the one sanctioned exception to timers-are-userland).
- Per-OS glue in leaves: IME, clipboard, menus, window management.
- `skene-gui` resolving native triples to it; `skene build`/`run` for `ios`, `android`
  and the desktop leaves, landing under `dist/<target>`.
