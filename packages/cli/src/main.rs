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
    server::{Binary, Dev, fetch, serve, spawn_state_server},
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
    /// Run the app on a target: the web dev loop, or the terminal build loop.
    #[command(subcommand)]
    Run(RunTarget),
    /// Attach the terminal app a `run tui` loop builds, in this terminal: runs the
    /// binary, and swaps each rebuilt one in with the state carried across.
    Attach {
      /// The loop's port, as `run tui` prints it.
      port: u16,
    },
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

  pub fn run(cli: Cli) -> Result<(), String> {
    if let Command::Attach { port } = cli.command {
      return attach(port);
    }

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

      Command::Run(RunTarget::Web { port, release, ssr }) => dev(app, port, release, ssr),

      Command::Run(RunTarget::Tui { release }) => run_tui(&app, release),

      Command::Attach { .. } => Ok(()),
    }
  }

  /// The web dev loop: build once, then serve while watching for changes; each
  /// successful rebuild tells every open page to reload. With `ssr` the server binary
  /// is built too and every route is rendered per request through it.
  fn dev(app: App, port: u16, release: bool, ssr: bool) -> Result<(), String> {
    pipeline::build_web(&app, release, true)?;

    let app = Arc::new(app);
    let dev = Dev::default();

    if ssr {
      dev.set_renderer(Some(load_renderer(&app, release)?));
    }

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

          let rebuilt = pipeline::build_web(&app, release, true).and_then(|()| {
            if ssr {
              dev.set_renderer(Some(load_renderer(&app, release)?));
            }

            Ok(())
          });

          match rebuilt {
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

  /// Builds the app's server binary and loads it into a runner.
  fn load_renderer(app: &App, release: bool) -> Result<skene_runner::Runner, String> {
    let artifact = pipeline::build_server(app, release)?;

    skene_runner::Runner::load(&artifact).map_err(|error| format!("{error:#}"))
  }

  /// The terminal build loop: build, publish the binary for an attached terminal, and
  /// on each change rebuild and publish the next—the CLI-held state carries across
  /// the swap. Runs until interrupted.
  fn run_tui(app: &App, release: bool) -> Result<(), String> {
    pipeline::build_tui_binary(app, release).map_err(|error| format!("build failed:\n{error}"))?;

    let dev = Dev::default();
    let port = spawn_state_server(dev.clone())?;
    let (changes, _watcher) = watch_sources(app)?;
    let binary = pipeline::tui_binary(app, release);
    let mut running = binary_digest(&binary);

    dev.set_binary(binary.clone(), app.dir.clone());
    println!(
      "built; attach the app in another terminal:\n  {} attach {port}",
      cli_command()
    );

    while changes.recv().is_ok() {
      drain_burst(&changes);
      println!("change detected; rebuilding…");

      // A rebuild the change did not reach—an unrelated file, a comment elsewhere—
      // leaves the binary identical, and the attached app (and its state) stays up
      // instead of restarting for nothing.
      match pipeline::build_tui_binary(app, release) {
        Ok(()) => {
          dev.clear_notice();

          let rebuilt = binary_digest(&binary);

          if rebuilt.is_some() && rebuilt == running {
            println!("rebuilt; nothing changed");

            continue;
          }

          running = rebuilt;
          dev.set_binary(binary.clone(), app.dir.clone());
          println!("rebuilt; swapping");
        }
        Err(error) => {
          // The attached app presents the notice in a modal as well.
          dev.set_notice(error.clone());
          eprintln!("rebuild failed:\n{error}");
        }
      }
    }

    Ok(())
  }

  /// Runs the binary a `run tui` loop on `port` publishes, in this terminal, with the
  /// state handshake; swaps in each generation the loop publishes, restoring the
  /// terminal between instances. Ends when the app itself exits.
  fn attach(port: u16) -> Result<(), String> {
    let published = || {
      fetch(port, "/skene:binary").and_then(|body| Binary::parse(&String::from_utf8_lossy(&body)))
    };
    let mut current = published().ok_or_else(|| {
      format!("no `run tui` loop answers on port {port}; start one and attach to its port")
    })?;
    // The terminal state before any app instance touches it. A swap kills the running
    // instance mid-raw-mode; restoring this before the next spawn keeps each
    // instance's own saved "original" sane, so the final quit restores a working
    // terminal.
    let sane_tty = crate::tty::Tty::capture();

    loop {
      let mut child = std::process::Command::new(&current.path)
        .current_dir(&current.dir)
        .env("SKENE_DEV", port.to_string())
        .spawn()
        .map_err(|error| format!("{}: {error}", current.path.display()))?;

      loop {
        if let Ok(Some(_)) = child.try_wait() {
          // The user quit the app; the attachment ends with it.
          return Ok(());
        }

        thread::sleep(Duration::from_millis(250));

        match published() {
          Some(next) if next.generation != current.generation => {
            current = next;

            let _ = child.kill();
            let _ = child.wait();

            if let Some(tty) = &sane_tty {
              tty.restore();
            }

            break;
          }
          Some(_) => {}
          None => {
            // The loop is gone; the app runs on until it quits.
            let _ = child.wait();

            return Ok(());
          }
        }
      }
    }
  }

  /// How this CLI is invoked, for a command printed to run: through the workspace's
  /// `cargo skene` alias when the binary sits in a cargo target directory.
  fn cli_command() -> &'static str {
    let under_target = std::env::current_exe().is_ok_and(|exe| {
      exe
        .components()
        .any(|component| component.as_os_str() == "target")
    });

    if under_target { "cargo skene" } else { "skene" }
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
