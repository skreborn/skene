//! # The harness
//!
//! Runs the conformance suite in a real browser: serves the built app (the crate's
//! library, built by `skene build web`) from the directory its first argument names,
//! starts the browser's driver that `GECKODRIVER` or `CHROMEDRIVER` names, and acts out
//! every scenario through a WebDriver session, each in a fresh window on the page it
//! names, by a user's trusted input.
//!
//! `harness <app> --goldens <directory> --firefox <version>` takes the screenshot
//! goldens instead: every page as the Firefox of that version draws it in the pinned
//! font, compared against `<directory>/<page>.png`, and rewritten there under
//! `SKENE_GOLDENS=overwrite`.
//!
//! Reports as a test binary does, a line per scenario or page and a `test result:`
//! line, and fails where any fails.

#[cfg(not(target_arch = "wasm32"))]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod engine;
#[cfg(not(target_arch = "wasm32"))]
mod goldens;
#[cfg(not(target_arch = "wasm32"))]
mod session;
#[cfg(not(target_arch = "wasm32"))]
mod site;

#[cfg(target_arch = "wasm32")]
const fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
  match run() {
    Ok(true) => std::process::ExitCode::SUCCESS,
    Ok(false) => std::process::ExitCode::FAILURE,
    Err(message) => {
      eprintln!("error: {message}");

      std::process::ExitCode::FAILURE
    }
  }
}

/// What the arguments ask for.
#[cfg(not(target_arch = "wasm32"))]
struct Arguments {
  /// The directory the app was built into.
  site: std::path::PathBuf,
  /// Where the goldens are kept and the Firefox version they hold in, when the goldens
  /// are asked for.
  goldens: Option<(std::path::PathBuf, String)>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Arguments {
  /// The arguments the harness was given.
  fn parse() -> Result<Self, String> {
    const USAGE: &str =
      "usage: harness <the built app's directory> [--goldens <directory> --firefox <version>]";

    let mut arguments = std::env::args_os().skip(1);
    let site = arguments.next().ok_or(USAGE)?.into();
    let rest: Vec<String> = arguments
      .map(|argument| argument.to_string_lossy().into_owned())
      .collect();
    let goldens = match rest.as_slice() {
      [] => None,
      [flag, directory, firefox, version] if flag == "--goldens" && firefox == "--firefox" => {
        Some((directory.into(), version.clone()))
      }
      _ => return Err(USAGE.to_owned()),
    };

    Ok(Self { site, goldens })
  }
}

/// Runs the suite or takes the goldens, returning whether every one passed.
#[cfg(not(target_arch = "wasm32"))]
fn run() -> Result<bool, String> {
  use crate::{
    browser::Browser,
    engine::{Engine, Purpose},
    site::Site,
  };

  let arguments = Arguments::parse()?;
  let site = Site::serve(arguments.site)?;
  let engine = Engine::from_environment()?;
  let purpose = if arguments.goldens.is_some() {
    Purpose::Goldens
  }
  else {
    Purpose::Scenarios
  };
  let started = engine.start(purpose)?;
  let session = started.session();
  let mut browser = Browser::new(session, &site);
  let mut report = Report::default();

  if let Some((directory, version)) = &arguments.goldens {
    if session.version() != version {
      return Err(format!(
        "the goldens hold in Firefox {version}, and this is Firefox {}",
        session.version()
      ));
    }

    let pages = skene_conformance::pages::<skene_web::Active>();

    println!(
      "\nrunning {} goldens in {} {}",
      pages.len(),
      engine.name(),
      session.version()
    );

    for page in pages {
      let began = std::time::Instant::now();
      let verdict = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        goldens::check(&mut browser, directory, page.name)
      }))
      .unwrap_or_else(|panic| Err(message(&*panic)));

      report.record(&format!("golden {}", page.name), began, verdict);
    }
  }
  else {
    let scenarios = skene_conformance::scenarios::<Browser>();

    println!(
      "\nrunning {} scenarios in {} {}",
      scenarios.len(),
      engine.name(),
      session.version()
    );

    for scenario in &scenarios {
      let began = std::time::Instant::now();
      let outcome = scenario.check(&mut browser);

      report.record(scenario.name, began, outcome);
    }
  }

  Ok(report.finish())
}

/// What a panic said.
#[cfg(not(target_arch = "wasm32"))]
fn message(panic: &(dyn std::any::Any + Send)) -> String {
  panic
    .downcast_ref::<String>()
    .cloned()
    .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
    .unwrap_or_else(|| "a panic without a message".to_owned())
}

/// What a run found: how many passed, and what each failure reported.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Report {
  passed: usize,
  failures: Vec<String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Report {
  /// Records and prints the outcome of `name`, begun at `began`.
  fn record(&mut self, name: &str, began: std::time::Instant, outcome: Result<(), String>) {
    println!(
      "test {name} ... {} ({:.1}s)",
      if outcome.is_ok() { "ok" } else { "FAILED" },
      began.elapsed().as_secs_f32()
    );

    match outcome {
      Ok(()) => self.passed += 1,
      Err(failure) => self.failures.push(format!("{name}: {failure}")),
    }
  }

  /// Prints the failures and the summary, returning whether nothing failed.
  fn finish(self) -> bool {
    if !self.failures.is_empty() {
      println!("\nfailures:\n    {}", self.failures.join("\n    "));
    }

    println!(
      "\ntest result: {}. {} passed; {} failed\n",
      if self.failures.is_empty() {
        "ok"
      }
      else {
        "FAILED"
      },
      self.passed,
      self.failures.len()
    );

    self.failures.is_empty()
  }
}
