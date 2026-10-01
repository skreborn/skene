//! The command line: its arguments, and what each command runs.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::{
  attach::attach,
  dev::{dev, run_terminal},
  manifest::App,
  pipeline,
  scaffold::{Kind, Platform, Source},
};

/// Build, statically generate, and develop skene applications.
#[derive(Parser)]
#[command(name = "skene", about, long_about = None)]
pub(crate) struct Cli {
  /// The app directory (holding `Cargo.toml` and, optionally, `skene.toml`).
  #[arg(long, default_value = ".", global = true)]
  dir: PathBuf,
  /// The command to run.
  #[command(subcommand)]
  command: Command,
}

#[derive(Subcommand)]
enum Command {
  /// Build the app for a target, into `<app>/dist/<target>`.
  #[command(subcommand)]
  Build(BuildTarget),
  /// Run the app on a target: the web dev loop, or the terminal build loop.
  #[command(subcommand)]
  Run(RunTarget),
  /// Attach the terminal app a `run terminal` loop builds, in this terminal: runs the
  /// binary, and swaps each rebuilt one in with the state carried across.
  Attach {
    /// The loop's id, as `run terminal` prints it.
    id: u32,
  },
  /// Create a project of a kind, in a new directory under the app directory.
  #[command(subcommand)]
  New(NewKind),
}

#[derive(Subcommand)]
enum NewKind {
  /// An app: a crate of logic and state the shells share, and a shell per platform.
  App {
    /// The directory to create; its last component names the app.
    path: PathBuf,
    /// The platforms to give the app a shell on.
    #[arg(required = true, value_enum)]
    platforms: Vec<Platform>,
    /// Take the skene crates from the checkout at this path instead of crates.io.
    #[arg(long)]
    skene: Option<PathBuf>,
  },
  /// A component library: a core of colors, sizes and views, and a crate per platform
  /// exposing the views as its own.
  Lib {
    /// The directory to create; its last component names the library.
    path: PathBuf,
    /// The platforms to give the library a crate on.
    #[arg(required = true, value_enum)]
    platforms: Vec<Platform>,
    /// Take the skene crates from the checkout at this path instead of crates.io.
    #[arg(long)]
    skene: Option<PathBuf>,
  },
}

#[derive(Subcommand)]
enum BuildTarget {
  /// The browser wasm and its bindings—or, with `--static`, the statically
  /// generated site—into `dist/web`, replacing the folder wholesale.
  Web {
    /// Statically generate routes through the app's server binary, assets alongside.
    #[arg(long)]
    r#static: bool,
    /// Routes to render (with `--static`); every enumerated route when omitted.
    #[arg(long)]
    path: Vec<String>,
    /// Build without optimizations instead of `--release`.
    #[arg(long)]
    debug: bool,
  },
  /// The terminal binary, into `dist/terminal`.
  Terminal {
    /// Build without optimizations instead of `--release`.
    #[arg(long)]
    debug: bool,
  },
}

#[derive(Subcommand)]
enum RunTarget {
  /// The dev loop: build, serve, and live-reload on change.
  Web {
    /// Port to serve on.
    #[arg(long, default_value_t = 8080)]
    port: u16,
    /// Build with optimizations (dev builds are unoptimized by default).
    #[arg(long)]
    release: bool,
    /// Render each route per request through the server binary, forwarding the
    /// request's client hints, instead of serving the generated page.
    #[arg(long)]
    ssr: bool,
  },
  /// The terminal build loop: build, rebuild on change, and hand each binary to
  /// the terminal `attach` runs it in—state carried across the swap.
  Terminal {
    /// Build with optimizations (dev builds are unoptimized by default).
    #[arg(long)]
    release: bool,
  },
}

impl Cli {
  /// Runs the command.
  pub(crate) fn run(self) -> Result<(), String> {
    if let Command::Attach { id } = self.command {
      return attach(id);
    }

    if let Command::New(new) = self.command {
      let (kind, path, platforms, skene) = match new {
        NewKind::App {
          path,
          platforms,
          skene,
        } => (Kind::App, path, platforms, skene),
        NewKind::Lib {
          path,
          platforms,
          skene,
        } => (Kind::Library, path, platforms, skene),
      };
      let source = match skene {
        Some(checkout) => Source::Checkout(
          checkout
            .canonicalize()
            .map_err(|error| format!("{}: {error}", checkout.display()))?,
        ),
        None => Source::Registry,
      };

      return kind.create(&self.dir.join(path), &platforms, &source);
    }

    let app = App::load(&self.dir)?;

    match self.command {
      Command::Build(BuildTarget::Web {
        r#static: false,
        path,
        debug,
      }) => {
        if !path.is_empty() {
          return Err("--path names routes to render, which only --static does".to_owned());
        }

        pipeline::build_web(&app, !debug, false)
      }

      Command::Build(BuildTarget::Web {
        r#static: true,
        path,
        debug,
      }) => pipeline::build_static(&app, &path, !debug),

      Command::Build(BuildTarget::Terminal { debug }) => pipeline::build_terminal(&app, !debug),

      Command::Run(RunTarget::Web { port, release, ssr }) => dev(app, port, release, ssr),

      Command::Run(RunTarget::Terminal { release }) => run_terminal(&app, release),

      Command::Attach { .. } | Command::New(_) => Ok(()),
    }
  }
}
