//! The browser the harness drives: its driver named as the browser tier names it, the
//! driver started on a free local port, and a session on it.

use std::{
  env,
  fs,
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

/// The fontconfig the browser taking goldens reads: no system font at all, and the
/// font a page draws in rasterized alike on every machine, unhinted and in grayscale.
const FONTCONFIG: &str = r#"<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <match target="pattern">
    <edit name="antialias" mode="assign"><bool>true</bool></edit>
    <edit name="hinting" mode="assign"><bool>false</bool></edit>
    <edit name="hintstyle" mode="assign"><const>hintnone</const></edit>
    <edit name="rgba" mode="assign"><const>none</const></edit>
    <edit name="lcdfilter" mode="assign"><const>lcdnone</const></edit>
  </match>
</fontconfig>
"#;

/// What a session is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Purpose {
  /// Acting out the scenarios.
  Scenarios,
  /// Taking the screenshot goldens: Firefox alone, drawing alike on every machine.
  Goldens,
}

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

  /// What a session for `purpose` asks of the browser: headless, scripts given a minute,
  /// and for the goldens one pixel to the CSS pixel, drawn in software, light, without
  /// motion.
  fn capabilities(&self, purpose: Purpose) -> Value {
    let timeouts = json!({ "script": SCRIPT_TIMEOUT });

    match self.kind {
      Kind::Firefox if purpose == Purpose::Goldens => json!({
        "browserName": "firefox",
        "moz:firefoxOptions": {
          "args": ["-headless"],
          "prefs": {
            "gfx.webrender.software": true,
            "layout.css.devPixelsPerPx": "1.0",
            "layout.css.prefers-color-scheme.content-override": 1,
            "ui.prefersReducedMotion": 1,
          },
        },
        "timeouts": timeouts,
      }),
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

  /// Starts the driver on a free local port and a session on it for `purpose`.
  pub(crate) fn start(&self, purpose: Purpose) -> Result<Started, String> {
    if purpose == Purpose::Goldens && self.kind != Kind::Firefox {
      return Err("the goldens are taken in Firefox alone; set GECKODRIVER".to_owned());
    }

    let port = TcpListener::bind("127.0.0.1:0")
      .and_then(|listener| listener.local_addr())
      .map_err(|error| error.to_string())?
      .port();
    let mut command = Command::new(&self.driver);

    if purpose == Purpose::Goldens {
      let fonts = env::temp_dir().join(format!("skene-goldens-{}.conf", std::process::id()));

      fs::write(&fonts, FONTCONFIG).map_err(|error| format!("{}: {error}", fonts.display()))?;
      command.env("FONTCONFIG_FILE", fonts);
    }

    let process = command
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
      &self.capabilities(purpose),
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
