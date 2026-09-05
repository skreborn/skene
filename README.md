# Skene

A client-side Rust UI framework. Write a GUI app once against one dependency, and it
runs on whatever driver the build selects—the web today (client-rendered,
server-rendered, or statically generated), native as it lands—while the terminal is a
facade of its own, equal in standing, with its own vocabulary typed to the medium. The
building block is a **view**; the reactive engine is fine-grained and vdom-free.

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

  Column::new()
    .set_spacing(20.)
    .set_children((
      Text::derive(move || format!("Count: {}", count.get())).set_font_size(32.),
      Row::new().set_spacing(12.).set_children((
        SimpleButton::new("−").handle_click(move || count.update(|n| *n = n.saturating_sub(1))),
        SimpleButton::new("+").handle_click(move || count.update(|n| *n = n.saturating_add(1))),
      )),
    ))
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
      ├── skene-web      the web driver: the typed vocabulary mapped to a live DOM
      │         │        (browser) or an HTML string (server), the content-addressed
      │         │        stylesheet, hydration, and the WASI reactor
      │         │
      │    skene-gui     the GUI facade and the single dependency a GUI app pulls in:
      │                  the built-in views realized once against the active driver,
      │                  the composition helpers, and the transparent entry
      │
      └── skene-tui      the terminal facade, equal in standing: its own views typed
                         to the medium, its driver carried in-crate

skene-simple       the Simple design language both facades import: the neutral
                   palette and the emphasis variants
```

| Crate | Role |
|-------|------|
| `skene-reactive` | `Signal`, `Memo`, `Effect`, `Owner`, `Trigger`. Synchronous on write. |
| `skene-core` | The spine: `NodeId`, the view spine (`View`, `AnyView`, `IntoView`), the typed props (`BoxProps`, `RunProps`), `IntoChildren`, `Reorder`, `Source`, `Boundary`, generic geometry, and the value vocabulary (`Color`, `Axis`, `Layout`, `Role`, `Border`, alignments, `MouseCursor`). Knows nothing of platforms. |
| `skene-web` | The web driver: two target-selected leaves (browser DOM, server string tree) implementing the typed vocabulary, the interned stylesheet, hydration, streaming, and the raw-DOM escape hatch. |
| `skene-gui` | The GUI facade: the built-in views (`Container`, `Text`, `RichText`, `TextTheme`), the raw controls (`RawButton`, `RawTextField`, …) and the `Simple*` components over them, realized once against `driver::*`; `spawn`/`Resource`/`Deferred`; `install_router` + `Link`; `#[lazy]` + `Lazy` for split-out modules; the transparent `launch` + `#[main]`, hydrating a server-rendered page in place. Re-exports core + reactive + `tracing`. |
| `skene-router` | Routing as reactive state: a typed `Route` in a signal, bound to the browser History API or the server's requested path. |
| `skene-runner` | Host-only library embedding a wasm runtime around the app's server build, so any server renders routes with a function call. Powers `skene ssg`. |
| `skene-tui` | The terminal facade, equal in standing to `skene-gui`: its own views typed to the medium (`u32` cells, `bool` bold, one-cell borders), taffy layout at `1.0 = 1 cell`, a cell-buffer renderer, keyboard focus. See `examples/tui/counter` (`cargo run -p tui-counter`). |
| `skene-simple` | The Simple design language shared by both facades' `Simple*` components: the neutral palette (`SimplePalette`) and the emphasis variants (`SimpleVariant`), over core's `Color`. |
| `skene-macros` | The `#[main]` and `#[lazy]` proc-macros. |
| `skene-cli` | The `skene` command: `run web`, `build web --static`, `run tui`, serving, live reload. |

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

Every layout is its own view—`Row`, `Column`, `Grid`, `Stack`, with the plain
`Container` decorating exactly one child—so only the properties a view honors exist to
call: a `Row` has an
`arrangement`, a `Grid` has `columns`, and nothing else compiles. Per-child properties
travel the same way, in wrapper views only the matching container accepts—`Flexible`
(take or give up space in a `Row`/`Column`), `Pinned` (against a `Stack`'s edges),
`GridCell` (placement in a `Grid`)—which create no node of their own. All of them share
one decoration surface (color, border, padding, events), defined once and generated onto
each.

### Controls: raw and Simple

Form controls come in two layers. The `Raw*` controls (`RawButton`, `RawCheckbox`,
`RawTextField`, …) are the unstyled, infinitely extensible bases every component
library builds on: full platform behavior, zero opinion about looks. The `Simple*`
components (`SimpleButton`, `SimpleCard`, `SimpleTextField`, …) are the ready-made
library on top—one neutral palette (`SimplePalette`, from the shared `skene-simple`
crate) and shape language, shared by the GUI and the terminal wherever the medium
allows, behind a default-on `simple-views` feature—so an app is presentable with no
styling at all:

```rust
SimpleButton::new("Save").handle_click(save),
SimpleButton::new("Delete").set_variant(SimpleVariant::Destructive).handle_click(delete),
```

### Defining your own view

The common case composes built-ins and implements `View` by delegating:

```rust
impl View for Labeled {
  fn build(self) -> NodeId {
    Column::new()
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

cargo skene --dir examples/gui/counter run web        # build, serve on :8080, live-reload
cargo skene --dir examples/gui/router build web --static   # statically generate the routed site
cargo skene --dir examples/tui/counter run tui    # build and run the terminal demo
```

Targeted examples live under `examples/gui` and `examples/tui`, one folder per
feature: `counter` (state, snapshot-preserving reloads), `router` (typed routes behind
real links, the about page split out and lazily loaded via `#[lazy]`), and `gallery`
(every built-in view and control, live)—the terminal has its own `counter` and
`gallery`.

The same app also renders server-side with no integration at all—its `wasm32-wasip2`
build is a plain command:

```sh
cargo build -p counter --target wasm32-wasip2
wasmtime target/wasm32-wasip2/debug/counter-server.wasm /   # prints the page
```

Then open <http://localhost:8080>. `skene run web` builds the `counter` example to
wasm, generates the bindings into `examples/gui/counter/dist/web`, serves it with the
assets its `skene.toml` declares, and rebuilds + reloads the page on change; pass
`--port <n>` or `--release`. Every build lands under `dist/<target>`—`web` (CSR and
static alike, the folder replaced wholesale each build), `tui`, `ios` and the rest as
they arrive. (`cargo skene` runs the
workspace-local CLI; `cargo install --path packages/cli` makes it a plain `skene`.)
The example is a single crate (`examples/gui/counter`) depending only on `skene-gui`.

## Testing

```sh
cargo test --workspace      # host: reactive, the spine, headless views (headless driver)
cargo test -p skene-gui --target wasm32-unknown-unknown   # browser: real DOM, headless browser
cargo test -p skene-gui --target wasm32-wasip2            # server: wasmtime, incl. snapshot
```

Drivers test against their real sinks: the browser suite drives a real DOM via
`wasm-bindgen-test`; the server suite runs under wasmtime and pins a byte-exact render
snapshot.
