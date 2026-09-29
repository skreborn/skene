//! The public API snapshots: each library crate's surface as `cargo public-api` lists
//! it, read on every leaf whose surface differs, and kept under `api/` so a change to
//! the surface shows in review.
//!
//! The gate's `api` step fails when a snapshot differs from the tree's surface, and
//! `cargo xtask api` rewrites the snapshots from the tree.

use std::{
  collections::BTreeSet,
  fmt::Write as _,
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use crate::{Gate, Log, Outcome, SERVER_CFG, WASM, describe};

/// The toolchain every surface is read with; a snapshot holds under this toolchain's
/// rustdoc alone.
const TOOLCHAIN: &str = "nightly-2026-08-13";

/// The `cargo public-api` release every surface is listed by; a snapshot holds under
/// this release's listing alone.
const VERSION: &str = "0.52.0";

/// How many added and removed lines the gate prints for each snapshot that differs.
const SHOWN: usize = 10;

/// Where a surface is read.
#[derive(Clone, Copy)]
enum Leaf {
  /// Natively, with no platform named.
  Host,
  /// Natively, with the web's server leaf named.
  Server,
  /// On the browser's target.
  Browser,
}

impl Leaf {
  /// The leaf's name, as `skene_leaf` spells it.
  const fn name(self) -> &'static str {
    match self {
      Self::Host => "host",
      Self::Server => "server",
      Self::Browser => "browser",
    }
  }
}

/// A snapshot: the crate, and the leaf its surface is read on.
struct Snapshot {
  /// The crate's package name.
  package: &'static str,
  /// Where its surface is read.
  leaf: Leaf,
}

/// Every snapshot: each library crate on the host, and those whose surface differs by
/// leaf on each of theirs.
const SNAPSHOTS: &[Snapshot] = &[
  Snapshot::new("skene-common", Leaf::Host),
  Snapshot::new("skene-core", Leaf::Host),
  Snapshot::new("skene-gui", Leaf::Host),
  Snapshot::new("skene-gui", Leaf::Server),
  Snapshot::new("skene-gui", Leaf::Browser),
  Snapshot::new("skene-headless", Leaf::Host),
  Snapshot::new("skene-macros", Leaf::Host),
  Snapshot::new("skene-reactive", Leaf::Host),
  Snapshot::new("skene-simple", Leaf::Host),
  Snapshot::new("skene-transport", Leaf::Host),
  Snapshot::new("skene-transport", Leaf::Browser),
  Snapshot::new("skene-tui", Leaf::Host),
  Snapshot::new("skene-web", Leaf::Host),
  Snapshot::new("skene-web", Leaf::Server),
  Snapshot::new("skene-web", Leaf::Browser),
];

impl Snapshot {
  const fn new(package: &'static str, leaf: Leaf) -> Self {
    Self { package, leaf }
  }

  /// The snapshot's file, relative to the workspace.
  fn file(&self) -> PathBuf {
    Path::new("api").join(format!("{}.{}.txt", self.package, self.leaf.name()))
  }

  /// The command listing the crate's surface on its leaf.
  fn command(&self, gate: &Gate) -> Command {
    let mut command = gate.cargo([
      &format!("+{TOOLCHAIN}"),
      "public-api",
      "--package",
      self.package,
      "--omit",
      "blanket-impls,auto-trait-impls",
      "--color",
      "never",
    ]);

    match self.leaf {
      Leaf::Host => {
        command.env("CARGO_TARGET_DIR", gate.target.join("api"));
      }
      Leaf::Server => {
        command
          .env("RUSTFLAGS", SERVER_CFG)
          .env("RUSTDOCFLAGS", SERVER_CFG)
          .env("CARGO_TARGET_DIR", gate.target.join("api-server"));
      }
      Leaf::Browser => {
        command
          .args(["--target", WASM])
          .env("CARGO_TARGET_DIR", gate.target.join("api"));
      }
    }

    command
  }

  /// The crate's surface on its leaf, one item per line; what the listing printed
  /// beside it goes to `log`.
  fn read(&self, gate: &Gate, log: &mut Log) -> Result<String, String> {
    let mut command = self.command(gate);
    let described = describe(&command);
    let output = command
      .output()
      .map_err(|error| format!("{described}: {error}"))?;

    log.note(&format!("$ {described}"));
    log.note(&String::from_utf8_lossy(&output.stderr));

    if !output.status.success() {
      return Err(format!("`{described}` failed"));
    }

    String::from_utf8(output.stdout).map_err(|error| format!("{described}: {error}"))
  }
}

/// Fails unless `cargo public-api` at [`VERSION`] and [`TOOLCHAIN`], with the browser's
/// target, are installed.
fn require(gate: &Gate) -> Outcome {
  let listed = gate
    .cargo(["public-api", "--version"])
    .output()
    .is_ok_and(|output| {
      output.status.success()
        && String::from_utf8_lossy(&output.stdout).trim() == format!("cargo-public-api {VERSION}")
    });

  if !listed {
    return Err(format!(
      "cargo public-api {VERSION} is missing: `cargo install cargo-public-api --version \
       {VERSION} --locked`"
    ));
  }

  let installed = Command::new("rustup")
    .args([&format!("+{TOOLCHAIN}"), "target", "list", "--installed"])
    .output()
    .is_ok_and(|output| {
      output.status.success()
        && String::from_utf8_lossy(&output.stdout)
          .lines()
          .any(|line| line == WASM)
    });

  if installed {
    Ok(())
  }
  else {
    Err(format!(
      "{TOOLCHAIN} with the {WASM} target is missing: `rustup toolchain install {TOOLCHAIN} \
       --profile minimal --target {WASM}`"
    ))
  }
}

/// Checks every snapshot against the tree's surface.
pub(crate) fn check(gate: &Gate, log: &mut Log) -> Outcome {
  require(gate)?;

  let mut differences = Vec::new();

  for snapshot in SNAPSHOTS {
    let listed = snapshot.read(gate, log)?;
    let file = snapshot.file();
    let kept = fs::read_to_string(gate.root.join(&file)).unwrap_or_default();

    if listed != kept {
      differences.push(difference(&file, &kept, &listed));
    }
  }

  if differences.is_empty() {
    Ok(())
  }
  else {
    differences.push("review the surface and write it with `cargo xtask api`".to_owned());

    Err(differences.join("\n"))
  }
}

/// Rewrites every snapshot from the tree's surface, naming each one that changed.
pub(crate) fn write(gate: &Gate, log: &mut Log) -> Outcome {
  require(gate)?;
  fs::create_dir_all(gate.root.join("api")).map_err(|error| format!("api: {error}"))?;

  for snapshot in SNAPSHOTS {
    let listed = snapshot.read(gate, log)?;
    let file = snapshot.file();
    let path = gate.root.join(&file);
    let kept = fs::read_to_string(&path).unwrap_or_default();

    if listed != kept {
      fs::write(&path, &listed).map_err(|error| format!("{}: {error}", file.display()))?;
      println!("{}", difference(&file, &kept, &listed));
    }
  }

  Ok(())
}

/// What changed between the `kept` and `listed` surfaces of `file`: how many items each
/// side alone has, and the first of them.
fn difference(file: &Path, kept: &str, listed: &str) -> String {
  let kept: BTreeSet<&str> = kept.lines().collect();
  let listed: BTreeSet<&str> = listed.lines().collect();
  let added: Vec<&str> = listed.difference(&kept).copied().collect();
  let removed: Vec<&str> = kept.difference(&listed).copied().collect();
  let mut text = format!(
    "{}: {} added, {} removed",
    file.display(),
    added.len(),
    removed.len()
  );

  for line in added.iter().take(SHOWN) {
    let _ = write!(text, "\n  + {line}");
  }

  for line in removed.iter().take(SHOWN) {
    let _ = write!(text, "\n  - {line}");
  }

  text
}
