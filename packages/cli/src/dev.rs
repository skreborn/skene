//! The dev loops: the web's, serving the page and reloading it on each rebuild, and the
//! terminal's, rebuilding the binary an attached terminal swaps in.

use std::{path::PathBuf, sync::mpsc, thread, time::Duration};

use crate::{
  manifest::{App, LiveApp},
  pipeline,
  server::{Dev, serve, spawn_state_server},
};

/// The web dev loop: build once, then serve while watching for changes; each change
/// reads the app's `skene.toml` again, and each successful rebuild tells every open
/// page to reload. With `ssr` the server binary is built too and every route is
/// rendered per request through it.
pub(crate) fn dev(app: App, port: u16, release: bool, ssr: bool) -> Result<(), String> {
  pipeline::build_web(&app, release, true)?;

  let app = LiveApp::new(app);
  let dev = Dev::default();

  if ssr {
    dev.set_renderer(Some(load_renderer(&app.current(), release)?));
  }

  let mut sources = Sources::watch(&app.current())?;

  {
    let app = app.clone();
    let dev = dev.clone();

    thread::spawn(move || {
      while sources.next() {
        println!("change detected; rebuilding…");

        let rebuilt = app.reload().and_then(|current| {
          pipeline::build_web(&current, release, true)?;

          if ssr {
            dev.set_renderer(Some(load_renderer(&current, release)?));
          }

          Ok(())
        });

        sources.follow(&app.current());

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

/// Builds the app's server binary and starts it as a render worker whose pages join
/// the dev loop.
fn load_renderer(app: &App, release: bool) -> Result<crate::render::Renderer, String> {
  let artifact = pipeline::build_server(app, release)?;

  crate::render::Renderer::load(artifact, app.dir.clone(), true)
}

/// The terminal build loop: build, publish the binary for an attached terminal, and
/// on each change read the app's `skene.toml` again, rebuild and publish the next—the
/// CLI-held state carries across the swap. Runs until interrupted.
pub(crate) fn run_terminal(mut app: App, release: bool) -> Result<(), String> {
  let binary = pipeline::build_terminal_binary(&app, release)
    .map_err(|error| format!("build failed:\n{error}"))?;
  let dev = Dev::default();
  let id = spawn_state_server(dev.clone())?;
  let mut sources = Sources::watch(&app)?;
  let mut running = binary_digest(&binary);

  dev.set_binary(binary.clone(), app.dir.clone());
  println!(
    "built; attach the app in another terminal:\n  {} attach {id}",
    cli_command()
  );

  while sources.next() {
    println!("change detected; rebuilding…");

    let rebuilt = App::load(&app.dir).and_then(|current| {
      app = current;
      sources.follow(&app);

      pipeline::build_terminal_binary(&app, release)
    });

    // A rebuild the change did not reach—an unrelated file, a comment elsewhere—
    // leaves the binary identical, and the attached app (and its state) stays up
    // instead of restarting for nothing.
    match rebuilt {
      Ok(binary) => {
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

/// The sources a dev loop watches: the workspace's packages, the app, and its asset
/// roots, watched again when the roots change.
struct Sources {
  changes: mpsc::Receiver<()>,
  _watcher: notify::RecommendedWatcher,
  assets: Vec<PathBuf>,
}

impl Sources {
  /// Watches `app`'s sources.
  fn watch(app: &App) -> Result<Self, String> {
    use notify::{RecursiveMode, Watcher};

    let (sender, changes) = mpsc::channel();
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
    let assets = app.asset_dirs();
    let mut roots = vec![
      app.workspace_root.join("packages"),
      app.dir.join("src"),
      app.dir.join("skene.toml"),
    ];

    roots.extend(assets.iter().cloned());

    for root in roots {
      if root.exists() {
        watcher
          .watch(&root, RecursiveMode::Recursive)
          .map_err(|error| format!("watch {}: {error}", root.display()))?;
      }
    }

    Ok(Self {
      changes,
      _watcher: watcher,
      assets,
    })
  }

  /// Waits for a change and the rest of the burst one save produces; `false` once the
  /// watch has ended.
  fn next(&self) -> bool {
    if self.changes.recv().is_err() {
      return false;
    }

    while self
      .changes
      .recv_timeout(Duration::from_millis(200))
      .is_ok()
    {}

    true
  }

  /// Watches `app`'s sources afresh where its asset roots differ from those watched.
  fn follow(&mut self, app: &App) {
    if app.asset_dirs() == self.assets {
      return;
    }

    match Self::watch(app) {
      Ok(sources) => *self = sources,
      Err(error) => eprintln!("{error}"),
    }
  }
}
