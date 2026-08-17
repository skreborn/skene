# Agent & Contributor Guide

## Hard constraints

- Skene is a **client-side** framework. No server-side features, ever: no server
  functions, no backend framework integrations. SSR/SSG are rendering capabilities; the
  runner (`skene-runner`) is an embeddable library a user's server calls, not a server.
- The spine (`skene-reactive` + `skene-core`) stays platform-agnostic: no platform
  assumptions or nomenclature. Core hosts the reactive engine, the view spine, the typed
  property vocabulary (`BoxProps`, `RunProps`, later `StyleSet`), `Boundary`, generic
  geometry (over a plain `f32` scalar), and universal values.
- The canonical building block is a **view** (the `View` trait); never "widget". Two
  equal facades exist—`skene-gui` (pixel media: web, native later) and `skene-tui` (the
  terminal)—each owning its builder vocabulary, typed to exactly its medium; neither is
  the blessed one. Within a facade, views are realized **once** against its typed driver
  vocabulary—no per-platform `impl View`, no "backend" concept.
- A GUI **driver** is a separate crate (`skene-web`, later `skene-native`) exporting the
  same typed vocabulary mapped to its own sink; `skene-tui` carries its driver
  internally. Drivers depend on core, never on each other; a facade is the only place
  that names its drivers. The structural machinery (`Children`, `reconcile`, `derived`)
  lives in core, generic over a zero-sized `TreeOps` parameter each facade aliases
  away—monomorphized, so there is no `dyn` driver, no downcast, and no runtime swap.
- Selection: by **facade dependency** (GUI apps depend on `skene-gui`, terminal apps on
  `skene-tui`), then by target within a facade (wasm32 → web; native triples → the
  native driver, later). No feature flags. Target-gating is the enforcement layer: the
  web csr/ssr leaves stay split on `target_os` (`unknown`/`wasi`), so the server leaf
  physically cannot touch the DOM.
- The **style model is data** (`StyleSet`: typed props + open per-family passthrough,
  media/state variants, transitions, keyframes). Layout **runs on taffy**—the framework
  invents no layout system and exposes taffy's full capability—but the **public
  vocabulary is not CSS's**: names and values follow the modern declarative frameworks
  (Flutter, SwiftUI, Jetpack Compose), mixed and matched for whichever is clearest—
  explicit and self-describing; CSS naming stays only where it is genuinely the best
  name in context. Concretely: distributions are *arrangements* (`arrangement`,
  `line_arrangement`: `Arrangement`), per-item properties are *alignments*
  (`item_alignment`, two-axis `Alignment` for grids and stacks)—never
  `justify-*`/`align-*`; `Dimension::Fraction` is a ratio of the parent (`0.5` = half,
  not clamped, not a weight); weights are the wrappers' grow/shrink factors and
  `GridTrack::Flex`; `spacing`, not `gap`; `columns`/`implicit_columns`/`flow` on a
  grid, not templates and auto-flow. CSS nomenclature lives only inside the web
  driver's mapping layer. This is settled—do not re-litigate it toward CSS naming.
- **Layout is statically constrained.** Each layout is its own container view—`Row`,
  `Column`, `Grid`, `Stack`—exposing only the properties it honors; per-child properties
  travel in wrapper views (`Flexible`, `Pinned`, `GridCell`) accepted only by the
  matching container through its child kind (`HasChildren::Kind`). Wrappers create no
  node; they annotate their child's. `Container` is decoration and sizing around
  **exactly one** child—no multi-child flow; `Column`, `Row`, and `Wrap` cover those
  cases. **Only `Container` is decorated**: it is the one freely-stylable box—
  decoration, sizing, events, alignment, and a single child—and every layout view
  carries layout properties plus its own padding and nothing else, so decorating a
  layout means wrapping it. Conditional styling is per property (`set_spacing_when`)
  rather than a whole style under one condition, which keeps a variant expressible only
  where the property is. There is no box-surface forwarding to solve, and no trait for
  it. A control that draws nothing needs none of it either—its chrome is the `Container`
  its child is made of. A control that owns an element of its own spells that element's
  decoration per property too, through `HasDecoration` (`HasMarks` in the terminal,
  where a control decorates its marks rather than a surface); the `BoxStyle` blob is
  gone from every control surface. A
  third-party layout view is built from the public `LayoutBox` exactly as `Row` is;
  there is no privileged base.
  On the web a `StyleSet` compiles to real CSS inside one interned class, so SSR/SSG
  output is responsive, hoverable and animated with **no JS/WASM**. `derive_` closures
  remain for genuinely dynamic values only.
- **A subtree's colors come from one theme, and the scheme is a condition.** `SimpleTheme`
  holds a light and a dark `Palette` and provides them through context; a themed property
  is spelled `theme_<prop>(|palette| palette.token)` and expands to ordinary conditional
  styling—the light value plus a `DarkMode` variant—so light/dark costs no script and a
  server-rendered page answers the reader's preference before any bundle arrives. Fixing
  the scheme (`set_scheme`) collapses that to one value. There is no reactive scheme
  setter: a variant is static data by the time a driver sees it, so a scheme bound to a
  user setting rebuilds the subtree through `derived`, which is the mechanism that
  already exists. The theme's child is a closure, since a `theme_` setter resolves when
  it is called and an argument is evaluated before its call.
- **Progressive enhancement is doctrine.** Everything that can work without JavaScript
  must: server-rendered and statically-generated pages deliver their full content as
  markup (`#[lazy]` bodies included—the server leaf never splits), navigation works as
  real links, and the client bundle layers interactivity on top by hydrating in
  place—content is never gated on JS. Prefer the latest CSS standards and pay nothing
  for obscure or ancient engines; when a slightly older engine can be supported
  without giving anything up in new ones, support it.
- The **interner contract is load-bearing**: content-addressed class names
  (`skene-{hash:x}`, forward probe on collision), reference-counted, deterministic and
  identical across csr and ssr—hydration and stylesheet parity depend on it. The pure core
  lives in `skene-core`; each driver layers only its sink.
- **Painted** content is vello, everywhere interactive: native layers, web `<canvas>`
  via wgpu. SSR/SSG render it empty; TUI does not realize it. **Embeds** are
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
  owns behavior; above a driver, nothing states one. A control the driver realizes never
  asks to suppress the platform's response, because there that response **is** the
  control. A box keeps the one decision that is genuinely its own: `Container`'s
  handlers return `Into<Bubbling>`—`()` for the ordinary case, `Bubbling::Stop` to keep
  the event from the ancestors—so a box chooses whether it consumes, and a control has
  that chosen for it. The name is deliberate: it is about onward travel only, since a
  view may handle an event and still let it climb. Nothing above a driver states more.
- View logic tests run headless where possible, via the recording driver
  (`skene-record`, the host driver). Drivers test against their real sinks (web: real DOM via `wasm-bindgen-test`; ssr:
  wasmtime under a byte-exact snapshot; native: its toolkit). There is no mock driver.
- HTTP, WebSockets and timers are userland. The framework ships the seams—`spawn`,
  `Resource`, the SSR reactor's public `register`, `Boundary`—so third-party integrations
  sit on the same footing first-party ones would. (Exception: a native driver owning its
  event loop may ship a loop-integrated timer.)

> **Transition.** The crate split and realize-once views are in place (`packages/skene`,
> `packages/web`). `DESIGN.md` is the target; `ROADMAP.md` tracks which phase the code is
> in. New work follows the target.

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

- The spine stays minimal and structural. The built-in views live in the umbrella
  (`skene`), realized once against the driver vocabulary; users extend by composing
  built-ins, painting a `Painted` node (once it lands), or authoring a driver-specific
  view against the driver's escape hatch.
- Builder methods: `set_<prop>` for a fixed value, `derive_<prop>` for a reactive one,
  `theme_<prop>` for one taken from the inherited theme's palette, `handle_<event>` for
  an event handler (e.g. `Container::handle_click`). A driver function that realizes the
  handler is named the same.
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
  (`Flexible`, `Pinned`, `GridCell`, the themes) goes further and requires the child in
  the type system: it holds the child as a type parameter, starting at `NoChild`, and
  only the form carrying one is a view.
- **One property, one setter.** Every property is settable exactly one way (plus its
  `derive_` twin). A concept with components gets one shared value type set as a
  whole—`Inset` for padding/margin, `Spacing` for per-axis spacing, `Flexibility` for
  grow/shrink/initial—never per-component setters (`set_padding_left`, `set_row_gap`).
  Convenience conversions (`From<f32> for Spacing`) keep the common case terse without
  adding a second path.
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
  reads as a passing one. A browser failure also poisons the platform for the tests that
  follow (`RefCell already borrowed`), so fix the first failure before believing the rest.
- Prefer `#[expect(lint)]` over `#[allow(lint)]`.
- Order dependencies alphabetically by crate name within every Cargo manifest table.
- Macros must pull their weight: use one only when it eliminates real boilerplate or repetition
  (the entry proc-macro is the exemplar). Never reach for a macro that adds little while degrading
  `rustfmt`/rust-analyzer support, readability, or performance; prefer a function or generic when
  one suffices.
- All reactive-system work keeps the state-snapshot use case in mind (ROADMAP backlog:
  serializable reactive state): signal identities must be able to stay stable across two runs
  of the same binary. Store/restore is on by default and opt-out—the built-in and simple types
  just work with no tag—so `Signal::new` carries the serde bounds, an explicit constructor
  opts a signal out and asks nothing of its type, and a tag is only an override for the
  positional diff. (This reverses the former opt-in-only rule and retires the `snapshot`
  cargo feature: an additive feature can never gate a trait bound.)
- Aim for the absolute least duplication that sacrifices neither ergonomics nor performance:
  when facades need the same machinery over different vocabularies, the spine hosts it once as
  a small monomorphized generic (`TreeOps`, `Composable`), each facade keeping its own flat,
  medium-typed surface. Never share via feature-gated variants or fields: cargo features are
  additive and unify across the build graph, so mutually-exclusive cfgs break any build
  containing both facades.
- Examples are targeted: one per feature, under `examples/gui/<name>` and
  `examples/tui/<name>` (folder names without facade suffixes), never crammed into an
  existing example. Every common user-facing capability gets one; the `gallery`
  example shows off every built-in view and control with minimal but pretty styling.
- Retrofit as you raise the bar: when a change establishes a better pattern (a helper, a naming
  convention, a clearer structure), bring existing code up to it rather than leaving two standards.
- Keep parallel code visually parallel: when two files do essentially the same thing in different
  places, shape them so a human can compare them at a glance—same names where they make sense,
  same order of items, same structure. (This is legibility, not coupling; sharing helpers to cut
  duplication is still encouraged.)
