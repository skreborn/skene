//! `skene.toml`: the framework manifest, and asset collection across the dependency
//! graph.
//!
//! Every setting is optional; an app with no manifest gets working defaults from its
//! `Cargo.toml`. Assets are declared as manifest-relative files or directories and
//! become reachable at `/<package>/<declared path>`—the providing package's name scopes
//! every URI, so the app's and its libraries' assets can never collide. Predictable,
//! never statically checked. A library crate's own `skene.toml` (found beside its
//! `Cargo.toml` through cargo metadata) contributes assets the same way.

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use serde::Deserialize;

/// The parsed `skene.toml` of one package.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ManifestFile {
  /// App-level settings.
  #[serde(default)]
  pub app: AppSection,
  /// Web build settings.
  #[serde(default)]
  pub web: WebSection,
  /// Asset declarations.
  #[serde(default)]
  pub assets: AssetsSection,
}

/// The `[web]` section.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct WebSection {
  /// Whether web builds split `#[lazy]` functions into lazily-fetched wasm modules.
  #[serde(default)]
  pub split: bool,
}

/// The `[app]` section.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct AppSection {
  /// The cargo package to build; the directory's own package when absent.
  pub package: Option<String>,
  /// The server binary rendered through the runner for `skene ssg`.
  pub server_bin: Option<String>,
}

/// The `[assets]` section.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct AssetsSection {
  /// Manifest-relative files or directories, each reachable at its own relative path
  /// as a URI.
  #[serde(default)]
  pub paths: Vec<String>,
}

/// The resolved app context every command runs against.
pub struct App {
  /// The app package's directory.
  pub dir: PathBuf,
  /// The workspace root.
  pub workspace_root: PathBuf,
  /// The cargo package name.
  pub package: String,
  /// The wasm artifact stem (the package name with `-` as `_`).
  pub stem: String,
  /// The server binary for ssg, when declared.
  pub server_bin: Option<String>,
  /// Whether web builds split `#[lazy]` functions into lazily-fetched wasm modules.
  pub split: bool,
  /// Every asset root contributing to this app, the app's own first.
  pub assets: Vec<AssetRoot>,
}

/// One package's declared assets: its name (the URI scope), its directory, and its
/// manifest-relative paths.
pub struct AssetRoot {
  /// The package name, scoping every URI as `/<package>/<path>`.
  pub package: String,
  /// The directory `skene.toml` lives in.
  pub dir: PathBuf,
  /// The declared manifest-relative paths.
  pub paths: Vec<String>,
}

impl App {
  /// Resolves the app at `dir`: its `skene.toml` (optional), its package name, and the
  /// asset roots of the whole dependency graph.
  pub fn load(dir: &Path) -> Result<Self, String> {
    let dir = dir
      .canonicalize()
      .map_err(|error| format!("app directory {}: {error}", dir.display()))?;
    let manifest = read_manifest(&dir)?.unwrap_or_default();
    let metadata = cargo_metadata(&dir)?;
    let workspace_root = metadata["workspace_root"]
      .as_str()
      .ok_or("cargo metadata: no workspace_root")?
      .into();
    let package = match manifest.app.package {
      Some(package) => package,
      None => package_name(&dir)?,
    };

    let mut assets = Vec::new();

    if !manifest.assets.paths.is_empty() {
      assets.push(AssetRoot {
        package: package.clone(),
        dir: dir.clone(),
        paths: manifest.assets.paths,
      });
    }

    if let Some(packages) = metadata["packages"].as_array() {
      for entry in packages {
        let (Some(name), Some(manifest_path)) =
          (entry["name"].as_str(), entry["manifest_path"].as_str())
        else {
          continue;
        };

        let Some(package_dir) = Path::new(manifest_path).parent()
        else {
          continue;
        };

        if package_dir == dir {
          continue;
        }

        if let Some(found) = read_manifest(package_dir)?
          && !found.assets.paths.is_empty()
        {
          assets.push(AssetRoot {
            package: name.to_owned(),
            dir: package_dir.to_path_buf(),
            paths: found.assets.paths,
          });
        }
      }
    }

    Ok(Self {
      stem: package.replace('-', "_"),
      dir,
      workspace_root,
      package,
      server_bin: manifest.app.server_bin,
      split: manifest.web.split,
      assets,
    })
  }

  /// The file an asset URI resolves to, if declared: the URI's first segment names the
  /// providing package, the rest a path that package declared (itself or within a
  /// declared directory).
  pub fn resolve_asset(&self, uri: &str) -> Option<PathBuf> {
    let relative = uri.trim_start_matches('/');

    if relative.contains("..") {
      return None;
    }

    let (package, relative) = relative.split_once('/')?;
    let root = self.assets.iter().find(|root| root.package == package)?;

    for declared in &root.paths {
      let declared = declared.trim_end_matches('/');

      if relative == declared || relative.starts_with(&format!("{declared}/")) {
        let path = root.dir.join(relative);

        if path.is_file() {
          return Some(path);
        }
      }
    }

    None
  }

  /// Copies every declared asset into `out`, preserving its package-scoped URI path.
  pub fn copy_assets(&self, out: &Path) -> Result<(), String> {
    let mut copied: Vec<PathBuf> = Vec::new();

    for root in &self.assets {
      for declared in &root.paths {
        let declared = declared.trim_end_matches('/');
        let source = root.dir.join(declared);

        copy_tree(
          &source,
          &format!("{}/{declared}", root.package),
          out,
          &mut copied,
        )?;
      }
    }

    Ok(())
  }

  /// Every directory to watch for asset changes.
  pub fn asset_dirs(&self) -> Vec<PathBuf> {
    self
      .assets
      .iter()
      .flat_map(|root| {
        root
          .paths
          .iter()
          .map(|declared| root.dir.join(declared.trim_end_matches('/')))
      })
      .filter(|path| path.is_dir())
      .collect()
  }
}

/// Copies `source` (file or directory) to `out/<relative>`, recursing into
/// directories.
fn copy_tree(
  source: &Path,
  relative: &str,
  out: &Path,
  copied: &mut Vec<PathBuf>,
) -> Result<(), String> {
  if source.is_dir() {
    let entries = fs::read_dir(source).map_err(|error| error.to_string())?;

    for entry in entries {
      let entry = entry.map_err(|error| error.to_string())?;
      let name = entry.file_name();
      let name = name.to_string_lossy();

      copy_tree(&entry.path(), &format!("{relative}/{name}"), out, copied)?;
    }

    return Ok(());
  }

  if !source.is_file() {
    eprintln!(
      "warning: declared asset {} does not exist",
      source.display()
    );

    return Ok(());
  }

  let target = out.join(relative);

  if copied.contains(&target) {
    eprintln!("warning: asset {relative} shadowed by an earlier declaration");

    return Ok(());
  }

  if let Some(parent) = target.parent() {
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
  }

  fs::copy(source, &target).map_err(|error| error.to_string())?;
  copied.push(target);

  Ok(())
}

/// The parsed `skene.toml` beside `dir`'s `Cargo.toml`, or `None` when there is none.
fn read_manifest(dir: &Path) -> Result<Option<ManifestFile>, String> {
  let path = dir.join("skene.toml");

  if !path.is_file() {
    return Ok(None);
  }

  let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;

  toml::from_str(&text)
    .map(Some)
    .map_err(|error| format!("{}: {error}", path.display()))
}

/// The `[package] name` of the `Cargo.toml` in `dir`.
fn package_name(dir: &Path) -> Result<String, String> {
  let path = dir.join("Cargo.toml");
  let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
  let value: toml::Value = toml::from_str(&text).map_err(|error| error.to_string())?;

  value
    .get("package")
    .and_then(|package| package.get("name"))
    .and_then(|name| name.as_str())
    .map(str::to_owned)
    .ok_or_else(|| format!("{}: no [package] name", path.display()))
}

/// The full cargo metadata for the workspace `dir` belongs to.
fn cargo_metadata(dir: &Path) -> Result<serde_json::Value, String> {
  let output = Command::new("cargo")
    .current_dir(dir)
    .args(["metadata", "--format-version", "1"])
    .output()
    .map_err(|error| format!("cargo metadata: {error}"))?;

  if !output.status.success() {
    return Err(format!(
      "cargo metadata failed: {}",
      String::from_utf8_lossy(&output.stderr)
    ));
  }

  serde_json::from_slice(&output.stdout).map_err(|error| format!("cargo metadata: {error}"))
}
