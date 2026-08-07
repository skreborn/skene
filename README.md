# Skene

A client-side Rust GUI framework. Write an app once against one dependency, and it runs
on whatever driver the build selects—the web today (client-rendered, server-rendered, or
statically generated), native and terminal drivers as they land. The building block is a
**view**; the reactive engine is fine-grained and vdom-free.

## Reactivity

A signal write re-runs exactly the effects that depend on it, and each effect patches the
node it owns—no virtual DOM, no tree diffing. Effects run synchronously on write.

```rust
let count = Signal::new(0);
count.update(|n| *n += 1);   // re-runs only the effects that read `count`
```

## Hello, counter

A GUI app depends only on `skene-gui`, marks its entry with `#[main]`, and names no driver:

```rust
use skene_gui::prelude::*;

#[main]
fn app() -> impl IntoView {
  let count = Signal::new(0u64);

  Container::column()
    .set_gap(20.)
    .set_children((
      Text::derive(move || format!("Count: {}", count.get())).set_font_size(32.),
      Container::row().set_gap(12.).set_children((
        button("−", move || count.update(|n| *n = n.saturating_sub(1))),
        button("+", move || count.update(|n| *n = n.saturating_add(1))),
      )),
    ))
}

fn button(symbol: &'static str, action: impl Fn() + 'static) -> impl IntoView {
  Container::new()
    .set_role(Role::Button)
    .set_padding(Inset::symmetric(16., 12.))
    .set_cursor(MouseCursor::Click)
    .set_child(Text::new(symbol))
    .handle_click(action)
}
```

`#[main]` expands to the target's entry glue (on the web, a `wasm-bindgen` start that calls
`launch`). Nothing in this code names the platform.

## Architecture

```
skene-reactive     the reactive engine—standalone
      │
skene-core         + reactive: the view spine, the typed property vocabulary, Source,
      │            Boundary, Reorder, IntoChildren, generic geometry, and the value
      │            vocabulary. No views, no platforms.
      │
skene-web          the web driver: the typed vocabulary mapped to a live DOM (browser)
      │            or an HTML string (server), the content-addressed stylesheet,
      │            hydration, and the WASI reactor
      │
skene              the umbrella and the single dependency an app pulls in: the built-in
                   views realized once against the active driver, the composition
                   helpers, and the transparent entry
```

| Crate | Role |
|-------|------|
| `skene-reactive` | `Signal`, `Memo`, `Effect`, `Owner`, `Trigger`. Synchronous on write. |
| `skene-core` | The spine: `NodeId`, the view spine (`View`, `AnyView`, `IntoView`), the typed props (`BoxProps`, `RunProps`), `IntoChildren`, `Reorder`, `Source`, `Boundary`, generic geometry, and the value vocabulary (`Color`, `Axis`, `Layout`, `Role`, `Border`, alignments, `MouseCursor`). Knows nothing of platforms. |
| `skene-web` | The web driver: two target-selected leaves (browser DOM, server string tree) implementing the typed vocabulary, the interned stylesheet, hydration, streaming, and the raw-DOM escape hatch. |
| `skene-gui` | The GUI facade: the built-in views (`Container`, `Text`, `RichText`, `TextTheme`, `Input`) realized once against `driver::*`; `spawn`/`Resource`/`Deferred`; `install_router`; the transparent `launch` + `#[main]`. Re-exports core + reactive + `tracing`. |
| `skene-router` | Routing as reactive state: a typed `Route` in a signal, bound to the browser History API or the server's requested path. |
| `skene-runner` | Host-only library embedding a wasm runtime around the app's server build, so any server renders routes with a function call. Powers `skene ssg`. |
| `skene-tui` | The terminal facade, equal in standing to `skene-gui`: its own views typed to the medium (`u16` cells, `bool` bold, one-cell borders), taffy layout at `1.0 = 1 cell`, a cell-buffer renderer, keyboard focus. See `examples/tui-counter` (`cargo run -p tui-counter`). |

**Realize once, drive per platform.** Views are realized a single time against a typed,
driver-neutral vocabulary (`create_box(&BoxProps)`, `update_box`, `create_run`, tree
mutations); each driver crate exports that vocabulary mapped to its own sink. Exactly one
driver is compiled, resolved by the umbrella at compile time—no "backend" trait, no
`dyn` dispatch, no runtime swap.

### Styling on the web platform

Styling reaches the web platform as CSS `Declarations`; each declaration block is hashed to a
deterministic class and interned once in a single `<style>` element, reference-counted by
live node so identical styles collapse to one class and unused ones are collected. A view's
initial style is a shared class; reactive changes apply as inline overrides, so a running
animation never rewrites the shared rule. The determinism keeps it server-side-render-ready.

## Views

A view *is* its own builder *is* a `View`: chain typed setters and pass it to a parent—no
separate builder type, no `.build()` to call. One setter convention: `set_<prop>` for a fixed
value, `derive_<prop>` for a reactive one, `handle_<event>` for a handler.

There is no `Button` view: a button is a `Container` with `Role::Button` and a click handler,
so its label is any view and its styling is yours. There is likewise no separate
`List`/`Grid`/`Stack`: one `Container` decorates itself and lays out its children in the mode
its constructor picks—`new()` (block), `row()`/`column()` (flex), `grid(n)`, or `stack()`
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

For web-specific work beyond the built-ins, just call the driver's functions from a
view, cfg-gated to that target—there is no helper in between (`empty()` is the no-op for
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
rustup target add wasm32-unknown-unknown wasm32-wasip2
cargo install wasm-bindgen-cli   # version must match the wasm-bindgen crate (see Cargo.lock)

cargo skene --dir examples/counter run web        # build, serve on :8080, live-reload
cargo skene --dir examples/counter build web --static   # statically generate the site
cargo skene --dir examples/tui-counter run tui    # build and run the terminal demo
```

The same app also renders server-side with no integration at all—its `wasm32-wasip2`
build is a plain command:

```sh
cargo build -p counter --target wasm32-wasip2
wasmtime target/wasm32-wasip2/debug/counter-server.wasm /   # prints the page
```

Then open <http://localhost:8080>. `skene run web` builds the `counter` example to
wasm, generates the bindings into `examples/counter/dist/web`, serves it with the
assets its `skene.toml` declares, and rebuilds + reloads the page on change; pass
`--port <n>` or `--release`. Every build lands under `dist/<target>`—`web` (CSR and
static alike, the folder replaced wholesale each build), `tui`, `ios` and the rest as
they arrive. (`cargo skene` runs the
workspace-local CLI; `cargo install --path packages/cli` makes it a plain `skene`.)
The example is a single crate (`examples/counter`) depending only on `skene-gui`.

## Testing

```sh
cargo test --workspace      # host: reactive + the spine's pure helpers
cargo test -p skene-gui --target wasm32-unknown-unknown   # browser: real DOM, headless browser
cargo test -p skene-gui --target wasm32-wasip2            # server: wasmtime, incl. snapshot
```

Drivers test against their real sinks: the browser suite drives a real DOM via
`wasm-bindgen-test`; the server suite runs under wasmtime and pins a byte-exact render
snapshot.
