//! The `skene` command: build, statically generate, and develop skene applications.
//!
//! Reads `skene.toml`—the framework manifest—beside the app's `Cargo.toml`; every
//! setting is optional. `skene dev` is the dev loop: build, serve, watch, rebuild, and
//! reload the page over SSE.

#[cfg(not(target_arch = "wasm32"))]
mod manifest;
#[cfg(not(target_arch = "wasm32"))]
mod pipeline;
#[cfg(not(target_arch = "wasm32"))]
mod server;

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
  use clap::Parser;

  let cli = host::Cli::parse();

  match host::run(cli) {
    Ok(()) => std::process::ExitCode::SUCCESS,
    Err(message) => {
      eprintln!("error: {message}");

      std::process::ExitCode::FAILURE
    }
  }
}

#[cfg(not(target_arch = "wasm32"))]
mod host {
  use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
  };

  use clap::{Parser, Subcommand};

  use crate::{
    manifest::App,
    pipeline,
    server::{Reload, serve},
  };

  /// Build, statically generate, and develop skene applications.
  #[derive(Parser)]
  #[command(name = "skene", about, long_about = None)]
  pub struct Cli {
    /// The app directory (holding `Cargo.toml` and, optionally, `skene.toml`).
    #[arg(long, default_value = ".", global = true)]
    dir: PathBuf,
    #[command(subcommand)]
    command: Command,
  }

  #[derive(Subcommand)]
  enum Command {
    /// Build the browser wasm and its bindings into `<app>/dist`.
    Build {
      /// Build without optimizations instead of `--release`.
      #[arg(long)]
      debug: bool,
    },
    /// Statically generate routes through the embedded runner, assets alongside.
    Ssg {
      /// Routes to render.
      #[arg(long, default_values_t = [String::from("/")])]
      path: Vec<String>,
      /// Output directory, relative to the app directory.
      #[arg(long, default_value = "ssg")]
      out: PathBuf,
      /// Build without optimizations instead of `--release`.
      #[arg(long)]
      debug: bool,
    },
    /// Build, serve, and live-reload on change—the dev loop.
    Dev {
      /// Port to serve on.
      #[arg(long, default_value_t = 8080)]
      port: u16,
      /// Build with optimizations (dev builds are unoptimized by default).
      #[arg(long)]
      release: bool,
    },
  }

  pub fn run(cli: Cli) -> Result<(), String> {
    let app = App::load(&cli.dir)?;

    match cli.command {
      Command::Build { debug } => pipeline::build_web(&app, !debug),

      Command::Ssg { path, out, debug } => pipeline::ssg(&app, &path, &app.dir.join(out), !debug),

      Command::Dev { port, release } => dev(app, port, release),
    }
  }

  /// The dev loop: build once, then serve while watching for changes; each successful
  /// rebuild tells every open page to reload.
  fn dev(app: App, port: u16, release: bool) -> Result<(), String> {
    pipeline::build_web(&app, release)?;

    let app = Arc::new(app);
    let reload = Reload::default();

    watch(Arc::clone(&app), release, reload.clone())?;

    serve(&app, port, Some(&reload))
  }

  /// Watches the workspace's packages, the app, and the asset roots; debounces bursts
  /// of events, rebuilds, and triggers a reload on success.
  fn watch(app: Arc<App>, release: bool, reload: Reload) -> Result<(), String> {
    use notify::{RecursiveMode, Watcher};

    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event: Result<notify::Event, _>| {
      if let Ok(event) = event
        && matches!(
          event.kind,
          notify::EventKind::Create(_)
            | notify::EventKind::Modify(_)
            | notify::EventKind::Remove(_)
        )
      {
        let _ = sender.send(());
      }
    })
    .map_err(|error| error.to_string())?;

    let mut roots = vec![
      app.workspace_root.join("packages"),
      app.dir.join("src"),
      app.dir.join("index.html"),
      app.dir.join("skene.toml"),
    ];

    roots.extend(app.asset_dirs());

    for root in roots {
      if root.exists() {
        watcher
          .watch(&root, RecursiveMode::Recursive)
          .map_err(|error| format!("watch {}: {error}", root.display()))?;
      }
    }

    thread::spawn(move || {
      // Keep the watcher alive for the whole loop.
      let _watcher = watcher;

      while receiver.recv().is_ok() {
        // Debounce the burst a save produces.
        while receiver.recv_timeout(Duration::from_millis(200)).is_ok() {}

        println!("change detected; rebuilding…");

        match pipeline::build_web(&app, release) {
          Ok(()) => {
            reload.trigger();
            println!("rebuilt; reloading");
          }
          Err(error) => eprintln!("rebuild failed: {error}"),
        }
      }
    });

    Ok(())
  }
}
