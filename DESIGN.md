# Design

The architecture of skene. Facts and the reasons behind the load-bearing decisions.
Conventions and invariants live in `AGENTS.md`.

The building block of the tree is a **view** (the `View` trait, `build(self) -> NodeId`).
"View" is the canonical term throughout — there is no separate "widget" concept.

## Crates

```
skene-reactive     reactive engine (signals, effects, memos, scopes)
skene-core         spine: view spine, Source, Reorder, IntoChildren, root scope,
                   generic geometry, values. Knows nothing of platforms.
                   Depends on reactive.
skene-gui          facade + the single dependency a GUI app pulls in: the built-in views
                   and the values they take, the shared composition helpers, the platforms
                   themselves (target-gated), and the transparent entry.
skene-gui-macros   the #[skene_gui::main] entry proc-macro.
```

TUI is a future parallel facade (`skene-tui`) reusing the spine and universal values only.

## No backends, no shared `Style`

There is no "backend" concept and no trait describing one. A **platform** is a plain module
of free-standing types and functions under `skene-gui`'s `platform/` — `web` has
`create_element`/`element`/`document`, `set_style`/`override_style` (declarations in, class
or inline out), `set_text`, `handle_click`, `install`/`mount`/`launch` — holding its state
privately in a thread-local. Exactly one is compiled, chosen by target-gating.

  **Why a module, not an object.** This is `std`'s shape: `std::sys` behind `std::fs`, with
  the platform-specific extras under `std::os::unix`. A trait would only buy runtime
  polymorphism, and there is deliberately none — a build targets exactly one platform, so
  every call site already knows at compile time which one it is talking to. An earlier
  `Backend` trait existed solely to make a thread-local storable as `dyn`, and cost an
  `unsafe` pointer deref, a `Box::leak`, and a `TypeId` downcast that could panic — all to
  hand back a type the caller had already named. Putting the state in the module deletes the
  lot.

  **Why no shared `Style`.** We ship no predefined set of styles — not for containers, not
  for text, not for anything. A shared `Style` would force every platform onto one fixed
  vocabulary and one interpretation; instead each styles the way its platform does (web →
  CSS classes; a native one → `taffy` + native paint). The built-in views' *API* is still
  uniform, so app code is portable; only realization differs.

## Built-in views: shared API, per-platform realization

A built-in view (`Container`, `Text`, `TextTheme`) is a **shared** struct + builder API in
`skene-gui` (`container.rs`, `text.rs`), with **no** `View` impl of its own. Its realization
lives in a *family* module and is written against a per-leaf vocabulary, target-gated:

(files follow the `foo.rs` + `foo/` convention — no `mod.rs` anywhere)

```
platform.rs            declares the families; re-exports the structural seam (tree, spawn)
                       up from the active family, used by reconcile / Children / spawn
platform/web.rs        the web FAMILY (target_arch = "wasm32"): the realization shared by
platform/web/          every web leaf — impl View for Container / Text / TextTheme (views/),
  views.rs  views/     the Styler, and the CSS Declarations vocabulary — written against the
  styler.rs            leaf vocabulary and re-exposed as the escape hatch (skene_gui::web).
  declarations.rs      Declares and re-exports its leaves:
  csr.rs  csr/           the browser LEAF (target_os = "unknown"): private State + the
                         DOM-backed vocabulary (create_element, set_style, …) + the raw
                         `element` handle. An `ssr` leaf (target_os = "wasi") slots in beside
                         it for SSR, sharing everything above it under platform/web/.
```

The shared views call the web vocabulary (`create_element`, `set_style`, `set_attribute`, …)
re-exported at `platform::web`, and build styling in CSS `Declarations` — never naming a leaf
or `web-sys`. A library author writes a view the same way — a shared builder plus a cfg-gated
`build` calling the platform's functions — with no special access the built-ins don't have.
**That is the whole escape hatch**; there is no `realize` helper and no framework `cfg` alias.

  **No platform, no realization.** A target with no platform has no `View` impl for the
  built-in views, so building one there is a **compile error**, not a runtime panic — an app
  is built for the platform(s) it targets (the example gates itself to `wasm32`). The
  `skene-gui` *library* still compiles anywhere; only realization is gated.

  **Testability cost.** Because `build` is per-platform, built-in views have no
  host-runnable path; their tests are per-platform (web → browser `wasm-bindgen-test`).

## Platform selection

- One built executable targets exactly **one** platform, selected by target-gating (the
  facade lists each platform's deps under `[target.'cfg(…)'.dependencies]`; the module and
  each realization are `#[cfg]`-gated). No runtime swap. A foreign-platform call isn't a
  runtime possibility — it isn't linked.
- Shared code reaches the platform **by name**: `reconcile`, `dynamic` and `Children` call
  `platform::tree::{attach, detach, destroy, create_group}`, and the target decides what
  that resolves to. Static, monomorphic, inlinable.

  **Why cfg selection, not `dyn`.** Since a build has exactly one platform, an abstraction
  buys nothing: it can only re-discover at runtime what the compiler already knew. Naming
  the module *is* the dispatch.

  **Why `Children` and `reconcile` sit in the facade, not the spine.** `Children` must be
  one portable type (it is a field of the shared `Container` builder), and `Children::derive`
  erases its key `K`, so whoever reconciles must be where `K` is still concrete. Putting them
  in the facade lets that code name `platform::tree` — which the spine, knowing nothing of
  platforms, could not. The genuinely pure parts stay in the spine (`Reorder`,
  `IntoChildren`), so a second facade (TUI) reimplements as little as possible.

## Spine (`skene-core`)

- **View spine:** `View` (`build(self) -> NodeId`), `AnyView`, `IntoView`.
- **`IntoChildren`:** converts tuples/arrays/`Vec`s of views into `Vec<AnyView>`. Pure, so
  every facade gets it free.
- **`Reorder`:** the pure minimal-reorder decision (longest increasing run of previous
  positions) — integers in, integers out, no platform and no reactive involvement, and
  unit-tested in isolation. The one genuinely tricky piece of reconciliation, shared by
  every facade.
- **`Source<T>`:** a property value, `Static` or `Derived`; `bind` re-runs an effect on
  change when derived. `Clone` (the derived closure is `Rc`-shared), so one inherited
  source can be handed to many nodes — this is how a `TextStyle` carries static and
  reactive properties uniformly and threads through the theme `Context`.
- **Runtime:** `root_scope` and `reset_root_scope`, and nothing else — the spine has no
  notion of a platform to install or dispatch to.
- **Geometry:** `Inset<T>`, `Size<T>`, `Point<T>`, `Rect<T>`, generic over a scalar (a
  `T: ConstZero` bound supplies `ZERO`). `skene-gui` aliases them to `f32`.
- **Universal values:** `Color`, `MouseCursor`, `Axis`, `MainAxisAlignment`,
  `CrossAxisAlignment`.

  **A plain `f32` scalar.** Content-addressed styling keys on each declaration's rendered
  CSS text, not on typed-value equality, so the geometry/`Color` scalar needs no
  `Eq`/`Hash`/`Ord` and is a plain `f32`. `Color` stores its OKLCH components as `f32`, at
  full precision.

## Web platform (`skene-gui`'s `platform::web`)

- Private `State` (element registry, `StyleStore`, listener closures) lives in a
  thread-local; `with_state` only ever takes a shared borrow, so a click handler may
  re-enter and build or destroy nodes. The public surface is free functions.
- `set_style` interns `Declarations` into a content-addressed CSS class (kept SSR-friendly)
  via the `StyleStore`; `override_style` applies inline overrides (the reactive-delta path).
- **Logging:** installs a `ConsoleSubscriber` at `launch` routing `tracing` events to
  `console.error`/`warn`/`log` by level.

## Facade & entry (`skene-gui`)

- Holds the built-in views + value types (`Border`, `Role`, `Layout`, `TextStyle`), the
  geometry aliases, the composition helpers (`Children`, `HasChildren`, `reconcile`,
  `dynamic`, `empty`), the platforms themselves, and re-exports the spine + reactive +
  `tracing`. A `prelude` bundles the lot plus the `tracing` level macros, `launch`, `main`.
- **The escape hatch is the platform module.** A low-level view calls `web::create_element`
  and friends inside an `AnyView`, cfg-gated to that target — the same access the built-ins
  have, with no helper in between. **`empty() -> impl View`** renders nothing — the no-op
  branch for an unsupported platform.

  **No error boundaries.** A UI is all-or-nothing; a view that fails to draw panics with a
  clear message rather than degrading. Platform coverage is the author's responsibility.
- **`launch`** is a thin wrapper over the target's platform `launch` (wasm →
  `platform::web::launch`), adding the one readiness log shared by every platform; it exists
  only on a target with a platform, so naming it elsewhere is a compile error.
  **`#[skene_gui::main]`** expands to the target's entry glue — on wasm a
  `#[wasm_bindgen(start)]` calling `launch`.

  **True single dependency.** The macro relocates wasm-bindgen's crate path with
  `wasm_bindgen(start, wasm_bindgen = ::skene_gui::wasm_bindgen)`, and the facade re-exports
  wasm-bindgen, so app code names no platform and no wasm-bindgen — only `skene-gui`.

## Logging

Framework and app code log through `tracing`. Each platform installs its subscriber at
`launch` (web → console; future: android → logcat, ios → `os_log`, desktop → stderr). The
spine carries no subscriber.
