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
mod results;

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

use crate::{manifests::Workspace, results::Minimums};

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
    name: "selection",
    run: selection,
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
    name: "clippy:terminal",
    run: clippy_terminal,
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
    name: "scaffolds",
    run: scaffolds,
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
    name: "doc:terminal",
    run: doc_terminal,
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
    name: "test:terminal",
    run: test_terminal,
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
const APPLE_PACKAGES: &[&str] = &["skene-tui-terminal"];

/// The flag that names the web in a build: on [`WASM`], its browser leaf.
const WEB_CFG: &str = r#"--cfg skene_platform="web""#;

/// The flags that name the web's server leaf in a build.
const SERVER_CFG: &str = r#"--cfg skene_platform="web" --cfg skene_server"#;

/// The flag that names the terminal in a build.
const TERMINAL_CFG: &str = r#"--cfg skene_platform="terminal""#;

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

  /// Fails unless the standard library for `target` is installed, in `toolchain` where
  /// one is named and in the pinned toolchain otherwise.
  fn require_target(target: &str, toolchain: Option<&str>) -> Outcome {
    let mut list = Command::new("rustup");

    list.args(["target", "list", "--installed"]);

    if let Some(toolchain) = toolchain {
      list.args(["--toolchain", toolchain]);
    }

    let installed = list.output().is_ok_and(|output| {
      String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line == target)
    });

    if installed {
      Ok(())
    }
    else {
      Err(match toolchain {
        Some(toolchain) => format!(
          "the {target} target is missing from {toolchain}: `rustup target add --toolchain \
           {toolchain} {target}`"
        ),
        None => format!("the {target} target is missing: `rustup target add {target}`"),
      })
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

  /// A cargo command building for the server leaf on the host's target, its cfgs in the
  /// target's rustc and rustdoc flags, in a target directory of its own.
  fn server_cargo<I, S>(&self, arguments: I) -> Command
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let mut command = self.cargo_for(&self.host, SERVER_CFG, arguments);

    command.env("CARGO_TARGET_DIR", self.target.join("server"));

    command
  }

  /// A cargo command building for the terminal on the host's target, its cfg in the
  /// target's rustc and rustdoc flags, in a target directory of its own.
  fn terminal_cargo<I, S>(&self, arguments: I) -> Command
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let mut command = self.cargo_for(&self.host, TERMINAL_CFG, arguments);

    command.env("CARGO_TARGET_DIR", self.target.join("terminal"));

    command
  }

  /// A cargo command building for the browser leaf, its cfg in [`WASM`]'s rustc and
  /// rustdoc flags.
  fn wasm_cargo<I, S>(&self, arguments: I) -> Command
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    self.cargo_for(WASM, WEB_CFG, arguments)
  }

  /// A cargo command building for `target` with `flags` in its rustc and rustdoc flags
  /// alone; `--target` goes before any `--`.
  fn cargo_for<I, S>(&self, target: &str, flags: &str, arguments: I) -> Command
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let mut command = self.cargo(targeted(target, arguments));

    command
      .env(Self::target_flags(target, "RUSTFLAGS"), flags)
      .env(Self::target_flags(target, "RUSTDOCFLAGS"), flags);

    command
  }

  /// A cargo command run in the scaffolded `project`, building for `build` in the build's
  /// target directory under [`SCAFFOLDS`]: a build with flags has them in `RUSTFLAGS` and
  /// `RUSTDOCFLAGS`, which outrank the project's own configuration, and one without is
  /// named by that configuration alone.
  fn scaffold_cargo<I, S>(&self, project: &Path, build: &ScaffoldBuild, arguments: I) -> Command
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let mut command = Command::new("cargo");

    command
      .current_dir(project)
      .env("CARGO_TERM_COLOR", "never")
      .env(
        "CARGO_TARGET_DIR",
        self.target.join(SCAFFOLDS).join(build.name),
      );

    for variable in SCAFFOLD_OUTRANKING {
      command.env_remove(variable);
    }

    match build.flags {
      Some(flags) => {
        command
          .env("RUSTFLAGS", flags)
          .env("RUSTDOCFLAGS", flags)
          .args(targeted(build.target.unwrap_or(&self.host), arguments));
      }
      None => {
        command.args(arguments);
      }
    }

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
    self.output(command).map(drop)
  }

  /// Runs `command`, failing when it does, and returns what it printed.
  fn output(&mut self, command: Command) -> Result<String, String> {
    let described = describe(&command);
    let ran = self.run(command)?;

    if ran.success {
      Ok(ran.output)
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

/// `arguments` with `--target target` before any `--`.
fn targeted<I, S>(target: &str, arguments: I) -> Vec<OsString>
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

  arguments.splice(end..end, ["--target".into(), target.into()]);

  arguments
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
fn packages<S: AsRef<str>>(packages: &[S]) -> Vec<String> {
  packages
    .iter()
    .flat_map(|package| ["-p".to_owned(), package.as_ref().to_owned()])
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
  log.check(gate.cargo(["deny", "--workspace", "--locked", "check", "licenses"]))
}

fn clippy_host(gate: &Gate, log: &mut Log) -> Outcome {
  let workspace = Workspace::read(&gate.root)?;
  let mut command = gate.cargo(["clippy", "--all-targets", "--locked"]);

  command
    .args(packages(&workspace.host()))
    .args(["--", "-D", "warnings"]);
  log.check(command)
}

/// What each facade reports for each build it refuses: the facade, the flags naming the
/// platform (none, then on the host's target), the message it fails with.
const REFUSED: &[(&str, Option<&str>, &str)] = &[
  (
    "skene-gui",
    None,
    "skene-gui builds for the platform its build names",
  ),
  (
    "skene-gui",
    Some(WEB_CFG),
    r#"skene_platform = "web" builds the browser on wasm32-unknown-unknown alone"#,
  ),
  (
    "skene-gui",
    Some(r#"--cfg skene_platform="linux""#),
    "skene-gui realizes no linux yet",
  ),
  (
    "skene-tui",
    None,
    "skene-tui builds for the platform its build names",
  ),
];

/// Holds each facade to refusing each build [`REFUSED`] names with its own message
/// alone.
fn selection(gate: &Gate, log: &mut Log) -> Outcome {
  let mut problems = Vec::new();

  for (package, flags, message) in REFUSED {
    let arguments = ["check", "--locked", "-p", package];
    let command = match flags {
      None => gate.cargo(arguments),
      Some(flags) => {
        let mut command = gate.cargo_for(&gate.host, flags, arguments);

        command.env("CARGO_TARGET_DIR", gate.target.join("selection"));

        command
      }
    };
    let described = describe(&command);
    let ran = log.run(command)?;
    let alone = ran.output.contains(&format!(
      "could not compile `{package}` (lib) due to 1 previous error"
    ));

    if ran.success || !ran.output.contains(message) || !alone {
      problems.push(format!(
        "`{described}` did not fail with {package}'s message alone: {message}"
      ));
    }
  }

  if problems.is_empty() {
    Ok(())
  }
  else {
    Err(problems.join("\n"))
  }
}

fn clippy_release(gate: &Gate, log: &mut Log) -> Outcome {
  let workspace = Workspace::read(&gate.root)?;
  let mut host = gate.cargo(["clippy", "--all-targets", "--locked", "--release"]);

  host
    .args(packages(&workspace.host()))
    .args(["--", "-D", "warnings"]);
  log.check(host)?;

  let mut server = gate.server_cargo(["clippy", "--all-targets", "--locked", "--release"]);

  server
    .args(packages(&workspace.web()))
    .args(["--", "-D", "warnings"]);
  log.check(server)?;

  let mut terminal = gate.terminal_cargo(["clippy", "--all-targets", "--locked", "--release"]);

  terminal
    .args(packages(&workspace.terminal()))
    .args(["--", "-D", "warnings"]);
  log.check(terminal)
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

fn clippy_terminal(gate: &Gate, log: &mut Log) -> Outcome {
  log.check(gate.terminal_cargo([
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
  log.check(gate.wasm_cargo([
    "clippy",
    "--workspace",
    "--all-targets",
    "--locked",
    "--",
    "-D",
    "warnings",
  ]))?;
  log.check(gate.wasm_cargo([
    "clippy",
    "-p",
    "skene-gui",
    "-p",
    "skene-gui-web",
    "--lib",
    "--locked",
    "--",
    "-D",
    "warnings",
  ]))
}

fn clippy_apple(gate: &Gate, log: &mut Log) -> Outcome {
  Gate::require_target(APPLE, None)?;

  let mut command = gate.cargo_for(APPLE, TERMINAL_CFG, ["clippy", "--all-targets", "--locked"]);

  command
    .args(packages(APPLE_PACKAGES))
    .args(["--", "-D", "warnings"])
    .env("CARGO_TARGET_DIR", gate.target.join("apple"));
  log.check(command)
}

/// The directory under the target directory the scaffolds and their builds are in.
const SCAFFOLDS: &str = "scaffolds";

/// The kinds of project `skene new` scaffolds, each written to the directory of its name
/// under [`SCAFFOLDS`].
const SCAFFOLD_PROJECTS: &[&str] = &["app", "lib"];

/// The platforms each scaffold is given, the terminal first, so the build plain cargo
/// runs needs no test runner.
const SCAFFOLD_PLATFORMS: &[&str] = &["terminal", "web"];

/// The variables that would outrank a scaffold's own configuration, which a scaffold's
/// build never inherits from the gate's environment.
const SCAFFOLD_OUTRANKING: &[&str] = &[
  "CARGO_BUILD_RUSTDOCFLAGS",
  "CARGO_BUILD_RUSTFLAGS",
  "CARGO_BUILD_TARGET",
  "CARGO_ENCODED_RUSTDOCFLAGS",
  "CARGO_ENCODED_RUSTFLAGS",
  "RUSTDOCFLAGS",
  "RUSTFLAGS",
];

/// A build a scaffold is linted in.
struct ScaffoldBuild {
  /// The build's name, which names its target directory under [`SCAFFOLDS`].
  name: &'static str,
  /// What the build is, as a failure names it.
  description: &'static str,
  /// The build's target, the host's where none is named; ignored without `flags`.
  target: Option<&'static str>,
  /// The flags naming the build's platform, none where it is the platform the project's
  /// configuration names, the first of [`SCAFFOLD_PLATFORMS`].
  flags: Option<&'static str>,
}

/// Every build of the platforms the scaffolds name, the one the project's configuration
/// names last; the doctests run in those on the host.
const SCAFFOLD_BUILDS: &[ScaffoldBuild] = &[
  ScaffoldBuild {
    name: "browser",
    description: "the web's browser leaf",
    target: Some(WASM),
    flags: Some(WEB_CFG),
  },
  ScaffoldBuild {
    name: "server",
    description: "the web's server leaf",
    target: None,
    flags: Some(SERVER_CFG),
  },
  ScaffoldBuild {
    name: "terminal",
    description: "the terminal",
    target: None,
    flags: None,
  },
];

/// The arguments of the `skene` command scaffolding `project` at `path` from the checkout
/// at `root`.
fn scaffold_arguments(project: &str, path: &Path, root: &Path) -> Vec<OsString> {
  ["new", project]
    .into_iter()
    .map(OsString::from)
    .chain([path.as_os_str().to_owned()])
    .chain(SCAFFOLD_PLATFORMS.iter().map(OsString::from))
    .chain(["--skene".into(), root.as_os_str().to_owned()])
    .collect()
}

/// Writes the app and the library `skene new` scaffolds from the checkout, and holds each
/// clippy-clean in every build of the platforms they name, their doctests passing in the
/// builds on the host.
fn scaffolds(gate: &Gate, log: &mut Log) -> Outcome {
  let directory = gate.target.join(SCAFFOLDS);

  for project in SCAFFOLD_PROJECTS {
    let path = directory.join(project);

    if path.exists() {
      fs::remove_dir_all(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    }
  }

  fs::create_dir_all(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;

  for project in SCAFFOLD_PROJECTS {
    let path = directory.join(project);

    log.note(&format!("# the {project} scaffold, at {}", path.display()));

    let mut new = gate.cargo(["run", "--locked", "-p", "skene-cli", "--"]);

    new.args(scaffold_arguments(project, &path, &gate.root));
    log
      .check(new)
      .map_err(|reason| format!("{reason}: `skene new {project}` writes no scaffold"))?;

    let mut lock = Command::new("cargo");

    lock
      .current_dir(&path)
      .env("CARGO_TERM_COLOR", "never")
      .args(["update", "--workspace", "--offline"]);
    log
      .check(lock)
      .map_err(|reason| format!("{reason}: the {project} scaffold's lock takes its own crates"))?;

    for build in SCAFFOLD_BUILDS {
      log
        .check(gate.scaffold_cargo(
          &path,
          build,
          [
            "clippy",
            "--all-targets",
            "--locked",
            "--offline",
            "--",
            "-D",
            "warnings",
          ],
        ))
        .map_err(|reason| {
          format!(
            "{reason}: the {project} scaffold fails clippy in {}'s build",
            build.description
          )
        })?;

      if build.target.is_none() {
        log
          .check(gate.scaffold_cargo(&path, build, ["test", "--doc", "--locked", "--offline"]))
          .map_err(|reason| {
            format!(
              "{reason}: the {project} scaffold's doctests fail in {}'s build",
              build.description
            )
          })?;
      }
    }
  }

  Ok(())
}

fn doc_host(gate: &Gate, log: &mut Log) -> Outcome {
  let workspace = Workspace::read(&gate.root)?;
  let mut command = gate.cargo(["doc", "--no-deps", "--locked"]);

  command
    .args(packages(&workspace.host()))
    .env("RUSTDOCFLAGS", "-D warnings");
  log.check(command)
}

fn doc_server(gate: &Gate, log: &mut Log) -> Outcome {
  let workspace = Workspace::read(&gate.root)?;
  let mut command = gate.server_cargo(["doc", "--no-deps", "--locked"]);

  command.args(packages(&workspace.web())).env(
    Gate::target_flags(&gate.host, "RUSTDOCFLAGS"),
    format!("{SERVER_CFG} -D warnings"),
  );
  log.check(command)
}

fn doc_terminal(gate: &Gate, log: &mut Log) -> Outcome {
  let workspace = Workspace::read(&gate.root)?;
  let mut command = gate.terminal_cargo(["doc", "--no-deps", "--locked"]);

  command.args(packages(&workspace.terminal())).env(
    Gate::target_flags(&gate.host, "RUSTDOCFLAGS"),
    format!("{TERMINAL_CFG} -D warnings"),
  );
  log.check(command)
}

fn doc_wasm(gate: &Gate, log: &mut Log) -> Outcome {
  let workspace = Workspace::read(&gate.root)?;
  let mut command = gate.wasm_cargo(["doc", "--no-deps", "--locked"]);

  command.args(packages(&workspace.web_build())).env(
    Gate::target_flags(WASM, "RUSTDOCFLAGS"),
    format!("{WEB_CFG} -D warnings"),
  );
  log.check(command)
}

/// The nextest run of a test step, every passed test's status printed whatever the
/// environment asks.
const NEXTEST: [&str; 6] = [
  "nextest",
  "run",
  "--locked",
  "--no-fail-fast",
  "--status-level",
  "pass",
];

/// The host run's least counts. A binary or doctest set passing fewer than listed fails
/// the gate, as does one passing tests unlisted, so a new test raises its count here and
/// a moved one lowers one count and raises another.
const HOST: Minimums = Minimums {
  binaries: &[
    ("skene-cli::bin/skene", 31),
    ("skene-conformance", 1),
    ("skene-dev", 6),
    ("skene-foundation", 268),
    ("skene-layout", 24),
    ("skene-macros", 7),
    ("skene-parley::it", 42),
    ("skene-reactive", 6),
    ("skene-reactive::it", 123),
    ("skene-simple", 2),
    ("skene-transport", 1),
    ("skene-transport::http", 4),
    ("skene-transport::ws", 4),
    ("xtask::bin/xtask", 38),
  ],
  doctests: &[("skene_foundation", 10)],
};

/// The server leaf's run's least counts. A binary or doctest set passing fewer than
/// listed fails the gate, as does one passing tests unlisted, so a new test raises its
/// count here and a moved one lowers one count and raises another.
const SERVER: Minimums = Minimums {
  binaries: &[
    ("skene-gui-web", 21),
    ("skene-gui::compile_fail", 1),
    ("skene-gui::server_head", 5),
    ("skene-gui::server_lazy_column", 2),
    ("skene-gui::server_overlay", 4),
    ("skene-gui::server_platform", 16),
    ("skene-gui::server_render", 66),
    ("skene-gui::server_router", 4),
    ("skene-gui::server_snapshot", 1),
    ("skene-gui::server_style", 37),
    ("skene-gui::server_suspense", 6),
    ("skene-gui::server_text_field", 8),
  ],
  doctests: &[
    ("guide", 45),
    ("readme", 2),
    ("skene_gui", 46),
    ("skene_gui_web", 1),
    ("skene_simple_gui", 12),
  ],
};

/// The terminal run's least counts. A binary or doctest set passing fewer than listed
/// fails the gate, as does one passing tests unlisted, so a new test raises its count
/// here and a moved one lowers one count and raises another.
const TERMINAL: Minimums = Minimums {
  binaries: &[
    ("acme-sparkline::render", 2),
    ("skene-simple-tui::it", 9),
    ("skene-tui-terminal", 76),
    ("skene-tui::it", 454),
    ("tui-context-menu::render", 1),
    ("tui-enhance::render", 2),
    ("tui-gallery::named", 1),
    ("tui-gallery::render", 1),
    ("tui-gallery::styled", 1),
    ("tui-http::render", 1),
    ("tui-text-sizing::render", 1),
    ("tui-theme::render", 1),
    ("tui-ws::render", 1),
  ],
  doctests: &[
    ("guide", 5),
    ("readme", 2),
    ("skene_simple_tui", 12),
    ("skene_tui", 43),
  ],
};

fn test_host(gate: &Gate, log: &mut Log) -> Outcome {
  gate.require("nextest", "cargo-nextest")?;

  let workspace = Workspace::read(&gate.root)?;
  let mut tests = gate.cargo(NEXTEST);

  tests.args(packages(&workspace.host()));

  let tests = log.output(tests)?;
  let mut doctests = gate.cargo(["test", "--locked", "--no-fail-fast", "--doc"]);

  doctests.args(packages(&workspace.host()));

  let doctests = log.output(doctests)?;

  HOST.check(&tests, &doctests)
}

fn test_server(gate: &Gate, log: &mut Log) -> Outcome {
  gate.require("nextest", "cargo-nextest")?;

  let workspace = Workspace::read(&gate.root)?;
  let mut tests = gate.server_cargo(NEXTEST);

  tests.args(packages(&workspace.web()));

  let tests = log.output(tests)?;
  let mut doctests = gate.server_cargo(["test", "--locked", "--no-fail-fast", "--doc"]);

  doctests.args(packages(&workspace.web()));

  let doctests = log.output(doctests)?;

  SERVER.check(&tests, &doctests)
}

fn test_terminal(gate: &Gate, log: &mut Log) -> Outcome {
  gate.require("nextest", "cargo-nextest")?;

  let workspace = Workspace::read(&gate.root)?;
  let mut tests = gate.terminal_cargo(NEXTEST);

  tests.args(packages(&workspace.terminal()));

  let tests = log.output(tests)?;
  let mut doctests = gate.terminal_cargo(["test", "--locked", "--no-fail-fast", "--doc"]);

  doctests.args(packages(&workspace.terminal()));

  let doctests = log.output(doctests)?;

  TERMINAL.check(&tests, &doctests)
}

fn msrv(gate: &Gate, log: &mut Log) -> Outcome {
  let version = manifests::rust_version(&gate.root)?;
  let toolchain = installed_toolchain(&version).ok_or_else(|| {
    format!("no toolchain for Rust {version} is installed: `rustup toolchain install {version}`")
  })?;
  Gate::require_target(WASM, Some(&toolchain))?;

  let toolchain = format!("+{toolchain}");
  let workspace = Workspace::read(&gate.root)?;
  let check = ["check", "--all-targets", "--locked"];
  let mut host = gate.cargo([toolchain.as_str()]);

  host
    .args(check)
    .args(packages(&workspace.host()))
    .env("CARGO_TARGET_DIR", gate.target.join("msrv").join("host"));
  log.check(host)?;

  let mut server = gate.server_cargo([toolchain.as_str()].into_iter().chain(check));

  server
    .arg("--workspace")
    .env("CARGO_TARGET_DIR", gate.target.join("msrv").join("server"));
  log.check(server)?;

  let mut terminal = gate.terminal_cargo([toolchain.as_str()].into_iter().chain(check));

  terminal.arg("--workspace").env(
    "CARGO_TARGET_DIR",
    gate.target.join("msrv").join("terminal"),
  );
  log.check(terminal)?;

  let mut browser = gate.wasm_cargo([toolchain.as_str()].into_iter().chain(check));

  browser
    .arg("--workspace")
    .env("CARGO_TARGET_DIR", gate.target.join("msrv").join("browser"));
  log.check(browser)
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

#[cfg(test)]
mod tests {
  use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command,
  };

  use super::{
    Gate,
    SCAFFOLD_BUILDS,
    SCAFFOLD_OUTRANKING,
    SCAFFOLD_PLATFORMS,
    SERVER_CFG,
    ScaffoldBuild,
    WASM,
    WEB_CFG,
    scaffold_arguments,
    targeted,
  };

  fn gate() -> Gate {
    Gate {
      root: PathBuf::from("/src/skene"),
      target: PathBuf::from("/src/skene/target"),
      toolchain: String::new(),
      host: "x86_64-unknown-linux-gnu".to_owned(),
    }
  }

  fn words(arguments: &[OsString]) -> Vec<&str> {
    arguments
      .iter()
      .map(|argument| argument.to_str().expect("the argument is UTF-8"))
      .collect()
  }

  #[test]
  fn the_target_goes_before_the_tools_arguments() {
    assert_eq!(
      words(&targeted(
        WASM,
        ["clippy", "--locked", "--", "-D", "warnings"]
      )),
      [
        "clippy", "--locked", "--target", WASM, "--", "-D", "warnings"
      ]
    );
    assert_eq!(
      words(&targeted(WASM, ["check"])),
      ["check", "--target", WASM]
    );
  }

  #[test]
  fn a_scaffold_names_the_terminal_first_and_the_checkout() {
    assert_eq!(
      words(&scaffold_arguments(
        "lib",
        Path::new("/src/skene/target/scaffolds/lib"),
        Path::new("/src/skene")
      )),
      [
        "new",
        "lib",
        "/src/skene/target/scaffolds/lib",
        "terminal",
        "web",
        "--skene",
        "/src/skene"
      ]
    );
  }

  /// What `command` sets the variable `name` to, if it sets it.
  fn variable<'a>(command: &'a Command, name: &str) -> Option<&'a OsStr> {
    command
      .get_envs()
      .find(|(key, _)| *key == name)
      .and_then(|(_, value)| value)
  }

  /// Whether `command` removes the variable `name` from what it inherits.
  fn removes(command: &Command, name: &str) -> bool {
    command
      .get_envs()
      .any(|(key, value)| key == name && value.is_none())
  }

  fn build(name: &str) -> &'static ScaffoldBuild {
    SCAFFOLD_BUILDS
      .iter()
      .find(|build| build.name == name)
      .expect("the build is listed")
  }

  /// A scaffold's build names its platform in the flags that outrank the project's own
  /// configuration, never in the target's, which cargo would join to the project's.
  #[test]
  fn a_scaffold_build_outranks_the_projects_platform() {
    let gate = gate();
    let project = Path::new("/src/skene/target/scaffolds/app");
    let command = gate.scaffold_cargo(project, build("server"), ["clippy", "--", "-D", "warnings"]);

    assert_eq!(command.get_current_dir(), Some(project));
    assert_eq!(
      variable(&command, "RUSTFLAGS"),
      Some(OsStr::new(SERVER_CFG))
    );
    assert_eq!(
      variable(&command, "RUSTDOCFLAGS"),
      Some(OsStr::new(SERVER_CFG))
    );
    assert_eq!(
      variable(&command, "CARGO_TARGET_DIR"),
      Some(OsStr::new("/src/skene/target/scaffolds/server"))
    );

    for name in ["CARGO_ENCODED_RUSTFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS"] {
      assert!(removes(&command, name), "{name}");
    }

    assert!(
      command
        .get_envs()
        .all(|(key, _)| !key.to_string_lossy().starts_with("CARGO_TARGET_X86")),
      "{:?}",
      command.get_envs().collect::<Vec<_>>()
    );
    assert_eq!(
      command.get_args().collect::<Vec<_>>(),
      [
        "clippy",
        "--target",
        "x86_64-unknown-linux-gnu",
        "--",
        "-D",
        "warnings"
      ]
    );
  }

  /// The build of the platform the project's configuration names is what plain cargo
  /// builds there: no flag, target or variable of the gate's outranks the configuration.
  #[test]
  fn the_configured_build_is_plain_cargo() {
    let gate = gate();
    let project = Path::new("/src/skene/target/scaffolds/lib");
    let terminal = build("terminal");
    let command = gate.scaffold_cargo(project, terminal, ["clippy", "--", "-D", "warnings"]);

    assert_eq!(SCAFFOLD_PLATFORMS.first(), Some(&"terminal"));
    assert_eq!(terminal.flags, None);

    for name in SCAFFOLD_OUTRANKING {
      assert!(removes(&command, name), "{name}");
    }

    assert_eq!(
      variable(&command, "CARGO_TARGET_DIR"),
      Some(OsStr::new("/src/skene/target/scaffolds/terminal"))
    );
    assert_eq!(
      command.get_args().collect::<Vec<_>>(),
      ["clippy", "--", "-D", "warnings"]
    );
  }

  #[test]
  fn every_platform_a_scaffold_names_is_built() {
    let builds: Vec<(&str, Option<&str>, Option<&str>)> = SCAFFOLD_BUILDS
      .iter()
      .map(|build| (build.name, build.target, build.flags))
      .collect();

    assert_eq!(
      builds,
      [
        ("browser", Some(WASM), Some(WEB_CFG)),
        ("server", None, Some(SERVER_CFG)),
        ("terminal", None, None),
      ]
    );
  }
}
