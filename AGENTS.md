# Agent & Contributor Guide

## Hard constraints

- The shared layer (reactive engine, value vocabulary, view spine) stays platform-agnostic: no
  platform assumptions, no platform nomenclature.
- A backend must be addable as its own crate—own views, styling, input—reusing only the shared layer.
- A core must not depend on any backend crate, so it can't reference a platform type.
- `skene-web` must stay compatible with eventual server-side rendering.

## Style and documentation conventions

- Use 2-space indentation.
- Never surround an em-dash with spaces.
- Attach a helper to the type it operates on as a method, not a free function.
- Put `#[must_use]` on the type itself, not on the functions that return it, whenever it
  semantically makes sense.
- Apply `#[inline(always)]` wherever appropriate: the framework has a great many tiny functions
  that are best inlined across crate boundaries.
- Document every public API; comment non-public code only when critical.
- Comments state contracts only, never history, rationale, "why", or implementation.
- A comment's first line is a short summary; add further lines only when needed to use the item.
- Split a comment into paragraphs when it is long or covers distinct points.
- Comment a struct's fields or an enum's variants all or none: never document some siblings and
  leave others bare. The only exception is when only the public fields/variants are documented.
- Omit blank lines only between lines of the same shape (consecutive `let`s, struct fields).
- Surround every `return`, conditional, loop, block, and multi-line statement with a blank line.

## Naming & API conventions

- The shared layer stays minimal and structural; everything with an appearance lives in a backend,
  built for users to extend or customize.
- Builder methods: `set_<prop>` for a fixed value, `derive_<prop>` for a reactive one, `on_<event>`
  for a handler.

## Tooling & workflow rules

- Always format with `cargo +nightly fmt`.
- Prefer `#[expect(lint)]` over `#[allow(lint)]`.
- Order dependencies alphabetically by crate name within every Cargo manifest table.
