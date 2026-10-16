//! `skene`: makes, runs, tests, checks and builds a Skene app, one platform per build.
//!
//! ```text
//! skene new <name> [--platforms linux,web,...]
//! skene run [linux] [-- <app arguments>]
//! skene test [linux] [-- <cargo test arguments>]
//! skene check
//! skene build linux
//! ```
//!
//! A platform is a build fact: its real target triple and `--cfg skene_platform="<platform>"`.
//! The app's `.cargo/config.toml` (which `skene new` writes) names the platform plain cargo
//! builds for, so `cargo check`, `cargo test`, `cargo run`, clippy, rustdoc and rust-analyzer
//! need no CLI. The CLI builds for a platform the configuration may not name: it reads the
//! flags cargo would use, from cargo's own sources in cargo's precedence order, replaces the
//! `skene_platform` cfg among them where it stands (so the flags of the configuration's own
//! platform stay byte-identical and nothing rebuilds), and hands them to cargo as
//! `CARGO_ENCODED_RUSTFLAGS`/`CARGO_ENCODED_RUSTDOCFLAGS`, which cargo ranks above every other
//! source. Where it cannot tell what cargo would use, it refuses rather than guess. A Linux
//! build whose configuration names no runner for the triple runs through the display runner
//! beside the CLI (`CARGO_TARGET_<TRIPLE>_RUNNER`), as one `skene new` configured for Linux
//! does.
//!
//! In this prototype only Linux (drawn by Skene) is realized; `skene check` fails where the
//! app declares a platform that is not, naming it. A test's session is the runner's X11 leg
//! unless `SKENE_TEST_SESSION` names another (`wayland`: headless mutter; `sway`).

mod flags;
mod new;

use std::{
  env, fs,
  path::{Path, PathBuf},
  process::{Command, ExitCode, Stdio},
};

use crate::flags::Flags;

const USAGE: &str = "usage:
  skene new <name> [--platforms linux,web,...]
  skene run [linux] [-- <app arguments>]
  skene test [linux] [-- <cargo test arguments>]
  skene check
  skene build linux
platforms: linux (realized: drawn by Skene), web, server, ios, android, macos, windows (not realized in this prototype)
tests run in a private session: SKENE_TEST_SESSION=x11 (the default), wayland (headless mutter) or sway";

/// A platform: its name, its target triple, and whether this prototype realizes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Platform {
  /// The name the cfg takes.
  pub name: &'static str,
  /// The real target triple, `None` for the host's.
  pub triple: Option<&'static str>,
  /// Whether a build for it can succeed in this prototype.
  pub realized: bool,
}

/// Every platform skene-gui declares.
pub const PLATFORMS: &[Platform] = &[
  Platform {
    name: "linux",
    triple: None,
    realized: true,
  },
  Platform {
    name: "web",
    triple: Some("wasm32-unknown-unknown"),
    realized: false,
  },
  Platform {
    name: "server",
    triple: None,
    realized: false,
  },
  Platform {
    name: "ios",
    triple: Some("aarch64-apple-ios"),
    realized: false,
  },
  Platform {
    name: "android",
    triple: Some("aarch64-linux-android"),
    realized: false,
  },
  Platform {
    name: "macos",
    triple: Some("aarch64-apple-darwin"),
    realized: false,
  },
  Platform {
    name: "windows",
    triple: Some("x86_64-pc-windows-msvc"),
    realized: false,
  },
];

impl Platform {
  /// The platform named `name`.
  fn named(name: &str) -> Result<Self, String> {
    PLATFORMS
      .iter()
      .find(|platform| platform.name == name)
      .copied()
      .ok_or_else(|| {
        format!(
          "unknown platform `{name}`; the platforms are {}",
          PLATFORMS
            .iter()
            .map(|platform| platform.name)
            .collect::<Vec<_>>()
            .join(", ")
        )
      })
  }

  /// Its target triple: its own, or the host's.
  fn triple(self) -> Result<String, String> {
    match self.triple {
      Some(triple) => Ok(triple.to_owned()),
      None => host(),
    }
  }

  /// Refuses a platform this prototype does not realize, or Linux off a Linux host.
  fn realizable(self) -> Result<Self, String> {
    if !self.realized {
      return Err(format!("{} is not realized in this prototype", self.name));
    }

    if self.name == "linux" && !host()?.contains("-linux") {
      return Err("linux builds on a Linux host".to_owned());
    }

    Ok(self)
  }
}

/// The host's target triple, from `rustc -vV`.
pub fn host() -> Result<String, String> {
  let output = output(Command::new("rustc").arg("-vV"), "rustc -vV")?;

  output
    .lines()
    .find_map(|line| line.strip_prefix("host: "))
    .map(str::to_owned)
    .ok_or_else(|| "rustc -vV names no host".to_owned())
}

fn main() -> ExitCode {
  let arguments: Vec<String> = env::args().skip(1).collect();

  match run(&arguments) {
    Ok(code) => code,
    Err(error) => {
      eprintln!("skene: {error}");

      ExitCode::FAILURE
    }
  }
}

fn run(arguments: &[String]) -> Result<ExitCode, String> {
  let Some((command, rest)) = arguments.split_first() else {
    return Err(USAGE.to_owned());
  };
  let (options, passed) = match rest.iter().position(|argument| argument == "--") {
    Some(split) => (&rest[..split], &rest[split + 1..]),
    None => (rest, &[][..]),
  };

  match command.as_str() {
    "new" => new::new(options).map(|()| ExitCode::SUCCESS),
    "run" => App::find()?.run(platform(options)?, passed),
    "test" => App::find()?.test(platform(options)?, passed),
    "check" => {
      if let Some(extra) = options.first() {
        return Err(format!(
          "`skene check` checks every platform the app declares; unexpected `{extra}`"
        ));
      }

      App::find()?.check()
    }
    "build" => {
      let [name] = options else {
        return Err(format!("name the platform to build\n{USAGE}"));
      };

      App::find()?.build(Platform::named(name)?.realizable()?)
    }
    "help" | "--help" | "-h" => {
      println!("{USAGE}");

      Ok(ExitCode::SUCCESS)
    }
    other => Err(format!("unknown command `{other}`\n{USAGE}")),
  }
}

/// The platform `options` names, Linux where it names none.
fn platform(options: &[String]) -> Result<Platform, String> {
  match options {
    [] => Platform::named("linux")?.realizable(),
    [name] => Platform::named(name)?.realizable(),
    [_, extra, ..] => Err(format!("unexpected `{extra}`\n{USAGE}")),
  }
}

/// Runs a command and returns its standard output.
pub fn output(command: &mut Command, what: &str) -> Result<String, String> {
  let output = command
    .stdin(Stdio::null())
    .output()
    .map_err(|error| format!("could not run {what}: {error}"))?;

  if !output.status.success() {
    return Err(format!(
      "{what} failed: {}",
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }

  Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Cargo, as the environment names it.
fn cargo() -> Command {
  Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

/// The app in the current directory.
struct App {
  dir: PathBuf,
  package: String,
  platforms: Vec<Platform>,
}

impl App {
  /// The package whose `Cargo.toml` is in the current directory, and the platforms its
  /// `skene.toml` declares (Linux alone where it has none).
  fn find() -> Result<Self, String> {
    let dir = env::current_dir().map_err(|error| error.to_string())?;
    let manifest = fs::read_to_string(dir.join("Cargo.toml"))
      .map_err(|error| format!("no Cargo.toml here ({error}): run skene in the app's directory"))?;
    let manifest: toml::Table = manifest
      .parse()
      .map_err(|error| format!("Cargo.toml: {error}"))?;
    let package = manifest
      .get("package")
      .and_then(|package| package.get("name"))
      .and_then(toml::Value::as_str)
      .ok_or("Cargo.toml names no package")?
      .to_owned();
    let platforms = match fs::read_to_string(dir.join("skene.toml")) {
      Ok(text) => {
        let table: toml::Table = text
          .parse()
          .map_err(|error| format!("skene.toml: {error}"))?;

        table
          .get("platforms")
          .and_then(toml::Value::as_array)
          .ok_or("skene.toml declares no `platforms`")?
          .iter()
          .map(|name| Platform::named(name.as_str().unwrap_or_default()))
          .collect::<Result<_, _>>()?
      }
      Err(_) => vec![Platform::named("linux")?],
    };

    Ok(Self {
      dir,
      package,
      platforms,
    })
  }

  /// Cargo running `subcommand` for `platform`: its triple, the flags cargo would use with the
  /// platform's cfg in place of whichever the build names, and on Linux the display runner
  /// where the configuration names no runner.
  fn cargo(&self, subcommand: &str, platform: Platform) -> Result<Command, String> {
    let triple = platform.triple()?;
    let rustflags = Flags::resolve(&self.dir, &triple, flags::Kind::Rust)?;
    let rustdocflags = Flags::resolve(&self.dir, &triple, flags::Kind::Rustdoc)?;
    let mut command = cargo();

    eprintln!(
      "skene: {} ({triple}): rustflags from {}, rustdocflags from {}",
      platform.name, rustflags.source, rustdocflags.source
    );
    command
      .current_dir(&self.dir)
      .args([subcommand, "--target", &triple])
      .env(
        "CARGO_ENCODED_RUSTFLAGS",
        rustflags.substitute(platform.name)?.join("\x1f"),
      )
      .env(
        "CARGO_ENCODED_RUSTDOCFLAGS",
        rustdocflags.substitute(platform.name)?.join("\x1f"),
      )
      .env_remove("RUSTFLAGS")
      .env_remove("RUSTDOCFLAGS");

    if platform.name == "linux" && !Flags::names_runner(&self.dir, &triple)? {
      let runner = env::current_exe()
        .map_err(|error| error.to_string())?
        .with_file_name("skene-runner");

      command.env(
        format!(
          "CARGO_TARGET_{}_RUNNER",
          triple.to_uppercase().replace(['-', '.'], "_")
        ),
        runner,
      );
    }

    Ok(command)
  }

  /// `cargo run` for `platform`, through the configuration's runner (the app's own display
  /// where the user has one).
  fn run(&self, platform: Platform, passed: &[String]) -> Result<ExitCode, String> {
    let mut command = self.cargo("run", platform)?;

    command.arg("--").args(passed);

    status(&mut command, "cargo run")
  }

  /// `cargo test` for `platform`: every test binary through the runner, each on a display of
  /// its own.
  fn test(&self, platform: Platform, passed: &[String]) -> Result<ExitCode, String> {
    Flags::refuse_config(passed)?;

    let mut command = self.cargo("test", platform)?;

    command.args(passed);

    status(&mut command, "cargo test")
  }

  /// Clippy, warnings denied, on every target of each platform the app declares; a declared
  /// platform this prototype does not realize fails the check, named.
  fn check(&self) -> Result<ExitCode, String> {
    let mut failed = false;

    for platform in &self.platforms {
      if !platform.realized {
        println!(
          "skene check: {}: FAILED: the app declares it (skene.toml), and it is not realized in this prototype ({})",
          platform.name,
          platform.triple()?
        );

        failed = true;

        continue;
      }

      let mut command = self.cargo("clippy", platform.realizable()?)?;

      command.args(["--all-targets", "--", "-D", "warnings"]);

      let passed = status(&mut command, "cargo clippy")? == ExitCode::SUCCESS;

      println!(
        "skene check: {}: {}",
        platform.name,
        if passed { "passed" } else { "FAILED" }
      );

      failed |= !passed;
    }

    Ok(if failed {
      ExitCode::FAILURE
    } else {
      ExitCode::SUCCESS
    })
  }

  /// A release binary for `platform`, its path and size printed.
  fn build(&self, platform: Platform) -> Result<ExitCode, String> {
    let mut command = self.cargo("build", platform)?;

    command.arg("--release");

    if status(&mut command, "cargo build")? != ExitCode::SUCCESS {
      return Ok(ExitCode::FAILURE);
    }

    let binary = self
      .target_directory()?
      .join(platform.triple()?)
      .join("release")
      .join(&self.package);
    let size = fs::metadata(&binary)
      .map_err(|error| format!("{}: {error}", binary.display()))?
      .len();

    println!(
      "skene build: {}: {} ({:.1} MB)",
      platform.name,
      binary.display(),
      size as f64 / 1_048_576.
    );

    Ok(ExitCode::SUCCESS)
  }

  /// Cargo's target directory for the app.
  fn target_directory(&self) -> Result<PathBuf, String> {
    let metadata = output(
      cargo()
        .current_dir(&self.dir)
        .args(["metadata", "--format-version", "1", "--no-deps"]),
      "cargo metadata",
    )?;
    let key = "\"target_directory\":\"";
    let start = metadata
      .find(key)
      .ok_or("cargo metadata names no target directory")?
      + key.len();
    let end = metadata[start..]
      .find('"')
      .ok_or("cargo metadata is malformed")?;

    Ok(Path::new(&metadata[start..start + end]).to_path_buf())
  }
}

/// Runs `command` with its output shown; its exit code.
fn status(command: &mut Command, what: &str) -> Result<ExitCode, String> {
  let status = command
    .status()
    .map_err(|error| format!("could not run {what}: {error}"))?;

  Ok(if status.success() {
    ExitCode::SUCCESS
  } else {
    ExitCode::from(u8::try_from(status.code().unwrap_or(1)).unwrap_or(1))
  })
}
