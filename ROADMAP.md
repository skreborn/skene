# Roadmap

What remains, in the order to take it. The architecture is in `DESIGN.md`, the doctrine
in `AGENTS.md`, the decisions against in `DECISIONS.md`; this file records none of them,
only the work that is open and the shape it lands in. What has shipped is not listed:
the code, `DESIGN.md` and the tests describe it.

`AGENTS.md` states the doctrine of the redesign (two facades, `skene-gui` and
`skene-tui`, over one foundation; one platform per build; Skene's drawn controls and the
platform's own text on every platform; testing on the real platforms alone). Its first
part has landed: the web and the terminal stand on the reshaped foundation, one platform
per build, as the two facades over realization crates of their own, and app code names
no platform. The rest of the redesign follows this list, and until an item lands the
code and `DESIGN.md` still describe the design it replaces: where they disagree,
`AGENTS.md` says where the code is going and `DESIGN.md` where it is.

## Open

One list, every item big and small, top first. An item is resolved by landing it, by
proving it unnecessary, or by recording it in `DECISIONS.md`; it is never left half-done
in a code comment.

1. **One runtime beneath every platform but the web.** What the terminal keeps for
    itself, Linux and Windows need as well, since they too own their window, and the
    compositor platforms need its layout and its motion; each mechanism is written once.
    - The terminal's retained runtime beyond its layout tree and its restyle's schedule
    (the restyle's resolution caches and presentation, the damage and the measured
    rectangles, hit testing, focus, capture, wheel latching, selection, motion) moves
    into `skene-layout`, generic over the platform's scalar, beside the style mapping,
    the `Tree` and the `Restyle`, its frames hit through, clipped and scrolled as a
    browser's are; a paragraph is kept as one laid-out paragraph of its runs, which
    places them and their inline boxes for the frames, a paragraph's document selection
    and its text content kept once above the engine, in the runtime, where each
    realization keeps them today. A drawn entry's key default is written once there
    too, over the shared entry the scene keeps: the core's `Binding` table, run after
    the app's key handlers as a browser's `<input>` default runs (so never as a
    `TextEntry::on_key`, which would run before them), the terminal's own key default
    consulting the table until then.
    - Layout: taffy owns it on every platform but the web, one tree per container, the
    platform's measure as its leaves, a minimum as a floor and fractional rectangles;
    layout-only nodes flatten by type, since only `Container` is decorated; layout
    settles before a frame commits, the terminal's settle loop becoming the frame
    contract; taffy nodes persist, and `Painted` resizes land with the layout commit.
    `Tree::lay_out_revisited` (a walk of the scene revisiting the moved nodes alone) and
    `TextFlow::trimmed` (a text trimmed as `text-box-trim` trims it), which no platform
    calls, become that runtime's or leave.
    - Beside it: vello paint for `Painted`; the frame clock running
    `skene_foundation::interpolate`, with opacity, transform and color transitions on
    the platform's compositor where it has one and the frame clock their floor;
    `Condition` resolution against window and pointer signals through the one resolution
    point; the `Environment` per window, a scene per root and a `Window` beside them,
    the web and the terminal having one window each, so embedding into an existing
    native screen falls out; typed events, a platform's Back request routed to overlays
    and the router; the executor's wake hook and off-thread delivery through the event
    loop's proxy; a loop-integrated timer, the one sanctioned exception to
    timers-are-userland; view transitions snapshotting the outgoing tree into a layer
    and interpolating named pairs; and a native counterpart to "mimic the web",
    following each platform's conventions (macOS Tab, iPad focus groups, Android Back,
    long-press menus).
2. **Lists and overlays, designed for both kinds of platform.** Every realization below
    needs both, each shaped once for the compositor platforms, which place views among
    native ones, and for the window platforms, which draw everything. Lists are designed
    (`spikes/linux/design/lists.md`: `LazyColumn`, `LazyRow`, the lazy grids and
    `Listbox<C>` over one windowing engine, with how iOS, Android, macOS, the web and the
    terminal realize it) and realized on Linux; overlays (popovers, dialogs, menus) are
    not designed beyond the prototype's popup subset.
    - Lists: a listbox choosing several, `Chosen` for `HashSet<T>` and `BTreeSet<T>`
    beside today's `Option<T>`, the multi-select `Listbox<C>`: a behavior of its own
    over the collection's erased choice (`AnyChosen` keeps its type's rules), with the
    ranges and the select-all a reader expects and a scriptless form of its own (a
    `<select multiple>`, or a group of checkboxes).
    - One candidate the design weighs: menus and text tooltips as data, a list of
    `MenuEntry`s that `NSMenu`, `UIMenu` and `PopupMenu` would realize where the
    platform composites views, since a drawn Paste triggers iOS's paste prompt and
    macOS 26's pasteboard alert.
    - A context menu's `set_placement` places nothing on any platform: the list is a
    pinned overlay, and its `Placement` content has a pinned overlay ignore its
    placement, so the web and the terminal both open the list's top-start corner at the
    pointer whatever the menu says; the design places a pinned overlay against its
    point, or the setter goes, and its test returns where a platform passes it.
    - On the web an overlay placed against the viewport keeps the margins its first
    `Placement` content compiled into its class: a derived placement re-places an
    anchored or pinned popup as it opens, and the terminal any overlay at its next
    frame, but the web moves a viewport-placed one nowhere, and the inline position an
    anchored or pinned opening writes stays once it is neither; the design writes the
    margins per placement, as a rule of their own or inline, so every change of the
    content shows.
3. **Linux, drawn on its own protocols.** A window platform: Skene owns the whole
    window, draws everything and speaks Linux's text, input and accessibility protocols
    itself, the GTK prototype remaining a reference whose transcripts are the bar. The
    drawn prototype (`spikes/linux`) realizes the whole on X11, mutter, sway, weston and
    KWin, on the latest release of every package it uses but those `spikes/linux` holds
    with a reason (zbus 6, skrifa, harfrust and read-fonts behind Parley 0.12's pins, wesl,
    generic-array): it starts in 34–49 ms where GTK takes 297–371, a one-line change to
    an app rebuilds in about a second (the facade compiling its views once, an app's
    crate 923 functions where it was 8,661), a screen reader's text queries meet no error
    where `GtkEntry`'s meet five, its stripped release binary is 17.4 MB with no GTK in
    it, and its full `run.sh` passes in 75 minutes. The defaults it took that nothing settled are in
    `spikes/linux/DEFAULTS.md`, its designs in `spikes/linux/design`, and its reports for
    upstream, unfiled, in `spikes/linux/upstream`. What it leaves open:
    - The seam: who lays a realization out (`LayoutOwner`: the shared engine or the
    toolkit) and whether it draws a scroll box's bars from the box's parts, associated
    items of `Realization` landing with this realization, their first reader, since the
    web's layout is CSS and the terminal maps its own taffy tree and draws its bars from
    `TerminalStyle`.
    - The first frame: 1000 eager rows take 64.5–73.6 ms (layout 17.3–19.7 ms) against a
    target of 50 (16), shaping 3,000 paragraphs being 37–45 ms of it, while a lazy list of
    1,000 to 100,000 rows draws in 12.7–22.8 ms. What remains is cheaper shaping per
    paragraph (a font-selection cache in fontique, no line-break analysis for a text that
    fits, one word cache shared across shaping threads), shaping only the rows in view, a
    flex fast path like the box's, and an arena for layout nodes, each judged by an
    interleaved comparison, since one `run.sh` varies 20–60% between sessions.
    - The GPU path (vello_gpu over wgpu, the CPU its floor): frames are limited to their
    damage, popups paint on the GPU and a lost device wakes the window. Every frame drawn
    under the band clip makes a whole repaint about 13 ms slower at 1080p on lavapipe;
    partial frames copy the whole frame, wgpu having no buffer age; shadows blur live on
    the GPU and the window's shadow images are remade on every resize; an image brush over
    2048 pixels or in another format paints nothing; and no hardware adapter has been
    measured, so the CPU floor, which misses 60 Hz at 4K, is not yet called sufficient.
    - Lists: sticky headers and sections as readers see them, a row's state kept when it
    leaves the window, `derive_entries` (the store's port), the picker's open list
    windowed, and type-ahead over names that come from content.
    - The scroll bar and right to left: the Linux realization lays nothing out right to
    left, so the bar cannot move to the leading edge; a held track press pages once; and
    the bar's parts show `Pressed` for any press inside the box.
    - The shell: Wayland through smithay-client-toolkit and X11 through x11rb, with
    xkbcommon, rather than winit, which never sends surrounding text, with kinetic
    flings, touch with GTK 4's selection handles and magnifier, drag and drop with its
    icon, and fractional scale per popup. The handles and the magnifier have fixed looks,
    against "presentation is never hardcoded", so they take tokens or setters.
    - Text: a decision on the lines that differ from Pango (Khmer, Lao, Burmese, Thai at
    narrow widths, hyphenation, ellipsis); the word cache shared across threads and split
    at more than U+0020; mark stacking proven beyond Latin, Greek and Cyrillic; the
    Hebrew soft-hyphen rule checked against Chromium; the dictionaries' size in the
    binary measured; Ctrl+Arrow moving by Parley's visual words while deletion takes
    UAX #29 words, which part on the dictionary scripts; the patched Parley's cases that
    need a system font (the Hebrew and Arabic soft hyphen, mark stacking beyond one
    acute) held in `packages/parley`'s tests once a font for them is bundled; and
    parley's `system` feature, fontconfig's fallback and FreeType's masks joining
    `skene-parley` with the Linux realization.
    - Accessibility and input: Orca on X11 differs from `GtkEntry` in seven hunks, all on
    GTK's side; there is no emoji chooser, so no Insert Emoji; XIM carries no password
    purpose; a switch's role travels in `xml-roles`, AT-SPI 2.52 having none; the
    caret's rectangle on headless mutter needs a gnome-shell leg for the GUI tests; and
    AccessKit's adapters: the prototype's patched `accesskit_unix` and
    `accesskit_atspi_common` (the editable-text interface and the edit calls, caret and
    selection writes taking effect at once, the text methods AT-SPI asks for) carried
    under `vendor/` with this realization, their first client, and the Android and iOS
    adapter changes (a field's own view in the virtual tree, text ranges) with items 5
    and 6; the core already reports text runs, character geometry and selection.
    - Naming and invariants: how justification is named, `TextAlignment` having no variant
    for it while the gate bans the word, and the layout engine's debug assertion that a
    container laid out at its natural height keeps it fires on some random layouts.
    - Testing: driver only (`DECISIONS.md`, 2026-10-16), through a display runner giving
    each test process a private display, session bus and AT-SPI bus, over ten legs (X11
    on Xorg with a touchscreen, mutter, sway, sway nested in Xorg for touch, weston,
    KWin, KWin's Xwayland, and mutter, sway, KWin, KWin's Xwayland and X11 at scale 1.5).
    Some shell scenarios still wait with sleeps; mutter's screen reads judge freshness by
    a quiet time rather than presentation feedback; and the GPU tests at 1.5 run on
    neither mutter nor KWin.
    - What this host cannot answer: a hardware GPU; touch on weston and uinput; touchpad
    gestures on X11 (a protocol limit), weston 13 and KWin 5.27; the caret's rectangle
    from outside the app on sway 1.9 and weston 13; fractional scale on weston 13;
    Orca 47's list counts and Orca's own keys on Wayland; and every compositor and screen
    reader newer than Ubuntu 24.04's.
    - Upstream: the reports in `spikes/linux/upstream` (Parley, its engine, taffy,
    vello_cpu, vello_gpu, AccessKit, mutter, cargo-public-api, criterion) wait for the
    author's decision to file them.
4. **The spikes on hardware.** The text-engine spikes ran on GitHub's runners
    (`spikes/`, the `spike-apple`, `spike-android` and `spike-windows` workflows) and
    passed: TextKit 2 under taffy draws what `UILabel` and `NSTextField` draw, taffy
    measures through JNI at one crossing per leaf per pass once warm and within
    Compose's time, and DirectWrite's glyph runs keep z-order, clips and group opacity
    in vello by both paths. The text revisit (2026-10-17) added `spikes/text-parity`,
    one corpus of 131 variants and a comparison of breaks, metrics, fonts and pixels,
    run through TextKit 2, `android.text` and DirectWrite against Parley in both forms
    (`DECISIONS.md` records the outcome). What they found lands in the platform items
    below; what only a device or a GPU answers remains:
    - iOS: the system's selection beside a link's tap, the loupe and handles,
    `UITextSelectionDisplayInteraction` drawing `selection_background`, frame times on a
    mid-range iPhone, the iOS 17 floor (the runner offered iOS 26 alone), Dynamic Type
    and Bold Text changed in Settings while running, and Genmoji; the drawn field (CJK
    input, AutoFill, VoiceOver editing, gesture arbitration); and whether a plain
    `UITextInput` view with secure entry and a new-password content type gets the
    strong-password offer and the save prompt, and whether a capture blanks it.
    - macOS: whether `NSTextContent` brings AutoFill to a drawn field, and how a secure
    field's marked text and press-and-hold accents behave under its input context.
    - Android: absolute timings against Compose on a mid-range device at API 29 and 34,
    ink bounds under hardware rendering with OEM and updatable fonts, and high-contrast
    text, font scale and bold text through the engine's paint.
    - Windows: GPU frame time for 1000 labels through the atlas (under 4 ms), ClearType
    and per-monitor rendering parameters above 96 DPI, presentation through a swap chain
    or DirectComposition with opacity animated over frames, and COLR v1 emoji on a
    consumer Windows 11 build.
    - Every engine: one selection test driven through Parley and DirectWrite alike, so
    the engine trait the core runs over is proven engine-agnostic.
5. **Android.** A compositor platform: Skene's views are Android `View`s interleaved
    with native ones, written through a Kotlin shim in batches, one JNI call per frame
    carrying a byte buffer of operations and frames applied in `onLayout`, snapped to
    device pixels, the scene handing each changed parent's real children to the platform
    in one call at the end of a flush. API 29 is the floor, and what later APIs add an
    upgrade a signal proves.
    - Text: android.text over Minikin through JNI (`BoringLayout`, `StaticLayout`,
    `DynamicLayout`), each text node drawing its layout in `onDraw`, the shared painter
    drawing the decorations `Paint` lacks.
    - Text, from the spike: `StaticLayout` for every text below API 33, since
    `BoringLayout`'s older path drops fallback line spacing and clips CJK and Thai ink
    by 3 to 5 px; intrinsics fetched only for a min-content or max-content query and
    computed once per text, `PrecomputedText` reused across them, three laid results per
    leaf keyed by width on the Rust side, and only the axis taffy asks for answered;
    min-content from line-break segments, an approximation of Minikin's own breaks (no
    hyphenation or phrase breaking) that a `LineBreaker` pass would make exact.
    - Fields: one Skene view per field, its `InputConnection` written over the editing
    core with no mirror `Editable`, a full `CursorAnchorInfo`, its glyphs from the same
    engine as every label; handles, the magnifier and the floating action mode assembled
    from the platform's parts; keyboard avoidance through the IME's insets; a password
    field the same view, masked, with the password input type and a real autofill view
    with hints, committed and cancelled by its form's explicit submit and cancel;
    accessibility reported by the field's own view, joined to AccessKit's virtual tree.
    - The platform: `OnBackInvokedCallback` for predictive Back; touch mode as
    focus-visible; `configChanges` including `uiMode`; edge-to-edge from API 35; process
    death writing the snapshot of the state an app opted into on backgrounding and
    restoring it at launch, under the header's build id, secure entries excluded.
    - Its own controls in `platform::android`, beside a library's through the native
    door.
6. **iOS and macOS.** Compositor platforms: Skene's views are layer-backed `UIView`s and
    `NSView`s interleaved with native ones, reached through objc2. iOS 17 and macOS 14
    are the floors, and Writing Tools, Genmoji and phrase breaking upgrades a signal
    proves.
    - Text: TextKit 2, one stack per text node drawing its layout fragments into its own
    layer, with no Core Text path beside it; the shared painter draws the decorations it
    lacks.
    - Text, from the spike: a node measures without the extra spacing TextKit adds after
    the last line of a text-style font, as `UILabel` does; `natural` alignment follows
    the paragraph's content direction, as TextKit and CSS's `dir=auto` do, where
    `UILabel` follows the interface's; an empty text synthesizes its one line;
    `truncated` is read from the last line's range rather than from a second layout; a
    line's selection rectangle is the union of its segments, since TextKit 2 repeats
    segments on iOS 26; and a node never clips to its box, drawing within its fragments'
    rendering surfaces, which Thai marks, emoji and Arabic descenders leave.
    - Fields: Skene's own field view on TextKit 2 for every field, a password field
    included. On iOS it speaks `UITextInput` with `UITextInteraction`, the system's
    caret, highlight and handles tinted by Skene's style and Skene-drawn ones a setter
    away, keyboard avoidance follows `UIKeyboardLayoutGuide`, and a capture in progress
    suppresses a secure field's echo. On macOS it speaks `NSTextInputClient` and
    `NSTextCheckingClient` beneath the system's insertion indicator, Skene drawing the
    selection and writing its gestures and menu, and a secure field holds secure event
    input and ASCII-capable input sources while focused. The credential extras tied to
    Apple's own controls (iOS's strong-password and save offers, macOS's inline
    AutoFill) may be lost, app-initiated saving their fallback.
    - The platform: an overlay host view, a child `NSPanel` for popups that leave the
    window, context menus as data, a generated default Edit menu; text tooltips are
    impossible on touch, and Tab on macOS reaches text boxes alone unless keyboard
    navigation is on; a Mac is needed.
    - Their own controls in `platform::ios`, `platform::macos` and `platform::apple`,
    beside a library's through the native door.
7. **Windows.** A window platform, Skene owning the window as on Linux, its
    accessibility UI Automation through AccessKit; Windows hardware is needed.
    - Text: DirectWrite's layout, its glyph runs collected by a custom text renderer
    into Skene's renderer, vello's outlines the floor and a grayscale atlas DirectWrite
    rasterizes the upgrade.
    - Text, from the spike: a paragraph's line count is read from its line heights,
    since a height-capped layout still lists the trimmed lines; colour glyphs go into a
    colour atlas drawn by `DrawGlyphRunWithColorSupport` where Direct2D has COLR v1, its
    COLR v0 layers being the older designs, and a layer's format read as a set of flags;
    and the grayscale atlas still darkens against Direct2D under enhanced contrast, its
    gamma by text colour unreplicated.
    - The native door, open: whether it hosts a Win32 control inside the window Skene
    draws, the hybrid `DECISIONS.md` sets aside, or adopts no toolkit's view, as on
    Linux.
    - Text input, open: TSF, or IMM32, feeding Skene's field over the editing core is
    evaluated as iOS, Android and macOS were before the field lands, by what it keeps of
    the platform's input methods, selection, autofill, dictation and handwriting.
    - Contrast themes swap an app's colors for system colors, which literal palettes
    break; the floor is unsettled.
8. **The two facades name for name.** `skene-tui`'s vocabulary is made parallel to
    `skene-gui`'s name for name, settled against `spikes/linux/DEFAULTS.md` and
    `spikes/linux/design/api.md` (a scheme's name, item and axis alignments, a
    viewport's size, `set_children` beside `add_child`).
    - The surface traits without a parameter on both facades, as AGENTS states, so a
    facade's impl line is `impl ContainerBase for Container {}` and a library bounds
    on its facade's own traits (`E: TextEntryBase + DecorationBase`), retiring
    `DecoratedTextEntry`; each facade's trait keeps a member resolvable while the
    foundation's is in scope too, which a same-named subtrait does not
    (`DECISIONS.md`, 2026-10-26).
    - The web: `EditContext` or a rendering of its own a progressive enhancement over
    its form elements, never a replacement.
9. **The CLI for every platform.**
    - Configuration: `skene check` fails when a declared platform is not realized, and a
    command passing arguments on to cargo (`skene test`) refuses a `--config` among
    them that may change the flags it substitutes the platform's cfg in.
    - `skene build`, `run`, `test` and `check` for every platform from `web` and
    `terminal` to `ios`, `android`, `macos`, `windows` and `linux`, landing under
    `dist/<target>`, with `skene devices`, `skene doctor` and `skene eject`; native
    projects generated and disposable in `target/skene/<target>`, so each year's
    platform requirements land once in Skene's templates, and ejected when a team must
    own one; signing and packaging (Play App Signing and app bundles, automatic signing
    or `fastlane match`, cargo-packager, MSIX); density variants, per-platform fonts,
    localization catalogs, icons and splash screens from the assets and `[app]`; `[app]
    links` generating App Links, associated domains and both `.well-known` files in the
    static web output.
    - Manifests: `skene.toml` collects per target; declarations are keyed by OS, never
    by Skene platform; the app overrides or removes anything, a library's strings being
    defaults; secrets are placeholders validated before any native build; every build
    writes a provenance report. A library's native code is declared in its manifest and
    kept in conventional `native/` folders where it is unavoidable, Skene handing it the
    Activity, the JavaVM and class loader, the UI thread, a batched update path and a
    generated plugin registrant for lifecycle callbacks; platform APIs without UI are
    plain `target_os` crates, committing to `ndk-context` and `raw-window-handle` as
    their context seams, with results through `spawn` and `Resource`.
    - CI cross-clippies every pixel target against the real bindings, realization crates
    enabling only the binding features they use and keeping target-side C build scripts
    out, and no build script runs Gradle, Xcode or SwiftPM, so `cargo check --target
    aarch64-apple-ios` works on Linux.
    - The dev reload on devices: the desktops reuse the terminal's `execve` launcher;
    Android connects over `adb reverse`, writes its snapshot to app storage on `swap`,
    reinstalls with `adb install -r` and restores at boot; iOS does the same through
    `simctl` and `devicectl`; a reload takes seconds.
10. **Testing on the real platforms.** No headless platform and no test double: each
    platform is tested on itself, through what its readers read and by the input its
    users give, Linux's harness (item 3) the model.
    - Conformance: one suite of scenarios, a page per facade in its own vocabulary, run
    through a `Driver` per platform over its accessibility layer and its real input, on
    the emulators, simulators and runners the gate reaches; goldens per platform, since
    each lays its text out its own way.
    - The browser tier and the server's byte-exact snapshot stay, and the terminal keeps
    its tier.
11. **The border's constructor.** Whether a border is a `Border::new(width)` value with
    its color a property of its own, `set_border_color`, as the GTK prototype's fixes
    split it, or the split is reverted: unanswered.
12. **One owner for style and interaction semantics.** The web compiler and the spine's
    `Resolver` implement one meaning twice, and the terminal and the web each implement
    focus, press, hover and overlays; each mechanism gets one owner, and the browser is
    the oracle.
    - The parity harness (`browser_parity`) gains a case for each fix below.
    - Interaction: a group run's state variants reach its spans, as a rule on the
    group's class redefining the ink where the state holds; the token definitions it
    redefines condition on media alone, so this waits on the styling direction.
    - Interaction: on the web a state a control carries reaches the controls nested
    inside it, its rule matching any enclosing control where the scene resolves a part
    against its nearest one, so a focused picker face highlights every row of its list
    (each row a control whose `Focused` is its highlight); the compiled rule stops at
    the nearest control, and `browser_simple`'s picker test then reads the rows while
    the face holds focus.
    - Hit testing: a run is hit-tested by its untrimmed line box, which overhangs the
    box `text-box-trim` trims its line to, so in both engines a press near the edge of
    one of a column's text boxes lands on the next box's run, at a row's vertical
    centre in Chromium; a run inside a control takes no pointer, and every other run
    still takes the press its overhang covers, its own `Hovered` with it.
    - Text: a `RichText` whose spans are all empty lays out no line on the web, its
    `flow-root` holding an empty inline span the `:where(:empty)` rule of a `Line` run
    never reaches, where every other engine lays out one; and a `Container` of a fixed
    width lets a word too wide for it push past it on every platform, its one track's
    automatic minimum being the word, where a `minmax(0, 1fr)` track (and taffy's like)
    would break the word inside it as the text core asks, which is to be decided.
    - Typed events: a gesture arena before touch lands, its exception to "a view that
    handles an interaction owns it" written down.
    - Controls: `AccessProp` moves onto AccessKit's schema, beside its open `Role`: its
    properties in place of the closed report, so a third party builds switches, tabs,
    comboboxes and trees; parts become styling targets or go.
    - Structure: the web's anchors exist in the scene alone, as the terminal's do, its
    `create` making no node for them, and its markup keeps comment pairs only where
    streaming needs a marker.
    - The web's hydration: the server tier writes real pages that the browser tier
    hydrates with zero mismatches, where every browser test but one representative
    tree fakes the served markup today; structural hydration keys per scope replace the
    one global counter, so async rebuilds and nested lazy content keep their markers and
    their server markup; where `appearance: base-select` is supported, the picker is a
    styled native `<select>` and the hydration swap goes, the drawn face remaining
    elsewhere.
13. **The quadratic hot paths.**
    - The terminal: a structure change, a token's redefinition and a container's resize
    still restyle every node, and a structure change hands every node its layout
    children afresh, where each should reach only the subtree it touched (per-scope
    token and per-container query invalidation, `em` from the node's own font size);
    a hit test and a partial repaint still visit every child of a box, 1.9 ms for a
    pointer move over 16k text rows where an unchanged frame costs 28 µs (`cargo bench
    -p skene-tui --bench frame --target host-tuple --config
    "target.'cfg(all())'.rustflags = ['--cfg', 'skene_platform=\"terminal\"']"`);
    selection spans compute once per frame; clusters re-segment around an edit, and undo
    stores range diffs.
    - Token caches take per-scope epochs in place of the generations every redefinition,
    structure change and environment change moves for every scope today, and interned
    names, so a palette change costs the number of tokens beyond the web too, in the
    spine's `ResolutionContext`, which every platform but the web resolves through; a
    resolution reports the environment it read as it reports the interaction, so a
    resize restyles only what reads the viewport.
    - Constant factors: the lazy list reading a data source (a count and the item at an
    index) through a Fenwick tree, and evicting measured extents; the terminal's text
    leaves, each a paragraph of the cell engine, costing a 16k-row mount a quarter more
    than the wrap they replaced (about 500 ms against 390) and a keyed row's relayout a
    fifth more (72 ms against 60), taffy asking each leaf eleven measures a relayout; one
    constructable stylesheet with deferred collection and a set of adopted names; release
    web output precompressed, fingerprinted and preloaded, a split build's modules
    through wasm-opt as an unsplit bundle is, with one cached stylesheet shared by static
    pages instead of inlined into each; a bundle budget the gate holds, a release
    `counter` after `wasm-bindgen` having grown from 911 KB to 1.27 MB between 2026-09-15
    and 2026-10-02, `skene-web` and `skene-foundation` most of it.
    - The terminal's resized 16k-row frame measured between a twentieth and a twelfth
    slower (54 ms against 50 over eleven alternating runs, 47.5 against 45.0 over five
    more) once the forced inlines on bodies that are not tiny went (2026-10-26), under
    run-to-run noise of a tenth; restoring the spine's left it slow and restoring the
    rest brought it back. Callgrind's instruction counts on builds that keep their
    symbols settle it against its parent, and a gain shown from one forced inline
    reopens the `#[inline(always)]` entry in `DECISIONS.md`.
14. **The dev reload costs production nothing.** The saved tier is no second API:
    Android's process death writes the same snapshot of the state an app saves (item 5),
    and the web and the desktops keep none beyond the dev reload.
    - Identity: typed owner steps aligned per parent, so a `Visibility` inserted before a
    component stops resetting everything after it; keyed roots for detached owners and `root_owner`; a
    positional scope per keyed list; grouping by live owner in place of the lossy
    dedup; a tagged match leaving the untagged cursor alone; node positions counted
    apart from signal positions; in debug, call-site hints aligned first and every
    restore decision logged; the bookkeeping in a side table off the owner records.
    - Encoding: a header (magic, format, app and build id); a bridge crate that
    snapshots a foreign type through serde instead of leaving it unsaved.
    - One serialization for state crossing a boundary: hydration carries through
    `Snapshot` and serde_json leaves the web leaf, or `DESIGN.md` states that network
    data is serde's.
    - Restoration in the spine: focus, selection with its direction, and scroll restore
    by snapshot position through platform methods, shared by every platform and tested
    on each; focus following a keyed item across a reorder on the next run, which the
    terminal and the browser tests check on their own today, then becomes a conformance
    scenario, a `Driver` opening a page on a next run with the snapshot the last one
    took.
    - The terminal's swap: the launcher owns the state file, keeps it on an abnormal
    exit and respawns on the next build, so a startup panic after an edit keeps the
    state; descriptors are close-on-exec except across the swap, and the dev variables
    are unset for children; the app snapshots before restoring the terminal and stages
    before install; a second `swap` in one read coalesces; each generation's binary is
    kept for a rollback with its state.
    - The web: a staged snapshot boots a client render instead of a hydration; storage
    is keyed per app and tab; the `pagehide` session-storage copy restores on reloads
    alone and is cleared at boot.
15. **The app layer before 1.0.** Every surveyed framework treats these as table stakes.
    - Forms: an entry's `Secure` and `Keyboard` contents carried by every further
    platform's input protocol as it lands, the one field masked under the platform's
    secure-entry flag and its keyboard, autofill and hints in the protocol's own terms
    (text-input-v3's content purpose and hints, `UITextInputTraits`, `EditorInfo`); a
    `Form` view emitting a real `<form>` with field names and typed
    buttons, its GET submission a route with a query, so constraints such as `required`
    hold without script, and elsewhere its explicit submit and cancel drive the
    platform's credential commit (Android's autofill, app-initiated saving on Apple);
    `Action<I, O>` with pending, error and an optimistic overlay; a `Validate` trait
    feeding `Invalid` and `:user-invalid`.
    - Images and assets: `asset!()` handles checked at compile time, fingerprinted and
    scoped by the crate's own name, in place of hand-written URIs;
    `Image::new(asset).set_description(..)`, its description the floor where pixels
    don't show, as `Painted`'s is; a font-family property and token. `skene.toml`
    declares assets today, served in dev, copied into static output and collected
    across the dependency graph; it gains further settings as they earn their place
    (Flutter's pubspec is the model). Font assets add generated metadata—per-face
    ascender, descender, line gap, cap height, read at build time—so the web can
    emulate text-box trimming where an engine lacks it, and the platforms Parley serves
    lay text out with the same numbers.
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
16. **Names and structure while the API is unstable.** One rename wave brings the names
    in line with the vocabulary rule, the structure loses its bad edges and copies, and
    the documents shrink to what tooling cannot enforce.
    - Manifests: wgpu 30 once vello_hybrid moves past the 29 it pins; the duplicate syn
    (serde's and clap's derives on 2, thiserror's on 3) and hashbrown versions
    (wasm-split-cli's walrus 0.23 beside wasm-bindgen's 0.26) gone as their dependents
    converge. Criterion stays at 0.5 while 0.8 builds alloca's C on every unix target,
    which the Apple clippy check cannot cross-compile on Linux; base64 stays doubled
    while sfv 0.15 holds 0.22. Parley from its release once that carries the changes
    `vendor/parley` and `vendor/parley_engine` hold, since `[patch.crates-io]` stays in
    the workspace and a published crate takes Parley unpatched.
    - The documents: `AGENTS.md` at 150 lines or fewer of numbered one-to-three-sentence
    invariants, each linking its `DESIGN.md` section; `DESIGN.md` rewritten as each step
    of the migration lands, one chapter per mechanism, each with its contract, mechanism
    and decisions, the terminal's split by concern; fine facts (DECRQM modes, OSC
    numbers, keyboard tables, hydration markers) in the owning module's rustdoc;
    `README.md` for users; the guide's book built by `mdbook` in the gate, so a broken
    link or chapter fails it.
    - Doc comments: contracts alone, without rationale or narrated implementation.
    - Exports: each facade re-exports every type its surface asks an app to name, which
    today misses `MenuItemContent` (a context menu's item view's argument) and
    `Boundary` on both; the seam's `define_tokens` names its node for what it is, any
    node defining tokens, not a scope.
    - `OneOf`'s `set_key` shares its name with `View::set_key`, so where a facade's
    prelude is in scope `OneOf::new().set_key(key)` is ambiguous and only `derive_key`
    compiles; the key of the case shown takes a name of its own.
17. **Pictures beyond direct kitty transmission.** Shared memory or a file where the
    protocol's query over that medium shows the terminal shares the host, sparing the
    base64 for every change; unicode placeholders (`U=1`) through tmux, which passes no
    APC; sixel (DA1 attribute 4: xterm, foot, WezTerm, mlterm, Windows Terminal, VTE;
    not kitty, not Ghostty), palette-bound and without ids or z-order, so a change
    repaints the box whole; iTerm2's OSC 1337 last, for iTerm2, WezTerm and mintty.
18. **Internationalization**: message catalogs, locale-aware formatting, reactive
    locale switching. Catalogs are provided as a context memo, the `Theme` pattern, so
    switching locale is one write; formatting follows ICU4X and MessageFormat 2; a
    direction signal lays right-to-left out through taffy over the logical edges; rich
    text becomes an annotated string, one run per paragraph with attribute ranges as
    Compose's `AnnotatedString`, fewer nodes and a natural target for translations.
19. **The rest of the missing vocabulary.** What an ordinary app reaches for after the
    table stakes.
    - Views and controls: switch, progress and tabs first, then a spinner, a combobox,
    date and time, a number field, a tree, a table, a sheet, an alert dialog, a toast,
    a toggle button, a disclosure, a toolbar and a menu bar, on the open role
    vocabulary and `Collection<Policy>`.
    - Motion: `set_enter` and `set_exit`, compiled to `@starting-style` with a deferred
    detach on the web, and springs, compiled to CSS `linear()`.
    - Scrolling: a `ScrollHandle` with `scroll_to`, which crosses the seam as a
    `ScrollOffset` content, and scroll-into-view; `LazyColumn::scroll_to_item` and an
    index API; lazy grids and sticky headers.
    - Style: opacity and transforms as compositor-friendly properties of their own,
    gradients, box shadows, italic, text alignment, ellipsis and a maximum line count,
    shared with stated terminal degradations and the web-only ones in `WebStyle`.
20. **The terminal's app shell.** Inline mode and declarative terminal modes beside the
    alternate screen the app always takes today; printing above the app; `exec` and
    `suspend`; log capture, to a file or over the attach pipe, since the terminal
    installs no `tracing` subscriber; a screen-reader launch mode mapping the roles
    controls already report to a linear output, as Ink does; and further `Device`s the
    loop runs on through `Launch::run_on`: an SSH session's channel, and xterm.js in a
    page over a socket.
21. **Devtools on a tee of the seam.** A tee on the seam's records, a platform being no
    type to wrap now that one is chosen per build, streams every create, attach, update
    and token definition, with owners, handlers, snapshot positions and the cause of
    each effect re-run, over one protocol from any platform; the inspector is written in
    Skene and runs in a second terminal or a browser tab, since the platforms' own
    inspectors show the sink tree alone. The same stream records and replays golden
    tests. Beneath it a data-only diagnostics tier names signals and emits re-run causes
    as `tracing` spans, and debug builds warn on a signal read while building a view,
    which freezes it, and on a write inside a memo.
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
22. **Themes split by facade, and design tokens in the DTCG format.** A theme's core is
    plain constants shared by its crate for each facade (its colors, and its sizes named
    rather than measured, since cells and pixels share no unit), and each facade's theme
    crate gives the named sizes their values and builds its own themes from the core.
    Tokens stay as they are on every platform for now; whether a token must define a
    variant for each scheme is revisited later. A
    core imports from and exports to the W3C Design Tokens Community Group format,
    stable since October 2025, so a palette travels to and from design tools.
23. **Notifications and progress from the terminal**: OSC 99 (kitty, with ids and
    closing) or OSC 9 for a desktop notification, OSC 9;4 for the taskbar's progress
    (Windows Terminal, ConEmu, Ghostty); a facade seam beside the title, ignored where
    unsupported. Low value until an app asks.
24. **The terminal runs on Windows.** The `tty` module has a unix layer alone; the
    console layer (ConPTY's virtual terminal input, which helix found buggy with CJK
    and non-ASCII text, or the legacy console input records as the fallback) is
    unwritten, and no Windows machine is at hand; the item waits for one. `skene
    attach` waits with it: the control pipe an app inherits across its swap exists on
    Unix alone, and where a process cannot be replaced the app exits with status 75
    for the launcher to spawn it. The console is a `Device` beside the tty, the loop
    unchanged: its input as bytes, its resize and end requests as notes.
25. **A button inks its body as it inks its label.** `Button::set_text_color` colors the
    button's own label alone, so a body `set_child` adds, an icon, keeps the ink it
    inherits, where the core's Simple button inked an icon in its variant's ink; and an
    icon button is `Button::new("")`, its empty label laid out beside the icon. The text
    color reaches every run inside the button, as `color` inherits on the web, and a
    button drawn by its body alone has no label, so a Simple icon button is inked in its
    variant's ink again.
26. **A Simple field and card ink the text on their own surface.** A text entry has no
    text color of its own and a `Container` hands none to its runs, so
    `SimpleTextField`, `SimpleTextEditor` and `SimpleCard`, which paint Simple's card
    surface, leave the text on it to the typography in force: Simple's foreground
    beneath a `Theme` of the palette, the platform's or the app's ink with none. Decide
    how a text entry takes its value's color and a box the color its runs inherit, as a
    button inks its label (`Button::set_text_color`), and ink them in
    `SimplePalette::FOREGROUND`, so a preset inks its own views with no theme above it.
27. **The slider's standard parts on the web.** The web slider is drawn, so nothing
    waits on them, but when engines ship `::slider-fill-*` (csswg-drafts#12419) the
    native input's track can be restyled through them instead. Blocked on engines.
28. **A drawn slider stands upright.** The drawn `Slider`'s default track runs along
    its row alone, so a vertical slider is a `BasicSlider` over the behavior set
    vertical (`behavior::Slider::set_axis`) with parts of its own. The drawn one takes
    the behavior's axis by a setter named apart from the control's own row axis
    (`ControlBase::set_axis`), its default fill growing from the bottom and its thumb
    travelling up, in pixels and in cells alike.
29. **A terminal field centers its value in a taller box.** A browser draws a
    single-line `<input>`'s value in the middle of a box taller than its line; the
    terminal draws it on the box's first row, now that a row stretching its items
    stretches the field to its height. The field's value row, its caret, a press's
    mapping to its value (`field_inset`) and its paint's inset move together.
30. **A plain toggle draws its box on the web.** The reset's default chrome for an
    `<input type="checkbox">` or `"radio"` no Skene control builds (one a `NativeView`
    makes) has no border, so an unchecked one draws nothing; it takes a hairline in the
    inherited ink, as the reset's comment describes.
31. **The server's opening focus passes over a box it knows is hidden.** The
    browser leaf passes over a control set to autofocus inside a box its computed style
    hides, as the terminal does; the server computes no style and marks the first that
    asks. A box whose `hidden` is fixed, or derived and so known as the render settles,
    can be passed over there too, only a `hidden` under a condition left to the engine.
32. **A socket's send wakes its thread.** The host WebSocket's thread writes what was
    queued between reads of 10 ms; a wake pipe polled beside the socket would send at
    once and let an idle socket sleep.
33. **Server rendering beyond the native binary.** The native server binary stays the
    deployment, documented in a container (a Dockerfile and docs for Lambda, Cloud Run,
    Fly.io, Cloudflare Containers and Vercel's Rust runtime), and the WASI leaf is not
    restored. Nearly all the demand for wasm rendering is on JavaScript isolates
    (Cloudflare Workers, Deno Deploy, Netlify and Supabase edge, Node, Bun), where the
    server leaf already renders on `wasm32-unknown-unknown` after two small patches;
    three cheap steps keep that build cheap to add, and the isolate leaf itself (an
    async settle loop, a JavaScript timer and clock, exports returning a `Promise` and
    a `ReadableStream`, a per-isolate render queue, a fourth test tier under Node;
    600–900 lines) waits until someone asks.
    - The render deadline through a clock that works on every target, `Instant`
    natively and the JavaScript clock on `wasm32-unknown-unknown`.
    - All blocking behind one seam: the settle loop async, with a native `block_on`
    over today's condition variable, and `AGENTS.md` stating that the server leaf
    blocks only there.
    - `skene-transport`'s web path calling the global `fetch` rather than
    `window().fetch`, which also serves Web Workers.

34. **Confirm docs.rs documents every platform's build.** `skene-gui`, `skene-gui-web`
    and `skene-simple-gui` name each target's leaf, and `skene-tui`,
    `skene-tui-terminal` and `skene-simple-tui` the terminal, through `cargo-args`
    setting `target.<triple>.rustflags` and `rustdocflags`, and cargo takes a target's
    flags over `build.*` and an environment variable over both, so on the first publish
    either docs.rs's own rustdoc flags or these may be dropped: check every target's
    pages, and move the cfgs to wherever docs.rs's flags merge with them if any is
    missing.
35. **skrifa moves with Parley.** `skene-parley` draws glyph outlines through skrifa
    0.44, the version the vendored Parley 0.12 (its newest release and tag) reads the
    same fonts through, as vello 0.3's glifo does; skrifa 0.49 is out and compiles
    against `skene-parley` as it stands, but taking it alone builds a second skrifa and
    read-fonts (0.46 beside 0.41) into every terminal binary, parsing each font twice.
    Move it with the next Parley release, rebasing `vendor/parley`'s and
    `vendor/parley_engine`'s patches (their `SKENE.md`) onto it, the text and
    guaranteed-core tests green.

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
- Fuchsia, should it become a target: whether it is a compositor platform, whose views
    Skene's interleave with its own, or a window platform Skene draws whole is
    undecided.

## The terminal's enhancements, as a whole

Every item above that asks the terminal for a capability—the further picture
transports and notifications—shares one rule with those that shipped (synchronized
output, pointer shapes, in-band resize, underlines, the keyboard flags, grapheme
clustering, the pixel mouse, text sizing, the graphics layer with the decoration on
it, the palette read back, the answering clipboard, the explicit BiDi mode). A plain emulator is the floor and stays whole: every capability is asked for at launch,
confirmed by a parsed reply or ignored in silence, and degrades to the cell rendering
that exists today. None becomes a requirement, a type bound or a launch setting a
terminal must meet, and none is inferred from a name (XTVERSION, `TERM`), since a name
is not a capability. Multiplexers are part of the floor: tmux answers most queries
itself, passes graphics only as unicode placeholders and passes no pixel mouse, so
every item is proven through tmux and ssh as well as bare.
