//! `skene new <name>`: an app crate plain cargo builds, runs and tests for its first realized
//! platform with no other setup.
//!
//! It writes:
//! - `Cargo.toml`: the package, on `skene-gui`, a workspace of its own, its development builds
//!   optimized (Skene draws on the CPU);
//! - `Cargo.lock`: the versions skene-gui's own workspace locks, where it has a lock;
//! - `skene.toml`: the platforms the app declares, which `skene check` checks;
//! - `rust-toolchain.toml`: the Rust skene-gui needs;
//! - `.cargo/config.toml`: the platform plain cargo builds for (decision 2): `[build] target`
//!   its real triple, `--cfg skene_platform="<platform>"` with its `--check-cfg` in
//!   `[target.'cfg(all())'] rustflags` (which trybuild keeps) and in `build.rustdocflags` (which
//!   doctests see), and on Linux the display runner every test and run goes through;
//! - `src/lib.rs` (the app), `src/main.rs` (its `#[skene_gui::main]` entry) and
//!   `tests/app.rs` (a test on the real platform).

use std::{env, fs, path::Path};

use crate::{Platform, flags};

/// Where skene-gui is, for the new app's path dependency: this prototype's own.
const SKENE_GUI: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../gui");

/// Makes the app `name` in the current directory.
pub fn new(options: &[String]) -> Result<(), String> {
  let mut name = None;
  let mut platforms = vec!["linux".to_owned()];
  let mut options = options.iter();

  while let Some(option) = options.next() {
    match option.as_str() {
      "--platforms" => {
        platforms = options
          .next()
          .ok_or("`--platforms` takes a list: linux,web,...")?
          .split(',')
          .map(|platform| platform.trim().to_owned())
          .collect();
      }
      flag if flag.starts_with('-') => return Err(format!("unknown option `{flag}`")),
      given if name.is_none() => name = Some(given.to_owned()),
      extra => return Err(format!("unexpected `{extra}`")),
    }
  }

  let name = name.ok_or("name the app: skene new <name>")?;

  if name.is_empty()
    || !name.chars().all(|character| {
      character.is_ascii_lowercase()
        || character.is_ascii_digit()
        || character == '-'
        || character == '_'
    })
  {
    return Err(format!(
      "`{name}` is not a package name: lowercase letters, digits, `-` and `_`"
    ));
  }

  let platforms: Vec<Platform> = platforms
    .iter()
    .map(|name| Platform::named(name))
    .collect::<Result<_, _>>()?;
  let configured = platforms
    .iter()
    .find(|platform| platform.realized)
    .copied()
    .unwrap_or(platforms[0]);
  let dir = env::current_dir()
    .map_err(|error| error.to_string())?
    .join(&name);

  if dir.exists() {
    return Err(format!("{} exists already", dir.display()));
  }

  let skene_gui = fs::canonicalize(SKENE_GUI).map_err(|error| format!("{SKENE_GUI}: {error}"))?;
  let runner = env::current_exe()
    .map_err(|error| error.to_string())?
    .with_file_name("skene-runner");

  if configured.name == "linux" {
    runner_exists(&runner)?;
  }
  let library = name.replace('-', "_");
  let triple = configured.triple()?;

  write(
    &dir.join("Cargo.toml"),
    &format!(
      r#"[package]
name = "{name}"
edition = "2024"
publish = false
version = "0.1.0"

[dependencies]
skene-gui = {{ path = "{}" }}

# The app is a workspace of its own.
[workspace]

# Skene draws every frame on the CPU: its crates and the app's dependencies are optimized in
# development builds too, and line tables are all the debug information kept.
[profile.dev]
debug = "line-tables-only"
opt-level = 1

[profile.dev.package."*"]
debug = false
opt-level = 2
{}"#,
      skene_gui.display(),
      patches(&skene_gui)?
    ),
  )?;

  // The versions skene-gui was built and tested with, where its workspace has them.
  if let Some(lock) = skene_gui.parent().map(|root| root.join("Cargo.lock"))
    && lock.exists()
  {
    fs::copy(&lock, dir.join("Cargo.lock"))
      .map_err(|error| format!("{}: {error}", lock.display()))?;
  }
  write(
    &dir.join("skene.toml"),
    &format!(
      "# The platforms the app is for: `skene check` checks each.\nplatforms = [{}]\n",
      platforms
        .iter()
        .map(|platform| format!("\"{}\"", platform.name))
        .collect::<Vec<_>>()
        .join(", ")
    ),
  )?;
  write(
    &dir.join(".cargo/config.toml"),
    &config(configured, &triple, &runner),
  )?;
  write(&dir.join(".gitignore"), "/target\n")?;
  write(
    &dir.join("rust-toolchain.toml"),
    "# The Rust skene-gui needs.\n[toolchain]\nchannel = \"1.99.0\"\n",
  )?;
  write(&dir.join("src/lib.rs"), &LIBRARY.replace("NAME", &name))?;
  write(&dir.join("src/main.rs"), &MAIN.replace("LIBRARY", &library))?;
  write(
    &dir.join("tests/app.rs"),
    &TEST.replace("LIBRARY", &library),
  )?;

  println!(
    "skene new: {} for {} (plain cargo builds for {} on {triple})",
    dir.display(),
    platforms
      .iter()
      .map(|platform| platform.name)
      .collect::<Vec<_>>()
      .join(", "),
    configured.name
  );

  if !configured.realized {
    println!(
      "skene new: {} is not realized in this prototype, so its build fails saying so",
      configured.name
    );
  }

  Ok(())
}

/// Refuses where the display runner the configuration would name is not an executable file:
/// every test and run of the app would fail on it.
fn runner_exists(runner: &Path) -> Result<(), String> {
  use std::os::unix::fs::PermissionsExt as _;

  let metadata = fs::metadata(runner).map_err(|error| {
    format!(
      "the display runner {} is not there ({error}): install `skene-runner` beside `skene`, then run skene new again",
      runner.display()
    )
  })?;

  if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
    return Err(format!(
      "the display runner {} is not an executable file: install `skene-runner` beside `skene`, then run skene new again",
      runner.display()
    ));
  }

  Ok(())
}

/// The `[patch.crates-io]` section of skene-gui's workspace, its paths made absolute: the
/// crates Skene carries its own changes to (AccessKit's Unix adapter, the XIM client) until
/// they are published under Skene's names; empty where the workspace patches nothing.
fn patches(skene_gui: &Path) -> Result<String, String> {
  let Some(root) = skene_gui.parent() else {
    return Ok(String::new());
  };
  let Ok(manifest) = fs::read_to_string(root.join("Cargo.toml")) else {
    return Ok(String::new());
  };
  let manifest: toml::Table = manifest
    .parse()
    .map_err(|error| format!("{}: {error}", root.join("Cargo.toml").display()))?;
  let Some(patches) = manifest
    .get("patch")
    .and_then(|patch| patch.get("crates-io"))
    .and_then(toml::Value::as_table)
  else {
    return Ok(String::new());
  };
  let mut section = String::from(
    "\n# The crates Skene carries its own changes to, as skene-gui's workspace patches them.\n[patch.crates-io]\n",
  );

  for (name, patch) in patches {
    let Some(path) = patch.get("path").and_then(toml::Value::as_str) else {
      continue;
    };

    section.push_str(&format!(
      "{name} = {{ path = \"{}\" }}\n",
      root.join(path).display()
    ));
  }

  Ok(section)
}

/// The configuration plain cargo builds `platform` with.
fn config(platform: Platform, triple: &str, runner: &Path) -> String {
  let flags = format!(
    r#"[
  "--cfg", 'skene_platform="{}"',
  "--check-cfg", '{}',
]"#,
    platform.name,
    flags::CHECK_CFG
  );
  let runner = match platform.name {
    "linux" => format!(
      "\n# Every test (on a private display, session bus and accessibility bus) and every run (on\n# the user's display, a private one where there is none) goes through the display runner.\n[target.{triple}]\nrunner = \"{}\"\n",
      runner.display()
    ),
    _ => String::new(),
  };

  format!(
    "# Written by `skene new`: the platform plain cargo builds for, `{name}`.\n[build]\ntarget = \"{triple}\"\nrustdocflags = {flags}\n\n[target.'cfg(all())']\nrustflags = {flags}\n{runner}",
    name = platform.name
  )
}

/// Writes `contents` to `path`, making its directory.
fn write(path: &Path, contents: &str) -> Result<(), String> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
  }

  fs::write(path, contents).map_err(|error| format!("{}: {error}", path.display()))
}

const LIBRARY: &str = r#"//! NAME.

use skene_gui::prelude::*;

/// The app.
pub fn app() -> impl View {
  let name = Signal::new(String::new());

  Container::new().set_padding(24).set_child(
    Column::new()
      .set_spacing(12)
      .add_child(Text::new("NAME").set_font_size(28))
      .add_child(TextField::new("Name").bind_text(name))
      .add_child(Text::derive(move || match name.get().trim() {
        "" => "Who's there?".to_owned(),
        name => format!("Hello, {name}!"),
      })),
  )
}
"#;

const MAIN: &str = r#"//! The app's entry.

/// The app's root.
#[skene_gui::main]
fn app() -> impl skene_gui::View {
  LIBRARY::app()
}
"#;

const TEST: &str = r#"//! The app on the real platform: found as a reader finds it, acted on as a user acts.

use skene_gui::testing::{self, Role};

#[skene_gui::test]
fn typing_a_name_greets_it() {
  let screen = testing::render(LIBRARY::app);

  screen.find_by_text("Who's there?");
  screen.find_by_role(Role::TextField, "Name").type_text("Ada");
  screen.find_by_text("Hello, Ada!");
}
"#;
