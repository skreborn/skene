# Skene

A Rust GUI framework. Write an app once against one dependency, and it runs on whatever
platform the build targets — the web today, native platforms as they land. The building block
is a **view**; the reactive engine is fine-grained and vdom-free.

## Reactivity

A signal write re-runs exactly the effects that depend on it, and each effect patches the
node it owns — no virtual DOM, no tree diffing. Effects run synchronously on write.

```rust
let count = RwSignal::new(0);
count.update(|n| *n += 1);   // re-runs only the effects that read `count`
```

## Hello, counter

An app depends only on `skene-gui`, marks its entry with `#[main]`, and names no platform:

```rust
use skene_gui::prelude::*;

#[main]
fn app() -> impl IntoView {
  let count = RwSignal::new(0u64);

  Container::column()
    .set_spacing(20)
    .set_children((
      Text::derive(move || format!("Count: {}", count.get())).set_font_size(32),
      Container::row().set_spacing(12).set_children((
        button("−", move || count.update(|n| *n = n.saturating_sub(1))),
        button("+", move || count.update(|n| *n = n.saturating_add(1))),
      )),
    ))
}

fn button(symbol: &'static str, action: impl Fn() + 'static) -> impl IntoView {
  Container::new()
    .set_role(Role::Button)
    .set_padding(Inset::symmetric(16, 12))
    .set_cursor(MouseCursor::Click)
    .set_child(Text::new(symbol))
    .handle_click(action)
}
```

`#[main]` expands to the target's entry glue (on the web, a `wasm-bindgen` start that calls
`launch`). Nothing in this code names the platform.

## Architecture

```
skene-reactive     the reactive engine — standalone
      │
skene-core         + reactive: the view spine, Source, Reorder, IntoChildren, generic
      │            geometry, and the value vocabulary. No views, no platforms.
      │
skene-gui          the facade: built-in views, composition helpers, the platforms
                   themselves (target-gated), and the single dependency an app pulls in
```

| Crate | Role |
|-------|------|
| `skene-reactive` | `RwSignal`, `Memo`, `Effect`, `Scope`, `Trigger`. Synchronous on write. |
| `skene-core` | The spine: `NodeId`, the view spine (`View`, `AnyView`, `IntoView`), `IntoChildren`, `Reorder` (the pure minimal-reorder decision), `Source`, generic geometry (`Inset<T>`, `Size<T>`, `Point<T>`, `Rect<T>`), and the value vocabulary (`Color`, `Axis`, alignments, `MouseCursor`). Knows nothing of platforms. |
| `skene-gui` | The facade: the built-in views (`Container`, `Text`, `TextTheme`) and the values they take; the composition helpers (`Children`, `reconcile`, `dynamic`); the platforms themselves under `platform/` (`web` today); the transparent `launch` + `#[main]`. Re-exports core + reactive + `tracing`. |

**No backends, no framework-defined styles.** There is no "backend" trait or object. A
platform is a plain module of free-standing functions under `skene-gui`'s `platform/`,
holding its state privately — the shape `std` uses. Exactly one is compiled, chosen by
target-gating, so there is no `dyn` dispatch and no runtime swap. The built-in views are a
*shared builder API* with a *per-platform*, target-gated `build` — app code is portable while
realization is native to each platform. The framework ships no predefined `Style`; each
platform styles its own way.

### Styling on the web platform

Styling reaches the web platform as CSS `Declarations`; each declaration block is hashed to a
deterministic class and interned once in a single `<style>` element, reference-counted by
live node so identical styles collapse to one class and unused ones are collected. A view's
initial style is a shared class; reactive changes apply as inline overrides, so a running
animation never rewrites the shared rule. The determinism keeps it server-side-render-ready.

## Views

A view *is* its own builder *is* a `View`: chain typed setters and pass it to a parent — no
separate builder type, no `.build()` to call. One setter convention: `set_<prop>` for a fixed
value, `derive_<prop>` for a reactive one, `handle_<event>` for a handler.

There is no `Button` view: a button is a `Container` with `Role::Button` and a click handler,
so its label is any view and its styling is yours. There is likewise no separate
`List`/`Grid`/`Stack`: one `Container` decorates itself and lays out its children in the mode
its constructor picks — `new()` (block), `row()`/`column()` (flex), `grid(n)`, or `stack()`
(children overlapping in one cell).

### Defining your own view

The common case composes built-ins and implements `View` by delegating:

```rust
impl View for Labeled {
  fn build(self) -> NodeId {
    Container::column()
      .set_children((Text::new(self.label), self.child))
      .build()
  }
}
```

For platform-specific work beyond the built-ins, just call the platform's functions from a
view, cfg-gated to that target — there is no helper in between (`empty()` is the no-op for
platforms a view doesn't support):

```rust
#[cfg(target_arch = "wasm32")]
fn canvas() -> impl IntoView {
  AnyView::new(|| {
    let node = web::create_element("canvas");
    // …raw DOM / JS interop the framework doesn't model…
    node
  })
}
```

## Build & run

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli   # version must match the wasm-bindgen crate (see Cargo.lock)

cargo xtask web                  # build the wasm demo and serve it on :8080
```

Then open <http://localhost:8080>. `cargo xtask web` builds the `counter` example to wasm,
generates the bindings into `examples/counter/dist`, and serves it; pass `--port <n>`,
`--debug`, or `--no-serve`. The example is a single crate (`examples/counter`) depending only
on `skene-gui`.

## Testing

```sh
cargo test --workspace      # host: reactive + the spine's pure helpers (Reorder)
cargo test -p skene-gui --target wasm32-unknown-unknown   # web: real DOM, headless browser
```

The built-in views realize per platform, so their tests are per-platform: the web suite
drives a real DOM via `wasm-bindgen-test`. There is no generic mock — a generic platform has
no realization to assert against.
