//! `skene new`: a project from a template.

use std::{
  fs,
  path::{Path, PathBuf},
};

/// A platform an app is given a shell on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Platform {
  /// The web: the browser, and a server render.
  Web,
  /// The terminal.
  Terminal,
}

impl Platform {
  /// The shell's directory, and the suffix of its crate's name.
  const fn directory(self) -> &'static str {
    match self {
      Self::Web => "web",
      Self::Terminal => "terminal",
    }
  }

  /// The skene crates the shell depends on.
  const fn crates(self) -> [&'static str; 2] {
    match self {
      Self::Web => ["skene-simple-web", "skene-web"],
      Self::Terminal => ["skene-simple-terminal", "skene-terminal"],
    }
  }

  /// The shell's manifest and library, as templates.
  const fn templates(self) -> (&'static str, &'static str) {
    match self {
      Self::Web => (
        include_str!("../templates/app/web-Cargo.toml.tmpl"),
        include_str!("../templates/app/web-lib.rs.tmpl"),
      ),
      Self::Terminal => (
        include_str!("../templates/app/terminal-Cargo.toml.tmpl"),
        include_str!("../templates/app/terminal-lib.rs.tmpl"),
      ),
    }
  }
}

/// Where a generated project takes the skene crates from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Source {
  /// crates.io, at the version of this command.
  Registry,
  /// The skene checkout at this path.
  Checkout(PathBuf),
}

impl Source {
  /// The dependency entry naming `name`.
  fn entry(&self, name: &str) -> String {
    match self {
      Self::Registry => format!(
        "{name} = \"{}.{}\"",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR")
      ),
      Self::Checkout(root) => {
        let directory = name.strip_prefix("skene-").unwrap_or(name);

        format!(
          "{name} = {{ path = \"{}\" }}",
          root.join("packages").join(directory).display()
        )
      }
    }
  }
}

/// Creates the app at `path`, named for its last component: a crate of shared logic and
/// state, and a shell for each of `platforms`, the skene crates taken from `source`.
pub(crate) fn new_app(path: &Path, platforms: &[Platform], source: &Source) -> Result<(), String> {
  let name = path
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| format!("{}: names no directory", path.display()))?;

  check_name(name)?;

  if path.exists() {
    return Err(format!("{} already exists", path.display()));
  }

  let mut platforms = platforms.to_vec();

  platforms.sort_by_key(|platform| platform.directory());
  platforms.dedup();

  for (file, contents) in app_files(name, &platforms, source) {
    let file = path.join(file);

    if let Some(parent) = file.parent() {
      fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }

    fs::write(&file, contents).map_err(|error| format!("{}: {error}", file.display()))?;
  }

  Ok(())
}

/// Checks that `name` makes a crate name: a lowercase letter, then lowercase letters,
/// digits, hyphens and underscores.
fn check_name(name: &str) -> Result<(), String> {
  let mut characters = name.chars();
  let starts = characters
    .next()
    .is_some_and(|first| first.is_ascii_lowercase());
  let continues = characters
    .all(|next| next.is_ascii_lowercase() || next.is_ascii_digit() || next == '-' || next == '_');

  if starts && continues {
    Ok(())
  }
  else {
    Err(format!(
      "`{name}` makes no crate name: start with a lowercase letter, then use lowercase \
       letters, digits, `-` and `_`"
    ))
  }
}

/// Every file of the app named `name`, by its path within the app.
fn app_files(name: &str, platforms: &[Platform], source: &Source) -> Vec<(PathBuf, String)> {
  let fill = |template: &str| {
    template
      .replace("{name}", name)
      .replace("{crate}", &name.replace('-', "_"))
  };
  let mut members = vec!["\"shared\"".to_owned()];
  let mut dependencies = vec![
    format!("{name}-shared = {{ path = \"shared\" }}"),
    source.entry("skene-reactive"),
  ];
  let mut files = vec![
    (
      PathBuf::from(".gitignore"),
      include_str!("../templates/app/gitignore.tmpl").to_owned(),
    ),
    (
      PathBuf::from("shared/Cargo.toml"),
      fill(include_str!("../templates/app/shared-Cargo.toml.tmpl")),
    ),
    (
      PathBuf::from("shared/src/lib.rs"),
      fill(include_str!("../templates/app/shared-lib.rs.tmpl")),
    ),
  ];

  for platform in platforms {
    let directory = platform.directory();
    let (manifest, library) = platform.templates();

    members.push(format!("\"{directory}\""));
    dependencies.extend(platform.crates().map(|name| source.entry(name)));
    files.push((PathBuf::from(directory).join("Cargo.toml"), fill(manifest)));
    files.push((PathBuf::from(directory).join("src/lib.rs"), fill(library)));
    files.push((
      PathBuf::from(directory).join("skene.toml"),
      include_str!("../templates/app/skene.toml.tmpl").to_owned(),
    ));
  }

  dependencies.sort();

  let workspace = include_str!("../templates/app/Cargo.toml.tmpl")
    .replace("{members}", &members.join(", "))
    .replace("{dependencies}", &dependencies.join("\n"));

  files.insert(0, (PathBuf::from("Cargo.toml"), workspace));

  files
}

#[cfg(test)]
mod tests {
  use std::path::{Path, PathBuf};

  use super::{Platform, Source, app_files, check_name};

  /// The contents of the file at `path` among `files`.
  fn file<'a>(files: &'a [(PathBuf, String)], path: &str) -> &'a str {
    &files
      .iter()
      .find(|(file, _)| file == Path::new(path))
      .unwrap_or_else(|| panic!("{path} is generated"))
      .1
  }

  /// An app holds its shared crate and a shell per platform named, each depending on the
  /// shared crate and on its platform's skene crates alone.
  #[test]
  fn an_app_has_a_shell_per_platform() {
    let files = app_files("my-app", &[Platform::Web], &Source::Registry);
    let paths: Vec<_> = files
      .iter()
      .map(|(path, _)| path.display().to_string())
      .collect();

    assert_eq!(
      paths,
      [
        "Cargo.toml",
        ".gitignore",
        "shared/Cargo.toml",
        "shared/src/lib.rs",
        "web/Cargo.toml",
        "web/src/lib.rs",
        "web/skene.toml",
      ]
    );

    let workspace = file(&files, "Cargo.toml");

    assert!(
      workspace.contains("members = [\"shared\", \"web\"]"),
      "{workspace}"
    );
    assert!(workspace.contains("skene-web = \"0.1\""), "{workspace}");
    assert!(!workspace.contains("skene-terminal"), "{workspace}");
    assert!(
      file(&files, "web/src/lib.rs").contains("use my_app_shared::Counter;"),
      "the shell names the shared crate by its Rust name"
    );
    assert!(file(&files, "web/Cargo.toml").contains("name = \"my-app-web\""));
  }

  /// A checkout source names each skene crate by its path in the checkout.
  #[test]
  fn a_checkout_source_takes_paths() {
    let files = app_files(
      "demo",
      &[Platform::Terminal],
      &Source::Checkout(PathBuf::from("/src/skene")),
    );
    let workspace = file(&files, "Cargo.toml");

    assert!(
      workspace.contains("skene-terminal = { path = \"/src/skene/packages/terminal\" }"),
      "{workspace}"
    );
    assert!(
      workspace
        .contains("skene-simple-terminal = { path = \"/src/skene/packages/simple-terminal\" }"),
      "{workspace}"
    );
  }

  /// A name is a crate name or nothing.
  #[test]
  fn a_name_makes_a_crate_name() {
    assert!(check_name("my-app").is_ok());
    assert!(check_name("app_2").is_ok());
    assert!(check_name("My-App").is_err());
    assert!(check_name("2app").is_err());
    assert!(check_name("my app").is_err());
  }
}
