//! `skene.toml`: the framework manifest, and asset collection across the dependency
//! graph.
//!
//! Every setting is optional; an app with no manifest gets working defaults from its
//! `Cargo.toml`. Assets are declared as manifest-relative files or directories and
//! become reachable at `/<package>/<declared path>`—the providing package's name scopes
//! every URI, so the app's and its libraries' assets can never collide. Predictable,
//! never statically checked. A library the app depends on, directly or through
//! another, over normal dependencies, contributes the assets its own `skene.toml`
//! declares the same way.
//!
//! An app's `public` directory holds the files served at the site's root as they are,
//! unscoped: `public/favicon.ico` at `/favicon.ico`, `public/.well-known/` at
//! `/.well-known/`.

use std::{
  collections::{HashMap, HashSet, VecDeque},
  fs,
  hash::{DefaultHasher, Hash, Hasher},
  path::{Path, PathBuf},
  sync::{Arc, PoisonError, RwLock},
};

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand, Node, PackageId};
use serde::Deserialize;

/// The parsed `skene.toml` of one package.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct ManifestFile {
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
pub(crate) struct WebSection {
  /// Whether web builds split `#[lazy]` functions into lazily-fetched wasm modules.
  #[serde(default)]
  pub split: bool,
}

/// The `[app]` section.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct AppSection {
  /// The cargo package to build; the directory's own package when absent.
  pub package: Option<String>,
}

/// The `[assets]` section.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct AssetsSection {
  /// Manifest-relative files or directories, each reachable at its own relative path
  /// as a URI.
  #[serde(default)]
  pub paths: Vec<String>,
}

/// The resolved app context every command runs against.
pub(crate) struct App {
  /// The app package's directory.
  pub dir: PathBuf,
  /// The workspace root.
  pub workspace_root: PathBuf,
  /// The cargo package name.
  pub package: String,
  /// The library's crate name (the package name with `-` as `_`), naming its wasm and
  /// its binaries.
  pub stem: String,
  /// Whether web builds split `#[lazy]` functions into lazily-fetched wasm modules.
  pub split: bool,
  /// Every asset root contributing to this app, the app's own first.
  pub assets: Vec<AssetRoot>,
  /// The directories of the local packages the app is built from, its own first: every
  /// path dependency over normal and build dependencies.
  pub package_dirs: Vec<PathBuf>,
}

/// The app a dev loop builds and serves, replaced whole each time it is loaded again.
#[derive(Clone)]
pub(crate) struct LiveApp(Arc<RwLock<Arc<App>>>);

impl LiveApp {
  /// Shares `app`.
  pub(crate) fn new(app: App) -> Self {
    Self(Arc::new(RwLock::new(Arc::new(app))))
  }

  /// The app as last loaded.
  pub(crate) fn current(&self) -> Arc<App> {
    Arc::clone(&self.0.read().unwrap_or_else(PoisonError::into_inner))
  }

  /// Loads the app again from its directory, its `skene.toml` included, and returns it;
  /// on an error the app stands as it was.
  pub(crate) fn reload(&self) -> Result<Arc<App>, String> {
    let app = Arc::new(App::load(&self.current().dir)?);

    *self.0.write().unwrap_or_else(PoisonError::into_inner) = Arc::clone(&app);

    Ok(app)
  }
}

/// The directories of the local packages `app` is built from, its own first: every
/// path package it reaches over normal and build dependencies.
fn local_package_dirs(
  metadata: &Metadata,
  app: &PackageId,
  nodes: &HashMap<&PackageId, &Node>,
) -> Vec<PathBuf> {
  let mut seen: HashSet<&PackageId> = HashSet::from([app]);
  let mut pending: VecDeque<&PackageId> = VecDeque::from([app]);
  let mut dirs = Vec::new();

  while let Some(id) = pending.pop_front() {
    if let Some(package) = metadata.packages.iter().find(|package| &package.id == id)
      && package.source.is_none()
      && let Some(dir) = package.manifest_path.parent()
    {
      dirs.push(dir.to_path_buf().into_std_path_buf());
    }

    let Some(node) = nodes.get(id)
    else {
      continue;
    };

    for dependency in &node.deps {
      let built = dependency
        .dep_kinds
        .iter()
        .any(|kind| matches!(kind.kind, DependencyKind::Normal | DependencyKind::Build));

      if built && seen.insert(&dependency.pkg) {
        pending.push_back(&dependency.pkg);
      }
    }
  }

  dirs
}

/// One package's declared assets: its name (the URI scope), its directory, and its
/// manifest-relative paths.
pub(crate) struct AssetRoot {
  /// The package name, scoping every URI as `/<package>/<path>`.
  pub package: String,
  /// The directory `skene.toml` lives in.
  pub dir: PathBuf,
  /// The declared manifest-relative paths.
  pub paths: Vec<String>,
}

impl App {
  /// Resolves the app at `dir`: its `skene.toml` (optional), its package name, the
  /// asset roots of the packages it depends on over normal dependencies, and its local
  /// packages.
  pub(crate) fn load(dir: &Path) -> Result<Self, String> {
    let dir = dir
      .canonicalize()
      .map_err(|error| format!("app directory {}: {error}", dir.display()))?;
    let manifest = read_manifest(&dir)?.unwrap_or_default();
    let metadata = MetadataCommand::new()
      .current_dir(&dir)
      .exec()
      .map_err(|error| format!("cargo metadata: {error}"))?;
    let workspace_root = metadata.workspace_root.clone().into_std_path_buf();
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

    let app = metadata
      .packages
      .iter()
      .find(|entry| {
        entry
          .manifest_path
          .parent()
          .is_some_and(|parent| parent == dir)
      })
      .ok_or_else(|| format!("cargo metadata: no package at {}", dir.display()))?;
    let resolve = metadata
      .resolve
      .as_ref()
      .ok_or("cargo metadata: no resolve graph")?;
    let nodes: HashMap<&PackageId, _> = resolve.nodes.iter().map(|node| (&node.id, node)).collect();
    let mut seen: HashSet<&PackageId> = HashSet::from([&app.id]);
    let mut pending: VecDeque<&PackageId> = VecDeque::from([&app.id]);

    while let Some(id) = pending.pop_front() {
      let Some(node) = nodes.get(id)
      else {
        continue;
      };

      for dependency in &node.deps {
        let normal = dependency
          .dep_kinds
          .iter()
          .any(|kind| kind.kind == DependencyKind::Normal);

        if normal && seen.insert(&dependency.pkg) {
          pending.push_back(&dependency.pkg);

          let Some(entry) = metadata
            .packages
            .iter()
            .find(|entry| entry.id == dependency.pkg)
          else {
            continue;
          };
          let Some(package_dir) = entry.manifest_path.parent()
          else {
            continue;
          };

          if let Some(found) = read_manifest(package_dir.as_std_path())?
            && !found.assets.paths.is_empty()
          {
            assets.push(AssetRoot {
              package: entry.name.to_string(),
              dir: package_dir.to_path_buf().into_std_path_buf(),
              paths: found.assets.paths,
            });
          }
        }
      }
    }

    let package_dirs = local_package_dirs(&metadata, &app.id, &nodes);

    Ok(Self {
      stem: package.replace('-', "_"),
      dir,
      workspace_root,
      package,
      split: manifest.web.split,
      assets,
      package_dirs,
    })
  }

  /// The file an asset URI resolves to, if declared: the URI's first segment names the
  /// providing package, the rest a path that package declared (itself or within a
  /// declared directory).
  pub(crate) fn resolve_asset(&self, uri: &str) -> Option<PathBuf> {
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
  pub(crate) fn copy_assets(&self, out: &Path) -> Result<(), String> {
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

  /// Copies the app's `public` directory, where it has one, to the root of `out`.
  pub(crate) fn copy_public(&self, out: &Path) -> Result<(), String> {
    let public = self.public_dir();

    if !public.is_dir() {
      return Ok(());
    }

    copy_tree(&public, "", out, &mut Vec::new())
  }

  /// The app's `public` directory: the files served at the site's root as they are.
  pub(crate) fn public_dir(&self) -> PathBuf {
    self.dir.join("public")
  }

  /// The file of the app's `public` directory a path from the site's root names, if
  /// there is one.
  pub(crate) fn public_file(&self, relative: &str) -> Option<PathBuf> {
    let path = self.public_dir().join(relative);

    path.is_file().then_some(path)
  }

  /// Every directory to watch for asset changes, the `public` one among them.
  pub(crate) fn asset_dirs(&self) -> Vec<PathBuf> {
    self
      .assets
      .iter()
      .flat_map(|root| {
        root
          .paths
          .iter()
          .map(|declared| root.dir.join(declared.trim_end_matches('/')))
      })
      .chain([self.public_dir()])
      .filter(|path| path.is_dir())
      .collect()
  }

  /// A digest of every declared asset file's and public file's path, length and
  /// modification time, which moves when one is added, removed or changed.
  pub(crate) fn asset_stamp(&self) -> u64 {
    let mut hasher = DefaultHasher::new();
    let declared = self.assets.iter().flat_map(|root| {
      root
        .paths
        .iter()
        .map(|declared| root.dir.join(declared.trim_end_matches('/')))
    });

    for path in declared.chain([self.public_dir()]) {
      let entries = walkdir::WalkDir::new(path)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok);

      for entry in entries {
        if let Ok(metadata) = entry.metadata()
          && metadata.is_file()
        {
          entry.path().hash(&mut hasher);
          metadata.len().hash(&mut hasher);
          metadata.modified().ok().hash(&mut hasher);
        }
      }
    }

    hasher.finish()
  }
}

/// Copies `source` (file or directory) to `out/<relative>`, every file beneath a
/// directory, symbolic links followed until one loops back, which is skipped.
fn copy_tree(
  source: &Path,
  relative: &str,
  out: &Path,
  copied: &mut Vec<PathBuf>,
) -> Result<(), String> {
  if !source.exists() {
    eprintln!(
      "warning: declared asset {} does not exist",
      source.display()
    );

    return Ok(());
  }

  for entry in walkdir::WalkDir::new(source)
    .follow_links(true)
    .sort_by_file_name()
  {
    let entry = match entry {
      Ok(entry) => entry,
      Err(error) if error.loop_ancestor().is_some() => {
        eprintln!("warning: {error}; the loop is skipped");

        continue;
      }
      Err(error) => return Err(error.to_string()),
    };

    if !entry.path().is_file() {
      continue;
    }

    let below = entry
      .path()
      .strip_prefix(source)
      .map_err(|error| error.to_string())?;
    let target = if below.as_os_str().is_empty() {
      out.join(relative)
    }
    else {
      out.join(relative).join(below)
    };

    if copied.contains(&target) {
      eprintln!(
        "warning: asset {} shadowed by an earlier declaration",
        target.display()
      );

      continue;
    }

    if let Some(parent) = target.parent() {
      fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    fs::copy(entry.path(), &target).map_err(|error| error.to_string())?;
    copied.push(target);
  }

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

#[cfg(all(test, unix))]
mod tests {
  use std::{fs, os::unix::fs::symlink, path::Path};

  use super::{App, copy_tree};

  /// A symbolic link that loops back is skipped, and every file beneath is copied once.
  #[test]
  fn copying_a_looping_tree_ends() {
    let root = std::env::temp_dir().join(format!("skene-copy-tree-{}", std::process::id()));
    let source = root.join("assets");
    let out = root.join("out");

    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(source.join("deep")).expect("make the tree");
    fs::write(source.join("deep/file.txt"), "x").expect("write a file");
    symlink("..", source.join("deep/loop")).expect("link back up");

    let mut copied = Vec::new();

    copy_tree(&source, "app/assets", &out, &mut copied).expect("the copy ends");

    assert_eq!(copied, [out.join("app/assets/deep/file.txt")]);

    let _ = fs::remove_dir_all(&root);
  }

  /// An app's local packages are those it builds from, its own first: its path
  /// dependencies over normal and build dependencies, and nothing else in the workspace.
  #[test]
  fn an_app_builds_from_its_local_packages() {
    let counter = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/web/counter");
    let app = App::load(&counter).unwrap();
    let builds_from = |package: &str| {
      app
        .package_dirs
        .iter()
        .any(|dir| dir.ends_with(Path::new("packages").join(package)))
    };

    assert_eq!(app.package_dirs.first(), Some(&app.dir));
    assert!(builds_from("web"));
    assert!(builds_from("foundation"));
    assert!(builds_from("build"));
    assert!(!builds_from("cli"));
    assert!(!builds_from("terminal"));
  }

  /// The public directory's files land at the output's root, and a path from the site's
  /// root names the one it holds.
  #[test]
  fn public_files_land_at_the_site_root() {
    let dir = std::env::temp_dir().join(format!("skene-public-{}", std::process::id()));
    let public = dir.join("public");
    let out = dir.join("out");
    let _ = fs::remove_dir_all(&dir);

    fs::create_dir_all(public.join(".well-known")).unwrap();
    fs::write(public.join("robots.txt"), "User-agent: *").unwrap();
    fs::write(public.join(".well-known/security.txt"), "Contact: x").unwrap();

    let app = App {
      dir: dir.clone(),
      workspace_root: dir.clone(),
      package: "app".to_owned(),
      stem: "app".to_owned(),
      split: false,
      assets: Vec::new(),
      package_dirs: vec![dir.clone()],
    };

    app.copy_public(&out).unwrap();

    assert!(out.join("robots.txt").is_file());
    assert!(out.join(".well-known/security.txt").is_file());
    assert_eq!(
      app.public_file(".well-known/security.txt"),
      Some(public.join(".well-known/security.txt"))
    );
    assert_eq!(app.public_file("missing.txt"), None);

    fs::remove_dir_all(&dir).unwrap();
  }
}
