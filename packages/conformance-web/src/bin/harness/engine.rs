//! The browser the harness drives: its driver named as the browser tier names it, the
//! driver started on a free local port, and a session on it.

use std::{
  env,
  net::TcpListener,
  path::PathBuf,
  process::{Child, Command, Stdio},
  time::Duration,
};

use serde_json::{Value, json};

use crate::session::Session;

/// How long a driver may take to come up.
const STARTUP: Duration = Duration::from_secs(30);

/// How long a script may run before WebDriver gives up on it.
const SCRIPT_TIMEOUT: u64 = 60_000;

/// A browser engine, by the variable naming its driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
  /// Firefox, through geckodriver (`GECKODRIVER`).
  Firefox,
  /// Chromium, through chromedriver (`CHROMEDRIVER`).
  Chromium,
}

/// The engine the harness drives: its kind and its driver.
pub(crate) struct Engine {
  /// Which browser it is.
  kind: Kind,
  /// The driver's executable.
  driver: PathBuf,
}

impl Engine {
  /// The engine the one of `GECKODRIVER` and `CHROMEDRIVER` that is set names.
  pub(crate) fn from_environment() -> Result<Self, String> {
    let named: Vec<Self> = [
      (Kind::Firefox, "GECKODRIVER"),
      (Kind::Chromium, "CHROMEDRIVER"),
    ]
    .into_iter()
    .filter_map(|(kind, variable)| {
      env::var_os(variable).map(|driver| Self {
        kind,
        driver: PathBuf::from(driver),
      })
    })
    .collect();

    match <[Self; 1]>::try_from(named) {
      Ok([engine]) => Ok(engine),
      Err(_) => Err("set exactly one of GECKODRIVER and CHROMEDRIVER to its driver".to_owned()),
    }
  }

  /// The browser's name in a report.
  pub(crate) const fn name(&self) -> &'static str {
    match self.kind {
      Kind::Firefox => "Firefox",
      Kind::Chromium => "Chromium",
    }
  }

  /// What a session asks of the browser: headless, scripts given a minute.
  fn capabilities(&self) -> Value {
    let timeouts = json!({ "script": SCRIPT_TIMEOUT });

    match self.kind {
      Kind::Firefox => json!({
        "browserName": "firefox",
        "moz:firefoxOptions": { "args": ["-headless"] },
        "timeouts": timeouts,
      }),
      Kind::Chromium => json!({
        "browserName": "chrome",
        "goog:chromeOptions": {
          "args": ["--headless=new", "--no-sandbox", "--disable-dev-shm-usage"],
        },
        "timeouts": timeouts,
      }),
    }
  }

  /// Starts the driver on a free local port and a session on it.
  pub(crate) fn start(&self) -> Result<Started, String> {
    let port = TcpListener::bind("127.0.0.1:0")
      .and_then(|listener| listener.local_addr())
      .map_err(|error| error.to_string())?
      .port();
    let process = Command::new(&self.driver)
      .arg(format!("--port={port}"))
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .map_err(|error| format!("{}: {error}", self.driver.display()))?;
    let mut started = Started {
      process,
      session: None,
    };

    started.session = Some(Session::start(
      &format!("http://127.0.0.1:{port}"),
      &self.capabilities(),
      STARTUP,
    )?);

    Ok(started)
  }
}

/// A running driver and the session on it: the session ended and the driver stopped
/// when dropped.
pub(crate) struct Started {
  /// The driver's process.
  process: Child,
  /// The session, once started.
  session: Option<Session>,
}

impl Started {
  /// The session on the driver.
  pub(crate) const fn session(&self) -> &Session {
    self
      .session
      .as_ref()
      .expect("a started driver holds its session")
  }
}

impl Drop for Started {
  fn drop(&mut self) {
    if let Some(session) = self.session.take() {
      session.end();
    }

    let _ = self.process.kill();
    let _ = self.process.wait();
  }
}
