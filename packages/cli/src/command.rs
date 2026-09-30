//! The command line: its arguments, and what each command runs.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::{
  attach::attach,
  dev::{dev, run_tui},
  manifest::App,
  pipeline,
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
  /// Attach the terminal app a `run tui` loop builds, in this terminal: runs the
  /// binary, and swaps each rebuilt one in with the state carried across.
  Attach {
    /// The loop's id, as `run tui` prints it.
    id: u32,
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
  /// The terminal binary, into `dist/tui`.
  Tui {
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
  Tui {
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

      Command::Build(BuildTarget::Tui { debug }) => pipeline::build_tui(&app, !debug),

      Command::Run(RunTarget::Web { port, release, ssr }) => dev(app, port, release, ssr),

      Command::Run(RunTarget::Tui { release }) => run_tui(&app, release),

      Command::Attach { .. } => Ok(()),
    }
  }
}
