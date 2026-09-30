//! The dev loops: the web's, serving the page and reloading it on each rebuild, and the
//! terminal's, rebuilding the binary an attached terminal swaps in.

use std::{
  sync::{Arc, mpsc},
  thread,
  time::Duration,
};

use crate::{
  manifest::App,
  pipeline,
  server::{Dev, serve, spawn_state_server},
};

/// The web dev loop: build once, then serve while watching for changes; each
/// successful rebuild tells every open page to reload. With `ssr` the server binary
/// is built too and every route is rendered per request through it.
pub(crate) fn dev(app: App, port: u16, release: bool, ssr: bool) -> Result<(), String> {
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

/// Builds the app's server binary and starts it as a render worker whose pages join
/// the dev loop.
fn load_renderer(app: &App, release: bool) -> Result<crate::render::Renderer, String> {
  let artifact = pipeline::build_server(app, release)?;

  crate::render::Renderer::load(artifact, app.dir.clone(), true)
}

/// The terminal build loop: build, publish the binary for an attached terminal, and
/// on each change rebuild and publish the next—the CLI-held state carries across
/// the swap. Runs until interrupted.
pub(crate) fn run_tui(app: &App, release: bool) -> Result<(), String> {
  let binary =
    pipeline::build_tui_binary(app, release).map_err(|error| format!("build failed:\n{error}"))?;
  let dev = Dev::default();
  let id = spawn_state_server(dev.clone())?;
  let (changes, _watcher) = watch_sources(app)?;
  let mut running = binary_digest(&binary);

  dev.set_binary(binary.clone(), app.dir.clone());
  println!(
    "built; attach the app in another terminal:\n  {} attach {id}",
    cli_command()
  );

  while changes.recv().is_ok() {
    drain_burst(&changes);
    println!("change detected; rebuilding…");

    // A rebuild the change did not reach—an unrelated file, a comment elsewhere—
    // leaves the binary identical, and the attached app (and its state) stays up
    // instead of restarting for nothing.
    match pipeline::build_tui_binary(app, release) {
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

/// Watches the workspace's packages, the app, and the asset roots; sends one unit
/// per change. The returned watcher must stay alive as long as the receiver.
fn watch_sources(app: &App) -> Result<(mpsc::Receiver<()>, notify::RecommendedWatcher), String> {
  use notify::{RecursiveMode, Watcher};

  let (sender, receiver) = mpsc::channel();
  let mut watcher = notify::recommended_watcher(move |event: Result<notify::Event, _>| {
    if let Ok(event) = event
      && matches!(
        event.kind,
        notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_)
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
