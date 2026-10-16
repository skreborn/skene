//! `skene-runner`: the target runner `skene new` names, which cargo runs every test binary,
//! doctest and app of a Linux build through.
//!
//! ```text
//! skene-runner <program> [arguments...]
//! ```
//!
//! - **A test** (a binary in a `deps` directory, an example cargo built as a test, or a
//!   doctest) always gets a session of its own, the leg `SKENE_TEST_SESSION` names:
//!   - `x11` (the default): a private Xvfb;
//!   - `wayland` or `mutter`: headless mutter (a virtual monitor, its RemoteDesktop service
//!     over a private PipeWire, which the test's input goes through);
//!   - `sway`: headless sway (pixman), its virtual pointer and keyboard;
//!
//!   each with a private D-Bus session (which starts the accessibility bus on demand), a
//!   private runtime directory and configuration home, and none of the user's display,
//!   accessibility bus, input method or scale settings. The test is told its leg
//!   (`SKENE_TEST_SESSION`) and its display (`SKENE_DISPLAY`). It never touches the user's
//!   display, so parallel test processes (cargo-nextest's) share no pointer, keyboard focus or
//!   window stack, and a test reads what a screen reader reads of its own app alone. Listing
//!   the tests (`--list`) needs none of it and runs with no display at all.
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
const USER_ONLY: [&str; 13] = [
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
  "SKENE_BACKEND",
];

/// The virtual screen a test's session shows.
const SCREEN: (u32, u32) = (1280, 1024);

/// Which private session a test gets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Leg {
  /// Xvfb.
  X11,
  /// Headless mutter, with PipeWire for its RemoteDesktop service.
  Mutter,
  /// Headless sway.
  Sway,
}

impl Leg {
  /// The leg `SKENE_TEST_SESSION` names, X11 where it names none.
  fn chosen() -> Result<Self, String> {
    match env::var("SKENE_TEST_SESSION").as_deref() {
      Err(_) | Ok("" | "x11") => Ok(Self::X11),
      Ok("wayland" | "mutter") => Ok(Self::Mutter),
      Ok("sway") => Ok(Self::Sway),
      Ok(other) => Err(format!(
        "SKENE_TEST_SESSION={other}: the legs are x11 (the default), wayland (headless mutter) and sway"
      )),
    }
  }

  /// The name the test is told.
  const fn name(self) -> &'static str {
    match self {
      Self::X11 => "x11",
      Self::Mutter => "mutter",
      Self::Sway => "sway",
    }
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
    match leg {
      Leg::X11 => {
        self.start_xvfb()?;
        self.start_bus()
      }
      Leg::Mutter => {
        self.start_bus()?;
        self.start_pipewire()?;
        self.start_mutter()
      }
      Leg::Sway => {
        self.start_bus()?;
        self.start_sway()
      }
    }
  }

  /// Xvfb on the first free display number it finds itself.
  fn start_xvfb(&mut self) -> Result<(), String> {
    let mut display = spawn(Command::new("Xvfb").args([
      "-displayfd",
      "1",
      "-screen",
      "0",
      &format!("{}x{}x24", SCREEN.0, SCREEN.1),
      "-nolisten",
      "tcp",
      "-noreset",
    ]))
    .map_err(|error| format!("could not start Xvfb (install xvfb): {error}"))?;
    let number = first_line(display.stdout.take(), "Xvfb");

    self.helpers.push(display);

    let number = number.and_then(|line| {
      line
        .trim()
        .parse::<u32>()
        .map_err(|_| "Xvfb did not report its display number".to_owned())
    })?;

    self.display = Some(("DISPLAY", format!(":{number}")));

    Ok(())
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

    command
  }

  /// PipeWire, which mutter's RemoteDesktop session streams through.
  fn start_pipewire(&mut self) -> Result<(), String> {
    let pipewire = spawn(&mut self.helper("pipewire"))
      .map_err(|error| format!("could not start pipewire (install pipewire): {error}"))?;

    self.helpers.push(pipewire);
    wait_for("PipeWire's socket", || {
      self.runtime().join("pipewire-0").exists()
    })
  }

  /// Headless mutter on a virtual monitor, its Wayland socket named for the session.
  fn start_mutter(&mut self) -> Result<(), String> {
    let socket = format!("skene-{}", std::process::id());
    let mut command = self.helper("mutter");

    command.args([
      "--headless",
      "--wayland",
      "--no-x11",
      "--wayland-display",
      &socket,
      "--virtual-monitor",
      &format!("{}x{}", SCREEN.0, SCREEN.1),
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

  /// Headless sway on one output, drawn with pixman, every window floating (sized by its
  /// app, as on a stacking desktop).
  fn start_sway(&mut self) -> Result<(), String> {
    let config = self.scratch.join("sway.conf");

    fs::write(
      &config,
      format!(
        "output HEADLESS-1 resolution {}x{} position 0 0 scale 1\ndefault_border none\nfor_window [app_id=\".*\"] floating enable\n",
        SCREEN.0, SCREEN.1
      ),
    )
    .map_err(|error| format!("{}: {error}", config.display()))?;

    let mut command = self.helper("sway");

    command
      .args(["--unsupported-gpu", "-c"])
      .arg(&config)
      .env("WLR_BACKENDS", "headless")
      .env("WLR_LIBINPUT_NO_DEVICES", "1")
      .env("WLR_RENDERER", "pixman");

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
          "SKENE_BACKEND",
          if leg == Leg::X11 { "x11" } else { "wayland" },
        )
        .env("SKENE_SCREEN", format!("{}x{}", SCREEN.0, SCREEN.1))
        .env("XDG_RUNTIME_DIR", self.runtime())
        .env("XDG_CONFIG_HOME", self.scratch.join("config"))
        .env("GSETTINGS_BACKEND", "memory")
        .env("EGL_LOG_LEVEL", "fatal");

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
