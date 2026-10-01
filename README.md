# Skene

A client-side Rust UI framework: one spine, a facade per platform. A web app depends on
`skene-web` and runs wherever the build targets it—the browser, client-rendered; a
server, rendering or statically generating pages from the same app crate built natively.
A terminal app depends on `skene-terminal`, a facade of equal standing with its own
vocabulary typed to the medium. Every built-in view is written once, generic over the
platform a facade selects; the reactive engine is signal-based, fine-grained and
vdom-free; styling is data, compiled to CSS on the web and resolved natively elsewhere,
so a server-rendered page is responsive, themed, hoverable and animated with no script
at all.

## Reactivity

```rust
let count = Signal::new(0);
count.update(|n| *n += 1);   // re-runs only the effects that read `count`
```

A signal write re-runs exactly the effects that depend on it, and each effect patches
the node it owns—no virtual DOM, no tree diffing. Signals snapshot by default, so a
dev-loop restart resumes where the last run stood; a keyed list item or a view given
`set_key` keeps its state by key rather than by position.

## A web app

A web app depends on `skene-web` and, for the ready-made components, on
`skene-simple-web`; it marks its entry with `#[main]` and names no platform
(`examples/web/counter`):

```rust
use skene_simple_web::prelude::*;
use skene_web::prelude::*;

#[main]
fn app() -> impl View {
  let count = Signal::tagged("count", 0u64);

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    Container::new()
      .set_color(SimplePalette::BACKGROUND)
      .set_child(
        Column::new()
          .set_arrangement(Arrangement::Center)
          .set_item_alignment(ItemAlignment::Center)
          .add_child(
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
requested path to stdout (`counter-server /` prints the page); on the
host it builds the tree against the headless platform, which is what view tests run on.

## A terminal app

A terminal app depends on `skene-terminal` (and `skene-simple-terminal` for the components) and
calls `launch` (`examples/terminal/counter`):

```rust
use skene_simple_terminal::prelude::*;
use skene_terminal::prelude::*;

fn app() -> impl View {
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
  skene_terminal::launch(app);
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
skene-foundation      the spine (the scene, the Platform trait, dispatch, the property
      │               model) and every built-in view once over it, generic over the
      │               platform; the terminal and web media (terminal, web), the router
      │               (routing)
      ├── skene-headless   the host platform: an inspectable record store for tests
      ├── skene-terminal   the terminal facade and its platform
      └── skene-web        the web facade and its platform, the browser and server
                           leaves, over the headless platform off its target
skene-simple          the Simple component library's core over skene-foundation: its
      │               palette, shapes and marks, and its views, generic and hidden
      ├── skene-simple-terminal   the Simple views on the terminal, with a prelude
      └── skene-simple-web        the Simple views on the web, with a prelude
skene-macros          #[main], #[lazy], #[derive(Store)], #[derive(Snapshot)],
                      #[derive(View)], #[derive(Wraps)]
skene-cli             the `skene` command
```

| Crate | Path | Role |
|-------|------|------|
| `skene-reactive` | `packages/reactive` | `Signal`, `Memo`, `Effect`, `Owner`, `Store`, `Resource`, context, batching, snapshot by position with `Key` anchors, the `spawn` seam a platform installs an executor into. |
| `skene-foundation` | `packages/foundation` | The spine: `Scene<P>`, the zero-sized `Platform` trait, event dispatch over the scene, `BoxStyle`/`RunStyle` with their visitors, `Value`/`Leaf`/`Expr`, `Media`/`State`/`Condition`, `Token`/`TokenSet`, `Boundary`, motion and interpolation, generic geometry and values, knowing nothing of platforms. Beside it, every built-in view: `Container`, `Row`/`Column`/`Grid`/`Stack`/`LayoutBox`, `Flexible`/`Pinned`/`GridCell`, `Text`/`Span`/`RichText`, `Control<B: Behavior>` with `Toggle`/`Slider`/`RadioGroup`/`Popup`/`Activate`/`TextEntry`/`NativePicker` and the `Basic*` aliases, `Overlay`, `Tooltip`, `ContextMenu` with `MenuEntry` and the `Menu`/`MenuOption` behaviors, `LazyColumn`/`LazyRow`, `Show`/`Switch`/`Suspense`/`ErrorBoundary`/`Enhance`, `Theme`, `typography`, and the painting path (`Painting`, `Canvas`, `Painted` with its fallback, `IntoBrush`, the `Renderer` a canvas replays into) through vello; `terminal` (`TerminalStyle`, `BorderGlyphs`, the terminal `Typography`), `web` (`WebStyle`, the web's `Typography`, the document `Head`) and `routing` (`Route`, `StaticRoutes`, `Router`: a typed route in a signal with a sink seam each facade binds). |
| `skene-headless` | `packages/headless` | The host `Platform`: records what any platform receives, resolves properties through tokens and conditions, drives events through `trigger_*` seams and a user's gestures through `user`, finds nodes by role, name and text, runs tasks on `pump_tasks`. |
| `skene-web` | `packages/web` | The web facade and platform: `f32`, `ActivePlatform`, a type of its own for every view, `Typography`, `media`, `Resource` across the server boundary, `Deferred`, `AsyncView`, `Link`, `head`, `view_transitions`, `launch`/`render_to_string`/`render_main` and `#[main]`; and `platform`, the web `Platform`: a DOM sink in the browser, a string tree on the server, one compiler, the content-addressed interner, hydration, streaming, the server executor, client hints, the `WebMarkup` and `WebDom` escape traits. |
| `skene-terminal` | `packages/terminal` | The terminal facade and platform: `u32` cells, `Terminal`, the same views, its own decoration on them, `Typography`, `BorderGlyphs`, `media`, `CellPainter`, `launch`, and `testing`: `render_to_text`, the `trigger_*` seams, `user` and the queries by role, name and text. |
| `skene-conformance` | `packages/conformance` | One suite of behaviors every platform shows alike, run through the `Driver` a platform implements over its test seams. |
| `skene-simple` | `packages/simple` | The Simple core: `SimplePalette` (a `TokenSet`), `SimpleLook`, `SimpleVariant`, `SimpleShape`, `SimpleGlyphs` (the terminal marks), and the views, generic over the platform and hidden, their members in a `Simple*Base` trait each. |
| `skene-simple-terminal` | `packages/simple-terminal` | The Simple views on the terminal: `SimpleButton`, `SimpleCard`, `SimpleCheckbox`, `SimpleRadio`, `SimplePicker`, `SimpleSlider`, `SimpleTextField`, `SimpleTextEditor`, `SimpleTooltip`, `SimpleContextMenu`, the palette in cells, and a `prelude` an app imports beside the facade's own. |
| `skene-simple-web` | `packages/simple-web` | The Simple views on the web, as on the terminal, the palette in pixels. |
| `skene-macros` | `packages/macros` | `#[main]`, `#[lazy]`, `#[derive(Store)]`, `#[derive(Snapshot)]`, and `#[derive(View)]` and `#[derive(Wraps)]` for a view over another. |
| `skene-transport` | `packages/transport` | The clients on the public seams alone: `http` (`Request`, `Response`, `send`; `fetch` on the web, a thread around `ureq` on every other host) and `ws` (`connect`, `send`, `receive`; the browser's `WebSocket` on the web, a thread around `tungstenite` elsewhere). |
| `skene-cli` | `packages/cli` | `skene build web [--static]`, `skene build terminal`, `skene run web [--ssr]`, `skene run terminal` and `skene attach <id>`: bundling, the dev server with live reload and state kept across it in the page's own storage, the terminal build loop and the launcher that swaps each binary in place with the state carried across, static generation, `skene.toml` assets. |

## Views, controls, themes

A view is its own builder and a `View`: chain typed setters and pass it to a parent.
One convention: `set_<prop>` for a fixed value, `derive_<prop>` for a reactive one,
`handle_<event>` for a handler, and children last. Every setter takes a value that may
be fixed, a token, an expression, derived from signals, or any of those under a
condition: `set_spacing(Spacing::from(8.).when(Media::MinWidth(900.), 16.))`.

Each layout is its own view—`Row`, `Column`, `Grid`, `Stack`—exposing only the
properties it honors; per-child properties ride in `Flexible`, `Pinned` and `GridCell`,
accepted only by the matching container. `Container` is the one decorated box around
exactly one child. A control is `Control<B: Behavior>`; `BasicButton`, `BasicCheckbox`,
`BasicSlider`, `BasicRadioGroup`, `BasicPicker`, `BasicTextField` and `BasicTextEditor` are
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
rustup target add wasm32-unknown-unknown

cargo skene --dir examples/web/counter run web             # build, serve on :8080, live-reload
cargo skene --dir examples/web/router build web --static   # statically generate the routed site
cargo skene --dir examples/terminal/counter run terminal   # build, rebuild on change, print the attach command
cargo skene attach <id>                                    # run the app here, swapped on each rebuild
```

Examples live under `examples/web` and `examples/terminal`, one folder per feature, the same
names on both facades where the feature exists: `counter` (state, snapshot-preserving
reloads), `gallery` (every built-in view and control, a live palette switch), `theme`
(a palette built per property, an accent the reader switches, a nested theme, the color
scheme), `store` (a todo list over one store: rows following their own entries, a list
following the shape, a memoized count), `context-menu` (a menu each view opts into, the
clipboard's standard items, the platform's menu everywhere else), `enhance` (a view's
baseline and its enhancement, swapped as the page hydrates or pixels come), `http` (a
resource or a task fetching through `skene_transport::http`), `ws` (a socket to an echo service
through `skene_transport::ws`), the web-only `router` (typed routes behind real links, the about page split
out with `#[lazy]`), and the terminal-only `text-sizing` (font sizes as scales of the
cell, drawn where the terminal proves the text sizing protocol). `examples/third-party` holds the crates written from outside the
framework. Every output lands under `<app>/dist/<target>`; `cargo skene` runs the
workspace-local CLI (`cargo install --path packages/cli` makes it `skene`).

The server build of a web app is a plain, native command, with no integration at all:

```sh
RUSTFLAGS='--cfg skene_platform="server"' cargo build -p counter --bin counter-server
target/debug/counter-server /   # prints the page
```

## Testing

```sh
cargo xtask gate                 # every check a change passes
cargo xtask gate test:browser    # one step; a group such as `test` or `clippy` runs each of its steps
```

The gate checks formatting, manifest order, the house rules, unused dependencies, clippy
and rustdoc on every leaf, clippy on the terminal's macOS paths, manifests free of cargo
features, the minimum Rust version, and three test tiers, of which the host alone proves
nothing about the web: the host tier runs the engine, the spine and every view on the
headless platform; the server tier builds the server leaf natively, a byte-exact render
snapshot among its tests; the browser tier drives a real Firefox and a real Chromium,
several sessions at once, the `gallery` example running axe-core over every built-in.
Each step logs to `target/gate/<step>.log`, and a step whose inputs are unchanged since it
passed is skipped unless `--force` is given. The gate needs the nightly toolchain for
`rustfmt`, the toolchain the workspace's `rust-version` names, the
`wasm32-unknown-unknown` and `aarch64-apple-darwin` targets, `cargo-nextest`,
`cargo-shear`, Firefox, a chromedriver matching the installed Chrome, and a
`wasm-bindgen-test-runner` matching the locked `wasm-bindgen`; a step lacking one fails
naming it.

## Extending

A component library depends on `skene-foundation`, writes generic over `P: Platform` exactly as
the built-ins do, states what it needs from a medium as a bound on the platform's
associated types, and works on every facade from one crate. A low-level view is written
against a platform's escape trait (`WebDom`, `WebMarkup`, `CellPainter`) and realized
through the same scene as a built-in; `Painted` is the portable one for pixels, a
vello scene the platform renders, the terminal through the kitty graphics protocol
where it proves one. `examples/third-party` shows all three: `acme-ui`
(a palette and a badge over any platform), `acme-map` (a canvas on the browser leaf, a
placeholder before it and on the server, an image box on headless) and `acme-sparkline` (bars painted into the terminal's cells, and as pixels over them
where the terminal proves its graphics), each used by a gallery and tested on its
platform.
