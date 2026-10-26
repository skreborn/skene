# Skene

A client-side Rust UI framework: one spine, a facade per platform. A web app depends on
`skene-gui` and runs wherever the build targets it—the browser, client-rendered; a
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

A web app depends on `skene-gui` and, for the ready-made components, on
`skene-simple-gui`; it marks its entry with `#[main]` and names no platform
(`examples/gui/counter`):

```rust
# #[cfg(skene_platform = "web")] {
use skene_gui::prelude::*;
use skene_simple_gui::prelude::*;

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
          .add_child(SimpleCard::new().set_child(
            Column::new().set_spacing(24).set_children((
              Text::derive(move || count.get().to_string()).set_font_size(56.),
              Row::new().set_spacing(12).set_children((
                SimpleButton::new("−").handle_activate(move || count.update(|n| *n = n.saturating_sub(1))),
                SimpleButton::new("+").handle_activate(move || count.update(|n| *n = n.saturating_add(1))),
              )),
            )),
          )),
      )
  })
}
# }
```

`#[main]` expands to the entry the build names: in the browser, the `web` build on
`wasm32-unknown-unknown`, it launches the app, hydrating a server-rendered page in
place; on the server leaf, the `web` build with `--cfg skene_server` beside it, it
renders the requested path to stdout (the server build's `counter /` prints the page).
A build naming no platform, or a pixel platform `skene-gui` does not realize yet, fails
with `skene-gui`'s message naming the web's builds, and a terminal build compiles
`skene-gui` empty, as it does an app gated on the web.
The app is its library alone, which `skene` builds as each target's crate type.

## A terminal app

A terminal app depends on `skene-terminal` (and `skene-simple-terminal` for the components) and
marks its entry with `#[main]` (`examples/terminal/counter`):

```rust
# #[cfg(skene_platform = "terminal")] {
use skene_simple_terminal::prelude::*;
use skene_terminal::prelude::*;

#[main]
fn app() -> impl View {
  let count = Signal::tagged("count", 0u64);

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    SimpleCard::new().set_child(
      Column::new().set_spacing(SimpleCard::spacing()).set_children((
        Text::derive(move || count.get().to_string()).set_color(SimplePalette::FOREGROUND),
        Row::new().set_spacing(2).set_children((
          SimpleButton::new("-").handle_activate(move || count.update(|n| *n = n.saturating_sub(1))),
          SimpleButton::new("+").handle_activate(move || count.update(|n| *n = n.saturating_add(1))),
        )),
      )),
    )
  })
}
# }
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
| `skene-foundation` | `packages/foundation` | The spine: `Scene<P>`, the zero-sized `Realization` trait, event dispatch over the scene, `BoxStyle`/`RunStyle` with their visitors, `Value`/`Leaf`/`Expr`, `Media`/`State`/`Condition`, `Token`/`TokenSet`, `Boundary`, motion and interpolation, generic geometry and values, knowing nothing of platforms. Beside it, every built-in view: `Container`, `Row`/`Column`/`Grid`/`Stack`/`LayoutBox`, `Flexible`/`Pinned`/`GridCell`, `Text`/`Span`/`RichText`, `Control<B: Behavior>` with `Toggle`/`Slider`/`RadioGroup`/`Popup`/`Activate`/`TextEntry`/`NativePicker` and the `Basic*` aliases, `Overlay`, `Tooltip`, `ContextMenu` with `MenuEntry` and the `Menu`/`MenuOption` behaviors, `LazyColumn`/`LazyRow`, `Link` to a route, `Visibility`/`OneOf`/`Suspense`/`ErrorBoundary`/`Enhance`, `Theme`, `typography`, `SystemColors` (the platform's colors, which each realization defines at the root), the painting path (`Painting`, `Canvas`, `Painted` with its fallback, `IntoBrush`, the `Renderer` a canvas replays into) through vello, and `text`: the attributed text model, the `TextEngine` contract every realization's engine implements, the layout's `TextLeaf`, the `TextIndex` between bytes, code points, UTF-16 and grapheme clusters, and `editing`, the editing core every drawn entry shares, beside the spine's `Edit`, `Verdict`, `Unit` and `AccessText`; `terminal` (`TerminalStyle`, `BorderGlyphs`, the terminal `Typography`), `web` (`WebStyle`, the web's `Typography`, the document `Head`, and `WebHydration` with the hydrating `Resource`, the streaming `Deferred` and the lazily loaded `AsyncView`) and `routing` (`Route`, `StaticRoutes`, `Router`, `install_router`: a typed route in a signal, bound to the platform's history through its seams). |
| `skene-layout` | `packages/layout` | The layout every platform but the web computes: the resolved style model on taffy (`BoxLayout`), generic over the scalar, a `Tree` keyed by the scene's nodes, retained across layouts and written node by node, by a walk of the scene or for the nodes a shared `Restyle` says moved, each node's part classified by its platform, and `TextFlow`, a box's inline runs and boxes laid out as one text through the platform's text engine, answering which run a point falls on. |
| `skene-parley` | `packages/parley` | The `TextEngine` over the patched Parley (`vendor/parley`), on the typefaces it is given and no system font: paragraphs shaped once and broken per width, on threads off the web for a batch, bidirectional carets and movement, ICU4X's dictionaries for complex-script breaking, and drawing through vello. Its tests and `skene-layout`'s shape in the pinned Inter face under `fonts`. |
| `skene-gui` | `packages/gui` | The web facade and platform: `f32`, `Active`, a type of its own for every view, the drawn built-in controls (`Button`, `Checkbox`, `Toggle`, `RadioGroup<T>`/`Radio`, `Slider`, `TextField`, `TextEditor`, `Picker<T>`, `Listbox<C>`/`ListboxOption`) in the platform's colors, `behavior` (the shipped behaviors a `Control` runs over, `Behavior` with what its hooks name, `ControlHandle`, `ControlOps`, `Roster`, `TypeAhead`, the collections' contexts and the slider's), `Typography`, `SystemColors` in pixels, defined at the root as CSS's system colors, `media`, `Resource` across the server boundary, `Deferred`, `AsyncView`, `head`, `view_transitions`, `launch`/`render_to_string`/`render_main` and `#[main]`; `Platform`, the build's platform as a const (`CURRENT`, `IS_SERVER`, `IS_APPLE`); and `platform`, a module per pixel platform's own vocabulary: `platform::web`, the web `Realization`: a DOM sink in the browser, a string tree on the server, one compiler, the content-addressed interner, hydration, streaming, the server executor, client hints, the native door `NativeView` (an element built from its `NativeMarkup`, served as a placeholder and adopted in place at hydration), and `skene-dev`'s served `Document`, re-exported; and `android`, `apple`, `ios`, `linux`, `macos` and `windows`, empty today. |
| `skene-terminal` | `packages/terminal` | The terminal facade and platform: `i32` cells, the same views and built-in controls drawn in cells, the same `behavior` module, its own decoration on them, `Typography`, `SystemColors` in cells, defined at the root from the colors the terminal reports, `BorderGlyphs`, `media`, `launch`, `Platform` (`CURRENT`), `platform::terminal` (`Terminal`, the `CellPainter` escape trait and its `Cells`, and the seams `install`, `mount` and `uninstall` an in-process test drives), and `testing`: `render_to_text`, the `trigger_*` seams, `user` and the queries by role, name and text. |
| `skene-conformance` | `packages/conformance` | One suite of behaviors every platform shows alike, run through the `Driver` a platform implements over its test seams, on the pages each facade writes in its own vocabulary under the names `PAGES` lists. |
| `skene-conformance-web` | `packages/conformance-web` | The conformance suite in real browsers: its pages, written in the web's vocabulary, served as one web app, acted on over WebDriver by trusted input. |
| `skene-simple` | `packages/simple` | The Simple core, naming no platform: `SimplePalette` (a `TokenSet`), `SimpleLook`, `SimpleVariant`, and the sizes per unit, `SimpleShape` through `SimpleUnit`. |
| `skene-simple-terminal` | `packages/simple-terminal` | The Simple presets of the terminal's built-in views, `SimpleButton`, `SimpleCard`, `SimpleTooltip`, `SimpleCheckbox`, `SimpleRadioGroup`/`SimpleRadio`, `SimplePicker`/`SimpleListboxOption`, `SimpleSlider`, `SimpleTextField`, `SimpleTextEditor` and `SimpleContextMenu`, each returning the built-in it styles; `SimpleGlyphs` (the token set its marks are drawn in) and its `glyph` tokens; the palette in cells, and a `prelude` an app imports beside the facade's own. |
| `skene-simple-gui` | `packages/simple-gui` | The Simple presets of the web's built-in views, as on the terminal, their marks drawn rather than glyphs, the palette in pixels, and its `prelude`. |
| `skene-macros` | `packages/macros` | `#[main]`, `#[lazy]`, `#[derive(Store)]`, `#[derive(Snapshot)]`, and `#[derive(View)]` and `#[derive(Wraps)]` for a view over another. |
| `skene-transport` | `packages/transport` | The clients on the public seams alone: `http` (`Request`, `Response`, `send`; `fetch` on the web, a thread around `ureq` on every other host) and `ws` (`connect`, `send`, `receive`; the browser's `WebSocket` on the web, a thread around `tungstenite` elsewhere). |
| `skene-dev` | `packages/dev` | The dev loop's protocol between the `skene` command and the app it runs, and the `Document` a web page is served as, the server leaf's pages and the command's client-rendered shell alike. |
| `skene-cli` | `packages/cli` | `skene new app <name> <platform>...`, `skene new lib <name> <platform>...`, `skene build web [--static]`, `skene build terminal`, `skene run web [--ssr]`, `skene run terminal` and `skene attach <id>`: bundling, the dev server with live reload and state kept across it in the page's own storage, the terminal build loop and the launcher that swaps each binary in place with the state carried across, static generation, `skene.toml` assets, and the `public` directory served at the site's root; a new project's `.cargo/config.toml` names its first platform for plain cargo, and the command builds every other by substituting that platform's cfg where cargo reads it; it depends on no facade, writing its shell through `skene-dev`'s `Document`. |

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
exactly one child. A control is `Control<B: Behavior>`, the shipped behaviors in each
facade's `behavior` module; `BasicButton`, `BasicCheckbox`, `BasicSlider`,
`BasicTextField` and `BasicTextEditor` are aliases over them, `BasicRadioGroup<T>`,
`BasicListbox<C>` and `BasicPicker<T>` are views typed by what they choose, a value of
the app's own type. `Button`, `Checkbox`, `Toggle` (a switch), `RadioGroup<T>` with
its `Radio`s, `Slider`, `TextField`, `TextEditor`, `Picker<T>` and `Listbox<C>` with
their `ListboxOption`s are the facade's own drawn controls in the platform's colors,
every default a value their setters replace, `Button::set_prominent` marking a screen's
primary action; on the web the field, the editor and the picker before its page is live
are the page's own form elements, styled by CSS. The `Simple*` components dress the
controls, a Simple button being a preset that returns the `Button` it styles, a
constructor per variant:

```rust,ignore
SimpleButton::new("Save").handle_activate(save),
SimpleButton::destructive("Delete").handle_activate(delete),
```

A Simple preset is inked in its palette's default with no theme above it.
`Theme::new(palette)` redefines a token set beneath it and
`Theme::derive(|| palette.get())` follows a signal, redefining the tokens in place so
nothing rebuilds; `SimplePalette` is one such set, and a third party's palette is on
identical footing, its references carrying its default as Simple's do. Light and dark
are one condition, `Media::Dark`, answered per token; `media::set_scheme` puts a scheme
in force for the whole document.

## Build & run

```sh
rustup target add wasm32-unknown-unknown

skene new app counter web terminal                         # a shared crate and a shell per platform
skene new lib acme-ui web terminal                         # a library's core and a crate per platform
cargo skene --dir examples/gui/counter run web             # build, serve on :8080, live-reload
cargo skene --dir examples/gui/router build web --static   # statically generate the routed site
cargo skene --dir examples/terminal/counter run terminal   # build, rebuild on change, print the attach command
cargo skene attach <id>                                    # run the app here, swapped on each rebuild
```

Examples live under `examples/gui` and `examples/terminal`, one folder per feature, the same
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
its library built as a binary for the host, the build naming the web and its server leaf
beside it. `RUSTFLAGS` stands above every rustflags a configuration sets, the
`.cargo/config.toml` `skene new` writes among them, and beside `--target` it reaches no
build script or proc macro, so the same command builds the server in an app.

```sh
RUSTFLAGS='--cfg skene_platform="web" --cfg skene_server' \
  cargo rustc -p counter --lib --crate-type bin --target host-tuple
target/x86_64-unknown-linux-gnu/debug/counter /   # prints the page, on an x86-64 Linux host
```

## Testing

```sh
cargo xtask gate                 # every check a change passes
cargo xtask gate test:browser    # one step; a group such as `test` or `clippy` runs each of its steps
```

The gate checks formatting, manifest order, every patched crate vendored under
`vendor/`, the house rules, unused dependencies, clippy and rustdoc on every leaf,
each facade refusing with its own message a build that names no platform and
`skene-gui` one it does not realize, clippy on the terminal's macOS paths, manifests
free of cargo features, the minimum Rust version in the host's, the server's, the
terminal's and the browser's builds, and four test tiers. A crate reaching a facade is
built for that facade's platform alone, and the host tier runs the tests of every crate
reaching neither, which name no platform and which plain cargo at the root builds: the
foundation's and the layout engine's unit tests among them. The terminal tier builds
every crate reaching `skene-terminal` in the terminal's build and runs their tests and
doctests, the terminal's in-process tests, which act through real input and read what a
reader finds, and the guide's and this README's terminal examples among them. The server
tier builds every crate reaching the web on the server leaf natively and runs their
tests and doctests, the guide's and this README's web examples, a byte-exact render
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

The repository names no default platform: the guide's
[contributing chapter](guide/src/contributing.md) has the cargo alias each platform's
build takes at the root, the editor settings rust-analyzer reads and the gate's workflow.

## Extending

A component library is a core and a crate per platform, as Simple is: the core depends on
`skene-foundation` and holds the colors, the named sizes and the logic both media share,
naming no realization, and each platform's crate writes presets returning its facade's
built-ins styled from the core and, where a preset is not enough, concrete views of its
own, naming no realization either, since the facade names what the foundation's surface
asks for (`Scene`, `IntoChildOf<Kind>`, `DecoratedTextEntry`, `Condition`);
`skene new lib` writes that shape. A low-level view goes
through its facade's `platform::<p>` module, the web's native door
(`skene_gui::platform::web::NativeView`, a DOM element served as its markup and adopted in
place) or the terminal's escape trait (`skene_terminal::platform::terminal::CellPainter`),
takes the `Realization` seam from `skene-foundation` where it needs it, since a facade
re-exports none, and is realized through the same scene as a built-in, a concrete view of
its facade's `Active` (`acme_map::Map`, `acme_sparkline::Sparkline`); `Painted` is the
portable one for pixels, a vello scene the platform renders, the web on the CPU or, with
`skene-gui`'s `gpu` feature, on the GPU where the page has WebGPU, and the terminal
through the kitty graphics protocol where it proves one. `examples/third-party` shows
both paths: `acme-map` (a canvas through the web's door, its markup served as the
placeholder the browser adopts and paints) and `acme-sparkline` (bars painted into the terminal's cells, and as pixels over them
where the terminal proves its graphics), each used by its platform's gallery and tested
on its platform.

## License

MIT; see the `LICENSE` file.
