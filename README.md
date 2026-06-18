# Skene

Skene builds multi-platform apps as a **shared core plus thin platform shells**, all in Rust.

An application is a Cargo workspace: one platform-agnostic business-logic *core* that holds all state
and logic, and one slim *shell* per platform that renders it. A shell reads the core's reactive state
directly—no serialization, no view-model, no diffing—so the boundary between logic and UI is an
ordinary Rust function call.

This is the [crux](https://github.com/redbadger/crux) architecture with one change: crux keeps the
core in Rust but writes each shell in the platform's native framework (Swift/Kotlin/TypeScript) across
a serialized FFI edge. Skene writes the shells in Rust too, so a shell reads the core's signals
directly and there is no boundary to cross.

## Reactivity

Fine-grained and vdom-free. A signal write re-runs exactly the effects that depend on it, and each
effect patches the node it owns—no virtual DOM, no tree diffing. Effects run synchronously on write.

```rust
let count = RwSignal::new(0);
count.update(|n| *n += 1);   // re-runs only the effects that read `count`
```

## Architecture

Four library crates under `packages/`:

```
skene-reactive          the reactive engine — standalone, no skene deps
      │
      ▼
   skene-core           + reactive: NodeId, the structural Backend trait, the value
      │                 vocabulary, the reconciliation helpers, and the (non-generic,
      ├───────────┐     backend-agnostic) View scaffolding — no widgets
      ▼           ▼
    skene      skene-web        skene   = facade re-exporting reactive + core
  (facade)    (a backend)       web     = the browser/DOM backend, compiled to WASM
```

| Crate | Role |
|-------|------|
| `skene-reactive` | Reactive engine: `RwSignal`, `Memo`, `Effect`, `Scope`, `Trigger`. Effects run synchronously on write. |
| `skene-core` | The shared foundation: `NodeId`, the structural `Backend` trait, the value vocabulary (`Color`, `Length`, `Factor`, `Inset`, `Axis`, alignments, `MouseCursor`), the reconciliation helpers (`reconcile`, `dynamic`, `Source`), and the non-generic view scaffolding (`View`, `AnyView`, `IntoView`, `Children`, `IntoChildren`, `HasChildren`, `DynamicView`). No widgets. |
| `skene` | Facade re-exporting `reactive` + `core`; the shared foundation a core and any shell build on. |
| `skene-web` | The web backend (`WebBackend`, compiled to WASM): its own view types (`Container`, `Text`, `Span`, `RichText`, …) that implement core's `View`, its style model and CSS translation, mouse input, and `launch`. |

There is no shared widget set and no per-backend type parameter. The view scaffolding is shared and
non-generic—an app targets one backend, so `View` needn't distinguish them—while each backend owns its
concrete view types. A native or TUI backend slots in as its own crate (its own views, styling, and
input), reusing only the reactive engine, the value vocabulary, and the scaffolding.

### Styling on the web backend

Only the web backend knows what CSS is: styling reaches a backend as backend-agnostic values, and
`skene-web` turns them into CSS. Each rendered declaration block is hashed to a deterministic class and
interned once in a single `<style>` element, reference-counted by live node so identical styles collapse
to one class and unused ones are collected. A node's initial style is a shared class; later reactive
changes apply as inline overrides, so a running animation never rewrites the shared rule. The
determinism keeps the scheme compatible with server-side rendering.

## An app is a workspace

```
my-app/
  core/          # my-app-core:   state + logic, depends on `skene` (reactive + core) only
  shell-web/     # my-app-web:    depends on core + skene-web
  shell-native/  # my-app-native: depends on core + skene-native  (future)
```

The core exposes its state as signals and depends on no backend, so it cannot reference a platform
type:

```rust
pub struct Counter {
  pub count: RwSignal<i32>,
}

impl Counter {
  pub fn new() -> Self {
    Self { count: RwSignal::new(0) }
  }

  pub fn increment(&self) {
    self.count.update(|n| *n += 1);
  }
}
```

A shell reads those signals directly and builds a native view that reacts:

```rust
fn ui(counter: Counter) -> impl IntoView {
  Container::new(Text::derive(move || format!("count: {}", counter.count.get())))
}
```

`counter.count` is one signal: the shell's `Text` subscribes to it and the core's `increment` writes
it, across a plain call. A second shell reuses `Counter` unchanged and builds a wholly different view
tree from the same signals.

## Widgets

Every widget *is* its own builder *is* a `View`: chain typed setters and pass it straight to a
parent—no separate builder type, no `.build()` to call. One setter convention: `set_<prop>` for a
fixed value, `derive_<prop>` for a reactive one, `on_<event>` for a handler.

```rust
use skene::prelude::*;
use skene_web::prelude::*;

fn app(counter: Counter) -> impl IntoView {
  Container::column()
    .set_cross_axis_alignment(CrossAxisAlignment::Center)
    .set_spacing(20.0)
    .set_padding(40.0)
    .set_children((
      Text::derive(move || format!("Count: {}", counter.count.get()))
        .set_font_size(32.0)
        .set_font_weight(700)
        .set_color(Color::rgb(0x20, 0x20, 0x20)),
      Container::row().set_spacing(12.0).set_children((
        counter_button("−", move || counter.decrement()),
        counter_button("+", move || counter.increment()),
      )),
    ))
}

/// A blue pill button: a container with the button role wrapping its label.
fn counter_button(symbol: &'static str, action: impl Fn() + 'static) -> impl IntoView {
  Container::new()
    .set_children([Text::new(symbol).set_color(Color::WHITE).set_font_size(20.0)])
    .set_role(Role::Button)
    .set_padding(Inset::symmetric(20.0, 10.0))
    .set_color(Color::rgb(0x37, 0x6e, 0xff))
    .set_border_radius(8.0)
    .set_cursor(MouseCursor::Click)
    .on_press(action)
}
```

There is no `Button` widget: a button is a `Container` with `Role::Button` and an `on_press` handler,
so its label is any view and its styling is yours. There is likewise no separate `List`/`Grid`/`Stack`:
one `Container` both decorates itself and lays out its children, in the mode its constructor picks—
`new()` (block flow), `row()`/`column()` (flex), `grid(n)`, or `stack()` (children overlapping in one
cell).

### Defining your own widget

A widget is a struct that implements core's `View` (`fn build(self) -> NodeId`). The common case
composes existing widgets and delegates:

```rust
#[must_use]
pub struct Labeled {
  label: String,
  child: AnyView,
}

impl View for Labeled {
  fn build(self) -> NodeId {
    Container::column()
      .set_children((Text::new(self.label), self.child))
      .build()
  }
}
```

A custom container reuses the shared children machinery through `HasChildren`—never reimplementing
child mounting or keyed reconciliation:

```rust
#[must_use]
pub struct Panel {
  children: Children,
}

impl HasChildren for Panel {
  fn children_slot(&mut self) -> &mut Children {
    &mut self.children
  }
}
```

That yields the chainable `set_children` / `derive_children` setters (settable anywhere in the chain).
`set_children` takes a fixed set—an array or `Vec` of one type, or a tuple of mixed types
(`impl IntoChildren`); `derive_children` takes a reactive, keyed set that reconciles minimally on
change (the same node stays put; only displaced or new children move).

### Escape hatches

The framework models the common case and gets out of the way for the rest. A view it can't express is
yours to build: implement `View` for your type and drive the backend's concrete API directly through
`with_backend_as`.

```rust
use skene_web::{NodeId, View, WebBackend, with_backend_as};

impl View for MyExoticView {
  fn build(self) -> NodeId {
    with_backend_as::<WebBackend, _>(|backend| {
      let node = backend.create_container(None);
      // …raw web-backend calls the framework doesn't model…
      node
    })
  }
}
```

## Build & run

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli   # version must match the wasm-bindgen crate (see Cargo.lock)

cargo xtask web                  # build the wasm demo and serve it on :8080
```

Then open <http://localhost:8080>. `cargo xtask web` runs the release wasm build, generates the
bindings into `examples/counter-web/dist`, and serves the demo; pass `--port <n>` to change the
port, `--debug` for a debug build, or `--no-serve` to build only. The underlying steps are:

```sh
cargo build --release --target wasm32-unknown-unknown -p counter-web
wasm-bindgen target/wasm32-unknown-unknown/release/counter_web.wasm \
    --out-dir examples/counter-web/dist --target web
```

The counter example is two crates—`examples/counter-core` (the platform-agnostic logic) and
`examples/counter-web` (the wasm shell)—demonstrating the core/shell split. A mismatch between the
`wasm-bindgen` CLI and the crate version fails the bindings step; `cargo xtask web` warns when it
detects one.

## Testing

```sh
cargo test --workspace      # host tests: core reconciliation + web CSS translation
```

`skene-core`'s reconciliation tests run against a structural `MockBackend` on the host.
`skene-web`'s widget tests (`packages/web/tests/widgets.rs`) drive the real `WebBackend` against a live
DOM, so they run under `wasm32` via `wasm-bindgen-test` in a browser rather than on the host.
