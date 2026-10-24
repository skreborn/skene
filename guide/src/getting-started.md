# Getting started

Install the `skene` command, and the target the web's browser build compiles for:

```sh
cargo install skene-cli
rustup target add wasm32-unknown-unknown
```

## A new app

`skene new app` creates an app with a shell for each platform named:

```sh
skene new app counter web terminal
```

```text
counter
├── .cargo         config.toml: the platform plain cargo builds, the web
├── Cargo.toml     the workspace
├── shared         counter-shared: the state and logic every shell shares
├── terminal       counter-terminal: the terminal's shell
└── web            counter-web: the web's shell
```

The shared crate depends on `skene-reactive` alone and knows nothing of any platform.
Each shell depends on the shared crate, on its platform's facade, and on that
platform's crate of Simple, the component library the template draws with. A shell's
entry is the function `#[main]` marks:

```rust
use skene_simple_web::prelude::*;
use skene_web::prelude::*;

#[main]
fn app() -> impl View {
  let count = Signal::tagged("count", 0u64);

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    SimpleCard::new().set_spacing(12).set_children((
      Text::derive(move || count.get().to_string()),
      SimpleButton::new("+").handle_activate(move || count.update(|count| *count += 1)),
    ))
  })
}
```

`#[main]` expands to the entry the build has: in the browser it hydrates the page the
server rendered, and on the server it renders the requested path. A build for neither
fails, naming the two.

## One platform per build

A build names its platform by a cfg, `--cfg skene_platform="web"` or
`--cfg skene_platform="terminal"`, the web's server leaf being the `web` build with
`--cfg skene_server` beside it. The first platform `skene new` is given is the one
plain cargo builds: the project's `.cargo/config.toml` names it, with its target as the
build's and its cfg in the target's rustflags and in rustdoc's.

```toml
[build]
target = "wasm32-unknown-unknown"
rustdocflags = ["--cfg", 'skene_platform="web"']

[target.'cfg(all())']
rustflags = ["--cfg", 'skene_platform="web"']

[target.wasm32-unknown-unknown]
runner = "wasm-bindgen-test-runner"
```

So `cargo check`, `cargo test`, `cargo clippy`, `cargo doc` and rust-analyzer work at
the project's root with no other setup, the browser's tests running through
wasm-bindgen's test runner, which `skene new` asks to be installed first. Each shell
is gated on its platform (`#![cfg(skene_platform = "web")]`), so in the terminal's build
the web's shell compiles empty, and the other way round. The `skene` command builds
every other platform by naming that platform's cfg where the configuration names this
one: `skene run terminal` in this project builds the terminal, and `skene run web`
shares what plain cargo compiled, its flags being the configuration's own.

## Running it

Each shell runs from its own directory:

```sh
cd counter/web
skene run web
```

`skene run web` builds the page, serves it on port 8080, and reloads it when a source
changes. `skene run terminal` builds the terminal binary, rebuilds it on every change,
and prints the command that runs it in another terminal:

```sh
cd counter/terminal
skene run terminal
skene attach <id>
```

A reload keeps the state the app opts in: `Signal::tagged` finds a signal's old value
by its tag, so one added before it shifts nothing, and `Signal::saved` by its place
among the saved and tagged signals of its owner. A plain `Signal::new` starts afresh.

## Building it

`skene build web` builds the page for release into `dist/web`, and
`skene build web --static` renders every route the app enumerates into static pages
beside it. `skene build terminal` builds the terminal binary into `dist/terminal`.