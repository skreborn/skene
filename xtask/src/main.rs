//! The workspace's gate: every check a change passes before it lands, in one command.
//!
//! `cargo xtask gate` runs every step in order, and `cargo xtask gate clippy test:browser`
//! runs the named ones, a bare group name such as `clippy` selecting each of its leaves. A
//! step writes its whole output to `target/gate/<step>.log`; the gate prints one line per
//! step and, for a step that failed, why and the end of its log.
//!
//! A step that builds or runs anything is skipped while nothing it reads has changed since
//! it last passed: every file of the workspace but the five project documents (`PROSE`),
//! the toolchain and the browser drivers. `--force`, or a `CI` variable, runs every selected
//! step regardless.
//!
//! `cargo xtask api` rewrites the public API snapshots the `api` step checks.

mod api;
mod browser;
mod lint;
mod manifests;

use std::{
  env,
  ffi::{OsStr, OsString},
  fs::{self, File},
  hash::{DefaultHasher, Hash as _, Hasher as _},
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
  /// Whether the step is skipped while its inputs are unchanged since it passed.
  stamped: bool,
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
    stamped: false,
  },
  Step {
    name: "manifests",
    run: manifest_rules,
    stamped: false,
  },
  Step {
    name: "lint",
    run: house_rules,
    stamped: false,
  },
  Step {
    name: "deps",
    run: dependencies,
    stamped: false,
  },
  Step {
    name: "licenses",
    run: licenses,
    stamped: false,
  },
  Step {
    name: "clippy:host",
    run: clippy_host,
    stamped: true,
  },
  Step {
    name: "clippy:release",
    run: clippy_release,
    stamped: true,
  },
  Step {
    name: "clippy:server",
    run: clippy_server,
    stamped: true,
  },
  Step {
    name: "clippy:wasm",
    run: clippy_wasm,
    stamped: true,
  },
  Step {
    name: "clippy:apple",
    run: clippy_apple,
    stamped: true,
  },
  Step {
    name: "doc:host",
    run: doc_host,
    stamped: true,
  },
  Step {
    name: "doc:server",
    run: doc_server,
    stamped: true,
  },
  Step {
    name: "doc:wasm",
    run: doc_wasm,
    stamped: true,
  },
  Step {
    name: "api",
    run: api::check,
    stamped: true,
  },
  Step {
    name: "test:host",
    run: test_host,
    stamped: true,
  },
  Step {
    name: "test:server",
    run: test_server,
    stamped: true,
  },
  Step {
    name: "test:browser",
    run: browser::run,
    stamped: true,
  },
  Step {
    name: "msrv",
    run: msrv,
    stamped: true,
  },
];

/// The browser leaf's target.
const WASM: &str = "wasm32-unknown-unknown";

/// The target the code written for Apple's platforms is linted for.
const APPLE: &str = "aarch64-apple-darwin";

/// The crates with code of their own for Apple's platforms, none of which compiles C,
/// which would need Apple's SDK.
const APPLE_PACKAGES: &[&str] = &["skene-terminal"];

/// The flag that names the web's server leaf in a build.
const SERVER_CFG: &str = r#"--cfg skene_platform="server""#;

/// The crates whose items depend on the server leaf.
const SERVER_PACKAGES: &[&str] = &["skene-web"];

/// The crates documented for the browser leaf: every crate an app there reaches, and the
/// third-party proofs written for it.
const WASM_DOCS: &[&str] = &[
  "acme-map",
  "skene-foundation",
  "skene-reactive",
  "skene-simple",
  "skene-simple-web",
  "skene-transport",
  "skene-web",
];

/// The five project documents, which no crate compiles and no stamped step reads; every
/// other file, every other document among them, is an input of a stamped step.
const PROSE: &[&str] = &[
  "AGENTS.md",
  "CLAUDE.md",
  "DECISIONS.md",
  "DESIGN.md",
  "ROADMAP.md",
];

/// How many lines of a failed step's log the gate prints.
const TAIL: usize = 30;

fn main() -> ExitCode {
  let arguments: Vec<String> = env::args().skip(1).collect();

  match arguments.split_first() {
    Some((command, selectors)) if command == "gate" => run_gate(selectors),
    Some((command, [])) if command == "api" => write_api(),
    _ => {
      eprintln!(
        "usage: cargo xtask gate [--force] [step ...] | cargo xtask api\nsteps: {}",
        step_names()
      );

      ExitCode::FAILURE
    }
  }
}

/// Rewrites the public API snapshots, logging to `target/gate/api-write.log`.
fn write_api() -> ExitCode {
  let written = Gate::new().and_then(|gate| {
    let logs = gate.target.join("gate");

    fs::create_dir_all(&logs)
      .map_err(|error| format!("{}: {error}", logs.display()))
      .and_then(|()| Log::create(logs.join("api-write.log")))
      .and_then(|mut log| api::write(&gate, &mut log))
  });

  match written {
    Ok(()) => ExitCode::SUCCESS,
    Err(reason) => {
      eprintln!("{reason}");

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

fn run_gate(arguments: &[String]) -> ExitCode {
  let Ok(gate) = Gate::new().inspect_err(|error| eprintln!("{error}"))
  else {
    return ExitCode::FAILURE;
  };

  let force = env::var_os("CI").is_some() || arguments.iter().any(|argument| argument == "--force");
  let selectors: Vec<&str> = arguments
    .iter()
    .map(String::as_str)
    .filter(|argument| *argument != "--force")
    .collect();
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

  let stamps = logs.join("stamps");
  let inputs = steps
    .iter()
    .any(|step| step.stamped)
    .then(|| gate.inputs().ok())
    .flatten();
  let mut failed = Vec::new();

  for step in steps {
    let stamp = stamps.join(step.name.replace(':', "-"));

    if step.stamped
      && !force
      && let Some(inputs) = &inputs
      && fs::read_to_string(&stamp).is_ok_and(|passed| passed == *inputs)
    {
      println!(
        "skip    {:<14}          unchanged since it passed",
        step.name
      );

      continue;
    }

    let started = Instant::now();
    let outcome =
      Log::create(logs.join(format!("{}.log", step.name.replace(':', "-")))).map(|mut log| {
        let outcome = (step.run)(&gate, &mut log);

        (log, outcome)
      });
    let seconds = started.elapsed().as_secs_f32();
    let passed = matches!(outcome, Ok((_, Ok(()))));

    if step.stamped {
      let _ = match (&inputs, passed) {
        (Some(inputs), true) => {
          fs::create_dir_all(&stamps).and_then(|()| fs::write(&stamp, inputs))
        }
        _ => fs::remove_file(&stamp),
      };
    }

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

/// Where the gate runs: the workspace, cargo's target directory and the toolchain.
struct Gate {
  /// The workspace's root.
  root: PathBuf,
  /// Cargo's target directory.
  target: PathBuf,
  /// What `rustc -vV` reports of the toolchain.
  toolchain: String,
  /// The host's target triple, as the toolchain reports it.
  host: String,
}

impl Gate {
  fn new() -> Result<Self, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .expect("the xtask crate sits in the workspace")
      .to_path_buf();
    let target =
      env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), |dir| root.join(dir));
    let output = Command::new("rustc")
      .arg("-vV")
      .output()
      .map_err(|error| format!("rustc -vV: {error}"))?;
    let toolchain = String::from_utf8_lossy(&output.stdout).into_owned();
    let host = toolchain
      .lines()
      .find_map(|line| line.strip_prefix("host: "))
      .ok_or("rustc -vV reports no host")?
      .to_owned();

    Ok(Self {
      root,
      target,
      toolchain,
      host,
    })
  }

  /// A hash of everything a stamped step reads: every file of the workspace, listed or
  /// not ignored, but the five project documents ([`PROSE`]); the toolchain; and the
  /// browser drivers.
  fn inputs(&self) -> Result<String, String> {
    let listed = Command::new("git")
      .args([
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
      ])
      .current_dir(&self.root)
      .output()
      .map_err(|error| format!("git ls-files: {error}"))?;
    let mut paths: Vec<&[u8]> = listed
      .stdout
      .split(|byte| *byte == 0)
      .filter(|path| !path.is_empty() && !PROSE.iter().any(|prose| *path == prose.as_bytes()))
      .collect();
    let mut hasher = DefaultHasher::new();

    paths.sort_unstable();
    paths.dedup();

    for path in paths {
      if let Ok(contents) = fs::read(self.root.join(String::from_utf8_lossy(path).as_ref())) {
        path.hash(&mut hasher);
        contents.hash(&mut hasher);
      }
    }

    self.toolchain.hash(&mut hasher);

    for variable in ["GECKODRIVER", "CHROMEDRIVER"] {
      env::var_os(variable).hash(&mut hasher);
    }

    Ok(format!("{:016x}", hasher.finish()))
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

  /// The variable cargo reads `kind` (`RUSTFLAGS` or `RUSTDOCFLAGS`) from for `target`
  /// alone.
  fn target_flags(target: &str, kind: &str) -> String {
    format!(
      "CARGO_TARGET_{}_{kind}",
      target.to_uppercase().replace(['-', '.'], "_")
    )
  }

  /// A cargo command building for the server leaf on the host's target, its cfg in the
  /// target's rustc and rustdoc flags, in a target directory of its own; `--target` goes
  /// before any `--`.
  fn server_cargo<I, S>(&self, arguments: I) -> Command
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let mut arguments: Vec<OsString> = arguments
      .into_iter()
      .map(|argument| argument.as_ref().to_owned())
      .collect();
    let end = arguments
      .iter()
      .position(|argument| argument == "--")
      .unwrap_or(arguments.len());

    arguments.splice(end..end, ["--target".into(), self.host.clone().into()]);

    let mut command = self.cargo(arguments);

    command
      .env(Self::target_flags(&self.host, "RUSTFLAGS"), SERVER_CFG)
      .env(Self::target_flags(&self.host, "RUSTDOCFLAGS"), SERVER_CFG)
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

fn licenses(gate: &Gate, log: &mut Log) -> Outcome {
  gate.require("deny", "cargo-deny")?;
  log.check(gate.cargo(["deny", "--locked", "check", "licenses"]))
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

fn clippy_release(gate: &Gate, log: &mut Log) -> Outcome {
  log.check(gate.cargo([
    "clippy",
    "--workspace",
    "--all-targets",
    "--locked",
    "--release",
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
  ]))?;
  log.check(gate.cargo([
    "clippy",
    "-p",
    "skene-web",
    "--lib",
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

  command.args(packages(SERVER_PACKAGES)).env(
    Gate::target_flags(&gate.host, "RUSTDOCFLAGS"),
    format!("{SERVER_CFG} -D warnings"),
  );
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
