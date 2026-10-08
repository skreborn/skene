# Agent & Contributor Guide

## Hard constraints

- Skene is a **client-side** framework. No server-side features, ever: no server
  functions, no backend framework integrations. SSR/SSG are rendering capabilities; the
  app's server binary is a plain command a user's server runs or embeds, not a server.
- **One foundation, a native shell per platform.** Each platform is its own crate and
  its own facade—`skene-web` and `skene-terminal`, later `skene-ios`, `skene-android`,
  `skene-windows`, `skene-macos` and `skene-linux`—with a vocabulary typed to exactly
  that platform, so what it cannot or should not do is unrepresentable in its code:
  types decide what a platform can do, and signals what an install of it can, which
  stays progressive enhancement within the platform's own vocabulary. Everything two or
  more platforms share with identical semantics lives in the foundation: mechanisms
  (the scene, dispatch, behaviors and their key tables, accessibility reporting, the
  style model, tokens and resolution, motion, layout), never vocabulary or
  presentation, and code that merely looks alike across platforms stays in each. The
  foundation may implement a view once where platforms share its semantics, each
  facade exposing it as a type of its own, and a platform realizes its own the moment
  they diverge. There is no cross-platform vocabulary: an app is a shared crate of
  logic and state with a shell per platform, and a component library a core of colors,
  named sizes and logic with a presentation crate per platform.
- The foundation (`skene-foundation`, over `skene-reactive`, which knows nothing of UI)
  stays platform-agnostic: no platform assumptions or nomenclature. Its **spine** (the
  `spine` module, naming nothing of the crate outside itself and keeping its internals
  `pub(in crate::spine)`, so the views beside it use only its public surface) hosts the
  retained **scene** (`Scene<P>`:
  parents, children, per-child annotations, token definitions, generational node ids), the
  zero-sized **`Platform`** trait every platform implements, event dispatch over the scene,
  the property model (`BoxStyle<N, S, X>`, `RunStyle<S, X>` and their visitors), values
  and expressions (`Value`, `Leaf`, `Expr`), conditions (`Media`, `State`, `Condition`),
  tokens (`Token`, `TokenSet`), `Boundary`, motion and interpolation, the runtime
  **`Resolver`** every non-CSS platform resolves its styles through (with the erased
  `TokenValue` and `SlotValue` it reads), the host **executor** a platform installs, generic
  geometry over the platform's scalar, and universal values. Beside the spine the
  foundation hosts every built-in view once, generic over `P: Platform`; each facade
  exposes them as views of its own, newtypes implementing the foundation's view traits.
- The canonical building block is a **view** (the `View` trait); never "widget". Two
  equal facades exist—`skene-web` (the web) and `skene-terminal` (the terminal)—each
  owning its builder vocabulary, typed to exactly its medium; neither is the blessed
  one. Within a facade, views are realized
  **once** against its typed platform vocabulary—no per-platform `impl View`, no
  "backend" concept.
- A **platform** is a type implementing `Platform`, carried by its facade's crate:
  `skene-web` carries the web's (`platform::Web`, its browser and server leaves) and
  `skene-terminal` the terminal's, down to the terminal device: its `device`, `tty`,
  `parse` and `escape` modules are the whole terminal layer, no terminal crate beneath,
  so a reply a new capability needs is parsed in the change that asks for it.
  `skene-headless` is the host platform the web compiles against off its target, driven
  by tests. What every platform of one medium shares above the spine lives in that
  medium's module of the foundation—`terminal` (`TerminalStyle`, `BorderGlyphs`, the
  terminal `Typography`) and `web` (`WebStyle`, the web's `Typography`, `Painting`, the
  document `Head`)—never in its spine, which knows no medium.
  Platforms depend on the foundation, never on each other; a facade is the
  only place that names its platforms, and it publishes the selected one as `ActivePlatform`.
  The trait is zero-sized and monomorphized: no `dyn` platform, no downcast, no runtime
  swap. It is **not sealed**, and the shared machinery bounds only on it and the public
  extension traits, so a third-party platform is a facade in all but ownership. Each platform
  crate also exposes an **escape trait** beside the portable one (`WebMarkup` on both web
  leaves, `WebDom` on the browser leaf only, `CellPainter` in the terminal, later
  `PlatformView`), so a low-level view is written against exactly the bound it needs and
  is realized through the same scene and node ids as a built-in (`skene_foundation::LowLevel`
  mints the node it realizes). `Painted` (vello) is the portable low-level path
  for pixels, through the foundation's `Painting` extension: the browser leaf renders the
  scene into a canvas, the headless platform renders it to pixels a test reads, the
  terminal shows it through its graphics protocol where one is proven, the server leaf
  renders it empty; the native realization comes with the native platform.
- Selection: by **facade dependency** (web apps depend on `skene-web` and a library's web
  crate, `skene-simple-web`; terminal apps on `skene-terminal` and its terminal one), then
  by target within a facade (`wasm32-unknown-unknown` → the web's
  browser leaf; every other host → the headless platform), and where one triple hosts several platforms the build names the
  platform by a cfg: `--cfg skene_platform="server"` selects the web's server leaf
  natively, set by the CLI for a server build through the host target's rustflags, so
  it never reaches a build script or a proc macro, and through `RUSTFLAGS` or
  `.cargo/config.toml` by hand. A cfg is a build fact like the triple, chosen once,
  unifying nothing, and never a cargo feature. Target-gating is the
  enforcement layer: the browser leaf compiles on `wasm32-unknown-unknown` alone and
  the server leaf where the build names it (`skene_web_platform`, one axis `skene-web`
  and a library with code per leaf derive from the triple and the cfg through the
  `skene-build` build-dependency; `browser`, `server` or `headless`, a build it has no
  leaf for failing), so the server leaf physically cannot touch the DOM. A cargo
  feature is an **opt-in upgrade** and nothing more: it enables optional dependencies
  and the code reaching them, never a trait bound, a type, a variant, a platform or an
  item an app names, so the build without it is complete and merely slower or larger
  (`skene-web`'s `gpu`, painting on the GPU where the page has WebGPU). None is on by
  default, and the gate's manifest check holds a feature to `dep:` crates alone.
- The **style model is data**: `BoxStyle<N, S, X>` and `RunStyle<S, X>` name every
  shared property once (typed properties, conditions riding the value, transitions,
  keyframes), and the medium's own properties come through its extension `X`. Layout
  **runs on taffy**—the framework invents no layout system and exposes taffy's full
  capability—but the **public vocabulary is not CSS's**: names and values follow the
  modern declarative frameworks (Flutter, SwiftUI, Jetpack Compose), mixed and matched
  for whichever is clearest—explicit and self-describing; CSS naming stays only where it
  is genuinely the best name in context. Concretely: distributions are *arrangements*
  (`arrangement`, `line_arrangement`: `Arrangement`), per-item properties are
  *alignments* (`item_alignment`, two-axis `Alignment` for grids and stacks)—never
  `justify-*`/`align-*`; `Dimension::Fraction` is a ratio of the parent (`0.5` = half,
  not clamped, not a weight); weights are the wrappers' grow/shrink factors and
  `GridTrack::Flex`; `spacing`, not `gap`; `columns`/`implicit_columns`/`flow` on a
  grid, not templates and auto-flow. CSS nomenclature lives only inside the web
  platform's mapping layer. This is settled—do not re-litigate it toward CSS naming.
  A run's decoration is three properties—`decoration` (`TextDecoration`),
  `decoration_style` (`TextDecorationStyle`) and `decoration_color`—Flutter's names;
  a link underlines unless its decoration says otherwise, and the terminal keeps the
  styles and color only where a DECRQSS probe proves it, a plain underline elsewhere.
  **Text keeps its blanks and newlines** as a native text view does, and a word too wide
  for a line of its own breaks where the line fills: the web's reset sets `white-space:
  pre-wrap` and `overflow-wrap: break-word` rather than collapsing them as a browser does
  by default, and the shared `Paragraph` lays text out by the same rules, Chromium's
  where the engines differ (a tab at a wrap hangs).
- **Layout is statically constrained.** Each layout is its own container view—`Row`,
  `Column`, `Grid`, `Stack`—exposing only the properties it honors; per-child properties
  travel in wrapper views (`Flexible`, `Pinned`, `GridCell`) accepted only by the
  matching container through its child kind (`HasChildren::Kind`). Wrappers create no
  node; their properties become ordinary properties of the child's own style, folded in
  before the child is created, so they compile into the class and take conditions,
  tokens and derived values like any property. `Container` is decoration and sizing around
  **exactly one** child—no multi-child flow; `Column`, `Row` and a wrapping `Row` cover
  those cases. **Only `Container` is decorated**: it is the one freely-stylable box—
  decoration, sizing, events, alignment, and a single child—and every layout view
  carries layout properties plus its own padding and nothing else, so decorating a
  layout means wrapping it. Conditional styling rides the value:
  `set_spacing(Spacing::from(8.).when(Media::MinWidth(900.), 16.))`. One setter per
  property plus its `derive_` twin, no `set_<prop>_when`, and a variant is expressible
  only where the property is. There is no box-surface forwarding to solve, and no trait
  for it. A control that draws nothing needs none of it either—its chrome is the
  `Container` its child is made of. A control that owns an element of its own spells that
  element's decoration per property too; no control surface takes a style blob. A
  third-party layout view is built from the public `LayoutBox` exactly as `Row` is;
  there is no privileged base.
  A property value has five kinds of leaf and the type says which: fixed; a **token**
  reference; an **expression** (`Expr<Length>`/`Expr<Number>` with ring arithmetic,
  `Expr<Color>` with the named color functions and nothing else, typed so an ill-formed
  expression does not compile); **derived** from signals by a closure that returns a
  value, a token or an expression; and any of those under a condition. The first three
  and the conditions compile to CSS, so SSR/SSG output is responsive, hoverable, themed
  and animated with **no JS/WASM**; a derived leaf is a per-node custom-property slot
  the class references, holding the literal or the token's `var()`, so it never
  outranks a variant and a state may choose between palette tokens. Every compiled rule
  has a class's specificity, its states, anchors and scheme guards in `:where()`, and
  sits in the `skene.style` cascade layer above the reset's `skene.reset`, so rule order
  alone decides between variants and an app's own CSS always wins. `derive_` closures
  are for **state**, never for environment or arithmetic.
- **A design token is a named, inherited slot; a theme defines it; a palette belongs to
  its library.** `TokenRef<T>` names a slot (a `const`; the name is an implementation
  detail on every platform, never a public hook) of a type a token holds (`Tokenable`:
  a color, a number, a length, a dimension or a text), `Token<T>` pairs the name with
  the definition a palette gives it, and every setter takes `impl Into<Value<T>>`, so
  `set_background(Color::WHITE)`, `set_background(palette.card)` and
  `set_background(SimplePalette::CARD)` are the same call and a consumer emits only the name.
  A length token (`Token<Length, N>`) serves every property measured in lengths,
  padding, spacing, margins, sizes, corner radii, font sizes and letter spacing alike,
  through its one reference, and a name is one kind wherever it is defined: a debug
  build panics on a second. A token's own conditions live in its **definition**, once:
  `Definition<T>` conditions on `Media` only and holds no closure, so a palette is
  plain data that serializes and hashes. On the web a token is a custom property
  defined on the theme's scope and referenced with `var()`; in the terminal and on
  native it resolves through the scene to the nearest defining ancestor, cached per
  node, and its definition is resolved there, nested tokens, font sizes and container
  conditions included, as CSS computes a registered custom property where it is
  declared. Nothing below a component library knows what a palette is: `skene-simple`
  owns `SimplePalette` as a struct of `Token`s implementing `TokenSet`, and a third
  party's palette is on identical footing.
  - **`Theme<P, S: TokenSet>` is the one theme view**, in `skene-foundation`:
    `Theme::new(palette)` or `Theme::derive(|| palette.get())`, then
    `set_child_builder(|| child)`. It realizes one layout-invisible scope node, provides
    `Memo<P>` through context (`P::current()`, `P::watch()`, from `ThemedIn`), and one
    effect rewrites its definitions when the palette changes: no consumer is touched and
    nothing rebuilds, so on the web a palette change costs the number of tokens, never
    the number of nodes (the terminal re-resolves every node beneath the scope until its
    token caches take per-scope epochs). Nesting a theme redefines beneath it. The child is a builder closure
    because context is owner-scoped and an argument is evaluated before the call it
    belongs to. `Typography` (all text inheritance, on both facades) and the terminal's
    `SimpleGlyphs` (the Simple library's terminal marks) are token sets on the same
    mechanism; there is no text theme, control theme
    or Simple theme, and `SimplePalette::default()` is built from public constructors,
    so what the library does is exactly what a third party can do.
  - **Light and dark are one condition**, `Media::Dark`, answered per property in a
    definition or a value like a viewport or a pointer. There is no light/dark pair and
    no palette-level conditional—the whole-palette overlay was considered and rejected,
    because a palette that switches wholesale re-privileges the very separation the
    per-property form removes.
  - **The scheme in force is the document's, not a theme's**, exactly as the platform's
    own preference is—so `set_scheme(Option<ColorScheme>)` and `derive_scheme` live in
    the facade's `media` module rather than on a theme, and `None` returns to following
    the platform. Nesting themes nests palettes; the scheme decides which value each of
    their conditional tokens takes.
  - **A scheme an app controls is resolved by the platform, never by a rebuild.** A
    `derived` rebuild destroys the old nodes and disposes their scope—measured: a signal
    inside the subtree goes back to its initial value, and focus, scroll offsets and
    native input state go with it. So `derive_scheme` tells the platform which scheme is in
    force and touches nothing: the web writes `data-skene-scheme` on the document root,
    which every scheme-conditional rule is already gated on, and the terminal reads the
    choice where it reads its detected background. One write re-inks the page.
    - The web emits each scheme variant **twice**—the media query guarded by
      `:where(:root:not([data-skene-scheme="light"]))`, and the choice as
      `:where(:root[data-skene-scheme="dark"])`. Both guards sit in `:where()` so they
      contribute no specificity: without that, a dark variant would start beating a
      `:hover` rule that is more specific in intent.
    - Scoping the override to a subtree is **not wanted**: an app has one scheme, the
      way the platform does. It was investigated, the guard half is genuinely hard, and
      the requirement was withdrawn rather than solved—do not reopen it as a bug.
    - A server-rendered page that offers a choice emits one inline **blocking** script in
      its head, applying the stored scheme before the first paint; deferred would run
      after the paint, which is the flash it exists to prevent. It carries the render's
      script nonce, so the page works under a strict Content Security Policy.
      `chosen_scheme()` reads the same key so an app's setting starts where the page
      already is.
  - **The scene is the tree.** Every view attaches to its parent's scene node, wrappers
    realize nothing, and `derived`, `Suspense`, `ErrorBoundary`, `Deferred` and `empty`
    mount between **anchors** rather than inside a box, so a reactive subtree contributes
    its own nodes to its parent's layout. An event inside such a segment climbs through
    its start, and an overlay anchored on one is placed against its first box, so a view
    wrapping a reactive subtree hears and anchors to its content. The platform's tree is
    a sink written from the scene, never read back for structure, and the scene tells it
    what happens to each child—attached, relocated, hidden to return, or removed for
    good—so only a hidden child keeps what leaving loses (the focus inside it, its
    scroll offsets) and a removal keeps nothing. Overlays are `<dialog>`/popover promoted to the
    top layer *in place* rather than portalled, and only the `<head>` is a real portal;
    an anchored popup is placed by CSS anchor positioning where the engine has it and at
    the anchor's measured rectangle where it does not. A native platform may still realize
    an overlay into its own layer, so nothing is built on ancestor-selector reachability
    beyond a control's own subtree.
  - **Environment is queryable.** `media::viewport()`, `media::scheme()`,
    `media::reduced_motion()`, `media::reduced_transparency()`,
    `media::high_contrast()`, `media::pointer()`, `media::hover()` and
    `media::safe_area()` are signals on every facade, one `Environment` in the spine
    holding what they report, `media::watch(&condition)` turns any declared condition
    into a memo, and a box's measured size reaches a view through
    `Platform::observe_size` and an app through a `NodeHandle` bound to the box. On
    the web they are live after hydration and hold the server's answer before it; the
    server's answer comes from client hints when the render is given them, else from
    documented defaults. Declare what must be right before the bundle arrives; derive
    what may wait for it.
- **Every realization has a floor the platform always has and an upgrade a signal
  proves, and nothing an app writes changes when the upgrade arrives.** The choice
  belongs to the platform when the floor is its own drawing (the terminal's backdrop in
  cells then in pixels, text at one cell then sized, truecolor over sixteen colors, a
  WebGPU canvas over a CPU one, anchor positioning over a measured rectangle) and to
  the view when the floor is another view (`Painted::set_fallback_builder`, the picker's
  `<select>`, `Enhance`). A low-level view hands a platform both forms where it has
  both, as `acme-sparkline` hands the terminal a cell painter and a pixel painter for
  one box, and the platform shows the upgrade where it proved it. Only a low-level
  view, written against an escape trait, ever concerns itself with what a platform
  can do; a default-bodied `Platform` method expresses an absence, never a behavior,
  and a view that needs the answer reads a signal, never a `cfg`.
- **Progressive enhancement is doctrine.** Everything that can work without JavaScript
  must: server-rendered and statically-generated pages deliver their full content as
  markup (`#[lazy]` bodies included—the server leaf never splits), navigation works as
  real links, and the client bundle layers interactivity on top by hydrating in
  place—content is never gated on JS. **A view with two forms declares both**:
  `Enhance` in `skene-foundation` builds `set_fallback_builder` while the signal it is
  given is false and `set_child_builder` once it is true, each at most once. There is no
  capability vocabulary: each signal belongs to what can lack it, and a web-only fact
  is named for the web. `Platform::hydration()` is the web's, `None` by default (the
  terminal and the native platforms never answer, so the terminal facade cannot name
  it): the web leaves return the signal that is false on the server leaf and while a
  hydration adopts the server's markup and true once it has finished (so the fallback
  is adopted, then rebuilt into the child in place, under a replacement owner), the
  headless platform returns one a test flips (`trigger_hydrated`), and the web facade
  reads it as `media::hydrated()`.
  `Painting::available()` is the pixels': false on the server and in the terminal until
  its graphics probe is answered, true on the browser leaf once live, `trigger_graphics`
  on the headless platform; it is what `Painted::set_fallback_builder` waits for, and
  the picker's prerendered `<select>` waits on `hydration()`, drawn outright where
  there is none. Prefer the latest CSS standards and pay nothing for obscure or ancient engines;
  when a slightly older engine can be supported without giving anything up in new ones,
  support it.
- The **interner contract is load-bearing**: content-addressed class names
  (`skene-{hash:x}` from a wide hash), reference-counted, deterministic and identical
  across the browser and server leaves—hydration and stylesheet parity depend on it.
  The pure core lives in `skene-web` and is shared by both leaves; each layers only its
  sink.
- **Painted** content is vello wherever it is realized: a `Canvas` is a vello scene in
  device pixels, drawn with kurbo shapes and peniko brushes, recorded once and replayed
  into whichever `Renderer` the platform owns. The browser leaf renders it into a
  `<canvas>` placed over the box and out of its flow, sized in the device pixels the
  engine reports (the content box times the pixel ratio where it reports none),
  repainting when either changes: through vello's hybrid renderer over wgpu where the
  app enables `skene-web`'s `gpu` feature and the page has WebGPU—one device shared by
  every painted box, a surface per canvas—and
  through vello's CPU renderer otherwise. The choice is made once per page when the
  first box paints, and a box paints nothing until it is made; a lost device is asked
  for afresh, a canvas whose GPU path fails paints on the CPU in a fresh canvas
  from then on, and one whose 2D context the browser loses is replaced by a fresh one
  and repainted. Native layers come with the native platform. SSR/SSG render it empty.
  The terminal renders it on the CPU at the box's cells times the cell's pixel size
  and shows it through the kitty graphics protocol where launch's query (`a=q`) was
  answered `OK`: transmitted once per change, compressed, placed over the box's cells cropped to
  those in view, left out under an open overlay, freed on unmount; elsewhere the
  fallback stands. Where graphics show, a box with a corner radius, a border or a
  shadow paints them as a backdrop image beneath every colored cell, one per box,
  its own cells left uncolored so it shows through, and every glyph stays a cell. `Painted`, `Canvas` and `Painting` live in `skene-foundation`, re-exported
  by its `web` module. **Embeds** are
  platform-specific views with platform-exposed constructors (raw DOM element on the
  web's browser leaf, real platform controls on native); the payload never crosses a
  crate boundary. On platforms with a real view hierarchy the platform realizes the
  tree into it, so embeds and painted layers interleave in z-order by the platform
  compositor.
- **The terminal targets browser parity.** Wherever the terminal can express it, behavior
  follows a modern browser: layout, features, settings, colors, selection, and every
  interaction's shape—clicks fire on press+release over one target while focus, caret
  placement, and drags act on press; wheel gestures latch their scroll target and
  consume at its limits until a pause or pointer move, an editor scrolls like a box
  (wheel, scrollbar, page keys) without moving its caret, and a field's window follows
  the horizontal wheel; a release ends a drag on the captured node wherever it lands; a
  link run is a tab stop that `Enter` follows and `Space` does not, a press on its cell
  focusing it as a click on an anchor does; `FocusVisible` holds as `:focus-visible`
  does, after a key or in a text entry and not since a pointer press, on the terminal
  and the headless platform alike; popups light-dismiss, except on their own anchor, whose
  click toggles them, and close as focus moves to a node outside them and their anchor;
  a modal takes focus on open and returns it on close; a key's
  default runs after the handlers' writes have taken effect, as after a browser's
  microtask checkpoint; a release dispatches as `KeyUp` with no default, a held key
  repeats its `KeyDown` with `repeat` set, and a modifier alone arrives under its W3C
  name without showing focus, the terminal asking the kitty keyboard protocol for its
  event types, alternate keys and every key as an escape code, and a release following
  each press at once until the flags the terminal kept confirm its event types;
  `Space` acts on its release, once, where `Enter` acts on every press; a letter's case
  follows `Shift` and Caps Lock as a browser's `key` does while the modifiers stay
  the keys held, so `Ctrl-Z` undoes under Caps Lock too, and a key's `code` names
  the physical key as a browser's does (`KeyA`, `Digit1`), from the terminal's
  base-layout alternate where it reports one; selection
  gestures match (drag, double-click word, triple-click run), inside a text entry as
  well, where keyboard focus selects a field's value, `Ctrl-Z` and
  `Ctrl-Shift-Z`/`Ctrl-Y` undo and redo per entry as an `<input>` does, a value
  written from the signal being a step of its own, and
  `Ctrl-C`/`X`/`V` copy, cut and paste through the ordinary dispatch, so `Ctrl-Q` is the
  quit; a right press, `Shift-F10` or the menu key opens the context menu the nearest
  wrapper serves, the standard one from the root where none does, and the entry beneath
  keeps its selection while the menu holds focus. **The clipboard is the
  app's launch choice**: OSC 52 has no capability query, so `Launch::set_clipboard`
  names which of copy, cut and paste exist (all by default; without copy nothing in
  the document is selectable), except where the terminal's clipboard answers (kitty's
  OSC 5522, proven by a DECRQM report of mode 5522), in which case every operation is
  offered, a copy is written through it and a paste is read through it, the text
  arriving in parts and a refusal pasting nothing; `Launch::set_mouse(false)` hands the mouse back
  wholesale, and the mouse key (`Alt-M` unless `Launch::set_mouse_key` says otherwise)
  hands it over at run time for one native selection, any key taking it back. Text
  is measured by extended grapheme clusters, each as wide as the terminal draws it: a
  wide cluster takes two cells, the second painting nothing, a combining sequence one,
  a tab the blanks to its next stop, eight cells apart from its line's start as a
  browser's `tab-size`, and a field's caret, window and selection index clusters. Grapheme clustering (mode
  2027) is asked for at launch and confirmed by DECRQM; where the terminal confirms it
  a cluster's width is the cluster's, and where it does not a cluster is as wide as its
  code points drawn one by one, so the layout agrees with the terminal either way. A
  run's font size is a scale of the cell, the shared `set_font_size` and the
  terminal `Typography`'s `size` token: text sizing (kitty's OSC 66) is probed at
  launch by a space at scale two and a cursor position report, a terminal that proves
  it draws every size (a whole scale of one to seven, or a fraction of a cell below
  one) with layout counting the scaled cells and a paragraph's smaller spans on its
  tallest span's baseline, and one that does not lays the same tree out at one cell. A pointer position is its cell's center, or exact where the terminal
  reports pixels (SGR-Pixels, asked for when the platform reports the window's pixel
  size and trusted from the DECRQM reply that confirms it); the window's size follows
  the resize notifications, the terminal's in-band one (mode 2048, set and confirmed
  by DECRQM, its pixel size feeding the pixel mouse where the platform's carries
  none) or the platform's own; queued pointer motion
  collapses to where it ends before a frame paints, and a frame is written under
  synchronized output (mode 2026) where the DECRQM reply says the terminal toggles it.
  The terminal's own colors are queried at launch (OSC 10, 11 and 4), so a 16-color
  paint picks the nearest base color the terminal shows and a shadow or a scrollbar
  mixes against its real foreground and background.
  Every terminal capability
  beyond a plain emulator is a progressive enhancement of that kind: asked for,
  confirmed by a parsed reply or ignored in silence, degrading to the cell rendering,
  never required and never inferred from a name; the plan is in `ROADMAP.md`. The escape hatch for a terminal without OSC 52 is the terminal's own
  selection—a drag with `Shift` held, which bypasses the app and needs nothing from
  it—or a plain drag with the mouse off. When an interaction's
  design is in doubt, mimic the web—**because the browser is the most constrained
  platform we target, not because it is the best design**. Parity is a floor. Where a
  browser behavior is merely a browser behavior, we are free to rule otherwise, and we
  record it here when we do.
- **Share the logic, match the native, then follow Chromium.** Every mechanism is
  written once, where its facts live, and each platform delegates to it rather than
  keeping a copy; a platform with a native behavior matches it; where there is no
  native (the terminal, headless) or engines differ, the most popular browser's
  behavior, Chromium's, decides.
- **A view that handles an interaction owns it.** Every control consumes the events it
  acts on: a press on a button is a press on the button, never also on the row it sits
  in. This is a **deliberate divergence**—a DOM `<button>` inside a clickable `<div>`
  does bubble to the div—settled because nested clickables are ambiguous to a user and
  the framework's idiom is a closure per view, not delegation. The cost is that a parent
  cannot observe its controls' clicks in one place; wrap the group in a `Container`,
  which is a box rather than a control and keeps the choice.
- **A verdict is the view's, never its user's.** `handle_<event>` takes a plain callback
  on every control, `Basic*` and `Simple*` alike. A control decides what the platform and
  the ancestors are told, once, where its behavior lives, so no app or library author
  asks "am I cancelling a platform default here". `Prevent` belongs to the layer that
  owns behavior (a `Behavior`, a platform); above a platform, nothing states one. A control
  the platform realizes natively never asks to suppress the native response, because there
  that response **is** the control. A box keeps the one decision that is genuinely its
  own: `Container`'s handlers return `Into<Bubbling>`—`()` for the ordinary case,
  `Bubbling::Stop` to keep the event from the ancestors—so a box chooses whether it
  consumes, and a control has that chosen for it. The name is deliberate: it is about
  onward travel only, since a view may handle an event and still let it climb. Nothing
  above a platform states more. Dispatch itself lives in the spine over the scene: a platform
  reports an event at a node, the spine walks the parents it had when the dispatch began,
  runs handlers deepest first, folds the verdicts and tells the platform what its
  native layer may do—so every platform, the headless one included, bubbles by one
  implementation. A click on a native web toggle is dispatched *observed*
  (`dispatch_click_observed`): every handler up the path runs except the acting one the
  behavior registered, since the platform's flip arrives as `change`, and the climb
  ends at the control, as the acting one's verdict ends it everywhere else.
- **A control is `Control<B: Behavior>`**: one builder in `skene-foundation` carrying focus,
  events, decoration, label, parts and accessibility once. `Toggle`, `Slider`, `Popup`,
  `Activate`, `TextEntry` and `Collection` are the shipped behaviors (`Member` the
  members'; `RadioGroup`, `Listbox` and `Menu` are collections under a `CollectionPolicy`),
  `Behavior` is public, and the keyboard tables live in the
  behaviors, so a third party's control and the terminal's slider get the same keys as the
  browser's. `BasicButton`, `BasicCheckbox`, `BasicSlider`, `BasicRadioGroup`/`BasicRadio`,
  `BasicListbox`/`BasicListboxOption`, `BasicTextField` and `BasicTextEditor` are aliases over them, and
  `BasicPicker` is composed from them. A listbox and the closed picker face have
  type-ahead (`TypeAhead`, its buffer reset on `Platform::schedule`) over the names the
  options' labels supply as they read now (`Label::set_name`, `Label::derive_name`), and
  the face's verdict on a typed key is the `Popup` behavior's through `set_key_fallback`. A
  control's label sits where its behavior places it (`Behavior::label_placement`):
  leading or trailing in its row, or beside its box—before it among its siblings on
  every platform, answering to the control, as the web's `<label>` does—the default
  for a control whose box holds a value, a text entry, a picker, a popup face. A label
  is a convenience, never a requirement: `set_labelled_by` names a control by a
  caption placed anywhere, and `set_accessible_name` by text alone. A control is
  **controlled** on every facade: it reports, and the bound signal is the
  truth, the platform brought back to it after every report, so a change its handler
  rejects is undone. A text entry's selection is controlled
  beside its value: `TextSelection` is a `start` and an `end` in grapheme clusters of
  the value, the end where the caret is and before the start when made backward,
  `derive_selection` places it, `handle_selection_change` hears every move of the
  caret, and `bind_selection` wires both to one signal; a platform clamps each index
  into the value, so nothing can name the inside of a cluster. A `State` a box cannot have on its own
  (`Checked`, `Indeterminate`, `Disabled`, `Invalid`, `Open`) resolves against the
  enclosing control; a node's own `Hovered`/`Focused` stay its own. Parts (`Track`,
  `Fill`, `Thumb`, `Mark`, `Face`, `List`, `Item`, `Label`, and a tooltip's `Bubble`)
  are the styling targets a
  control exposes; in the terminal the checkbox and radio `Mark` and the picker's
  indicator are glyphs from the Simple library's `SimpleGlyphs` token set, and the
  slider's track, fill and thumb are boxes there as everywhere. **Accessibility is
  reported, never spelled**: a behavior reports what it is (`Checked`, `Value`,
  `Expanded`, `HasPopup`, `Orientation`), a control's user adds what any control may carry
  (`Required`, `ReadOnly`, `Invalid`, `DescribedBy`, and a `Label` through
  `set_accessible_name` where no caption names it), and the platform maps each to its
  layer—on the web a form element takes the native attribute where one exists, so the form
  enforces it without script, and ARIA otherwise. A caption drawn by a view names its
  control by reference (`LabelledBy`, `aria-labelledby` on the web), since a combobox,
  a listbox or a radio group may not be named from content; a region, a dialog or a
  picture is named through `set_label`/`set_labelled_by` on `Container` and `Overlay`
  and `Painted::set_description`; `Role::Status` and `Role::Alert` are the live
  regions, `Role::Dialog` a popup's role, a tooltip's bubble is `role="tooltip"`, and
  a scrolling box is a tab stop on the web (`tabindex="0"`) as in Chromium. **A
  disabled control takes no focus**—the ARIA practices allow either, and a control
  the user cannot operate is kept out of the way rather than announced; the divergence
  is deliberate. The gallery runs under axe-core in the browser tier (`browser_axe`), as
  drawn and with each of its overlays open, the gate for every built-in's semantics. On the web a control that owns a native
  input keeps it as its scriptless form—a text field or editor *is* its `<input>` or
  `<textarea>`, its caption a `<label for>` the platform keeps before it among its
  siblings, a checkbox, radio or slider keeps its input inside its label beside the
  parts that draw it, covering the label's box unseen so a reader finds the control
  where it is drawn while a press reaches the parts beneath, a run inside any control
  taking no pointer of its own, and none needs a swap after hydration—and the picker, drawn on
  every engine, is an `Enhance` over its `NativePicker` form, a `<select>` the server
  renders and the reader's choice in it carries across the swap. The
  terminal's controls are composed trees painted by the same box and run painters as
  everything else, so they carry roles, accessibility and focus order like the web's.
  **A tooltip is a view**, `Tooltip` in `skene-foundation`, wrapping the one view it
  describes and showing text or any view in a `Bubble` beside it: open after a delay on
  hover and at once on focus that shows, open while the pointer rests on the view or
  the bubble (WCAG 1.4.13), closed a grace period after it leaves both and at once on
  blur and `Escape`, never focused. A text tooltip is reported as the `Description`,
  beside `DescribedBy`, of the node that takes focus within the described view, the
  view itself where it does, so a reader hears it where focus lands, and a view's
  reports are merged in the scene, the relations of one kind folding into one list, so a
  describing view never overwrites a control's own. On the web the server renders the description as `title`, the
  scriptless tooltip, and hydration **removes** it as the bubble (a `popover="hint"`
  promoted in place) takes over. `SimpleTooltip` is the styled one; any view takes a
  tooltip by wrapping. **A context menu is a view** the same way, `ContextMenu` in
  `skene-foundation`: wrapping is the opt-in, a secondary press, `Shift-F10` or the menu key
  on the wrapped view dispatches `EventKind::ContextMenu`, the deepest wrapper along the
  path opens its `List` pinned at the pointer and prevents the platform's menu, and
  wherever no wrapper is the browser keeps its own. Items are `MenuEntry`s, the standard
  `copy`/`cut`/`paste` among them, each asking the platform's `clipboard_availability`
  whether it is absent, disabled or enabled and acting through `Platform::clipboard`; the
  list is a `Menu` of `MenuOption` controls (`Role::Menu`/`Role::MenuItem`), the
  behaviors a third party's menu reuses. `SimpleContextMenu` is the styled one; the
  server renders nothing for a menu.
- **Presentation is never hardcoded.** The framework abstracts mechanisms—behavior,
  accessibility, a platform's realization—and never fixes presentation: wherever a
  built-in makes a presentation choice (where a label sits, how a control's row is
  spaced, where a popup opens, how thick a rule is), the choice is a default its user
  overrides, so first- and third-party libraries keep full creative control. A choice a
  behavior makes is a `Behavior` method with a default (`label_placement`); one a view
  makes is a setter; and where a built-in composes presentation of its own, the pieces
  it composes are public, so a library can compose them differently.
- **Third parties are first-class by construction.** A component library is a core and a
  crate per platform. Its core depends on `skene-foundation`, writes its views generic
  over `P: Platform` exactly as the built-ins do (`Container<P>`, `Control<P, Toggle>`,
  `Theme<P, SimplePalette>`), states what it needs from a medium as a bound on the
  platform's associated types (`P::StyleExt: CornerRadius` rounds where the medium can and
  writes nothing where it cannot; a capability a library *requires* is a hard bound, so
  the library does not exist on a medium lacking it), and keeps each view's members in a
  `<View>Base` trait; each platform's crate declares a newtype of its own per view over
  the core's, a line per trait, with a `prelude`. A low-level view is written against a
  platform's escape trait and selected through the facade's `ActivePlatform`, and a view
  library covers every platform of its medium, headless included, through a trait of its
  own each platform implements (`acme-map`'s `MapHost`), so an app writes no `cfg` for
  it. Nothing on the path a built-in takes is crate-private: the lint rejects a
  `pub(crate)` anywhere in the foundation. Simple is held to the rule from the other
  side: a facade never re-exports it, an app depends on its platform's crate explicitly
  (`skene-simple-web`, `skene-simple-terminal`), and its core hides its generic views
  behind the `Simple*Base` traits, so it is the standing proof of a component library,
  and `examples/third-party` (`acme-map`, `acme-sparkline`) of a low-level view; both
  are part of the gate.
- **Motion is declared once and runs everywhere.** `Transition` and keyframe `Animation`
  are data. The web compiles them to CSS; native and the terminal run them on the
  platform's frame clock from the resolution point that reports a changed value, never
  through the reactive graph per frame. Every change source animates: a condition
  flipping, a token redefined, an expression input moving, a derived write. Reduced
  motion is respected by default. `set_transition_name` and the router's view-transition
  wrap cover same-document transitions, `@view-transition` the cross-document ones, and
  both degrade to a cut.
- View logic tests run headless via `skene-headless`, the host platform, which implements
  the whole `Platform` trait so nothing is wasm-gated for want of a platform; its executor
  runs when a test calls `pump_tasks`, so a `Resource` loads in a test too. A test finds
  what a reader finds and acts as a user acts: `find_by_role`, `find_by_text` and
  `find_all_unnamed` read the accessible names the spine computes over the scene
  (`Scene::accessible_name`), `access_tree` and `access_outline` read the whole
  accessibility tree as AccessKit models it, and `user` turns a click, a key or typed
  text into the events a browser dispatches for it, on headless and the terminal alike. Parity is
  held by construction: `skene-conformance` is one suite of scenarios, each opening a
  page of the suite (a view built on the platform, publishing `Probes` of the signals
  it binds), acting as a user and checking what a reader finds, run through a `Driver`
  every platform implements over its own seams, in its own process or in another—a
  behavior every platform must share is a scenario there, and a divergence it finds is
  a bug in the platform, never a special case in the suite. A third-party platform
  proves itself the same way. The web runs it in real browsers:
  `skene-conformance-web` serves every page from one app built as an app is, and its
  harness acts each scenario out over a WebDriver session, each in a fresh window, by
  trusted input from WebDriver's actions—an event a page dispatches to itself runs no
  browser default—finding what it acts on by the role and the name the browser
  computes. Platforms
  test against their real sinks (web: real DOM via `wasm-bindgen-test`; the server
  leaf natively under a byte-exact snapshot; native: its toolkit). No mock stands in
  for a platform; only the spine's own unit tests run on a double, a `TestPlatform`
  recording the calls it receives.
- HTTP, WebSockets and timers are userland. `skene-transport` holds the network clients built on
  the public seams alone, in a crate nothing in the framework knows about, so it stands
  where a third party's would: `fetch` on the web and a thread around `ureq` on every
  other host, a server render included; `ws` is its WebSocket sibling, the browser's
  socket on the web and a thread around `tungstenite` elsewhere. The framework ships
  the seams—`spawn`, `Resource`, `Boundary`, the server leaf's executor (`sleep`, the
  render deadline), and `Platform::schedule`, a one-shot action on
  the platform's clock that a view's delay waits on (the tooltip's hover delay; the
  headless platform and the terminal fire it from the hand-stepped clock a test
  advances, the server never)—so third-party integrations sit on the same
  footing first-party ones would. (Exception: a native platform
  owning its event loop may ship a loop-integrated timer.)

## Style and documentation conventions

- Use 2-space indentation.
- Never surround an em-dash with spaces.
- Attach a helper to the type it operates on as a method, not a free function.
- Put `#[must_use]` on the type itself, not on the functions that return it, whenever it
  semantically makes sense.
- Apply `#[inline(always)]` wherever appropriate: the framework has a great many tiny functions
  that are best inlined across crate boundaries.
- Document every public API; comment non-public code only when critical. An item
  visible across modules (`pub(crate)`, `pub(super)`) is an interface between them and is
  documented as a public one is; an item private to its module is commented only when
  critical.
- Give a purpose-built trait a `#[diagnostic::on_unimplemented]` message: these traits
  exist to express one rule, so a failed bound should state that rule rather than the
  mechanics. Where a blanket impl would be blamed instead, mark it
  `#[diagnostic::do_not_recommend]` so the message the author needs is the one shown.
- Within a type's own `impl` blocks, refer to that type as `Self`, never by name (`Self::ZERO`,
  `Self(value)`, `-> Self`). Its name may still appear for a *different* type, e.g. the source type
  in a `From` impl.
- Comments state contracts only, never history, rationale, "why", or implementation.
  Examples teach: the comments of an example under `examples/`, of a doc example and of
  the guide may say why, as teaching code should.
- A comment's first line is a short summary; add further lines only when needed to use the item.
- Split a comment into paragraphs when it is long or covers distinct points.
- Comment a struct's fields or an enum's variants all or none: never document some siblings and
  leave others bare. The only exception is when only the public fields/variants are documented.
- Omit blank lines only between lines of the same shape (consecutive `let`s, struct fields).
- Surround every `return`, conditional, loop, block, and multi-line statement with a blank line.

## Naming & API conventions

- The spine stays minimal and structural. The built-in views live in `skene-foundation`,
  written once against `Platform`; each facade declares a newtype of its own per view
  (`#[derive(View, Wraps)] pub struct Container(skene_foundation::Container<ActivePlatform>);`)
  with its own documentation and a line per trait it takes its shared members from
  (`impl ContainerBase<ActivePlatform> for Container {}`), its child transitions and the
  facade's own members inherent beside them, and the rest of the surface every facade
  shares declared by one `skene_foundation::facade!`. A view's own members live in
  `<View>Base`, a surface several views share in its own trait (`DecorationBase`,
  `MotionBase`, `RunBase`), and what a capability adds in `<Trait>With<Capability>`
  (`DecorationWithCornerRadius`), implemented wherever the capability is; a wrapper
  (`Flexible`, `Pinned`, `GridCell`) derives `Wraps` alone and implements `IntoChildOf`
  for its one kind. Users extend by composing built-ins, painting a `Painted` node, or
  authoring a view against a platform's escape trait.
- Builder methods: `set_<prop>` for a fixed value, `derive_<prop>` for a reactive one,
  `add_<item>` for one appended to a list (`add_transition`, `add_child`),
  `handle_<event>` for an event handler (e.g. `Container::handle_click`), and
  `bind_<prop>` for a signal both at once, the value derived from it and each change
  written into it (`bind_checked`). A choice group's value is what it has `chosen`
  (`set_chosen`, `handle_choose`, `bind_chosen` on a picker, a listbox and a radio
  group) and a command's event its activation (`handle_activate` on a button, any
  control and a menu entry), where a box keeps its click. A platform function that
  realizes the handler is named the same.
- The layout vocabulary rule extends to the whole surface: views, controls, and
  composition helpers take the platform-native frameworks' names (`TextField`,
  `TextEditor`, `Picker`, `LazyColumn`, `derived`), never HTML element or React
  component names. Borrowed naming survives only where it is genuinely the domain's
  best term: the reactive vocabulary (`Signal`/`Memo`/`Effect`—industry-standard
  beyond any one framework), `Suspense`/`ErrorBoundary`, `reconcile`, and the
  web-facing terms of art (`hydrate`, `render_to_string`, the `head` module) in the
  layers that are web by definition.
- **Children come last.** Every view takes its child or children through a builder
  method, never a constructor argument, and a chain ends with them: properties first,
  content last. This is Flutter's `sort_child_properties_last` rule, and for the same
  reason—a nested child is visually large, so anything after it is easy to miss. A view
  of one child holds it in a slot `set_child` replaces (`HasChild`, `Container` alone);
  a layout view appends segments (`HasChildren`: `add_child`, `set_children`,
  `derive_children`). A view that wraps something (`Flexible`, `Pinned`, `GridCell`,
  `Theme`, `Visibility`, `Control`, `Overlay`, `Tooltip`, `Link` and their kin)
  requires the child in the type system: it holds the child as a type parameter,
  starting at `NoChild`, its setters exist only in that state, so a property after the
  child is a type error, and only the form carrying one is a view. `Theme` and
  `Deferred` take their child as a builder closure, `set_child_builder`, because it is
  built under the context or the boundary they provide, and `AsyncView` as a future's
  loader, `set_child_loader`.
- **One property, one setter.** Every property is settable exactly one way (plus its
  `derive_` twin). A concept with components gets one shared value type set as a
  whole—`Inset` for padding/margin, `Spacing` for per-axis spacing, `Flexibility` for
  grow/shrink/initial—never per-component setters (`set_padding_left`, `set_row_gap`).
  Convenience conversions (`From<f32> for Spacing`) keep the common case terse without
  adding a second path. Conditions ride the value (`value.when(condition, other)`, the
  `When` extension on any value or token), so there is no `set_<prop>_when` family.
- A type is named for the crate it belongs to, not the medium: `TerminalStyle` and
  `WebStyle` with their properties, fields and traits, `SimpleGlyphs` and every other
  `Simple*` item. A bare name is not an option where the facade's prelude already
  carries it (`Property`, `Value`, `Text`), and a scheme that is bare for some names
  and prefixed for others is worse than one rule.
- A platform's test seams share one naming rule: `trigger_<event>` simulates what reaches
  the app from outside, `take_<output>` drains what the app emitted since the last take,
  `render_to_<form>` paints into a form a test reads, `set_<setting>` configures the
  double, `is_<state>` answers a yes-or-no question, any other read is named for what it
  returns, and a user's gesture composed of events is named for the gesture in the
  platform's `user` module (`user::click`, `user::type_text`). Headless's are the crate's
  own, reached as `skene_web::headless`; the terminal's live in `skene_terminal::testing`; a
  browser test's helpers in `skene_web::testing`; and the accessibility queries every
  one of them offers are declared by `skene_foundation::access_queries!`.
- Make a function or method `const` whenever its body allows it (no trait-method calls such as
  `.into()`, no allocation, no interior mutability). Apply it broadly, and especially to small,
  frequently-constructed types like the geometry and value vocabulary. The API is early and
  unstable: prefer a signature that can be `const` over one that can't when the ergonomic cost is
  small.

## Tooling & workflow rules

- Always format with `cargo +nightly fmt`.
- **A dev reload snapshots once, at the swap, under a cooperative stop**, and the
  bytes go from the running app to its successor directly: the terminal app hears
  `swap` on its launcher's pipe, records to the unnamed file the launcher handed it by
  descriptor and becomes the new binary in place; the page records into the origin private file system on the
  reload message before it reloads. Nothing checkpoints per change and no server holds
  state. State crosses runs through `Snapshot`, the framework's own trait: a
  self-describing encoding a restore coerces across (a `bool` and an integer are one
  another, a missing field is its type's absent value, an unknown one is skipped, a
  gone variant is the default), derived with `#[derive(Snapshot)]`, never serde.
- **`cargo xtask gate` is the gate**: every check a change passes, in one command, and a
  change lands only when it reports `gate passed`. It checks formatting, every
  manifest's dependency order, its inheriting the workspace's lints and `rust-version`,
  its features enabling optional dependencies alone and a published crate's
  description, license and repository, the house rules no compiler checks (em-dashes
  unspaced, children last, no public name in another framework's vocabulary, fields and
  variants documented all or none, a test double's simulated events named `trigger_`,
  nothing in the foundation crate-private),
  the dependencies' licenses against the workspace's MIT, unused dependencies,
  clippy and rustdoc with warnings denied on all three leaves, clippy on the host in
  release as well, where code built for debug assertions alone drops out, each library
  crate's public API against its snapshot under `api/`, read by a pinned nightly and
  `cargo public-api`, clippy on the code written for Apple's platforms (the
  terminal's, on `aarch64-apple-darwin`), the three test tiers and the workspace on its
  declared `rust-version`. Everything but formatting, the API snapshots and that last
  check runs on the stable `rust-toolchain.toml` pins, so a Rust release changes nothing
  until a change of its own moves the pin and answers what the release asks. `cargo xtask gate test:browser` runs one step and `cargo xtask gate
  clippy` a group; each step logs to `target/gate/<step>.log`, and a failed one prints
  why beside the end of its log. A step that builds or runs anything is skipped while
  nothing it reads has changed since it last passed (every file but the documents, the
  toolchain, the browser drivers); `--force`, or a `CI` variable, runs it regardless.
  `cargo xtask api` rewrites the snapshots once a change to a surface is meant. CI runs
  the same command on every pull request.
- Tests run in three tiers, named by the tier that runs them, and the host tier alone
  proves nothing about the web: every `browser_*.rs` and `server_*.rs` is gated to a
  leaf, so on the host those binaries compile to zero tests, and a change can pass
  every host test while breaking the browser outright. The host tier
  runs through `cargo nextest run`, each test in a process of its own, the doctests
  beside it through `cargo test --doc`, and a crate's host integration tests are one
  binary (`tests/it`), since the process per test isolates them and every binary costs
  a link. Among it, `packages/web/tests/compile_fail`
  holds a program per rule the types enforce (layout's static constraints, children
  last, the expression kinds, the derives) beside the diagnostic it must raise,
  rewritten by `TRYBUILD=overwrite` once a change to one is meant, `proptest` drives
  the snapshot codec and the terminal parser with arbitrary input, and a terminal frame
  compares against its snapshot file through `insta` (`render_to_styled`: the text, each
  cell's style as a character, and what each stands for), rewritten by
  `INSTA_UPDATE=always` once a change to the frame is meant, and headless draws every
  page of the conformance suite in the pinned font (`render_to_image`) against its own
  golden (`packages/headless/tests/goldens`), rewritten by `SKENE_GOLDENS=overwrite`
  as the browser's are, and holds each golden to the browser's golden of its page, both
  blurred by three pixels, a pixel straying past what rasterizing glyphs apart accounts
  for failing it; where the engines size a page apart (an entry's intrinsic width),
  headless follows Chromium, so a page lays such a box out where its size is the
  layout's. The server tier is the server leaf built
  natively, the byte-exact render snapshot among it. The browser tier first links every
  library's tests for wasm32, so an app's own tests build there, then drives every
  browser binary, several sessions at once (`SKENE_GATE_BROWSERS` sets how many), through
  a real Firefox (geckodriver) and a real Chromium
  (chromedriver, from `CHROMEDRIVER`, `PATH` or `~/.local/share/chrome-for-testing`),
  each with WebGPU switched on by `packages/web/webdriver.json`, so a painted box
  renders on the GPU there, until headless Chromium's software adapter loses its
  device after the first frame and the box falls back to the CPU; without a GPU, Firefox finds an adapter only through Mesa's software Vulkan
  driver (`mesa-vulkan-drivers`, which CI installs); `browser_painted` hides
  `navigator.gpu` before its first box paints, so the CPU blit is proven in the same
  run. The tier then builds the conformance suite's app with the `skene` command and
  runs its harness in each engine beside the browser binaries, and on x86-64 Linux
  once more in a pinned Firefox it fetches into `target/browsers` and checks against
  its published digest, comparing every page of the suite, drawn in the pinned font
  the app serves under a fontconfig that admits no system font, pixel for pixel
  against its screenshot golden (`packages/conformance-web/goldens`), rewritten by
  `SKENE_GOLDENS=overwrite` once a change to a page's drawing is meant. The gate holds
  each browser binary to the least number of tests it lists for it in each engine,
  and the harness to the least number of scenarios and goldens, so a shrinking suite
  fails rather than passing quietly, and a new browser test, scenario or page raises
  its number there. By
  hand, read each binary's `test result:` line
  rather than cargo's exit code, and run Firefox's snap-bundled geckodriver
  (`GECKODRIVER=/snap/firefox/current/usr/lib/firefox/geckodriver`) with `TMPDIR` on a
  path the snap can read, as the gate does: the snap's own `geckodriver` cannot be
  stopped by the runner, which panics with `PermissionDenied` after reporting. A browser
  failure poisons the platform for the tests that follow (`RefCell already borrowed`),
  so fix the first failure before believing the rest.
- Prefer `#[expect(lint)]` over `#[allow(lint)]`.
- Order dependencies alphabetically by crate name within every Cargo manifest table.
- Macros must pull their weight: use one only when it eliminates real boilerplate or repetition
  (the entry proc-macro is the exemplar). Never reach for a macro that adds little while degrading
  `rustfmt`/rust-analyzer support, readability, or performance; prefer a function or generic when
  one suffices.
- All reactive-system work keeps the state snapshot in mind: signal identities must be
  able to stay stable across two runs of the same binary. Store/restore records wherever
  a dev channel is present at startup, costing a production build nothing, and is
  opt-out—the built-in and simple types just work with no tag—so `Signal::new` carries
  the `Snapshot` bound, `Signal::transient` opts a signal out and asks nothing of its type, and
  `Signal::tagged` is only an anchor for the positional diff within one owner, and a
  `Key` anchors a whole scope: `reconcile` keys every item's owner by the item's key and
  `View::set_key` builds a view under a keyed owner, one concept for both, so state
  under either restores wherever it sits. No cargo feature is involved:
  an additive feature can never gate a trait bound.
- Aim for the absolute least duplication that sacrifices neither ergonomics nor performance:
  when facades need the same machinery over different vocabularies, the spine hosts it once as
  a small monomorphized generic (`Scene<P>`, `Control<P, B>`, `ConditionTree`), each facade
  keeping its own flat, medium-typed surface. Never share via feature-gated variants or
  fields: cargo features are additive and unify across the build graph, so mutually-exclusive
  cfgs break any build containing both facades.
- Examples are targeted: one per feature, under `examples/web/<name>` and
  `examples/terminal/<name>` (folder names without facade suffixes, the same name on both
  facades where the feature exists), never crammed into an existing example. Every common
  user-facing capability gets one; the `gallery` example shows off every built-in view and
  control with minimal but pretty styling, and `theme` shows the token model.
  `examples/third-party` holds the low-level views written from outside the framework,
  one per platform's escape traits; Simple is the component library written the way a
  third party's is. `examples/docs` holds the crates compiling the documents' examples
  as doctests (`readme`, `guide`).
- **What comes next lives in `ROADMAP.md`**, under `Open`: one ordered list of every
  open item, big and small, top first. Nothing shipped is listed there. When asked
  what's next, answer from the top of it. A follow-up a change leaves behind is inserted
  there at its priority in the same change, never in a code comment, a commit message
  or an agent's memory; an item a change resolves is removed in the same change.
- **What was decided against lives in `DECISIONS.md`**, each alternative and each
  replaced design dated with the condition that reopens it. A decision reopens only
  when its condition holds, as an item in `ROADMAP.md`; a change that settles against
  an alternative records it there in the same change.
- Retrofit as you raise the bar: when a change establishes a better pattern (a helper, a naming
  convention, a clearer structure), bring existing code up to it rather than leaving two standards.
- Keep parallel code visually parallel: when two files do essentially the same thing in different
  places, shape them so a human can compare them at a glance—same names where they make sense,
  same order of items, same structure. (This is legibility, not coupling; sharing helpers to cut
  duplication is still encouraged.)
