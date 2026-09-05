# Roadmap

The phased migration to the driver-vocabulary architecture (`DESIGN.md`), then the platform build-out.
Phases are ordered; later phases assume the earlier ones. Native is deliberately
last—not a current priority. **Current focus: the redesign to one spine and thin
facades (the section at the end of this file), phases R1 and R2 in flight.** Everything
above that section describes the code as it stood when the redesign began.

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
- **Remaining:** none—the true generational arena and the `Id` allocation redesign
  landed with the reactive engine rewrite (backlog); the implicit batch around event
  dispatch landed with phase 4.

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
- **Recording driver—done.** `skene-record` is the host driver: the same typed
  vocabulary, mapped to an in-memory tree that records every realization call. The
  facade's views compile and realize on the host, so view logic tests headlessly
  (`packages/gui/tests/record.rs`): structure, derived props, reconcile reuse, and
  events through `trigger_` seams with the `Prevented` verdict returned. Recording is
  bookkeeping, not emulation—no layout, no built-in event defaults. The wasm-only
  surface (entry, hydration, tasks, resources, routing, head) stays web-gated.
- **Remaining:** driver selection by feature when a second same-triple driver lands.

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
- **Full layout vocabulary—done.** The box props expose taffy's complete capability
  on both facades, scalar-generic in core: `Dimension` lengths (fixed, `Fraction`
  ratios `0.`–`1.`, auto), margin, position + inset, aspect ratio, flex
  wrap/grow/shrink/basis, grid track templates (`Flex` shares, `Minmax`,
  `FitContent`), auto tracks and flow, line/span placements, and per-axis `spacing`.
  Naming and shape follow the settled rules in `AGENTS.md`: layouts are typed
  container views (`Row`/`Column`/`Grid`/`Stack`/`Container`) exposing only what they
  honor, per-child properties ride in wrapper views (`Flexible`/`Pinned`/`GridCell`)
  accepted through typed child kinds, distributions are arrangements and per-item
  properties alignments, and one setter serves each property. The web compiles it all
  to CSS; the TUI feeds taffy directly, in cells.
- **Remaining:**
  - Streaming style deltas: a `Deferred` chunk whose content interns a brand-new class
    carries that rule in its chunk.
  - Hydration adopts the server's `<style>` rules instead of rebuilding duplicates.
  - Variants for the raw-`Declarations` escape hatch (currently base-only).
  - Investigate double-classing: one shared class per element kind carrying its reset,
    one content-addressed class carrying its initial style (dynamic values stay inline
    overrides)—so the reset text stops being duplicated into, and hashed with, every
    generated class.
  - Adopt the standard slider parts for the filled run, once an engine ships them. **The
    doctrine breach is fixed**—`--skene-slider-fill` is now a running app's enhancement
    and nothing more. It was written by both leaves; the server leaf's write is gone, so
    a served page paints no fill (the `0%` fallback) rather than one frozen at the
    serialized value while the control kept working without script. Gecko's
    `::-moz-range-progress` is engine-native, follows the value with no script, and
    stays. Blink without script now shows an empty track, which is honest—the thumb's own
    position reports the value—and the browser leaf fills it from `create_slider` onward.
    What remains is cosmetic, not doctrinal, and is **blocked upstream**: the standard
    parts do not exist yet. csswg-drafts#12419 is open, proposing
    `::slider-fill-lower`/`::slider-fill-upper` with "names to be bikeshed" and labels
    *Needs Edits*, *Needs Naming*, *Needs Testcase (WPT)*; no engine ships slider parts;
    and while `appearance: base-select` shipped in Chrome 134—which is exactly why
    `SimplePicker` could take that route—"Base Appearance Form Controls" beyond select is
    still only Proposed, with range not in scope. Confirmed by direct check rather than
    shipping notes: on Chrome 148 `CSS.supports('appearance', 'base')` is false, so the
    prerequisite itself is unreachable and the vendor pseudo is the only route on current
    Blink. Revisit when that lands: take the parts, retire the Gecko pseudo once base
    appearance covers both engines, and drop the custom property. Also weighed and
    rejected on their own terms: a value-sized `linear-gradient` in the interned class (a
    class per value—unacceptable churn) and the fill as our own child views (loses the
    range element's keyboard and accessibility behavior, which is the reason the control
    is a real input).

## Phase 4—Events and first controls ✅ (core)

- **Done.** Portable event vocabulary: click, pointer enter/leave, focus/blur, key down
  (`KeyEvent`: W3C key names + modifiers), input. On the web the bubbling events are
  **delegated**—one listener per event type on the mount root, dispatch walks the
  target's ancestors by a stamped node id—and the non-bubbling pointer enter/leave
  attach per node. Every dispatch runs inside an implicit reactive batch. First control:
  `TextField` (web `<input>`, a void element in SSR output) with
  `bind_value`/`derive_value`/`handle_input`/`handle_key_down`, placeholder, and the full
  declarative styling surface (`set_style`, `set_when`, transitions, animations).
- **Events complete.** Pointer down/up/move (`PointerEvent`: viewport position, button,
  modifiers; delegated), scroll (`ScrollEvent` on a box with `set_overflow`), change
  (committed value), and focus management: `set_tab_index`/`set_autofocus` (serialized
  in SSR output) plus `FocusHandle` for programmatic focus. `KeyEvent` modifiers moved
  into a shared `Modifiers` struct.
- **Basic controls—done.** `Button` (the one focus-taking box; a `Container` never
  takes focus—controls do), `Checkbox`, `Radio` (grouped, arrow-key movement),
  `Picker` (fixed options), `TextEditor`, `Slider`—each realized as its semantic element
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
- **Typed links—done.** `Link::new(route)`: a real anchor carrying the route's path,
  so a page without JavaScript navigates by full page load, and a running app
  intercepts the click into `Router::navigate`—modified clicks stay with the browser,
  so open-in-new-tab keeps working. Route parsing trims trailing slashes (the root
  stays `/`), matching the directory URLs statically-hosted routes serve under.
- **Streaming through the runner—done.** The stream walk drives a sink
  (`render_to_stream_with` handing out `StreamPiece`s; the collected
  `render_to_stream` wraps it), `render_page_stream` assembles progressive full-page
  flushes, and the server entry's `--stream <path>` mode writes each flush to stdout
  as it is produced. `Runner::render_stream(path, sink)` forwards the guest's writes
  through a channel while the render still runs—the shell arrives before a deferred
  boundary settles—and the pieces concatenate byte-identically to `render(path)`.

## Phase 6—TUI facade ✅ (core)

- **Done, twice-shaped.** First landed as a driver of the shared GUI vocabulary; then
  reshaped into a **first-class, equal facade**: `skene-tui` owns its vocabulary
  outright—`u32` cells, `bool` bold, one-cell colored borders, terminal conditions
  (`MinWidth`/`MaxWidth`/`MinHeight`/`MaxHeight` in cells, `Focused`, `Hovered`,
  `DarkMode`/`LightMode`)—so nothing a terminal cannot render exists to call. Its
  `Container`/`Text`/`TextField` builders and realization live in-crate; layout is taffy at
  `1.0 = 1 cell`; frames paint via crossterm (truecolor with detected 256/16-color
  fallback, bold, reverse, `Tab`/`Enter` focus, input editing, `Ctrl-C`) or headlessly
  through `render_to_text`, with `trigger_key`/`focused_node` test seams.
- Alongside it: the umbrella returned to its `skene-gui` name (GUI is not the blessed
  way), its `tui` feature is gone (selection is by facade dependency), and the
  structural machinery (`Children`/`reconcile`/`derived`/`DerivedView`/`empty`) hoisted
  into core as zero-cost `TreeOps` generics both facades alias. The reactive UI-thread
  designation became resettable under the `test` feature for multi-threaded host
  harnesses.
- **Mouse—done.** Motion drives `Condition::Hovered` variants and pointer enter/leave
  handlers; left clicks hit-test against the taffy layout, focus the target, and run its
  click handlers; a hovered node's `cursor` sets the pointer shape via OSC 22. Headless
  `trigger_mouse_move`/`trigger_mouse_click` seams drive it in tests.
- **Parity—done.** Pointer down/up/move with the terminal's own `PointerEvent` (`u32`
  cells, terminal-absolute and view-relative, button, modifiers); `set_title`/
  `derive_title` via OSC 2; `FocusHandle` + `set_autofocus`; `Input::handle_change`
  matches the GUI facade's real-time naming. `Button` is the focus-taking box (`Tab`
  reaches it; `Enter` and `Space` activate it); focusability belongs to controls, never
  plain containers—a clickable box stays mouse-clickable but out of the tab order,
  exactly like the web.
- **Controls—done.** `Checkbox` (`☑`/`☐`), `Radio` (`◉`/`○`, group-exclusive, arrow
  keys move the check), `Picker` (current label plus indicator, `Enter`/arrows cycle
  the options), and `Slider` (`──●──` track; arrows nudge by step, a click jumps the
  thumb)—each on the shared `ControlBase` + `control_surface!` builder surface
  (`Button` and `TextField` retrofitted onto it), with the same
  `bind_`/`derive_`/`handle_` vocabulary as the GUI facade. Every control glyph is a
  single cell and configurable: `ControlTheme` restyles a subtree through the reactive
  context (nested themes overlay), and each control's own `set_marks`/`set_indicator`/
  `set_track` wins locally. TextEditor still waits on caret positioning.
- **Scroll—done.** `set_overflow` on the box vocabulary (taffy computes the scroll
  region); the wheel scrolls the innermost box that can still move, three rows per
  report as a browser's notch does, `PageUp`/`PageDown` the scrollable nearest the
  focus; children paint clipped and offset, with proportional track-and-thumb overlays
  on the overflowing edges; hit-testing resolves through the offsets; focusing a
  control scrolls it into view; `trigger_mouse_scroll` drives it headlessly.
- **Shadows and the compositor—done.** `set_shadow` on the box vocabulary casts a
  one-cell band down and right, the color mixed by its alpha into both the foreground
  and the background beneath (unset colors mix against the detected light/dark
  default); carried by the boxes that own their container—`Container`, `Button`,
  `TextField`, overlays—because their whole box is the interactive surface. The small
  controls are glyphs, not boxes: they take a `ControlStyle` (mark color, bold,
  cursor), every box property on one is a compile error, and a wrapping `Container`
  carries sizing, padding, borders, backgrounds, and shadows (the control paints no
  fill, so the wrapper's shows through; the slider fills its container's width). Every
  layer already paints into the one internal cell buffer—which the shadow and scrim
  mixes read—and the flush rewrites only the rows that changed since the previous
  frame. Modals default to a half-transparent black scrim alpha-mixed over the frame
  (both colors of every cell, characters dimming through); a fully transparent
  backdrop disables it, for the shadow-on-the-dialog look instead.
- **Event consumption contract—done.** Dispatch is one documented contract in core
  (`skene_core::events`): user handlers first in fixed order (deepest-first along the
  hit path; focused-then-ancestors for keys; every handler on one node always runs),
  then the platform's own response—`Tab` focus movement, `Enter`/`Space` and click
  activation of the control a driver realizes, input editing, `Escape` dismissal—unless
  prevented. Handlers return their verdict as anything `Into<Prevent>`: `()` observes
  (`From<()>` maps to `Prevent::None`), `Prevent::Platform` suppresses the platform's
  response but keeps bubbling, `Prevent::Bubbling` the reverse, `Prevent::All` both;
  prevention only accumulates. A view composing its own behavior out of these events is
  not the platform and `Prevent` does not reach it: it takes its user's handler as an
  `Override`, which runs first and either falls through (`ControlFlow::Continue`) or
  takes the event outright (`ControlFlow::Break`).
  The web
  maps a prevented default to `preventDefault` (canceling the same natives: Enter
  activation, checkbox toggling, text insertion) and stops its delegated walk on
  prevented bubbling, so the facades agree bit-for-bit. `Tab` and `Escape` stopped
  preempting handlers on the TUI; sliders rise on `Up`/`Right` like the platform range
  input. Notifications (change, toggle, scroll, focus/blur) take no verdict; `Ctrl-C`
  stays unconditional.
- **Text wrapping—done.** A run's leaf is auto-sized and measured through taffy: a
  greedy wrap at the available width (lines break at spaces, a word longer than a full
  line breaks mid-word, a newline always breaks), min-content the longest word,
  max-content one line. Paint re-wraps at the laid-out width, so stretched and shrunk
  runs agree with their measure.
- **Caret positioning—done.** An input's caret is a character position in its value:
  characters insert at it, `Backspace`/`Delete` remove around it, arrows and
  `Home`/`End` move it, focusing an input places it at the value's end, and a click
  places it at the clicked column. The focused field paints it as the one cell flipped
  back out of the reverse video; a long value slides a window that keeps the caret on
  the last cell and stays put on blur, as the web input's scroll does.
- **Border character sets—done.** `BorderGlyphs` on the box vocabulary
  (`set_border_glyphs`, with a `derive_` twin and variant support): six public
  characters—edges and corners—with the named sets `LIGHT` (the default), `ROUNDED`
  (`╭╮╰╯`—the terminal's border radius), `DOUBLE`, `HEAVY`, and `ASCII`; a custom set
  is the struct built directly.
- **Interaction polish—done.** Control marks are width-safe strings (`[x]`/`( )`
  defaults—box-symbol glyphs render two cells wide in some emulators—themable back
  via `ControlTheme`), and a checkbox or radio takes a `set_label` rendered as part of
  the control, so clicking the label toggles. The picker opens a real options popup on
  activation, anchored directly beneath the control (flipping above when the bottom
  runs out of room): arrows and the pointer move the highlight without committing,
  `Enter`, `Space`, or a row click chooses it, `Escape` dismisses with the value
  unchanged, and focus leaving the picker closes it; arrows still cycle directly
  while closed. A press inside an anchored popup keeps its control focused, as a
  browser's select does. Sliders drag: a press grabs,
  motion slides until release. Scrollbars respond to the mouse—a track press jumps the
  thumb, a thumb press grabs it, dragging follows—and thumbs paint at eighth-cell
  resolution (the `▁`–`▇`/`▏`–`▉` block runs, the missing directions as reversed
  complements), inked over a tinted track, so short tracks move smoothly instead of a
  full cell at a time. Hover pointer shapes (OSC 22) default by content—the pointer
  over buttons and toggling controls, the I-beam over inputs and selectable text—an
  explicit `set_cursor` winning.
- **Rich text—done (TUI core).** `RichText`/`Span`: inline spans flow and wrap as one
  paragraph with per-span color and weight; a linked span underlines in the scheme's
  link blue and anchors an OSC 8 hyperlink the terminal itself opens. Opt-in selection
  treats the paragraph as one source flow, copying across span boundaries. Verified
  under the pseudo-terminal: the anchor opens and closes around the span, underlined.
  Remaining: derived span content, span groups, and embedded views—the web facade's
  full `Span` surface.
- **State feedback—done.** Focus no longer inverts: controls carry built-in feedback
  as subtle background tints of the scheme's ink—three sixteenths for hover, four for
  focus, five for both, each distinct—applied only for states the author hasn't
  conditioned a variant on, so `set_when(Hovered, …)` replaces the default the way
  author CSS replaces UA styling. Reverse video is gone entirely: the scrollbar's partial caps
  paint their complement glyphs with explicitly swapped colors, and the input caret is
  the terminal's own bar cursor (`DECSCUSR`), blinking as the terminal does—hidden
  while a modal or a covering popup makes its input inert. The picker popup marks its
  current option with a highlight bar over the row instead of an `x` prefix.
- **Text selection—done.** Selection is controlled explicitly across backends and
  opt-in: `set_selectable(true)` marks a run selectable, control chrome—button labels,
  picker rows—never is, and on the web the reset turns `user-select` off until a run's
  flag restores it. In the terminal a drag selects across runs in document order, a
  double click takes the word, a triple click the run; the highlight paints in
  selection colors set per run or inherited through `TextTheme` (`::selection` rules
  on the web), defaulting per the detected background. A right click over a selection
  opens a copy menu anchored at the pressed cell; `Copy` writes the clipboard over
  OSC 52 with the text as it appears in the source—soft wraps restored to their
  spaces, runs joined by newlines. A terminal identifying as VTE over XTVERSION
  discards clipboard writes, so there `Copy` presents disabled with a hint. Verified
  under the pseudo-terminal: drag highlight, the anchored menu, the OSC 52 payload,
  and the disabled VTE path.
- **Web control defaults—done.** The web controls carry real defaults in place of the
  stripped UA chrome, all drawn in the inherited ink: checkboxes and radios a bordered
  box or ring whose mark scales in, the slider a track and thumb through the vendor
  pseudos, and the select's dropdown styled through the customizable-select rendering
  (`appearance: base-select`, `::picker(select)`) where the engine supports it. Hover
  and focus feedback is a slight ink wash over the background under a stronger ink mix
  into the border color, per state suppressed once an author variant conditions on it.
  Controls join the framework's text inheritance (`ControlProps::text`): the resolved
  ink, size, weight, and selection pair land in the interned class, so an input's text
  is never the browser's black. Checkbox and radio take the TUI's `set_label`, realized
  as a wrapping label element (`Role::Label`), so clicking the caption toggles. Fixes
  riding along: the mounted root scrolls by default, clickable controls carry the
  pointer, `Stack` is its pinned children's containing block, a modal dismisses only on
  clicks landing outside its box, one delegated `input` listener serves the change and
  toggle registries together (the second registrant was starved), controls are exempt
  from text-box trimming so descenders paint, and top-layer elements restate the
  selection opt-out. Both galleries tag their dynamic signals, so the dev loop restores
  them across restarts.
- **Raw controls + the Simple library—done.** Every form control renamed with a `Raw`
  prefix (`RawButton`, `RawCheckbox`, `RawRadio`, `RawPicker`, `RawSlider`,
  `RawTextField`, GUI-only `RawTextEditor`)—the unstyled, infinitely extensible bases
  component libraries build on. Over them, each facade ships the first-party `Simple*`
  set (`SimpleButton` with `SimpleVariant` emphases, `SimpleCard`, `SimpleCheckbox`,
  `SimpleRadio`, `SimplePicker`, `SimpleSlider`, `SimpleTextField`, GUI-only
  `SimpleTextEditor`), composing the raw controls through their public surface alone
  under one neutral palette (`SimplePalette`) and shape language shared across GUI and
  terminal—the web's corner radius is the terminal's arc-cornered border glyphs. Every
  example uses the Simple components instead of hand-styling raw ones.
- **Simple shared and configurable—done.** The palette and the variants' resolved
  colors extracted into `skene-simple`, imported by both facades behind a default-on
  `simple-views` feature. Every Simple component forwards the raw controls' non-styling
  surface (focus order, autofocus, focus/anchor binding, focus/blur/key handlers), and
  its semantic knobs pair `set_` with `derive_` where the machinery supports it—derived
  button labels (`SimpleButton::derive`), card spacing and alignment. Terminal Simple
  feedback moved off the backdrop onto the border: every Simple button is a rounded
  bordered box (a transparent border keeps its cells but paints no glyphs—new box
  vocabulary semantics—so `Ghost` holds its footprint), hover brightens the border to
  the ring, focus inks it in the control's own ink. TUI polish riding along: word
  motion and deletion break on character class (alphanumeric-plus-underscore runs,
  punctuation runs, whitespace skipped); the slider paints a dimmed full-block track
  under a full-ink thumb at eighth-cell precision (the scrollbar technique); `Tab`
  commits an open picker popup's highlight instead of moving focus, as the web does.
  The `raw`/`simple`/`lazy` view families moved into folder modules
  (`raw/button.rs`, `simple/card.rs`, `lazy/column.rs`) in both facades.
- **Links, cursors, and Simple polish—done.** Terminal links act like a browser's: a
  click opens the destination through the system handler (press and release on one
  cell, so selection drags never fire it), the pointing hand shows over them, and the
  OSC 8 anchors already emitted keep the terminal's own handling. Web links gained
  their defaults back—underlined, scheme link blue, the theme's base ink never colors
  an unstyled link—plus the `LinkPolicy` vocabulary: an external (absolute) destination
  opens a new page withholding the opener and referrer, all overridable, with the
  `nofollow`/`sponsored`/`ugc` endorsements. Sliders show the open hand at rest and the
  closed hand mid-drag on both platforms. A web press outside selectable text collapses
  the selection without making chrome selectable. The terminal's solid Simple buttons
  went borderless—one row, their surface shifting on hover and focus—while `Outline`
  keeps its border feedback; the galleries agree on rich text last, the pinned badge's
  corner, and card widths that split the swatch grid into whole cells.
- **Chromium follow-ups—done.** Running the gallery in Chromium surfaced what Firefox
  had hidden. The reset's old anchor-neutralizing rule (`color: inherit`) sat later in
  the file at the same zero specificity as the new link defaults and won, so links
  rendered black; the defaults now live in that one rule. Blink has no counterpart to
  Gecko's `::-moz-range-progress`, so a slider showed no filled run: the filled part
  now rides a `--skene-slider-fill` custom property, set as an inline override by both
  leaves (the browser leaf on every value change and edit, the server leaf during
  serialization) and consumed by a `background-size` longhand on
  `::-webkit-slider-runnable-track`. The customizable-select popup inherits the
  select's own background, which was transparent, so `SimplePicker` now sits on the
  card surface. `RawPicker` grew `set_highlight`, and the Simple popup highlight
  speaks the palette instead of the browser-blue default. The galleries space
  text-box-trimmed paragraphs wider, raise the stack demo, and ink "colors" in amber.

## Backlog—unphased features

Wanted, not yet scheduled into a phase. (Confirmed already true, recorded here so it
stays true: **URI paths on all platforms**—`Route::parse`/`to_path` speak URL paths
everywhere; the browser History API, the server request path, and the TUI's in-memory
sink are just different sinks for the same paths.)

- **Explore: split the TUI debug run like the web's.** `skene run tui` currently takes
  over the invoking terminal, so build errors and instance logs have nowhere to go but
  a dismissable in-app dialog that swallows them. Instead, keep the invoking terminal
  for the build loop and logs, and print a runner command the user executes in a
  second terminal to attach the actual TUI app—mirroring how the web flow keeps the
  terminal while the app lives in the browser.

- **Multi-line text input for the terminal.** `RawTextEditor`/`SimpleTextEditor` are
  GUI-only; the terminal's editing story stops at the single-line `RawTextField`. Wanted:
  the same pair in `skene-tui`—the value soft-wrapping at the field's width through the
  run measurement already in `paint`, the caret walking visual lines with a remembered
  goal column, `Up`/`Down` moving by wrapped line, `Enter` inserting a newline instead of
  activating, `Home`/`End` per visual line with the document ends under `Ctrl`, the value
  window scrolling vertically as the caret leaves the box (the horizontal slide the field
  already does), and explicit row sizing. Browser parity is the reference for every one of
  those, per the TUI's standing rule.

- **Selection and clipboard in every terminal text input.** Selection covers selectable
  runs today; the field and the coming editor have a caret and no selection, and the
  right-click menu offers `Copy` alone. Wanted: the source-anchored selection model
  extended over an input's value—drag, shift-arrows and shift-click extend, double click
  takes the word, triple click the line, insertion replaces the selection,
  `Backspace`/`Delete` remove it—and the menu grown to `Cut` and `Paste` over an input,
  each disabled per what the terminal supports and whether the input is read-only. `Copy`
  and `Cut` write OSC 52 through the path that already exists. **Paste is the hard half:**
  reading the clipboard needs an OSC 52 query reply, which most terminals refuse or gate
  behind permission, so the reachable design is bracketed paste (crossterm's
  `Event::Paste`) as the real source, with the menu item pulling from an OSC 52 read where
  XTVERSION says it works and otherwise presenting the terminal's own paste gesture as the
  hint. The VTE-discards detection already in place is the model for that capability
  probe.

- **`Ctrl-Q` quits; `Ctrl-C`/`X`/`V` do their real jobs.** `Ctrl-C` is an unconditional
  quit ahead of dispatch today (`terminal.rs`, `dispatch_key`), which costs the terminal
  the one clipboard shortcut every other platform has. Wanted: `Ctrl-Q` takes over as the
  unconditional quit—the guaranteed exit stays guaranteed, and raw mode already delivers
  these as keys rather than signals—while `Ctrl-C` copies, `Ctrl-X` cuts, and `Ctrl-V`
  pastes over the clipboard path above, running through the ordinary dispatch contract so
  a handler can prevent them. `Ctrl-C` without a selection is a no-op, as it is in a
  browser. Each menu item carries its shortcut as a hint, and the quit key is documented
  where `launch` is.

- **Explore: undo/redo in the terminal** (`Ctrl-Z`, `Ctrl-Shift-Z`/`Ctrl-Y`). Browser
  parity points at per-input history, not an app-wide command stack: the framework has no
  model of what a user action is, but a text control owning a bounded ring of
  (value, caret, selection) entries is self-contained and matches what the DOM gives an
  `<input>`. Open questions: coalescing (consecutive insertions or deletions of one
  character class collapse into one entry, caret jumps and blur close it), what a
  programmatic write does (the DOM clears the stack on a script-set value—`bind_value`
  writes are the counterpart), whether the stack survives the dev loop's restore, and
  whether `Ctrl-Z` should keep any route to suspend now that raw mode swallows `SIGTSTP`.
  The web facade needs no code for this—the DOM already undoes—so the work is the
  terminal's, and the parity claim is what the exploration must confirm.

- **Reactive engine rewrite—done.** The floem fork is gone; the engine is a clean slate
  with the same capabilities—signals, memos, effects, owners, context, batching, the
  sync/local storage split, stores, tagged snapshots—designed around our own mechanics
  (eager staleness marking, non-reentrant flushing, memo change-filtering) and speaking
  the ecosystem's standard reactive names: `Signal`/`signal()`, `Owner`,
  `provide_context`/`use_context`, free `batch`/`untrack`, `Effect::watch`, `Reaction`,
  `Slice` (in-place `Fn(&mut T, &O)` setters), `OwnedSignal`, and the
  `Get`/`With`/`Read`/`Set`/`Update`/`Write`/`Track` access traits with
  `ReadGuard`/`WriteGuard`. Phase 1's leftovers landed with it: local entities live in
  one generational arena per thread (index + generation, slot reuse through a free
  list), sync signals in a generational slab in the process-wide registry, and every id
  carries its realm and its runtime's nonce—a stale, foreign, or pre-reset id resolves
  to nothing. Engine-internal machinery (`EffectTrait`, priorities, current-effect
  manipulation) left the public surface. Standing constraints held: signal identities
  stable across runs via tags, serializability opt-in, the UI-thread designation
  resettable under the `test` feature.
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
- **Panic hook for TUI—done.** `launch` installs a hook that restores the terminal
  (cooked mode, main screen, mouse capture off, cursor shown) before the panic message
  prints, so a crash never leaves the terminal wedged; normal unwind still restores via
  the raw-mode guard.
- **Titles and head management**: **web done**—`head::set_title`/`derive_title` plus
  `add_meta`/`add_meta_property`/`add_link`/`add_stylesheet`/`add_script`, serialized
  into the SSR/SSG `<head>` (escaped, registration order) and applied live under CSR,
  where an identical element left by a server render is adopted, not duplicated.
  Remaining: the TUI title via OSC 0/2 and the native window title.
- **Light/dark detection in TUI—done.** `launch` queries the terminal background via
  OSC 11 (falling back to `COLORFGBG`, then to dark) and the `DarkMode`/`LightMode`
  condition leaves mirror the GUI vocabulary; `trigger_color_scheme` drives them in
  tests. Remaining: live change notifications (DECSET 2031) instead of the one launch
  query.
- **Truecolor detection in TUI—done.** `COLORTERM`/`TERM` name the depth; paint
  quantizes to the xterm 256-color palette or the 16 base colors when truecolor is
  absent, instead of assuming it.
- **The driver owns the platform's defaults.** The goal: nobody above a driver ever asks
  whether they are cancelling browser behavior. The driver holds the policy—per event
  kind and node kind—and cancels statically wherever *we* take the event over; above it
  only bubbling exists, and since a view that handles an interaction consumes it, even
  that never reaches a signature. `Prevent::Platform` becomes web-driver-internal.
  - **Scoped deliberately.** Scrolling stays the platform's: it is compositor and layout
    work, cancelling `wheel` forces non-passive listeners that degrade every scroll on
    the page, and a main-thread reimplementation forfeits momentum, overscroll and
    chaining. Text entry stays the platform's for the reasons already settled—caret and
    word motion, the selection model, IME, autofill, the undo stack. The `Web*` controls
    are out of scope by construction: they exist for the scriptless render and are
    replaced by the drawn control at hydration, and taking over events needs script
    anyway.
  - **Known limit.** The driver decides statically, from the event and the node. A case
    needing "cancel only when the handler took some branch" cannot be expressed; if one
    turns up, that is the signal to reconsider rather than to widen the policy quietly.
- **Retired before it shipped: a handler-side override.** `Override`/`ControlFlow`
  briefly let a user take an event from a control. It was cut: no first-party code
  wanted it, and it taxed every ordinary handler—`ControlFlow::Continue(())` spread
  through the counter, the gallery and a dozen tests—because the orphan rule blocks
  `From<()>` for a foreign type, so the common case could not stay implicit. If
  overriding a drawn control's keys is ever genuinely needed, bring it back as
  `override_<event>` on the controls that compose behavior, with a **local** enum so
  `()` keeps meaning "just observe".
- **Theming: one wrapper view, light and dark—done (web + TUI).**
  *Settled the earlier open question—derivable properties on every view versus a
  light+dark pair under one provider. It is the provider.* The shape that landed, after
  one wrong turn recorded below: a **design token is a value**, and a palette belongs to
  the component library that defines it.
  - **`Adaptive<T>` is the whole mechanism.** A base plus conditional overrides
    (`SmallVec<[(Condition, T); 1]>`—a scheme-following token is a base and exactly one
    variant), generic over the condition vocabulary each facade aliases away, since the
    terminal's conditions are cells and its own background. Every color and border setter
    takes `impl Into<Adaptive<_>>`, so `set_color(Color::WHITE)` and
    `set_color(palette.card)` are the same call; setting one fans its variants into the
    slots the view already carries, and **nothing new reaches a driver**. Under a
    condition of the setter's own, `set_color_when` *narrows* the token's conditions
    (`Hovered ∧ DarkMode`) rather than dropping them—`narrowed` is the one place a
    two-slot inline vec is right, since narrowing turns a base and its variant into two
    variants. `Composable` grew `dark_mode()` and `both()` to make that generic.
  - **`skene-simple` owns `SimplePalette`, and only owns it.** A struct of `Token`s—core's,
    since a design token belongs to no one library—with `set_<prop>` and
    `set_<prop>_when(condition, value)` per field; the crate holds no machinery a
    third-party library could not write for its own palette type, and
    `SimplePalette::default()` is built from those same public setters.
    `SimpleTheme::new(palette)` provides one through context. A token carrying a
    `DarkMode` variant compiles to a `prefers-color-scheme` media query, so a
    server-rendered page answers the reader with **no bundle loaded at all**.
  - **Retired: `SimpleTheme::adaptive(light, dark)` and the light/dark palette pair.**
    The theme took two palettes and folded one into the other as `DarkMode` variants,
    with a type-state (`OnePalette`/`TwoPalettes`) so only the paired form could be told
    which scheme to paint. It worked, and it made light/dark the one condition with
    machinery of its own—a palette could answer `MinWidth` only by reaching into a token,
    while it answered the scheme through a constructor. The per-property form removes the
    separation: `set_card_when(Condition::DarkMode, ink)` sits beside
    `set_card_when(Condition::MinWidth(1200.), wide)` and neither is privileged.
    A whole-palette `when(condition, overlay)` was proposed as the replacement and
    **rejected**—it is the same privileging one level down, and it cannot express a
    conditional border on a look whose base has none. `Adaptive::schemes` went with it.
  - **Retired before it shipped: `theme_<prop>` setters.** The first cut put
    `theme_color(|palette| palette.card)` on `Container`, `HasDecoration` and the run
    views through three extension traits. It read well and it was wrong: the token
    closure was `Fn(&Palette) -> Color` with `Palette` concrete, which welded the
    framework's own box view to the Simple library's palette type and left a third-party
    library unable to use its own. Equal footing is the point of `LayoutBox` and the
    `Raw*` split; a themed setter surface cannot have it. Making the token a *value* the
    ordinary setters accept deleted the whole surface, three traits, `Palettes`,
    `SchemeCondition` and two applier functions—the replacement is smaller than what it
    replaced.
  - **The theme's child is a closure.** A palette is read from context, and an argument
    is evaluated before the call it is an argument to, so a child passed by value is
    built before the theme provides anything. Found by the first test written against it.
  - **A scheme the app controls—done, and it is not a rebuild.** `derive_scheme` was
    first built on `derived` and cut before it shipped: a rebuild destroys the old nodes
    and disposes their scope, measured rather than assumed—a signal inside the themed
    subtree went `1` → `0` across a flip, and focus, scroll offsets and native input
    state went with the nodes. A theme toggle that empties the form under it is worse
    than none. What shipped instead tells the **driver** which scheme is in force and
    touches nothing:
    - The web compiles every scheme variant twice—the media query, guarded by
      `:where(:root:not([data-skene-scheme="light"]))` so an explicit choice wins, and
      that choice as `:where(:root[data-skene-scheme="dark"])`. `derive_scheme` writes
      the attribute (and `color-scheme`, so the user agent's own widgets follow) from an
      effect. **Both guards sit in `:where()`**, which was not a detail: contributing
      specificity would have made a dark variant beat a `:hover` rule that is more
      specific in intent.
    - The terminal keeps the choice beside its detected background and answers both
      through one `State::dark()`, so conditions, glyphs and eagerly-resolved colors all
      follow it, and the frame is simply marked dirty.
    - Pinned by regression tests on both facades: the same node, the same local state
      (`1`, not `0`), and one build of the subtree across a flip.
    - **A server-rendered page preempts its own flash.** A reader's choice lives in
      their browser, which a render cannot see, so the markup carries the
      platform-following form and a page that offers a choice emits one inline blocking
      script in its head—the driver's own text, reading the driver's own key—that applies
      the stored scheme before the first paint. Inline and blocking is the whole point: a
      deferred script runs after the page has been painted in the platform's scheme,
      which is the flash. `stored_scheme()` reads the same key, so an app initializes the
      setting it binds from the choice the page was already painted in, and a page that
      never offers a choice carries no script at all.
    - **A chosen scheme is the document's**, since the platform's own is—so
      `set_scheme(Option<ColorScheme>)` and `derive_scheme` are facade functions rather
      than theme methods, and `None` returns to following the platform. Nesting themes
      nests palettes; the scheme decides which value each of their conditional tokens
      takes.

- **Withdrawn: a scheme override that scopes to a subtree.** Investigated, and the
  requirement was dropped rather than met—an app has one scheme in force, the way the
  platform does, and a palette is what nests. Recorded so it is not reopened as a bug:
  the *choice* half scopes for free once the attribute moves off the root
  (`:where([data-skene-scheme="dark"]) .c` matches any ancestor), and the **guard** is
  the hard half, since the preference rule must apply only where no ancestor chose the
  other scheme and no combinator says "no ancestor has X". The three candidates measured
  were an inherited custom property gated by `@container style()` (Firefox has no style
  queries, and the scriptless path must keep working), `light-dark()` plus inherited
  `color-scheme` (cleanest and largest—it wants a token's two values in one declaration,
  which cannot express a scheme variant combined with `:hover`), and emitting both scopes
  and letting source order decide.
  A related fear was checked at the same time and is **unfounded today**: the view tree
  and the DOM tree do not diverge. Every view attaches its child to its own node and the
  annotating wrappers realize none, so the DOM is a contraction of the view tree;
  overlays are `<dialog>`/popover promoted to the top layer *in place*, not portalled,
  and only the `<head>` is a real portal. It is not a guarantee to build on, though—a
  native driver may realize an overlay into its own layer.
  - **An overlay carries no theme of its own**, as settled: it is layering, anchoring and
    dismissal. Both galleries dress a modal by putting a `SimpleCard` **inside** the
    overlay, which needs no new view—the web driver already strips the dialog's
    user-agent chrome. A `SimpleOverlay` convenience remains available to add later.
  - **Two places a variant cannot reach, both terminal-only**: the picker popup's whole
    `BoxStyle` and the highlight's color pair take no conditional twin, and a `Span`
    carries no variants. Those resolve eagerly through the public `painted(&token)`,
    which answers the scheme leaves alone—a component library faces this exactly as the
    built-ins do. The GUI facade has no such gap.
  - **Inherited text became scheme-aware to carry this**: `TextTheme` grew `set_when`,
    the inherited variants layer under each run's own, and an inherited variant never
    inks a link—matching the base style's existing rule. The terminal needed none of it:
    a terminal's default foreground already matches its background.
- **Asset system**: declared in `skene.toml`—the framework manifest, which will grow
  further settings over time (Flutter's pubspec is the model)—never in code and never
  statically checked. Declared files and directories become reachable at predictable
  URIs; the CLI serves them in dev, copies them into web/SSG output, and bundles them
  for native. A library crate's own `skene.toml` (found beside its `Cargo.toml` via
  cargo metadata) contributes assets the same way an app's does.
- **Font assets**: declared and copied like any other asset, from an app or a library
  crate alike, and reachable at the same predictable URIs. What a font adds on top is
  **generated metadata**—per-face ascender, descender, line gap, cap height, read at
  build time—because the platforms need measurements the running text stack does not
  hand them. The web needs them to emulate text-box trimming where an engine has not
  shipped it and Chrome has; a native driver and the TUI need the same numbers for
  their own layout. The metadata is generated output, so it is a build-time artifact
  beside the copied file, never something an author writes or keeps in sync by hand.
- **View transitions (web)**: best-effort—`document.startViewTransition` wrapping SPA
  route changes and `@view-transition` in generated CSS for MPA/SSG navigations,
  degrading to instant swaps where the browser lacks support.
- **Only `Container` is decorated; layouts lay out.** The Flutter split, settled after
  weighing the alternatives: `Container` is the one freely-stylable box—decoration,
  sizing, events, and a single child—while `Row`/`Column`/`Grid`/`Stack` carry layout
  properties and nothing else. **Done on both facades.**
  - **The forwarding problem dissolves.** With one decorated view there is nothing to
    forward: `HasSize`/`HasDecoration`/`HasEvents` are deleted and their methods are
    inherent on `Container`. `HasFocus`/`HasStyle` survive—orthogonal to this split,
    since every `Simple*` still wraps a `Raw*`—and go if the control redesign below
    lands. The earlier `BoxSurface` derive and its exported vocabulary macros are gone
    with them; the internal all-fields node builder is `pub(crate) BoxNode`.
  - **Equal footing is kept by `LayoutBox`**, public and carrying the full layout
    vocabulary over a child-kind parameter, so a third-party layout view is built from
    it exactly as `Row` is built from the node. A test in each facade builds a `Toolbar`
    on it—typed `FlexChild` children, a `Flexible` wrapper, its own spacing knob—and
    matches `Row` cell-for-cell in the terminal, node-for-node against the recording
    driver on the GUI side (which grew a `record::layout` accessor for it). The one
    asymmetry to note: the
    built-ins write their properties straight into the private node, so their forwarding
    is a line per property against the public route's three.
  - **Responsiveness moved to per-property variants.** `set_spacing_when(condition,
    spacing)` joins `set_spacing`/`derive_spacing`, so a layout keeps declarative
    responsiveness—compiled to CSS media queries on the web, with no JS—without carrying
    a whole `BoxStyle`. It also fixes a leak: a whole-style variant could set properties
    the view does not honor, silently doing nothing, where a per-property setter exists
    only where the property does. Storage is unchanged (each pushes a one-property
    variant), so the driver mapping is untouched. Accepted cost: a condition affecting
    two properties is now two calls.
  - **The fill rule, verified in real frames.** `Container` realizes as `Layout::Single`
    —one `1fr` track each way—so a sized container fills its child to the content box
    and an unsized one hugs it. Two findings the spike turned up, both now pinned by
    tests: `minmax(0, 1fr)` reports a *zero* max-content contribution, so an auto-sized
    parent measures the container as empty—plain `1fr` keeps the intrinsic contribution.
    A scrolling container needs no `align-items: start` of its own: the automatic minimum
    already leaves the child at natural size, and the branch that set it was dead, since
    the sizing pass reassigns `align-items` from the style right after.
  - **`Container` gained `set_alignment`**, the two-axis placement of its one child. The
    single-cell grid gives it for free, and without it centering one child would mean
    wrapping it in a `Row` for `item_alignment`.
  - **What layouts keep besides arrangement: padding**, since a layout's inset from its
    own children is distinct from a wrapping container's inset from the layout. Overflow
    stayed with `Container`—it is the box that carries a size, so it is the box that
    scrolls.
  - **A required child is a type-state—done.** `Flexible`, `Pinned`, `GridCell`, and the
    themes exist only to wrap something, so the child is not optional: each is
    `Wrapper<C = NoChild>` holding its child *as* the parameter, and `set_child`
    exchanges `NoChild` for the view, returning `Wrapper<AnyView>`. Only that form
    implements `View`/`IntoChildOf`, so forgetting the child is a compile error rather
    than an empty render—no `Option`, no placeholder, no unsafe, and the property
    setters stay generic over the state so ordering is free. The error explains itself:
    `IntoChildOf` and `IntoChildren` carry `#[diagnostic::on_unimplemented]`, and
    `#[diagnostic::do_not_recommend]` on the blanket `impl<V: IntoView> IntoChildOf for V`
    keeps the blame on the child bound instead of burying it in `View`—which also makes
    rustc name the missing piece, "expected `AnyView`, found `NoChild`". The wording
    stays generic ("complete") so a later required property reads the same way. Only
    traits that appear as *argument or return bounds* are worth annotating: a missing
    trait method is `E0599`, which never consults the attribute.
  - **Nothing remains here.** The control redesign it used to carry outgrew the item and
    is its own entry below.
- **Controls: draw our own where it pays, keep the platform's where it does not.** The
  `Raw*` controls stop being thin wrappers over native elements and become generic
  builders that return any view, supplying only accessibility, focus, and event
  handling; on the web they carry ARIA roles on their own nodes instead of the native
  void elements. `Web*` become separate views built against the browser's own controls,
  exposing exactly the customization that is deliverable cross-browser. `Simple*` build
  on `Raw*`. `HasStyle`/`StyleCore` die either way: a control's decoration is spelled
  the way `Container`'s is, per property, not as a foreign `BoxStyle` blob.
  - **The split is by where the cost falls, and it inverts cleanly.** Custom for
    `RawSlider`, `RawCheckbox`, `RawRadio`, `RawPicker`: native styling is worst exactly
    there—the slider needed vendor pseudos and a custom property to paint a filled
    run—while the platform residual is smallest, ARIA plus a form value. Native stays
    for `RawTextField`/`RawTextEditor`: a text box already styles fine through the box
    vocabulary, and the residual there is the whole of IME composition, autofill,
    spellcheck, the undo stack, and mobile keyboard selection. Nothing we write is going
    to be more correct than the platform at text entry, so we do not write it.
  - **The interaction machinery is shared, not rewritten—done.** A terminal has no
    native controls and already draws its own, so the logic exists; `SliderRange` and
    `Ring` in the spine hold the medium-agnostic part—value from a normalized fraction,
    stepping, wrapping movement through a group—and each facade converts its own cells
    or pixels at the edge. This already removed two live duplications: the slider
    fraction math had a copy in the terminal state and another in the web driver's fill
    refresh, and the wrapping cycle was written twice inside the terminal alone.
  - **The terminal's text machinery stays in the terminal.** Caret and word motion and
    the selection model are not hoisted: one consumer means no duplication to remove,
    and the spine takes no platform's concerns it does not need.
  - **The four drawn GUI controls—done.** `RawCheckbox`, `RawSlider`, `RawRadio`/
    `RawRadioGroup`, and `RawPicker`/`RawOption` are built from the public surface a
    third party has: `Container` for the node, role, focus order and reported state,
    `Overlay` for the picker's popup (layering, anchoring, light-dismiss), `Ring` and
    `SliderRange` from the spine, and context for group membership—so a member may be
    wrapped or interleaved with decoration rather than forced into a typed child kind.
    Traversal differs by control because the platform's does: a radio group wraps, a
    picker's list clamps. Both roving tab stops move with the selection, which is why
    focus order became reactive.
  - **The picker's shape.** `set_face` draws it closed and `set_options` draws the
    list—properties first, the large one last—over `Stack[face, Overlay[Listbox]]`, a
    tree with no privileged driver call in it. Its handlers cover the face; an option
    carries its own, because roving focus puts the keyboard on a row while the list is
    open. **Type-ahead is the one deliberate gap**: resetting its buffer needs a timer,
    timers are userland, and the terminal's picker has none either.
  - **Membership is read from the tree.** A grouping control's members live in the
    spine's `Roster`, which orders them by walking the subtree under its root rather
    than by the order they joined—so a reconciling set stays in document order however
    it changes, and a destroyed member is absent because the walk cannot reach it. That
    added `TreeOps::children`, the seam's one query beside its four mutations. Every
    leaf answers it from the child list it already keeps—the driver's own state is the
    source of truth, and a platform's tree is a sink it writes to, never one it reads
    structure back from.
  - **Deferred, and why: `aria-activedescendant`.** Announcing the highlight without
    moving focus would need a node-valued `AccessProp` and stable per-node ids in the
    web driver. Roving focus reaches the same place with machinery that already
    exists, so the id-relation vocabulary stays unbuilt until something needs it.
  - **The no-JS path—settled, and the seam is in.** A `<noscript>` fallback cannot hide
    the ARIA tree beside it, so a scriptless client would get both the working control
    and an inert one. `Enhanced` renders the native `Web*` control without script and
    rebuilds into the drawn `Raw*` once the client owns the page: content is never gated
    on JS, and the client layers interactivity on top.
    - **The swap runs after hydration, not during it.** Hydration matches client to
      server by node id (`data-h`), and a drawn control allocates a different number of
      nodes than a native one—so rebuilding mid-hydration shifts every id after it, and
      a later node can silently adopt an unrelated element of the same tag. Reproducing
      the fallback first keeps the matching exact; the rebuild then runs in a tree
      nobody is matching. That needed no change to hydration itself, only a reactive
      `settled()` on each driver: false while the client reproduces the server's tree,
      true otherwise—so a fresh client launch never builds the fallback at all.
    - **Footprint parity—done, and it found a real bug.** The two forms must claim the
      same box or the swap shifts layout. The Simple controls give their scriptless form
      the drawn one's metrics, and a browser test measures both: it caught the drawn
      control stretching to the full line as a flex item where the browser's own control
      hugs, so a drawn control now sizes to what it draws. Only a real browser answers
      this—whether a native `<input>` honors a given width is not something the host
      tier can tell us.
    - **The toggle—done.** `set_scriptless_fallback(false)` makes every enhancement
      render its enhanced form everywhere, server included. It also makes hydration
      cheaper: both sides build the same tree, so nothing is built to be replaced a
      moment later. On by default, since progressive enhancement is doctrine.
    - **Matching the two forms is the library's job, not the framework's.** The `Web*`
      controls keep a styling surface for exactly this; nothing automatic tries to keep
      a fallback and its enhanced form in step.
    - **The swap can flash, and how much is a styling question.** A page that renders a
      fallback repaints once when the client takes over, so a fallback styled unlike its
      enhanced form will visibly change. This is more tractable than it looks: the web
      driver already emits `appearance: none` resets (`TOGGLE_RESET`, `SELECT_RESET`),
      so a native checkbox, radio, or range is de-chromed and takes a background,
      border, and radius freely—a library can bring the two to near-identical rather
      than merely the same size. The residue is what a stylesheet cannot reach: the
      check glyph and the radio dot (a mark drawn by the engine, not a child we own),
      and a native select's open list—though that one never coexists with the swap,
      since a closed face is all a page shows at load. An app that wants none of it
      turns the fallback off. Worth revisiting if the residue ever looks worse than the
      alternative: emitting the fallback hidden and revealing it only if script never
      arrives costs a flash of *nothing* instead, which may or may not read better.
- **Component libraries**: third-party crates shipping components—views plus
  facade-specific code plus assets—consumable on equal footing with built-ins. Depends
  on the asset system for the assets half; the realize-once vocabulary already permits
  the code side, and the forwarding item above is what makes a third-party view as
  capable as a built-in one.
- **Serializable reactive state**: the signal graph snapshots to a byte stream and
  restores on startup. **Direction settled: on by default, opt-out**—the built-in and
  simple types restore with no tag and no feature, an explicit constructor opts a signal
  out, and a tag is only an override for the positional diff (see the two exploration
  items below; the opt-in-only shape below it is what landed first, not the target). The
  snapshot also carries the extras that
  live outside the graph—the focused node, scroll positions, and similar platform
  particularities—which the drivers fully own and can replay. Powers state-preserving
  dev reloads (full restart, no hot-patching) and stands on its own (session restore,
  crash recovery). **Core landed:** the opt-in `snapshot` cargo feature (forwarded by
  both facades) gives `Signal::tagged("tag", default)`—the tag is the identity across
  runs, so the arena redesign is decoupled—plus `snapshot()`/`restore()` over
  serde+postcard: staged values apply exactly once on creation, failed decodes degrade
  to the default, corrupt snapshots stage nothing, duplicate live tags panic.
  Remaining: driver extras riding along (focus, scroll), and the CLI holding the
  snapshot across dev reloads.
- **Explore: positional store/restore, tags as an override.** Tagging every signal by
  hand is the price of restoration today, and a tag is a process-global key: `snapshot.rs`
  holds one flat map from `&'static str`, so a duplicate live tag panics no matter how far
  apart the two declarations sit. Target shape: a signal's identity comes from *where* it
  was created and *what type* it holds, a tag is an explicit override that only
  disambiguates where the automatic match would go wrong, and a tag needs to be unique
  only within the function declaring it.
  - **Identity.** Candidates, cheapest first: `#[track_caller]` creation sites
    (file+line+column—stable until an edit moves the line, which is precisely when the
    diff must work); a proc-macro stamping a per-function id plus a creation counter; or
    the owner tree, where identity is the path of child indexes from the root owner plus
    the index within the owner. The owner path looks strongest—the engine already builds
    it, and it survives edits that don't change the tree's shape—but it must be checked
    against conditional and looped creation.
  - **The diff.** Per owner (or per function invocation) both sides are a sequence of
    entries keyed by (type, optional tag); align them, restore the matches, and let
    additions and deletions fall to the default. Myers LCS is the obvious tool, but
    **patience diff fits better**: tagged entries are exactly patience's unique anchors,
    and untagged runs align by index between anchors. Per the rule that makes this
    tractable, a tag present on only one side matches nothing—adding or removing a tag is
    a delete plus an insert, and the untagged neighbours realign around it. A true
    conflict (incompatible type at a matched position, or an ambiguous alignment) resets
    to the default, which is also what a failed decode already does.
  - **Serializability, and the end of the `snapshot` feature.** Store/restore becomes
    on-by-default and opt-out: the built-in and simple types just work, with no tag and
    no feature to remember. So `Signal::new` carries `Serialize`/`Deserialize`
    unconditionally and the `snapshot` cargo feature is deleted—it could never gate the
    bound anyway, since features are additive and a crate compiling alone would break the
    moment anything in the graph enabled it. The opt-out is an explicit constructor for
    values that cannot or should not round-trip (`Signal::transient` and friends—name to
    settle), which asks nothing of `T`; it also covers the type that is serializable but
    whose restored value would be wrong, like a live handle. Non-obvious consequence to
    design for: with restoration on by default, a stale snapshot reaching a changed
    program is the normal case, not the exception—so the diff's conflict-resets-to-default
    rule and the failed-decode degrade path carry the whole burden of correctness, and
    both need tests that edit the program between snapshot and restore.
  - Must keep working alongside it: `Store::tagged` (a store snapshots as one unit),
    `Resource::tagged` and its order-keyed default, and the dev-loop `/skene:state`
    protocol.
- **Explore: optional keys on every view, so driver extras restore the same way.** Scroll
  offsets ride the snapshot keyed by build-order node id and focus does not restore at
  all. If a view can carry an optional identity—by index by default, an explicit key
  overriding—then focus, scroll offsets, caret and selection positions, and open popup
  state all restore through the same alignment as the signals above, and restoration
  becomes one algorithm instead of one per extra. `set_key` is the name to weigh
  (Flutter's `Key`, SwiftUI's `.id()`, Compose's `key()`), and it should be the concept
  `reconcile` keys lists by rather than a second one. What the exploration has to answer:
  whether kind-plus-index is stable enough across conditional subtrees, what the
  per-node identity costs when nothing restores, and whether asking users to key views is
  tolerable devex or a sign the whole idea belongs only to the extras the drivers own.
  **Focus restoration ships either way**, exploration or not, and comes first: the
  terminal's snapshot provider carries scroll offsets alone, so a dev-loop restart lands
  scrolled where it stood with nothing focused—the caret, an open picker, and the tab
  position all lost. Restoring the focused node by the same build-order key scroll
  already uses is the immediate work; the keys then generalize it. The web leaf owes both
  halves—it registers no provider at all, so a dev reload there restores neither focus nor
  scroll.
- **Accessibility**: AccessKit on native (already in phase 8's shared machinery) and in
  the TUI where it can reach; on the web, `Role` expansion and semantic output audited
  against ARIA. A stated commitment across facades, not an afterthought.
- **Reactive stores—done.** `Store<T>` owns one plain value; `#[derive(Store)]`
  generates a `<Name>StoreFields` trait (blanket-implemented over `StoreField`, so
  accessors compose through nesting and collections) whose lenses subscribe at field,
  `Vec`-index (`.at`), and map-key (`.key`, `HashMap`/`BTreeMap`) granularity. A write
  notifies exactly what it can affect—the location, its ancestors, its descendants—via
  a lazily-built trigger trie; the value stays a plain struct, so its methods work and
  `Store::tagged` snapshots it as one unit under one tag. Deriving requires a direct
  `skene-reactive` dependency (the generated code names it). Remaining: derived
  (memoized) lens reads, generic structs in the derive, and a keyed-iteration helper
  feeding `reconcile`.
- **Error/suspense audit—done; fixes pending.** What holds today: `Boundary` counts a
  subtree's tasks through `Context`; `Deferred` streams around it on the server, and a
  boundary that cannot settle (nothing ready, nothing outstanding) flushes what it has;
  `Resource` keeps failure in the value (`Result` payloads); hydration mismatches are
  loud; a trapped server render surfaces as the runner's `Err`. The gaps, each its own
  work item:
  - **Client suspense—done.** `Boundary::is_settled` now tracks, and a shared
    `Suspense` view lives once in core, aliased by both facades: the content builds
    eagerly under a fresh boundary (its resources and tasks start right away), the
    fallback holds the screen while that boundary is pending, and the whole subtree
    counts as one pending unit toward an enclosing boundary—so a server render and
    the stream settle through it to the content. Hydration treats a marker the page
    lacks as a quiet fresh build (a detached fallback is legitimate); tag
    disagreements and never-adopted server elements stay loud.
  - **ErrorBoundary—done, on probation.** A deliberately thin `Errors` handle in
    `Context`, mirroring `Boundary`: captured at build, reported into from handlers
    and tasks, cleared to restore the content as it was; the `ErrorBoundary` view's
    fallback receives the same handle and rebuilds as the held errors change. Failure
    carried in values stays the default—this exists only for errors with no render
    site of their own, and panics are never caught. Kept experimentally: if
    per-resource matching keeps covering real apps, delete it.
  - **Transition semantics—done.** `DerivedView::transitional` (core `transition`):
    each rebuild runs under a fresh boundary and stays detached until it settles, the
    previous content holding the screen; the latest rebuild wins; an unsettled rebuild
    counts toward the enclosing boundary, so a server render and the stream settle
    through it; a `Suspense` covers the first load, which has nothing to hold.
  - **TUI async—done.** The terminal facade gained a local executor pumped from the
    event loop (wakers may fire from any thread through a shared readiness queue; the
    futures stay on the terminal thread), `spawn` over the spine's shared
    `scope_bound` wrapper—which now also backs the GUI facade's `spawn`—and a
    `Resource` mirroring the GUI one minus the server crossing, with a `pump_tasks`
    test seam.
  - **Resource adoption—keyed, done.** `Resource::tagged("tag", loader)` adopts and
    serializes by tag—unique per render; a duplicate panics the server render—and the
    hydration state carries the order-keyed entries plus the tag-keyed ones sorted by
    tag. Order-keyed `new` stays the default, its contract documented.
  - **Render deadline—done.** `web::set_render_deadline` bounds every render's
    settling (ten seconds by default): passing it—or the polling bound—stops the wait,
    logs what stayed alive (tasks, outstanding host operations), and serializes the
    tree as it stands, placeholders included; the reactor's block wakes at the
    deadline instead of waiting forever.
  - **Hydration mismatch—resolved.** The audit's "fatal" claim was wrong: a miss
    always built a fresh element. The diagnostics are now split—an absent marker
    builds fresh quietly (legitimate for server-detached subtrees), a tag disagreement
    and never-adopted leftovers stay loud—which is the production story.
- **All basic controls**: checkbox, picker, text editor, radio, slider, and friends—
  unstyled by default with full keyboard and ARIA behavior; a styled set may ship as a
  separate optional package.
- **List virtualization—done (web + TUI).** `LazyColumn` on both facades: a scrollable
  column of fixed-extent, index-keyed items, only the visible slice plus an overscan
  margin built (over `reconcile`), the rest carried by two derived spacers; the window
  math lives once in core (`virtual_window`). The web measures the viewport with a
  `ResizeObserver` and SSR serializes the overscan margin; the TUI reports viewport
  sizes after layout. Remaining: variable-extent items, horizontal lists, and the
  native realization with its driver.
- **Internationalization**: first-class eventually—message catalogs, locale-aware
  formatting, reactive locale switching.

Standing notes, attached to no step:

- **The children-last rule could ship as a lint.** Every view takes its children through
  a builder method so a chain ends with them, and `AGENTS.md` states the ordering as a
  convention; nothing enforces it. Rust has no stable way to ship a lint that runs under
  plain `cargo clippy`—the ecosystem answer is a **dylint** crate users invoke with
  `cargo dylint`, at the cost of pinning an exact nightly (it links rustc internals) and
  asking users to install the tool. Worth remembering, not worth scheduling.
- **`IntoChildren`, `IntoChildOf`, and `IntoView` cannot be replaced by std traits**—
  checked against the compiler, so the experiment need not be repeated. A blanket
  `impl<I: IntoIterator<Item = V>> IntoChildren for I` conflicts with the tuple impls,
  and the same conflict appears with `From` in place of the custom trait: rustc refuses
  both because an upstream crate *may* implement `IntoIterator` for tuples. Tuples are
  not optional—children are heterogeneous, so arrays cannot carry them—which makes the
  custom trait the only shape that accepts both. `Into<AnyView>` cannot replace
  `IntoView` either: `impl<V: View> From<V> for AnyView` collides with the reflexive
  `From<T> for T` because `AnyView` is itself a view. Separately, a `FromIterator` for
  `Children` would be unchecked as things stand: `Children<Ops>` is parameterized by the
  driver ops and erases the child kind at construction, so collecting into it would
  admit a `Flexible` into a stack—making it meaningful would mean threading the kind
  through as `Children<Ops, Kind>`, visible in every facade alias.

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
  (`web`—CSR and static alike, the folder replaced wholesale each build—and `tui`;
  `ios`/`android` when the native family arrives).
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
  A failed rebuild lands in the running app as a dismissable modal—the CLI holds the
  compiler error at `/skene:notice`, the app polls it and presents it over the content
  (Escape dismisses; a fix clears it), falling back to a raw print when nothing polls.
  Scroll offsets ride the snapshot beside the tagged signals, through the reactive
  engine's `provide`/`staged` seam, keyed by build-order node id—so a restart resumes
  scrolled where the last instance stood, restored offsets replaying through the
  scroll handlers so a lazy column rebuilds its window at the resumed position. Both
  verified under the pseudo-terminal.
- **Wasm-split lazy loading—done.** `#[lazy]` marks an async view function whose body—
  and everything only lazy bodies reach—splits out of the main wasm into its own
  module, fetched on the first call; the `Lazy` view renders one (`Lazy::new(about)`),
  pending toward the enclosing boundary so `Suspense` fallbacks and server renders
  compose. The macro and the `skene_gui::split` runtime speak the `wasm-split-cli`
  symbol convention (the split tool is embedded as a library; the CLI emits the
  modules, shared chunks, and the `__wasm_split.js` loader manifest beside the
  bindings), split-off code gates to the browser leaf alone—the server leaf and every
  other target call the body directly—and apps opt in with `[web] split = true` in
  `skene.toml`, which flips the app compile to relocation-keeping flags; a build
  carrying `#[lazy]` without the opt-in fails with the fix named. Verified end to end
  on the counter example, debug and release: boot without the module, one fetch on
  first open, none on reopen.
- **Progressively enhanced static output—done.** `build web --static` ships the client
  bundle beside the pages (split modules included) and every page boots it: the markup
  stands on its own—content works with JavaScript disabled—and a capable client
  hydrates it into the running app. `launch` recognizes a server-rendered page by its
  embedded state script and adopts it in place, so the one `#[main]` entry covers CSR
  and hydration alike. A `Lazy` view hydrating over server-rendered content claims it:
  the markup stays visible, already satisfying the enclosing boundary, until the module
  arrives and the real content replaces it. Verified on the static counter build:
  content in the HTML, hydration adopts without rebuilds, the lazy module fetches after
  boot and swaps invisibly.
- **Remaining here:** richer `skene.toml` settings as they earn their place; on a
  hydrated page with several lazy views racing, order-keyed resource adoption inside
  them can cross—tags already cover it, ordering the arrivals would too.
- The dev loop is watch + rebuild + full reload; state survives via the serializable
  reactive state in the backlog—the CLI holds the snapshot across the restart.
- `skene.toml` is read here: the manifest-declared assets (see the backlog) are served
  in dev, copied into SSG output, and collected from library crates' manifests across
  the dependency graph.

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


## The redesign—one spine, thin facades (in flight)

A full audit of every layer (reactive engine, spine, composition, controls, drivers,
theming) settled a redesign whose target is in `DESIGN.md` and whose doctrine is in
`AGENTS.md`. The public vocabulary stays; four of the five layers beneath it are
rebuilt. The structural move: core gains a retained scene and a zero-sized `Driver`
trait, `skene-views` hosts every built-in view once, generic over the driver, and the
facades shrink to a scalar, a condition set, a style extension and aliases. Phases are
ordered by dependency; R1 and R2 are independent and run in parallel. Every phase leaves
the workspace green on all three tiers.

### Defects the audit found

Independent of the redesign; each is wrong today and each phase below names the ones it
closes.

- `derive_child` never rebuilds: it builds its closure untracked through `reconcile`
  (verified with a throwaway host test). R3.
- A derived property is an inline style on the web and outranks every variant; the
  terminal overlays variants over it. R4.
- A GUI `RawButton` with a plain handler yields `Prevent::None` and bubbles to its row;
  the terminal's returns `Bubbling`. R6.
- Every drawn Simple control styles `Condition::Focused` on a child of the focusable
  node, so no focus ring ever renders. R6.
- Hydration injects the reset and one `<style>` per class beside the server's block:
  two resets, two copies of every rule. R4.
- Streaming renders styles once at the shell; a `Deferred` boundary introducing a new
  class streams unstyled forever. R4.
- `Suspense` and `ErrorBoundary` never destroy the branch they detached. R3.
- A memo is queued as an effect and recomputes unconditionally; a read inside a batch
  pulls it fresh and the flush computes it again; unobserved memos recompute. R1.
- `batch` and `Owner::enter` have no drop guard; a panic poisons the runtime. R1.
- `set_sync_effect_waker` has no caller; off-thread sync writes never drain. R1.
- `Roster` is read untracked inside a focus-order closure and members join after they
  build; a radio group with nothing chosen has no tab stop. R6.
- The recording driver runs only the target's click handlers, does not unlink on
  reparent, and numbers nodes from 0. R2.
- Class naming depends on intern order and liveness; rule order differs between the
  browser and the server. R4.
- A group node is a real `<div>` and a real taffy node, so a derived subtree in a `Row`
  is one flex item and a wrapper around it annotates the group. R2.
- The terminal calls `set_style` on every node every frame: one dirty bit is a full
  relayout and repaint. R5.
- Three control doc blocks describe the retired handler override as shipped. R6.

### R1—Reactive internals ✅

Public API unchanged, with additions (`EffectHandle`, `Effect::render`, first-class
`watch`, `Memo::new_with`, `Selector`, `Runtime::set_scheduler`/`flush_sync`).
Internals rewritten: split arenas, intrusive doubly-linked dependency edges with reuse,
two-color propagation with a dependency-ordered effect queue (no sort, truly lazy memos,
one compute per flush), drop guards on `batch` and `Owner::enter`, no user code under a
runtime borrow, the `test` feature no longer changing a type, the sync waker routed
through the scheduler.
**Gate:** the reactive suite passes with the eager-recompute test inverted; new tests
count memo computes in a batch (one), survive a panic in `batch`, dispose and pause an
effect, order render effects before ordinary ones, exercise `Selector`, coalesce N
writes into one scheduled flush, and drain an off-thread write through the scheduler;
criterion numbers recorded before and after.

### R2—Scene, driver trait, property model, on the headless driver ✅

Landed as `skene_core::next` (the model, the scene, `Driver`, dispatch, `View`/`Children`/
`derived`/`reconcile`/`Show` over anchors), `skene-headless` (the whole trait plus the
test seams) and a `skene-views` skeleton (`Container`, `Column`, `Text`, `Flexible`,
`Theme`), with the nine gate tests in `packages/views/tests/headless.rs`. Two notes for
R3: a token set with no theme above does not yet register its defaults at the root, and
`Value<T, Media<N>>` does not yet convert into `Value<T, Condition<N>>`, so the
`Spacing::from(8.).when(Media::MinWidth(900.), 16.)` spelling needs that conversion.

R2b (the data model, additive under `skene_core::next`): `Scalar`, `Shape`
(`Declared`/`Static`/`Plain`), `BoxStyle<N, S, X>`/`RunStyle<S, X>` with visitors and
`overlay`/`split` written once, `StyleExtension`, `StaticLeaf`/`Leaf`/`Conditional`/
`Value`/`Definition` and the `When` extension, typed `Expr` (lengths and numbers with
ring arithmetic, colors with the named functions, nothing else), `Media`/`State`/
`Condition` with container queries, `TokenName`/`TokenRef`/`Token`/`TokenSet`/`Themed`.
R2a: `Scene<D>` with generational node ids, anchors and per-child annotations; the
`Driver` trait with associated `Sink`, `Scalar`, `Media`, `State`, `StyleExt`,
`TokenValue`; event dispatch in core over the scene; `skene-headless` implementing the
whole trait; a minimal `Container`/`Text`/`Theme` over `D` to exercise it.
**Gate:** headless tests pin the outline of a derived child (no extra node), a wrapper
annotation landing on the content of a derived subtree, a reconciled item's scope being
disposed, a click bubbling to a parent container and stopping on `Bubbling::Stop`, a
themed color resolving through a nested theme, a palette flip on the same node id with
local state intact, an expression over a token, and a container condition against a
supplied size; adding one property touches the struct, the visitor and the two mappings
only; `cargo hack --feature-powerset` passes on all three tiers.

### R3—`skene-views`, and the facades thinned

Every built-in view generic over `D`: `Container`, the layouts and `LayoutBox`, the
wrappers (annotating before creation), `Text`/`Span`/`RichText`, `Overlay`,
`LazyColumn`, `Suspense`/`ErrorBoundary`/`Deferred` on `Show`/`Switch`, `Theme`.
`Children` as segments with a sealed kind; `derive_child` on `derived`; `Show`/`Switch`
attach and detach pre-built branches. Both facades reduced to scalar, leaves,
extension, `Typography`, `media` and aliases; `skene-record` deleted.
Re-cut after R2: a facade can only rewire once its driver implements `Driver` and once
the controls its examples use exist, so R3 completes `skene-views` against the headless
driver, R6 (controls, also on the headless driver) runs next, then R4 rewires the GUI
facade with the web driver and R5 the terminal facade with its own. **R3 landed** apart
from `Overlay`, `LazyColumn` and `Deferred`, which wait for the drivers.
**Gate:** every built-in view exists once in `skene-views` with a headless test per view;
a `Suspense` swap preserves a signal inside its content; `Show`/`Switch` keep a branch's
state; wrapper annotations land on the content of a derived subtree; a token set used
with no theme above resolves its defaults from the root. The facade gates (no same-named
file in both facades beyond `lib.rs`, `prelude.rs` and the medium's own modules; the
galleries unchanged beyond the theming renames; a third-party crate under `examples/`
with a component library generic over `D`, a web view bound on `WebDom` and a terminal
view bound on `CellPainter`) move to the end of R5.

### R4—Web driver (in flight)

**Landed:** the shared compiler for the new model (`skene_web::next::compile`), the
server and browser leaves' `Driver` impls over one record per node beside the scene,
content-addressed class names (a 128-bit hash, no probe, interning-order emission on
both leaves; the old snapshot re-baselined once, names and order only), root delegation
routed through core's dispatch, hydration of the new tree with the server's stylesheet
adopted rather than duplicated, the reader-got-there-first value carry, `matchMedia`
signals, and browser and wasi tests for each. **Also landed:** the GUI facade rewired
onto `skene-views` (aliases over `ActiveDriver`, `Typography`, `media`, the satellites
ported, the counter, gallery and theme examples on all three targets, the old views and
`simple-views` feature deleted, `skene-record` dropped as a dependency), and the scene
no longer calls a driver under its own borrow (mirrors run after release; the browser
re-delivers an event that arrives mid-borrow). **Also landed:** the old `packages/gui/tests/web_*.rs` binaries
ported to the new facade (the `Enhanced`/`Web*` and transitional-derived tests deleted
with their subjects, the snapshot re-baselined once), streaming on the new tree
(`Deferred` registers its segment as a boundary, the walk pauses at its start anchor,
and every chunk opens with a `<style>` delta for the classes interned since the previous
flush, which hydration adopts with the head's block), server markers counting elements
in creation order as the browser does, and a stack placing its children in its one cell
on the web. **Open:** client-hint intake; scroll and focus restoration; deleting the old
`skene-web` vocabulary and `skene-record` once nothing builds on them. **Caveat to revisit:** on the
web a click on a native checkbox or radio is not dispatched as a click at all (the
platform toggles and `change` reports it, so one press is one flip), which means a
`handle_click` observer on a web toggle never fires; the headless driver dispatches it.
Either dispatch an observe-only pass on the web or document the divergence.

The `Driver` impl over one node record for both leaves; content-addressed naming with a
wide hash and interning-order emission on both leaves; a class cache keyed by the props
hash; one stylesheet after hydration; a delta `<style>` per streamed chunk; one handler
record per node, taken not cloned; scroll, enter and leave delegated; one shared
`ResizeObserver`; tokens as custom properties, expressions as CSS math, derived slots,
`@property` registration, scope and root templates, `@container`, control-state
selectors, `MediaQueryList` signals, client-hint intake through the runner, scroll and
focus restoration.
**Gate:** `web_style` re-baselined once and reviewed; a hydrated page holds one reset and
one rule per class; a streamed boundary introducing a new class renders styled with
script disabled; a derived base with a hover variant reads the hovered color; a palette
flip keeps `1`; a page rendered with a dark scheme hint serializes the dark value of a
derived leaf.

### R5—Terminal driver (in flight)

**Landed:** `Tui` implements `Driver` over the scene with one record per node, one
resolution entry point cached per node under a generation covering definitions,
environment and interaction, `set_style` only on change, `structure`/`layout`/`paint`
dirty flags, container queries in a second pass, input routed through core's dispatch
(the closed activation and arrow matches are gone), asynchronous launch detection, and
the facade rewired onto `skene-views` with `Typography`, `Glyphs`, `media` and the test
seams; both terminal examples ported; thirteen new render tests. **R5b landed:** the
old `render.rs` invariants ported by subject (`layout`, `conditions`, `text_entry`,
`controls`, `pointer`, `overlays`, `scrolling`, `selection`, `async`, `snapshot`,
`misc`), the hover path as a set, damage rows (the union of the changed nodes' last and
current rectangles, over the previous frame; `repainted_rows()` under `test`), inline
boxes inside `RichText` measured by their own layout, and the Simple controls' cell
shape through `SimpleShape`/`SimpleUnit` per scalar.

The `Driver` impl replacing direct state mutation; effective style cached under a
generation covering environment, interaction and definitions, with `set_style` only on
change; `layout_dirty` and `paint_dirty` split, then damage rectangles; the hover path
as a set; scrollables indexed; resolver, expressions, second-pass container resolution,
media signals; asynchronous launch detection.
**Gate:** a render test counts `set_style` calls across a keystroke (changed subtree
only); a hover change triggers no relayout; palette and container-query render tests
cell for cell; the scheme tests unchanged.

### R6—Controls

`Control<B: Behavior>`, the shipped behaviors, parts, lazy registration, controlled
semantics on both facades, the terminal's controls as composed trees with roles and
focus order, the accessibility additions; checkbox, radio and slider without a swap;
the picker's swap carrying the value; hydration writing a changed live value into the
bound signal; `Simple*` rebuilt over `Control`.
**Gate:** a button click inside a container does not reach the container on either
facade; a focused drawn checkbox shows its ring in a browser test; a terminal radio
group with nothing chosen is reachable by Tab; a served page's checkbox ticked before
hydration is ticked after with the signal true; a third-party control built in a test
implements focus and decoration through public types alone.

### R7—Motion

The terminal clock and interpolators, reduced-motion gating on the web, token-aware
keyframes, `set_transition_name`, the router's view-transition wrap and the
cross-document opt-in; core's `transition` renamed `settled`.
**Gate:** a terminal render test steps a color transition across frames; a browser test
observes `startViewTransition` around a route change; SSG output carries
`@view-transition` when opted in.

### R8—Store, snapshot, resource

The store as a trie with interned paths and a keyed `AtKey`; the derive's `skip`,
`tag`, generics and a non-colliding surface; snapshot identity by owner path and
sequence with tags as anchors; serde bounds on `Signal::new` with an opt-out
constructor; `Resource` in the engine with a `Refetching` state; the `snapshot` and
`test` features deleted.
**Gate:** a snapshot survives a branch flip and a program edit that adds a signal; a
store with ten thousand tracked cells notifies one row edit in constant time in a bench.

### R9—Delete and rewrite

The `_when` families, `Adaptive`, `SimpleTheme`, the text and control themes, the
`Web*` toggles and range, the per-control `Raw*` structs, `Source` from public
builders, `simple-views`; the examples gain a live palette control and a theme example;
`AGENTS.md`, `DESIGN.md` and this file rewritten to read as one story.
**Gate:** no deleted name compiles anywhere; every example builds on all three tiers;
the design doc's crate map matches `Cargo.toml`.
