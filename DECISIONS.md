# Decisions

The alternatives set aside and the designs replaced, each with the date it was recorded
and the condition that reopens it. A reopened decision becomes an item in `ROADMAP.md`.

## Decided against

- **Signals shared across threads** (2026-09-29), as a registry each runtime drains
  through an inbox: a handle stays on its thread, and another thread's work reaches it
  through a task that thread's executor wakes. Revisit when the hop through the owning
  thread's executor dominates a real app's cross-thread update.
- **Hot-patching** (2026-09-16): state-preserving reloads come from the snapshot
  instead. Revisit when a reload through the snapshot is too slow to keep a developer's
  flow and a hot-patching toolchain serves both wasm and native.
- **One vocabulary across every platform** (2026-10-01), the terminal measuring in
  logical pixels: pixels unify the numbers but not the medium, since a border still
  costs a whole cell, small spacings round to nothing or to a whole row and a type
  hierarchy collapses into one row outside kitty, so writing once yields a correct
  structure and a poor look while every property owes a floor on every platform. Each
  platform keeps its own unit and its own vocabulary: types decide what a platform can
  do, and signals what an install of it can. Revisit when the terminals worth targeting
  draw borders, spacings and sizes below the cell as a matter of course.
- **A shell per platform family** (2026-10-01), one for the web and the desktops and one
  for both mobile OSes: iOS and Android differ in representation, app structure and
  navigation, and each desktop OS in its layout and control conventions. Revisit when
  two platforms of one family converge on their representation, structure and
  conventions.
- **A capabilities model of typed slots on `Platform`** (2026-09-17) in place of
  progressive enhancement: shelved, since it puts bounds on every shared signature; only
  a low-level view names what a platform can do. Revisit when an ordinary view, not a
  low-level one, needs to name what a platform can do.
- **Draw-everything as the native family's default** (2026-09-17): mobile text
  selection, IME and edit menus are years of platform work no framework should redo.
  Revisit when a drawn framework ships platform-grade text selection, IME and edit menus
  on both mobile OSes.
- **A hybrid native realization** (2026-09-28), painted content with native controls
  embedded in it: every embedded view is paid for, as Flutter's platform views show.
  Revisit when an embedded native view costs a frame no more than one in the platform's
  own hierarchy.
- **A painted native platform** (2026-09-28), on the desktop or on Android through
  Linebender's android-view: it meets the native family's non-negotiables only as that
  hybrid, and like Compose it owns text editing, the part left to the platform. Revisit
  when a painted platform can leave text editing to the platform without embedding its
  controls.
- **A WASI server leaf** (2026-09-17), under a wasmtime runner or as a `wasi:http`
  handler: it needs a second transport for every client, and the demand for wasm
  rendering is on JavaScript isolates. The server leaf compiles natively, selected by
  `--cfg skene_platform="server"`, so every crate on the host reaches a render; a wasm
  host is a runner someone may write, for pages that await nothing until the settle loop
  is async. Revisit when an app must render where only WASI runs, and WASI's clients
  cover what its pages fetch.
- **Islands architecture** (2026-09-16): compiled no-JS styling already removes most
  reasons a static page ships wasm. Revisit when static pages measured in practice ship
  wasm for a small interactive part of a large page.
- **LiveView-style server rendering** (2026-09-16): adjacent to the server-side
  non-goal. Not revisited while Skene is a client-side framework.
- **A webview desktop shell** (2026-09-16): a desktop gets a native shell of its own.
  Revisit when the native desktop shells are given up.
- **A lazy split of the WebGPU renderer** (2026-09-16), measured 2026-09-15 on release
  builds after `wasm-bindgen`: an app that paints nothing (`counter`) ships 911 KB
  carrying 10.6 KB of wgpu code and no shader text, since the linker drops what no box
  reaches; an app that paints (`gallery`) ships 1.9 MB carrying 135 KB for the GPU path
  beside the 271 KB CPU renderer it needs either way. Nothing worth a split. Revisit
  when an app that paints nothing carries more than a few kilobytes of the GPU path, or
  a painting app's GPU path outweighs its CPU renderer.
- **Rendering the whole terminal frame as one image** (2026-09-16): the text would then
  be ours to rasterize, and the terminal's font, selection, copy and accessibility would
  go with it, a poor GUI in place of a good terminal. Revisit when a terminal lets an
  image carry selectable, copyable and accessible text.
- **Any terminal enhancement gated by a terminal's name** (2026-09-16): a name
  (XTVERSION, `TERM`) is not a capability; a parsed reply proves one. Not revisited.
- **Inferring a terminal's depth** (2026-09-16): a terminal answering no DECRQSS paints
  at 16 colors, Terminal.app among them; the cost of inferring nothing, taken knowingly.
  Client hints are likewise Chromium-only (`Accept-CH`/`Critical-CH`), and other engines
  get the documented defaults. Revisit when a terminal that answers no DECRQSS shows
  truecolor to many of an app's users, or another engine sends client hints.
- **A vertical fractional edge in the terminal** (2026-09-16): only the lower block
  glyphs exist, so a top-anchored fill would need upper ones; a vertical fill ends on a
  whole cell. Revisit when the fonts terminals ship carry the upper eighth blocks of
  Symbols for Legacy Computing.
- **Moving an editor's caret off the cell past a full row** (2026-09-28), where a
  wrapping space was consumed: a browser's caret hangs past such a row into the
  textarea's padding, and the terminal's bar cursor in the cell past it, the padding or
  the border, shows the same position. Revisit when browsers stop hanging the caret past
  such a row.
- **Replacing `IntoChildren` and `IntoChildOf` with std traits** (2026-09-16), checked
  against the compiler: a blanket `impl<I: IntoIterator<Item = V>> IntoChildren for I`
  conflicts with the tuple impls, and the same conflict appears with `From`; tuples are
  not optional because children are heterogeneous. Revisit when coherence admits such a
  blanket impl beside the tuple impls on stable Rust.
- **A collision check on snapshot keys at every build** (2026-09-16): a key is matched
  by a 127-bit hash of its value, so two values hashing alike share one scope and their
  state crosses on a restore, a risk smaller than the check's cost. Revisit when two
  keys are seen to collide; the route is then a keyed step carrying the value's postcard
  bytes and a type hash, at a `Serialize + DeserializeOwned` bound on every key and a
  heap step per keyed scope.
- **A `&'static str` signal restoring** (2026-09-16): it is `transient`, since it cannot
  be deserialized owned. Revisit when apps keep state a reload should carry in `&'static
  str` signals.
- **A `HashMap` store field iterating in a stable order** (2026-09-16): its entries come
  in its iteration order; an ordered list wants a `BTreeMap` or a keyed `Vec`. Revisit
  when an app needs an order neither a `BTreeMap` nor a keyed `Vec` gives.
- **Another reactive engine** (2026-09-28): Leptos's `reactive_graph` schedules effects
  asynchronously behind `Arc` and `RwLock`, breaking the synchronous render-first flush
  the terminal's key defaults rely on; `sycamore-reactive` has eager memos and no render
  flavor; the Rust alien-signals port has no owners, contexts or cleanups. Its
  techniques are ported instead. Revisit when an engine offers a synchronous
  render-first flush with owners, contexts and cleanups.
- **Another terminal parser, or one beneath it** (2026-09-28): crossterm parses no
  replies, termina lacks APC and re-parses byte by byte, vte has no APC dispatch, and
  termwiz is heavy. Revisit when a parser handles the replies and APC strings this one
  does at no more weight.
- **The `color` crate for `Color`** (2026-09-28): byte-identical class names across the
  server and the browser need `libm` math, which its `libm` feature cannot promise once
  peniko and vello unify it with `std`; its CSS semantics are taken instead. Revisit
  when the crate's math is `libm` under every feature its dependents unify.
- **A view macro (`rsx!`) beside the builders** (2026-09-28): a macro costs rustfmt and
  rust-analyzer, and Freya left rsx for builders in 2026; the effort goes into
  diagnostics. Revisit when rustfmt and rust-analyzer format and complete inside a macro
  as they do outside it.
- **Collapsing the crates beyond `skene-foundation`** (2026-09-28): separate crates
  compile in parallel. Revisit when a measurement shows the crate boundaries costing
  more build time than their parallelism saves.
- **One app serving both media** (2026-09-28): the two galleries still differ on 431 of
  893 lines once numbers are stripped. Revisit when the galleries converge on most of
  their lines once numbers are stripped.
- **A stable plugin ABI** (2026-09-28): nobody asks for one. Revisit when someone asks
  for one.
- **A property table giving each property an interpolation class and a unit class**
  (2026-09-29): a value's `Interpolate` answers per pair, as CSS computes it per pair (a
  length and `Auto` are discrete, two lengths are not), and the value's variant is its
  unit. Revisit when a pair of values needs an interpolation its types cannot answer.

## Superseded

- **Per-platform view realization** (2026-07-31) fell to the orphan rule the moment
  platforms became separate crates, and to the duplication it forced. The typed
  vocabulary replaces it. Not revisited while platforms are separate crates.
- **"No framework style model"** (2026-07-31) existed so no platform would be forced
  onto a foreign vocabulary. Taffy-everywhere plus no-JS responsive SSG *is* a shared
  vocabulary by requirement; the model is framework-defined, and a medium extends it
  through its typed extension. Revisit when a medium needs a property its typed
  extension cannot carry.
- **"Selection is the target, only"** (2026-07-31) cannot distinguish desktop from the
  terminal on one triple. Target-gating is demoted from selector to enforcement. Revisit
  when every platform has a target triple of its own.
- **A `Backend` trait object** (2026-07-31) cost `unsafe`, `Box::leak`, and a fallible
  downcast. Still rejected: platforms are statically selected and the platform parameter
  monomorphizes. Revisit when a platform must be chosen at run time.
- **"The built-in views live in the umbrella"** (2026-09-03). True with one facade; with
  two it produced two umbrellas and two copies, sixty-five percent of them
  character-identical. The generic parameter already used for `Children` is applied to
  every view in `skene-foundation`. Revisit when a single facade remains.
- **"No persistent scene tree in core"** (2026-09-03). The objection stood for a tree
  that calls platforms. The scene calls none; every platform kept the structure anyway,
  four ways. Revisit when a platform keeps no structure of its own.
- **"The vocabulary is free functions resolved by name"** (2026-09-03). A missing
  function was only an error on a target-gated path, which let the host platform lack a
  third of it. A zero-sized trait makes the contract complete. Not revisited.
- **A recording platform (`skene-record`) beside the real ones** (2026-09-17). It ran
  only the target's handlers, did not unlink on reparent, and could not be complete
  because nothing forced it to be. The headless platform implements the whole trait and
  is selected by the web facade on the host, so it cannot drift. Revisit when the
  headless platform cannot implement the whole trait.
- **Group nodes for reactive subtrees** (2026-09-05). A real `<div>` and a real taffy
  node made every derived subtree one flex item. Subtrees mount between anchors. Revisit
  when a platform needs a node per reactive subtree.
- **A style blob per control (`BoxStyle` handed to `set_style`), the `set_<prop>_when`
  families and a whole-style `set_when`** (2026-09-05). A whole-style variant could set
  properties the view does not honor; a per-property setter exists only where the
  property does, and a condition rides the value. Revisit when styles arrive as data an
  app loads at run time, which no type checks.
- **`Adaptive<T>`: a token's conditions landing beside the property as ordinary
  variants** (2026-08-17). They were copied into every use site, doubled by the scheme
  guard, and a palette could change only by a rebuild that lost state. A token is a
  named inherited slot; its conditions live in its definition; a theme redefines it by
  one effect. Revisit when a token's conditions must differ per use site.
- **`SimpleTheme::adaptive(light, dark)`, a light/dark palette pair, and a whole-palette
  `when(condition, overlay)`** (2026-08-17). Each made light and dark the one condition
  with machinery of its own; the per-property form removes the separation, and the
  overlay is the same privileging one level down. `Media::Dark` in a definition is the
  whole mechanism. Revisit when a condition must switch a palette whole rather than
  token by token.
- **`theme_<prop>` setters taking `Fn(&Palette) -> Color`** (2026-08-17). They welded
  the framework's box view to one library's palette type. A token is a value the
  ordinary setters accept. Revisit when a themed value cannot be named by a token.
- **`TextTheme`, `ControlTheme`, `SimpleTheme`** (2026-09-03). Three inheritance
  mechanisms for one need; `Typography`, `SimpleGlyphs` and `SimplePalette` are token
  sets under the one `Theme`. Revisit when an inherited value cannot be a token.
- **A `derived`-based `derive_scheme`** (2026-09-05). A rebuild disposes the subtree's
  scope with its state—measured: a signal inside went back to its initial value. The
  platform is told the scheme and one write re-inks the page. Revisit when a rebuild
  keeps the state of the subtree it rebuilds.
- **A scheme override scoped to a subtree** (2026-09-05). The choice half scopes for
  free; the guard half (the preference rule applying only where no ancestor chose) has
  no scriptless answer Firefox supports. An app has one scheme, the way the platform
  does. Revisit when every engine the web targets answers the guard half without script.
- **A handler-side override (`Override`/`ControlFlow`) letting a user take an event from
  a control** (2026-09-05). No first-party code wanted it and it taxed every ordinary
  handler, because the orphan rule blocks `From<()>` for a foreign type. A control
  decides its verdict. Revisit when first-party code needs to take an event from a
  control.
- **`Enhanced`: a native `Web*` control rendered without script and swapped for a drawn
  `Basic*` after hydration** (2026-08-16). The scriptless form is the control: a text
  entry is its native element and a toggle or slider keeps its input inside the label. A
  view that genuinely has two forms declares them through `Enhance`, the picker
  included. Revisit when a control has two forms `Enhance` cannot declare.
- **A derived value as an inline override** (2026-09-05). Inline outranks every variant
  on the web and not in the terminal. A derived leaf is a per-node custom-property slot
  the class references. Revisit when a derived value should outrank every variant on
  every platform.
- **The `snapshot`, `test` and `simple-views` cargo features** (2026-08-12). Each gated
  a trait bound, a type or a platform, which an additive feature cannot do soundly.
  Snapshot is on by default and opt-out, the test seams always compile, and the Simple
  views are a crate. Revisit when a cargo feature can gate a bound soundly.
- **Transitional `next` namespaces** (2026-09-05): `skene_core::next`, `skene_web::next`
  and `skene_simple::next` carried the new model beside the old during the redesign and
  are flattened into their crate roots. Not revisited: they were scaffolding for one
  redesign.
