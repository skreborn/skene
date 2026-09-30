# Roadmap

What remains, in the order to take it. The architecture is in `DESIGN.md`, the doctrine
in `AGENTS.md`; this file records neither, only the work that is open and the shape it
lands in. What has shipped is not listed: the code, `DESIGN.md` and the tests describe
it.

The reactive engine's baseline, `cargo bench -p skene-reactive --bench engine` on the
development machine (2026-09-30), against which a change to the engine is measured:
creating a thousand signals with nothing recording snapshots 51 µs; a signal read
outside an effect 7.0 ns; a write reaching ten effects 299 ns; a write through a chain
of ten memos 551 ns; a store row's edit among a hundred tracked rows 185 ns, and among
ten thousand 162 ns, and by key 232 ns and 230 ns; a row pushed among a hundred 231 ns,
and among ten thousand 6.5 µs. Versions compare only when each is built in its own
target directory (a shared one reuses another tree's build) and the bench binaries run
alternately, pinned to one of the fast cores with address randomization off:
`setarch -R taskset -c 2 <binary> --bench`.
Callgrind's instruction counts compare two builds whatever the machine's load, cache
misses aside: `valgrind --tool=callgrind <binary>`, built with
`CARGO_PROFILE_RELEASE_STRIP=none` so the profile names its functions.

## Open

One list, every item big and small, top first. An item is resolved by landing it, by
proving it unnecessary, or by recording it under the decisions at the end; it is never
left half-done in a code comment.

1. **A license and the first CI runs.** The workspace declares no license, so
    `cargo deny check licenses` has nothing to hold the dependencies to. Neither CI
    workflow, the gate on every pull request nor the weekly RustSec advisories, has run
    yet.
2. **One owner for style and interaction semantics.** The web compiler and core's
    `Resolver` implement one meaning twice, and the terminal, headless and the web each
    implement focus, press, hover and overlays; each mechanism gets one owner, and the
    browser is the oracle.
    - The parity harness (`browser_parity`) gains a case for each fix below.
    - Interaction: a group run's state variants reach its spans, as a rule on the
    group's class redefining the ink where the state holds; the token definitions it
    redefines condition on media alone, so this waits on the styling direction.
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
3. **Tests that exercise what users see.** Headless becomes the GUI's real stand-in,
    and every tier asserts through roles, text and pixels.
    - Undecided, before the kit below: the terminal measures lengths in logical pixels,
    a cell standing for a fixed 8×16 of them, rather than in whole cells. Every
    platform's scalar is then `f32` and the `Scalar` parameter leaves the spine (the
    styles, the geometry, tokens, conditions and the environment), a library's literal
    lengths and font sizes mean the same visual size on both facades (a 16 px font one
    cell tall, `em` one text row), and corner radii, shadows and the pointer share the
    GUI's measure. Layout snaps to whole cells per axis (taffy rounding in cell units),
    text and scroll offsets stay on the grid, and only the graphics layer draws between
    cells; the cell size the terminal reports scales graphics alone, so a font change
    keeps an app's cell counts. `Length::characters` and `Length::lines` (the web's
    `ch` and `lh`) spell cell-exact lengths on every medium. The cost: a bare `1.`
    rounds to no cell in the terminal, so its builders need a guard or a lint, and every
    terminal example and snapshot moves to the new unit.
    - The terminal's retained runtime (the taffy store, the incremental restyle, the
    damage and the measured rectangles, hit testing, focus, capture, wheel latching,
    field editing, selection, motion) is extracted into a medium-generic kit; headless
    lays out through it and renders pixel snapshots, and it is the native family's
    first prerequisite.
    - A host-side WebDriver harness in the browser tier, over the WebDriver session,
    runs the conformance suite (`skene-conformance`) in Firefox and Chromium through
    trusted input from WebDriver's actions and takes the screenshot goldens: an event a page
    dispatches to itself runs no browser default—no focus on a press, no button's click
    on `Enter`, no text in an entry—so neither can run inside the page. The suite's
    scenarios then name their pages, the harness serving each from one test app and
    finding what a scenario acts on by WebDriver's computed role and label; the goldens
    need a pinned font and Firefox before they compare across machines.
    - The behaviors each platform's tests check on their own (focus showing after a key
    and not after a press, a press held along its path, focus entering and leaving a
    subtree, focus following a keyed item across a reorder) move into the conformance
    suite once a `Driver` observes what they read.
4. **The quadratic hot paths.**
    - The terminal: a structure change, a token's redefinition and a container's resize
    still restyle every node, and a structure change hands every node its layout
    children afresh, where each should reach only the subtree it touched (per-scope
    token and per-container query invalidation, `em` from the node's own font size);
    a hit test and a partial repaint still visit every child of a box, 0.3 ms for a
    pointer move over 16k text rows where an unchanged frame costs 21 µs (`cargo bench
    -p skene-tui --bench frame`); selection spans compute once per frame; wrapped lines
    cache per text generation and width, clusters re-segment around an edit, and undo
    stores range diffs.
    - Token caches take per-scope epochs in place of the generations every
    redefinition, structure change and environment change moves for every scope today,
    and interned names, so a palette change costs the number of tokens beyond the web
    too, in core's `ResolutionContext`, which headless and the terminal share; a
    resolution reports the environment it read as it reports the interaction, so a
    resize restyles only what reads the viewport.
    - Constant factors: the UI-thread assertion in debug alone, the iterative pull and
    alien-signals' propagation techniques, estimated to halve a read and a write;
    forced inlining off allocating functions; the lazy list reading a
    data source (a count and the item at an index) through a Fenwick tree, and evicting
    measured extents;
    one constructable stylesheet with deferred collection and a set of adopted names;
    release web output through wasm-opt, without `.d.ts` files or name sections,
    precompressed, fingerprinted and preloaded, with one cached stylesheet shared by
    static pages instead of inlined into each.
5. **The dev reload costs production nothing.** The saved tier is no second API: the
    Android leaf records and writes this snapshot (the native family), and the web and
    the desktop keep none beyond the dev reload.
    - Identity: typed owner steps aligned per parent, so a `Show` inserted before a
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
    - Restoration in core: focus, selection with its direction, and scroll restore by
    snapshot position through platform methods, testable in headless and shared by the
    web and the terminal.
    - The terminal's swap: the launcher owns the state file, keeps it on an abnormal
    exit and respawns on the next build, so a startup panic after an edit keeps the
    state; descriptors are close-on-exec except across the swap, and the dev variables
    are unset for children; the app snapshots before restoring the terminal and stages
    before install; a second `swap` in one read coalesces; each generation's binary is
    kept for a rollback with its state.
    - The web: `skene run web --ssr` pages get the dev script, and a staged snapshot
    boots a client render instead of a hydration; storage is keyed per app and tab; the
    `pagehide` session-storage copy restores on reloads alone and is cleared at boot.
    - The dev loop: `skene.toml` reloads per rebuild; the watch follows the app's path
    packages or its dep-info file, not this repository's `packages/`; cargo's `fresh`
    flag gates wasm-bindgen, the splitter and the reload, instead of the whole debug
    binary being hashed after every build; steps build into a temporary directory
    renamed on success; `cargo skene` builds with a thin-LTO profile;
    `notify-debouncer-full` catches saves that rename into place; `mime_guess`
    completes the MIME table.
6. **The app layer before 1.0.** Every surveyed framework treats these as table
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
    reserved and validated now, so libraries can start declaring; a root `public`
    directory for `/favicon.ico`, `/robots.txt` and `/.well-known/`, and files tried
    before routes under `--ssr`. Whether `[package.metadata.skene]` replaces
    `skene.toml` is settled before libraries declare.
    - Routing: `#[derive(Route)]` with params and a query, guaranteeing
    `parse(to_path(r)) == r`; paths decoded alike on every leaf; a stack with push, pop
    and `replace`, and a `Back` event dispatched like `ContextMenu`; an unparseable
    `popstate` handled; per-entry scroll and state on the snapshot's positions; a
    pending signal, the commit waiting until code and data settle; preloading and
    `aria-current`; `not_found()` and `redirect()` setting a status on `Rendered`, and
    a `404.html` from static generation; a keep-alive policy for `Switch` that pauses
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
7. **Names and structure while the API is unstable.** One rename wave brings the names
    in line with the vocabulary rule, the structure loses its bad edges and copies, and
    the documents shrink to what tooling cannot enforce.
    - `skene-core` and `skene-common` merge into `skene-foundation`, since every
    consumer depends on both: scene internals stay behind module visibility
    (`pub(in crate::spine)`), so built-in views still use only the public API, and a
    check keeps the spine modules from importing the medium modules; `skene-reactive`
    stays separate, knowing nothing of UI. The facades stop re-exporting the spine as
    `core`; `AGENTS.md` and `DESIGN.md` follow; `split_alignment` and
    `ThemedIn::define_root` stop being crate-private. The resolution then names the
    typography size token itself, so `ResolutionSource::size_token` goes, the terminal
    keeping only whether it proved text sizing, and the `em` fallback becomes the
    medium's, unless the terminal's logical pixels make it one value.
    - Moves: `AsyncView`, `Deferred`, the hydrating `Resource`, `Link` and the router
    binding into the foundation over platform seams, so a generic library can use them
    and the terminal gains `Link`; the title into a shared `window` module, which grows
    into the `Window` handle, while web-only page metadata stays in a web-only module;
    `Show` beside `Switch`; the lazy list's `ItemWindow` math and the slider math out
    of the spine; `root_owner` as the reactive crate's `Owner::root()`; `task`,
    `executor` and `boundary` as one module.
    - Edges: `skene-simple` stops depending on both facades, as two tiny alias crates
    or an alias macro the app invokes; painting's GPU stack stops weighing
    on the build of an app that paints nothing, `vello_hybrid` being over half of a
    clean gallery wasm build.
    - `#[main]` is the only entry point, emitting `fn main` per target with its native
    arms reserved, and the CLI picks the crate type per target through
    `cargo rustc --crate-type`, so apps stop declaring crate types and the `cdylib`
    plus `rlib` pair, `__skene_main` and app-level cfgs go.
    - Renames: `Show` and `Switch` to `Visibility` and `OneOf`, where `Visibility`
    already names the style value that hides a box; the choice and activation handlers
    and binders to `choose`/`chosen` for value groups and `activate` for commands, with
    `bind_` documented as a builder prefix.
    - Vocabulary: `FlexDirection`'s reversed variants become an axis plus `reversed`;
    `Overflow` becomes `Clip`, `Scroll` and `ScrollIfNeeded`; `RunDisplay`,
    `GridTrack::Minmax` and `Layout::Flow` lose their CSS names; `Expr::vw`, `vh`,
    `cqw`, `cqh` and `em` become `viewport_width(0.5)` and its kin, fractions from 0 to
    1; `Role`'s HTML sections become ARIA's `Banner`, `ContentInfo`, `Complementary`
    and `Region`; `LinkAnnotation`'s `NoFollow`, `Sponsored` and `Ugc` and the opener
    and referrer policy move to the web escape; `Role::is_spelled` leaves the spine;
    `Inset` and `Spacing` take logical `start`/`end` and `horizontal`/`vertical` before
    internationalization makes that breaking; `set_background` ends `set_color` meaning
    the background on `Container`. With `RunDisplay` renamed, `display` joins the
    lint's banned words, which then read the names the `api/` snapshots list, re-exports
    included, rather than the sources' own items.
    - Shapes: one setter per property for `Flexible` (`set_flexibility(impl
    Into<Value<Flexibility>>)`), the slider (`set_range`, `derive_range`) and
    `Animation` (iterations, alternation, reduced motion); appending setters named
    `add_*` (`set_transition`, `set_animation`, `set_case`, `RichText::set_spans`);
    children last in `Deferred`, `AsyncView` and `Link`; fallbacks spelled one way across
    `Enhance`, `Painted`, `Suspense` and `ErrorBoundary`; one alignment name across
    `Stack`, `Grid`, `GridCell` and `Flexible`; the same prelude on both facades and
    every leaf.
    - Manifests: every crate publishable, with a description and a repository, the
    versioned dev-dependency cycles broken, and the CLI publishable, as the README's
    `cargo install` assumes; web-sys, js-sys, wasm-bindgen-test and serde_json
    inherited from the workspace; docs.rs building wasm32 and the server cfg; taffy
    0.14 (with right-to-left), wgpu 30, base64 0.23, toml 1.x, criterion 0.8 and
    wasm-bindgen 0.2.129, the duplicate syn and hashbrown versions gone.
    - The documents: `AGENTS.md` at 150 lines or fewer of numbered
    one-to-three-sentence invariants, each linking its `DESIGN.md` section; `DESIGN.md`
    one chapter per mechanism, each with its contract, mechanism and decisions, the
    terminal's split by concern; fine facts (DECRQM modes, OSC numbers, keyboard
    tables, hydration markers) in the owning module's rustdoc; `README.md` for users,
    its quickstart compiled headless through `include_str!` and the crate table drawn
    once; this file for open items alone, its benchmark baseline moved beside the
    benchmark and re-measured with the WebGPU split's sizes, since neither reproduces;
    a glossary for the overloaded words (leaf, anchor, store, record, realize, build;
    floor, fallback and baseline against upgrade, enhancement and child); dated
    decision records with a revisit trigger; an architecture page on state (a `Store`,
    a local `Signal` or context, and no view models in context); a guide in an mdbook
    that compiles its snippets, and a `skene new` scaffold.
    - Wrong claims corrected: the native contradictions in `DESIGN.md` (painted-native
    desktop and mobile, a Linux leaf on winit, selection by triple), settled by the
    native family's decisions; "a palette change costs the number of tokens", until the
    token caches change; "nothing on the path a built-in takes is crate-private"; the
    README's uncompiled counter snippet; the undocumented server `main.rs`.
    - Doc comments: contracts alone, without rationale or narrated implementation;
    stale references to vanished parameters and false contracts (`Flexible`, the
    tooltip, `split.rs`, the setter macro's `derive_` boilerplate) fixed; short first
    paragraphs; every builder's example a doctest on the headless host. Two rule
    tensions are settled explicitly: documented `pub(crate)` items against "comment
    non-public code only when critical", and examples that explain why, as teaching code
    should.
8. **Pictures beyond direct kitty transmission.** Shared memory or a file where the
    protocol's query over that medium shows the terminal shares the host, sparing the
    base64 for every change; unicode placeholders (`U=1`) through tmux, which passes no
    APC; sixel (DA1 attribute 4: xterm, foot, WezTerm, mlterm, Windows Terminal, VTE;
    not kitty, not Ghostty), palette-bound and without ids or z-order, so a change
    repaints the box whole; iTerm2's OSC 1337 last, for iTerm2, WezTerm and mintty.
9. **Internationalization**: message catalogs, locale-aware formatting, reactive
    locale switching. Catalogs are provided as a context memo, the `Theme` pattern, so
    switching locale is one write; formatting follows ICU4X and MessageFormat 2; a
    direction signal lays right-to-left out through taffy over the logical edges; rich
    text becomes an annotated string, one run per paragraph with attribute ranges as
    Compose's `AnnotatedString`, fewer nodes and a natural target for translations.
10. **The rest of the missing vocabulary.** What an ordinary app reaches for after the
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
    gradients, GUI shadows, italic, text alignment, ellipsis and a maximum line count,
    and system colors, shared with stated terminal degradations and the pixel-only ones
    in `GuiStyle`.
11. **The terminal's app shell.** Inline mode and declarative terminal modes beside the
    alternate screen the app always takes today; printing above the app; `exec` and
    `suspend`; log capture, to a file or over the attach pipe, since the terminal
    installs no `tracing` subscriber; a screen-reader launch mode mapping the roles
    controls already report to a linear output, as Ink does; and further `Device`s the
    loop runs on through `Launch::run_on`: an SSH session's channel, and xterm.js in a
    page over a socket.
12. **Devtools on a tee platform.** A `Tee<P>` platform streams every create, attach,
    update and token definition, with owners, handlers, snapshot positions and the
    cause of each effect re-run, over one protocol from any platform; the inspector is
    written in Skene and runs in a second terminal or a browser tab, since the
    platforms' own inspectors show the sink tree alone. The same stream records and
    replays golden tests. Beneath it a data-only diagnostics tier names signals and
    emits re-run causes as `tracing` spans, and debug builds warn on a signal read
    while building a view, which freezes it, and on a write inside a memo.
    - Undecided: one terminal for the terminal's dev loop, the viewer hosted in the app.
    `skene run tui` runs the app in its own terminal as the launcher `attach` is today,
    building beside it and swapping each binary in place; cargo's output is captured, a
    failed build reaching the app as its notice and, while no instance runs (a panic at
    startup), printed on the restored terminal until the next build respawns it. The
    terminal hosts this item's viewer in-process, as a panel docked beside the app and
    toggled by a `Launch` key as the mouse key is, the stream's first events being the
    logs (the dev build's subscriber capturing them), the build's notices and the last
    crash, the inspector following; a tab or a window of its own hosts the same viewer
    on the web and native. The panel lives in a scene of its own, so the app's snapshot,
    focus, accessibility tree and tests never see it; docking shrinks the app's viewport
    as a browser's docked devtools do; and it compiles only under the dev cfg the CLI
    sets, so a production build carries none of it. Open: the key (`F12`, as browsers
    use), whether `attach` stays as the two-terminal form, and whether the first cut is
    the logs, the build and the last crash alone.
13. **Design tokens in the DTCG format.** `TokenSet` definitions import from and export
    to the W3C Design Tokens Community Group format, stable since October 2025, so a
    palette travels to and from design tools.
14. **Notifications and progress from the terminal**: OSC 99 (kitty, with ids and
    closing) or OSC 9 for a desktop notification, OSC 9;4 for the taskbar's progress
    (Windows Terminal, ConEmu, Ghostty); a facade seam beside the title, ignored where
    unsupported. Low value until an app asks.
15. **The terminal runs on Windows.** The `tty` module has a unix layer alone; the
    console layer (ConPTY's virtual terminal input, which helix found buggy with CJK
    and non-ASCII text, or the legacy console input records as the fallback) is
    unwritten, and no Windows machine is at hand; the item waits for one. `skene
    attach` waits with it: the control pipe an app inherits across its swap exists on
    Unix alone, and where a process cannot be replaced the app exits with status 75
    for the launcher to spawn it. The console is a `Device` beside the tty, the loop
    unchanged: its input as bytes, its resize and end requests as notes.
16. **The slider's standard parts on the web.** The web slider is drawn, so nothing
    waits on them, but when engines ship `::slider-fill-*` (csswg-drafts#12419) the
    native input's track can be restyled through them instead. Blocked on engines.
17. **A socket's send wakes its thread.** The host WebSocket's thread writes what was
    queued between reads of 10 ms; a wake pipe polled beside the socket would send at
    once and let an idle socket sleep.
18. **The native platform family.** The largest phase, roughly 21–30 person-months
    across five OSes, and deliberately late: it realizes a complete vocabulary once,
    instead of chasing additions, so its gaps (images, font families, text-entry
    purposes, opacity, transforms, insets, windows, drag and drop) close first, in the
    items above and the prerequisites below.
    - Premise: **native views, native compositing, native text.** Every platform
    realizes the tree into its own view hierarchy and composites it: a box is a view
    with its layers, a text entry is the platform's (`UITextField`, `EditText`,
    `GtkText`, a XAML `TextBox`) styled to Simple's look as an `<input>` is on the web,
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
    - Layout: taffy behind a small FFI library the platform's own layout primitives
    call (a `SkeneLayout` view on Android and iOS, a `LayoutManager` on GTK, a panel on
    XAML), one tree per root rather than per container, leaves measured by the
    platform's own text and controls through a call on the UI thread, frames set in one
    batch; the vocabulary's semantics stay taffy's on every platform. Layout-only nodes
    flatten by type, since only `Container` is decorated; layout settles before a frame
    commits, the terminal's settle loop becoming the frame contract; text measurement
    is memoized in Rust by text, style and constraint, since each question would cross
    JNI or Objective-C; taffy nodes persist; `Painted` resizes land with the layout
    commit.
    - Shared machinery beside it: vello paint for `Painted`, AccessKit only where a
    painted layer needs it since native views carry their own accessibility, the frame
    clock running `skene_core::interpolate` as the terminal does, `Condition`
    resolution against window and pointer signals through the one resolution point.
    Opacity, transform and color transitions run on the compositor, the frame clock
    their floor.
    - Selection: iOS and Android select their leaf by `target_os`, their triples
    hosting no other platform; on a desktop triple `skene-gui` names a native platform
    through the build-set cfg the server leaf already uses (`skene_platform`), since
    every desktop triple is a host triple too, and the headless platform stays the
    default so `cargo test` keeps meaning what it means.
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
    - `skene-gui` resolving native triples to it; `skene build`/`run` for `ios`,
    `android` and the desktop leaves, landing under `dist/<target>`.
    - Build and run beside it: `skene test`, `skene devices`, `skene doctor` and
    `skene eject`, for every target from `web` and `tui` to `macos`, `windows` and
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
19. **Server rendering beyond the native binary.** The native server binary stays the
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

## Decided against, for the record

- Signals shared across threads, as a registry each runtime drains through an inbox:
  a handle stays on its thread, and another thread's work reaches it through a task
  that thread's executor wakes.
- Hot-patching: state-preserving reloads come from the snapshot instead.
- Per-platform view vocabularies (`skene_ios::View`, a Simple per platform): the
  generic `View<P>` is that with the platform as a parameter, coherence forbids two
  impls of one shared view distinguished by a platform's kind, and the `Native*`
  family beside `Basic*` is the split that was wanted.
- A capabilities model of typed slots on `Platform` in place of progressive
  enhancement: shelved, since it puts bounds on every shared signature; only a
  low-level view names what a platform can do.
- Draw-everything as the native family's default: mobile text selection, IME and edit
  menus are years of platform work no framework should redo.
- A hybrid native realization, painted content with native controls embedded in it:
  every embedded view is paid for, as Flutter's platform views show.
- A painted native platform, on the desktop or on Android through Linebender's
  android-view: it meets the native family's non-negotiables only as that hybrid, and
  like Compose it owns text editing, the part left to the platform.
- A WASI server leaf, under a wasmtime runner or as a `wasi:http` handler: it needs a
  second transport for every client, and the demand for wasm rendering is on JavaScript
  isolates. The server leaf compiles natively, selected by
  `--cfg skene_platform="server"`, so every crate on the host reaches a render; a wasm
  host is a runner someone may write, for pages that await nothing until the settle
  loop is async.
- Islands architecture: compiled no-JS styling already removes most reasons a static
  page ships wasm.
- LiveView-style server rendering: adjacent to the server-side non-goal.
- A webview desktop shell.
- A lazy split of the WebGPU renderer, measured 2026-09-15 on release builds after
  `wasm-bindgen`: an app that paints nothing (`counter`) ships 911 KB carrying 10.6 KB
  of wgpu code and no shader text, since the linker drops what no box reaches; an app
  that paints (`gallery`) ships 1.9 MB carrying 135 KB for the GPU path beside the
  271 KB CPU renderer it needs either way. Nothing worth a split.
- Rendering the whole terminal frame as one image: the text would then be ours to
  rasterize, and the terminal's font, selection, copy and accessibility would go with
  it, a poor GUI in place of a good terminal.
- Any terminal enhancement gated by a terminal's name.
- Inferring a terminal's depth: a terminal answering no DECRQSS paints at 16 colors,
  Terminal.app among them; the cost of inferring nothing, taken knowingly. Client hints
  are likewise Chromium-only (`Accept-CH`/`Critical-CH`), and other engines get the
  documented defaults.
- A vertical fractional edge in the terminal: only the lower block glyphs exist, so a
  top-anchored fill would need upper ones; a vertical fill ends on a whole cell.
- Moving an editor's caret off the cell past a full row, where a wrapping space was
  consumed: a browser's caret hangs past such a row into the textarea's padding, and
  the terminal's bar cursor in the cell past it, the padding or the border, shows the
  same position.
- Replacing `IntoChildren` and `IntoChildOf` with std traits, checked against the
  compiler: a blanket `impl<I: IntoIterator<Item = V>> IntoChildren for I` conflicts
  with the tuple impls, and the same conflict appears with `From`; tuples are not
  optional because children are heterogeneous.
- A collision check on snapshot keys at every build (see the watched item above).
- A `&'static str` signal restoring: it is `transient`, since it cannot be deserialized
  owned.
- A `HashMap` store field iterating in a stable order: its entries come in its
  iteration order; an ordered list wants a `BTreeMap` or a keyed `Vec`.
- Another reactive engine: Leptos's `reactive_graph` schedules effects asynchronously
  behind `Arc` and `RwLock`, breaking the synchronous render-first flush the terminal's
  key defaults rely on; `sycamore-reactive` has eager memos and no render flavor; the
  Rust alien-signals port has no owners, contexts or cleanups. Its techniques are
  ported instead.
- Another terminal parser, or one beneath it: crossterm parses no replies, termina
  lacks APC and re-parses byte by byte, vte has no APC dispatch, and termwiz is heavy.
- The `color` crate for `Color`: byte-identical class names across the server and the
  browser need `libm` math, which its `libm` feature cannot promise once peniko and
  vello unify it with `std`; its CSS semantics are taken instead.
- A view macro beside the builders (`rsx!`): a macro costs rustfmt and rust-analyzer,
  and Freya left rsx for builders in 2026; the effort goes into diagnostics.
- Collapsing the crates beyond `skene-core` and `skene-common`: separate crates compile
  in parallel.
- One app serving both media: the two galleries still differ on 431 of 893 lines once
  numbers are stripped.
- A stable plugin ABI: nobody asks for one.
- A property table giving each property an interpolation class and a unit class: a
  value's `Interpolate` answers per pair, as CSS computes it per pair (a length and
  `Auto` are discrete, two lengths are not), and the value's variant is its unit.
