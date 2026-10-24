//! What the gate reads from the workspace's manifests and its lockfile.

use std::{
  collections::BTreeSet,
  fs,
  path::{Component, Path, PathBuf},
};

use toml_edit::{DocumentMut, Item, TableLike};

/// The dependency tables a manifest lists crates in, each alphabetically.
const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// The directory the crates a manifest patches in are vendored under.
const VENDOR: &str = "vendor";

/// The notes a vendored crate keeps beside its manifest: where it comes from and what
/// Skene changed in it.
const NOTES: &str = "SKENE.md";

/// Checks every manifest in the workspace: each dependency table and each `[patch]` table
/// lists its crates in alphabetical order, a cargo feature enables optional dependencies
/// alone and none is on by default, a crate is optional only where a feature enables it,
/// each member inherits the workspace's lints and rust-version, a crate under `packages/`
/// declares a description, inherits the license and the repository and names a workspace
/// crate it tests against by its path alone, every other member is never published, and
/// every crate a `[patch]` table patches in is a `path` under `vendor/` holding a
/// `SKENE.md`.
///
/// A vendored crate is exempt from the rest: it is no member (the workspace excludes
/// `vendor/`), so it keeps its published manifest and its own lints, outside the
/// workspace's lints, rust-version and publishing rules.
pub(crate) fn check(root: &Path) -> Result<(), String> {
  let mut problems = Vec::new();
  let workspace = root.join("Cargo.toml");
  let members: Vec<String> = manifest_paths(root)?
    .iter()
    .filter_map(|path| {
      let manifest = read(path).ok()?;

      manifest
        .get("package")?
        .get("name")?
        .as_str()
        .map(str::to_owned)
    })
    .collect();

  for path in manifest_paths(root)? {
    let manifest = read(&path)?;
    let name = path.strip_prefix(root).unwrap_or(&path).display();

    if path != workspace {
      if !inherits(&manifest, &["lints", "workspace"]) {
        problems.push(format!(
          "{name}: inherits no workspace lints (`[lints] workspace = true`)"
        ));
      }

      if !inherits(&manifest, &["package", "rust-version", "workspace"]) {
        problems.push(format!(
          "{name}: inherits no rust-version (`rust-version.workspace = true`)"
        ));
      }

      if path
        .strip_prefix(root)
        .is_ok_and(|relative| relative.starts_with("packages"))
      {
        check_publishable(&name.to_string(), &manifest, &members, &mut problems);
      }
      else if manifest
        .get("package")
        .and_then(|package| package.get("publish"))
        .and_then(Item::as_bool)
        != Some(false)
      {
        problems.push(format!(
          "{name}: lies outside `packages/` yet may be published (`publish = false`)"
        ));
      }
    }

    let enabled = features(&manifest, &name.to_string(), &mut problems);

    check_patches(
      &name.to_string(),
      &manifest,
      |path| root.join(path).join(NOTES).is_file(),
      &mut problems,
    );

    for (table, entries) in dependency_tables(&manifest) {
      for (dependency, entry) in entries.iter() {
        if entry.get("optional").and_then(Item::as_bool) == Some(true)
          && !enabled.iter().any(|crate_name| crate_name == dependency)
        {
          problems.push(format!(
            "{name}: [{table}] makes `{dependency}` optional with no feature enabling it \
             (`dep:{dependency}`)"
          ));
        }
      }

      check_order(&name.to_string(), &table, entries, &mut problems);
    }
  }

  check_default_members(root, &mut problems)?;

  if problems.is_empty() {
    Ok(())
  }
  else {
    Err(problems.join("\n"))
  }
}

/// Notes where the workspace's `default-members` differ from the directories of the
/// members reaching neither facade ([`Workspace::host`]), which plain cargo at the root
/// builds with no platform named, listed alphabetically.
fn check_default_members(root: &Path, problems: &mut Vec<String>) -> Result<(), String> {
  let workspace = Workspace::read(root)?;
  let host = workspace.host();
  let mut expected = Vec::new();

  for path in manifest_paths(root)? {
    let manifest = read(&path)?;
    let name = manifest
      .get("package")
      .and_then(|package| package.get("name"))
      .and_then(Item::as_str);

    if let Some(name) = name
      && host.contains(&name)
      && let Some(directory) = path
        .parent()
        .and_then(|parent| parent.strip_prefix(root).ok())
    {
      expected.push(directory.to_string_lossy().replace('\\', "/"));
    }
  }

  expected.sort();

  let listed: Vec<String> = read(&root.join("Cargo.toml"))?
    .get("workspace")
    .and_then(|workspace| workspace.get("default-members"))
    .and_then(Item::as_array)
    .map(|members| {
      members
        .iter()
        .filter_map(|member| member.as_str().map(str::to_owned))
        .collect()
    })
    .unwrap_or_default();

  if listed != expected {
    problems.push(format!(
      "Cargo.toml: [workspace] default-members lists {listed:?}; the members reaching \
       neither facade, alphabetically, are {expected:?}"
    ));
  }

  Ok(())
}

/// Notes what the crate under `packages/` that `manifest` (named `name`) lacks to
/// publish: a description, the workspace's license and repository, and its path alone
/// for each of the workspace's `members` it tests against.
fn check_publishable(
  name: &str,
  manifest: &DocumentMut,
  members: &[String],
  problems: &mut Vec<String>,
) {
  if manifest
    .get("package")
    .and_then(|package| package.get("description"))
    .and_then(Item::as_str)
    .is_none()
  {
    problems.push(format!("{name}: declares no description"));
  }

  for key in ["license", "repository"] {
    if !inherits(manifest, &["package", key, "workspace"]) {
      problems.push(format!(
        "{name}: inherits no {key} (`{key}.workspace = true`)"
      ));
    }
  }

  for (table, entries) in dependency_tables(manifest) {
    if !table.ends_with("dev-dependencies") {
      continue;
    }

    for (dependency, entry) in entries.iter() {
      if members.iter().any(|member| member == dependency)
        && (entry.get("path").is_none()
          || entry.get("version").is_some()
          || entry.get("workspace").is_some())
      {
        problems.push(format!(
          "{name}: [{table}] names the workspace's `{dependency}` by more than its path, \
           which a publish would wait on (`{{ path = \"..\" }}`)"
        ));
      }
    }
  }
}

/// Notes the first crate the dependency table `table` of the manifest named `name` lists
/// out of alphabetical order in `entries`.
fn check_order(name: &str, table: &str, entries: &dyn TableLike, problems: &mut Vec<String>) {
  let keys: Vec<&str> = entries.iter().map(|(key, _)| key).collect();
  let unordered = keys.windows(2).find_map(|pair| match *pair {
    [earlier, later] if earlier > later => Some((earlier, later)),
    _ => None,
  });

  if let Some((earlier, later)) = unordered {
    problems.push(format!(
      "{name}: [{table}] lists `{later}` after `{earlier}`"
    ));
  }
}

/// Notes each crate a `[patch]` table of `manifest` (named `name`) patches in that is not a
/// `path` under `vendor/` whose directory `holds_notes` (given the path as the manifest
/// spells it) answers holds a `SKENE.md`.
fn check_patches(
  name: &str,
  manifest: &DocumentMut,
  holds_notes: impl Fn(&Path) -> bool,
  problems: &mut Vec<String>,
) {
  for (table, entries) in patch_tables(manifest) {
    for (dependency, entry) in entries.iter() {
      let Some(path) = entry.get("path").and_then(Item::as_str).map(Path::new)
      else {
        problems.push(format!(
          "{name}: [{table}] patches `{dependency}` in from elsewhere than a path under \
           `{VENDOR}/` (`{{ path = \"{VENDOR}/{dependency}\" }}`)"
        ));

        continue;
      };

      let vendored = path.starts_with(VENDOR)
        && path.components().count() > 1
        && path
          .components()
          .all(|component| matches!(component, Component::Normal(_)));

      if !vendored {
        problems.push(format!(
          "{name}: [{table}] patches `{dependency}` in from `{}`, outside `{VENDOR}/`",
          path.display()
        ));
      }
      else if !holds_notes(path) {
        problems.push(format!(
          "{name}: [{table}] patches `{dependency}` in from `{}`, which holds no `{NOTES}` \
           naming its source and Skene's changes",
          path.display()
        ));
      }
    }
  }
}

/// The optional dependencies the features of `manifest` (named `name`) enable, a
/// problem noted for a default feature and for a feature enabling anything else.
fn features(manifest: &DocumentMut, name: &str, problems: &mut Vec<String>) -> Vec<String> {
  let mut enabled = Vec::new();
  let Some(features) = manifest.get("features").and_then(Item::as_table_like)
  else {
    return enabled;
  };

  for (feature, enables) in features.iter() {
    if feature == "default" {
      problems.push(format!(
        "{name}: turns features on by default, so the build without them is not the default"
      ));

      continue;
    }

    let dependencies: Option<Vec<&str>> = enables.as_array().and_then(|array| {
      array
        .iter()
        .map(|value| value.as_str().and_then(|value| value.strip_prefix("dep:")))
        .collect()
    });

    match dependencies {
      Some(dependencies) => enabled.extend(dependencies.into_iter().map(str::to_owned)),
      None => problems.push(format!(
        "{name}: feature `{feature}` enables more than optional dependencies (`dep:` crates)"
      )),
    }
  }

  enabled
}

/// Whether the key at `path` in `manifest` is `true`.
fn inherits(manifest: &DocumentMut, path: &[&str]) -> bool {
  let mut item = manifest.as_item();

  for key in path {
    match item.get(key) {
      Some(next) => item = next,
      None => return false,
    }
  }

  item.as_bool() == Some(true)
}

/// The web's facade: a package reaching it is built for the web's leaves alone.
pub(crate) const WEB: &str = "skene-web";

/// The terminal's facade: a package reaching it is built for the terminal alone.
pub(crate) const TERMINAL: &str = "skene-terminal";

/// The workspace's packages and what each depends on among them, from which every step
/// reads the packages it builds.
pub(crate) struct Workspace {
  /// Every member, in the order the workspace lists them.
  packages: Vec<Package>,
}

/// A member of the workspace and the members it depends on, for any target.
struct Package {
  /// Its package name.
  name: String,
  /// The members in its `[dependencies]` tables.
  normal: BTreeSet<String>,
  /// The members in its `[build-dependencies]` tables.
  build: BTreeSet<String>,
  /// The members in its `[dev-dependencies]` tables.
  dev: BTreeSet<String>,
}

impl Workspace {
  /// The workspace under `root`, read from its members' manifests.
  pub(crate) fn read(root: &Path) -> Result<Self, String> {
    let manifests = manifest_paths(root)?
      .iter()
      .map(|path| read(path))
      .collect::<Result<Vec<_>, _>>()?;

    Ok(Self::new(&manifests))
  }

  /// The workspace whose manifests are `manifests`, a manifest without a package (the
  /// workspace's own) adding none.
  fn new(manifests: &[DocumentMut]) -> Self {
    let names: BTreeSet<&str> = manifests
      .iter()
      .filter_map(|manifest| manifest.get("package")?.get("name")?.as_str())
      .collect();
    let packages = manifests
      .iter()
      .filter_map(|manifest| {
        let name = manifest.get("package")?.get("name")?.as_str()?;
        let mut package = Package {
          name: name.to_owned(),
          normal: BTreeSet::new(),
          build: BTreeSet::new(),
          dev: BTreeSet::new(),
        };

        for (kind, entries) in member_dependency_tables(manifest) {
          let members = match kind {
            "dependencies" => &mut package.normal,
            "build-dependencies" => &mut package.build,
            _ => &mut package.dev,
          };

          for (key, entry) in entries.iter() {
            let dependency = entry.get("package").and_then(Item::as_str).unwrap_or(key);

            if names.contains(dependency) {
              members.insert(dependency.to_owned());
            }
          }
        }

        Some(package)
      })
      .collect();

    Self { packages }
  }

  /// The members `seeds` are, and those each depends on through `edges`, directly or
  /// through others.
  fn closure<'a>(
    &'a self,
    seeds: impl IntoIterator<Item = &'a str>,
    edges: impl Fn(&'a Package) -> Vec<&'a BTreeSet<String>>,
  ) -> BTreeSet<&'a str> {
    let mut reached = BTreeSet::new();
    let mut pending: Vec<&str> = seeds.into_iter().collect();

    while let Some(next) = pending.pop() {
      if !reached.insert(next) {
        continue;
      }

      for package in self.packages.iter().filter(|package| package.name == next) {
        pending.extend(edges(package).into_iter().flatten().map(String::as_str));
      }
    }

    reached
  }

  /// Whether a build of the member `name`, its tests included, compiles `facade`: it is
  /// `facade`, or one of its dependencies of any kind is or builds it through normal and
  /// build dependencies, a dependency's own dev-dependencies never being built.
  fn reaches(&self, name: &str, facade: &str) -> bool {
    let dependencies = self
      .packages
      .iter()
      .filter(|package| package.name == name)
      .flat_map(|package| [&package.normal, &package.build, &package.dev])
      .flatten()
      .map(String::as_str);

    name == facade
      || self
        .closure(dependencies, |package| {
          vec![&package.normal, &package.build]
        })
        .contains(facade)
  }

  /// The members the host's own build checks, documents and tests, naming no platform:
  /// those reaching neither [`WEB`] nor [`TERMINAL`].
  pub(crate) fn host(&self) -> Vec<&str> {
    self
      .packages
      .iter()
      .map(|package| package.name.as_str())
      .filter(|name| !self.reaches(name, WEB) && !self.reaches(name, TERMINAL))
      .collect()
  }

  /// The members built for the web's leaves alone: those reaching [`WEB`], itself
  /// included.
  pub(crate) fn web(&self) -> Vec<&str> {
    self.reaching(WEB)
  }

  /// The members built for the terminal alone: those reaching [`TERMINAL`], itself
  /// included.
  pub(crate) fn terminal(&self) -> Vec<&str> {
    self.reaching(TERMINAL)
  }

  /// The members reaching `facade`, itself included, in the order the workspace lists
  /// them.
  fn reaching(&self, facade: &str) -> Vec<&str> {
    self
      .packages
      .iter()
      .map(|package| package.name.as_str())
      .filter(|name| self.reaches(name, facade))
      .collect()
  }

  /// Every member a web build compiles for the web's leaf: those reaching [`WEB`], and
  /// every member they depend on in a `[dependencies]` table, directly or through others.
  pub(crate) fn web_build(&self) -> Vec<&str> {
    let built = self.closure(self.web(), |package| vec![&package.normal]);

    self
      .packages
      .iter()
      .map(|package| package.name.as_str())
      .filter(|name| built.contains(name))
      .collect()
  }
}

/// The Rust version the workspace declares as its minimum.
pub(crate) fn rust_version(root: &Path) -> Result<String, String> {
  read(&root.join("Cargo.toml"))?
    .get("workspace")
    .and_then(|workspace| workspace.get("package"))
    .and_then(|package| package.get("rust-version"))
    .and_then(Item::as_str)
    .map(str::to_owned)
    .ok_or_else(|| "the workspace declares no rust-version".to_owned())
}

/// The version of `name` the lockfile pins.
pub(crate) fn locked_version(root: &Path, name: &str) -> Result<String, String> {
  read(&root.join("Cargo.lock"))?
    .get("package")
    .and_then(Item::as_array_of_tables)
    .and_then(|packages| {
      packages
        .iter()
        .find(|package| package.get("name").and_then(Item::as_str) == Some(name))
    })
    .and_then(|package| package.get("version"))
    .and_then(Item::as_str)
    .map(str::to_owned)
    .ok_or_else(|| format!("Cargo.lock pins no `{name}`"))
}

/// The directory of the workspace member named `name`.
pub(crate) fn package_directory(root: &Path, name: &str) -> Result<PathBuf, String> {
  for path in manifest_paths(root)? {
    let manifest = read(&path)?;

    if manifest
      .get("package")
      .and_then(|package| package.get("name"))
      .and_then(Item::as_str)
      == Some(name)
    {
      return path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{}: no directory", path.display()));
    }
  }

  Err(format!("no workspace member is named `{name}`"))
}

fn read(path: &Path) -> Result<DocumentMut, String> {
  fs::read_to_string(path)
    .map_err(|error| error.to_string())?
    .parse()
    .map_err(|error: toml_edit::TomlError| error.to_string())
    .map_err(|error| format!("{}: {error}", path.display()))
}

/// The workspace's manifest and every member's.
fn manifest_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
  let workspace = read(&root.join("Cargo.toml"))?;
  let members = workspace
    .get("workspace")
    .and_then(|workspace| workspace.get("members"))
    .and_then(Item::as_array)
    .ok_or_else(|| "the workspace lists no members".to_owned())?;
  let mut paths = vec![root.join("Cargo.toml")];

  for pattern in members.iter().filter_map(|member| member.as_str()) {
    paths.extend(expand(root, pattern));
  }

  Ok(paths)
}

/// The manifests a member pattern names, a `*` segment matching any directory.
fn expand(root: &Path, pattern: &str) -> Vec<PathBuf> {
  let mut directories = vec![root.to_path_buf()];

  for segment in pattern.split('/') {
    directories = directories
      .into_iter()
      .flat_map(|directory| {
        if segment == "*" {
          let mut entries: Vec<PathBuf> = fs::read_dir(&directory)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();

          entries.sort();
          entries
        }
        else {
          vec![directory.join(segment)]
        }
      })
      .collect();
  }

  directories
    .into_iter()
    .map(|directory| directory.join("Cargo.toml"))
    .filter(|path| path.is_file())
    .collect()
}

/// Every dependency table of `manifest`, named as its header spells it.
fn dependency_tables(manifest: &DocumentMut) -> Vec<(String, &dyn TableLike)> {
  let mut tables = Vec::new();

  for kind in DEPENDENCY_TABLES {
    if let Some(table) = manifest.get(kind).and_then(Item::as_table_like) {
      tables.push((kind.to_owned(), table));
    }
  }

  if let Some(table) = manifest
    .get("workspace")
    .and_then(|workspace| workspace.get("dependencies"))
    .and_then(Item::as_table_like)
  {
    tables.push(("workspace.dependencies".to_owned(), table));
  }

  if let Some(targets) = manifest.get("target").and_then(Item::as_table_like) {
    for (cfg, target) in targets.iter() {
      for kind in DEPENDENCY_TABLES {
        if let Some(table) = target.get(kind).and_then(Item::as_table_like) {
          tables.push((format!("target.'{cfg}'.{kind}"), table));
        }
      }
    }
  }

  tables.extend(patch_tables(manifest));

  tables
}

/// Every dependency table of the member `manifest`, for any target, each with its kind
/// (`dependencies`, `dev-dependencies` or `build-dependencies`).
fn member_dependency_tables(manifest: &DocumentMut) -> Vec<(&'static str, &dyn TableLike)> {
  let mut scopes = vec![manifest.as_item()];

  if let Some(targets) = manifest.get("target").and_then(Item::as_table_like) {
    scopes.extend(targets.iter().map(|(_, target)| target));
  }

  DEPENDENCY_TABLES
    .iter()
    .flat_map(|kind| {
      scopes
        .iter()
        .filter_map(move |scope| scope.get(kind).and_then(Item::as_table_like))
        .map(move |table| (*kind, table))
    })
    .collect()
}

/// Every `[patch]` table of `manifest`, one per source it patches, named as its header
/// spells it.
fn patch_tables(manifest: &DocumentMut) -> Vec<(String, &dyn TableLike)> {
  manifest
    .get("patch")
    .and_then(Item::as_table_like)
    .into_iter()
    .flat_map(TableLike::iter)
    .filter_map(|(source, table)| {
      table
        .as_table_like()
        .map(|table| (format!("patch.{source}"), table))
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use toml_edit::DocumentMut;

  use super::{Workspace, check_order, check_patches, dependency_tables};

  fn manifest(source: &str) -> DocumentMut {
    source.parse().expect("the manifest parses")
  }

  fn patch_problems(source: &str) -> Vec<String> {
    let mut problems = Vec::new();

    check_patches(
      "Cargo.toml",
      &manifest(source),
      |path| path != Path::new("vendor/bare"),
      &mut problems,
    );

    problems
  }

  #[test]
  fn a_patch_table_is_held_to_alphabetical_order() {
    let manifest = manifest(
      "[patch.crates-io]\nparley_engine = { path = \"vendor/parley_engine\" }\nparley = { \
       path = \"vendor/parley\" }\n",
    );
    let mut problems = Vec::new();

    for (table, entries) in dependency_tables(&manifest) {
      check_order("Cargo.toml", &table, entries, &mut problems);
    }

    assert_eq!(
      problems,
      ["Cargo.toml: [patch.crates-io] lists `parley` after `parley_engine`"]
    );
  }

  #[test]
  fn a_patch_from_a_vendored_path_with_its_notes_passes() {
    assert_eq!(
      patch_problems(
        "[patch.crates-io]\nparley = { path = \"vendor/parley\" }\nparley_engine = { path = \
         \"vendor/parley_engine\" }\n"
      ),
      Vec::<String>::new()
    );
  }

  #[test]
  fn a_patch_from_elsewhere_than_vendor_is_a_problem() {
    let problems = patch_problems(
      "[patch.crates-io]\na = { git = \"https://example.com/a\" }\nb = { path = \"../b\" \
       }\nc = { path = \"vendor/../c\" }\nd = { path = \"vendor\" }\ne = { path = \
       \"packages/e\" }\n",
    );

    assert_eq!(problems.len(), 5, "{problems:#?}");
  }

  #[test]
  fn a_vendored_patch_without_its_notes_is_a_problem() {
    let problems = patch_problems("[patch.crates-io]\nbare = { path = \"vendor/bare\" }\n");

    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert!(problems[0].contains("SKENE.md"), "{}", problems[0]);
  }

  #[test]
  fn a_member_reaches_a_facade_through_what_its_build_compiles() {
    let workspace = Workspace::new(&[
      manifest("[workspace]\nmembers = [\"*\"]\n"),
      manifest("[package]\nname = \"skene-web\"\n[dependencies]\nbase = { path = \"..\" }\n"),
      manifest("[package]\nname = \"base\"\n[dependencies]\nserde = \"1\"\n"),
      manifest("[package]\nname = \"lib\"\n[dependencies]\nskene-web.workspace = true\n"),
      manifest(
        "[package]\nname = \"app\"\n[target.'cfg(unix)'.dependencies]\nlib = { path = \"..\" }\n",
      ),
      manifest(
        "[package]\nname = \"docs\"\n[dev-dependencies]\nskene-terminal = { path = \"..\" }\nweb = { package = \"skene-web\", path = \"..\" }\n",
      ),
      manifest("[package]\nname = \"tool\"\n[build-dependencies]\nbase = { path = \"..\" }\n"),
      manifest("[package]\nname = \"probe\"\n[dev-dependencies]\nskene-web = { path = \"..\" }\n"),
      manifest("[package]\nname = \"user\"\n[dependencies]\nprobe = { path = \"..\" }\n"),
      manifest("[package]\nname = \"skene-terminal\"\n[dependencies]\nbase = { path = \"..\" }\n"),
      manifest(
        "[package]\nname = \"shell\"\n[target.'cfg(skene_platform = \"terminal\")'.dependencies]\nskene-terminal = { path = \"..\" }\n",
      ),
    ]);

    assert_eq!(
      workspace.web(),
      ["skene-web", "lib", "app", "docs", "probe"]
    );
    assert_eq!(workspace.terminal(), ["docs", "skene-terminal", "shell"]);
    assert_eq!(workspace.host(), ["base", "tool", "user"]);
    assert_eq!(
      workspace.web_build(),
      ["skene-web", "base", "lib", "app", "docs", "probe"]
    );
  }
}
