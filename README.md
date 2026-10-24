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

The guide, an mdbook in `guide/` whose examples compile against these crates, walks
through building an app and a component library.

## Reactivity

```rust,ignore
let count = Signal::new(0);
count.update(|n| *n += 1);   // re-runs only the effects that read `count`
```

A signal write re-runs exactly the effects that depend on it, and each effect patches
the node it owns—no virtual DOM, no tree diffing. A signal made with
`Signal::saved` or `Signal::tagged` rides the state snapshot, so a dev-loop restart
resumes where the last run stood; a keyed list item or a view given `set_key` keeps its
state by key rather than by position.

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
      .set_background(SimplePalette::BACKGROUND)
      .set_child(
        Column::new()
          .set_arrangement(Arrangement::Center)
          .set_item_alignment(ItemAlignment::Center)
          .add_child(
            SimpleCard::new().set_spacing(24).set_children((
              Text::derive(move || count.get().to_string()).set_font_size(56.),
              Row::new().set_spacing(12).set_children((
                SimpleButton::new("−").handle_activate(move || count.update(|n| *n = n.saturating_sub(1))),
                SimpleButton::new("+").handle_activate(move || count.update(|n| *n = n.saturating_add(1))),
              )),
            )),
          ),
      )
  })
}
```

`#[main]` expands to whichever entry the target has: in the browser it launches the
app, hydrating a server-rendered page in place; on the server leaf it renders the
requested path to stdout (the server build's `counter /` prints the page); on the
host it only compiles, against a stand-in that runs nothing.
The app is its library alone, which `skene` builds as each target's crate type.

## A terminal app

A terminal app depends on `skene-terminal` (and `skene-simple-terminal` for the components) and
marks its entry with `#[main]` (`examples/terminal/counter`):

```rust
use skene_simple_terminal::prelude::*;
use skene_terminal::prelude::*;

#[main]
fn app() -> impl View {
  let count = Signal::tagged("count", 0u64);

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    SimpleCard::new().set_children((
      Text::derive(move || count.get().to_string()).set_color(SimplePalette::FOREGROUND),
      Row::new().set_spacing(2).set_children((
        SimpleButton::new("-").handle_activate(move || count.update(|n| *n = n.saturating_sub(1))),
        SimpleButton::new("+").handle_activate(move || count.update(|n| *n = n.saturating_add(1))),
      )),
    ))
  })
}
```

Lengths are whole cells (`i32`), text is a color, a bold flag and a size in cells (a scale
the terminal draws where it sizes text, and one cell elsewhere) measured in grapheme
clusters (a wide character takes two cells), a border is one cell of box-drawing
glyphs, or a thin line in pixels beneath the text where the terminal shows graphics;
what the terminal cannot draw does not exist to call. `Tab` moves
focus, the controls own their keys, the mouse clicks, drags, scrolls and selects—inside
a text entry too, where `Ctrl-C`, `Ctrl-X` and `Ctrl-V` copy, cut and paste—and
`Ctrl-Q` quits. `Launch::new().set_clipboard(Clipboard::COPY).set_mouse(false).run(app)`,
in a library's own `pub fn main` in place of `#[main]`, chooses which clipboard
operations the app offers (OSC 52 has no capability query;
where the terminal's clipboard answers, kitty's OSC 5522, every operation is offered
from its reply) and
whether it takes the mouse; the terminal's own selection—a drag with `Shift` held—stays
available either way, `Alt-M` hands the mouse to the terminal for one plain drag, and
with the mouse off a plain drag is the terminal's.

## Crates

| Crate | Path | Role |
|-------|------|------|
| `skene-reactive` | `packages/reactive` | `Signal`, `Memo`, `Effect`, `Owner`, `Store`, `Resource`, context, batching, snapshot by position with `Key` anchors, the `spawn` seam a platform installs an executor into. |
| `skene-foundation` | `packages/foundation` | The spine: `Scene<P>`, the zero-sized `Realization` trait, event dispatch over the scene, `BoxStyle`/`RunStyle` with their visitors, `Value`/`Leaf`/`Expr`, `Media`/`State`/`Condition`, `Token`/`TokenSet`, `Boundary`, motion and interpolation, generic geometry and values, knowing nothing of platforms. Beside it, every built-in view: `Container`, `Row`/`Column`/`Grid`/`Stack`/`LayoutBox`, `Flexible`/`Pinned`/`GridCell`, `Text`/`Span`/`RichText`, `Control<B: Behavior>` with `Toggle`/`Slider`/`RadioGroup`/`Popup`/`Activate`/`TextEntry`/`NativePicker` and the `Basic*` aliases, `Overlay`, `Tooltip`, `ContextMenu` with `MenuEntry` and the `Menu`/`MenuOption` behaviors, `LazyColumn`/`LazyRow`, `Link` to a route, `Visibility`/`OneOf`/`Suspense`/`ErrorBoundary`/`Enhance`, `Theme`, `typography`, the painting path (`Painting`, `Canvas`, `Painted` with its fallback, `IntoBrush`, the `Renderer` a canvas replays into) through vello, and `text`: the attributed text model, the `TextEngine` contract every realization's engine implements, the layout's `TextLeaf`, the `TextIndex` between bytes, code points, UTF-16 and grapheme clusters, and `editing`, the editing core every drawn entry shares, beside the spine's `Edit`, `Verdict`, `Unit` and `AccessText`; `terminal` (`TerminalStyle`, `BorderGlyphs`, the terminal `Typography`), `web` (`WebStyle`, the web's `Typography`, the document `Head`, and `WebHydration` with the hydrating `Resource`, the streaming `Deferred` and the lazily loaded `AsyncView`) and `routing` (`Route`, `StaticRoutes`, `Router`, `install_router`: a typed route in a signal, bound to the platform's history through its seams). |
| `skene-layout` | `packages/layout` | The layout every platform but the web computes: the resolved style model on taffy (`BoxLayout`), generic over the scalar, a `Tree` keyed by the scene's nodes, retained across layouts and written node by node, by a walk of the scene or for the nodes a shared `Restyle` says moved, each node's part classified by its platform, and `TextFlow`, a box's inline runs and boxes laid out as one text through the platform's text engine, answering which run a point falls on. |
| `skene-parley` | `packages/parley` | The `TextEngine` over the patched Parley (`vendor/parley`), on the typefaces it is given and no system font: paragraphs shaped once and broken per width, on threads off the web for a batch, bidirectional carets and movement, ICU4X's dictionaries for complex-script breaking, and drawing through vello. |
| `skene-headless` | `packages/headless` | The host leaf `skene-web` selects off its targets, so the builds that only compile on the host (the docs crates' doctests, the web examples' libraries, `#[main]`'s host arm) have a `Realization` to build against; no test runs on it. |
| `skene-web` | `packages/web` | The web facade and platform: `f32`, `Active`, a type of its own for every view, `Typography`, `media`, `Resource` across the server boundary, `Deferred`, `AsyncView`, `head`, `view_transitions`, `launch`/`render_to_string`/`render_main` and `#[main]`; and `platform`, the web `Realization`: a DOM sink in the browser, a string tree on the server, one compiler, the content-addressed interner, hydration, streaming, the server executor, client hints, the `WebMarkup` and `WebDom` escape traits, and `skene-dev`'s served `Document`, re-exported. |
| `skene-terminal` | `packages/terminal` | The terminal facade and platform: `i32` cells, `Terminal`, the same views, its own decoration on them, `Typography`, `BorderGlyphs`, `media`, `CellPainter`, `launch`, and `testing`: `render_to_text`, the `trigger_*` seams, `user` and the queries by role, name and text. |
| `skene-conformance` | `packages/conformance` | One suite of behaviors every platform shows alike, run through the `Driver` a platform implements over its test seams. |
| `skene-conformance-web` | `packages/conformance-web` | The conformance suite in real browsers: its pages served as one web app, acted on over WebDriver by trusted input. |
| `skene-simple` | `packages/simple` | The Simple core: `SimplePalette` (a `TokenSet`), `SimpleLook`, `SimpleVariant`, `SimpleShape`, `SimpleGlyphs` (the terminal marks), and the views, generic over the platform and hidden, their members in a `Simple*Base` trait each. |
| `skene-simple-terminal` | `packages/simple-terminal` | The Simple views on the terminal: `SimpleButton`, `SimpleCard`, `SimpleCheckbox`, `SimpleRadio`, `SimplePicker`, `SimpleSlider`, `SimpleTextField`, `SimpleTextEditor`, `SimpleTooltip`, `SimpleContextMenu`, the palette in cells, and a `prelude` an app imports beside the facade's own. |
| `skene-simple-web` | `packages/simple-web` | The Simple views on the web, as on the terminal, the palette in pixels. |
| `skene-macros` | `packages/macros` | `#[main]`, `#[lazy]`, `#[derive(Store)]`, `#[derive(Snapshot)]`, and `#[derive(View)]` and `#[derive(Wraps)]` for a view over another. |
| `skene-transport` | `packages/transport` | The clients on the public seams alone: `http` (`Request`, `Response`, `send`; `fetch` on the web, a thread around `ureq` on every other host) and `ws` (`connect`, `send`, `receive`; the browser's `WebSocket` on the web, a thread around `tungstenite` elsewhere). |
| `skene-build` | `packages/build` | The build script naming the web leaf a build compiles for, a build-dependency of every crate with code per leaf. |
| `skene-dev` | `packages/dev` | The dev loop's protocol between the `skene` command and the app it runs, and the `Document` a web page is served as, the server leaf's pages and the command's client-rendered shell alike. |
| `skene-cli` | `packages/cli` | `skene new app <name> <platform>...`, `skene new lib <name> <platform>...`, `skene build web [--static]`, `skene build terminal`, `skene run web [--ssr]`, `skene run terminal` and `skene attach <id>`: bundling, the dev server with live reload and state kept across it in the page's own storage, the terminal build loop and the launcher that swaps each binary in place with the state carried across, static generation, `skene.toml` assets, and the `public` directory served at the site's root; it depends on no facade, writing its shell through `skene-dev`'s `Document`. |

## Views, controls, themes

A view is its own builder and a `View`: chain typed setters and pass it to a parent.
One convention: `set_<prop>` for a fixed value, `derive_<prop>` for a reactive one,
`handle_<event>` for a handler, and children last. Every setter takes a value that may
be fixed, a token, an expression, derived from signals, or any of those under a
condition: `set_spacing(Spacing::from(8).when(Media::min_width(900), 16))`. A length is
a plain number, an `i32` or an `f32`.

Each layout is its own view—`Row`, `Column`, `Grid`, `Stack`—exposing only the
properties it honors; per-child properties ride in `Flexible`, `Pinned` and `GridCell`,
accepted only by the matching container. `Container` is the one decorated box around
exactly one child. A control is `Control<B: Behavior>`; `BasicButton`, `BasicCheckbox`,
`BasicSlider`, `BasicRadioGroup`, `BasicPicker`, `BasicTextField` and `BasicTextEditor` are
aliases over the shipped behaviors, and the `Simple*` components dress them:

```rust,ignore
SimpleButton::new("Save").handle_activate(save),
SimpleButton::new("Delete").set_variant(SimpleVariant::Destructive).handle_activate(delete),
```

`Theme::new(palette)` defines a token set beneath it and `Theme::derive(|| palette.get())`
follows a signal, redefining the tokens in place so nothing rebuilds; `SimplePalette`
is one such set, and a third party's palette is on identical footing. Light and dark
are one condition, `Media::Dark`, answered per token; `media::set_scheme` puts a scheme
in force for the whole document.

## Build & run

```sh
rustup target add wasm32-unknown-unknown

skene new app counter web terminal                         # a shared crate and a shell per platform
skene new lib acme-ui web terminal                         # a library's core and a crate per platform
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
workspace-local CLI (`cargo install --path packages/cli` makes it `skene`). A release
web build runs its bundle through binaryen's `wasm-opt` where it is installed.

The server build of a web app is a plain, native command, with no integration at all:
its library built as a binary with the server platform named.

```sh
RUSTFLAGS='--cfg skene_platform="server"' cargo rustc -p counter --lib --crate-type bin
target/debug/counter /   # prints the page
```

## Testing

```sh
cargo xtask gate                 # every check a change passes
cargo xtask gate test:browser    # one step; a group such as `test` or `clippy` runs each of its steps
```

The gate checks formatting, manifest order, every patched crate vendored under
`vendor/`, the house rules, unused dependencies, clippy and rustdoc on every leaf,
clippy on the terminal's macOS paths, manifests free of cargo features, the minimum Rust
version in the host's, the server's and the browser's builds, and three test tiers. A
crate reaching `skene-web` is built for the web's leaves alone, and the host tier runs
every other crate's tests: the foundation's and the layout engine's unit tests and the
terminal's in-process tests, which act through real input and read what a reader finds.
The server tier builds every crate reaching the web on the server leaf natively and runs
their tests and doctests, the guide's and this README's examples, a byte-exact render
snapshot and the compile-fail suite (a program per rule the types enforce, beside the
diagnostic it must raise) among them; the browser tier drives a real Firefox and a real
Chromium, several sessions at once, the `gallery` example running axe-core over every
built-in, acts the conformance suite out in both by trusted input over WebDriver, and on
x86-64 Linux compares every page of the suite against its screenshot golden in a pinned
Firefox it fetches into `target/browsers` (`SKENE_GOLDENS=overwrite` rewrites them).
Each step logs to `target/gate/<step>.log`, and a step whose inputs are unchanged since
it passed is skipped unless `--force` is given. The gate needs the stable toolchain
`rust-toolchain.toml` pins, which rustup installs with clippy and the
`wasm32-unknown-unknown` and `aarch64-apple-darwin` targets, the nightly toolchain for
`rustfmt`, the toolchain the workspace's `rust-version` names with the
`wasm32-unknown-unknown` target, `cargo-nextest`, `cargo-shear`, Firefox, a chromedriver
matching the installed Chrome, `curl`, `tar` and `sha256sum` for the pinned Firefox, and
a `wasm-bindgen-test-runner` matching the locked `wasm-bindgen`; a step lacking one
fails naming it.

## Extending

A component library is a core and a crate per platform, as Simple is: the core depends on
`skene-foundation`, writes its views generic over `P: Realization` exactly as the built-ins do
and states what it needs from a medium as a bound on the platform's associated types, and
each platform's crate declares a newtype per view over the core's. A low-level view is written
against a platform's escape trait (`WebDom`, `WebMarkup`, `CellPainter`) and realized
through the same scene as a built-in; `Painted` is the portable one for pixels, a
vello scene the platform renders, the web on the CPU or, with `skene-web`'s `gpu`
feature, on the GPU where the page has WebGPU, and the terminal through the kitty
graphics protocol where it proves one. `examples/third-party` shows both escape paths: `acme-map` (a canvas
on the browser leaf, a placeholder before it and on the server, an image box on headless)
and `acme-sparkline` (bars painted into the terminal's cells, and as pixels over them
where the terminal proves its graphics), each used by its platform's gallery and tested
on its platform.

## License

MIT; see the `LICENSE` file.
