//! `skene new`: a project from a template.

use std::{
  fs,
  path::{Path, PathBuf},
};

/// A kind of project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
  /// An app: a crate of logic and state the shells share, and a shell per platform.
  App,
  /// A component library: a core of colors, sizes and views, and a crate per platform
  /// exposing the views as its own.
  Library,
}

impl Kind {
  /// Creates the project at `path`, named for its last component: the common crate,
  /// and a crate for each of `platforms`, the skene crates taken from `source`.
  pub(crate) fn create(
    self,
    path: &Path,
    platforms: &[Platform],
    source: &Source,
  ) -> Result<(), String> {
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

    for (file, contents) in self.files(name, &platforms, source) {
      let file = path.join(file);

      if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
      }

      fs::write(&file, contents).map_err(|error| format!("{}: {error}", file.display()))?;
    }

    Ok(())
  }

  /// The common crate's directory.
  const fn common(self) -> &'static str {
    match self {
      Self::App => "shared",
      Self::Library => "core",
    }
  }

  /// The workspace's dependency entry for the common crate of the project named `name`.
  fn common_entry(self, name: &str) -> String {
    match self {
      Self::App => format!("{name}-shared = {{ path = \"shared\" }}"),
      Self::Library => format!("{name} = {{ path = \"core\", version = \"0.1.0\" }}"),
    }
  }

  /// The skene crates the common crate depends on.
  const fn common_crates(self) -> &'static [&'static str] {
    match self {
      Self::App => &["skene-reactive"],
      Self::Library => &["skene-foundation"],
    }
  }

  /// The project's `.gitignore`, and the common crate's manifest and library, as
  /// templates.
  const fn templates(self) -> [&'static str; 3] {
    match self {
      Self::App => [
        include_str!("../templates/app/gitignore.tmpl"),
        include_str!("../templates/app/shared-Cargo.toml.tmpl"),
        include_str!("../templates/app/shared-lib.rs.tmpl"),
      ],
      Self::Library => [
        include_str!("../templates/lib/gitignore.tmpl"),
        include_str!("../templates/lib/core-Cargo.toml.tmpl"),
        include_str!("../templates/lib/core-lib.rs.tmpl"),
      ],
    }
  }

  /// Every file of the project named `name`, by its path within the project.
  fn files(self, name: &str, platforms: &[Platform], source: &Source) -> Vec<(PathBuf, String)> {
    let type_name = type_name(name);
    let fill = |template: &str| {
      template
        .replace("{name}", name)
        .replace("{crate}", &name.replace('-', "_"))
        .replace("{Type}", &type_name)
    };
    let common = self.common();
    let [gitignore, manifest, library] = self.templates();
    let mut members = vec![format!("\"{common}\"")];
    let mut dependencies = vec![self.common_entry(name)];
    let mut files = vec![
      (PathBuf::from(".gitignore"), gitignore.to_owned()),
      (PathBuf::from(common).join("Cargo.toml"), fill(manifest)),
      (PathBuf::from(common).join("src/lib.rs"), fill(library)),
    ];

    dependencies.extend(self.common_crates().iter().map(|name| source.entry(name)));

    for platform in platforms {
      let directory = platform.directory();
      let (manifest, library) = platform.templates(self);

      members.push(format!("\"{directory}\""));
      dependencies.extend(platform.crates(self).iter().map(|name| source.entry(name)));
      files.push((PathBuf::from(directory).join("Cargo.toml"), fill(manifest)));
      files.push((PathBuf::from(directory).join("src/lib.rs"), fill(library)));

      if self == Self::App {
        files.push((
          PathBuf::from(directory).join("skene.toml"),
          include_str!("../templates/app/skene.toml.tmpl").to_owned(),
        ));
      }
    }

    dependencies.sort();
    dependencies.dedup();

    let workspace = include_str!("../templates/Cargo.toml.tmpl")
      .replace("{members}", &members.join(", "))
      .replace("{dependencies}", &dependencies.join("\n"));

    files.insert(0, (PathBuf::from("Cargo.toml"), workspace));

    files
  }
}

/// A platform a project is given a crate for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Platform {
  /// The web: the browser, and a server render.
  Web,
  /// The terminal.
  Terminal,
}

impl Platform {
  /// The platform crate's directory, and the suffix of its name.
  const fn directory(self) -> &'static str {
    match self {
      Self::Web => "web",
      Self::Terminal => "terminal",
    }
  }

  /// The skene crates the platform crate of a project of `kind` depends on.
  const fn crates(self, kind: Kind) -> &'static [&'static str] {
    match (kind, self) {
      (Kind::App, Self::Web) => &["skene-simple-web", "skene-web"],
      (Kind::App, Self::Terminal) => &["skene-simple-terminal", "skene-terminal"],
      (Kind::Library, Self::Web) => &["skene-web"],
      (Kind::Library, Self::Terminal) => &["skene-terminal"],
    }
  }

  /// The platform crate's manifest and library in a project of `kind`, as templates.
  const fn templates(self, kind: Kind) -> (&'static str, &'static str) {
    match (kind, self) {
      (Kind::App, Self::Web) => (
        include_str!("../templates/app/web-Cargo.toml.tmpl"),
        include_str!("../templates/app/web-lib.rs.tmpl"),
      ),
      (Kind::App, Self::Terminal) => (
        include_str!("../templates/app/terminal-Cargo.toml.tmpl"),
        include_str!("../templates/app/terminal-lib.rs.tmpl"),
      ),
      (Kind::Library, Self::Web) => (
        include_str!("../templates/lib/web-Cargo.toml.tmpl"),
        include_str!("../templates/lib/web-lib.rs.tmpl"),
      ),
      (Kind::Library, Self::Terminal) => (
        include_str!("../templates/lib/terminal-Cargo.toml.tmpl"),
        include_str!("../templates/lib/terminal-lib.rs.tmpl"),
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

/// The prefix of the type names in a project named `name`: each of its words
/// capitalized, `my-lib` naming `MyLib`.
fn type_name(name: &str) -> String {
  name
    .split(['-', '_'])
    .flat_map(|word| {
      let mut characters = word.chars();

      characters
        .next()
        .map(|first| first.to_ascii_uppercase())
        .into_iter()
        .chain(characters)
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use std::path::{Path, PathBuf};

  use super::{Kind, Platform, Source, check_name, type_name};

  /// The contents of the file at `path` among `files`.
  fn file<'a>(files: &'a [(PathBuf, String)], path: &str) -> &'a str {
    &files
      .iter()
      .find(|(file, _)| file == Path::new(path))
      .unwrap_or_else(|| panic!("{path} is generated"))
      .1
  }

  /// The paths of `files`, in order.
  fn paths(files: &[(PathBuf, String)]) -> Vec<String> {
    files
      .iter()
      .map(|(path, _)| path.display().to_string())
      .collect()
  }

  /// An app holds its shared crate and a shell per platform named, each depending on the
  /// shared crate and on its platform's skene crates alone.
  #[test]
  fn an_app_has_a_shell_per_platform() {
    let files = Kind::App.files("my-app", &[Platform::Web], &Source::Registry);

    assert_eq!(
      paths(&files),
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

  /// A library holds its core and a crate per platform named, its types named for the
  /// library.
  #[test]
  fn a_library_has_a_crate_per_platform() {
    let files = Kind::Library.files(
      "acme-ui",
      &[Platform::Terminal, Platform::Web],
      &Source::Registry,
    );

    assert_eq!(
      paths(&files),
      [
        "Cargo.toml",
        ".gitignore",
        "core/Cargo.toml",
        "core/src/lib.rs",
        "terminal/Cargo.toml",
        "terminal/src/lib.rs",
        "web/Cargo.toml",
        "web/src/lib.rs",
      ]
    );

    let workspace = file(&files, "Cargo.toml");

    assert!(
      workspace.contains("members = [\"core\", \"terminal\", \"web\"]"),
      "{workspace}"
    );
    assert!(
      workspace.contains("acme-ui = { path = \"core\", version = \"0.1.0\" }"),
      "{workspace}"
    );
    assert!(!workspace.contains("skene-simple"), "{workspace}");
    assert!(file(&files, "core/src/lib.rs").contains("pub struct AcmeUiBadge<P: Realization>"));
    assert!(
      file(&files, "web/src/lib.rs")
        .contains("pub struct AcmeUiBadge(acme_ui::AcmeUiBadge<Active>);")
    );
    assert!(file(&files, "terminal/Cargo.toml").contains("name = \"acme-ui-terminal\""));
  }

  /// A checkout source names each skene crate by its path in the checkout.
  #[test]
  fn a_checkout_source_takes_paths() {
    let files = Kind::App.files(
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

  /// A type name capitalizes each word of the project's name.
  #[test]
  fn a_type_name_capitalizes_each_word() {
    assert_eq!(type_name("acme"), "Acme");
    assert_eq!(type_name("my-lib"), "MyLib");
    assert_eq!(type_name("ui_kit-2"), "UiKit2");
  }
}
