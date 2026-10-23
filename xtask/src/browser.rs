//! The browser tier: every browser test binary, and the conformance suite's harness
//! over the app serving its pages, run in Firefox through geckodriver and in Chromium
//! through chromedriver, several browser sessions at once.

use std::{
  env,
  fs,
  path::{Path, PathBuf},
  process::Command,
  sync::Mutex,
  thread,
};

use crate::{Gate, Log, Outcome, WASM, manifests};

/// A `cargo test` run on the browser leaf.
struct Run {
  /// The package whose browser binaries run.
  package: &'static str,
  /// Further arguments selecting the package's browser binaries.
  arguments: &'static [&'static str],
  /// Each browser binary with the number of tests it passes at least.
  binaries: &'static [(&'static str, usize)],
}

/// Every browser run. A binary passing fewer tests than listed fails the gate, so a new
/// browser test raises its binary's count here.
const RUNS: &[Run] = &[
  Run {
    package: "skene-web",
    arguments: &["--features", "gpu"],
    binaries: &[
      ("browser_dom", 84),
      ("browser_gpu_loss", 1),
      ("browser_hydrate", 20),
      ("browser_painted", 4),
      ("browser_parity", 1),
      ("browser_platform", 16),
      ("browser_resource", 5),
      ("browser_task", 4),
    ],
  },
  Run {
    package: "skene-transport",
    arguments: &[],
    binaries: &[("browser_http", 2), ("browser_ws", 2)],
  },
  Run {
    package: "acme-map",
    arguments: &[],
    binaries: &[("browser_map", 1)],
  },
  Run {
    package: "gallery",
    arguments: &["--test", "browser_axe"],
    binaries: &[("browser_axe", 1)],
  },
];

/// The conformance suite in real browsers: the app serving every page of the suite,
/// built as an app is built, and the harness acting each scenario out on it over a
/// WebDriver session.
struct Suite {
  /// The package holding the app and the harness.
  package: &'static str,
  /// The harness's binary.
  harness: &'static str,
  /// The number of scenarios the harness passes at least.
  scenarios: usize,
  /// The number of pages whose goldens the harness matches at least.
  pages: usize,
}

/// The conformance suite the tier runs. A harness passing fewer scenarios or goldens
/// than listed fails the gate, so a new scenario or page raises the count here.
const SUITE: Suite = Suite {
  package: "skene-conformance-web",
  harness: "harness",
  scenarios: 27,
  pages: 18,
};

/// A browser release the tier fetches once into `target/browsers`, checked against its
/// published digest.
struct Pin {
  /// The name of the directory it unpacks into, its version in it.
  name: &'static str,
  /// Where its archive is published.
  url: &'static str,
  /// The SHA-256 digest of its archive, in hex.
  sha256: &'static str,
  /// The executable its unpacked directory holds once complete, relative to it.
  executable: &'static str,
}

/// The version of the Firefox the goldens are taken in.
const FIREFOX_VERSION: &str = "158.0.1";

/// The Firefox the goldens are taken in, the release Mozilla publishes for x86-64 Linux.
const FIREFOX: Pin = Pin {
  name: "firefox-158.0.1",
  url: "https://archive.mozilla.org/pub/firefox/releases/158.0.1/linux-x86_64/en-US/firefox-158.0.1.tar.xz",
  sha256: "dce45470139242cf294eb6ada3d192a9bb334696842b02b71ade448a139eb41a",
  executable: "firefox/firefox",
};

/// The geckodriver driving the pinned Firefox.
const GECKODRIVER: Pin = Pin {
  name: "geckodriver-0.37.1",
  url: "https://github.com/mozilla/geckodriver/releases/download/v0.37.1/geckodriver-v0.37.1-linux64.tar.gz",
  sha256: "e815130ea95983e162ae91843b48d3a3ce991735635fce83a647afde21e09f7e",
  executable: "geckodriver",
};

/// The engine the goldens run in, by its name in a report.
const PINNED_FIREFOX: &str = "pinned Firefox";

/// How long, in seconds, the runner gives one binary in one browser: longer than any
/// test's own wait, the painted canvas's minute for its first GPU paint among them.
const BINARY_TIMEOUT: &str = "180";

/// The geckodriver bundled in Firefox's snap, which runs outside the snap's confinement.
const SNAP_GECKODRIVER: &str = "/snap/firefox/current/usr/lib/firefox/geckodriver";

/// A browser the tier runs every browser binary in.
struct Engine {
  /// The browser's name in a report.
  name: &'static str,
  /// The variable naming its driver, which the test runner reads.
  variable: &'static str,
  /// Where its driver is found when the variable names none.
  default: fn() -> Option<PathBuf>,
}

/// Every browser the tier runs in, Firefox first.
const ENGINES: &[Engine] = &[
  Engine {
    name: "Firefox",
    variable: "GECKODRIVER",
    default: snap_geckodriver,
  },
  Engine {
    name: "Chromium",
    variable: "CHROMEDRIVER",
    default: chromedriver,
  },
];

/// The geckodriver Firefox's snap bundles, where it is installed.
fn snap_geckodriver() -> Option<PathBuf> {
  Some(PathBuf::from(SNAP_GECKODRIVER)).filter(|path| path.exists())
}

/// A chromedriver on `PATH`, else the newest Chrome for Testing installed under
/// `~/.local/share/chrome-for-testing`.
fn chromedriver() -> Option<PathBuf> {
  let on_path = env::var_os("PATH").and_then(|path| {
    env::split_paths(&path)
      .map(|directory| directory.join("chromedriver"))
      .find(|driver| driver.is_file())
  });

  on_path.or_else(|| {
    let installs =
      Path::new(&env::var_os("HOME")?).join(".local/share/chrome-for-testing/chromedriver");
    let mut drivers: Vec<PathBuf> = fs::read_dir(installs)
      .ok()?
      .filter_map(Result::ok)
      .map(|install| install.path().join("chromedriver-linux64/chromedriver"))
      .filter(|driver| driver.is_file())
      .collect();

    drivers.sort();
    drivers.pop()
  })
}

/// What one binary reported.
struct Results {
  passed: usize,
  failed: usize,
}

/// A browser test binary cargo built, with the package it belongs to.
struct Binary {
  /// The package's name.
  package: &'static str,
  /// The binary's name: its source's file stem.
  name: String,
  /// The compiled module, or the harness's executable.
  path: PathBuf,
  /// The package's directory, where the runner finds its `webdriver.json`.
  directory: PathBuf,
  /// What runs it.
  runner: Runner,
}

/// What runs a browser binary in a browser.
enum Runner {
  /// `wasm-bindgen-test-runner`, a test module in a page of its own.
  Tests,
  /// The binary itself: the conformance harness, serving the app built into `site`.
  Harness {
    /// The directory the app was built into.
    site: PathBuf,
  },
  /// The conformance harness taking the goldens kept in `goldens` of the app built into
  /// `site`, in the pinned Firefox found in `firefox`.
  Goldens {
    /// The directory the app was built into.
    site: PathBuf,
    /// The directory the goldens are kept in.
    goldens: PathBuf,
    /// The directory holding the pinned Firefox.
    firefox: PathBuf,
  },
}

/// One binary to run in one engine, and what the run printed once it finished.
struct Job<'a> {
  engine: &'static str,
  variable: &'static str,
  driver: PathBuf,
  binary: &'a Binary,
  outcome: Option<Result<(bool, String), String>>,
}

/// How many browser sessions run at once, unless `SKENE_GATE_BROWSERS` says otherwise.
fn sessions() -> usize {
  env::var("SKENE_GATE_BROWSERS")
    .ok()
    .and_then(|count| count.parse().ok())
    .unwrap_or_else(|| {
      thread::available_parallelism().map_or(1, |cores| (cores.get() / 3).clamp(1, 8))
    })
    .max(1)
}

/// Links every library's tests for the browser, builds every browser binary, then runs
/// each in every engine, several sessions at once, and checks each against its
/// expected count; the browsers' temporary files go to a scratch directory emptied
/// first.
pub(crate) fn run(gate: &Gate, log: &mut Log) -> Outcome {
  check_runner(gate)?;

  let binaries = build(gate, log)?;
  let scratch = gate.target.join("gate").join("tmp");
  let _ = fs::remove_dir_all(&scratch);

  fs::create_dir_all(&scratch).map_err(|error| format!("{}: {error}", scratch.display()))?;

  let mut problems = Vec::new();
  let mut jobs = Vec::new();

  for engine in ENGINES {
    let Some(driver) = env::var_os(engine.variable)
      .map(PathBuf::from)
      .or_else(engine.default)
    else {
      problems.push(format!(
        "{}: no driver found; set {} to its path",
        engine.name, engine.variable
      ));

      continue;
    };

    jobs.extend(binaries.iter().map(|binary| Job {
      engine: engine.name,
      variable: engine.variable,
      driver: driver.clone(),
      binary,
      outcome: None,
    }));
  }

  let goldens = goldens(gate, log, &binaries)?;

  if let Some((binary, geckodriver)) = &goldens {
    jobs.push(Job {
      engine: PINNED_FIREFOX,
      variable: "GECKODRIVER",
      driver: geckodriver.clone(),
      binary,
      outcome: None,
    });
  }
  else {
    log.note("the goldens are skipped: the pinned Firefox is published for x86-64 Linux alone");
  }

  run_jobs(&mut jobs, &scratch);

  for job in &jobs {
    problems.extend(job.problems(log));
  }

  for run in RUNS {
    for (name, _) in run.binaries {
      if !binaries
        .iter()
        .any(|binary| binary.package == run.package && binary.name == *name)
      {
        problems.push(format!("{} `{name}`: was not built", run.package));
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

/// Links every library's tests for the browser and builds every run's binaries.
fn build(gate: &Gate, log: &mut Log) -> Result<Vec<Binary>, String> {
  log.check(gate.cargo([
    "test",
    "--locked",
    "--workspace",
    "--lib",
    "--no-run",
    "--target",
    WASM,
  ]))?;

  let mut binaries = Vec::new();

  for run in RUNS {
    let directory = manifests::package_directory(&gate.root, run.package)?;
    let mut command = gate.cargo([
      "test",
      "--locked",
      "--no-run",
      "--target",
      WASM,
      "-p",
      run.package,
    ]);

    command.args(run.arguments);

    let built = log.run(command)?;

    if !built.success {
      return Err(format!(
        "building the browser binaries of {} failed",
        run.package
      ));
    }

    binaries.extend(
      executables(&built.output)
        .into_iter()
        .map(|(name, path)| Binary {
          package: run.package,
          name,
          path: gate.root.join(path),
          directory: directory.clone(),
          runner: Runner::Tests,
        }),
    );
  }

  binaries.push(build_suite(gate, log)?);

  Ok(binaries)
}

/// The harness taking the goldens in the pinned Firefox, with the pinned geckodriver,
/// both fetched the first time; `None` on a host the pinned Firefox is not published
/// for.
fn goldens(
  gate: &Gate,
  log: &mut Log,
  binaries: &[Binary],
) -> Result<Option<(Binary, PathBuf)>, String> {
  if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    return Ok(None);
  }

  let Some((harness, site)) = binaries.iter().find_map(|binary| match &binary.runner {
    Runner::Harness { site } => Some((binary, site)),
    _ => None,
  })
  else {
    return Ok(None);
  };
  let browsers = gate.target.join("browsers");
  let firefox = FIREFOX.fetch(&browsers, log)?.join("firefox");
  let geckodriver = GECKODRIVER
    .fetch(&browsers, log)?
    .join(GECKODRIVER.executable);

  Ok(Some((
    Binary {
      package: SUITE.package,
      name: "goldens".to_owned(),
      path: harness.path.clone(),
      directory: harness.directory.clone(),
      runner: Runner::Goldens {
        site: site.clone(),
        goldens: harness.directory.join("goldens"),
        firefox,
      },
    },
    geckodriver,
  )))
}

impl Pin {
  /// The directory under `browsers` the release is unpacked in, fetched, checked
  /// against its digest and unpacked afresh first where it does not hold its executable.
  fn fetch(&self, browsers: &Path, log: &mut Log) -> Result<PathBuf, String> {
    let directory = browsers.join(self.name);

    if directory.join(self.executable).is_file() {
      return Ok(directory);
    }

    let _ = fs::remove_dir_all(&directory);

    let staging = browsers.join(format!("{}.staging", self.name));
    let archive = staging.join("archive");
    let _ = fs::remove_dir_all(&staging);

    fs::create_dir_all(&staging).map_err(|error| format!("{}: {error}", staging.display()))?;

    let mut download = Command::new("curl");

    download
      .args([
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--retry",
        "3",
      ])
      .arg("--output")
      .arg(&archive)
      .arg(self.url);
    log.check(download)?;

    let mut digest = Command::new("sha256sum");

    digest.arg(&archive);

    let digest = log.run(digest)?;
    let found = digest.output.split_whitespace().next().unwrap_or_default();

    if !digest.success || found != self.sha256 {
      return Err(format!(
        "{}: the archive's digest is {found}, not the pinned {}",
        self.url, self.sha256
      ));
    }

    let mut unpack = Command::new("tar");

    unpack.arg("-xf").arg(&archive).arg("-C").arg(&staging);
    log.check(unpack)?;
    fs::remove_file(&archive).map_err(|error| format!("{}: {error}", archive.display()))?;
    fs::rename(&staging, &directory)
      .map_err(|error| format!("{}: {error}", directory.display()))?;

    Ok(directory)
  }
}

/// Builds the conformance suite's app with the skene command, as an app is built, and
/// its harness.
fn build_suite(gate: &Gate, log: &mut Log) -> Result<Binary, String> {
  let directory = manifests::package_directory(&gate.root, SUITE.package)?;
  let mut app = gate.cargo(["run", "--locked", "-p", "skene-cli", "--", "--dir"]);

  app.arg(&directory).args(["build", "web", "--debug"]);
  log.check(app)?;
  log.check(gate.cargo([
    "build",
    "--locked",
    "-p",
    SUITE.package,
    "--bin",
    SUITE.harness,
  ]))?;

  Ok(Binary {
    package: SUITE.package,
    name: SUITE.harness.to_owned(),
    path: gate
      .target
      .join("debug")
      .join(format!("{}{}", SUITE.harness, env::consts::EXE_SUFFIX)),
    directory: directory.clone(),
    runner: Runner::Harness {
      site: directory.join("dist").join("web"),
    },
  })
}

impl Job<'_> {
  /// Logs what the job printed and returns what is wrong with it.
  fn problems(&self, log: &mut Log) -> Vec<String> {
    let binary = self.binary;
    let heading = format!("{} {} `{}`", self.engine, binary.package, binary.name);

    log.note(&format!("$ {heading}: {}", binary.path.display()));

    let (success, output) = match &self.outcome {
      None => return vec![format!("{heading}: never ran")],
      Some(Err(error)) => return vec![format!("{heading}: {error}")],
      Some(Ok(ran)) => ran,
    };

    log.note(output);

    let expected = match binary.runner {
      Runner::Tests => RUNS
        .iter()
        .filter(|run| run.package == binary.package)
        .flat_map(|run| run.binaries)
        .find(|(name, _)| *name == binary.name)
        .map(|(_, count)| *count),
      Runner::Harness { .. } => Some(SUITE.scenarios),
      Runner::Goldens { .. } => Some(SUITE.pages),
    };
    let mut problems = Vec::new();

    match (results(output), expected) {
      (None, Some(_)) => problems.push(format!("{heading}: reported no results")),
      (None, None) if !success => problems.push(format!("{heading}: failed")),
      (None, None) => {}
      (Some(Results { passed, failed }), expected) => {
        if failed > 0 || !success {
          problems.push(format!("{heading}: failed {failed} tests"));
        }

        if let Some(expected) = expected
          && passed < expected
        {
          problems.push(format!(
            "{heading}: passed {passed} tests, fewer than {expected}"
          ));
        }
      }
    }

    problems
  }
}

/// Runs every job, as many at once as [`sessions`] allows, recording what each printed.
fn run_jobs(jobs: &mut [Job<'_>], scratch: &Path) {
  let queue = Mutex::new(jobs.iter_mut().collect::<Vec<_>>());

  thread::scope(|scope| {
    for _ in 0..sessions() {
      scope.spawn(|| {
        while let Some(job) = queue.lock().ok().and_then(|mut queue| queue.pop()) {
          let mut command = match &job.binary.runner {
            Runner::Tests => {
              let mut command = Command::new("wasm-bindgen-test-runner");

              command
                .arg(&job.binary.path)
                .env("CARGO_MANIFEST_DIR", &job.binary.directory)
                .env("WASM_BINDGEN_TEST_TIMEOUT", BINARY_TIMEOUT);

              command
            }
            Runner::Harness { site } => {
              let mut command = Command::new(&job.binary.path);

              command.arg(site);

              command
            }
            Runner::Goldens {
              site,
              goldens,
              firefox,
            } => {
              let mut command = Command::new(&job.binary.path);
              let path = env::var_os("PATH").unwrap_or_default();
              let path =
                env::join_paths(std::iter::once(firefox.clone()).chain(env::split_paths(&path)))
                  .unwrap_or(path);

              command
                .arg(site)
                .arg("--goldens")
                .arg(goldens)
                .args(["--firefox", FIREFOX_VERSION])
                .env("PATH", path);

              command
            }
          };

          command
            .current_dir(&job.binary.directory)
            .env("TMPDIR", scratch);

          for engine in ENGINES {
            command.env_remove(engine.variable);
          }

          command.env(job.variable, &job.driver);

          job.outcome = Some(
            command
              .output()
              .map(|output| {
                let mut printed = String::from_utf8_lossy(&output.stdout).into_owned();

                printed.push_str(&String::from_utf8_lossy(&output.stderr));

                (output.status.success(), printed)
              })
              .map_err(|error| format!("wasm-bindgen-test-runner: {error}")),
          );
        }
      });
    }
  });
}

/// Each test executable a `cargo test --no-run` built, by its source's name, with its
/// path relative to the workspace.
fn executables(output: &str) -> Vec<(String, PathBuf)> {
  output
    .lines()
    .filter_map(|line| {
      let (source, path) = line
        .trim()
        .strip_prefix("Executable ")?
        .strip_suffix(')')?
        .split_once(" (")?;
      let source = source.strip_prefix("unittests ").unwrap_or(source);
      let name = Path::new(source).file_stem()?.to_str()?.to_owned();

      Some((name, PathBuf::from(path)))
    })
    .collect()
}

/// Checks that the installed test runner is the version of `wasm-bindgen` the lockfile
/// pins.
fn check_runner(gate: &Gate) -> Outcome {
  let locked = manifests::locked_version(&gate.root, "wasm-bindgen")?;
  let install = format!("`cargo install wasm-bindgen-cli --version {locked} --locked`");
  let output = Command::new("wasm-bindgen-test-runner")
    .arg("--version")
    .output()
    .map_err(|error| format!("wasm-bindgen-test-runner is missing ({error}): {install}"))?;
  let version = String::from_utf8_lossy(&output.stdout);
  let installed = version.split_whitespace().last().unwrap_or_default();

  if installed == locked {
    Ok(())
  }
  else {
    Err(format!(
      "wasm-bindgen-test-runner {installed} doesn't match wasm-bindgen {locked}: {install}"
    ))
  }
}

/// What one binary's run reported in `output`, where it reported a summary.
fn results(output: &str) -> Option<Results> {
  output
    .lines()
    .rev()
    .find_map(|line| line.trim().strip_prefix("test result: "))
    .map(|summary| Results {
      passed: count(summary, "passed"),
      failed: count(summary, "failed"),
    })
}

/// The number before `label` in a test summary such as `ok. 48 passed; 0 failed`.
fn count(summary: &str, label: &str) -> usize {
  summary
    .split(';')
    .find_map(|part| {
      let words: Vec<&str> = part.split_whitespace().collect();

      match words.as_slice() {
        [.., number, last] if *last == label => number.parse().ok(),
        _ => None,
      }
    })
    .unwrap_or(0)
}
