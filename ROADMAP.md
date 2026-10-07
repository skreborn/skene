# Roadmap

What remains, in the order to take it. The architecture is in `DESIGN.md`, the doctrine
in `AGENTS.md`, the decisions against in `DECISIONS.md`; this file records none of them,
only the work that is open and the shape it lands in. What has shipped is not listed:
the code, `DESIGN.md` and the tests describe it.

## Open

One list, every item big and small, top first. An item is resolved by landing it, by
proving it unnecessary, or by recording it in `DECISIONS.md`; it is never left half-done
in a code comment.

1. **One owner for style and interaction semantics.** The web compiler and the spine's
    `Resolver` implement one meaning twice, and the terminal, headless and the web each
    implement focus, press, hover and overlays; each mechanism gets one owner, and the
    browser is the oracle.
    - The parity harness (`browser_parity`) gains a case for each fix below.
    - Interaction: a group run's state variants reach its spans, as a rule on the
    group's class redefining the ink where the state holds; the token definitions it
    redefines condition on media alone, so this waits on the styling direction.
    - Hit testing: a run is hit-tested by its untrimmed line box, which overhangs the
    box `text-box-trim` trims its line to, so in both engines a press near the edge of
    one of a column's text boxes lands on the next box's run, at a row's vertical
    centre in Chromium; a run inside a control takes no pointer, and every other run
    still takes the press its overhang covers, its own `Hovered` with it.
    - Typed events: a gesture arena before touch lands, its exception to "a view that
    handles an interaction owns it" written down.
    - Controls: `AccessProp` moves onto AccessKit's schema, beside its open `Role`: its
    properties in place of the closed report, so a third party builds switches, tabs,
    comboboxes and trees; parts become styling targets or go.
    - Structure: anchors exist in the scene alone, platforms receiving the next real
    sibling and server output keeping comment pairs only where streaming needs a
    marker.
    - The web's hydration: the server tier writes real pages that the browser tier
    hydrates with zero mismatches, where the browser tests fake the served markup today;
    structural hydration keys per scope replace the one global counter, so async
    rebuilds and nested lazy content keep their markers and their server markup; where
    `appearance: base-select` is supported, the picker is a styled native `<select>` and
    the hydration swap goes, the drawn face remaining elsewhere.
2. **Tests that exercise what users see.** Headless becomes the web's real stand-in,
    and every tier asserts through roles, text and pixels.
    - The terminal's retained runtime (the taffy store, the incremental restyle, the
    damage and the measured rectangles, hit testing, focus, capture, wheel latching,
    field editing, selection, motion) moves into `skene-layout`, generic over the
    platform's scalar, which holds the style mapping and a tree headless lays out
    through today: the terminal's records lay out through its `Tree` and keep only
    their cells; paragraphs and inline boxes lay out as the terminal's do, where
    headless measures each run on its own; a run's font size inherits through the
    typography, where headless reads the run's own or the platform's; headless lays out
    at the values motion presents, hit-tests a point to a path, and renders pixel
    snapshots in the pinned font; the layout store becomes the crate the native shells
    call into.
    - Conformance by intent: once the shells stop sharing views, a scenario states what
    it builds (a button labelled Save that counts its clicks) and each platform's
    driver builds it in its own vocabulary, so one suite keeps holding every shell to
    the foundation's behaviors; headless tests the foundation and an app's shared
    logic, and a shell is tested end to end on its own platform.
3. **The quadratic hot paths.**
    - The terminal: a structure change, a token's redefinition and a container's resize
    still restyle every node, and a structure change hands every node its layout
    children afresh, where each should reach only the subtree it touched (per-scope
    token and per-container query invalidation, `em` from the node's own font size);
    a hit test and a partial repaint still visit every child of a box, 0.3 ms for a
    pointer move over 16k text rows where an unchanged frame costs 21 µs (`cargo bench
    -p skene-terminal --bench frame`); selection spans compute once per frame; paint, hit
    tests and the selection wrap a run's text afresh where the layout's measure keeps
    its shape; clusters re-segment around an edit, and undo stores range diffs.
    - Token caches take per-scope epochs in place of the generations every
    redefinition, structure change and environment change moves for every scope today,
    and interned names, so a palette change costs the number of tokens beyond the web
    too, in the spine's `ResolutionContext`, which headless and the terminal share; a
    resolution reports the environment it read as it reports the interaction, so a
    resize restyles only what reads the viewport.
    - Constant factors: the lazy list reading a data source (a count and the item at an
    index) through a Fenwick tree, and evicting measured extents; one constructable
    stylesheet with deferred collection and a set of adopted names; release web output
    precompressed, fingerprinted and preloaded, a split build's modules through wasm-opt
    as an unsplit bundle is, with one cached stylesheet shared by static pages instead
    of inlined into each; a bundle budget the gate holds, a release `counter` after
    `wasm-bindgen` having grown from 911 KB to 1.27 MB between 2026-09-15 and
    2026-10-02, `skene-web` and `skene-foundation` most of it.
4. **The dev reload costs production nothing.** The saved tier is no second API: the
    Android leaf records and writes this snapshot (the native family), and the web and
    the desktop keep none beyond the dev reload.
    - Identity: typed owner steps aligned per parent, so a `Visibility` inserted before a
    component stops resetting everything after it; keyed roots for detached owners and `root_owner`; a
    positional scope per keyed list; grouping by live owner in place of the lossy
    dedup; a tagged match leaving the untagged cursor alone; node positions counted
    apart from signal positions; in debug, call-site hints aligned first and every
    restore decision logged; the bookkeeping in a side table off the owner records.
    - Encoding: a header (magic, format, app and build id); a bridge crate that
    snapshots a foreign type through serde instead of forcing it `transient`.
    - One serialization for state crossing a boundary: hydration carries through
    `Snapshot` and serde_json leaves the web leaf, or `DESIGN.md` states that network
    data is serde's.
    - Restoration in the spine: focus, selection with its direction, and scroll restore by
    snapshot position through platform methods, testable in headless and shared by the
    web and the terminal; focus following a keyed item across a reorder on the next run,
    which the terminal and the browser tests check on their own today, then becomes a
    conformance scenario, a `Driver` opening a page on a next run with the snapshot the
    last one took.
    - The terminal's swap: the launcher owns the state file, keeps it on an abnormal
    exit and respawns on the next build, so a startup panic after an edit keeps the
    state; descriptors are close-on-exec except across the swap, and the dev variables
    are unset for children; the app snapshots before restoring the terminal and stages
    before install; a second `swap` in one read coalesces; each generation's binary is
    kept for a rollback with its state.
    - The web: a staged snapshot boots a client render instead of a hydration; storage
    is keyed per app and tab; the `pagehide` session-storage copy restores on reloads
    alone and is cleared at boot.
5. **The app layer before 1.0.** Every surveyed framework treats these as table
    stakes, and each is cheaper to design before the native family than after it.
    - Forms: text-entry purposes (password, email, number, search, tel, url) and
    autocomplete as reported properties on `ControlProps`, mapped on the web to `type`,
    `autocomplete`, `inputmode` and `enterkeyhint`, a password entry secure in the
    terminal; `set_value` written as a contract, a no-op when equal and never while an
    IME composes; a `Form` view emitting a real `<form>` with field names and typed
    buttons, its GET submission a route with a query, so constraints such as `required`
    hold without script; `Action<I, O>` with pending, error and an optimistic overlay;
    a `Validate` trait feeding `Invalid` and `:user-invalid`.
    - Images and assets: `asset!()` handles checked at compile time, fingerprinted and
    scoped by the crate's own name, in place of hand-written URIs;
    `Image::new(asset).set_description(..)`, its description the floor where pixels
    don't show, as `Painted`'s is; a font-family property and token. `skene.toml`
    declares assets today, served in dev, copied into static output and collected
    across the dependency graph; it gains further settings as they earn their place
    (Flutter's pubspec is the model). Font assets add generated metadata—per-face
    ascender, descender, line gap, cap height, read at build time—so the web can
    emulate text-box trimming where an engine lacks it and native and the terminal lay
    text out with the same numbers.
    - Manifests: a `schema` key and forward-compatible parsing, strict for the app and
    lenient for dependencies, so a library published against a newer skene no longer
    breaks an older CLI; `[app] id, name, icon, links, locales` and the per-OS tables
    reserved and validated now, so libraries can start declaring. Whether
    `[package.metadata.skene]` replaces `skene.toml` is settled before libraries
    declare.
    - Routing: `#[derive(Route)]` with params and a query, guaranteeing
    `parse(to_path(r)) == r`; paths decoded alike on every leaf; a stack with push, pop
    and `replace`, and a `Back` event dispatched like `ContextMenu`; an unparseable
    `popstate` handled; per-entry scroll and state on the snapshot's positions; a
    pending signal, the commit waiting until code and data settle; preloading and
    `aria-current`; `not_found()` and `redirect()` setting a status on `Rendered`, and
    a `404.html` from static generation; a keep-alive policy for `OneOf` that pauses
    detached pages and bounds them LRU.
    - Async that holds: `Suspense` shows its fallback until the first settle alone,
    later work raising `Boundary::is_pending()` while the content stays, with a
    fallback delay and a minimum duration; a client-only load started while hydrating
    stays out of the boundary; `Resource::derive(source, load)` splits the value from
    its load state, with `mutate` and an error state, and `reload()` re-runs readers
    once; `Result` renders as a view that reports its error, feeding `ErrorBoundary`;
    an opt-in keyed `Query<K, T>` cache lives in userland beside `skene-transport`.
    - State: a Copy, erased `Field<T>`, so store lenses leave signatures;
    `#[derive(Store)]` on defaulted generics; a bounded, keyed state holder that
    records a disposed keyed owner's snapshot entries and stages them when the key
    returns, so scrolled-out items and navigated-away pages keep their state.
6. **Names and structure while the API is unstable.** One rename wave brings the names
    in line with the vocabulary rule, the structure loses its bad edges and copies, and
    the documents shrink to what tooling cannot enforce.
    - A shell per platform: the facades' views and Simple's, newtypes implementing the
    traits of the core beneath them, stay on probation against writing each platform's
    views by hand.
    - Manifests: wgpu 30 once vello_hybrid moves past the 29 it pins; the duplicate syn
    (serde's and clap's derives on 2, thiserror's on 3) and hashbrown versions
    (wasm-split-cli's walrus 0.23 beside wasm-bindgen's 0.26) gone as their dependents
    converge. Criterion stays at 0.5 while 0.8 builds alloca's C on every unix target,
    which the Apple clippy check cannot cross-compile on Linux; base64 stays doubled
    while sfv 0.15 holds 0.22.
    - The documents: `AGENTS.md` at 150 lines or fewer of numbered one-to-three-sentence
    invariants, each linking its `DESIGN.md` section; `DESIGN.md` one chapter per
    mechanism, each with its contract, mechanism and decisions, the terminal's split by
    concern; fine facts (DECRQM modes, OSC numbers, keyboard tables, hydration markers)
    in the owning module's rustdoc; `README.md` for users; the guide's book built by
    `mdbook` in the gate, so a broken link or chapter fails it.
    - Doc comments: contracts alone, without rationale or narrated implementation.
7. **Pictures beyond direct kitty transmission.** Shared memory or a file where the
    protocol's query over that medium shows the terminal shares the host, sparing the
    base64 for every change; unicode placeholders (`U=1`) through tmux, which passes no
    APC; sixel (DA1 attribute 4: xterm, foot, WezTerm, mlterm, Windows Terminal, VTE;
    not kitty, not Ghostty), palette-bound and without ids or z-order, so a change
    repaints the box whole; iTerm2's OSC 1337 last, for iTerm2, WezTerm and mintty.
8. **Internationalization**: message catalogs, locale-aware formatting, reactive
    locale switching. Catalogs are provided as a context memo, the `Theme` pattern, so
    switching locale is one write; formatting follows ICU4X and MessageFormat 2; a
    direction signal lays right-to-left out through taffy over the logical edges; rich
    text becomes an annotated string, one run per paragraph with attribute ranges as
    Compose's `AnnotatedString`, fewer nodes and a natural target for translations.
9. **The rest of the missing vocabulary.** What an ordinary app reaches for after the
    table stakes, and what the native family needs before it realizes the vocabulary
    once.
    - Views and controls: switch, progress and tabs first, then a spinner, a combobox,
    date and time, a number field, a tree, a table, a sheet, an alert dialog, a toast,
    a toggle button, a disclosure, a toolbar and a menu bar, on the open role
    vocabulary and `Collection<Policy>`.
    - Motion: `set_enter` and `set_exit`, compiled to `@starting-style` with a deferred
    detach on the web, and springs, compiled to CSS `linear()`.
    - Scrolling: a `ScrollHandle` with `scroll_to` and scroll-into-view;
    `LazyColumn::scroll_to_item` and an index API; lazy grids and sticky headers.
    - Style: opacity and transforms as compositor-friendly properties of their own,
    gradients, box shadows, italic, text alignment, ellipsis and a maximum line count,
    and system colors, shared with stated terminal degradations and the web-only ones
    in `WebStyle`.
10. **The terminal's app shell.** Inline mode and declarative terminal modes beside the
    alternate screen the app always takes today; printing above the app; `exec` and
    `suspend`; log capture, to a file or over the attach pipe, since the terminal
    installs no `tracing` subscriber; a screen-reader launch mode mapping the roles
    controls already report to a linear output, as Ink does; and further `Device`s the
    loop runs on through `Launch::run_on`: an SSH session's channel, and xterm.js in a
    page over a socket.
11. **Devtools on a tee platform.** A `Tee<P>` platform streams every create, attach,
    update and token definition, with owners, handlers, snapshot positions and the
    cause of each effect re-run, over one protocol from any platform; the inspector is
    written in Skene and runs in a second terminal or a browser tab, since the
    platforms' own inspectors show the sink tree alone. The same stream records and
    replays golden tests. Beneath it a data-only diagnostics tier names signals and
    emits re-run causes as `tracing` spans, and debug builds warn on a signal read
    while building a view, which freezes it, and on a write inside a memo.
    - Undecided: one terminal for the terminal's dev loop, the viewer hosted in the app.
    `skene run terminal` runs the app in its own terminal as the launcher `attach` is
    today, building beside it and swapping each binary in place; cargo's output is
    captured, a failed build reaching the app as its notice and, while no instance runs
    (a panic at startup), printed on the restored terminal until the next build respawns
    it. The terminal hosts this item's viewer in-process, as a panel docked beside the
    app and toggled by a `Launch` key as the mouse key is, the stream's first events
    being the logs (the dev build's subscriber capturing them), the build's notices and
    the last crash, the inspector following; a tab or a window of its own hosts the same
    viewer on the web and native. The panel lives in a scene of its own, so the app's
    snapshot, focus, accessibility tree and tests never see it; docking shrinks the
    app's viewport as a browser's docked devtools do; and it compiles only under the dev
    cfg the CLI sets, so a production build carries none of it. Open: the key (`F12`, as
    browsers use), whether `attach` stays as the two-terminal form, and whether the
    first cut is the logs, the build and the last crash alone.
12. **Themes split by platform, and design tokens in the DTCG format.** A theme's
    core is plain constants shared by every shell (its colors, and its sizes named
    rather than measured, since cells, points, dp and pixels share no unit), and each
    platform's theme crate gives the named sizes their values and builds its own
    themes from the core. Open, and revisited once the shells are split: whether the
    shells other than the web keep token sets or theme through reactive values
    throughout their trees, the web keeping them so a scriptless page themes through
    CSS. A core imports from and exports to the W3C Design Tokens Community Group
    format, stable since October 2025, so a palette travels to and from design tools.
13. **Notifications and progress from the terminal**: OSC 99 (kitty, with ids and
    closing) or OSC 9 for a desktop notification, OSC 9;4 for the taskbar's progress
    (Windows Terminal, ConEmu, Ghostty); a facade seam beside the title, ignored where
    unsupported. Low value until an app asks.
14. **The terminal runs on Windows.** The `tty` module has a unix layer alone; the
    console layer (ConPTY's virtual terminal input, which helix found buggy with CJK
    and non-ASCII text, or the legacy console input records as the fallback) is
    unwritten, and no Windows machine is at hand; the item waits for one. `skene
    attach` waits with it: the control pipe an app inherits across its swap exists on
    Unix alone, and where a process cannot be replaced the app exits with status 75
    for the launcher to spawn it. The console is a `Device` beside the tty, the loop
    unchanged: its input as bytes, its resize and end requests as notes.
15. **The slider's standard parts on the web.** The web slider is drawn, so nothing
    waits on them, but when engines ship `::slider-fill-*` (csswg-drafts#12419) the
    native input's track can be restyled through them instead. Blocked on engines.
16. **A socket's send wakes its thread.** The host WebSocket's thread writes what was
    queued between reads of 10 ms; a wake pipe polled beside the socket would send at
    once and let an idle socket sleep.
17. **The native platform family.** The largest phase, roughly 21–30 person-months
    across five OSes, and deliberately late: each shell is designed once against a
    complete foundation, instead of chasing additions, so the gaps (images, font
    families, text-entry purposes, opacity, transforms, insets, windows, drag and drop)
    close first, in the items above and the prerequisites below.
    - Premise: **native views, native compositing, native text.** Each OS is a shell of
    its own (`skene-ios`, `skene-android`, `skene-windows`, `skene-macos`,
    `skene-linux`), its crate its facade, with a vocabulary for that OS's controls,
    structure and navigation. Every platform
    realizes the tree into its own view hierarchy and composites it: a box is a view
    with its layers, a text entry is the platform's (`UITextField`, `EditText`,
    `GtkText`, a XAML `TextBox`) styled by the platform's Simple crate as an `<input>`
    is on the web,
    since selection, IME, autofill and the edit menu are the platform's and mobile
    users notice a replica, a `Native*` control is the platform's own where it has one
    beside the composed `Basic*` and `Simple*` forms, and only `Painted` draws, in a
    layer-backed view. A `Native*` control takes a small common option set applied
    best-effort (a plain `<input>` on the web) and a per-instance hook handing over the
    platform's own control for everything outside it, so nobody forks a control to set
    one property. Native text entries, menus and clipboard, scroll views with their
    physics, and accessibility are non-negotiable on every OS, and no realization is a
    hybrid of painted content around embedded native controls. Where Rust cannot reach
    the toolkit the shim is the platform's language: Kotlin on Android, the tree
    written in batches over JNI; objc2 reaches AppKit and UIKit directly; windows-rs
    reaches WinUI 3 directly, with no C++/WinRT shim and, through self-contained
    deployment, no Win32 floor; Linux is GTK4 through gtk4-rs. On Windows every run is
    a WinUI text element, so text keeps the platform's subpixel rendering.
    - Prerequisites, taken first, each also improving the web or the terminal:
    incremental resolution in the runtime kit (the tests and hot-paths items); the
    `Environment` per window, a scene per root in place of one per platform, and a
    `Window` concept beside them, the web and the terminal having one window each, so
    the leaves are built for several and embedding into an existing native screen falls
    out; the typed events, with a platform Back request routed to overlays and the
    router; menus and text tooltips as data, through a default-bodied method taking
    `MenuEntry`s that NSMenu, UIMenu, PopupMenu, MenuFlyout and PopoverMenu realize,
    since a drawn Paste triggers iOS's paste prompt and macOS 26's pasteboard alert;
    text-entry purposes and the `set_value` contract; the missing vocabulary; the
    executor wake hook; selection by `target_os`; and a native counterpart to "mimic
    the web", following the platform wherever it has a convention (macOS Tab behavior,
    iPad focus groups, Android Back, long-press menus).
    - The first proof: GTK4 native views, spiked locally for two to three weeks against
    a web-versus-native conformance gallery before Apple and Android are committed to.
    No painted realization is spiked beside it, since a painted leaf meets the
    non-negotiables only as the hybrid ruled out.
    - Apple: native views, layer-backed; one `NSAttributedString` per paragraph
    measured with TextKit, inline views through `NSTextAttachmentViewProvider`; an
    overlay host view, a child `NSPanel` for popups that leave the window, context
    menus once menus are data, a generated default Edit menu. Insets are missing from
    the vocabulary, text tooltips are impossible on touch, Tab on macOS reaches text
    boxes alone unless keyboard navigation is on, and a Mac is needed.
    - Android: Views through the Kotlin shim, one JNI call per frame carrying a byte
    buffer of operations and frames applied in `onLayout`, snapped to device pixels, the
    scene handing each changed parent's real children to the platform in one call at the
    end of a flush where it hands each child's attach, move, hide or removal today; an
    `EditText` that never rewrites a composing span; `OnBackInvokedCallback` for
    predictive Back; touch mode as focus-visible; `configChanges` including `uiMode`;
    edge-to-edge from API 35. Views are in maintenance mode since May 2026, and remain
    the one path where text entry, IME, autofill, selection handles, scroll physics and
    accessibility all come from the platform.
    - Windows: WinUI 3, with composable subclassing (a custom panel, an automation
    peer) proven first; `TextBlock` and `RichTextBlock` inlines, `TextBox` with TSF,
    `MenuFlyout`, `FocusState` as focus-visible, composition animations off the UI
    thread. Contrast themes swap app colors for system colors, which literal palettes
    break, and Windows hardware is needed.
    - Linux: GTK4 through gtk4-rs 0.11, a subclassed container with a taffy
    `LayoutManager`, `gtk::Text` and `TextView`, Pango-attributed labels, popovers,
    gesture controllers and AT-SPI; developable on the machine at hand. The drawn Linux
    leaf leaves the documents.
    - Layout: the platform's own layout containers for its native look, and beside them
    taffy behind a small FFI library the platform's own layout primitives call (a
    `SkeneLayout` view on Android and iOS, a `LayoutManager` on GTK, a panel on XAML),
    one tree per root rather than per container, crossed once per layout pass, leaves
    measured by the platform's own text and controls through a call on the UI thread,
    frames set in one batch; taffy's containers keep taffy's semantics on every
    platform, for an app that looks its own everywhere. Layout-only nodes
    flatten by type, since only `Container` is decorated; layout settles before a frame
    commits, the terminal's settle loop becoming the frame contract; text measurement
    is memoized in Rust by text, style and constraint, since each question would cross
    JNI or Objective-C; taffy nodes persist; `Painted` resizes land with the layout
    commit.
    - Shared machinery beside it: vello paint for `Painted`, AccessKit only where a
    painted layer needs it since native views carry their own accessibility, the frame
    clock running `skene_foundation::interpolate` as the terminal does, `Condition`
    resolution against window and pointer signals through the one resolution point.
    Opacity, transform and color transitions run on the compositor, the frame clock
    their floor.
    - Selection: an app's shell depends on its OS's crate, which builds for that OS's
    triples alone, so no cfg names the platform; a shell is tested end to end on its
    own OS, and what the shells share is tested headless.
    - The `PlatformView` escape trait and per-leaf embed constructors
    (`ios::native_view`, …): real text fields, maps, web views.
    - Libraries: a low-level view library implements its host trait for each native
    platform type, as for the web and headless; one with Kotlin or Swift sources and
    Gradle or SwiftPM dependencies keeps them in per-OS crates that declare them in
    `skene.toml`, and Skene hands it the Activity, the JavaVM and class loader, the UI
    thread, a batched update path and a generated plugin registrant for lifecycle
    callbacks; platform APIs without UI are plain `target_os` crates, the leaves
    committing to `ndk-context` and `raw-window-handle` as their context seams, with
    results through `spawn` and `Resource`, lifecycle hooks through the registrant and
    fakes through context in tests.
    - Manifests: `skene.toml` collects per target; declarations are keyed by OS, never
    by Skene platform; the app overrides or removes anything, a library's strings being
    defaults; secrets are placeholders validated before any native build; every build
    writes a provenance report; build scripts never run Gradle, Xcode or SwiftPM, so
    `cargo check --target aarch64-apple-ios` works on Linux CI.
    - `skene build`/`run` for `ios`, `android` and the desktop shells, landing under
    `dist/<target>`.
    - Build and run beside it: `skene test`, `skene devices`, `skene doctor` and
    `skene eject`, for every target from `web` and `terminal` to `macos`, `windows` and
    `linux`; native projects generated and disposable in `target/skene/<target>`, so
    each year's platform requirements land once in Skene's templates, and ejected when
    a team must own one; signing and packaging (Play App Signing and app bundles,
    automatic signing or `fastlane match`, cargo-packager, MSIX); density variants,
    per-platform fonts, localization catalogs, icons and splash screens from the assets
    and `[app]`; `[app] links` generating App Links, associated domains and both
    `.well-known` files in the static web output; `cargo check --target` for every OS
    triple on Linux, and `skene test` on emulators, simulators and devices.
    - The dev reload on devices: desktop reuses the terminal's `execve` launcher;
    Android connects over `adb reverse`, writes its snapshot to app storage on `swap`,
    reinstalls with `adb install -r` and restores at boot; iOS does the same through
    `simctl` and `devicectl`; a reload takes seconds.
    - Android's process death: the leaf writes the existing snapshot on backgrounding
    through a lifecycle hook and restores it at launch, under the header's build id,
    secure entries excluded; no new API.
    - Per-platform code has one home each: a capability difference is a signal and
    `Enhance`, never a cfg; a realization difference lives in a library's per-platform
    implementation; an OS service is a `target_os` crate; a product choice prefers a
    library, else a per-OS module at the app's edge; app-owned native code is declared
    in the app's `skene.toml`, as a library's is.
    - View transitions: snapshot the outgoing tree into a layer and interpolate named
    pairs.
    - Off-thread delivery: the engine's scheduler wired to the event-loop proxy.
    - A loop-integrated timer (the one sanctioned exception to timers-are-userland).
    - Per-OS glue in leaves: IME, clipboard, menus, window management.
18. **Server rendering beyond the native binary.** The native server binary stays the
    deployment, documented in a container (a Dockerfile and docs for Lambda, Cloud Run,
    Fly.io, Cloudflare Containers and Vercel's Rust runtime), and the WASI leaf is not
    restored. Nearly all the demand for wasm rendering is on JavaScript isolates
    (Cloudflare Workers, Deno Deploy, Netlify and Supabase edge, Node, Bun), where the
    server leaf already renders on `wasm32-unknown-unknown` after two small patches;
    four cheap steps keep that build cheap to add, and the isolate leaf itself (an
    async settle loop, a JavaScript timer and clock, exports returning a `Promise` and
    a `ReadableStream`, a per-isolate render queue, a fourth test tier under Node;
    600–900 lines) waits until someone asks.
    - The render deadline through a clock that works on every target, `Instant`
    natively and the JavaScript clock on `wasm32-unknown-unknown`.
    - `#[main]` never emitting the browser entry in a server build.
    - All blocking behind one seam: the settle loop async, with a native `block_on`
    over today's condition variable, and `AGENTS.md` stating that the server leaf
    blocks only there.
    - `skene-transport`'s web path calling the global `fetch` rather than
    `window().fetch`, which also serves Web Workers.

Watched, not scheduled:

- `aria-activedescendant` for the listbox, the menu and the picker's list: focus moves
    into the options today, the roving pattern, which assistive technology follows;
    the composite pattern keeps focus on the container and needs a node-valued report
    and stable per-node ids in the web platform. Worth it only if a screen reader
    stumbles on the roving form.
- A scrolling box as a tab stop in the terminal: the web gives one `tabindex="0"` so
    the keyboard reaches its content, as Chromium does; the terminal's scroll boxes
    are reached by the wheel and the page keys from a focused child alone.
- A reload the reader makes (F5) keeps the page's state through session storage as a
    best effort, under its cap; the origin private file system carries it whole only
    on the dev server's reload message, where the write is awaited.

- `ErrorBoundary` stays on probation: it exists only for errors with no render site of
    their own. If per-resource matching keeps covering real apps, delete it.
- A snapshot key is matched by a 127-bit hash of its value, so two values hashing alike
    share one scope and their state crosses on a restore; a collision check at every
    build was judged not worth its cost. If the hash ever proves short, the route is a
    keyed step carrying the value's postcard bytes and a type hash, at a
    `Serialize + DeserializeOwned` bound on every key and a heap step per keyed scope.
- The hybrid renderer also runs on WebGL2 (its `webgl` feature, no wgpu) for engines
    without WebGPU; not built, since the CPU path already covers them.
- Atomic CSS, interning single declarations as StyleX does, and marker-less hydration,
    build order equal to document order so hydration walks siblings: each measured on
    the gallery before it is taken.
- Building in place (`View::build(self, parent, before)`), so static children need no
    segment: measured first, since it saves about one comment pair per layout view.
- Merging the owner tree into the scene, every owner a scene node, disposal `destroy`
    and context lookup an ancestry walk: once the ownership fixes land.
- One cascade IR, ordered rules of zero specificity that the web prints as CSS and the
    runtime platforms evaluate: once the parity harness exists.
- Reactive painting: the painter in an effect with size and pixel ratio as signals, and
    fixed-size painted boxes rasterized to `<img>` on the server, so charts work
    without script; it pairs with the image view.
- A terminal writer thread with latest-frame-wins backpressure, and hardware scrolling
    (DECSTBM with SU and SD), so a scroll over ssh costs one row: for ssh-heavy use.
- A userland render pool beside `skene-transport`, N `--serve` workers with timeouts,
    respawn and backpressure behind one function a Rust server calls: once someone
    deploys server rendering.
- Out-of-order streaming: each deferred boundary streams as soon as it settles rather
    than in document order, behind a switch on by default that an app may turn off,
    once HTML's declarative out-of-order patching (`<template for>`) is Baseline
    widely available.

## The terminal's enhancements, as a whole

Every item above that asks the terminal for a capability—the further picture
transports and notifications—shares one rule with those that shipped (synchronized
output, pointer shapes, in-band resize, underlines, the keyboard flags, grapheme
clustering, the pixel mouse, text sizing, the graphics layer with the decoration on
it, the palette read back, the answering clipboard). A plain emulator is the floor and stays whole: every capability is asked for at launch,
confirmed by a parsed reply or ignored in silence, and degrades to the cell rendering
that exists today. None becomes a requirement, a type bound or a launch setting a
terminal must meet, and none is inferred from a name (XTVERSION, `TERM`), since a name
is not a capability. Multiplexers are part of the floor: tmux answers most queries
itself, passes graphics only as unicode placeholders and passes no pixel mouse, so
every item is proven through tmux and ssh as well as bare.
