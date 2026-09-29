//! The browser tier: every browser test binary, run in Firefox through geckodriver.

use std::{collections::HashMap, env, fs, path::Path, process::Command};

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
    package: "skene-gui",
    arguments: &[],
    binaries: &[
      ("web_dom", 65),
      ("web_gpu_loss", 1),
      ("web_hydrate", 13),
      ("web_painted", 3),
      ("web_parity", 1),
      ("web_resource", 5),
      ("web_task", 4),
    ],
  },
  Run {
    package: "skene-web",
    arguments: &[],
    binaries: &[("web_dom", 15)],
  },
  Run {
    package: "skene-transport",
    arguments: &[],
    binaries: &[("web_http", 2), ("web_ws", 2)],
  },
  Run {
    package: "acme-map",
    arguments: &[],
    binaries: &[("web_map", 1)],
  },
  Run {
    package: "gallery",
    arguments: &["--test", "web_axe"],
    binaries: &[("web_axe", 1)],
  },
];

/// The geckodriver bundled in Firefox's snap, which runs outside the snap's confinement.
const SNAP_GECKODRIVER: &str = "/snap/firefox/current/usr/lib/firefox/geckodriver";

/// What one binary reported.
struct Results {
  passed: usize,
  failed: usize,
}

/// Links every library's tests for the browser, then runs every browser binary and
/// checks each against its expected count.
pub(crate) fn run(gate: &Gate, log: &mut Log) -> Outcome {
  check_runner(gate)?;
  log.check(gate.cargo([
    "test",
    "--locked",
    "--workspace",
    "--lib",
    "--no-run",
    "--target",
    WASM,
  ]))?;

  let scratch = gate.target.join("gate").join("tmp");

  fs::create_dir_all(&scratch).map_err(|error| format!("{}: {error}", scratch.display()))?;

  let mut problems = Vec::new();

  for run in RUNS {
    let mut command = gate.cargo([
      "test",
      "--locked",
      "--no-fail-fast",
      "--target",
      WASM,
      "-p",
      run.package,
    ]);

    command.args(run.arguments).env("TMPDIR", &scratch);

    if env::var_os("GECKODRIVER").is_none() && Path::new(SNAP_GECKODRIVER).exists() {
      command.env("GECKODRIVER", SNAP_GECKODRIVER);
    }

    let ran = log.run(command)?;
    let results = results(&ran.output);
    let mut found = Vec::new();

    for &(binary, expected) in run.binaries {
      match results.get(binary) {
        None => found.push(format!("{}: `{binary}` reported no results", run.package)),
        Some(&Results { passed, failed }) => {
          if failed > 0 {
            found.push(format!("{}: `{binary}` failed {failed} tests", run.package));
          }

          if passed < expected {
            found.push(format!(
              "{}: `{binary}` passed {passed} tests, fewer than {expected}",
              run.package
            ));
          }
        }
      }
    }

    if !ran.success && found.is_empty() {
      found.push(format!("{}: `cargo test` failed", run.package));
    }

    problems.extend(found);
  }

  if problems.is_empty() {
    Ok(())
  }
  else {
    Err(problems.join("\n"))
  }
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

/// Each test binary cargo ran in `output`, by its source's name, with what it reported.
fn results(output: &str) -> HashMap<String, Results> {
  let mut binary = None;
  let mut results = HashMap::new();

  for line in output.lines().map(str::trim) {
    if let Some(target) = line.strip_prefix("Running ")
      && let Some((source, _)) = target
        .strip_suffix(')')
        .and_then(|named| named.split_once(" ("))
    {
      let source = source.strip_prefix("unittests ").unwrap_or(source);

      binary = Path::new(source)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned);
    }
    else if let Some(summary) = line.strip_prefix("test result: ")
      && let Some(name) = binary.take()
    {
      results.insert(
        name,
        Results {
          passed: count(summary, "passed"),
          failed: count(summary, "failed"),
        },
      );
    }
  }

  results
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
