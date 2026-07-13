# Agent & Contributor Guide

## Hard constraints

- The spine (`skene-reactive` + `skene-core`) stays platform-agnostic: no platform assumptions or
  nomenclature. It hosts the reactive engine, view spine, generic geometry (over a plain `f32`
  scalar), and universal values (`Color`, `MouseCursor`, `Axis`, alignment) only.
- **There are no "backends", and no trait describing one.** A platform is a plain module of
  free-standing types and functions under `skene-gui`'s `platform/` (`web`, later `android`,
  `unix`, …), holding its own state privately — the shape `std` uses (`std::sys` behind
  `std::fs`, extras under `std::os::unix`). Exactly one is compiled, chosen by target-gating.
  There is no `dyn` dispatch, no downcast, and no runtime swap: every call site already knows,
  at compile time, which platform it is talking to.
- Shared code reaches the platform by *name*, not through an abstraction: it calls
  `platform::tree::attach`, and the target decides what that is. A target with no platform has
  no realization, so building a view there is a compile error, not a runtime panic.
- There is no framework-defined `Style`; a platform styles its own way. The framework ships no
  predefined set of styles, for any view.
- Built-in views live in the facade (`skene-gui`) as a shared builder API with per-platform,
  target-gated realization under `platform/<name>/views/`. The API is uniform, so app code is
  portable; only realization differs. There is no shared "vocabulary" crate. TUI is a parallel
  facade sharing only the spine and universal values.
- The canonical building block is a **view** (the `View` trait, `build(self) -> NodeId`); never
  "widget". A library view is authored the same way as a built-in: a shared builder plus a
  cfg-gated `build` calling its platform's functions. That is the whole escape hatch — there is
  no `realize` helper. No framework `cfg` helper, no prescribed module shape.
- The spine must not reference a platform, and now cannot: it has no notion of one. The facade
  (`skene-gui`) owns the platforms, target-gates their dependencies, and is the single
  dependency an app pulls in.
- The web platform must stay compatible with eventual server-side rendering (content-addressed
  class interning in the `StyleStore` preserved).
- Each platform tests itself against its own concrete output (the web platform against a real
  DOM, via `wasm-bindgen-test`). There is no shared or generic "mock" platform for exercising
  the built-in views: with per-platform realization and no shared trait, a generic platform has
  no realization to assert against — it would only test a fiction. What the spine keeps is pure
  and unit-tested in isolation (`Reorder`), needing no platform at all.

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

- The spine stays minimal and structural. The built-in views live in the facade (`skene-gui`),
  realized per backend; users extend by composing built-ins or authoring their own per-backend
  views.
- Builder methods: `set_<prop>` for a fixed value, `derive_<prop>` for a reactive one,
  `handle_<event>` for an event handler (e.g. `Container::handle_click`). A backend method that
  realizes the handler is named the same (e.g. `WebBackend::handle_click`).
- A test double's method that simulates an external event is named `trigger_<event>` (e.g. a mock
  backend drives a node's click handlers with `trigger_click`).
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
  places, shape them so a human can compare them at a glance — same names where they make sense,
  same order of items, same structure. (This is legibility, not coupling; sharing helpers to cut
  duplication is still encouraged.)
