# Agent & Contributor Guide

## Hard constraints

- Skene is a **client-side** framework. No server-side features, ever: no server
  functions, no backend framework integrations. SSR/SSG are rendering capabilities; the
  runner (`skene-runner`) is an embeddable library a user's server calls, not a server.
- The spine (`skene-reactive` + `skene-core`) stays platform-agnostic: no platform
  assumptions or nomenclature. Core hosts the reactive engine, the view spine, the typed
  property vocabulary (`BoxProps`, `RunProps`, later `StyleSet`), `Boundary`, generic
  geometry (over a plain `f32` scalar), and universal values.
- The canonical building block is a **view** (the `View` trait); never "widget". Views
  are realized **once**, in the umbrella, against the typed driver vocabulary—there is no
  per-platform `impl View`, no "backend" concept, and no trait describing a platform.
- A **driver** is a separate crate (`skene-web`, `skene-native`, `skene-tui`) exporting
  the same typed vocabulary (`create_box`, `update_box`, `create_run`, tree mutations,
  the async/hydration seams) mapped to its own sink. Drivers depend on core, never on
  each other. The umbrella is the only crate that names a driver (`driver::` resolved at
  compile time); there is no `dyn` driver, no downcast, and no runtime swap.
- Driver selection: by target while web is the only driver; **exactly one umbrella
  feature** once same-triple drivers exist (desktop/tui), with more than one a
  `compile_error!`. Target-gating is the enforcement layer: the web csr/ssr leaves stay
  split on `target_os` (`unknown`/`wasi`), so the server leaf physically cannot touch the
  DOM.
- The **style model is data** (`StyleSet`: typed props + open per-family passthrough,
  media/state variants, transitions, keyframes). Layout vocabulary is taffy's, adopted
  not invented. On the web a `StyleSet` compiles to real CSS inside one interned class, so
  SSR/SSG output is responsive, hoverable and animated with **no JS/WASM**. `derive_`
  closures remain for genuinely dynamic values only.
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
- View logic tests run headless where possible (via a recording driver, once it exists).
  Drivers test against their real sinks (web: real DOM via `wasm-bindgen-test`; ssr:
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
  `handle_<event>` for an event handler (e.g. `Container::handle_click`). A driver
  function that realizes the handler is named the same.
- A test double's method that simulates an external event is named `trigger_<event>`.
- Make a function or method `const` whenever its body allows it (no trait-method calls such as
  `.into()`, no allocation, no interior mutability). Apply it broadly, and especially to small,
  frequently-constructed types like the geometry and value vocabulary. The API is early and
  unstable: prefer a signature that can be `const` over one that can't when the ergonomic cost is
  small.

## Tooling & workflow rules

- Always format with `cargo +nightly fmt`.
- Prefer `#[expect(lint)]` over `#[allow(lint)]`.
- Order dependencies alphabetically by crate name within every Cargo manifest table.
- Macros must pull their weight: use one only when it eliminates real boilerplate or repetition
  (the entry proc-macro is the exemplar). Never reach for a macro that adds little while degrading
  `rustfmt`/rust-analyzer support, readability, or performance; prefer a function or generic when
  one suffices.
- Retrofit as you raise the bar: when a change establishes a better pattern (a helper, a naming
  convention, a clearer structure), bring existing code up to it rather than leaving two standards.
- Keep parallel code visually parallel: when two files do essentially the same thing in different
  places, shape them so a human can compare them at a glance—same names where they make sense,
  same order of items, same structure. (This is legibility, not coupling; sharing helpers to cut
  duplication is still encouraged.)
