//! # The harness
//!
//! Runs the conformance suite in a real browser: serves the built app (the crate's
//! library, built by `skene build web`) from the directory its one argument names,
//! starts the browser's driver that `GECKODRIVER` or `CHROMEDRIVER` names, and acts out
//! every scenario through a WebDriver session, each in a fresh window on the page it
//! names, by a user's trusted input. Reports as a test binary does, a line per scenario
//! and a `test result:` line, and fails where any scenario fails.

#[cfg(not(target_arch = "wasm32"))]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod engine;
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

/// Runs the suite, returning whether every scenario passed.
#[cfg(not(target_arch = "wasm32"))]
fn run() -> Result<bool, String> {
  use std::{path::PathBuf, time::Instant};

  use skene_conformance::scenarios;

  use crate::{browser::Browser, engine::Engine, site::Site};

  let root = std::env::args_os()
    .nth(1)
    .map(PathBuf::from)
    .ok_or("usage: harness <the built app's directory>")?;
  let site = Site::serve(root)?;
  let engine = Engine::from_environment()?;
  let started = engine.start()?;
  let session = started.session();
  let mut browser = Browser::new(session, &site);
  let all = scenarios::<Browser>();
  let mut failures = Vec::new();

  println!("\nrunning {} scenarios in {}", all.len(), engine.name());

  for scenario in &all {
    let began = Instant::now();
    let outcome = scenario.check(&mut browser);

    println!(
      "test {} ... {} ({:.1}s)",
      scenario.name,
      if outcome.is_ok() { "ok" } else { "FAILED" },
      began.elapsed().as_secs_f32()
    );

    if let Err(failure) = outcome {
      failures.push(format!("{}: {failure}", scenario.name));
    }
  }

  if !failures.is_empty() {
    println!("\nfailures:\n    {}", failures.join("\n    "));
  }

  println!(
    "\ntest result: {}. {} passed; {} failed\n",
    if failures.is_empty() { "ok" } else { "FAILED" },
    all.len() - failures.len(),
    failures.len()
  );

  Ok(failures.is_empty())
}
