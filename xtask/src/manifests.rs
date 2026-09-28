//! What the gate reads from the workspace's manifests and its lockfile.

use std::{
  fs,
  path::{Path, PathBuf},
};

use toml_edit::{DocumentMut, Item, TableLike};

/// The dependency tables a manifest lists crates in, each alphabetically.
const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// Checks that every manifest in the workspace lists each dependency table's crates in
/// alphabetical order.
pub(crate) fn check_order(root: &Path) -> Result<(), String> {
  let mut problems = Vec::new();

  for path in manifest_paths(root)? {
    let manifest = read(&path)?;
    let name = path.strip_prefix(root).unwrap_or(&path).display();

    for (table, entries) in dependency_tables(&manifest) {
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
  }

  if problems.is_empty() {
    Ok(())
  }
  else {
    Err(problems.join("\n"))
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

  tables
}
