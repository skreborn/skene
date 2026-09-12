# Roadmap

What is open and what is planned. The architecture is in `DESIGN.md`, the doctrine in
`AGENTS.md`; this file records neither, only the work that remains and the shape it
lands in.

## The redesign shipped

The redesign to one spine and thin facades is complete: every phase landed and the
workspace is green on all three tiers. The shape it left is the one `DESIGN.md`
describes—core owns a retained scene and the zero-sized `Driver` trait; `skene-views`
hosts every built-in view once, generic over the driver; `skene-gui-core` and
`skene-tui-core` hold what each medium's drivers share; `skene-web`, `skene-headless`
and the terminal implement the whole trait; the facades are a scalar, a condition set, a
style extension and aliases; `skene-simple` is an explicit dependency, aliased per facade by `skene-simple-gui` and `skene-simple-tui`; a control is `Control<B: Behavior>`; a token is a named
inherited slot a `Theme` defines; motion is data; the snapshot is positional and on by
default; the old vocabulary and the transitional namespaces are gone; and `README.md`,
`DESIGN.md`, `AGENTS.md` and this file describe the same code.

The gates it passes, and which stay part of every change: no deleted name compiles
anywhere; `cargo hack check --feature-powerset --workspace` on every tier; the host,
wasi and browser suites; every example on every target it supports; the third-party
crates under `examples/third-party` building and testing on their drivers; the design
doc's crate map matching `Cargo.toml`.

The reactive engine's baseline, `cargo bench -p skene-reactive --bench engine` on the
development machine at the redesign's close (2026-09-05), against which a change to
the engine is measured: creating a thousand signals 78 µs; a signal read outside an
effect 11 ns; a write reaching ten effects 260 ns; a write through a chain of ten memos
393 ns.

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

Follow-ups the redesign noted and left; none blocks a release, each is scoped. The
`Next` list is the order to take them in; the areas below hold every item with its
reasoning.

### Next

1. **Stores:** derived (memoized) lens reads and a keyed-iteration helper feeding
   `reconcile`.
2. **`LazyColumn` variable-extent items and horizontal lists**, then CSS anchor
   positioning for overlays.

Watched, not scheduled: `ErrorBoundary` stays on probation until usage decides it; the
picker's type-ahead waits on a clock no behavior has.

- **Spine and views**
  - The picker has no type-ahead. Type-ahead needs a timer to reset its buffer, timers
    are userland, and no driver exposes a clock to a behavior; the terminal's picker
    lacks it as well. `Tab` commits the focused option on both.
  - `Property::Border` transitions the width and the ink together with one timing; the
    terminal draws any width as one cell, so only the ink is visible there.
  - `ErrorBoundary` stays on probation: it exists only for errors with no render site
    of their own. If per-resource matching keeps covering real apps, delete it.
  - **The context menu is not a view** (reported 2026-09-12). The terminal paints its
    copy and edit menus inside the driver (`selection.rs`, `paint.rs`), and the web
    leaves the browser's own. Wanted on every facade: a reusable default menu a view
    can open, extend with its own items, or replace, with the built-in copy, cut and
    paste entries composed from the same public parts.
- **Web**
  - **The WebGPU renderer behind `Canvas`** landed (2026-09-08): vello's hybrid
    renderer over wgpu, chosen over the compute-shader `vello` crate because it shares
    `vello_common`'s scene vocabulary with the CPU renderer, so a `Canvas` records once
    and replays into either through one `Renderer` trait, and because it needs no
    compute shaders. One device is shared by every painted box, each canvas owns a
    surface, and the renderer resizes per frame. The CPU path stays the fallback where
    `navigator.gpu` is absent. The browser tier proves the GPU path: Firefox runs with
    WebGPU switched on through `packages/gui/webdriver.json`, and the painted test reads
    the fill back through a scratch canvas, which works for either path. Left:
    - The CPU fallback is no longer exercised by the browser tier; it is the same
      recording rendered by the same `vello_cpu` the headless tier reads, so only the
      blit is unproven there.
    - The hybrid renderer also runs on WebGL2 (its `webgl` feature, no wgpu) for
      engines without WebGPU; not built, since the CPU path already covers them.
    - wgpu adds noticeably to the bundle; measure it before an app that paints nothing
      pays for it, and consider a lazy split if it does.
  - Client hints are Chromium-only (`Accept-CH`/`Critical-CH`); other engines get the
    documented defaults.
  - A text entry is its own `<input>`/`<textarea>`, which holds no children, so its
    `Label` part and body land nowhere on the web (the accessible name still does).
    A caption wants a `<label for>` sibling the driver places beside the field.
  - **Progressive enhancement is only the built-in controls'** (reported 2026-09-12).
    A control keeps its scriptless form and hydrates in place, but a user's view has no
    equivalent seam: a date view that renders an absolute date on the server and
    upgrades to a live "ago" reading once the bundle arrives has to be hand-rolled.
    Wanted: a documented, public path for a view to declare its no-JS form and its
    hydrated enhancement, on the same footing as the built-ins.
- **Terminal**
  - Selection and clipboard inside a text entry landed (2026-09-08): the entry's own
    anchor beside its caret, `Cut`/`Copy`/`Paste` in its edit menu, `Ctrl-C`/`X`/`V`
    through the ordinary dispatch, bracketed paste as the paste source everywhere, and an
    OSC 52 read where XTVERSION names kitty, ghostty, foot, xterm, tmux or contour. A
    drag held past an entry's edge autoscrolls it on the frame clock (2026-09-08): the
    window moves only to keep the caret in view, as a browser's does, and the pointer's
    distance past the edge sets the pace, in the field and the editor alike.
  - **Replies are events, never waits** (landed 2026-09-08). The loop runs on termina,
    whose parser delivers CSI, OSC and DCS replies in the input stream beside the keys,
    so no query has a deadline, nothing deafens the loop, and a late reply cannot leak
    into the key stream. Launch writes its queries and paints at once under its
    assumptions—dark, 16 colors, legacy keys; a reply upgrades an assumption whenever
    it lands, and a 2031 notification moves the scheme the same way. **Every
    capability is queried, never inferred**: a 2031 report and OSC 11 for the
    background, the kitty keyboard flags query, and a DECRQSS SGR round trip for the
    depth, after a true-color and after a palette background. XTVERSION, `COLORTERM`
    and `TERM` go unread—a name is not a capability, and a list of names is a guess
    that ages. OSC 52 has no capability query and no negative acknowledgement in either
    direction, so `Cut`, `Copy` and `Paste` are offered unconditionally: a write a
    terminal discards is silent, and a read fires on demand—a terminal that asks its
    user asks then, never at launch—and inserts when the report arrives, into the entry
    that asked, dropped if that entry is gone or none asked; a second `Ctrl-V` while
    one is pending retargets rather than queues, and a terminal that never answers
    leaves its own paste key, which arrives bracketed. Termina parses no OSC 52 report
    upstream; the fork's `parse-selection` branch is the git dependency until the PR
    merges. Owning the read loop around termina's public `Parser` was the fallback and
    is not needed. **The clipboard and the mouse are launch configuration** (landed
    2026-09-08): `Launch::set_clipboard` names which of copy, cut and paste exist
    (`Clipboard::ALL` by default; without copy nothing in the document is selectable,
    a key not offered is an ordinary key, and the edit menu lists only what is), and
    `Launch::set_mouse(false)` sets no mouse mode and ignores every mouse event, so a
    plain drag is the terminal's selection. Left:
    - A runtime mouse release—a key handing the mouse to the terminal for one native
      selection and taking it back—would spare relaunching with the mouse off. Not
      built; every mouse mode is set once at launch.
    - A terminal answering no DECRQSS paints at 16 colors, Terminal.app among them; the
      cost of inferring nothing, taken knowingly.
    - Termina's SGR reply parser reads `:`-separated color parameters only, so a
      terminal reporting `48;2;r;g;b` has its reply dropped and stays at 16 colors—an
      upstream fix.
    - Windows input runs through ConPTY, which helix found buggy with CJK and
      non-ASCII text and reverted; termina's `windows-legacy` reader is the fallback,
      and neither is tested here.
    - Queries written before raw mode takes effect print their replies to the screen
      (helix's finding over SSH); the loop enters raw mode first, and keeps that order.
  - Explore per-input undo/redo (`Ctrl-Z`, `Ctrl-Shift-Z`/`Ctrl-Y`): a bounded ring of
    (value, caret, selection) per text control, matching what the DOM gives an `<input>`.
  - Explore splitting `skene run tui` like the web's: keep the invoking terminal for the
    build loop and logs, and print a runner command that attaches the app in a second
    terminal.
  - **The terminal reports of 2026-09-12 landed** (2026-09-12). A key's default runs
    after the handlers' writes have taken effect, as a browser's does after its
    microtask checkpoint, so `Tab` on a picker's option commits, closes the popup and
    travels on to the next stop instead of into the closed list. A press on the anchor
    a popup opened from is the anchor's click and never a dismissal, as on the web, so
    the face of an open picker closes it. A modal takes focus to its first tab stop as
    it opens and returns it to the node that held focus as it closes, and a popup
    closing with focus inside returns it the same way, as `<dialog>` and a popover do.
    The wheel scrolls an editor's window over its wrapped rows without moving the
    caret, consuming at its ends like any scroll target; a caret scrolled out of view
    hides until focus or an edit reveals it, and the window remembers the width it was
    settled at so only a resize re-settles it. A borderless box that is its parent's
    only laid-out child and holds none paints the eighths its laid-out width ends in
    as a block glyph in its color over what lies beneath, so a slider's fill moves by
    eighth cells. The badge's size was the library's own: `Inset::symmetric(8, 2)` in
    cells is eight columns by two rows, so `acme-ui` now states its inset per unit
    through `AcmeUnit`, exactly as `SimpleUnit` carries the Simple shape. Left:
    - The thumb is a one-cell box at a rounded position, so it still moves by whole
      cells; only the fill is smooth.
    - A fractional edge is horizontal only; a vertical fill ends on a whole cell, since
      only the lower blocks exist and a top-anchored fill would need upper ones.
- **Snapshot and stores**
  - A key is matched by a 127-bit hash of its value, so two values hashing alike share
    one scope and their state crosses on a restore; a collision check at every build
    was judged not worth its cost. The exact form—a keyed step carrying the value's
    postcard bytes and a type hash, at a `Serialize + DeserializeOwned` bound on every
    key and a heap step per keyed scope—is the route if the hash ever proves short. `Position` and node positions (2026-09-07) closed the
    creation-index carry: the web and the terminal restore focus, a text control's
    selection or caret and scroll offsets by position, keys included, and open-popup
    state is the app's bound signal already. The headless driver carries no
    restoration; a test of it runs on a real driver.
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
