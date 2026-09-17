# Skene

A client-side Rust UI framework: one spine, two facades. A GUI app depends on
`skene-gui` and runs wherever the build targets it—the browser, client-rendered; a
server, rendering or statically generating pages from the same wasm module; native as
it lands. A terminal app depends on `skene-tui`, a facade of equal standing with its own
vocabulary typed to the medium. Every built-in view is written once, generic over the
platform a facade selects; the reactive engine is signal-based, fine-grained and vdom-free;
styling is data, compiled to CSS on the web and resolved natively elsewhere, so a
server-rendered page is responsive, themed, hoverable and animated with no script at all.

## Reactivity

```rust
let count = Signal::new(0);
count.update(|n| *n += 1);   // re-runs only the effects that read `count`
```

A signal write re-runs exactly the effects that depend on it, and each effect patches
the node it owns—no virtual DOM, no tree diffing. Signals snapshot by default, so a
dev-loop restart resumes where the last run stood; a keyed list item or a view given
`set_key` keeps its state by key rather than by position.

## A GUI app

A GUI app depends on `skene-gui` and, for the ready-made components, on
`skene-simple-gui`; it marks its entry with `#[main]` and names no platform
(`examples/gui/counter`):

```rust
use skene_gui::prelude::*;
use skene_simple_gui::prelude::*;

#[main]
fn app() -> impl IntoView {
  let count = Signal::tagged("count", 0u64);

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    Container::new()
      .set_color(SimplePalette::BACKGROUND)
      .set_child(
        Column::new()
          .set_arrangement(Arrangement::Center)
          .set_item_alignment(ItemAlignment::Center)
          .set_child(
            SimpleCard::new().set_spacing(24.).set_children((
              Text::derive(move || count.get().to_string()).set_font_size(56.),
              Row::new().set_spacing(12.).set_children((
                SimpleButton::new("−").handle_click(move || count.update(|n| *n = n.saturating_sub(1))),
                SimpleButton::new("+").handle_click(move || count.update(|n| *n = n.saturating_add(1))),
              )),
            )),
          ),
      )
  })
}
```

`#[main]` expands to whichever entry the target has: in the browser it launches the
app, hydrating a server-rendered page in place; on the server leaf it renders the
requested path to stdout (`wasmtime counter-server.wasm /` prints the page); on the
host it builds the tree against the headless platform, which is what view tests run on.

## A terminal app

A terminal app depends on `skene-tui` (and `skene-simple-tui` for the components) and
calls `launch` (`examples/tui/counter`):

```rust
use skene_simple_tui::prelude::*;
use skene_tui::prelude::*;

fn app() -> impl IntoView {
  let count = Signal::tagged("count", 0u64);

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    SimpleCard::new().set_children((
      Text::derive(move || count.get().to_string()).set_color(SimplePalette::FOREGROUND),
      Row::new().set_spacing(2).set_children((
        SimpleButton::new("-").handle_click(move || count.update(|n| *n = n.saturating_sub(1))),
        SimpleButton::new("+").handle_click(move || count.update(|n| *n = n.saturating_add(1))),
      )),
    ))
  })
}

fn main() {
  skene_tui::launch(app);
}
```

Lengths are cells (`u32`), text is a color, a bold flag and a size in cells (a scale
the terminal draws where it sizes text, and one cell elsewhere) measured in grapheme
clusters (a wide character takes two cells), a border is one cell of box-drawing
glyphs, or a thin line in pixels beneath the text where the terminal shows graphics;
what the terminal cannot draw does not exist to call. `Tab` moves
focus, the controls own their keys, the mouse clicks, drags, scrolls and selects—inside
a text entry too, where `Ctrl-C`, `Ctrl-X` and `Ctrl-V` copy, cut and paste—and
`Ctrl-Q` quits. `Launch::new().set_clipboard(Clipboard::COPY).set_mouse(false).run(app)`
chooses which clipboard operations the app offers (OSC 52 has no capability query;
where the terminal's clipboard answers, kitty's OSC 5522, every operation is offered
from its reply) and
whether it takes the mouse; the terminal's own selection—a drag with `Shift` held—stays
available either way, `Alt-M` hands the mouse to the terminal for one plain drag, and
with the mouse off a plain drag is the terminal's.

## Crates

```
skene-reactive        the engine
      │
skene-core            the spine: the scene, the Platform trait, dispatch, the property model
      │
skene-common          every built-in view once, generic over the platform; the pixel and
      │               terminal media (gui, tui), the router (routing)
      ├── skene-web        the web platform: browser (csr) and server (ssr) leaves
      ├── skene-headless   the host platform: an inspectable record store for tests
      └── skene-tui        the terminal facade and its platform
skene-gui             the GUI facade: aliases over the selected platform
skene-simple          the Simple component library over skene-common, generic
      ├── skene-simple-gui   its aliases and prelude for skene-gui
      └── skene-simple-tui   its aliases and prelude for skene-tui
skene-runner          a host library embedding wasmtime around the server build
skene-macros          #[main], #[lazy], #[derive(Store)]
skene-cli             the `skene` command
```

| Crate | Path | Role |
|-------|------|------|
| `skene-reactive` | `packages/reactive` | `Signal`, `Memo`, `Effect`, `Owner`, `Store`, `Resource`, context, batching, snapshot by position with `Key` anchors, the `spawn` seam a platform installs an executor into. |
| `skene-core` | `packages/core` | `Scene<P>`, the zero-sized `Platform` trait, event dispatch over the scene, `BoxStyle`/`RunStyle` with their visitors, `Value`/`Leaf`/`Expr`, `Media`/`State`/`Condition`, `Token`/`TokenSet`, `Boundary`, motion and interpolation, generic geometry and values. Knows nothing of platforms. |
| `skene-common` | `packages/common` | `Container`, `Row`/`Column`/`Grid`/`Stack`/`LayoutBox`, `Flexible`/`Pinned`/`GridCell`, `Text`/`Span`/`RichText`, `Control<B: Behavior>` with `Toggle`/`Range`/`Roving`/`Popup`/`Activate`/`TextEntry`/`NativePicker` and the `Raw*` aliases, `Overlay`, `Tooltip`, `ContextMenu` with `MenuItem` and the `Menu`/`MenuOption` behaviors, `LazyColumn`/`LazyRow`, `Show`/`Switch`/`Suspense`/`ErrorBoundary`/`Enhance`, `Theme`, `typography`, and the painting path (`Painting`, `Canvas`, `Painted` with its fallback, `IntoBrush`, the `Renderer` a canvas replays into) through vello; `gui` (`GuiExt`, the pixel `Typography`), `tui` (`TuiExt`, `BorderGlyphs`, the terminal `Typography`) and `routing` (`Route`, `StaticRoutes`, `Router`: a typed route in a signal with a sink seam each facade binds). |
| `skene-web` | `packages/web` | The web `Platform`: a DOM sink in the browser, a string tree on the server, one compiler, the content-addressed interner, hydration, streaming, the WASI reactor, client hints; the `WebMarkup` and `WebDom` escape traits. |
| `skene-headless` | `packages/headless` | The host `Platform`: records what any platform receives, resolves properties through tokens and conditions, drives events through `trigger_*` seams, runs tasks on `pump_tasks`. |
| `skene-gui` | `packages/gui` | The GUI facade: `f32`, `ActivePlatform`, aliases for every view, `Typography`, `media`, `Resource` across the server boundary, `Deferred`, `Lazy`, `Link`, `head`, `view_transitions`, `launch`/`render_to_string`/`render_main` and `#[main]`. |
| `skene-tui` | `packages/tui` | The terminal facade and platform: `u32` cells, `Tui`, the same aliases, `Typography`, `BorderGlyphs`, `media`, `CellPainter`, `launch`, `render_to_text` and the `trigger_*` seams. |
| `skene-simple` | `packages/simple` | `SimplePalette` (a `TokenSet`), `SimpleLook`, `SimpleVariant`, `SimpleShape`, `SimpleGlyphs` (the terminal marks), and `SimpleButton`, `SimpleCard`, `SimpleCheckbox`, `SimpleRadio`, `SimplePicker`, `SimpleSlider`, `SimpleTextField`, `SimpleTextEditor`, `SimpleTooltip`, `SimpleContextMenu`, generic over the platform. |
| `skene-simple-gui`, `skene-simple-tui` | `packages/simple-gui`, `packages/simple-tui` | The Simple types aliased to that facade's `ActivePlatform`, with a `prelude` an app imports beside the facade's own. |
| `skene-runner` | `packages/runner` | `Runner`: loads the app's `wasm32-wasip2` build, renders a route or a stream, reads client hints, enumerates static routes. |
| `skene-macros` | `packages/macros` | `#[main]`, `#[lazy]`, `#[derive(Store)]`. |
| `skene-http` | `packages/http` | An HTTP client on the public seams alone: `Request`, `Response`, `send`; `fetch` on the web, `wasi:http` through the server leaf's reactor on a server render, a thread around `ureq` on every other host. |
| `skene-ws` | `packages/ws` | A WebSocket client on the public seams alone: `connect`, `send`, `receive`; the browser's `WebSocket` on the web, a thread around `tungstenite` on other hosts, `wasi:sockets` with rustls on a server render. |
| `skene-cli` | `packages/cli` | `skene build web [--static]`, `skene build tui`, `skene run web [--ssr]`, `skene run tui` and `skene attach <port>`: bundling, the dev server with live reload and state-preserving restarts, the terminal build loop and the terminal it attaches to, static generation, `skene.toml` assets. |

## Views, controls, themes

A view is its own builder and a `View`: chain typed setters and pass it to a parent.
One convention: `set_<prop>` for a fixed value, `derive_<prop>` for a reactive one,
`handle_<event>` for a handler, and children last. Every setter takes a value that may
be fixed, a token, an expression, derived from signals, or any of those under a
condition: `set_spacing(Spacing::from(8.).when(Media::MinWidth(900.), 16.))`.

Each layout is its own view—`Row`, `Column`, `Grid`, `Stack`—exposing only the
properties it honors; per-child properties ride in `Flexible`, `Pinned` and `GridCell`,
accepted only by the matching container. `Container` is the one decorated box around
exactly one child. A control is `Control<B: Behavior>`; `RawButton`, `RawCheckbox`,
`RawSlider`, `RawRadioGroup`, `RawPicker`, `RawTextField` and `RawTextEditor` are
aliases over the shipped behaviors, and the `Simple*` components dress them:

```rust
SimpleButton::new("Save").handle_click(save),
SimpleButton::new("Delete").set_variant(SimpleVariant::Destructive).handle_click(delete),
```

`Theme::new(palette)` defines a token set beneath it and `Theme::derive(|| palette.get())`
follows a signal, redefining the tokens in place so nothing rebuilds; `SimplePalette`
is one such set, and a third party's palette is on identical footing. Light and dark
are one condition, `Media::Dark`, answered per token; `media::set_scheme` puts a scheme
in force for the whole document.

## Build & run

```sh
rustup target add wasm32-unknown-unknown wasm32-wasip2
cargo install wasm-bindgen-cli   # version must match the wasm-bindgen crate (see Cargo.lock)

cargo skene --dir examples/gui/counter run web             # build, serve on :8080, live-reload
cargo skene --dir examples/gui/router build web --static   # statically generate the routed site
cargo skene --dir examples/tui/counter run tui             # build, rebuild on change, print the attach command
cargo skene attach <port>                                  # run the app here, swapped on each rebuild
```

Examples live under `examples/gui` and `examples/tui`, one folder per feature, the same
names on both facades where the feature exists: `counter` (state, snapshot-preserving
reloads), `gallery` (every built-in view and control, a live palette switch), `theme`
(a palette built per property, an accent the reader switches, a nested theme, the color
scheme), `store` (a todo list over one store: rows following their own entries, a list
following the shape, a memoized count), `context-menu` (a menu each view opts into, the
clipboard's standard items, the platform's menu everywhere else), `enhance` (a view's
baseline and its enhancement, swapped as the page hydrates or pixels come), `http` (a
resource or a task fetching through `skene-http`), `ws` (a socket to an echo service
through `skene-ws`), the GUI-only `router` (typed routes behind real links, the about page split
out with `#[lazy]`), and the TUI-only `text-sizing` (font sizes as scales of the
cell, drawn where the terminal proves the text sizing protocol). `examples/third-party` holds the crates written from outside the
framework. Every output lands under `<app>/dist/<target>`; `cargo skene` runs the
workspace-local CLI (`cargo install --path packages/cli` makes it `skene`).

The server build of a GUI app is a plain command, with no integration at all:

```sh
cargo build -p counter --target wasm32-wasip2
wasmtime target/wasm32-wasip2/debug/counter-server.wasm /   # prints the page
```

## Testing

Three tiers, and the host tier alone proves nothing about the web:

```sh
cargo test --workspace                                    # host: the engine, the spine, every view on the headless platform
cargo test -p skene-gui --target wasm32-wasip2            # server leaf: wasmtime, a byte-exact render snapshot
cargo test -p skene-gui --target wasm32-unknown-unknown   # browser leaf: a real DOM through geckodriver
```

Read the `test result:` lines rather than the exit codes: wasmtime aborts on the first
panic and prints `Error: failed to run main module` instead of a `FAILED` line, and the
browser runner panics on cleanup after reporting, which stops cargo before the
remaining binaries run—so run each browser binary by name (`--test web_dom`,
`web_hydrate`, `web_painted`, `web_resource`, `web_task`) and add the totals. `skene-web` carries its
own `web_dom` and `web_ssr` binaries on the same two targets, and the `gallery` example
runs axe-core over every built-in in `web_axe`.

## Extending

A component library depends on `skene-common`, writes generic over `P: Platform` exactly as
the built-ins do, states what it needs from a medium as a bound on the platform's
associated types, and works on every facade from one crate. A low-level view is written
against a platform's escape trait (`WebDom`, `WebMarkup`, `CellPainter`) and realized
through the same scene as a built-in; `Painted` is the portable one for pixels, a
vello scene the platform renders, the terminal through the kitty graphics protocol
where it proves one. `examples/third-party` shows all three: `acme-ui`
(a palette and a badge over any platform), `acme-map` (a canvas on the browser leaf with a
server placeholder) and `acme-sparkline` (bars painted into the terminal's cells), each
used by a gallery and tested on its platform.
