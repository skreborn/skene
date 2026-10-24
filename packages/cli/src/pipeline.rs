//! The build steps: cargo to wasm, wasm-bindgen to bindings, the server binary to pages.

use std::{
  fs,
  io::{BufReader, Read as _},
  path::{Path, PathBuf},
  process::{Command, Stdio},
  thread,
};

use cargo_metadata::{Artifact, Message};

use crate::{flags::Selection, manifest::App};

/// The message format a build reports its artifacts in, its diagnostics rendered.
const MESSAGE_FORMAT: &str = "--message-format=json-render-diagnostics";

/// Builds the app's browser wasm, generates its bindings, writes the boot page and
/// copies the assets—all into `<app>/dist/web`, replacing the folder wholesale once
/// every step has succeeded, which deploys as a site's root. A dev build's page joins
/// the dev loop.
pub(crate) fn build_web(app: &App, release: bool, dev: bool) -> Result<(), String> {
  let wasm = build_wasm(app, release)?;

  publish_web(app, &wasm.path, release, dev)
}

/// Rebuilds the dev loop's web output: wholesale where cargo rebuilt the wasm or `stale`
/// says the output missed the last build, and otherwise the assets alone, copied over
/// the output in place. Returns whether the bundle was rebuilt.
pub(crate) fn rebuild_web(app: &App, release: bool, stale: bool) -> Result<bool, String> {
  let wasm = build_wasm(app, release)?;

  if wasm.fresh && !stale {
    let out = app.dir.join("dist/web");

    app.copy_public(&out)?;
    app.copy_assets(&out)?;

    return Ok(false);
  }

  publish_web(app, &wasm.path, release, true)?;

  Ok(true)
}

/// Copies the public files, generates the bindings of `wasm`, splits or, in `release`,
/// optimizes it, writes the boot page and copies the assets into a staged output,
/// published as `<app>/dist/web` once every step has succeeded.
fn publish_web(app: &App, wasm: &Path, release: bool, dev: bool) -> Result<(), String> {
  let out = WebOut::stage(&app.dir.join("dist"))?;

  app.copy_public(out.path())?;
  generate_bindings(wasm, out.path())?;

  if !crate::split::split_web(app, wasm, out.path())? && release {
    optimize(app, out.path())?;
  }

  write_page(app, dev, out.path())?;
  app.copy_assets(out.path())?;
  out.publish()
}

/// A crate cargo built: its file, and whether cargo found it up to date.
pub(crate) struct Built {
  /// The built file.
  pub path: PathBuf,
  /// Whether cargo reused the file unchanged.
  pub fresh: bool,
}

/// A web build's output, written beside `dist/web` and moved into its place once
/// published, so a failed build leaves the served output standing; dropped unpublished,
/// it is removed.
struct WebOut {
  dist: PathBuf,
  staging: PathBuf,
  published: bool,
}

impl WebOut {
  /// An empty staging directory for the next output under `dist`.
  fn stage(dist: &Path) -> Result<Self, String> {
    let staging = dist.join("web.staging");
    let _ = fs::remove_dir_all(&staging);

    fs::create_dir_all(&staging).map_err(|error| format!("{}: {error}", staging.display()))?;

    Ok(Self {
      dist: dist.to_owned(),
      staging,
      published: false,
    })
  }

  /// Where the build writes.
  fn path(&self) -> &Path {
    &self.staging
  }

  /// Replaces `dist/web` with the staged output.
  fn publish(mut self) -> Result<(), String> {
    let web = self.dist.join("web");
    let replaced = self.dist.join("web.replaced");
    let _ = fs::remove_dir_all(&replaced);

    if web.exists() {
      fs::rename(&web, &replaced).map_err(|error| format!("{}: {error}", web.display()))?;
    }

    fs::rename(&self.staging, &web).map_err(|error| format!("{}: {error}", web.display()))?;
    self.published = true;

    let _ = fs::remove_dir_all(&replaced);

    Ok(())
  }
}

impl Drop for WebOut {
  fn drop(&mut self) {
    if !self.published {
      let _ = fs::remove_dir_all(&self.staging);
    }
  }
}

/// Writes the generated CSR boot page into `out`: the served document around an empty
/// body, titled after the package, booting the bundle and, in `dev`, joining the dev
/// loop.
fn write_page(app: &App, dev: bool, out: &Path) -> Result<(), String> {
  let document = skene_dev::Document {
    bundle: Some(app.stem.clone()),
    dev,
    ..skene_dev::Document::default()
  };
  let mut title = String::from("<title>");

  title.push_str(&app.package);
  title.push_str("</title>");

  let page = document.opening(&title, "") + &document.closing(None);

  fs::write(out.join("index.html"), page).map_err(|error| error.to_string())
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
fn binary(app: &App, artifacts: &[Artifact]) -> Result<Built, String> {
  artifacts
    .iter()
    .filter(|artifact| artifact.target.name == app.stem)
    .find_map(|artifact| {
      let path = artifact
        .executable
        .clone()
        .or_else(|| artifact.filenames.first().cloned())?;

      Some(Built {
        path: path.into_std_path_buf(),
        fresh: artifact.fresh,
      })
    })
    .ok_or_else(|| format!("cargo built no binary for {}", app.package))
}

/// A command building the app's library as the crate type `crate_type` for
/// `selection`'s platform, named as cargo in the workspace's root would name it.
fn build_library(app: &App, crate_type: &str, selection: Selection) -> Result<Command, String> {
  let mut command = Command::new("cargo");

  command.current_dir(&app.workspace_root).args([
    "rustc",
    "--lib",
    "--crate-type",
    crate_type,
    "-p",
    &app.package,
    MESSAGE_FORMAT,
  ]);
  selection.name(&mut command, &app.workspace_root)?;

  Ok(command)
}

/// Builds the app's terminal binary for the host, with the terminal named, quietly, and
/// returns its path; on failure returns cargo's diagnostics.
pub(crate) fn build_terminal_binary(app: &App, release: bool) -> Result<Built, String> {
  let mut command = build_library(app, "bin", Selection::Terminal)?;

  if release {
    command.arg("--release");
  }

  let artifacts = cargo_artifacts(&mut command, true)?;

  binary(app, &artifacts)
}

/// Builds the app's terminal binary and places it in `<app>/dist/terminal`, named after
/// the package.
pub(crate) fn build_terminal(app: &App, release: bool) -> Result<(), String> {
  let binary = build_terminal_binary(app, release)?.path;
  let out_dir = app.dir.join("dist/terminal");

  fs::create_dir_all(&out_dir).map_err(|error| error.to_string())?;

  let out = out_dir
    .join(&app.package)
    .with_extension(std::env::consts::EXE_EXTENSION);

  fs::copy(&binary, &out).map_err(|error| format!("{}: {error}", binary.display()))?;
  println!("built {}", out.display());

  Ok(())
}

/// Builds the app's browser wasm.
fn build_wasm(app: &App, release: bool) -> Result<Built, String> {
  let mut command = build_library(app, "cdylib", Selection::Web)?;

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
    .find_map(|artifact| {
      let file = artifact
        .filenames
        .iter()
        .find(|file| file.extension() == Some("wasm"))?;

      Some(Built {
        path: file.clone().into_std_path_buf(),
        fresh: artifact.fresh,
      })
    })
    .ok_or_else(|| format!("cargo built no wasm for {}", app.package))
}

/// Generates the bindings of the built `wasm` into `out`.
fn generate_bindings(wasm: &Path, out: &Path) -> Result<(), String> {
  wasm_bindgen_cli_support::Bindgen::new()
    .input_path(wasm)
    .omit_default_module_path(false)
    .web(true)
    .map_err(|error| format!("wasm-bindgen: {error:#}"))?
    .generate(out)
    .map_err(|error| format!("wasm-bindgen: {error:#}"))
}

/// The wasm features the optimizer may keep using: those rustc emits for
/// `wasm32-unknown-unknown`.
const WASM_FEATURES: [&str; 6] = [
  "--enable-bulk-memory",
  "--enable-multivalue",
  "--enable-mutable-globals",
  "--enable-nontrapping-float-to-int",
  "--enable-reference-types",
  "--enable-sign-ext",
];

/// Optimizes the bindgened wasm in `out` for size through binaryen's `wasm-opt`, where it
/// is installed, and leaves it as it is where it is not.
fn optimize(app: &App, out: &Path) -> Result<(), String> {
  let wasm = out.join(format!("{}_bg.wasm", app.stem));
  let status = Command::new("wasm-opt")
    .arg("-Oz")
    .args(WASM_FEATURES)
    .arg(&wasm)
    .arg("-o")
    .arg(&wasm)
    .status();

  match status {
    Ok(status) if status.success() => Ok(()),
    Ok(status) => Err(format!("wasm-opt exited with {status}")),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
      println!("wasm-opt is not installed; the bundle is not optimized");

      Ok(())
    }
    Err(error) => Err(format!("wasm-opt: {error}")),
  }
}

/// Builds the app's server binary for the host, with the web's server leaf named, and
/// returns its path.
pub(crate) fn build_server(app: &App, release: bool) -> Result<Built, String> {
  let mut command = build_library(app, "bin", Selection::Server)?;

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
  let artifact = build_server(app, release)?.path;
  let wasm = build_wasm(app, release)?.path;
  let staged = WebOut::stage(&app.dir.join("dist"))?;
  let out = staged.path();

  app.copy_public(out)?;
  generate_bindings(&wasm, out)?;

  if !crate::split::split_web(app, &wasm, out)? && release {
    optimize(app, out)?;
  }

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
    println!(
      "rendered {route} -> {}",
      app.dir.join(route_file(route)).display()
    );
  }

  app.copy_assets(out)?;
  staged.publish()
}

/// The file a route's page is published at, under `dist/web`.
fn route_file(route: &str) -> PathBuf {
  Path::new("dist/web")
    .join(route.trim_start_matches('/'))
    .join("index.html")
}

#[cfg(test)]
mod tests {
  use std::fs;

  use super::WebOut;

  /// A build dropped unpublished leaves the output standing and its staging gone, and a
  /// published one replaces the output whole.
  #[test]
  fn a_build_replaces_the_output_once_published() {
    let dist = std::env::temp_dir().join(format!("skene-web-out-{}", std::process::id()));
    let web = dist.join("web");
    let _ = fs::remove_dir_all(&dist);

    fs::create_dir_all(&web).unwrap();
    fs::write(web.join("stale.js"), "old").unwrap();

    let failed = WebOut::stage(&dist).unwrap();

    fs::write(failed.path().join("index.html"), "new").unwrap();
    drop(failed);

    assert!(web.join("stale.js").is_file());
    assert!(!dist.join("web.staging").exists());

    let built = WebOut::stage(&dist).unwrap();

    fs::write(built.path().join("index.html"), "new").unwrap();
    built.publish().unwrap();

    assert!(!web.join("stale.js").exists());
    assert_eq!(fs::read_to_string(web.join("index.html")).unwrap(), "new");

    fs::remove_dir_all(&dist).unwrap();
  }
}
