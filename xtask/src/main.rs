//! The workspace's gate: every check a change passes before it lands, in one command.
//!
//! `cargo xtask gate` runs every step in order, and `cargo xtask gate clippy test:browser`
//! runs the named ones, a bare group name such as `clippy` selecting each of its leaves. A
//! step writes its whole output to `target/gate/<step>.log`; the gate prints one line per
//! step and, for a step that failed, why and the end of its log.

mod browser;
mod lint;
mod manifests;

use std::{
  env,
  ffi::OsStr,
  fs::{self, File},
  io::Write as _,
  path::{Path, PathBuf},
  process::{Command, ExitCode},
  time::Instant,
};

/// What a step reports: nothing when it passes, why when it fails.
type Outcome = Result<(), String>;

/// One check of the gate.
struct Step {
  /// The step's name; the part before a `:` is the group it belongs to.
  name: &'static str,
  /// Runs the step, writing everything it prints to the log it is given.
  run: fn(&Gate, &mut Log) -> Outcome,
}

impl Step {
  fn is_selected_by(&self, selector: &str) -> bool {
    self.name == selector || self.name.split(':').next() == Some(selector)
  }
}

/// Every step, in the order the gate runs them.
const STEPS: &[Step] = &[
  Step {
    name: "fmt",
    run: fmt,
  },
  Step {
    name: "manifests",
    run: manifest_rules,
  },
  Step {
    name: "lint",
    run: house_rules,
  },
  Step {
    name: "deps",
    run: dependencies,
  },
  Step {
    name: "clippy:host",
    run: clippy_host,
  },
  Step {
    name: "clippy:server",
    run: clippy_server,
  },
  Step {
    name: "clippy:wasm",
    run: clippy_wasm,
  },
  Step {
    name: "clippy:apple",
    run: clippy_apple,
  },
  Step {
    name: "doc:host",
    run: doc_host,
  },
  Step {
    name: "doc:server",
    run: doc_server,
  },
  Step {
    name: "doc:wasm",
    run: doc_wasm,
  },
  Step {
    name: "test:host",
    run: test_host,
  },
  Step {
    name: "test:server",
    run: test_server,
  },
  Step {
    name: "test:browser",
    run: browser::run,
  },
  Step {
    name: "features",
    run: features,
  },
  Step {
    name: "msrv",
    run: msrv,
  },
];

/// The browser leaf's target.
const WASM: &str = "wasm32-unknown-unknown";

/// The target the code written for Apple's platforms is linted for.
const APPLE: &str = "aarch64-apple-darwin";

/// The crates with code of their own for Apple's platforms, none of which compiles C,
/// which would need Apple's SDK.
const APPLE_PACKAGES: &[&str] = &["skene-tui"];

/// The flag that names the web's server leaf in a build.
const SERVER_CFG: &str = r#"--cfg skene_platform="server""#;

/// The crates whose items depend on the server leaf.
const SERVER_PACKAGES: &[&str] = &["skene-gui", "skene-web"];

/// The crates documented for the browser leaf: every crate an app there reaches, and the
/// third-party proofs written for it.
const WASM_DOCS: &[&str] = &[
  "acme-map",
  "acme-ui",
  "skene-common",
  "skene-core",
  "skene-gui",
  "skene-reactive",
  "skene-simple",
  "skene-transport",
  "skene-web",
];

/// How many lines of a failed step's log the gate prints.
const TAIL: usize = 30;

fn main() -> ExitCode {
  let arguments: Vec<String> = env::args().skip(1).collect();

  match arguments.split_first() {
    Some((command, selectors)) if command == "gate" => run_gate(selectors),
    _ => {
      eprintln!(
        "usage: cargo xtask gate [step ...]\nsteps: {}",
        step_names()
      );

      ExitCode::FAILURE
    }
  }
}

fn step_names() -> String {
  STEPS
    .iter()
    .map(|step| step.name)
    .collect::<Vec<_>>()
    .join(", ")
}

fn run_gate(selectors: &[String]) -> ExitCode {
  let gate = Gate::new();
  let steps: Vec<&Step> = STEPS
    .iter()
    .filter(|step| {
      selectors.is_empty()
        || selectors
          .iter()
          .any(|selector| step.is_selected_by(selector))
    })
    .collect();

  if steps.is_empty() {
    eprintln!(
      "no step is named {}; the steps are {}",
      selectors.join(" "),
      step_names()
    );

    return ExitCode::FAILURE;
  }

  let logs = gate.target.join("gate");

  if let Err(error) = fs::create_dir_all(&logs) {
    eprintln!("{}: {error}", logs.display());

    return ExitCode::FAILURE;
  }

  let mut failed = Vec::new();

  for step in steps {
    let started = Instant::now();
    let outcome =
      Log::create(logs.join(format!("{}.log", step.name.replace(':', "-")))).map(|mut log| {
        let outcome = (step.run)(&gate, &mut log);

        (log, outcome)
      });
    let seconds = started.elapsed().as_secs_f32();

    match outcome {
      Ok((_, Ok(()))) => println!("ok      {:<14}{seconds:>7.1} s", step.name),
      Ok((mut log, Err(reason))) => {
        println!(
          "FAILED  {:<14}{seconds:>7.1} s  {}",
          step.name,
          log.path.display()
        );

        for line in log.tail(TAIL) {
          println!("      | {line}");
        }

        for line in reason.lines() {
          println!("        {line}");
        }

        log.note(&reason);
        failed.push(step.name);
      }
      Err(reason) => {
        println!("FAILED  {:<14}{seconds:>7.1} s  {reason}", step.name);
        failed.push(step.name);
      }
    }
  }

  if failed.is_empty() {
    println!("gate passed");

    ExitCode::SUCCESS
  }
  else {
    println!("gate failed: {}", failed.join(", "));

    ExitCode::FAILURE
  }
}

/// Where the gate runs: the workspace and cargo's target directory.
struct Gate {
  root: PathBuf,
  target: PathBuf,
}

impl Gate {
  fn new() -> Self {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .expect("the xtask crate sits in the workspace")
      .to_path_buf();
    let target =
      env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), |dir| root.join(dir));

    Self { root, target }
  }

  /// A cargo command run in the workspace, printing without color.
  fn cargo<I, S>(&self, arguments: I) -> Command
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let mut command = Command::new("cargo");

    command
      .current_dir(&self.root)
      .env("CARGO_TERM_COLOR", "never")
      .args(arguments);

    command
  }

  /// Fails unless the cargo subcommand `subcommand`, which `package` installs, is present.
  fn require(&self, subcommand: &str, package: &str) -> Outcome {
    let present = self
      .cargo([subcommand, "--version"])
      .output()
      .is_ok_and(|output| output.status.success());

    if present {
      Ok(())
    }
    else {
      Err(format!(
        "cargo {subcommand} is missing: `cargo install {package} --locked`"
      ))
    }
  }

  /// Fails unless the standard library for `target` is installed.
  fn require_target(target: &str) -> Outcome {
    let installed = Command::new("rustup")
      .args(["target", "list", "--installed"])
      .output()
      .is_ok_and(|output| {
        String::from_utf8_lossy(&output.stdout)
          .lines()
          .any(|line| line == target)
      });

    if installed {
      Ok(())
    }
    else {
      Err(format!(
        "the {target} target is missing: `rustup target add {target}`"
      ))
    }
  }

  /// A cargo command building for the server leaf, in a target directory of its own.
  fn server_cargo<I, S>(&self, arguments: I) -> Command
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let mut command = self.cargo(arguments);

    command
      .env("RUSTFLAGS", SERVER_CFG)
      .env("CARGO_TARGET_DIR", self.target.join("server"));

    command
  }
}

/// A step's log: the output of every command the step ran, in order.
struct Log {
  path: PathBuf,
  file: File,
}

/// What a command did: whether it succeeded, and what it printed.
struct Ran {
  success: bool,
  output: String,
}

impl Log {
  fn create(path: PathBuf) -> Result<Self, String> {
    let file = File::create(&path).map_err(|error| format!("{}: {error}", path.display()))?;

    Ok(Self { path, file })
  }

  /// Runs `command` with everything it prints appended to the log, and returns that.
  fn run(&mut self, mut command: Command) -> Result<Ran, String> {
    let described = describe(&command);
    let failed = |error: std::io::Error| format!("{described}: {error}");

    writeln!(self.file, "$ {described}").map_err(failed)?;

    let start = self.file.metadata().map_err(failed)?.len();
    let stdout = self.file.try_clone().map_err(failed)?;
    let stderr = self.file.try_clone().map_err(failed)?;
    let status = command
      .stdout(stdout)
      .stderr(stderr)
      .status()
      .map_err(failed)?;
    let printed = fs::read(&self.path).map_err(failed)?;
    let start = usize::try_from(start)
      .unwrap_or(printed.len())
      .min(printed.len());

    writeln!(self.file).map_err(failed)?;

    Ok(Ran {
      success: status.success(),
      output: String::from_utf8_lossy(&printed[start..]).into_owned(),
    })
  }

  /// Runs `command`, failing when it does.
  fn check(&mut self, command: Command) -> Outcome {
    let described = describe(&command);

    if self.run(command)?.success {
      Ok(())
    }
    else {
      Err(format!("`{described}` failed"))
    }
  }

  fn note(&mut self, text: &str) {
    let _ = writeln!(self.file, "{text}");
  }

  /// The last `count` lines of the log.
  fn tail(&self, count: usize) -> Vec<String> {
    let text = fs::read_to_string(&self.path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();

    lines[lines.len().saturating_sub(count)..]
      .iter()
      .map(|line| (*line).to_owned())
      .collect()
  }
}

/// `command` as a shell would read it, with the variables the gate set on it.
fn describe(command: &Command) -> String {
  let mut words: Vec<String> = command
    .get_envs()
    .filter(|(key, _)| key.to_str() != Some("CARGO_TERM_COLOR"))
    .filter_map(|(key, value)| value.map(|value| format!("{}={}", key.display(), quote(value))))
    .collect();

  words.push(command.get_program().display().to_string());
  words.extend(command.get_args().map(quote));
  words.join(" ")
}

fn quote(word: &OsStr) -> String {
  let word = word.display().to_string();

  if word.contains([' ', '"']) {
    format!("'{word}'")
  }
  else {
    word
  }
}

/// `-p` for each of `packages`.
fn packages(packages: &[&str]) -> Vec<String> {
  packages
    .iter()
    .flat_map(|package| ["-p".to_owned(), (*package).to_owned()])
    .collect()
}

fn fmt(gate: &Gate, log: &mut Log) -> Outcome {
  log.check(gate.cargo(["+nightly", "fmt", "--all", "--check"]))
}

fn manifest_rules(gate: &Gate, _: &mut Log) -> Outcome {
  manifests::check(&gate.root)
}

fn house_rules(gate: &Gate, _: &mut Log) -> Outcome {
  lint::check(&gate.root)
}

fn dependencies(gate: &Gate, log: &mut Log) -> Outcome {
  gate.require("shear", "cargo-shear")?;
  log.check(gate.cargo(["shear", "--deny-warnings", "--locked"]))
}

fn clippy_host(gate: &Gate, log: &mut Log) -> Outcome {
  log.check(gate.cargo([
    "clippy",
    "--workspace",
    "--all-targets",
    "--locked",
    "--",
    "-D",
    "warnings",
  ]))
}

fn clippy_server(gate: &Gate, log: &mut Log) -> Outcome {
  log.check(gate.server_cargo([
    "clippy",
    "--workspace",
    "--all-targets",
    "--locked",
    "--",
    "-D",
    "warnings",
  ]))
}

fn clippy_wasm(gate: &Gate, log: &mut Log) -> Outcome {
  log.check(gate.cargo([
    "clippy",
    "--workspace",
    "--all-targets",
    "--locked",
    "--target",
    WASM,
    "--",
    "-D",
    "warnings",
  ]))
}

fn clippy_apple(gate: &Gate, log: &mut Log) -> Outcome {
  Gate::require_target(APPLE)?;

  let mut command = gate.cargo(["clippy", "--all-targets", "--locked", "--target", APPLE]);

  command
    .args(packages(APPLE_PACKAGES))
    .args(["--", "-D", "warnings"])
    .env("CARGO_TARGET_DIR", gate.target.join("apple"));
  log.check(command)
}

fn doc_host(gate: &Gate, log: &mut Log) -> Outcome {
  let mut command = gate.cargo(["doc", "--workspace", "--no-deps", "--locked"]);

  command.env("RUSTDOCFLAGS", "-D warnings");
  log.check(command)
}

fn doc_server(gate: &Gate, log: &mut Log) -> Outcome {
  let mut command = gate.server_cargo(["doc", "--no-deps", "--locked"]);

  command
    .args(packages(SERVER_PACKAGES))
    .env("RUSTDOCFLAGS", format!("{SERVER_CFG} -D warnings"));
  log.check(command)
}

fn doc_wasm(gate: &Gate, log: &mut Log) -> Outcome {
  let mut command = gate.cargo(["doc", "--no-deps", "--locked", "--target", WASM]);

  command
    .args(packages(WASM_DOCS))
    .env("RUSTDOCFLAGS", "-D warnings");
  log.check(command)
}

fn test_host(gate: &Gate, log: &mut Log) -> Outcome {
  gate.require("nextest", "cargo-nextest")?;
  log.check(gate.cargo([
    "nextest",
    "run",
    "--workspace",
    "--locked",
    "--no-fail-fast",
  ]))?;
  log.check(gate.cargo(["test", "--workspace", "--locked", "--no-fail-fast", "--doc"]))
}

fn test_server(gate: &Gate, log: &mut Log) -> Outcome {
  let mut command = gate.server_cargo(["test", "--locked", "--no-fail-fast"]);

  command.args(packages(SERVER_PACKAGES));
  log.check(command)
}

fn features(gate: &Gate, log: &mut Log) -> Outcome {
  gate.require("hack", "cargo-hack")?;
  log.check(gate.cargo([
    "hack",
    "--feature-powerset",
    "check",
    "--workspace",
    "--locked",
  ]))
}

fn msrv(gate: &Gate, log: &mut Log) -> Outcome {
  let version = manifests::rust_version(&gate.root)?;
  let toolchain = installed_toolchain(&version).ok_or_else(|| {
    format!("no toolchain for Rust {version} is installed: `rustup toolchain install {version}`")
  })?;
  let mut command = gate.cargo([
    format!("+{toolchain}"),
    "check".to_owned(),
    "--workspace".to_owned(),
    "--all-targets".to_owned(),
    "--locked".to_owned(),
  ]);

  command.env("CARGO_TARGET_DIR", gate.target.join("msrv"));
  log.check(command)
}

/// The installed toolchain for Rust `version`, its first patch release preferred;
/// nothing is installed on the way.
fn installed_toolchain(version: &str) -> Option<String> {
  let output = Command::new("rustup")
    .args(["toolchain", "list"])
    .output()
    .ok()?;
  let list = String::from_utf8_lossy(&output.stdout);
  let names: Vec<&str> = list
    .lines()
    .filter_map(|line| line.split_whitespace().next())
    .collect();

  [
    format!("{version}.0-"),
    format!("{version}-"),
    format!("{version}."),
  ]
  .iter()
  .find_map(|prefix| names.iter().find(|name| name.starts_with(prefix.as_str())))
  .map(|name| (*name).to_owned())
}
