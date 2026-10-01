//! The build steps: cargo to wasm, wasm-bindgen to bindings, the server binary to pages.

use std::{
  fs,
  io::{BufReader, Read as _},
  path::{Path, PathBuf},
  process::{Command, Stdio},
  sync::OnceLock,
  thread,
};

use cargo_metadata::{Artifact, Message};

use crate::manifest::App;

/// The message format a build reports its artifacts in, its diagnostics rendered.
const MESSAGE_FORMAT: &str = "--message-format=json-render-diagnostics";

/// Builds the app's browser wasm, generates its bindings, writes the boot page and
/// copies the assets—all into `<app>/dist/web`, replacing the folder wholesale, which
/// deploys as a site's root. A dev build's page joins the dev loop.
pub(crate) fn build_web(app: &App, release: bool, dev: bool) -> Result<(), String> {
  let wasm = build_wasm(app, release)?;

  clear_web_out(app);
  generate_bindings(app, &wasm)?;
  crate::split::split_web(app, &wasm)?;
  write_page(app, dev)?;
  app.copy_assets(&app.dir.join("dist/web"))
}

/// Deletes `<app>/dist/web`. Every web build—CSR or static—replaces the folder
/// wholesale, after its compile succeeds, so stale artifacts never linger and a
/// failed rebuild leaves the served output standing.
fn clear_web_out(app: &App) {
  let _ = fs::remove_dir_all(app.dir.join("dist/web"));
}

/// Writes the generated CSR boot page: the web platform's document around an empty
/// body, titled after the package, booting the bundle and, in `dev`, joining the dev
/// loop.
fn write_page(app: &App, dev: bool) -> Result<(), String> {
  let document = skene_web::platform::Document {
    bundle: Some(app.stem.clone()),
    dev,
    ..skene_web::platform::Document::default()
  };
  let mut title = String::from("<title>");

  title.push_str(&app.package);
  title.push_str("</title>");

  let page = document.opening(&title, "") + &document.closing(None);

  fs::write(app.dir.join("dist/web/index.html"), page).map_err(|error| error.to_string())
}

/// The host's target triple, as rustc reports it.
fn host_triple() -> Result<&'static str, String> {
  static HOST: OnceLock<Result<String, String>> = OnceLock::new();

  HOST
    .get_or_init(|| {
      let output = Command::new("rustc")
        .arg("-vV")
        .output()
        .map_err(|error| format!("failed to run rustc -vV: {error}"))?;

      String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| "rustc -vV names no host".to_owned())
    })
    .as_deref()
    .map_err(Clone::clone)
}

/// Runs the cargo build `command`, which asks for [`MESSAGE_FORMAT`], and returns the
/// artifacts it reports; its diagnostics go to the terminal, or, `quiet`, into the
/// error it fails with.
fn cargo_artifacts(command: &mut Command, quiet: bool) -> Result<Vec<Artifact>, String> {
  command.stdout(Stdio::piped());

  if quiet {
    command.stderr(Stdio::piped());
  }

  let mut child = command
    .spawn()
    .map_err(|error| format!("failed to run cargo: {error}"))?;
  let diagnostics = child.stderr.take().map(|mut stderr| {
    thread::spawn(move || {
      let mut text = String::new();
      let _ = stderr.read_to_string(&mut text);

      text
    })
  });
  let stdout = child.stdout.take().ok_or("cargo has no stdout")?;
  let artifacts = Message::parse_stream(BufReader::new(stdout))
    .filter_map(|message| match message {
      Ok(Message::CompilerArtifact(artifact)) => Some(artifact),
      _ => None,
    })
    .collect();
  let status = child
    .wait()
    .map_err(|error| format!("failed to wait for cargo: {error}"))?;
  let diagnostics = diagnostics
    .and_then(|reader| reader.join().ok())
    .unwrap_or_default();

  if status.success() {
    Ok(artifacts)
  }
  else if quiet {
    Err(diagnostics)
  }
  else {
    Err(format!("cargo exited with {status}"))
  }
}

/// The binary the app's library built as among `artifacts`.
fn binary(app: &App, artifacts: &[Artifact]) -> Result<PathBuf, String> {
  artifacts
    .iter()
    .filter(|artifact| artifact.target.name == app.stem)
    .find_map(|artifact| {
      artifact
        .executable
        .clone()
        .or_else(|| artifact.filenames.first().cloned())
    })
    .map(cargo_metadata::camino::Utf8PathBuf::into_std_path_buf)
    .ok_or_else(|| format!("cargo built no binary for {}", app.package))
}

/// A command building the app's library as the crate type `crate_type` for `target`.
fn build_library(app: &App, crate_type: &str, target: &str) -> Command {
  let mut command = Command::new("cargo");

  command.current_dir(&app.workspace_root).args([
    "rustc",
    "--lib",
    "--crate-type",
    crate_type,
    "--target",
    target,
    "-p",
    &app.package,
    MESSAGE_FORMAT,
  ]);

  command
}

/// Builds the app's terminal binary for the host, quietly, and returns its path; on
/// failure returns cargo's diagnostics.
pub(crate) fn build_terminal_binary(app: &App, release: bool) -> Result<PathBuf, String> {
  let mut command = build_library(app, "bin", host_triple()?);

  if release {
    command.arg("--release");
  }

  let artifacts = cargo_artifacts(&mut command, true)?;

  binary(app, &artifacts)
}

/// Builds the app's terminal binary and places it in `<app>/dist/terminal`, named after
/// the package.
pub(crate) fn build_terminal(app: &App, release: bool) -> Result<(), String> {
  let binary = build_terminal_binary(app, release)?;
  let out_dir = app.dir.join("dist/terminal");

  fs::create_dir_all(&out_dir).map_err(|error| error.to_string())?;

  let out = out_dir
    .join(&app.package)
    .with_extension(std::env::consts::EXE_EXTENSION);

  fs::copy(&binary, &out).map_err(|error| format!("{}: {error}", binary.display()))?;
  println!("built {}", out.display());

  Ok(())
}

/// Builds the app's browser wasm and returns its path.
fn build_wasm(app: &App, release: bool) -> Result<PathBuf, String> {
  let mut command = build_library(app, "cdylib", "wasm32-unknown-unknown");

  // A split build keeps what the split tool reads—relocations, symbol names, unfused
  // functions—with the extra flags scoped to the app's own compile.
  if app.split && release {
    command.args([
      "--config",
      "profile.release.lto=false",
      "--config",
      "profile.release.strip=false",
      "--config",
      "profile.release.debug=1",
    ]);
  }

  if release {
    command.arg("--release");
  }

  if app.split {
    command.args(["--", "-Clink-arg=--emit-relocs"]);
  }

  let artifacts = cargo_artifacts(&mut command, false)?;

  artifacts
    .iter()
    .filter(|artifact| artifact.target.name == app.stem)
    .flat_map(|artifact| &artifact.filenames)
    .find(|file| file.extension() == Some("wasm"))
    .map(|file| file.clone().into_std_path_buf())
    .ok_or_else(|| format!("cargo built no wasm for {}", app.package))
}

/// Generates the bindings of the built `wasm` into `<app>/dist/web`.
fn generate_bindings(app: &App, wasm: &Path) -> Result<(), String> {
  wasm_bindgen_cli_support::Bindgen::new()
    .input_path(wasm)
    .omit_default_module_path(false)
    .web(true)
    .map_err(|error| format!("wasm-bindgen: {error:#}"))?
    .generate(app.dir.join("dist/web"))
    .map_err(|error| format!("wasm-bindgen: {error:#}"))
}

/// The flag naming the server platform.
const SERVER_CFG: &str = "--cfg skene_platform=\"server\"";

/// Names the server platform for what the build `command` compiles for `host` alone,
/// never its build scripts or proc macros: through the target's rustflags, or, where
/// `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS` supersedes those, joined to it.
fn name_server_platform(command: &mut Command, host: &str) {
  if let Ok(mut flags) = std::env::var("CARGO_ENCODED_RUSTFLAGS") {
    if !flags.is_empty() {
      flags.push('\x1f');
    }

    flags.push_str("--cfg\x1fskene_platform=\"server\"");
    command.env("CARGO_ENCODED_RUSTFLAGS", flags);

    return;
  }

  if let Ok(mut flags) = std::env::var("RUSTFLAGS") {
    if !flags.contains(SERVER_CFG) {
      if !flags.is_empty() {
        flags.push(' ');
      }

      flags.push_str(SERVER_CFG);
    }

    command.env("RUSTFLAGS", flags);

    return;
  }

  command.arg("--config").arg(format!(
    r#"target.{host}.rustflags=["--cfg", "skene_platform=\"server\""]"#
  ));
}

/// Builds the app's server binary for the host, with the server platform named, and
/// returns its path.
pub(crate) fn build_server(app: &App, release: bool) -> Result<PathBuf, String> {
  let host = host_triple()?;
  let mut command = build_library(app, "bin", host);

  name_server_platform(&mut command, host);

  if release {
    command.arg("--release");
  }

  let artifacts = cargo_artifacts(&mut command, false)?;

  binary(app, &artifacts)
}

/// Statically generates through the app's server binary into `<app>/dist/web`,
/// replacing the folder wholesale, assets alongside: the given routes, or—given
/// none—every route the app enumerates.
///
/// Each page carries its full content as markup and works without JavaScript; the
/// client bundle lands beside the pages and every page boots it, so a capable client
/// hydrates the markup into the running app. Paths are root-absolute: the output
/// expects `dist/web` served as the site root.
pub(crate) fn build_static(app: &App, routes: &[String], release: bool) -> Result<(), String> {
  let out = &app.dir.join("dist/web");
  let artifact = build_server(app, release)?;
  let wasm = build_wasm(app, release)?;

  clear_web_out(app);
  generate_bindings(app, &wasm)?;
  crate::split::split_web(app, &wasm)?;

  let mut renderer = crate::render::Renderer::load(artifact, app.dir.clone(), false)?;
  // No explicit routes: render every route the app enumerates (see
  // `install_static_router`), or the root alone.
  let routes = if routes.is_empty() {
    let enumerated = renderer.routes()?;

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
    let page = renderer
      .render(route, [])
      .map_err(|error| format!("render {route}: {error}"))?;
    let directory = out.join(route.trim_start_matches('/'));

    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;

    let file = directory.join("index.html");

    fs::write(&file, page).map_err(|error| error.to_string())?;
    println!("rendered {route} -> {}", file.display());
  }

  app.copy_assets(out)
}
