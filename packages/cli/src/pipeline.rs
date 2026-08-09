//! The build steps: cargo to wasm, wasm-bindgen to bindings, the runner to pages.

use std::{fs, path::Path, process::Command};

use crate::manifest::App;

/// The dev additions to the generated page: the marker the framework's boot handshake
/// looks for, and the script that beacons the state snapshot and reloads on the
/// server's say-so.
const DEV_PAGE_ADDITIONS: &str = "<meta name=\"skene-dev\"><script>(() => {\n\
  const save = () => {\n\
    if (window.skene_snapshot) navigator.sendBeacon(\"/skene:state\", window.skene_snapshot());\n\
  };\n\
  addEventListener(\"pagehide\", save);\n\
  new EventSource(\"/skene:reload\").onmessage = () => {\n\
    save();\n\
    location.reload();\n\
  };\n\
})();</script>";
/// Builds the app's browser wasm, generates its bindings, and writes the boot page—
/// all into `<app>/dist/web`, replacing the folder wholesale. A dev build's page
/// carries the dev marker and the reload script.
pub fn build_web(app: &App, release: bool, dev: bool) -> Result<(), String> {
  build_wasm(app, release)?;
  clear_web_out(app);
  generate_bindings(app, release)?;
  crate::split::split_web(app, release)?;
  write_page(app, dev)
}

/// Deletes `<app>/dist/web`. Every web build—CSR or static—replaces the folder
/// wholesale, after its compile succeeds, so stale artifacts never linger and a
/// failed rebuild leaves the served output standing.
fn clear_web_out(app: &App) {
  let _ = fs::remove_dir_all(app.dir.join("dist/web"));
}

/// Writes the generated CSR boot page: the module import of the generated bindings,
/// initialized on load.
fn write_page(app: &App, dev: bool) -> Result<(), String> {
  let additions = if dev { DEV_PAGE_ADDITIONS } else { "" };
  let page = format!(
    "<!DOCTYPE html>\n\
     <html>\n\
       <head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{package}</title>{additions}\n\
       </head>\n\
       <body>\n\
         <script type=\"module\">\n\
           import init from \"/dist/web/{stem}.js\";\n\n\
           init();\n\
         </script>\n\
       </body>\n\
     </html>\n",
    package = app.package,
    stem = app.stem,
  );

  fs::write(app.dir.join("dist/web/index.html"), page).map_err(|error| error.to_string())
}

/// Builds the app's terminal binary, quietly; on failure returns cargo's output.
pub fn build_tui_binary(app: &App, release: bool) -> Result<(), String> {
  let mut command = Command::new("cargo");

  command
    .current_dir(&app.workspace_root)
    .args(["build", "-p", &app.package]);

  if release {
    command.arg("--release");
  }

  let output = command
    .output()
    .map_err(|error| format!("failed to run cargo build: {error}"))?;

  if output.status.success() {
    Ok(())
  }
  else {
    Err(String::from_utf8_lossy(&output.stderr).into_owned())
  }
}

/// The built terminal binary's path.
pub fn tui_binary(app: &App, release: bool) -> std::path::PathBuf {
  let profile = if release { "release" } else { "debug" };

  app
    .workspace_root
    .join("target")
    .join(profile)
    .join(&app.package)
}

/// Builds the app's terminal binary and places it in `<app>/dist/tui`.
pub fn build_tui(app: &App, release: bool) -> Result<(), String> {
  build_tui_binary(app, release)?;

  let binary = tui_binary(app, release);
  let out_dir = app.dir.join("dist/tui");

  fs::create_dir_all(&out_dir).map_err(|error| error.to_string())?;

  let out = out_dir.join(&app.package);

  fs::copy(&binary, &out).map_err(|error| format!("{}: {error}", binary.display()))?;
  println!("built {}", out.display());

  Ok(())
}

fn build_wasm(app: &App, release: bool) -> Result<(), String> {
  let mut command = Command::new("cargo");

  command.current_dir(&app.workspace_root);

  // A split build keeps what the split tool reads—relocations, symbol names, unfused
  // functions—with the extra flags scoped to the app's own compile.
  if app.split {
    command.args([
      "rustc",
      "--lib",
      "--target",
      "wasm32-unknown-unknown",
      "-p",
      &app.package,
    ]);

    if release {
      command.args([
        "--config",
        "profile.release.lto=false",
        "--config",
        "profile.release.strip=false",
        "--config",
        "profile.release.debug=1",
        "--release",
      ]);
    }

    command.args(["--", "-Clink-arg=--emit-relocs"]);
  }
  else {
    command.args([
      "build",
      "--target",
      "wasm32-unknown-unknown",
      "-p",
      &app.package,
    ]);

    if release {
      command.arg("--release");
    }
  }

  run(&mut command, "cargo build")
}

/// The built browser wasm artifact's path.
pub fn wasm_artifact(app: &App, release: bool) -> std::path::PathBuf {
  let profile = if release { "release" } else { "debug" };

  app
    .workspace_root
    .join("target/wasm32-unknown-unknown")
    .join(profile)
    .join(format!("{}.wasm", app.stem))
}

fn generate_bindings(app: &App, release: bool) -> Result<(), String> {
  warn_on_version_mismatch(&app.workspace_root);

  let wasm = wasm_artifact(app, release);
  let out_dir = app.dir.join("dist/web");
  let mut command = Command::new("wasm-bindgen");

  command
    .arg(&wasm)
    .arg("--out-dir")
    .arg(&out_dir)
    .args(["--target", "web"]);

  run(&mut command, "wasm-bindgen").map_err(|error| {
    format!("{error}\nis `wasm-bindgen-cli` installed? `cargo install wasm-bindgen-cli`")
  })
}

/// Builds the app's server binary for `wasm32-wasip2` and returns the artifact path.
pub fn build_server(app: &App, release: bool) -> Result<std::path::PathBuf, String> {
  let server_bin = app.server_bin.as_deref().ok_or(
    "skene.toml declares no [app] server-bin; `build web --static` renders through the \
     server binary",
  )?;
  let mut command = Command::new("cargo");

  command.current_dir(&app.workspace_root).args([
    "build",
    "--target",
    "wasm32-wasip2",
    "-p",
    &app.package,
  ]);

  if release {
    command.arg("--release");
  }

  run(&mut command, "cargo build")?;

  let profile = if release { "release" } else { "debug" };

  Ok(
    app
      .workspace_root
      .join("target/wasm32-wasip2")
      .join(profile)
      .join(format!("{server_bin}.wasm")),
  )
}

/// Statically generates through the embedded runner into `<app>/dist/web`, replacing
/// the folder wholesale, assets alongside: the given routes, or—given none—every
/// route the app enumerates.
pub fn build_static(app: &App, routes: &[String], release: bool) -> Result<(), String> {
  let out = &app.dir.join("dist/web");
  let artifact = build_server(app, release)?;

  clear_web_out(app);
  let runner = skene_runner::Runner::load(&artifact).map_err(|error| format!("{error:#}"))?;
  // No explicit routes: render every route the app enumerates (see
  // `install_static_router`), or the root alone.
  let routes = if routes.is_empty() {
    let enumerated = runner.routes().map_err(|error| format!("{error:#}"))?;

    if enumerated.is_empty() {
      vec!["/".to_owned()]
    }
    else {
      enumerated
    }
  }
  else {
    routes.to_vec()
  };

  for route in &routes {
    let page = runner
      .render(route)
      .map_err(|error| format!("render {route}: {error:#}"))?;
    let directory = out.join(route.trim_start_matches('/'));

    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;

    let file = directory.join("index.html");

    fs::write(&file, page).map_err(|error| error.to_string())?;
    println!("rendered {route} -> {}", file.display());
  }

  app.copy_assets(out)
}

fn run(command: &mut Command, label: &str) -> Result<(), String> {
  let status = command
    .status()
    .map_err(|error| format!("failed to run {label}: {error}"))?;

  if status.success() {
    Ok(())
  }
  else {
    Err(format!("{label} exited with {status}"))
  }
}

/// Warns when the installed `wasm-bindgen` CLI disagrees with the locked crate, the
/// usual cause of a failed bindings step.
fn warn_on_version_mismatch(root: &Path) {
  let Some(crate_version) = locked_wasm_bindgen_version(root)
  else {
    return;
  };

  let Some(cli_version) = Command::new("wasm-bindgen")
    .arg("--version")
    .output()
    .ok()
    .filter(|output| output.status.success())
    .and_then(|output| String::from_utf8(output.stdout).ok())
    .and_then(|text| text.split_whitespace().nth(1).map(str::to_string))
  else {
    return;
  };

  if cli_version != crate_version {
    eprintln!(
      "warning: wasm-bindgen CLI {cli_version} != crate {crate_version}; \
       run `cargo install -f wasm-bindgen-cli --version {crate_version}` if bindings fail"
    );
  }
}

fn locked_wasm_bindgen_version(root: &Path) -> Option<String> {
  let lock = fs::read_to_string(root.join("Cargo.lock")).ok()?;
  let mut lines = lock.lines();

  while let Some(line) = lines.next() {
    if line == "name = \"wasm-bindgen\"" {
      let version = lines.next()?;

      return version
        .strip_prefix("version = \"")
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_string);
    }
  }

  None
}
