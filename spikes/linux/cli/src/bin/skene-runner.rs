//! `skene-runner`: the target runner `skene new` names, which cargo runs every test binary,
//! doctest and app of a Linux build through.
//!
//! ```text
//! skene-runner <program> [arguments...]
//! ```
//!
//! - **A test** (a binary in a `deps` directory, an example cargo built as a test, or a
//!   doctest) always gets a session of its own, the leg `SKENE_TEST_SESSION` names:
//!   - `x11` (the default): a private X server: Xorg on its dummy video driver with an inputtest
//!     touchscreen where both drivers are installed (the test told the touchscreen's socket,
//!     `SKENE_TOUCH_SOCKET`), Xvfb otherwise or where `SKENE_X11_SERVER=xvfb`;
//!   - `wayland` or `mutter`: headless mutter (a virtual monitor, its RemoteDesktop service
//!     over a private PipeWire, which the test's input goes through, and its ScreenCast
//!     service, which the test reads the screen through, WirePlumber linking the streams);
//!   - `sway`: headless sway (pixman), its virtual pointer and keyboard; with
//!     `SKENE_SWAY_BACKEND=x11`, sway on its X11 backend instead, its output a window filling
//!     the screen of a private Xorg with an inputtest touchscreen (as the `x11` leg's), which
//!     takes touch (sway 1.9's X11 backend reports a window's frame presented only once the
//!     output shows a later one, so a recording there misses the last frame);
//!   - `weston`: weston on its X11 backend (pixman), its output a window of a private Xvfb
//!     whose XTest the test's input goes through;
//!   - `kwin`: KWin's windowed backend (QPainter), its output a window of a private Xvfb, its
//!     fake input, a restricted interface KWin grants by a desktop file of the session's to
//!     `kwin-fake-input` beside the runner (or `SKENE_KWIN_FAKE_INPUT`) alone, which the test
//!     acts through (`SKENE_KWIN_FAKE_INPUT`), and as its input method the stand-in
//!     `kwin-input-method` beside the runner (or `SKENE_KWIN_INPUT_METHOD`) where it is there;
//!     `kwin-x11`: the same, the test's windows on KWin's Xwayland.
//!     `SKENE_KWIN` names the `kwin_wayland` to run, where the one on `PATH` cannot be (its
//!     file capability beyond the process's bounding set);
//!
//!   `SKENE_TEST_SCALE` (1 where unset) scales the leg's screen fractionally, as large in logical
//!   pixels at every scale: sway's output scale, KWin's `--scale` and mutter's monitor scale (its
//!   `scale-monitor-framebuffer` feature on, the scale in its `monitors.xml`) reach the app through
//!   fractional-scale-v1; on `kwin-x11` Xwayland shows the screen's device pixels (KWin's
//!   `[Xwayland] Scale`, as a desktop leaving X11 apps to scale themselves sets it) and the test
//!   stands in for the desktop's XSETTINGS daemon, publishing `Xft/DPI`; on `x11` it is the
//!   realization's `SKENE_SCALE` (a bare Xvfb has no desktop to publish `Xft/DPI`); the `weston`
//!   leg takes 1 alone (weston 13 offers no fractional-scale-v1);
//!
//!   each with a private D-Bus session (which starts the accessibility bus on demand), a
//!   private runtime directory and configuration home, and none of the user's display,
//!   accessibility bus, input method or scale settings. The test is told its leg
//!   (`SKENE_TEST_SESSION`) and its display (`SKENE_DISPLAY`); where a nested compositor
//!   shows its output in a window of the private Xvfb (weston, KWin), that X display
//!   (`SKENE_OUTPUT_DISPLAY`), which the test reads the screen through; and where the
//!   compositor draws the pointer into its output (weston, KWin, sway's pixman renderer), a
//!   cursor theme whose every cursor is transparent (`XCURSOR_THEME`, `XCURSOR_PATH`), so
//!   what the test reads of the screen is the windows alone. It never touches the user's
//!   display, so parallel test processes (cargo-nextest's) share no pointer, keyboard focus or
//!   window stack, and a test reads what a screen reader reads of its own app alone. Listing
//!   the tests (`--list`) needs none of it and runs with no display at all. A test that had
//!   nothing to test on its leg says so in the file `SKENE_NOT_TESTED` names, and the runner
//!   lists those after the binary's own results: each passed without testing anything there.
//! - **An app** runs on the user's display where there is one (`DISPLAY` or
//!   `WAYLAND_DISPLAY`), and on a private Xvfb otherwise; it gets a private session bus where
//!   the user has none.
//!
//! What the runner starts ends with it: on a normal exit, on `SIGINT`, `SIGTERM`, `SIGHUP` or
//! `SIGQUIT` (forwarded to the program first, then raised again once the session is gone), and
//! when the runner itself is killed (each process it starts is signalled when it dies).
//!
//! `SKENE_RUNNER=test` or `SKENE_RUNNER=app` overrides the classification; the runner's own
//! failures exit with 101, as a failed test does.

use std::{
  env,
  ffi::{OsStr, c_ulong},
  fs,
  io::{BufRead as _, BufReader, Read as _},
  os::unix::{fs::PermissionsExt as _, process::CommandExt as _},
  path::{Path, PathBuf},
  process::{Child, Command, ExitCode, Stdio},
  sync::atomic::{AtomicI32, Ordering},
  time::{Duration, Instant},
};

unsafe extern "C" {
  /// POSIX `kill(2)`.
  fn kill(pid: i32, signal: i32) -> i32;
  /// POSIX `signal(2)`, the handler an address.
  fn signal(signal: i32, handler: usize) -> usize;
  /// POSIX `raise(3)`.
  fn raise(signal: i32) -> i32;
  /// Linux `prctl(2)`.
  fn prctl(option: i32, ...) -> i32;
}

/// `SIGTERM`.
const TERMINATE: i32 = 15;

/// The signals that end the runner, each forwarded to the program before the session ends.
const ENDING: [i32; 4] = [1, 2, 3, TERMINATE];

/// `SIG_DFL`.
const DEFAULT: usize = 0;

/// `PR_SET_PDEATHSIG`.
const PARENT_DEATH_SIGNAL: i32 = 1;

/// How long Xvfb and the bus have to come up.
const START_TIMEOUT: Duration = Duration::from_secs(10);

/// The program's process id while it runs, 0 otherwise.
static PROGRAM: AtomicI32 = AtomicI32::new(0);

/// The ending signal the runner received, 0 while none.
static RECEIVED: AtomicI32 = AtomicI32::new(0);

/// Records an ending signal and forwards it to the program.
extern "C" fn forward(received: i32) {
  RECEIVED.store(received, Ordering::SeqCst);

  let program = PROGRAM.load(Ordering::SeqCst);

  if program > 0 {
    // SAFETY: `kill` is async-signal-safe and reads its two integers alone.
    unsafe {
      kill(program, received);
    }
  }
}

/// The variables a test never takes from the user's session: its display, accessibility bus,
/// input method and scale are the runner's.
const USER_ONLY: [&str; 14] = [
  "DISPLAY",
  "WAYLAND_DISPLAY",
  "SWAYSOCK",
  "AT_SPI_BUS_ADDRESS",
  "GDK_SCALE",
  "GDK_DPI_SCALE",
  "NO_AT_BRIDGE",
  "GTK_IM_MODULE",
  "QT_IM_MODULE",
  "XMODIFIERS",
  "IBUS_ADDRESS",
  "SKENE_IME",
  "SKENE_WINDOW_SYSTEM",
  "SKENE_TOUCH_SOCKET",
];

/// The virtual screen a test's session shows, in logical pixels.
const SCREEN: (u32, u32) = (1280, 1024);

/// The virtual screen a test's session shows at `scale`, in device pixels: [`SCREEN`] times
/// the scale, or just under it where that would not be whole pixels, so the compositor's
/// logical space maps onto the device pixels exactly.
fn screen_at(scale: f64) -> (u32, u32) {
  let whole = |logical: u32| {
    let mut logical = f64::from(logical);

    while logical > 1. && ((logical * scale) - (logical * scale).round()).abs() > 1e-9 {
      logical -= 1.;
    }

    (logical * scale).round() as u32
  };

  (whole(SCREEN.0), whole(SCREEN.1))
}

/// The scale `SKENE_TEST_SCALE` asks a test's session for: 1 where unset.
///
/// # Errors
///
/// Where it is not a number from 1 to 4.
fn test_scale() -> Result<f64, String> {
  match env::var("SKENE_TEST_SCALE") {
    Err(_) => Ok(1.),
    Ok(text) => text
      .parse::<f64>()
      .ok()
      .filter(|scale| (1. ..=4.).contains(scale))
      .ok_or_else(|| format!("SKENE_TEST_SCALE={text}: a scale is a number from 1 to 4")),
  }
}

/// Which private session a test gets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Leg {
  /// Xvfb.
  X11,
  /// Headless mutter, with PipeWire for its RemoteDesktop service.
  Mutter,
  /// Headless sway.
  Sway,
  /// Weston on its X11 backend in a private Xvfb.
  Weston,
  /// KWin's windowed backend in a private Xvfb.
  Kwin,
  /// KWin's windowed backend in a private Xvfb, the test's windows on its Xwayland.
  KwinX11,
}

impl Leg {
  /// The leg `SKENE_TEST_SESSION` names, X11 where it names none.
  fn chosen() -> Result<Self, String> {
    match env::var("SKENE_TEST_SESSION").as_deref() {
      Err(_) | Ok("" | "x11") => Ok(Self::X11),
      Ok("wayland" | "mutter") => Ok(Self::Mutter),
      Ok("sway") => Ok(Self::Sway),
      Ok("weston") => Ok(Self::Weston),
      Ok("kwin") => Ok(Self::Kwin),
      Ok("kwin-x11") => Ok(Self::KwinX11),
      Ok(other) => Err(format!(
        "SKENE_TEST_SESSION={other}: the legs are x11 (the default), wayland (headless mutter), sway, weston, kwin and kwin-x11"
      )),
    }
  }

  /// The name the test is told.
  const fn name(self) -> &'static str {
    match self {
      Self::X11 => "x11",
      Self::Mutter => "mutter",
      Self::Sway => "sway",
      Self::Weston => "weston",
      Self::Kwin => "kwin",
      Self::KwinX11 => "kwin-x11",
    }
  }

  /// Whether the test's windows go to X11.
  const fn is_x11(self) -> bool {
    matches!(self, Self::X11 | Self::KwinX11)
  }

  /// Whether the leg's compositor draws the pointer into the output a test reads.
  const fn draws_pointer(self) -> bool {
    matches!(self, Self::Sway | Self::Weston | Self::Kwin | Self::KwinX11)
  }
}

fn main() -> ExitCode {
  let mut arguments = env::args_os().skip(1);
  let Some(program) = arguments.next() else {
    eprintln!("usage: skene-runner <program> [arguments...]");

    return ExitCode::from(101);
  };
  let program = PathBuf::from(program);
  let arguments: Vec<_> = arguments.collect();
  let test = match env::var("SKENE_RUNNER").as_deref() {
    Ok("test") => true,
    Ok("app") => false,
    _ => Kind::of(&program) == Kind::Test,
  };

  if test && arguments.iter().any(|argument| argument == "--list") {
    let mut command = Command::new(&program);

    command
      .args(&arguments)
      .env_remove("DISPLAY")
      .env_remove("DBUS_SESSION_BUS_ADDRESS");

    for variable in USER_ONLY {
      command.env_remove(variable);
    }

    return exec(&mut command);
  }

  let display = env::var_os("DISPLAY").is_some() || env::var_os("WAYLAND_DISPLAY").is_some();

  if !test && display && env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
    return exec(Command::new(&program).args(&arguments));
  }

  for ending in ENDING {
    // SAFETY: `forward` only stores to atomics and calls `kill`, both async-signal-safe.
    unsafe {
      signal(ending, forward as extern "C" fn(i32) as usize);
    }
  }

  let code = match Session::start(test, !test && display) {
    Ok(session) => {
      let status = if RECEIVED.load(Ordering::SeqCst) == 0 {
        session.run(Command::new(&program).args(&arguments))
      } else {
        Ok(ExitCode::from(101))
      };

      session.stop();

      match status {
        Ok(code) => code,
        Err(error) => {
          eprintln!("skene-runner: {error}");

          ExitCode::from(101)
        }
      }
    }
    Err(error) => {
      eprintln!("skene-runner: {error}");

      ExitCode::from(101)
    }
  };
  let received = RECEIVED.load(Ordering::SeqCst);

  if received != 0 {
    // SAFETY: restoring the default action and raising the signal read integers alone.
    unsafe {
      signal(received, DEFAULT);
      raise(received);
    }
  }

  code
}

/// Replaces this process with `command`.
fn exec(command: &mut Command) -> ExitCode {
  let error = command.exec();

  eprintln!("skene-runner: could not run the program: {error}");

  ExitCode::from(101)
}

/// What cargo asked the runner to run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
  /// A test binary or a doctest.
  Test,
  /// An app (`cargo run`).
  App,
}

impl Kind {
  /// A binary cargo built in a `deps` directory is a test (`cargo run` runs the binary cargo
  /// copied out of it), and so is an example built as a test (`examples/<name>-<hash>`, where
  /// `cargo run --example` runs `examples/<name>`) and a doctest rustdoc compiled.
  fn of(program: &Path) -> Self {
    let directory = program.parent().and_then(Path::file_name);
    let in_deps = directory.is_some_and(|name| name == "deps");
    let example_test = directory.is_some_and(|name| name == "examples")
      && program
        .file_name()
        .and_then(OsStr::to_str)
        .and_then(|name| name.rsplit_once('-'))
        .is_some_and(|(_, hash)| {
          hash.len() == 16
            && hash
              .bytes()
              .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        });
    let doctest = program.components().any(|part| {
      part
        .as_os_str()
        .to_string_lossy()
        .starts_with("rustdoctest")
    });

    if in_deps || example_test || doctest {
      Self::Test
    } else {
      Self::App
    }
  }
}

/// A private display where the user's is not used, a private session bus where one is
/// needed, and for a test a private runtime directory and configuration home.
struct Session {
  /// What the session started, in order: stopped the other way round.
  helpers: Vec<Child>,
  /// The display variable the program is given, and its value, where the session made one.
  display: Option<(&'static str, String)>,
  /// The session bus's address, where the session started one.
  bus: Option<String>,
  /// The session's scratch directory, removed after.
  scratch: PathBuf,
  /// The leg of a test's session; `None` for an app.
  test: Option<Leg>,
  /// `SWAYSOCK` of a sway leg.
  sway_socket: Option<PathBuf>,
  /// More variables the test is given: the X display a nested compositor shows in, the
  /// Wayland display beside an Xwayland one, the realization's scale on a scaled X11 leg.
  extra: Vec<(&'static str, String)>,
  /// The directory of the transparent cursor theme, where the leg's compositor draws the
  /// pointer into its output: the compositor and the test both take it.
  cursors: Option<PathBuf>,
  /// The X11 leg's touchscreen socket where it lies outside the scratch directory, removed
  /// after.
  outside: Option<PathBuf>,
}

impl Session {
  /// Starts a test's session (`test`), or an app's: Xvfb unless `user_display` keeps the
  /// user's, and the bus where the user has none.
  fn start(test: bool, user_display: bool) -> Result<Self, String> {
    let stamp = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map_or(0, |since| since.as_nanos());
    let scratch = env::temp_dir().join(format!("skene-runner-{}-{stamp}", std::process::id()));

    fs::create_dir_all(scratch.join("config"))
      .and_then(|()| fs::create_dir_all(scratch.join("runtime")))
      .and_then(|()| {
        fs::set_permissions(scratch.join("runtime"), fs::Permissions::from_mode(0o700))
      })
      .and_then(|()| compose_key(&scratch.join("config")))
      .map_err(|error| format!("{}: {error}", scratch.display()))?;

    let mut session = Self {
      helpers: Vec::new(),
      display: None,
      bus: None,
      scratch,
      test: None,
      sway_socket: None,
      extra: Vec::new(),
      cursors: None,
      outside: None,
    };
    let started = if test {
      Leg::chosen().and_then(|leg| {
        session.test = Some(leg);
        session.start_test(leg)
      })
    } else {
      session.start_app(user_display)
    };

    match started {
      Ok(()) => Ok(session),
      Err(error) => {
        session.stop();

        Err(error)
      }
    }
  }

  /// The runtime directory a test's session lives in.
  fn runtime(&self) -> PathBuf {
    self.scratch.join("runtime")
  }

  /// An app's session: Xvfb where the user has no display, a bus where the user has none.
  fn start_app(&mut self, user_display: bool) -> Result<(), String> {
    if !user_display {
      self.start_xvfb()?;
    }

    if env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
      self.start_bus()?;
    }

    Ok(())
  }

  /// A test's session on `leg`.
  fn start_test(&mut self, leg: Leg) -> Result<(), String> {
    let scale = test_scale()?;

    if scale != 1. && leg == Leg::Weston {
      return Err(format!(
        "SKENE_TEST_SCALE={scale}: the weston leg takes scale 1 alone (weston 13 offers no fractional-scale-v1)"
      ));
    }

    if scale != 1. && leg == Leg::X11 {
      self.extra.push(("SKENE_SCALE", scale.to_string()));
    }

    if leg.draws_pointer() {
      let icons = self.scratch.join("icons");

      blank_cursors(&icons).map_err(|error| format!("{}: {error}", icons.display()))?;
      self.extra.push(("XCURSOR_THEME", BLANK_THEME.to_owned()));
      self
        .extra
        .push(("XCURSOR_PATH", icons.to_string_lossy().into_owned()));
      self.cursors = Some(icons);
    }

    match leg {
      Leg::X11 => {
        self.start_x11()?;
        self.start_bus()
      }
      Leg::Mutter => {
        self.start_bus()?;
        self.start_pipewire()?;
        self.start_wireplumber()?;
        self.start_mutter()
      }
      Leg::Sway => {
        let parent = self.start_sway_parent()?;

        self.start_bus()?;
        self.start_sway(parent.as_deref())
      }
      Leg::Weston => {
        self.start_xvfb()?;
        self.start_bus()?;
        self.start_weston()
      }
      Leg::Kwin | Leg::KwinX11 => {
        self.start_xvfb()?;
        self.start_bus()?;
        self.start_kwin(leg == Leg::KwinX11)
      }
    }
  }

  /// The X11 leg's X server: Xorg on its dummy video driver with an inputtest touchscreen,
  /// which takes touch as a real one's, where both drivers are installed and
  /// `SKENE_X11_SERVER` does not ask for Xvfb; Xvfb otherwise, and where Xorg does not come up.
  /// The test is told the touchscreen's socket (`SKENE_TOUCH_SOCKET`).
  fn start_x11(&mut self) -> Result<(), String> {
    let chosen = env::var("SKENE_X11_SERVER").unwrap_or_default();

    if chosen != "xvfb" && XORG_DRIVERS.iter().all(|driver| Path::new(driver).exists()) {
      match self.start_xorg() {
        Ok(()) => return Ok(()),
        Err(error) if chosen == "xorg" => return Err(error),
        Err(_) => {}
      }
    }

    self.start_xvfb()
  }

  /// Xorg on its dummy video driver, the screen as large as Xvfb's would be, its one input
  /// device an inputtest touchscreen (XInput 2.2 touch, the core pointer and keyboard XTest's),
  /// on the first free display number it finds itself.
  fn start_xorg(&mut self) -> Result<(), String> {
    let (width, height) = screen_at(test_scale()?);
    // A socket's path is at most 107 bytes: the runner's scratch directory where it fits.
    let touch = if self.scratch.as_os_str().len() < 90 {
      self.scratch.join("touch")
    } else {
      let outside = PathBuf::from(format!("/tmp/skene-touch-{}", std::process::id()));

      self.outside = Some(outside.clone());

      outside
    };
    let config = self.scratch.join("xorg.conf");
    let directory = self.scratch.join("xorg.conf.d");

    fs::create_dir_all(&directory)
      .and_then(|()| {
        fs::write(
          &config,
          format!(
            "Section \"ServerFlags\"\n  Option \"AutoAddDevices\" \"false\"\n  Option \"AutoEnableDevices\" \"false\"\n  Option \"AutoAddGPU\" \"false\"\n  Option \"DontVTSwitch\" \"true\"\nEndSection\n\
             Section \"Device\"\n  Identifier \"dummy\"\n  Driver \"dummy\"\n  VideoRam 256000\nEndSection\n\
             Section \"Monitor\"\n  Identifier \"monitor\"\n  HorizSync 1.0-2000.0\n  VertRefresh 1.0-200.0\n  Modeline \"screen\" {clock:.2} {width} {} {} {} {height} {} {} {}\nEndSection\n\
             Section \"Screen\"\n  Identifier \"screen\"\n  Device \"dummy\"\n  Monitor \"monitor\"\n  DefaultDepth 24\n  SubSection \"Display\"\n    Depth 24\n    Virtual {width} {height}\n    Modes \"screen\"\n  EndSubSection\nEndSection\n\
             Section \"InputDevice\"\n  Identifier \"touchscreen\"\n  Driver \"inputtest\"\n  Option \"SocketPath\" \"{}\"\n  Option \"DeviceType\" \"Touch\"\n  Option \"TouchCount\" \"5\"\nEndSection\n\
             Section \"ServerLayout\"\n  Identifier \"layout\"\n  Screen \"screen\"\n  InputDevice \"touchscreen\"\nEndSection\n",
            width + 8,
            width + 16,
            width + 24,
            height + 1,
            height + 2,
            height + 3,
            touch.display(),
            // 60 Hz over the mode's whole raster, in MHz.
            clock = f64::from((width + 24) * (height + 3)) * 60. / 1e6,
          ),
        )
      })
      .map_err(|error| format!("could not write Xorg's configuration: {error}"))?;

    let log = self.scratch.join("xorg.log");
    let number = self.start_x_server("Xorg", |display| {
      let mut command = Command::new("Xorg");

      command
        .arg(display)
        .arg("-config")
        .arg(&config)
        .arg("-configdir")
        .arg(&directory)
        .arg("-logfile")
        .arg(&log)
        .args(["-nolisten", "tcp", "-noreset", "-novtswitch", "-sharevts"]);

      command
    })?;

    wait_for("the inputtest touchscreen's socket", || touch.exists())?;
    self.display = Some(("DISPLAY", format!(":{number}")));
    self
      .extra
      .push(("SKENE_TOUCH_SOCKET", touch.to_string_lossy().into_owned()));

    Ok(())
  }

  /// Xvfb on a free display number ([`Self::start_x_server`]).
  fn start_xvfb(&mut self) -> Result<(), String> {
    let screen = screen_at(if self.test.is_some() {
      test_scale()?
    } else {
      1.
    });
    let size = format!("{}x{}x24", screen.0, screen.1);
    let number = self.start_x_server("Xvfb (install xvfb)", |display| {
      let mut command = Command::new("Xvfb");

      command.args([
        display,
        "-screen",
        "0",
        &size,
        "-nolisten",
        "tcp",
        "-noreset",
      ]);

      command
    })?;

    self.display = Some(("DISPLAY", format!(":{number}")));

    Ok(())
  }

  /// Starts the X server `command` makes for a display (`:<number>`) on the first number it
  /// takes, from one the runner's process ID picks among [`X_DISPLAYS`], the server writing the
  /// display's lock file: its number, once the lock file names it and its socket is there.
  fn start_x_server(
    &mut self,
    what: &str,
    mut command: impl FnMut(&str) -> Command,
  ) -> Result<u32, String> {
    let count = X_DISPLAYS.end - X_DISPLAYS.start;
    let first = std::process::id() % count;

    for offset in 0..count {
      let number = X_DISPLAYS.start + (first + offset) % count;
      let lock = PathBuf::from(format!("/tmp/.X{number}-lock"));
      let socket = PathBuf::from(format!("/tmp/.X11-unix/X{number}"));

      if lock.exists() || socket.exists() {
        continue;
      }

      let mut server = tied(&mut command(&format!(":{number}")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|error| format!("could not start {what}: {error}"))?;
      let started = Instant::now();

      loop {
        if let Ok(Some(_)) = server.try_wait() {
          break;
        }

        let locked = fs::read_to_string(&lock)
          .ok()
          .and_then(|pid| pid.trim().parse::<u32>().ok())
          == Some(server.id());

        if locked && socket.exists() {
          self.helpers.push(server);

          return Ok(number);
        }

        if started.elapsed() > START_TIMEOUT {
          end(&mut server);

          return Err(format!("{what} did not start within {START_TIMEOUT:?}"));
        }

        std::thread::sleep(Duration::from_millis(10));
      }
    }

    Err(format!("{what}: no display free among {X_DISPLAYS:?}"))
  }

  /// A private session bus.
  fn start_bus(&mut self) -> Result<(), String> {
    let mut command = Command::new("dbus-daemon");

    command
      .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
      .env("XDG_RUNTIME_DIR", self.runtime())
      .env("XDG_CONFIG_HOME", self.scratch.join("config"));

    for variable in USER_ONLY {
      command.env_remove(variable);
    }

    if let Some((name, value)) = &self.display {
      command.env(name, value);
    }

    let mut bus = spawn(&mut command)
      .map_err(|error| format!("could not start dbus-daemon (install dbus): {error}"))?;
    let address = first_line(bus.stdout.take(), "dbus-daemon");

    self.helpers.push(bus);
    self.bus = Some(address?.trim().to_owned());

    Ok(())
  }

  /// A helper command with the test session's environment.
  fn helper(&self, program: &str) -> Command {
    let mut command = Command::new(program);

    for variable in USER_ONLY {
      command.env_remove(variable);
    }

    command
      .env("XDG_RUNTIME_DIR", self.runtime())
      .env("XDG_CONFIG_HOME", self.scratch.join("config"))
      .env("GSETTINGS_BACKEND", "keyfile");

    if let Some(address) = &self.bus {
      command.env("DBUS_SESSION_BUS_ADDRESS", address);
    }

    if let Some(cursors) = &self.cursors {
      command
        .env("XCURSOR_THEME", BLANK_THEME)
        .env("XCURSOR_PATH", cursors);
    }

    command
  }

  /// PipeWire, which mutter's RemoteDesktop session and its screen casts stream through.
  fn start_pipewire(&mut self) -> Result<(), String> {
    let pipewire = spawn(&mut self.helper("pipewire"))
      .map_err(|error| format!("could not start pipewire (install pipewire): {error}"))?;

    self.helpers.push(pipewire);
    wait_for("PipeWire's socket", || {
      self.runtime().join("pipewire-0").exists()
    })
  }

  /// WirePlumber, PipeWire's session manager, which links a screen cast's stream to the test
  /// that reads it.
  fn start_wireplumber(&mut self) -> Result<(), String> {
    let wireplumber = spawn(&mut self.helper("wireplumber"))
      .map_err(|error| format!("could not start wireplumber (install wireplumber): {error}"))?;

    self.helpers.push(wireplumber);

    Ok(())
  }

  /// Headless mutter on a virtual monitor, its Wayland socket named for the session, its
  /// windows mapped and moved without animation (so a window is where it stays from its first
  /// frame, however the screen is watched); at a scale other than 1, the monitor scaled
  /// fractionally, as a desktop's display settings scale it (mutter's
  /// `scale-monitor-framebuffer` feature on, the scale in its `monitors.xml`).
  fn start_mutter(&mut self) -> Result<(), String> {
    let scale = test_scale()?;
    let screen = screen_at(scale);

    if scale != 1. {
      self
        .scale_mutter(scale, screen)
        .map_err(|error| format!("could not write mutter's configuration: {error}"))?;
    }

    let socket = format!("skene-{}", std::process::id());
    let mut command = self.helper("mutter");

    command.env("MUTTER_DEBUG_DISABLE_ANIMATIONS", "1").args([
      "--headless",
      "--wayland",
      "--no-x11",
      "--wayland-display",
      &socket,
      "--virtual-monitor",
      &format!("{}x{}", screen.0, screen.1),
    ]);

    let mutter = spawn(&mut command)
      .map_err(|error| format!("could not start mutter (install mutter): {error}"))?;

    self.helpers.push(mutter);
    wait_for("mutter's Wayland socket", || {
      self.runtime().join(&socket).exists()
    })?;

    let bus = self.bus.clone().unwrap_or_default();

    wait_for("mutter's RemoteDesktop service", || {
      Command::new("dbus-send")
        .args([
          &format!("--bus={bus}"),
          "--print-reply",
          "--dest=org.freedesktop.DBus",
          "/org/freedesktop/DBus",
          "org.freedesktop.DBus.NameHasOwner",
          "string:org.gnome.Mutter.RemoteDesktop",
        ])
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).contains("true"))
    })?;
    self.display = Some(("WAYLAND_DISPLAY", socket));

    Ok(())
  }

  /// Writes mutter's settings and monitor configuration for a virtual monitor of `screen`
  /// device pixels at `scale`: the experimental feature that scales a monitor fractionally,
  /// in the keyfile its settings are read from (`GSETTINGS_BACKEND=keyfile`), and the
  /// monitor's scale in `monitors.xml`.
  fn scale_mutter(&self, scale: f64, screen: (u32, u32)) -> std::io::Result<()> {
    let config = self.scratch.join("config");
    let settings = config.join("glib-2.0/settings");

    fs::create_dir_all(&settings)?;
    fs::write(
      settings.join("keyfile"),
      "[org/gnome/mutter]\nexperimental-features=['scale-monitor-framebuffer']\n",
    )?;
    fs::write(
      config.join("monitors.xml"),
      format!(
        "<monitors version=\"2\">\n  <configuration>\n    <logicalmonitor>\n      <x>0</x>\n      <y>0</y>\n      <scale>{scale}</scale>\n      <primary>yes</primary>\n      <monitor>\n        <monitorspec>\n          <connector>Meta-0</connector>\n          <vendor>MetaVendor</vendor>\n          <product>MetaVirtualMonitor</product>\n          <serial>0x00</serial>\n        </monitorspec>\n        <mode>\n          <width>{}</width>\n          <height>{}</height>\n          <rate>60.000</rate>\n        </mode>\n      </monitor>\n    </logicalmonitor>\n  </configuration>\n</monitors>\n",
        screen.0, screen.1
      ),
    )
  }

  /// The X server the sway leg nests sway in where `SKENE_SWAY_BACKEND=x11` asks for it: Xorg
  /// with an inputtest touchscreen ([`Self::start_xorg`]), whose touches reach sway through
  /// its X11 backend; `None` (headless sway) otherwise.
  fn start_sway_parent(&mut self) -> Result<Option<String>, String> {
    match env::var("SKENE_SWAY_BACKEND").as_deref() {
      Ok("x11") => {
        self.start_xorg()?;

        Ok(self.display.as_ref().map(|(_, display)| display.clone()))
      }
      Ok("headless") | Err(_) => Ok(None),
      Ok(other) => Err(format!(
        "SKENE_SWAY_BACKEND={other}: sway's backends are headless (the default) and x11"
      )),
    }
  }

  /// Sway on one output, drawn with pixman, every window floating (sized by its app, as on a
  /// stacking desktop), its seat's cursors the transparent theme (which sway draws for a
  /// window that names a cursor by its shape): on its X11 backend, its output a window
  /// filling the screen of the X server `parent` names, or headless where none is named.
  fn start_sway(&mut self, parent: Option<&str>) -> Result<(), String> {
    let scale = test_scale()?;
    let screen = screen_at(scale);
    let config = self.scratch.join("sway.conf");
    let (backend, output) = if parent.is_some() {
      ("x11", "X11-1")
    } else {
      ("headless", "HEADLESS-1")
    };

    fs::write(
      &config,
      format!(
        "output {output} resolution {}x{} position 0 0 scale {}\ndefault_border none\nfor_window [app_id=\".*\"] floating enable\nseat * xcursor_theme {BLANK_THEME} 24\n",
        screen.0,
        screen.1,
        scale
      ),
    )
    .map_err(|error| format!("{}: {error}", config.display()))?;

    let mut command = self.helper("sway");

    command
      .args(["--unsupported-gpu", "-c"])
      .arg(&config)
      .env("WLR_BACKENDS", backend)
      .env("WLR_LIBINPUT_NO_DEVICES", "1")
      .env("WLR_RENDERER", "pixman");

    if let Some(parent) = parent {
      command.env("DISPLAY", parent);
    }

    let sway = spawn(&mut command)
      .map_err(|error| format!("could not start sway (install sway): {error}"))?;

    self.helpers.push(sway);

    let runtime = self.runtime();
    let find = |prefix: &str| {
      fs::read_dir(&runtime).ok().and_then(|entries| {
        entries
          .filter_map(Result::ok)
          .map(|entry| entry.file_name().to_string_lossy().into_owned())
          .find(|name| name.starts_with(prefix) && !name.ends_with(".lock"))
      })
    };

    wait_for("sway's Wayland socket", || {
      find("wayland-").is_some() && find("sway-ipc.").is_some()
    })?;
    self.display = find("wayland-").map(|socket| ("WAYLAND_DISPLAY", socket));
    self.sway_socket = find("sway-ipc.").map(|socket| runtime.join(socket));

    Ok(())
  }

  /// Weston on its X11 backend, drawn with pixman, its output a window of the session's
  /// Xvfb as large as the screen; weston decorates nothing (no xdg-decoration), so a window
  /// draws its own.
  fn start_weston(&mut self) -> Result<(), String> {
    let screen = screen_at(test_scale()?);

    let socket = format!("skene-{}", std::process::id());
    let Some(("DISPLAY", x_display)) = self.display.clone() else {
      return Err("weston's leg starts Xvfb first".to_owned());
    };
    // Weston takes the X server's keymap when it starts: Compose on the right Control key, as
    // the other legs' keymaps have it.
    let _ = Command::new("setxkbmap")
      .args([
        "-display",
        &x_display,
        "-layout",
        "us",
        "-option",
        "compose:rctrl",
      ])
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .status();

    let mut command = self.helper("weston");

    command
      .args([
        "--backend=x11",
        "--renderer=pixman",
        "--idle-time=0",
        &format!("--socket={socket}"),
        &format!("--width={}", screen.0),
        &format!("--height={}", screen.1),
      ])
      .env("DISPLAY", &x_display);

    let weston = spawn(&mut command)
      .map_err(|error| format!("could not start weston (install weston): {error}"))?;

    self.helpers.push(weston);
    wait_for("weston's Wayland socket", || {
      self.runtime().join(&socket).exists()
    })?;
    self.display = Some(("WAYLAND_DISPLAY", socket));
    self.extra.push(("SKENE_INPUT_DISPLAY", x_display.clone()));
    self.extra.push(("SKENE_OUTPUT_DISPLAY", x_display));

    Ok(())
  }

  /// KWin's windowed backend on one output, a window of the session's Xvfb as large as the
  /// screen, composited with QPainter, its permission checks on; its fake input (a restricted
  /// interface) granted to the helper the test acts through alone, with Xwayland where
  /// `xwayland`: the test's windows go there, Xwayland showing the screen's device pixels where
  /// the leg is scaled.
  fn start_kwin(&mut self, xwayland: bool) -> Result<(), String> {
    let scale = test_scale()?;
    let screen = screen_at(scale);
    let socket = format!("skene-{}", std::process::id());
    let Some(("DISPLAY", x_display)) = self.display.clone() else {
      return Err("KWin's leg starts Xvfb first".to_owned());
    };
    let program = env::var("SKENE_KWIN").unwrap_or_else(|_| "kwin_wayland".to_owned());
    let config = self.scratch.join("config");

    // KWin places a window where it overlaps the fewest (its Smart policy): a test's second
    // application opens beside its window, not centered over it. Scaled, Xwayland shows the
    // screen's device pixels, X11 apps scaling themselves as a desktop leaves them to.
    let xwayland_scale = if xwayland && scale != 1. {
      format!("[Xwayland]\nScale={scale}\n")
    } else {
      String::new()
    };

    fs::create_dir_all(&config)
      .and_then(|()| {
        fs::write(
          config.join("kwinrc"),
          format!("[Windows]\nPlacement=Smart\n{xwayland_scale}"),
        )
      })
      .map_err(|error| format!("could not write KWin's configuration: {error}"))?;

    // A desktop file in the session's own data home grants KWin's restricted fake input to the
    // helper alone.
    let fake_input = kwin_fake_input().ok_or(
      "the KWin legs act through kwin-fake-input beside skene-runner (or SKENE_KWIN_FAKE_INPUT): build skene-shell-tools",
    )?;
    let data = self.scratch.join("data");

    fs::create_dir_all(data.join("applications"))
      .and_then(|()| {
        fs::write(
          data.join("applications/skene-kwin-fake-input.desktop"),
          format!(
            "[Desktop Entry]\nType=Application\nName=Skene's fake input\nNoDisplay=true\nExec={}\nX-KDE-Wayland-Interfaces=org_kde_kwin_fake_input\n",
            fake_input.display()
          ),
        )
      })
      .map_err(|error| format!("could not write the fake input's desktop file: {error}"))?;

    let mut command = self.helper(&program);

    // In windowed mode the output's size is logical: its window is that times the scale.
    command
      .args([
        "--x11-display",
        &x_display,
        "--socket",
        &socket,
        "--width",
        &(f64::from(screen.0) / scale).round().to_string(),
        "--height",
        &(f64::from(screen.1) / scale).round().to_string(),
        "--scale",
        &scale.to_string(),
      ])
      .env("DISPLAY", &x_display)
      .env("XDG_DATA_HOME", &data)
      .env("XDG_CACHE_HOME", self.scratch.join("cache"))
      .env("KWIN_COMPOSE", "Q")
      .env("QT_QPA_PLATFORM", "offscreen");

    if xwayland {
      command.arg("--xwayland");
    }

    if let Some(input_method) = kwin_input_method() {
      command.arg("--inputmethod").arg(input_method);
    }

    let kwin = spawn(&mut command).map_err(|error| {
      format!("could not start {program} (install kwin-wayland; SKENE_KWIN names another): {error}")
    })?;

    self.helpers.push(kwin);
    wait_for("KWin's Wayland socket", || {
      self.runtime().join(&socket).exists()
    })?;

    if xwayland {
      let mut display = None;

      let output = x_display.trim_start_matches(':').parse::<u32>().ok();

      // KWin takes a free display number for its Xwayland, its lock file naming a process of
      // this session's (other than the Xvfb its output shows in).
      wait_for("KWin's Xwayland", || {
        display = (0..1000).find(|number| {
          Some(*number) != output
            && Path::new(&format!("/tmp/.X11-unix/X{number}")).exists()
            && fs::read_to_string(format!("/tmp/.X{number}-lock"))
              .ok()
              .and_then(|pid| pid.trim().parse::<u32>().ok())
              .is_some_and(|pid| is_descendant(pid, std::process::id()))
        });

        display.is_some()
      })?;

      let number = display.unwrap_or_default();

      self.extra.push(("WAYLAND_DISPLAY", socket));
      self.display = Some(("DISPLAY", format!(":{number}")));
    } else {
      self.display = Some(("WAYLAND_DISPLAY", socket));
    }

    self.extra.push(("SKENE_OUTPUT_DISPLAY", x_display));
    self.extra.push((
      "SKENE_KWIN_FAKE_INPUT",
      fake_input.to_string_lossy().into_owned(),
    ));

    Ok(())
  }

  /// Where a test of the session says it had nothing to test on its leg.
  fn not_tested(&self) -> PathBuf {
    self.scratch.join("not-tested")
  }

  /// Lists the tests that said they had nothing to test on `leg`, after the program's own
  /// results: each passed without testing anything here.
  fn report_not_tested(&self, leg: Leg) {
    let Ok(said) = fs::read_to_string(self.not_tested()) else {
      return;
    };
    let lines: Vec<&str> = said.lines().filter(|line| !line.is_empty()).collect();

    eprintln!(
      "skene-runner: tested nothing on the {1} leg, though counted as passed: {0}",
      lines.len(),
      leg.name()
    );

    for line in lines {
      eprintln!("  {line}");
    }
  }

  /// Runs `command` in the session and waits for it, forwarding an ending signal to it; its
  /// exit code, or 128 plus the signal that ended it.
  fn run(&self, command: &mut Command) -> Result<ExitCode, String> {
    if let Some(leg) = self.test {
      for variable in USER_ONLY {
        command.env_remove(variable);
      }

      command
        .env("SKENE_TEST_SESSION", leg.name())
        .env(
          "SKENE_WINDOW_SYSTEM",
          if leg.is_x11() { "x11" } else { "wayland" },
        )
        .env("SKENE_SCREEN", {
          let screen = screen_at(test_scale().unwrap_or(1.));

          format!("{}x{}", screen.0, screen.1)
        })
        .env("XDG_RUNTIME_DIR", self.runtime())
        .env("XDG_CONFIG_HOME", self.scratch.join("config"))
        .env("GSETTINGS_BACKEND", "memory")
        .env("EGL_LOG_LEVEL", "fatal")
        .env("SKENE_NOT_TESTED", self.not_tested());

      if !utf8_locale() {
        command.env("LC_CTYPE", "C.UTF-8");
      }

      if let Some(socket) = &self.sway_socket {
        command.env("SWAYSOCK", socket);
      }
    }

    if let Some((name, value)) = &self.display {
      command
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env(name, value)
        .env("SKENE_DISPLAY", value);
    }

    for (name, value) in &self.extra {
      command.env(name, value);
    }

    if let Some(address) = &self.bus {
      command.env("DBUS_SESSION_BUS_ADDRESS", address);
    }

    let mut child = tied(command)
      .spawn()
      .map_err(|error| format!("could not run the program: {error}"))?;

    PROGRAM.store(
      i32::try_from(child.id()).unwrap_or_default(),
      Ordering::SeqCst,
    );

    let received = RECEIVED.load(Ordering::SeqCst);

    if received != 0 {
      forward(received);
    }

    let status = child.wait();

    PROGRAM.store(0, Ordering::SeqCst);

    if let Some(leg) = self.test {
      self.report_not_tested(leg);
    }

    let status = status.map_err(|error| format!("could not wait for the program: {error}"))?;

    Ok(match status.code() {
      Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(101)),
      None => {
        use std::os::unix::process::ExitStatusExt as _;

        ExitCode::from(128 + u8::try_from(status.signal().unwrap_or(0)).unwrap_or(0))
      }
    })
  }

  /// Ends what the session started (the bus takes the accessibility bus it started), the last
  /// first, and removes the scratch directory.
  fn stop(mut self) {
    while let Some(mut helper) = self.helpers.pop() {
      end(&mut helper);
    }

    let _ = fs::remove_dir_all(&self.scratch);

    if let Some(outside) = &self.outside {
      let _ = fs::remove_file(outside);
    }
  }
}

/// Gives the session's keyboard a Compose key on the right Control key, as a user configures
/// xkbcommon: the US layout's basic symbols in the configuration home's `xkb/symbols/us`,
/// which xkbcommon reads before the system's, with `compose(rctrl)` included (mutter's
/// keymap is the US layout with no options).
fn compose_key(config: &Path) -> std::io::Result<()> {
  let system = fs::read_to_string("/usr/share/X11/xkb/symbols/us")?;
  let marker = "xkb_symbols \"basic\" {";
  let Some(at) = system.find(marker) else {
    return Ok(());
  };
  let patched = format!(
    "{}\n    include \"compose(rctrl)\"\n{}",
    &system[..at + marker.len()],
    &system[at + marker.len()..]
  );

  fs::create_dir_all(config.join("xkb/symbols"))?;
  fs::write(config.join("xkb/symbols/us"), patched)
}

/// The display numbers the runner's X servers take: below the session scripts' (400 to 699)
/// and above those KWin's Xwayland takes (from 0).
const X_DISPLAYS: std::ops::Range<u32> = 100..400;

/// The drivers the X11 leg's Xorg needs: its dummy video driver and its inputtest input driver.
const XORG_DRIVERS: [&str; 2] = [
  "/usr/lib/xorg/modules/drivers/dummy_drv.so",
  "/usr/lib/xorg/modules/input/inputtest_drv.so",
];

/// The cursor theme of a leg whose compositor draws the pointer into its output.
const BLANK_THEME: &str = "skene-blank";

/// The cursor names a theme answers to: CSS's, and the X11 core and legacy names toolkits ask
/// for.
const CURSOR_NAMES: [&str; 62] = [
  "default",
  "context-menu",
  "help",
  "pointer",
  "progress",
  "wait",
  "cell",
  "crosshair",
  "text",
  "vertical-text",
  "alias",
  "copy",
  "move",
  "no-drop",
  "not-allowed",
  "grab",
  "grabbing",
  "all-scroll",
  "col-resize",
  "row-resize",
  "n-resize",
  "e-resize",
  "s-resize",
  "w-resize",
  "ne-resize",
  "nw-resize",
  "se-resize",
  "sw-resize",
  "ew-resize",
  "ns-resize",
  "nesw-resize",
  "nwse-resize",
  "zoom-in",
  "zoom-out",
  "left_ptr",
  "arrow",
  "top_left_arrow",
  "hand1",
  "hand2",
  "pointing_hand",
  "xterm",
  "ibeam",
  "watch",
  "left_ptr_watch",
  "fleur",
  "dnd-copy",
  "dnd-move",
  "dnd-link",
  "dnd-none",
  "dnd-ask",
  "top_side",
  "bottom_side",
  "left_side",
  "right_side",
  "top_left_corner",
  "top_right_corner",
  "bottom_left_corner",
  "bottom_right_corner",
  "sb_h_double_arrow",
  "sb_v_double_arrow",
  "question_arrow",
  "X_cursor",
];

/// Writes [`BLANK_THEME`] under `icons`: an Xcursor file per name of [`CURSOR_NAMES`], each
/// one transparent image 8 pixels square.
fn blank_cursors(icons: &Path) -> std::io::Result<()> {
  /// An Xcursor file's image chunk type.
  const IMAGE: u32 = 0xfffd_0002;
  /// The image's nominal size, and its width and height.
  const SIZE: u32 = 8;

  let theme = icons.join(BLANK_THEME);
  let cursors = theme.join("cursors");
  let mut file = Vec::new();

  // The header (its magic, its size, the version, one table entry), the entry, then the
  // image's header (its size, type, nominal size, version, width, height, hot spot, delay).
  for value in [0x7275_6358, 16, 0x1_0000, 1, IMAGE, SIZE, 28] {
    file.extend_from_slice(&u32::to_le_bytes(value));
  }

  for value in [36, IMAGE, SIZE, 1, SIZE, SIZE, 0, 0, 0] {
    file.extend_from_slice(&u32::to_le_bytes(value));
  }

  file.resize(file.len() + (SIZE * SIZE * 4) as usize, 0);
  fs::create_dir_all(&cursors)?;
  fs::write(
    theme.join("index.theme"),
    format!("[Icon Theme]\nName={BLANK_THEME}\n"),
  )?;

  for name in CURSOR_NAMES {
    fs::write(cursors.join(name), &file)?;
  }

  Ok(())
}

/// Whether the locale in force reads UTF-8.
fn utf8_locale() -> bool {
  ["LC_ALL", "LC_CTYPE", "LANG"]
    .into_iter()
    .find_map(|name| env::var(name).ok().filter(|value| !value.is_empty()))
    .is_some_and(|locale| {
      let locale = locale.to_ascii_lowercase();

      locale.contains("utf-8") || locale.contains("utf8")
    })
}

/// Polls `ready` until it holds, within [`START_TIMEOUT`].
fn wait_for(what: &str, mut ready: impl FnMut() -> bool) -> Result<(), String> {
  let started = Instant::now();

  while !ready() {
    if started.elapsed() > START_TIMEOUT {
      return Err(format!("{what} did not come up within {START_TIMEOUT:?}"));
    }

    std::thread::sleep(Duration::from_millis(10));
  }

  Ok(())
}

/// Starts `command` as a helper of the session: its output piped, in a process group of its
/// own (so a terminal's signals reach the program alone), and signalled when the runner dies.
fn spawn(command: &mut Command) -> std::io::Result<Child> {
  tied(command)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .process_group(0)
    .spawn()
}

/// `command`, its process sent `SIGTERM` when the runner dies.
fn tied(command: &mut Command) -> &mut Command {
  // SAFETY: the hook runs between fork and exec and calls `prctl` alone, which is
  // async-signal-safe.
  unsafe {
    command.pre_exec(|| {
      prctl(PARENT_DEATH_SIGNAL, TERMINATE as c_ulong);

      Ok(())
    })
  }
}

/// Terminates `child` and every process of its group (what a bus started on demand), then
/// reaps it.
fn end(child: &mut Child) {
  if let Ok(group) = i32::try_from(child.id()) {
    // SAFETY: `kill` reads its two integers alone; the group is the child's own.
    unsafe {
      kill(-group, TERMINATE);
    }
  }

  let started = Instant::now();

  while started.elapsed() < Duration::from_secs(2) {
    if let Ok(Some(_)) = child.try_wait() {
      return;
    }

    std::thread::sleep(Duration::from_millis(5));
  }

  let _ = child.kill();
  let _ = child.wait();
}

/// The first line `what` writes to the pipe `stdout`, within [`START_TIMEOUT`]; the rest is
/// drained, so `what` never blocks on the pipe.
fn first_line(stdout: Option<std::process::ChildStdout>, what: &str) -> Result<String, String> {
  let stdout = stdout.ok_or_else(|| format!("{what} has no output pipe"))?;
  let (sender, receiver) = std::sync::mpsc::channel();

  std::thread::spawn(move || {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let read = reader.read_line(&mut line).map(|_| line);

    let _ = sender.send(read);

    let mut sink = Vec::new();
    let _ = reader.read_to_end(&mut sink);
  });

  match receiver.recv_timeout(START_TIMEOUT) {
    Ok(Ok(line)) if !line.trim().is_empty() => Ok(line),
    Ok(Ok(_)) => Err(format!("{what} exited before it started")),
    Ok(Err(error)) => Err(format!("{what}: {error}")),
    Err(_) => Err(format!("{what} did not start within {START_TIMEOUT:?}")),
  }
}

/// The input method KWin starts on the KWin legs, a stand-in a test reads what KWin relays of a
/// window's text-input-v3 through: `SKENE_KWIN_INPUT_METHOD`, or `kwin-input-method` beside the
/// runner where it is there.
fn kwin_input_method() -> Option<PathBuf> {
  env::var_os("SKENE_KWIN_INPUT_METHOD")
    .map(PathBuf::from)
    .or_else(|| {
      env::current_exe()
        .ok()
        .and_then(|runner| Some(runner.parent()?.join("kwin-input-method")))
    })
    .filter(|path| path.is_file())
}

/// The helper KWin grants its fake input to on the KWin legs, which a test acts through:
/// `SKENE_KWIN_FAKE_INPUT`, or `kwin-fake-input` beside the runner, its canonical path (KWin
/// matches a desktop file's program to a client's by it).
fn kwin_fake_input() -> Option<PathBuf> {
  env::var_os("SKENE_KWIN_FAKE_INPUT")
    .map(PathBuf::from)
    .or_else(|| {
      env::current_exe()
        .ok()
        .and_then(|runner| Some(runner.parent()?.join("kwin-fake-input")))
    })
    .filter(|path| path.is_file())
    .and_then(|path| fs::canonicalize(path).ok())
}

/// Whether process `pid` descends from process `ancestor`, by `/proc`'s parents.
fn is_descendant(pid: u32, ancestor: u32) -> bool {
  let mut current = pid;

  for _ in 0..64 {
    if current == ancestor {
      return true;
    }

    let Some(parent) = fs::read_to_string(format!("/proc/{current}/stat"))
      .ok()
      .and_then(|stat| {
        let after = &stat[stat.rfind(')')? + 2..];

        after.split(' ').nth(1)?.parse::<u32>().ok()
      })
    else {
      return false;
    };

    if parent <= 1 {
      return false;
    }

    current = parent;
  }

  false
}
