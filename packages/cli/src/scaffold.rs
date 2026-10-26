//! `skene new`: a project from a template, which plain cargo builds for the first
//! platform it is given.

use std::{
  env,
  fs,
  path::{Path, PathBuf},
};

/// A kind of project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
  /// An app: a crate of logic and state the shells share, and a shell per platform.
  App,
  /// A component library: a core of colors, sizes and logic, and a crate per platform
  /// with its presets.
  Library,
}

impl Kind {
  /// Creates the project at `path`, named for its last component: the common crate,
  /// and a crate for each of `platforms`, the skene crates taken from `source`, plain
  /// cargo building it for the first of `platforms`. Refuses where that platform's
  /// tests run through a program `installed` does not find.
  pub(crate) fn create(
    self,
    path: &Path,
    platforms: &[Platform],
    source: &Source,
    installed: impl Fn(&str) -> bool,
  ) -> Result<(), String> {
    let name = path
      .file_name()
      .and_then(|name| name.to_str())
      .ok_or_else(|| format!("{}: names no directory", path.display()))?;

    check_name(name)?;

    if path.exists() {
      return Err(format!("{} already exists", path.display()));
    }

    let configured = *platforms.first().ok_or("name a platform")?;

    if let Some(runner) = configured.runner()
      && !installed(runner)
    {
      return Err(format!(
        "`{runner}`, which cargo runs the {} build's tests through, is not installed: \
         install it (`cargo install wasm-bindgen-cli`, at the version of `wasm-bindgen` the \
         project locks), then run skene new again",
        configured.cfg()
      ));
    }

    let mut platforms = platforms.to_vec();

    platforms.sort_by_key(|platform| platform.directory());
    platforms.dedup();

    for (file, contents) in self.files(name, &platforms, configured, source) {
      let file = path.join(file);

      if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
      }

      fs::write(&file, contents).map_err(|error| format!("{}: {error}", file.display()))?;
    }

    if let Some(lock) = source.lock() {
      fs::copy(&lock, path.join("Cargo.lock"))
        .map_err(|error| format!("{}: {error}", lock.display()))?;
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

  /// Every file of the project named `name`, by its path within the project, plain
  /// cargo building it for `configured`.
  fn files(
    self,
    name: &str,
    platforms: &[Platform],
    configured: Platform,
    source: &Source,
  ) -> Vec<(PathBuf, String)> {
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
      .replace("{dependencies}", &dependencies.join("\n"))
      .replace("{patches}", &source.patches());

    files.insert(0, (PathBuf::from("Cargo.toml"), workspace));
    files.insert(
      1,
      (PathBuf::from(".cargo/config.toml"), configured.config()),
    );

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

  /// The value of `skene_platform` naming the platform.
  const fn cfg(self) -> &'static str {
    match self {
      Self::Web => "web",
      Self::Terminal => "terminal",
    }
  }

  /// The target triple plain cargo builds the platform for.
  const fn triple(self) -> &'static str {
    match self {
      Self::Web => "wasm32-unknown-unknown",
      Self::Terminal => "host-tuple",
    }
  }

  /// The program cargo runs the platform's tests through, where it needs one.
  const fn runner(self) -> Option<&'static str> {
    match self {
      Self::Web => Some("wasm-bindgen-test-runner"),
      Self::Terminal => None,
    }
  }

  /// The `.cargo/config.toml` naming the platform for plain cargo: its triple as the
  /// build's target, its cfg in the target's rustflags and in rustdoc's, and the
  /// runner its tests need.
  fn config(self) -> String {
    let flags = format!("[\"--cfg\", 'skene_platform=\"{}\"']", self.cfg());
    let runner = match self.runner() {
      Some(runner) => format!(
        "\n# Every test of the {} build runs through its test runner.\n[target.{}]\nrunner = \"{runner}\"\n",
        self.cfg(),
        self.triple()
      ),
      None => String::new(),
    };

    include_str!("../templates/config.toml.tmpl")
      .replace("{platform}", self.cfg())
      .replace("{triple}", self.triple())
      .replace("{flags}", &flags)
      .replace("{runner}", &runner)
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
  /// A skene checkout.
  Checkout {
    /// The checkout's root.
    root: PathBuf,
    /// The crates its workspace patches, each with the directory of its patched copy.
    patches: Vec<(String, PathBuf)>,
  },
}

impl Source {
  /// The checkout at `root`, with the patches its workspace's manifest makes.
  pub(crate) fn checkout(root: &Path) -> Result<Self, String> {
    let root = root
      .canonicalize()
      .map_err(|error| format!("{}: {error}", root.display()))?;
    let path = root.join("Cargo.toml");
    let manifest = fs::read_to_string(&path)
      .map_err(|error| format!("{}: {error}", path.display()))?
      .parse::<toml::Table>()
      .map_err(|error| format!("{}: {error}", path.display()))?;
    let patches = manifest
      .get("patch")
      .and_then(|patch| patch.get("crates-io"))
      .and_then(toml::Value::as_table)
      .into_iter()
      .flatten()
      .filter_map(|(name, patch)| {
        let directory = patch.get("path")?.as_str()?;

        Some((name.clone(), root.join(directory)))
      })
      .collect();

    Ok(Self::Checkout { root, patches })
  }

  /// The project's `[patch.crates-io]` section: the checkout's patches, so the project
  /// builds the crates the checkout carries its own changes to as the checkout does.
  fn patches(&self) -> String {
    match self {
      Self::Checkout { patches, .. } if !patches.is_empty() => {
        let mut section = String::from(
          "\n# The crates the skene checkout carries its own changes to, patched as it patches them.\n[patch.crates-io]\n",
        );

        section.extend(
          patches.iter().map(|(name, directory)| {
            format!("{name} = {{ path = \"{}\" }}\n", directory.display())
          }),
        );

        section
      }
      Self::Registry | Self::Checkout { .. } => String::new(),
    }
  }

  /// The lock file the project starts from: the checkout's, so it builds against the
  /// versions the checkout was built and tested with.
  fn lock(&self) -> Option<PathBuf> {
    match self {
      Self::Registry => None,
      Self::Checkout { root, .. } => Some(root.join("Cargo.lock")).filter(|lock| lock.is_file()),
    }
  }

  /// The dependency entry naming `name`.
  fn entry(&self, name: &str) -> String {
    match self {
      Self::Registry => format!(
        "{name} = \"{}.{}\"",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR")
      ),
      Self::Checkout { root, .. } => {
        let directory = name.strip_prefix("skene-").unwrap_or(name);

        format!(
          "{name} = {{ path = \"{}\" }}",
          root.join("packages").join(directory).display()
        )
      }
    }
  }
}

/// Whether `program` is a file in a directory of the `PATH`.
pub(crate) fn on_path(program: &str) -> bool {
  env::var_os("PATH").is_some_and(|path| {
    env::split_paths(&path).any(|directory| {
      directory
        .join(program)
        .with_extension(env::consts::EXE_EXTENSION)
        .is_file()
    })
  })
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
  use std::{
    fs,
    path::{Path, PathBuf},
  };

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

  /// A fresh directory for a test named `name`, gone first if a run left it.
  fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("skene-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);

    directory
  }

  /// An app holds its shared crate and a shell per platform named, each depending on the
  /// shared crate and on its platform's skene crates alone, each shell gated on its
  /// platform, every crate declaring the cfgs naming the build's platform, and the
  /// configuration naming the first platform for plain cargo.
  #[test]
  fn an_app_has_a_shell_per_platform() {
    let files = Kind::App.files(
      "my-app",
      &[Platform::Terminal, Platform::Web],
      Platform::Web,
      &Source::Registry,
    );

    assert_eq!(
      paths(&files),
      [
        "Cargo.toml",
        ".cargo/config.toml",
        ".gitignore",
        "shared/Cargo.toml",
        "shared/src/lib.rs",
        "terminal/Cargo.toml",
        "terminal/src/lib.rs",
        "terminal/skene.toml",
        "web/Cargo.toml",
        "web/src/lib.rs",
        "web/skene.toml",
      ]
    );

    let workspace = file(&files, "Cargo.toml");

    assert!(
      workspace.contains("members = [\"shared\", \"terminal\", \"web\"]"),
      "{workspace}"
    );
    assert!(
      workspace.contains("my-app-shared = { path = \"shared\" }"),
      "{workspace}"
    );
    assert!(
      workspace.contains("skene-reactive = \"0.1\""),
      "{workspace}"
    );
    assert!(workspace.contains("skene-web = \"0.1\""), "{workspace}");
    assert!(
      workspace.contains("skene-simple-terminal = \"0.1\""),
      "{workspace}"
    );
    assert!(workspace.contains("'cfg(skene_server)'"), "{workspace}");
    assert!(
      workspace.contains(r#"'cfg(skene_platform, values("android", "ios", "linux", "macos", "terminal", "web", "windows"))'"#),
      "{workspace}"
    );
    assert!(!workspace.contains("[patch"), "{workspace}");
    assert_eq!(file(&files, ".cargo/config.toml"), Platform::Web.config());
    assert_eq!(file(&files, ".gitignore"), "/target\n/*/dist\n");
    assert!(
      file(&files, "shared/src/lib.rs").contains("pub struct Counter"),
      "the shared crate holds the state"
    );
    assert!(file(&files, "shared/Cargo.toml").contains("name = \"my-app-shared\""));

    for (shell, platform, facade, other) in [
      ("terminal", "terminal", "skene_terminal", "web"),
      ("web", "web", "skene_web", "terminal"),
    ] {
      let manifest = file(&files, &format!("{shell}/Cargo.toml"));
      let library = file(&files, &format!("{shell}/src/lib.rs"));

      assert!(
        manifest.contains(&format!("name = \"my-app-{shell}\"")),
        "{manifest}"
      );

      for name in [format!("skene-{shell}"), format!("skene-simple-{shell}")] {
        assert!(
          manifest.contains(&format!("{name}.workspace = true")),
          "{manifest}"
        );
      }

      for name in [format!("skene-{other}"), format!("skene-simple-{other}")] {
        assert!(!manifest.contains(&name), "{manifest}");
      }

      assert!(
        manifest.contains("my-app-shared.workspace = true"),
        "{manifest}"
      );
      assert!(
        library.contains(&format!("#![cfg(skene_platform = \"{platform}\")]")),
        "{library}"
      );
      assert!(library.contains("use my_app_shared::Counter;"), "{library}");
      assert!(
        library.contains(&format!("use {facade}::prelude::*;")),
        "{library}"
      );
      assert!(library.contains("#[main]"), "{library}");
      assert!(file(&files, &format!("{shell}/skene.toml")).starts_with('#'));
    }

    assert!(
      ["shared", "terminal", "web"].iter().all(|member| file(
        &files,
        &format!("{member}/Cargo.toml")
      )
      .contains("[lints]\nworkspace = true")),
      "every member declares the platform's cfgs"
    );
  }

  /// A project of one platform depends on that platform's skene crates alone.
  #[test]
  fn a_project_depends_on_its_platforms_alone() {
    for (kind, platform, own, other) in [
      (
        Kind::App,
        Platform::Web,
        &["skene-simple-web", "skene-web"][..],
        &["skene-simple-terminal", "skene-terminal"][..],
      ),
      (
        Kind::App,
        Platform::Terminal,
        &["skene-simple-terminal", "skene-terminal"],
        &["skene-simple-web", "skene-web"],
      ),
      (
        Kind::Library,
        Platform::Web,
        &["skene-web"],
        &["skene-simple", "skene-terminal"],
      ),
      (
        Kind::Library,
        Platform::Terminal,
        &["skene-terminal"],
        &["skene-simple", "skene-web"],
      ),
    ] {
      let files = kind.files("demo", &[platform], platform, &Source::Registry);
      let workspace = file(&files, "Cargo.toml");

      for name in own {
        assert!(
          workspace.contains(&format!("{name} = \"0.1\"")),
          "{workspace}"
        );
      }

      for name in other {
        assert!(!workspace.contains(name), "{workspace}");
      }
    }
  }

  /// A library holds its core and a crate per platform named, its types named for the
  /// library: the core plain data naming no realization, and each platform's crate gated
  /// on its platform, with its presets.
  #[test]
  fn a_library_has_a_crate_per_platform() {
    let files = Kind::Library.files(
      "acme-ui",
      &[Platform::Terminal, Platform::Web],
      Platform::Terminal,
      &Source::Registry,
    );

    assert_eq!(
      paths(&files),
      [
        "Cargo.toml",
        ".cargo/config.toml",
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
    assert!(
      workspace.contains("skene-foundation = \"0.1\""),
      "{workspace}"
    );
    assert!(!workspace.contains("skene-simple"), "{workspace}");
    assert!(workspace.contains("'cfg(skene_server)'"), "{workspace}");
    assert_eq!(
      file(&files, ".cargo/config.toml"),
      Platform::Terminal.config()
    );
    assert_eq!(file(&files, ".gitignore"), "/target\n");
    assert!(file(&files, "core/Cargo.toml").contains("name = \"acme-ui\""));
    assert!(
      ["skene-terminal", "skene-web"]
        .iter()
        .all(|facade| !file(&files, "core/Cargo.toml").contains(facade)),
      "the core names no facade"
    );

    let core = file(&files, "core/src/lib.rs");

    assert!(
      core.contains("pub struct AcmeUiPalette<N: Scalar>"),
      "{core}"
    );
    assert!(core.contains("pub trait AcmeUiUnit: Scalar"), "{core}");
    assert!(
      !core.contains("Realization") && !core.contains("View"),
      "{core}"
    );

    for (directory, platform) in [("terminal", "terminal"), ("web", "web")] {
      let library = file(&files, &format!("{directory}/src/lib.rs"));

      assert!(
        file(&files, &format!("{directory}/Cargo.toml"))
          .contains(&format!("name = \"acme-ui-{directory}\"")),
      );
      assert!(
        library.contains(&format!("#![cfg(skene_platform = \"{platform}\")]")),
        "{library}"
      );
      assert!(library.contains("pub enum AcmeUiBadge {}"), "{library}");
      assert!(
        library.contains("pub fn new(label: impl Into<String>) -> Container"),
        "{library}"
      );
      assert!(
        library.contains("pub fn derive(label: impl Fn() -> String + 'static) -> Container"),
        "{library}"
      );
      assert!(
        library.contains("pub fn styled(label: Text) -> Container"),
        "{library}"
      );
      assert!(
        library.contains(&format!(
          "pub type AcmeUiPalette = acme_ui::AcmeUiPalette<{}>;",
          if platform == "web" { "f32" } else { "i32" }
        )),
        "{library}"
      );
      assert!(
        !file(&files, &format!("{directory}/Cargo.toml")).contains("skene-foundation"),
        "a preset names its facade alone"
      );
    }

    assert!(
      ["core", "terminal", "web"].iter().all(|member| file(
        &files,
        &format!("{member}/Cargo.toml")
      )
      .contains("[lints]\nworkspace = true")),
      "every member declares the platform's cfgs"
    );
  }

  /// The configuration names the platform for plain cargo: its triple as the build's
  /// target, its cfg in the target's rustflags and rustdoc's, and the browser's runner.
  #[test]
  fn the_configuration_names_one_platform() {
    let web = Platform::Web.config();
    let terminal = Platform::Terminal.config();

    for (config, triple, cfg) in [
      (&web, "wasm32-unknown-unknown", "web"),
      (&terminal, "host-tuple", "terminal"),
    ] {
      let table: toml::Table = config.parse().expect("the configuration is TOML");
      let flags = toml::Value::Array(vec![
        "--cfg".into(),
        format!("skene_platform=\"{cfg}\"").into(),
      ]);

      assert_eq!(table["build"]["target"].as_str(), Some(triple), "{config}");
      assert_eq!(table["build"]["rustdocflags"], flags, "{config}");
      assert_eq!(
        table["target"]["cfg(all())"]["rustflags"], flags,
        "{config}"
      );
    }

    let web: toml::Table = web.parse().expect("TOML");
    let terminal: toml::Table = terminal.parse().expect("TOML");

    assert_eq!(
      web["target"]["wasm32-unknown-unknown"]["runner"].as_str(),
      Some("wasm-bindgen-test-runner")
    );
    assert_eq!(terminal["target"].as_table().map(toml::Table::len), Some(1));
  }

  /// A project is written where its runner is installed, the first platform named the
  /// configured one, and refused, writing nothing, where it is not.
  #[test]
  fn a_project_needs_its_platforms_runner() {
    let directory = scratch("runner");
    let app = directory.join("demo");

    assert!(
      Kind::App
        .create(
          &app,
          &[Platform::Web, Platform::Terminal],
          &Source::Registry,
          |_| false
        )
        .is_err()
    );
    assert!(!app.exists(), "a refused project writes nothing");

    Kind::App
      .create(
        &app,
        &[Platform::Terminal, Platform::Web],
        &Source::Registry,
        |_| false,
      )
      .expect("the terminal needs no runner");

    assert_eq!(
      fs::read_to_string(app.join(".cargo/config.toml")).expect("written"),
      Platform::Terminal.config()
    );
    assert!(app.join("web/src/lib.rs").is_file());
    assert!(!app.join("Cargo.lock").exists(), "the registry has no lock");

    let library = directory.join("acme");

    Kind::Library
      .create(&library, &[Platform::Web], &Source::Registry, |runner| {
        runner == "wasm-bindgen-test-runner"
      })
      .expect("the runner is found");

    assert_eq!(
      fs::read_to_string(library.join(".cargo/config.toml")).expect("written"),
      Platform::Web.config()
    );

    fs::remove_dir_all(&directory).expect("removed");
  }

  /// A checkout source names each skene crate by its path in the checkout, patches what
  /// the checkout patches and starts from its lock.
  #[test]
  fn a_checkout_source_takes_paths() {
    let source = Source::Checkout {
      root: PathBuf::from("/src/skene"),
      patches: vec![(
        "parley".to_owned(),
        PathBuf::from("/src/skene/vendor/parley"),
      )],
    };
    let files = Kind::App.files("demo", &[Platform::Terminal], Platform::Terminal, &source);
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
    assert!(
      workspace.contains("[patch.crates-io]\nparley = { path = \"/src/skene/vendor/parley\" }\n"),
      "{workspace}"
    );

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let checkout = Source::checkout(&root).expect("this checkout");
    let Source::Checkout { root, patches } = &checkout
    else {
      panic!("a checkout");
    };

    assert!(
      patches
        .iter()
        .any(|(name, directory)| name == "parley" && *directory == root.join("vendor/parley")),
      "{patches:?}"
    );
    assert_eq!(checkout.lock(), Some(root.join("Cargo.lock")));
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
