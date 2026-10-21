# The prototype's defaults

Every choice the drawn Linux prototype took that neither `AGENTS.md` nor the author's answers settled, accepted as taken on 2026-10-21 and revisited when the prototype is ported into `packages/`. Each states the default and, where one was weighed, the alternative.

## The lists design

- Toolkit-name aliases go on platform controls and prefixed setters, not on shared views (alternative: shared view types too, e.g. `LazyColumn` aliased `RecyclerView`).
- Choosing lives in `Listbox<C>` (alternative: `LazyColumn::bind_chosen`).
- `Tab` looks one page ahead, then leaves the list (alternative: an exact, unbounded search).
- The server renders every item and the app pages long lists with links (alternative: a `set_server_limit` prefix plus a link).
- `ScrollPosition` by index (alternative: by key); a written position animates if the list has a transition.
- Unmeasured rows use an estimate updated only at rest (alternatives: the fixed declared estimate, or a live mean).
- An insertion at offset zero shows, as in Chromium (alternative: Compose keeps the first item).
- `GridCells::Count` rather than Compose's `Fixed`.
- Each section is its own set for a reader (alternative: one set with uncounted headers).
- Stickiness is one list property, on by default (alternative: per header).
- Type-ahead takes names from labels, cached (alternative: a `set_item_name` closure).
- A lazy list on an unbounded axis panics in debug builds.
- A row's opted-in state is kept by key in production (alternative: rows keep nothing).
- A list box's scriptless web form is an `Enhance` over `<select>` (alternative: rows with native inputs).
- A list is named with `set_label`/`set_labelled_by` (alternative: wrap it in a named `Container`).
- Android uses Skene's own `ViewGroup` (alternative: the platform `ScrollView`, correcting only at rest).
- A copy across unbuilt rows is exact, building them (alternative: built rows only, as the web does).

## The answers' API design

- `T: Hash` (alternative: `Clone + Eq` only).
- "Nothing chosen" in a picker or radio group is the app's own `Option<V>` (alternative: `Signal<Option<T>>` everywhere).
- The list box is typed by its choice, `Listbox<C>` (alternative: `Listbox<T>` with each member generic).
- Disabled composes through nested controls (alternative: each control's own).
- A box that acts keeps its own hover, press and focus (alternative: it takes the control's).
- The scroll box controls its bar's parts (alternative: the bar is its own control).
- `Expr::<Color>::ink()` for the thumb colour (alternative: two `SystemColors` tokens).
- A track press follows the platform, GTK's setting on Linux (alternative: Chromium's paging everywhere).
- A closing popup returns focus to whatever held it at open (alternative: its anchor).
- An autofocused control shows its focus ring (alternative: hidden until a key).
- The whole view layer moves into the foundation (alternative: move only the painting and menu pieces now).
- The web and compositor platforms keep their native scroll bars (alternative: a drawn bar on the web now).
- Options are content and come last (alternative: options as properties, any order).
- A standard menu entry's handler runs after the platform's own action (alternative: it replaces it).
- Web options are named by the hash of their tag (alternative: an app-supplied key).
- The styled list-box member is named `ListboxItem` (alternative: `ListboxOption`).

## Controls, choices and the scroll bar

- Bar parts' Pressed currently follows the scroll box's own Pressed, which holds for any press inside the box (like CSS :active on an ancestor). Default taken: keep it, since the default thumb has no Pressed variant. Should it be bar-only as §5.3 says?
- Track paging amount: default taken is 0.875 of the viewport, the same as PageDown and Space in the shell (Chromium's step). GTK uses its own page increment.
- A secondary press on a bar is consumed and does nothing (no context menu). Default taken: consume it.
- Inline list boxes no longer move focus on hover; only menus and popup lists follow the pointer (JUDGMENT: GTK's list box and Chromium's <select multiple> behave this way). Confirm.
- Expr::ink().with_alpha(a) replaces the ink's alpha, as packages' oklch(from ...) does. Ink with no alpha interpolates as alpha 1, and Ink to or from another fill cuts instead of animating.
- The thumb's length comes from the track's inner length and its own least size along the axis; a height or width a builder sets along the axis is ignored. Default taken as §5.1 states.
- The test driver now sends keys through mutter's RemoteDesktop D-Bus and keeps EIS for the pointer and touch, until mutter fixes the popup-grab modifiers. Keep this workaround?

## Parley

- Tab stops follow CSS and Blink: 8 × (space advance + letter and word spacing), measured from the line's start, with a half-space minimum. Pango puts its stops at 40/80 px against Parley's 38.2/76.4. Default taken: CSS/Blink.
- A soft hyphen's break draws U+2010 where the font has it, else '-'. Default taken: U+2010 first.
- The shape cache defaults to 4096 words per context, and each shaping thread keeps its own (Minikin keeps 5000). Default taken: 4096 per context.
- A run splits into words only after U+0020; NBSP and other spaces are not split points. For fonts with glyph hazards, splitting is limited to left-to-right Latin, Greek, Cyrillic and Common runs, which keeps it safe. Default taken: that conservative rule.
- Mark stacking moves a colliding mark away from its base by font_size/16, the gap HarfBuzz's fallback stacks marks with. It is on by default for every font. Default taken: always on.
- Where Pango breaks inside a Khmer, Lao or Burmese cluster, Skene keeps the cluster whole. Default taken: keep clusters whole.

## Lists

- Shift-click in a multi-choice list box adds the range to what is already chosen, and the anchor is the last option chosen alone. Default taken: union with the existing choice, as above.
- The default list box list has a maximum height of 320 px (a presentation default the user can override). Default taken: 320.
- A scroll box that overflows and holds no focusable stop becomes a tab stop, as Chromium's keyboard-focusable scrollers are. Default taken: yes.
- List items take no name from their content, following ARIA; the driver identifies a ListItem by its place in the set. Default taken: unnamed.
- List boxes gained type-ahead over their options' names, built or not. Default taken: on.
- A plain click still toggles an option in a multi-choice set. Default taken: toggle.
- The Box fast path applies only when the single child is a container. A box holding a text directly still goes through taffy's grid, because the text-leaf baseline path differs. Default taken: containers only.
- The lazy tests were loosened to hold whatever a wheel detent scrolls, rather than making the KWin driver's wheel arrive as lines. Default taken: loosen the tests and leave the driver as it is.

## Raster and the GPU path

- GPU whole frames are drawn under the same clip as bands. Default taken: exactness. A band then matches the whole frame pixel for pixel at scale 1 and 1.5, but a full repaint on lavapipe costs about 13 ms more at 1080p. Alternative: clip bands only, for faster whole frames and up to one level of difference at fractional rectangle edges (483 pixels at 1.5 on the test page).
- Outer box shadows now follow CSS and Chromium: the box's own anti-aliased area is cut out of the shadow. A translucent box no longer shows its shadow through itself, and the card's corner edge pixels in hello's screenshots carry less shadow (94 pixels, up to 6 levels). Default taken: the CSS semantics.
- The CPU shadow-image cache holds 32 MB and is keyed by the shadow's device-pixel geometry relative to the pixel grid. Images are made while recording, so on a first frame the cost moves from raster to record. Default taken: as described.
- trigger_device_loss no longer invalidates the window itself: the loss reaches the window only through wgpu's callback and the loop's wake, as a real driver reset would. Default taken: this semantics, so the GPU tests cover the wake.
- On the GPU path, a band covering three quarters of the frame or more is drawn as the whole frame, which avoids the copy. Default taken: three quarters.
- run.sh runs the X11 GPU conformance suite 3 more times (GPU_SUITE_ROUNDS), counting aborts, to keep the teardown fix covered. Default taken: 3, which adds about 3.5 minutes.

## The shell

- Drag cursor when the cursor theme has no 'dnd-move': default taken is the 'default' cursor, so a move never shows a missing-cursor X. Should we load GDK's own fallback cursor instead?
- Handle drags: default taken is that a handle never crosses the other end. GTK 4 lets the handles swap roles mid-drag and keeps CURSOR/BOUND (focus/anchor) separate. Should we copy that instead?
- Handles are drawn as teardrops from the palette (palette.control fill, palette.outline stroke) rather than Adwaita's PNG assets, and the magnifier's OSD colours are fixed (dark rgba(37,37,37,0.7), light rgba(53,53,53,0.7), a 10% white border). The doctrine says presentation is never hardcoded: should handle and magnifier looks become tokens or setters? Default taken: built-in drawing.
- Non-composited X11: default taken is the drag icon drawn on the field's fill and clipped to a bounding shape built from its alpha runs, as GDK does on a screen that composites nothing. Is a shaped window acceptable for the icon?
- Multi-line drag icon hotspot: default taken is the press's offset from the selection's first range, as GTK computes it. This puts the pointer off the text when the press was on a later line. Keep GTK's behaviour?
- A scaled leg's test screen is now SCREEN in logical pixels (1920x1536 device pixels at 1.5) rather than SCREEN in device pixels. Screencasts get bigger, but every scale gives the same room. Keep this?
- Mutter's screencast quiet time is now 200 ms (was 60) for every screen read on the mutter legs. Default taken: no wp_presentation-based freshness check yet.

## Test infrastructure

- Default POOL=4 on this 4-core machine. POOL=4 with release builds beside the jobs gave mutter flakes (run 2); POOL=4 with no builds beside them passed every test job. Default taken: 4, with no build in phase 3.
- Should the skene-new app's build, checks and run go in phase 2, beside the workspace builds, rather than in phase 3 beside the tests? It costs nothing in phase 3 and keeps phase 3 free of compile load. Default taken: phase 2, as a job.
- Should the shell's sessions (clock-paced scenarios plus start-up TIMING lines) run alone in phase 4 rather than side by side? Default taken: alone (86 s).
- Should KWin run with its permission checks on, granting fake input to the kwin-fake-input helper through a desktop file in the session's own data home, instead of KWIN_WAYLAND_NO_PERMISSION_CHECKS=1? Default taken: checks on, with the helper.
- Should the driver hold a pressed key until a round trip on the window's connection completes, rather than releasing as soon as the press has arrived? Default taken: the round trip, which adds one round trip per key press.
- Should the frame benchmark default to 6 rounds, extend undecided judged cases up to MAX_ROUNDS=15, and report 'undecided' instead of forcing a verdict from the median alone? Default taken: yes.
- Should sway's nested-in-Xorg touch variant stay opt-in (SKENE_SWAY_BACKEND=x11, run by run.sh only for gestures:: and handles::), since its motion tests fail there? Default taken: opt-in.

## Upgrades: the prototype

- taffy #1284: a Box or Stack is now no shorter at its least than its content, within its max. Inside a shrinking Column it overflows instead of shrinking, as a CSS grid item does. Default taken: follow taffy and CSS, which is also what the web realization does with a Container as a grid. The alternative is a vendored taffy change that keeps the old minimum, at the cost of diverging from the web.
- taffy #1234 and #1206 change the layout of a child pinned to one edge (it shrinks to fit beside that edge) and of a child pinned to both edges with a self alignment (it takes that alignment). Default taken: follow CSS. Stack's doc ('a Pinned child keeps its distance to the edges it names') still holds.
- The pre-existing picker flake on mutter at 1.5 is reported, not fixed (outside an upgrade). A ROADMAP item is left for the author's session.

## Upgrades: the main workspace

- criterion: default taken is hold at 0.7.0. Alternatives: move to 0.8 and cfg-gate the terminal bench off Apple, or set a clang for the Apple target in the gate.
- API nightly: default taken is hold nightly-2026-08-13. Alternative: move to the newest nightly and rewrite the 792 `Self` lines.
- taffy 0.15: default taken is that every skene-layout node is relative, as under 0.14. Alternative: only stacks are relative, as the web's CSS does it, which changes overlay and pinned placement.
- Runner: default taken is ubuntu-26.04 now, unverified in CI. Alternative: wait for ubuntu-latest to move in November.
- Inter 4.1: default taken is adopt it in packages/ (one golden and one test value change). Alternative: keep 4.0.

## Upgrades: the spikes

- JDK for Gradle: Temurin 25, the newest LTS, rather than the plan's default of 17. It builds identically locally and in CI.
- targetSdk 37, the newest. Emulators below 37 are unaffected; on API 37 devices, 37's target-gated behaviours apply. Alternative: target 35/36.
- API 37 emulator: 37.0 Google APIs (4 KB pages) rather than a 37.2 16 KB-page image.
- Pinning ndkVersion (r30) makes AGP strip the packaged .so files, which it could not before. The measured sizes are taken before packaging and are not affected.
- Error policy for jni 0.22: ThrowRuntimeExAndDefault, so a Rust panic or JNI error throws a Java RuntimeException instead of aborting the process. Alternative: LogErrorAndDefault.
- Swift: tools version 6.3 in Swift 5 language mode. Alternative: migrate the sources to Swift 6 mode.
- setup-gradle v6 with the MIT 'basic' cache provider rather than the default 'enhanced', which is not MIT-licensed.
- Windows keeps both windows-2025 and windows-2022 in the matrix, since they measure two DirectWrite versions.
- Re-mark the Android spike's vendored parley_engine as 'Skene:' and give it a SKENE.md, as the prototype's vendored crates have? Default taken: left as it is.

## The audit's fixes

- F4: a control named by its content (option, button, menu item) takes the texts its caption shows, in order, joined by a space, where the caption states no name; Label::set_name and derive_name still win. Default taken: name from content, as GTK 4 and Chromium do. Should type-ahead use these content names too?
- Soft hyphen after a Hebrew letter: default taken is to break there, as Chromium 156 does, overriding ICU4X's LB21a; Pango 1.52 does not break. Keep the Chromium rule?
- List box caption: default taken is Beside, before the list box among its siblings, through a new CollectionPolicy::LABEL_PLACEMENT. Radio groups and menus keep Leading. Confirm.
- A length token given to set_item_height/set_item_width is a Fixed extent. Is that right, or should a token be able to name an estimate?
- Media::min_width/max_width now hold an f32 compared and hashed by its bits, so 900 and 900.0 are equal while -0.0 and 0.0 are not. Acceptable?
- F9: text_menu() and operation_label() are required Realization methods with no default, so each realization names its own menu order and English labels. Should labels come from a localization source instead?
- The window-system environment variable SKENE_BACKEND was renamed to SKENE_WINDOW_SYSTEM along with the type, in the runner, tests and tool scripts. Keep the new name?
- One shm pool per Wayland buffer instead of SCTK's growing shared pool. This costs a memfd and mmap per new buffer, avoids weston's crash, and lets popups reuse released buffers. Keep it, or switch to a pool that grows only when it is not shown?

## The final run

- Stand-ins are paced by a SIGUSR1 per step, with the process ids exported by session.sh. Default taken: signals. The alternative is a D-Bus method on the portal stand-in and an X property on the XSETTINGS manager.
- Should the realization guard against the X11 keymap race, for example by reloading only when a notify's serial is newer than the key's? Default taken: no, as GTK does not; the scenario waits instead.
- The shell sessions stay alone in run.sh's phase 4 (77 s), because their start-up TIMING and FRAMES lines are measurements. Default taken: keep them alone.

## Rebuild and start-up

- A view generic over the app's types is thin over a core that is not (`Picker<T>` over `PickerCore`, `Control<B>` over `ControlCore`), and its facade builds the core through a non-generic function of `skene-gui` (`wraps!`'s `thin`), so the core is compiled once whether or not the framework's crates share their generics (alternative: rely on `-Zshare-generics`, which a `skene new` app, its framework crates built at `opt-level = 2`, does not have).
- A choice group's core holds its values as `AnyChoice`, compared and hashed through the value's own `Eq` and `Hash` (alternative: the core generic over the value's type).
- `Control<B>` holds its behavior as `Rc<dyn Behavior>` once built (alternative: one build per behavior).
- A styled control's constructor stores what it is given, its defaults applied as it is built, beneath what its user set (`Control::over`); a user's empty caption still removes the default's (alternative: defaults set by the constructor).
- A keyed list (`derive_children`) tells its keys apart by their 128-bit wide hash (`Key`), as a lazy list does (alternative: the key type's own `Eq`).
- The fonts, fontconfig's configuration and the compose table load on threads of their own from the start, the first engine and the first key waiting for them (alternative: the main thread, in turn).
- The settings portal is read with one `ReadAll`, each key alone only where `ReadAll` answers an error (alternative: `ReadOne`, then `Read`, per key).

## The default scroll anchor

- `Edge` names the two ends of a list's axis, `Start` and `End`, `Start` where unset (alternative: SwiftUI's `UnitPoint`, which can also name the center or any fraction).
- `Edge` sits among the foundation's values beside `Edges`, in the facade's prelude (alternative: in the `lazy` module beside `ScrollPosition`, for lazy lists alone).
- One anchor serves both where the list starts and what it holds as its content and its viewport change (alternative: SwiftUI 18's per-role form, `defaultScrollAnchor(_:for:)`, with an initial offset apart from size changes).

## The GTK realization's removal

- The foundation's `contents_gate` stays, with no realization in the prototype running it now that the GTK realization's gate test is gone (alternatives: run it on the drawn realization, or remove it).
