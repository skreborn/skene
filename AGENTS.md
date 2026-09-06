# Agent & Contributor Guide

## Hard constraints

- Skene is a **client-side** framework. No server-side features, ever: no server
  functions, no backend framework integrations. SSR/SSG are rendering capabilities; the
  runner (`skene-runner`) is an embeddable library a user's server calls, not a server.
- The spine (`skene-reactive` + `skene-core` + `skene-views`) stays platform-agnostic: no
  platform assumptions or nomenclature. Core hosts the retained **scene** (`Scene<D>`:
  parents, children, per-child annotations, token definitions, generational node ids),
  the zero-sized **`Driver`** trait every driver implements, event dispatch over the
  scene, the property model (`BoxStyle<N, S, X>`, `RunStyle<S, X>` and their visitors),
  values and expressions (`Value`, `Leaf`, `Expr`), conditions (`Media`, `State`,
  `Condition`), tokens (`Token`, `TokenSet`), `Boundary`, motion and interpolation, the
  runtime **`Resolver`** every non-CSS driver resolves its styles through (with the
  erased `TokenValue` and `SlotValue` it reads), the host **executor** a driver
  installs, generic geometry over the driver's scalar, and universal values. `skene-views` hosts
  every built-in view once, generic over `D: Driver`; a facade aliases them.
- The canonical building block is a **view** (the `View` trait); never "widget". Two
  equal facades exist—`skene-gui` (pixel media: web, native later) and `skene-tui` (the
  terminal)—each owning its builder vocabulary, typed to exactly its medium; neither is
  the blessed one. Within a facade, views are realized **once** against its typed driver
  vocabulary—no per-platform `impl View`, no "backend" concept.
- A **driver** is a crate implementing `Driver`: `skene-web` (the csr and ssr leaves),
  `skene-headless` (the host driver every facade compiles against off its target, driven
  by tests), later `skene-native`; `skene-tui` carries its own. Drivers depend on core,
  never on each other; a facade is the only place that names its drivers, and it
  publishes the selected one as `ActiveDriver`. The trait is zero-sized and
  monomorphized: no `dyn` driver, no downcast, no runtime swap. It is **not sealed**, and
  the shared machinery bounds only on it and the public extension traits, so a
  third-party driver is a facade in all but ownership. Each driver crate also exposes an
  **escape trait** beside the portable one (`WebMarkup` on both web leaves, `WebDom` on
  the browser leaf only, `CellPainter` in the terminal, later `PlatformView`), so a
  low-level view is written against exactly the bound it needs and is realized through
  the same scene and node ids as a built-in (`skene_views::LowLevel` registers the sink
  it made as a node). `Painted` (vello) is the portable low-level path for pixel media,
  through core's `Painting` extension: the browser leaf renders the scene on the CPU
  into a canvas, the headless driver renders it to pixels a test reads, the server leaf
  renders it empty; the native realization comes with the native driver.
- Selection: by **facade dependency** (GUI apps depend on `skene-gui`, terminal apps on
  `skene-tui`), then by target within a facade (wasm32 → web; native triples → the
  native driver, later; every other host → the headless driver). No feature flags.
  Target-gating is the enforcement layer: the web csr/ssr leaves stay split on
  `target_os` (`unknown`/`wasi`), so the server leaf physically cannot touch the DOM. A
  cargo feature may only **add items**: never a trait bound, a type, a variant or a
  driver. No cargo feature exists in the workspace, and `cargo hack --feature-powerset`
  is part of the gate so none creeps back in.
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
  driver's mapping layer. This is settled—do not re-litigate it toward CSS naming.
- **Layout is statically constrained.** Each layout is its own container view—`Row`,
  `Column`, `Grid`, `Stack`—exposing only the properties it honors; per-child properties
  travel in wrapper views (`Flexible`, `Pinned`, `GridCell`) accepted only by the
  matching container through its child kind (`HasChildren::Kind`). Wrappers create no
  node; their properties become ordinary properties of the child's own style, folded in
  before the child is created, so they compile into the class and take conditions,
  tokens and derived values like any property. `Container` is decoration and sizing around
  **exactly one** child—no multi-child flow; `Column`, `Row`, and `Wrap` cover those
  cases. **Only `Container` is decorated**: it is the one freely-stylable box—
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
  expression does not compile); **derived** from signals by a closure; and any of those
  under a condition. The first three and the conditions compile to CSS, so SSR/SSG
  output is responsive, hoverable, themed and animated with **no JS/WASM**; a derived
  leaf is a per-node custom-property slot the class references, so it never outranks a
  variant. `derive_` closures are for **state**, never for environment or arithmetic.
- **A design token is a named, inherited slot; a theme defines it; a palette belongs to
  its library.** `TokenRef<T>` names a slot (a `const`; the name is an implementation
  detail on every platform, never a public hook), `Token<T>` pairs the name with the
  definition a palette gives it, and every setter takes `impl Into<Value<T>>`, so
  `set_color(Color::WHITE)`, `set_color(palette.card)` and
  `set_color(SimplePalette::CARD)` are the same call and a consumer emits only the name.
  A token's own conditions live in its **definition**, once: `Definition<T>` conditions
  on `Media` only and holds no closure, so a palette is plain data that serializes and
  hashes. On the web a token is a custom property defined on the theme's scope and
  referenced with `var()`; in the terminal and on native it resolves through the scene to
  the nearest defining ancestor, cached per node. Nothing below a component library
  knows what a palette is: `skene-simple` owns `SimplePalette` as a struct of `Token`s
  implementing `TokenSet`, and a third party's `AcmePalette` is on identical footing.
  - **`Theme<P: TokenSet>` is the one theme view**, in `skene-views`:
    `Theme::new(palette)` or `Theme::derive(|| palette.get())`, then
    `set_child_builder(|| child)`. It realizes one layout-invisible scope node, provides
    `Memo<P>` through context (`P::current()`, `P::watch()`, from `ThemedIn`), and one
    effect rewrites its definitions when the palette changes: no consumer is touched and
    nothing rebuilds, so a palette change costs the number of tokens, never the number
    of nodes. Nesting a theme redefines beneath it. The child is a builder closure
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
  - **A scheme an app controls is resolved by the driver, never by a rebuild.** A
    `derived` rebuild destroys the old nodes and disposes their scope—measured: a signal
    inside the subtree goes back to its initial value, and focus, scroll offsets and
    native input state go with it. So `derive_scheme` tells the driver which scheme is in
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
      after the paint, which is the flash it exists to prevent. `stored_scheme()` reads
      the same key so an app's setting starts where the page already is.
  - **The scene is the tree.** Every view attaches to its parent's scene node, wrappers
    realize nothing, and `derived`, `Suspense`, `ErrorBoundary`, `Deferred` and `empty`
    mount between **anchors** rather than inside a box, so a reactive subtree contributes
    its own nodes to its parent's layout. The platform's tree is a sink written from the
    scene, never read back for structure. Overlays are `<dialog>`/popover promoted to the
    top layer *in place* rather than portalled, and only the `<head>` is a real portal. A
    native driver may still realize an overlay into its own layer, so nothing is built on
    ancestor-selector reachability beyond a control's own subtree.
  - **Environment is queryable.** `media::viewport()`, `media::scheme()` and
    `media::reduced_motion()` are signals on every facade, `media::watch(&condition)`
    turns any declared condition into a memo, and a box's measured size reaches a view
    through `Driver::observe_size` and an app through a `SizeHandle` bound to the box. On
    the web they are live after hydration and hold the server's answer before it; the
    server's answer comes from client hints when the runner is given them, else from
    documented defaults. Declare what must be right before the bundle arrives; derive
    what may wait for it.
- **Progressive enhancement is doctrine.** Everything that can work without JavaScript
  must: server-rendered and statically-generated pages deliver their full content as
  markup (`#[lazy]` bodies included—the server leaf never splits), navigation works as
  real links, and the client bundle layers interactivity on top by hydrating in
  place—content is never gated on JS. Prefer the latest CSS standards and pay nothing
  for obscure or ancient engines; when a slightly older engine can be supported
  without giving anything up in new ones, support it.
- The **interner contract is load-bearing**: content-addressed class names
  (`skene-{hash:x}` from a wide hash), reference-counted, deterministic and identical
  across csr and ssr—hydration and stylesheet parity depend on it. The pure core lives
  in `skene-web` and is shared by both leaves; each layers only its sink.
- **Painted** content is vello wherever it is realized: a `Canvas` is a vello scene in
  device pixels, drawn with kurbo shapes and peniko brushes. The browser leaf renders it
  with vello's CPU renderer into a `<canvas>` sized to the box at the device pixel
  ratio, repainting on resize (a WebGPU renderer is a later upgrade behind the same
  `Canvas`); native layers come with the native driver. SSR/SSG render it empty; TUI
  does not realize it. **Embeds** are
  driver-specific views with driver-exposed constructors (raw DOM element on web csr,
  real platform controls on native); the payload never crosses a crate boundary. On
  platforms with a real view hierarchy the driver realizes the tree into it, so embeds
  and painted layers interleave in z-order by the platform compositor.
- **The TUI targets browser parity.** Wherever the terminal can express it, behavior
  follows a modern browser: layout, features, settings, colors, selection, and every
  interaction's shape—clicks fire on press+release over one target while focus, caret
  placement, and drags act on press; wheel gestures latch their scroll target and
  consume at its limits until a pause or pointer move; popups light-dismiss; selection
  gestures match (drag, double-click word, triple-click run). When an interaction's
  design is in doubt, mimic the web—**because the browser is the most constrained
  platform we target, not because it is the best design**. Parity is a floor. Where a
  browser behavior is merely a browser behavior, we are free to rule otherwise, and we
  record it here when we do.
- **A view that handles an interaction owns it.** Every control consumes the events it
  acts on: a press on a button is a press on the button, never also on the row it sits
  in. This is a **deliberate divergence**—a DOM `<button>` inside a clickable `<div>`
  does bubble to the div—settled because nested clickables are ambiguous to a user and
  the framework's idiom is a closure per view, not delegation. The cost is that a parent
  cannot observe its controls' clicks in one place; wrap the group in a `Container`,
  which is a box rather than a control and keeps the choice.
- **A verdict is the view's, never its user's.** `handle_<event>` takes a plain callback
  on every control, `Raw*` and `Simple*` alike. A control decides what the platform and
  the ancestors are told, once, where its behavior lives, so no app or library author
  asks "am I cancelling a platform default here". `Prevent` belongs to the layer that
  owns behavior (a `Behavior`, a driver); above a driver, nothing states one. A control
  the driver realizes never asks to suppress the platform's response, because there
  that response **is** the control. A box keeps the one decision that is genuinely its
  own: `Container`'s handlers return `Into<Bubbling>`—`()` for the ordinary case,
  `Bubbling::Stop` to keep the event from the ancestors—so a box chooses whether it
  consumes, and a control has that chosen for it. The name is deliberate: it is about
  onward travel only, since a view may handle an event and still let it climb. Nothing
  above a driver states more. Dispatch itself lives in core over the scene: a driver
  reports an event at a node, core walks the parents, runs handlers deepest first,
  folds the verdicts and tells the driver what the platform may do—so every driver, the
  headless one included, bubbles by one implementation. A click on a native web toggle
  is dispatched *observed* (`dispatch_click_observed`): every handler up the path runs
  except the acting one the behavior registered, since the platform's flip arrives as
  `change`.
- **A control is `Control<B: Behavior>`**: one builder in `skene-views` carrying focus,
  events, decoration, label, parts and accessibility once. `Toggle`, `Range`, `Roving`,
  `Popup`, `Activate`, `TextEntry` and `Listbox` are the shipped behaviors
  (`RadioMember` and `ListItem` the members'), `Behavior` is public, and the keyboard
  tables live in the behaviors, so a third party's control
  and the terminal's slider get the same keys as the browser's. `RawButton`,
  `RawCheckbox`, `RawSlider`, `RawRadioGroup`/`RawRadio`, `RawListbox`/`RawOption`,
  `RawTextField` and `RawTextEditor` are aliases over them, and `RawPicker` is composed
  from them. A control is **controlled** on every facade: it reports, and the bound
  signal is the truth. A `State` a box cannot have on its own (`Checked`,
  `Indeterminate`, `Disabled`, `Invalid`, `Open`) resolves against the enclosing
  control; a node's own `Hovered`/`Focused` stay its own. Parts (`Track`, `Fill`,
  `Thumb`, `Mark`, `Face`, `List`, `Item`, `Label`) are the styling targets a control
  exposes; in the terminal the checkbox and radio `Mark` and the picker's indicator are
  glyphs from the Simple library's `SimpleGlyphs` token set, and the slider's track, fill and thumb are boxes
  there as everywhere. On the web a control that owns
  a native input keeps it as its scriptless form—checkbox, radio, slider and the text
  fields need no swap after hydration—and the picker keeps a swap that carries the
  reader's value. The terminal's controls are composed trees painted by the same box
  and run painters as everything else, so they carry roles, accessibility and focus
  order like the GUI's.
- **Third parties are first-class by construction.** A component library depends on
  `skene-views`, writes generic over `D: Driver` exactly as the built-ins do
  (`Container<D>`, `Control<D, Toggle>`, `Theme<D, AcmePalette>`), states what it needs
  from a medium as a bound on the driver's associated types (`D::StyleExt: CornerRadius`
  rounds where the medium can and writes nothing where it cannot; a capability a library
  *requires* is a hard bound, so the library does not exist on a medium lacking it), and
  works on every facade from one crate. A low-level view is written against a driver's
  escape trait and selected through the facade's `ActiveDriver`. Nothing on the path a
  built-in takes is crate-private; making something `pub(crate)` on that path is a
  regression. `examples/third-party` is the standing proof and part of the gate.
- **Motion is declared once and runs everywhere.** `Transition` and keyframe `Animation`
  are data. The web compiles them to CSS; native and the terminal run them on the
  driver's frame clock from the resolution point that reports a changed value, never
  through the reactive graph per frame. Every change source animates: a condition
  flipping, a token redefined, an expression input moving, a derived write. Reduced
  motion is respected by default. `set_transition_name` and the router's view-transition
  wrap cover same-document transitions, `@view-transition` the cross-document ones, and
  both degrade to a cut.
- View logic tests run headless via `skene-headless`, the host driver, which implements
  the whole `Driver` trait so nothing is wasm-gated for want of a driver; its executor
  runs when a test calls `pump_tasks`, so a `Resource` loads in a test too. Drivers
  test against their real sinks (web: real DOM via `wasm-bindgen-test`; ssr: wasmtime
  under a byte-exact snapshot; native: its toolkit). There is no mock driver.
- HTTP, WebSockets and timers are userland. The framework ships the seams—`spawn`,
  `Resource`, `Boundary`, the server leaf's reactor (`register` for a `wasi:*`
  pollable, `sleep`, the render deadline)—so third-party integrations sit on the same
  footing first-party ones would. (Exception: a native driver
  owning its event loop may ship a loop-integrated timer.)

## Style and documentation conventions

- Use 2-space indentation.
- Never surround an em-dash with spaces.
- Attach a helper to the type it operates on as a method, not a free function.
- Put `#[must_use]` on the type itself, not on the functions that return it, whenever it
  semantically makes sense.
- Apply `#[inline(always)]` wherever appropriate: the framework has a great many tiny functions
  that are best inlined across crate boundaries.
- Document every public API; comment non-public code only when critical.
- Give a purpose-built trait a `#[diagnostic::on_unimplemented]` message: these traits
  exist to express one rule, so a failed bound should state that rule rather than the
  mechanics. Where a blanket impl would be blamed instead, mark it
  `#[diagnostic::do_not_recommend]` so the message the author needs is the one shown.
- Within a type's own `impl` blocks, refer to that type as `Self`, never by name (`Self::ZERO`,
  `Self(value)`, `-> Self`). Its name may still appear for a *different* type, e.g. the source type
  in a `From` impl.
- Comments state contracts only, never history, rationale, "why", or implementation.
- A comment's first line is a short summary; add further lines only when needed to use the item.
- Split a comment into paragraphs when it is long or covers distinct points.
- Comment a struct's fields or an enum's variants all or none: never document some siblings and
  leave others bare. The only exception is when only the public fields/variants are documented.
- Omit blank lines only between lines of the same shape (consecutive `let`s, struct fields).
- Surround every `return`, conditional, loop, block, and multi-line statement with a blank line.

## Naming & API conventions

- The spine stays minimal and structural. The built-in views live in `skene-views`,
  written once against `Driver`; a facade aliases them
  (`type Container = skene_views::Container<ActiveDriver>`). Users extend by composing
  built-ins, painting a `Painted` node, or authoring a view against a driver's escape
  trait.
- Builder methods: `set_<prop>` for a fixed value, `derive_<prop>` for a reactive one,
  `handle_<event>` for an event handler (e.g. `Container::handle_click`). A driver
  function that realizes the handler is named the same.
- The layout vocabulary rule extends to the whole surface: views, controls, and
  composition helpers take the platform-native frameworks' names (`TextField`,
  `TextEditor`, `Picker`, `LazyColumn`, `derived`), never HTML element or React
  component names. Borrowed naming survives only where it is genuinely the domain's
  best term: the reactive vocabulary (`Signal`/`Memo`/`Effect`—industry-standard
  beyond any one framework), `Suspense`/`ErrorBoundary`, `reconcile`, and the
  web-facing terms of art (`hydrate`, `render_to_string`, the `head` module) in the
  layers that are web by definition.
- **Children come last.** Every view takes its child or children through a builder
  method (`set_child`/`set_children`/`derive_children`), never a constructor argument, and
  a chain ends with them: properties first, content last. This is Flutter's
  `sort_child_properties_last` rule, and for the same reason—a nested child is visually
  large, so anything after it is easy to miss. A view that exists only to wrap something
  (`Flexible`, `Pinned`, `GridCell`, `Theme`) goes further and requires the child in
  the type system: it holds the child as a type parameter, starting at `NoChild`, and
  only the form carrying one is a view. `Theme` alone takes its child as a builder
  closure, `set_child_builder`, because it provides context the child is built under.
- **One property, one setter.** Every property is settable exactly one way (plus its
  `derive_` twin). A concept with components gets one shared value type set as a
  whole—`Inset` for padding/margin, `Spacing` for per-axis spacing, `Flexibility` for
  grow/shrink/initial—never per-component setters (`set_padding_left`, `set_row_gap`).
  Convenience conversions (`From<f32> for Spacing`) keep the common case terse without
  adding a second path. Conditions ride the value (`value.when(condition, other)`, the
  `When` extension on any value or token), so there is no `set_<prop>_when` family.
- A test double's method that simulates an external event is named `trigger_<event>`.
- Make a function or method `const` whenever its body allows it (no trait-method calls such as
  `.into()`, no allocation, no interior mutability). Apply it broadly, and especially to small,
  frequently-constructed types like the geometry and value vocabulary. The API is early and
  unstable: prefer a signature that can be `const` over one that can't when the ergonomic cost is
  small.

## Tooling & workflow rules

- Always format with `cargo +nightly fmt`.
- Tests run in three tiers, and the host tier alone proves nothing about the web. Run
  `cargo test --workspace` for the host, `cargo test -p skene-gui --target wasm32-wasip2` for
  the server leaf, and `cargo test -p skene-gui --target wasm32-unknown-unknown` for the
  browser leaf, which drives a real Firefox through geckodriver. Every `web_*.rs` is gated to
  a wasm target, so on the host those binaries compile to zero tests and report
  `ok. 0 passed`—compiling them (`cargo check`) is not running them, and a change can pass
  every host test while breaking the browser outright. Read the `test result:` line rather
  than the exit code or a `FAILED` grep, because both wasm tiers mislead: wasmtime aborts on
  the first panic, hiding every test after it in that binary and printing `Error: failed to
  run main module` instead of a `FAILED` line, while the browser runner panics on cleanup
  after reporting and exits non-zero even when everything passed. That cleanup panic
  (`PermissionDenied`) also stops cargo before the remaining binaries run, so the one-command
  browser form silently covers only the first of them—run each browser binary by name
  (`--test web_dom`, `web_hydrate`, `web_resource`, `web_task`; the other `web_*.rs` are
  wasi-gated and report `no tests to run!` here) and add the totals, or a shrinking suite
  reads as a passing one. `skene-web` and `acme-map` carry browser binaries of their own
  (`web_dom`, `web_map`); run them the same way. A browser failure also poisons the platform
  for the tests that follow (`RefCell already borrowed`), so fix the first failure before
  believing the rest.
- Prefer `#[expect(lint)]` over `#[allow(lint)]`.
- Order dependencies alphabetically by crate name within every Cargo manifest table.
- Macros must pull their weight: use one only when it eliminates real boilerplate or repetition
  (the entry proc-macro is the exemplar). Never reach for a macro that adds little while degrading
  `rustfmt`/rust-analyzer support, readability, or performance; prefer a function or generic when
  one suffices.
- All reactive-system work keeps the state snapshot in mind: signal identities must be
  able to stay stable across two runs of the same binary. Store/restore is on by default
  and opt-out—the built-in and simple types just work with no tag—so `Signal::new` carries
  the serde bounds, `Signal::opaque` opts a signal out and asks nothing of its type, and
  `Signal::tagged` is only an anchor for the positional diff. No cargo feature is involved:
  an additive feature can never gate a trait bound.
- Aim for the absolute least duplication that sacrifices neither ergonomics nor performance:
  when facades need the same machinery over different vocabularies, the spine hosts it once as
  a small monomorphized generic (`Scene<D>`, `Control<D, B>`, `Composable`), each facade
  keeping its own flat, medium-typed surface. Never share via feature-gated variants or
  fields: cargo features are additive and unify across the build graph, so mutually-exclusive
  cfgs break any build containing both facades.
- Examples are targeted: one per feature, under `examples/gui/<name>` and
  `examples/tui/<name>` (folder names without facade suffixes, the same name on both
  facades where the feature exists), never crammed into an existing example. Every common
  user-facing capability gets one; the `gallery` example shows off every built-in view and
  control with minimal but pretty styling, and `theme` shows the token model.
  `examples/third-party` holds the crates written from outside the framework, one per
  extension path.
- Retrofit as you raise the bar: when a change establishes a better pattern (a helper, a naming
  convention, a clearer structure), bring existing code up to it rather than leaving two standards.
- Keep parallel code visually parallel: when two files do essentially the same thing in different
  places, shape them so a human can compare them at a glance—same names where they make sense,
  same order of items, same structure. (This is legibility, not coupling; sharing helpers to cut
  duplication is still encouraged.)
