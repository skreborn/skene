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
#[cfg(not(target_arch = "wasm32"))]
mod split;
#[cfg(not(target_arch = "wasm32"))]
mod tty;

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
    server::{Dev, serve, spawn_state_server},
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
    /// Build the app for a target, into `<app>/dist/<target>`.
    #[command(subcommand)]
    Build(BuildTarget),
    /// Run the app on a target: the web dev loop, or the terminal binary.
    #[command(subcommand)]
    Run(RunTarget),
  }

  #[derive(Subcommand)]
  enum BuildTarget {
    /// The browser wasm and its bindings—or, with `--static`, the statically
    /// generated site—into `dist/web`, replacing the folder wholesale.
    Web {
      /// Statically generate routes through the embedded runner, assets alongside.
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
    },
    /// The terminal dev loop: build and run the binary, rebuild and restart on
    /// change—state carried across the restart.
    Tui {
      /// Build with optimizations (dev builds are unoptimized by default).
      #[arg(long)]
      release: bool,
    },
  }

  pub fn run(cli: Cli) -> Result<(), String> {
    let app = App::load(&cli.dir)?;

    match cli.command {
      Command::Build(BuildTarget::Web {
        r#static: false,
        debug,
        ..
      }) => pipeline::build_web(&app, !debug, false),

      Command::Build(BuildTarget::Web {
        r#static: true,
        path,
        debug,
      }) => pipeline::build_static(&app, &path, !debug),

      Command::Build(BuildTarget::Tui { debug }) => pipeline::build_tui(&app, !debug),

      Command::Run(RunTarget::Web { port, release }) => dev(app, port, release),

      Command::Run(RunTarget::Tui { release }) => run_tui(&app, release),
    }
  }

  /// The web dev loop: build once, then serve while watching for changes; each
  /// successful rebuild tells every open page to reload.
  fn dev(app: App, port: u16, release: bool) -> Result<(), String> {
    pipeline::build_web(&app, release, true)?;

    let app = Arc::new(app);
    let dev = Dev::default();
    let (changes, watcher) = watch_sources(&app)?;

    {
      let app = Arc::clone(&app);
      let dev = dev.clone();

      thread::spawn(move || {
        // Keep the watcher alive for the whole loop.
        let _watcher = watcher;

        while changes.recv().is_ok() {
          drain_burst(&changes);
          println!("change detected; rebuilding…");

          match pipeline::build_web(&app, release, true) {
            Ok(()) => {
              dev.trigger();
              println!("rebuilt; reloading");
            }
            Err(error) => eprintln!("rebuild failed: {error}"),
          }
        }
      });
    }

    serve(&app, port, Some(&dev))
  }

  /// The terminal dev loop: run the binary with the state handshake, and on each
  /// change rebuild first, then swap in the new binary—the CLI-held state carries
  /// across the swap. Ends when the app itself exits.
  fn run_tui(app: &App, release: bool) -> Result<(), String> {
    pipeline::build_tui_binary(app, release).map_err(|error| format!("build failed:\n{error}"))?;

    let dev = Dev::default();
    let port = spawn_state_server(dev.clone())?;
    let (changes, _watcher) = watch_sources(app)?;
    let binary = pipeline::tui_binary(app, release);
    let mut running = binary_digest(&binary);
    // The terminal state before any app instance touches it. A swap kills the running
    // instance mid-raw-mode; restoring this before the next spawn keeps each
    // instance's own saved "original" sane, so the final quit restores a working
    // terminal.
    let sane_tty = crate::tty::Tty::capture();

    loop {
      let mut child = std::process::Command::new(&binary)
        .current_dir(&app.dir)
        .env("SKENE_DEV", port.to_string())
        .spawn()
        .map_err(|error| format!("{}: {error}", binary.display()))?;

      loop {
        if let Ok(Some(_)) = child.try_wait() {
          // The user quit the app; the loop ends with it.
          return Ok(());
        }

        match changes.recv_timeout(Duration::from_millis(100)) {
          Ok(()) => {
            drain_burst(&changes);

            // Rebuild before swapping, so the running app stays up through the compile
            // (and through a failed one). A rebuild the change did not reach—an
            // unrelated file, a comment elsewhere—leaves the binary identical, and the
            // running app (and its state) stays up instead of restarting for nothing.
            match pipeline::build_tui_binary(app, release) {
              Ok(()) => {
                let rebuilt = binary_digest(&binary);

                if rebuilt.is_some() && rebuilt == running {
                  continue;
                }

                running = rebuilt;

                let _ = child.kill();
                let _ = child.wait();

                if let Some(tty) = &sane_tty {
                  tty.restore();
                }

                break;
              }
              Err(error) => announce(&format!("rebuild failed:\n{error}")),
            }
          }
          Err(mpsc::RecvTimeoutError::Timeout) => {}
          Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = child.kill();
            let _ = child.wait();

            return Ok(());
          }
        }
      }
    }
  }

  /// Prints into the terminal the running app holds in raw mode: every line break
  /// carries a carriage return, so the text reads straight instead of stair-stepping.
  fn announce(text: &str) {
    use std::io::Write;

    let mut out = std::io::stderr();

    for line in text.lines() {
      let _ = write!(out, "{line}\r\n");
    }

    let _ = out.flush();
  }

  /// The binary's content digest, to tell a rebuild that changed nothing from one
  /// that needs a swap.
  fn binary_digest(path: &std::path::Path) -> Option<u64> {
    use std::hash::{DefaultHasher, Hash, Hasher};

    let bytes = std::fs::read(path).ok()?;
    let mut hasher = DefaultHasher::new();

    bytes.hash(&mut hasher);

    Some(hasher.finish())
  }

  /// Watches the workspace's packages, the app, and the asset roots; sends one unit
  /// per change. The returned watcher must stay alive as long as the receiver.
  fn watch_sources(app: &App) -> Result<(mpsc::Receiver<()>, notify::RecommendedWatcher), String> {
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

    Ok((receiver, watcher))
  }

  /// Drains the burst of filesystem events one save produces.
  fn drain_burst(changes: &mpsc::Receiver<()>) {
    while changes.recv_timeout(Duration::from_millis(200)).is_ok() {}
  }
}
