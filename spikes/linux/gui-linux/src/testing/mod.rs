//! `skene_gui::testing` on Linux: the [`Driver`] a test reads and acts through, over the
//! window Skene draws.
//!
//! - **Reads** go through the accessibility layer, as a screen reader's do: AT-SPI over D-Bus
//!   ([`atspi`]), from the test's own process; the app answers from AccessKit's thread.
//! - **Actions** are real input ([`input`]): XTest on X11, mutter's EIS (through reis) and
//!   sway's virtual pointer and keyboard on Wayland, aimed at the bounds a reader reads. A
//!   Wayland client is not told where its window is, so the driver finds out as a user's
//!   pointer would: it moves the pointer onto the window and reads where the window saw it,
//!   again after every time the window was given a size.
//! - **Settling** waits for the input sent to arrive, then turns the window's loop until a
//!   quiet window passes in which no frame is painted, nothing is reported and no input
//!   arrives; a find looks again for up to a second.
//! - **The reader's settings** come from a stand-in of the settings portal ([`portal`]) on the
//!   test's private bus, which the app follows through its real path; a test that needs an
//!   input method serves a stand-in of Fcitx5 there ([`input_method`]), which a window on
//!   X11 talks to as to Fcitx5.
//!
//! What the window holds is used only to aim input (where its content starts, where the
//! pointer was seen) and to wait for it (whether input arrived, whether a frame is due),
//! never to assert what a reader finds. Tests run in the private session the display runner
//! (`skene-runner`) gives each test process, named by `SKENE_TEST_SESSION`: `x11` (Xvfb),
//! `mutter` (headless mutter with PipeWire), `sway` (headless sway), `weston` (weston on its
//! X11 backend inside a private Xvfb), `kwin` (KWin's virtual backend) or `kwin-x11` (the
//! window on KWin's Xwayland).

pub mod atspi;
pub mod input;
pub mod input_method;
pub mod portal;

use std::{
  cell::{Cell, OnceCell, RefCell},
  path::Path,
  rc::Rc,
  sync::{OnceLock, mpsc},
  time::{Duration, Instant},
};

use skene_foundation::{
  NodeId, Owner, Scheme,
  layout::Rect,
  seam::{
    access::{Checked, Role},
    activity,
    content::{Graphemes, TextSelection, Unit},
    report::Modifiers,
    testing::{
      AccessSnapshot, Action, Driver, EnvironmentRequest, Frame, Issue, Query, Screenshot,
    },
    tree,
  },
};
use xkbcommon::xkb;

use self::{
  atspi::{Atspi, Item, Object, state},
  input::Input,
  portal::{Portal, Setting},
};
use crate::{
  Backend, Linux, environment,
  paint::{PaintPath, PathChoice},
  shell, trace,
  window::Window,
};

/// How long `settle` must see no frame, no report and no input: above two 60 Hz frames.
const QUIET: Duration = Duration::from_millis(40);

/// How long a find looks again before it gives up.
pub const FIND_TIMEOUT: Duration = Duration::from_secs(1);

/// How long the platform may stay busy before `settle` fails.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(5);

/// How long input sent may take to arrive.
const INPUT_TIMEOUT: Duration = Duration::from_secs(2);

/// The size a test's window opens at.
const SIZE: (u32, u32) = (480, 640);

/// Which private session the display runner gave the test.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Session {
  /// Xvfb.
  X11,
  /// Headless mutter, with PipeWire for its RemoteDesktop session.
  Mutter,
  /// Headless sway.
  Sway,
  /// Weston on its X11 backend, its output a window of a private Xvfb.
  Weston,
  /// KWin's virtual backend.
  Kwin,
  /// The window on KWin's Xwayland, KWin's fake input acting.
  KwinX11,
}

impl Session {
  /// The session `SKENE_TEST_SESSION` names, where the runner named one.
  #[must_use]
  pub fn current() -> Option<Self> {
    match std::env::var("SKENE_TEST_SESSION").ok()?.as_str() {
      "x11" => Some(Self::X11),
      "mutter" => Some(Self::Mutter),
      "sway" => Some(Self::Sway),
      "weston" => Some(Self::Weston),
      "kwin" => Some(Self::Kwin),
      "kwin-x11" => Some(Self::KwinX11),
      _ => None,
    }
  }

  /// The display server the session's windows go to.
  #[must_use]
  pub const fn backend(self) -> Backend {
    match self {
      Self::X11 | Self::KwinX11 => Backend::X11,
      Self::Mutter | Self::Sway | Self::Weston | Self::Kwin => Backend::Wayland,
    }
  }

  /// The screen's size, as the runner gave it.
  fn screen() -> (u32, u32) {
    std::env::var("SKENE_SCREEN")
      .ok()
      .and_then(|screen| {
        let (width, height) = screen.split_once('x')?;

        Some((width.parse().ok()?, height.parse().ok()?))
      })
      .unwrap_or((1280, 1024))
  }

  /// The leg's scale, as the runner gave it (`SKENE_TEST_SCALE`): a Wayland leg's pointer
  /// moves in the compositor's logical pixels, the screen's device pixels over it.
  fn scale() -> f64 {
    std::env::var("SKENE_TEST_SCALE")
      .ok()
      .and_then(|scale| scale.parse::<f64>().ok())
      .filter(|scale| *scale >= 1.)
      .unwrap_or(1.)
  }

  fn input(self) -> input::Result<Box<dyn Input>> {
    Ok(match self {
      Self::X11 => Box::new(input::X11::new()?),
      Self::Mutter => Box::new(input::Ei::new()?),
      Self::Sway => Box::new(input::Sway::new(Self::screen(), Self::scale())?),
      Self::Weston => {
        let display = std::env::var("SKENE_INPUT_DISPLAY").map_err(
          |_| "the weston leg names the X display weston shows in (SKENE_INPUT_DISPLAY)",
        )?;

        Box::new(input::X11::nested(&display)?)
      }
      Self::Kwin | Self::KwinX11 => {
        let (width, height) = Self::screen();
        let scale = Self::scale();

        Box::new(input::Kwin::new((
          f64::from(width) / scale,
          f64::from(height) / scale,
        ))?)
      }
    })
  }
}

/// The window a test shows, and where its geometry sits in the pointer's space.
struct Shown {
  /// The window.
  window: Window,
  /// The owner the tree was built under, disposed with the window.
  owner: Option<Owner>,
  /// The window's object on the accessibility bus.
  frame: Option<Object>,
  /// The geometry's origin in the pointer space, and the window's size it was found at.
  origin: Option<((f64, f64), (u32, u32))>,
}

thread_local! {
  static SHOWN: RefCell<Option<Shown>> = const { RefCell::new(None) };
  static INPUT: RefCell<Option<Box<dyn Input>>> = const { RefCell::new(None) };
  static ATSPI: OnceCell<Atspi> = const { OnceCell::new() };
  static PORTAL: OnceCell<Option<Portal>> = const { OnceCell::new() };
  static WINDOWS: Cell<u32> = const { Cell::new(0) };
  /// A composition the driver started, waiting for its last key.
  static COMPOSING: Cell<Option<char>> = const { Cell::new(None) };
}

/// Runs `then` with the process's input devices, opened on first use.
///
/// # Panics
///
/// Where the session's input injection cannot be reached.
fn with_input<T>(then: impl FnOnce(&mut dyn Input) -> T) -> T {
  INPUT.with(|slot| {
    let mut slot = slot.borrow_mut();
    let input = slot.get_or_insert_with(|| {
      let session = Session::current().unwrap_or(Session::X11);

      session
        .input()
        .unwrap_or_else(|error| panic!("input injection on {session:?}: {error}"))
    });

    then(input.as_mut())
  })
}

/// Runs `then` with the process's accessibility client, connected on first use.
///
/// # Panics
///
/// Where there is no accessibility bus: the display runner gives every test one.
fn with_atspi<T>(then: impl FnOnce(&Atspi) -> T) -> T {
  ATSPI.with(|cell| {
    if cell.get().is_none() {
      let mut last = String::new();
      let started = Instant::now();

      // The app registers on the bus from the accessibility layer's thread; the window's loop
      // turns meanwhile, so the layer's requests are answered.
      while started.elapsed() < Duration::from_secs(10) {
        match Atspi::connect(Duration::ZERO) {
          Ok(atspi) => {
            let _ = cell.set(atspi);

            break;
          }
          Err(error) => last = error,
        }

        pump(Duration::from_millis(20));
      }

      assert!(
        cell.get().is_some(),
        "AT-SPI: {last}; tests run through the display runner (`skene-runner`), which gives each a session and an accessibility bus"
      );
    }

    then(cell.get().expect("connected"))
  })
}

/// Runs `then` with the stand-in settings portal, where it serves.
fn with_portal(then: impl FnOnce(&Portal)) {
  PORTAL.with(|cell| {
    if let Some(portal) = cell.get_or_init(|| Portal::serve().ok()) {
      then(portal);
    }
  });
}

/// The window's thread for tests: each test's body runs there, one at a time.
type Job = Box<dyn FnOnce() + Send>;

/// The process's window thread, started on first use: the stand-in portal served, the
/// accessibility bus turned on and the portal followed, once.
fn ui_thread() -> &'static mpsc::SyncSender<Job> {
  static UI: OnceLock<mpsc::SyncSender<Job>> = OnceLock::new();

  UI.get_or_init(|| {
    let (sender, receiver) = mpsc::sync_channel::<Job>(0);

    std::thread::Builder::new()
      .name("skene-ui".to_owned())
      .spawn(move || {
        with_portal(|_| {});

        if let Err(error) = atspi::enable() {
          eprintln!("skene testing: {error}");
        }

        environment::follow_portal();

        for job in receiver {
          job();
        }
      })
      .expect("the window thread starts");

    sender
  })
}

/// Runs a test's `body` on the process's window thread, under an owner of its own, the
/// environment and the reader's settings reset first; its window is closed and its owner
/// disposed when it returns or panics, and a panic fails the calling test with the body's
/// message.
///
/// # Panics
///
/// Where `body` panicked, and before it runs where the process is not in the display runner's
/// private session (`SKENE_TEST_SESSION`, and `SKENE_DISPLAY` naming the display in use): a
/// test never runs on the user's display.
pub fn run_test(name: &'static str, body: fn()) {
  let session = Session::current();
  let display = match session.map(Session::backend) {
    Some(Backend::X11) => std::env::var_os("DISPLAY"),
    Some(Backend::Wayland) => std::env::var_os("WAYLAND_DISPLAY"),
    None => None,
  };

  assert!(
    session.is_some() && display.is_some() && display == std::env::var_os("SKENE_DISPLAY"),
    "{name}: a test runs in the display runner's private session, never on the user's display; run it through `skene-runner` (the target runner `skene new` writes into .cargo/config.toml, or `SKENE_RUNNER=test`)"
  );

  let (sender, receiver) = mpsc::sync_channel(1);

  ui_thread()
    .send(Box::new(move || {
      reset();

      let owner = Owner::root();
      let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| owner.run(body)));

      close();
      owner.dispose();

      let outcome = outcome.map_err(|panic| {
        panic
          .downcast_ref::<String>()
          .cloned()
          .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
          .unwrap_or_else(|| "a panic without a message".to_owned())
      });

      let _ = sender.send(outcome);
    }))
    .expect("the window thread runs");

  if let Err(message) = receiver.recv().expect("the window thread answers") {
    panic!("{name}: {message}");
  }
}

/// Puts what a previous test may have changed back where the session started it: the
/// environment, the app's scheme, the reader's settings.
fn reset() {
  PathChoice::set_current(None);
  skene_foundation::Environment::current().reset();
  environment::set_scheme(None);
  with_portal(|portal| {
    for setting in Setting::DEFAULTS {
      let _ = portal.set(setting);
      let (namespace, key, value) = setting.heard();

      // What the portal's signal will say, heard now: the next window starts from it.
      environment::setting_changed(namespace, key, &value);
    }
  });
}

/// The frames being recorded: when recording began, the frames so far, and how many frames the
/// window had painted at the last look.
struct Recording {
  started: Instant,
  frames: Vec<Frame>,
  painted: u64,
}

thread_local! {
  static RECORDING: RefCell<Option<Recording>> = const { RefCell::new(None) };
}

/// Records the frame the open window painted since the last look, where recording, stamped
/// with when it was painted.
fn record() {
  let painted = SHOWN.with(|shown| {
    shown.borrow().as_ref().map(|shown| {
      let shell = shown.window.shell();

      (shell.frames, shell.painted_at)
    })
  });

  RECORDING.with(|recording| {
    if let (Some(recording), Some((painted, at))) = (recording.borrow_mut().as_mut(), painted)
      && painted > recording.painted
    {
      recording.painted = painted;
      recording.frames.push(Frame {
        at: at.map_or_else(
          || recording.started.elapsed(),
          |at| at.saturating_duration_since(recording.started),
        ),
        shot: Drawn::with_window(Drawn::shot),
      });
    }
  });
}

/// Turns the open window's loop for `duration`, or sleeps where none is open.
fn pump(duration: Duration) {
  let until = Instant::now() + duration;

  loop {
    let left = until.saturating_duration_since(Instant::now());
    let stepped = SHOWN.with(|shown| {
      shown.borrow_mut().as_mut().map(|shown| {
        shown
          .window
          .step(Some(left))
          .unwrap_or_else(|error| panic!("the window's connection failed: {error}"));
      })
    });

    if stepped.is_none() {
      std::thread::sleep(left);
    }

    record();

    if Instant::now() >= until {
      return;
    }
  }
}

/// Turns the loop until `done` holds or `timeout` passes; whether it held.
fn pump_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
  let started = Instant::now();

  loop {
    if done() {
      return true;
    }

    if started.elapsed() >= timeout {
      return false;
    }

    pump(Duration::from_millis(2));
  }
}

/// Whether the open window has a frame to paint.
fn wants_frame() -> bool {
  SHOWN.with(|shown| {
    shown
      .borrow()
      .as_ref()
      .is_some_and(|shown| shown.window.shell().wants_frame())
  })
}

/// Runs the platform until it is idle: the input sent delivered, then one quiet window in
/// which no frame was painted, nothing reported and no input arrived.
///
/// # Panics
///
/// Where frames or reports keep coming for five seconds (an endless animation).
pub fn settle() {
  let started = Instant::now();

  if INPUT.with(|input| input.borrow().is_some()) {
    let _ = with_input(|input| input.sync());
  }

  loop {
    pump(Duration::ZERO);

    let before = (activity::now(), shell::inputs());

    pump(QUIET);

    if (activity::now(), shell::inputs()) == before && !wants_frame() {
      return;
    }

    assert!(
      started.elapsed() < SETTLE_TIMEOUT,
      "the window did not settle within {SETTLE_TIMEOUT:?}: frames or reports kept coming ({before:?} then {:?})",
      activity::now()
    );
  }
}

/// Waits until the window has handled `count` more input events than `before`, as each key
/// and button sent reaches it; whether they all arrived.
fn arrived(before: u64, count: u64) -> bool {
  pump_until(INPUT_TIMEOUT, || shell::inputs() >= before + count)
}

/// Closes the window open now, and disposes what was built into it.
pub fn close() {
  let shown = SHOWN.with(|shown| shown.borrow_mut().take());

  if let Some(mut shown) = shown {
    if let Some(owner) = shown.owner.take() {
      owner.dispose();
    }

    drop(shown);
  }

  COMPOSING.with(|composing| composing.set(None));

  // The pointer goes to the screen's far corner, off where the next window opens, so it
  // opens with nothing over it.
  if INPUT.with(|input| input.borrow().is_some()) {
    let _ = with_input(|input| {
      let (width, height) = input.extent();

      input.move_to(width - 2., height - 2.)?;
      input.sync()
    });
  }
}

/// A node a reader found: its object on the accessibility bus, and what was read of it then.
#[derive(Clone, PartialEq, Debug)]
pub struct Element {
  /// The object on the accessibility bus.
  pub object: Object,
  /// Its role, mapped back to Skene's.
  pub role: Role,
  /// AT-SPI's name for its role.
  pub platform_role: String,
  /// Its accessible name when it was found.
  pub name: String,
}

/// Skene's role for AT-SPI's role name, as the drawn realization's accessibility layer
/// (AccessKit's AT-SPI mapping) names them.
#[must_use]
pub fn role_of(platform_role: &str, states: u64) -> Role {
  /// `ATSPI_STATE_SELECTABLE`.
  const SELECTABLE: u32 = 21;

  match platform_role {
    "push button" | "button" | "toggle button" => Role::Button,
    "switch" => Role::Switch,
    "check box" | "checkbox" => Role::Checkbox,
    "radio button" => Role::Radio,
    "slider" => Role::Slider,
    "spin button" => Role::SpinButton,
    "text" | "entry" | "password text" | "text box" => {
      if states & (1 << state::MULTI_LINE) != 0 {
        Role::TextEditor
      } else {
        Role::TextField
      }
    }
    "combo box" => Role::ComboBox,
    "list box" => Role::ListBox,
    "list item" if states & (1 << SELECTABLE) != 0 => Role::Option,
    "list item" => Role::Row,
    "option" => Role::Option,
    "label" | "static" => Role::Text,
    "heading" => Role::Heading(0),
    "image" => Role::Image,
    "link" => Role::Link,
    "progress bar" => Role::ProgressBar,
    "landmark" | "region" => Role::Region,
    "panel" | "group" | "grouping" => Role::Group,
    "radio menu item" | "menu item" => Role::MenuItem,
    "menu" => Role::Menu,
    "dialog" => Role::Dialog,
    "status bar" => Role::Status,
    "notification" | "alert" => Role::Alert,
    "tool tip" => Role::Tooltip,
    "scroll pane" => Role::ScrollArea,
    _ => Role::Generic,
  }
}

/// Whether two roles are one to a reader on Linux: a text box is one role there, a switch is
/// a check box (as GTK reports `GtkSwitch`), a row and an option are list items, and a
/// heading's level is not part of the role.
#[must_use]
pub fn same_role(found: Role, wanted: Role) -> bool {
  match (found, wanted) {
    (Role::TextField | Role::TextEditor, Role::TextField | Role::TextEditor)
    | (Role::Switch | Role::Checkbox, Role::Switch | Role::Checkbox)
    | (Role::Row | Role::Option, Role::Row | Role::Option)
    | (Role::Heading(_), Role::Heading(_)) => true,
    (found, wanted) => found == wanted,
  }
}

/// Whether `query` finds `item`.
fn matches(query: &Query, item: &Item) -> bool {
  let role = role_of(&item.role, item.states);

  match query {
    Query::Role(wanted, name) => {
      let role_matches = match wanted {
        Role::TextEditor => role == Role::TextEditor,
        wanted => same_role(role, *wanted),
      };

      role_matches && item.name == **name
    }
    Query::Text(text) => role == Role::Text && item.name == **text,
    Query::Focused => item.states & (1 << state::FOCUSED) != 0,
  }
}

/// The keysym a key's name presses: the X11 name (`Return`, `Escape`), its browser alias
/// (`Enter`, `Esc`), or a single character.
///
/// # Panics
///
/// Where no key is named `name`.
fn keysym_of(name: &str) -> u32 {
  let name = match name {
    "Enter" => "Return",
    "Esc" => "Escape",
    "Space" | " " => "space",
    "Backspace" => "BackSpace",
    "PageUp" => "Prior",
    "PageDown" => "Next",
    "ArrowLeft" => "Left",
    "ArrowRight" => "Right",
    "ArrowUp" => "Up",
    "ArrowDown" => "Down",
    "plus" => "plus",
    name => name,
  };
  let mut characters = name.chars();

  if let (Some(character), None) = (characters.next(), characters.next()) {
    return xkb::utf32_to_keysym(u32::from(character)).raw();
  }

  let sym = xkb::keysym_from_name(name, xkb::KEYSYM_NO_FLAGS).raw();

  assert!(sym != 0, "no key is named {name:?}");

  sym
}

/// The keysym typing `character`.
fn keysym_for(character: char) -> u32 {
  match character {
    '\n' => keysym_of("Return"),
    '\t' => keysym_of("Tab"),
    character => xkb::utf32_to_keysym(u32::from(character)).raw(),
  }
}

/// The modifier keysyms `modifiers` holds.
fn held(modifiers: Modifiers) -> Vec<u32> {
  [
    (modifiers.control, 0xffe3),
    (modifiers.shift, 0xffe1),
    (modifiers.alt, 0xffe9),
    (modifiers.meta, 0xffeb),
  ]
  .into_iter()
  .filter_map(|(held, sym)| held.then_some(sym))
  .collect()
}

/// The compose sequence typing `character` after `Multi_key`: its mark's key, and the letter
/// that ends it.
fn compose_sequence(character: char) -> Option<(char, char)> {
  let lower = character.to_lowercase().next()?;
  let (mark, base) = match lower {
    'á' => ('\'', 'a'),
    'à' => ('`', 'a'),
    'â' => ('^', 'a'),
    'ä' => ('"', 'a'),
    'ã' => ('~', 'a'),
    'é' => ('\'', 'e'),
    'è' => ('`', 'e'),
    'ê' => ('^', 'e'),
    'ë' => ('"', 'e'),
    'í' => ('\'', 'i'),
    'ì' => ('`', 'i'),
    'î' => ('^', 'i'),
    'ï' => ('"', 'i'),
    'ó' => ('\'', 'o'),
    'ò' => ('`', 'o'),
    'ô' => ('^', 'o'),
    'ö' => ('"', 'o'),
    'õ' => ('~', 'o'),
    'ú' => ('\'', 'u'),
    'ù' => ('`', 'u'),
    'û' => ('^', 'u'),
    'ü' => ('"', 'u'),
    'ñ' => ('~', 'n'),
    'ç' => (',', 'c'),
    _ => return None,
  };
  let base = if character.is_uppercase() {
    base.to_ascii_uppercase()
  } else {
    base
  };

  Some((mark, base))
}

/// The Linux driver: one window at a time, read through AT-SPI and acted on through the
/// session's real input.
#[derive(Default)]
pub struct Drawn;

impl Drawn {
  /// A driver; the window open on this thread is the one it reads.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  /// Shows the node `root` (built under `owner`) in a new window 480 by 640 pixels, the
  /// previous one closed, and settles once the window is drawn and a reader can see it.
  ///
  /// # Panics
  ///
  /// Where the window never draws or never reaches the accessibility bus.
  pub fn show(&mut self, owner: Owner, root: NodeId) {
    close();

    let session = Session::current().unwrap_or(Session::X11);
    let number = WINDOWS.with(|count| {
      count.set(count.get() + 1);
      count.get()
    });
    let title = format!("skene-test-{number}");
    let mut shell = shell::Shell::new(&title, SIZE.0, SIZE.1);

    if let Some(node) = tree::native::<Linux>(root) {
      shell.set_root(node);
    }

    let window = Window::open(session.backend(), shell, "dev.skene.test")
      .unwrap_or_else(|error| panic!("the window does not open on {session:?}: {error}"));

    SHOWN.with(|shown| {
      *shown.borrow_mut() = Some(Shown {
        window,
        owner: Some(owner),
        frame: None,
        origin: None,
      });
    });
    // A recording goes on into the new window from its first frame.
    RECORDING.with(|recording| {
      if let Some(recording) = recording.borrow_mut().as_mut() {
        recording.painted = 0;
      }
    });

    let drawn = pump_until(Duration::from_secs(10), || {
      SHOWN.with(|shown| {
        shown
          .borrow()
          .as_ref()
          .is_some_and(|shown| shown.window.shell().frames > 0)
      })
    });

    assert!(drawn, "the window never drew on {session:?}");

    settle();

    let started = Instant::now();

    loop {
      let frame = with_atspi(|atspi| atspi.toplevel(&title)).unwrap_or(None);

      if let Some(frame) = frame {
        SHOWN.with(|shown| {
          if let Some(shown) = shown.borrow_mut().as_mut() {
            shown.frame = Some(frame);
          }
        });

        break;
      }

      assert!(
        started.elapsed() < Duration::from_secs(10),
        "the window never reached the accessibility bus"
      );

      pump(Duration::from_millis(10));
    }

    settle();
  }

  /// Closes the window and disposes what was built into it.
  pub fn close(&mut self) {
    close();
  }

  /// The window's object on the accessibility bus.
  fn frame(&self) -> Option<Object> {
    SHOWN.with(|shown| {
      shown
        .borrow()
        .as_ref()
        .and_then(|shown| shown.frame.clone())
    })
  }

  /// What a reader finds in the window now, depth first.
  fn items(&self) -> Vec<Item> {
    let frame = self
      .frame()
      .unwrap_or_else(|| panic!("no window is open, or a reader cannot see it"));

    with_atspi(|atspi| atspi.tree(&frame)).unwrap_or_else(|error| panic!("AT-SPI: {error}"))
  }

  /// Runs `then` with the open window.
  ///
  /// # Panics
  ///
  /// Where no window is open.
  fn with_window<T>(then: impl FnOnce(&mut Window) -> T) -> T {
    SHOWN.with(|shown| {
      then(
        &mut shown
          .borrow_mut()
          .as_mut()
          .expect("a window is open")
          .window,
      )
    })
  }

  /// The content's origin in the window's geometry (below a title bar Skene draws), and the
  /// device pixels per pixel.
  fn content_origin() -> ((f32, f32), f64) {
    Self::with_window(|window| {
      let shell = window.shell();

      ((0., shell.chrome.header()), shell.scale)
    })
  }

  /// `found`'s border box in the window's content, in pixels, as a reader reads its extents.
  fn bounds(found: &Element) -> Rect {
    let (x, y, width, height) = with_atspi(|atspi| atspi.extents(&found.object, atspi::WINDOW))
      .unwrap_or_else(|error| panic!("AT-SPI extents of {:?}: {error}", found.name));
    let ((ox, oy), scale) = Self::content_origin();
    let scale = scale as f32;

    Rect {
      x: x as f32 / scale - ox,
      y: y as f32 / scale - oy,
      width: width as f32 / scale,
      height: height as f32 / scale,
    }
  }

  /// The window geometry's origin in the pointer's space: known on X11, found on Wayland by
  /// moving the pointer onto the window and reading where the window saw it, once per size
  /// the window was given; the pointer is moved off the window after.
  fn origin() -> (f64, f64) {
    let size = Self::with_window(|window| window.shell().size);
    let known = SHOWN.with(|shown| {
      shown
        .borrow()
        .as_ref()
        .and_then(|shown| shown.origin)
        .filter(|(_, at)| *at == size)
        .map(|(origin, _)| origin)
    });

    if let Some(origin) = known {
      return origin;
    }

    let origin = with_input(|input| input.window_origin()).unwrap_or_else(Self::calibrate);

    SHOWN.with(|shown| {
      if let Some(shown) = shown.borrow_mut().as_mut() {
        shown.origin = Some((origin, size));
      }
    });

    origin
  }

  /// Finds the window's origin by moving the pointer over points of the screen until the
  /// window sees it.
  ///
  /// # Panics
  ///
  /// Where the window sees the pointer at none of them.
  fn calibrate() -> (f64, f64) {
    let (width, height) = with_input(|input| input.extent());
    let mut points = vec![(width / 2., height / 2.)];

    for (fx, fy) in [(0.3, 0.3), (0.7, 0.3), (0.3, 0.7), (0.7, 0.7), (0.1, 0.1)] {
      points.push((width * fx, height * fy));
    }

    for point in points {
      let point = with_input(|input| input.landing(point.0, point.1));
      let before = trace::last_pointer();

      with_input(|input| {
        input.move_to(point.0, point.1)?;
        input.sync()
      })
      .unwrap_or_else(|error| panic!("pointer: {error}"));

      let seen = pump_until(Duration::from_millis(500), || {
        trace::last_pointer().0 != before.0
      });

      if seen && let (_, Some((x, y))) = trace::last_pointer() {
        let origin = (point.0 - f64::from(x), point.1 - f64::from(y));

        Self::park(origin, (width, height));

        return origin;
      }
    }

    panic!("the window never saw the pointer: it is not on the screen the input reaches");
  }

  /// Moves the pointer to a corner of the screen off the window, where one is.
  fn park(origin: (f64, f64), extent: (f64, f64)) {
    let (window_width, window_height) = Self::with_window(|window| {
      let (width, height) = window.shell().size;

      (f64::from(width), f64::from(height))
    });
    let off = |(x, y): (f64, f64)| {
      x < origin.0 - 2.
        || y < origin.1 - 2.
        || x > origin.0 + window_width + 2.
        || y > origin.1 + window_height + 2.
    };
    let corners = [
      (1., 1.),
      (extent.0 - 2., 1.),
      (1., extent.1 - 2.),
      (extent.0 - 2., extent.1 - 2.),
    ];

    if let Some(corner) = corners.into_iter().find(|corner| off(*corner)) {
      let _ = with_input(|input| {
        input.move_to(corner.0, corner.1)?;
        input.sync()
      });

      settle();
    }
  }

  /// The pointer-space point at fractions `(x, y)` of `found`'s bounds: on X11 its screen
  /// extents (the pointer's space is the screen's device pixels), on Wayland its window
  /// extents in pixels from the window's origin.
  fn point(found: &Element, x: f64, y: f64) -> (f64, f64) {
    let screen = with_input(|input| input.window_origin()).is_some();
    let kind = if screen { atspi::SCREEN } else { atspi::WINDOW };
    let (left, top, width, height) = with_atspi(|atspi| atspi.extents(&found.object, kind))
      .unwrap_or_else(|error| panic!("AT-SPI extents of {:?}: {error}", found.name));
    let (origin, unit) = if screen {
      ((0., 0.), 1.)
    } else {
      (
        Self::origin(),
        Self::with_window(|window| window.shell().scale),
      )
    };
    let at = |start: i32, length: i32, fraction: f64| {
      (f64::from(start) + f64::from(length) * fraction) / unit
    };

    (origin.0 + at(left, width, x), origin.1 + at(top, height, y))
  }

  /// How many of the pointer space's units a pixel of the window is.
  fn pointer_unit() -> f64 {
    if with_input(|input| input.window_origin()).is_some() {
      Self::with_window(|window| window.shell().scale)
    } else {
      1.
    }
  }

  /// Makes sure the window hears keys: on Wayland, where a compositor delivers a new virtual
  /// keyboard's keys only once it has handled the session's pointer (sway) or once it has
  /// set the device up (mutter), the window is found with the pointer first, as a click
  /// finds it.
  fn reach_keyboard() {
    if with_input(|input| input.window_origin()).is_none() {
      Self::origin();
    }
  }

  /// Sends what `send` sends, which is `count` key or button events, and waits until the
  /// window has handled them, then settles; whether they all arrived.
  fn send(count: u64, send: impl FnOnce(&mut dyn Input) -> input::Result<()>) -> bool {
    let before = shell::inputs();

    with_input(|input| send(input).and_then(|()| input.sync()))
      .unwrap_or_else(|error| panic!("input: {error}"));

    let all = arrived(before, count);

    settle();

    all
  }

  /// Sends `count` key events as [`send`](Self::send) does; where none of them reached the
  /// window (a compositor that has not yet set up the session's new keyboard), says so and
  /// sends them once more.
  fn send_keys(count: u64, send: impl Fn(&mut dyn Input) -> input::Result<()>) {
    let before = shell::inputs();

    if Self::send(count, &send) || shell::inputs() != before {
      return;
    }

    eprintln!("skene testing: {count} key events reached no window; sending them again");

    if !Self::send(count, &send) {
      eprintln!("skene testing: the key events sent again reached no window either");
    }
  }

  /// Clicks `button` at `point`; whether the window heard the press and the release.
  fn click_at(point: (f64, f64), button: u32) -> bool {
    Self::send(2, |input| {
      input.move_to(point.0, point.1)?;
      input.sync()?;
      input.button(button, true)?;
      input.button(button, false)
    })
  }

  /// Moves the pointer to `at` (fractions of `found`'s width and height) and returns the
  /// point. On Wayland, where the window sees the pointer elsewhere than aimed (the compositor
  /// moved the window since the driver found it, and says nothing of it), the driver finds the
  /// window again, says so, and aims once more.
  fn aim(found: &Element, at: (f64, f64)) -> (f64, f64) {
    let point = Self::point(found, at.0, at.1);

    if with_input(|input| input.window_origin()).is_some() {
      return point;
    }

    let origin = Self::origin();
    let before = trace::last_pointer();

    with_input(|input| {
      input.move_to(point.0, point.1)?;
      input.sync()
    })
    .unwrap_or_else(|error| panic!("pointer: {error}"));

    let _ = pump_until(Duration::from_millis(300), || {
      trace::last_pointer().0 != before.0
    });

    let wanted = (point.0 - origin.0, point.1 - origin.1);
    let seen = trace::last_pointer().1;
    let near = seen.is_some_and(|(x, y)| {
      (f64::from(x) - wanted.0).abs() <= 1.5 && (f64::from(y) - wanted.1).abs() <= 1.5
    });

    if near {
      return point;
    }

    SHOWN.with(|shown| {
      if let Some(shown) = shown.borrow_mut().as_mut() {
        shown.origin = None;
      }
    });
    eprintln!(
      "skene testing: the pointer aimed at {wanted:?} in the window over {:?} {:?} was seen at {seen:?}; aiming again",
      found.role, found.name
    );

    Self::point(found, at.0, at.1)
  }

  /// Clicks `button` at `at` (fractions of `found`'s width and height). Where the window hears
  /// no click there, the window was not where the driver aimed (a Wayland compositor says
  /// nothing of where it puts a window): the driver finds the window again, says so, and
  /// clicks once more.
  fn click(found: &Element, at: (f64, f64), button: u32) {
    let point = Self::aim(found, at);

    if Self::click_at(point, button) {
      return;
    }

    let origin = SHOWN.with(|shown| {
      shown
        .borrow_mut()
        .as_mut()
        .and_then(|shown| shown.origin.take())
    });

    eprintln!(
      "skene testing: a click at {point:?} (window origin {origin:?}) on {:?} {:?} reached no window; the pointer was last seen at {:?}; aiming again",
      found.role,
      found.name,
      trace::last_pointer().1
    );

    let point = Self::point(found, at.0, at.1);

    if !Self::click_at(point, button) {
      eprintln!("skene testing: the click at {point:?} reached no window either");
    }
  }

  /// Panics, saying so, where the session's input sends no touch.
  fn need_touch() {
    if !with_input(|input| input.has_touch()) {
      let name = with_input(|input| input.name());

      panic!(
        "{name} sends no touch: a touch test runs where the session can inject it (Drawn::sends_touch)"
      );
    }
  }

  /// Flings `found` on a touchpad: the pointer over its center, two fingers moved `right` and
  /// `down` pixels of scrolling in ten frames a hundredth of a second apart, then lifted; settles
  /// once the fling is at rest.
  fn fling(found: &Element, right: f64, down: f64) {
    if !with_input(|input| input.has_finger_scroll()) {
      let name = with_input(|input| input.name());

      panic!(
        "{name} sends no touchpad scrolling: a fling test runs where the session can inject it (Drawn::sends_touchpad_scrolling)"
      );
    }

    let point = Self::aim(found, (0.5, 0.5));
    let unit = Self::pointer_unit();

    Self::send(11, |input| {
      input.move_to(point.0, point.1)?;
      input.sync()?;

      for _ in 0..10 {
        input.finger_scroll(right * unit / 10., down * unit / 10.)?;
        input.sync()?;
        std::thread::sleep(Duration::from_millis(10));
      }

      input.finger_scroll_stop()
    });
  }

  /// Taps `found` at `at` (fractions of its width and height) on a touchscreen.
  fn tap(found: &Element, at: (f64, f64)) {
    Self::need_touch();

    let point = Self::aim(found, at);

    Self::send_touch(2, |input| {
      input.touch_down(0, point.0, point.1)?;
      input.sync()?;
      std::thread::sleep(Duration::from_millis(40));
      input.touch_up(0)
    });
  }

  /// Sends `count` touch events as [`send`](Self::send) does; where none of them reached the
  /// window (a compositor that makes its virtual touchscreen on the first touch, mutter),
  /// says so and sends them once more.
  fn send_touch(count: u64, send: impl Fn(&mut dyn Input) -> input::Result<()>) {
    let before = shell::inputs();

    if Self::send(count, &send) || shell::inputs() != before {
      return;
    }

    eprintln!("skene testing: {count} touch events reached no window; sending them again");

    if !Self::send(count, &send) {
      eprintln!("skene testing: the touch events sent again reached no window either");
    }
  }

  /// Holds a finger on `found`'s center for 800 ms, longer than a long press, then lifts it.
  fn long_press(found: &Element) {
    Self::need_touch();

    let point = Self::aim(found, (0.5, 0.5));

    Self::send_touch(1, |input| input.touch_down(0, point.0, point.1));
    pump(Duration::from_millis(800));
    Self::send(1, |input| input.touch_up(0));
  }

  /// Puts a finger on `found`'s center, moves it `right` and `down` pixels in ten steps and lifts
  /// it.
  fn swipe(found: &Element, right: f64, down: f64) {
    Self::need_touch();

    let from = Self::aim(found, (0.5, 0.5));
    let unit = Self::pointer_unit();
    let to = (from.0 + right * unit, from.1 + down * unit);

    Self::send_touch(2, |input| {
      input.touch_down(0, from.0, from.1)?;
      input.sync()?;
      std::thread::sleep(Duration::from_millis(20));

      for step in 1..=10 {
        let t = f64::from(step) / 10.;

        input.touch_motion(
          0,
          from.0 + (to.0 - from.0) * t,
          from.1 + (to.1 - from.1) * t,
        )?;
        input.sync()?;
        std::thread::sleep(Duration::from_millis(15));
      }

      input.touch_up(0)
    });
  }

  /// Whether the session's input sends touch (a touchscreen): mutter's EIS and KWin's fake
  /// input do; X11's XTest, sway's virtual devices and XTest into weston do not.
  #[must_use]
  pub fn sends_touch(&self) -> bool {
    with_input(|input| input.has_touch())
  }

  /// Whether the session's input sends a touchpad's scrolling with the end of its motion:
  /// mutter's EIS and sway's virtual pointer do; X11's XTest (a wheel's buttons alone), XTest
  /// into weston and KWin's fake input (an axis with no end) do not.
  #[must_use]
  pub fn sends_touchpad_scrolling(&self) -> bool {
    with_input(|input| input.has_finger_scroll())
  }

  /// The session's input, by name.
  #[must_use]
  pub fn input_name(&self) -> &'static str {
    with_input(|input| input.name())
  }

  /// The point at fractions `(x, y)` of `found`'s bounds in the pointer's space, the space
  /// [`drag`](Self::drag) and [`move_pointer`](Self::move_pointer) take: the screen's device
  /// pixels on X11, the compositor's global pixels on Wayland.
  #[must_use]
  pub fn point_in(&self, found: &Element, x: f64, y: f64) -> (f64, f64) {
    Self::point(found, x, y)
  }

  /// The point at fraction `across` of the width of `found`'s character at `offset` (in
  /// characters), halfway down its line, in the pointer's space: `0.` its leading edge, where
  /// a drop goes before it; `1.` its trailing edge, where a drop goes after it.
  ///
  /// # Panics
  ///
  /// Where `found` has no text, or no character at `offset`.
  #[must_use]
  pub fn character_point(&self, found: &Element, offset: usize, across: f64) -> (f64, f64) {
    let (x, y, width, height) = self.character_rect(found, offset);

    (x + width * across, y + height / 2.)
  }

  /// The rectangle of `found`'s character at `offset` (in characters) in the pointer's space,
  /// as a reader reads it: `x, y, width, height`.
  ///
  /// # Panics
  ///
  /// Where `found` has no text, or no character at `offset`.
  #[must_use]
  pub fn character_rect(&self, found: &Element, offset: usize) -> (f64, f64, f64, f64) {
    let screen = with_input(|input| input.window_origin()).is_some();
    let kind = if screen { atspi::SCREEN } else { atspi::WINDOW };
    let offset = i32::try_from(offset).unwrap_or(i32::MAX);
    let (left, top, width, height) =
      with_atspi(|atspi| atspi.character_extents(&found.object, offset, kind))
        .unwrap_or_else(|error| panic!("AT-SPI character {offset} of {:?}: {error}", found.name));

    assert!(
      width > 0 && height > 0,
      "{:?} has no character at {offset}",
      found.name
    );

    let (origin, unit) = if screen {
      ((0., 0.), 1.)
    } else {
      (
        Self::origin(),
        Self::with_window(|window| window.shell().scale),
      )
    };

    (
      origin.0 + f64::from(left) / unit,
      origin.1 + f64::from(top) / unit,
      f64::from(width) / unit,
      f64::from(height) / unit,
    )
  }

  /// The window's rectangle in the pointer's space: `(x, y, width, height)`.
  #[must_use]
  pub fn window_rect(&self) -> (f64, f64, f64, f64) {
    if with_input(|input| input.window_origin()).is_some() {
      let frame = self.frame().expect("a window is open");
      let (x, y, width, height) = with_atspi(|atspi| atspi.extents(&frame, atspi::SCREEN))
        .unwrap_or_else(|error| panic!("AT-SPI extents of the window: {error}"));

      return (
        f64::from(x),
        f64::from(y),
        f64::from(width),
        f64::from(height),
      );
    }

    let origin = Self::origin();
    let (width, height) = Self::with_window(|window| {
      let (width, height) = window.shell().size;

      (f64::from(width), f64::from(height))
    });

    (origin.0, origin.1, width, height)
  }

  /// The window was moved by something other than the driver (a compositor's command): where
  /// it is is found again before the pointer next aims at it.
  pub fn window_moved(&mut self) {
    SHOWN.with(|shown| {
      if let Some(shown) = shown.borrow_mut().as_mut() {
        shown.origin = None;
      }
    });
    settle();
  }

  /// The pointer space's size: the screen's.
  #[must_use]
  pub fn screen_extent(&self) -> (f64, f64) {
    with_input(|input| input.extent())
  }

  /// The scale the platform gave the window: device pixels per pixel.
  #[must_use]
  pub fn scale(&self) -> f64 {
    Self::with_window(|window| window.shell().scale)
  }

  /// Clicks the primary button at `at` in the pointer's space, then settles.
  pub fn click_point(&mut self, at: (f64, f64)) {
    Self::click_at(at, input::PRIMARY);
  }

  /// Clicks the primary button at `at` in the pointer's space with the left Shift held down
  /// around the click, then settles.
  pub fn shift_click_point(&mut self, at: (f64, f64)) {
    Self::send(4, |input| {
      input.move_to(at.0, at.1)?;
      input.sync()?;
      input.key(input::SHIFT, true)?;
      input.button(input::PRIMARY, true)?;
      input.button(input::PRIMARY, false)?;
      input.key(input::SHIFT, false)
    });
  }

  /// Clicks the primary button twice at `at` in the pointer's space, as fast as a hand
  /// double-clicks, then settles.
  pub fn double_click_point(&mut self, at: (f64, f64)) {
    Self::send(4, |input| {
      input.move_to(at.0, at.1)?;
      input.sync()?;

      for _ in 0..2 {
        input.button(input::PRIMARY, true)?;
        input.button(input::PRIMARY, false)?;
      }

      Ok(())
    });
  }

  /// Moves the pointer to `to` in the pointer's space, then turns the window's loop for a
  /// moment and settles.
  pub fn move_pointer(&mut self, to: (f64, f64)) {
    with_input(|input| {
      input.move_to(to.0, to.1)?;
      input.sync()
    })
    .unwrap_or_else(|error| panic!("pointer: {error}"));
    pump(Duration::from_millis(50));
    settle();
  }

  /// Drags with the primary button from `from` to `to` (points in the pointer's space) as a
  /// hand does: a press, the pointer moved there in twelve steps with the window's loop
  /// turning between them (so a drag the press starts begins while the button is held, as a
  /// compositor needs), a rest over `to`, the release; then settles. A drag that leaves the
  /// window goes on to whatever is under the pointer, another application's window included.
  pub fn drag(&mut self, from: (f64, f64), to: (f64, f64)) {
    Self::send(1, |input| {
      input.move_to(from.0, from.1)?;
      input.sync()?;
      input.button(input::PRIMARY, true)
    });

    for step in 1..=12 {
      let t = f64::from(step) / 12.;

      with_input(|input| {
        input.move_to(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)?;
        input.sync()
      })
      .unwrap_or_else(|error| panic!("pointer: {error}"));
      pump(Duration::from_millis(25));
    }

    pump(Duration::from_millis(150));
    with_input(|input| {
      input.button(input::PRIMARY, false)?;
      input.sync()
    })
    .unwrap_or_else(|error| panic!("pointer: {error}"));
    pump(Duration::from_millis(300));
    settle();
  }

  /// Presses the key `name` with `modifiers` held, focus where it is.
  fn press(name: &str, modifiers: Modifiers) {
    Self::reach_keyboard();

    let held = held(modifiers);
    let sym = keysym_of(name);
    let count = 2 + 2 * held.len() as u64;

    Self::send_keys(count, |input| input.chord(&held, sym));
  }

  /// Types `text`, a key per character, focus where it is.
  fn type_text(text: &str) {
    Self::reach_keyboard();

    for character in text.chars() {
      let sym = keysym_for(character);

      Self::send_keys(2, |input| input.tap(sym));
    }
  }

  /// Starts composing `character` as a user does on a keyboard with a Compose key:
  /// `Multi_key` and the mark, the sequence left open (the composition showing).
  ///
  /// # Panics
  ///
  /// Where `character` has no compose sequence the driver knows.
  fn compose(character: char) {
    let (mark, base) = compose_sequence(character)
      .unwrap_or_else(|| panic!("no compose sequence the driver knows types {character:?}"));

    COMPOSING.with(|composing| composing.set(Some(base)));
    Self::reach_keyboard();
    Self::send_keys(4, |input| {
      input.tap(0xff20)?;
      input.tap(keysym_for(mark))
    });
  }

  /// Ends the open composition with its last key.
  ///
  /// # Panics
  ///
  /// Where no composition is open.
  fn commit() {
    let base = COMPOSING
      .with(Cell::take)
      .expect("a composition is open (`Action::Compose`)");

    Self::type_text(&base.to_string());
  }

  /// Acts where focus is: a key pressed, text typed, a composition started or committed; then
  /// settles.
  ///
  /// # Panics
  ///
  /// Where the action goes to a point (a click, a hover, a scroll): those act on an element.
  pub fn act_at_focus(&mut self, action: &Action) {
    match action {
      Action::Press(name, modifiers) => Self::press(name, *modifiers),
      Action::Type(text) => Self::type_text(text),
      Action::Compose(character) => Self::compose(*character),
      Action::Commit => Self::commit(),
      Action::Click
      | Action::ClickAt(..)
      | Action::ContextClick
      | Action::Hover
      | Action::Drag(..)
      | Action::Scroll(..)
      | Action::Fling(..)
      | Action::Tap
      | Action::TapAt(..)
      | Action::LongPress
      | Action::Swipe(..)
      | Action::Access(_) => {
        panic!("{action:?} acts on an element, not at the focus")
      }
    }
  }

  /// Moves focus to `found` as a user does from the keyboard: `Tab` until a reader reads it
  /// focused.
  ///
  /// # Panics
  ///
  /// Where `Tab` never reaches it.
  pub fn focus(&mut self, found: &Element) {
    let focused = || {
      with_atspi(|atspi| atspi.states(&found.object))
        .is_ok_and(|states| states & (1 << state::FOCUSED) != 0)
    };

    if focused() {
      return;
    }

    let stops = self.items().len() + 2;

    for _ in 0..stops {
      Self::press("Tab", Modifiers::default());

      if focused() {
        return;
      }
    }

    panic!(
      "Tab never reached {:?} {:?}; a reader finds:\n{}",
      found.role,
      found.name,
      self.outline()
    );
  }

  /// What a reader finds for `query`, looking again for up to a second.
  pub fn find_soon(&mut self, query: &Query) -> Vec<Element> {
    self.find(query, FIND_TIMEOUT)
  }

  /// The window drawn now, its content alone (below a title bar Skene draws), as RGBA.
  ///
  /// # Panics
  ///
  /// Where no window is open or it has not drawn.
  #[must_use]
  pub fn content(&self) -> Screenshot {
    settle();

    Self::with_window(Self::shot)
  }

  /// `window`'s content as drawn last, alone (below a title bar Skene draws), as RGBA.
  fn shot(window: &mut Window) -> Screenshot {
    let shell = window.shell_mut();
    let scale = shell.scale;
    let (ox, oy) = shell.chrome.origin();
    let header = shell.chrome.header();
    let (width, height) = shell.content_size();
    let pixmap = shell.last_frame().expect("the window has drawn");

    Self::crop(
      pixmap,
      scale,
      (
        (f64::from(ox) * scale).round() as usize,
        (f64::from(oy + header) * scale).round() as usize,
      ),
      (
        (f64::from(width) * scale).round() as usize,
        (f64::from(height) * scale).round() as usize,
      ),
    )
  }

  /// `size` device pixels of `pixmap` (drawn at `scale`) from `top_left`, as straight RGBA.
  fn crop(
    pixmap: &vello_cpu::Pixmap,
    scale: f64,
    (left, top): (usize, usize),
    (width, height): (usize, usize),
  ) -> Screenshot {
    let stride = usize::from(pixmap.width());
    let data = pixmap.data();
    let mut rgba = Vec::with_capacity(width * height * 4);

    for row in top..top + height {
      for column in left..left + width {
        let [red, green, blue, alpha] = data
          .get(row * stride + column)
          .map_or([0; 4], |pixel| [pixel.r, pixel.g, pixel.b, pixel.a]);
        let unpremultiply = |channel: u8| {
          if alpha == 0 {
            0
          } else {
            ((u32::from(channel) * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255) as u8
          }
        };

        rgba.extend_from_slice(&[
          unpremultiply(red),
          unpremultiply(green),
          unpremultiply(blue),
          alpha,
        ]);
      }
    }

    Screenshot {
      width: width as u32,
      height: height as u32,
      scale,
      rgba,
    }
  }

  /// The open popup's surface as it drew it last, whole, as RGBA in device pixels, and its top
  /// left in the pointer's space; `None` while no popup is open.
  #[must_use]
  pub fn popup_content(&self) -> Option<(Screenshot, (f64, f64))> {
    settle();

    let unit = Self::pointer_unit();
    let (x, y, ..) = self.window_rect();

    Self::with_window(|window| {
      let shell = window.shell();
      let open = shell.popup()?;
      let rect = open.rect();
      let pixmap = open.pixmap();
      let shot = Self::crop(
        pixmap,
        shell.scale,
        (0, 0),
        (usize::from(pixmap.width()), usize::from(pixmap.height())),
      );

      Some((
        shot,
        (x + f64::from(rect.x) * unit, y + f64::from(rect.y) * unit),
      ))
    })
  }

  /// Writes the window's content as a PNG at `path`.
  ///
  /// # Errors
  ///
  /// Where the file cannot be written.
  pub fn save_png(&self, path: &Path) -> Result<(), String> {
    let shot = self.content();
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), shot.width, shot.height);

    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
      .write_header()
      .and_then(|mut writer| writer.write_image_data(&shot.rgba))
      .map_err(|error| error.to_string())
  }

  /// The window's content size, in pixels.
  #[must_use]
  pub fn size(&self) -> (f32, f32) {
    Self::with_window(|window| window.shell().content_size())
  }

  /// Changes a reader's setting on the stand-in portal and waits until the app has heard it
  /// through the portal's signal.
  fn change(setting: Setting) {
    let before = environment::settings_heard();
    let mut changed = false;

    with_portal(|portal| changed = portal.set(setting).is_ok_and(|changed| changed));

    if changed {
      let _ = pump_until(INPUT_TIMEOUT, || environment::settings_heard() > before);
    }
  }

  /// Changes a setting of the settings portal's stand-in, which the app hears through its
  /// real path, and settles.
  pub fn set_portal(&mut self, setting: Setting) {
    Self::change(setting);
    settle();
  }

  /// The app's scheme choice, through the platform's own path.
  pub fn set_scheme(&mut self, scheme: Option<Scheme>) {
    self.set_environment(EnvironmentRequest::Scheme(scheme));
  }

  /// Makes `choice` the path the windows this thread opens from now on ask for, in place of
  /// `SKENE_RENDERER`'s, until the test ends; `None` returns to it.
  pub fn set_path_choice(choice: Option<PathChoice>) {
    PathChoice::set_current(choice);
  }

  /// The path the open window paints with.
  #[must_use]
  pub fn paint_path(&self) -> PaintPath {
    Self::with_window(|window| window.shell().paint_path())
  }

  /// The GPU's device is lost under the open window, as a driver reset or a removed GPU loses
  /// it, and the window settles: it paints on the CPU from then on. Whether it painted on the
  /// GPU.
  pub fn trigger_device_loss(&mut self) -> bool {
    let lost = Self::with_window(|window| window.shell_mut().lose_device());

    settle();

    lost
  }
}

impl Driver for Drawn {
  type Found = Element;

  fn open(&mut self, page: &str) {
    let owner = Owner::new();
    let root = owner
      .run(|| skene_foundation::seam::testing::build(page))
      .unwrap_or_else(|| {
        panic!(
          "no page is registered as {page:?}; the pages are {:?}",
          skene_foundation::seam::testing::pages()
        )
      });

    self.show(owner, root);
  }

  fn settle(&mut self) {
    settle();
  }

  fn find_now(&mut self, query: &Query) -> Vec<Element> {
    self
      .items()
      .into_iter()
      .filter(|item| matches(query, item))
      .map(|item| Element {
        role: role_of(&item.role, item.states),
        platform_role: item.role,
        name: item.name,
        object: item.object,
      })
      .collect()
  }

  fn read(&mut self, found: &Element) -> AccessSnapshot {
    let object = &found.object;
    let fail = |error: String| -> ! { panic!("AT-SPI: {error}") };
    let (states, role_name, interfaces, description, name) = with_atspi(|atspi| {
      (
        atspi.states(object).unwrap_or_else(|error| fail(error)),
        atspi.role(object).unwrap_or_else(|error| fail(error)),
        atspi.interfaces(object).unwrap_or_default(),
        atspi.description(object).unwrap_or_default(),
        atspi.name(object).unwrap_or_else(|error| fail(error)),
      )
    });
    let role = role_of(&role_name, states);
    let has = |interface: &str| interfaces.iter().any(|name| name == interface);
    let bit = |bit: u32| states & (1 << bit) != 0;
    let text = (has("org.a11y.atspi.Text") && role != Role::Text)
      .then(|| with_atspi(|atspi| atspi.text(object)).ok())
      .flatten();
    let selection = text.as_ref().and_then(|text| {
      let (caret, start, end) = with_atspi(|atspi| atspi.caret_and_selection(object)).ok()?;
      let cluster = |offset: i32| {
        Graphemes::cluster(text, Unit::CodePoint, usize::try_from(offset).unwrap_or(0))
      };
      let (start, end) = (cluster(start), cluster(end));
      let caret = cluster(caret);

      Some(if start != end && caret == start {
        TextSelection {
          start: end,
          end: start,
        }
      } else {
        TextSelection { start, end }
      })
    });
    let checkable = matches!(role, Role::Switch | Role::Checkbox | Role::Radio);

    AccessSnapshot {
      role,
      platform_role: role_name.into(),
      name: name.into(),
      description: (!description.is_empty()).then(|| description.into()),
      checked: checkable.then(|| {
        if bit(state::INDETERMINATE) {
          Checked::Mixed
        } else if bit(state::CHECKED) {
          Checked::On
        } else {
          Checked::Off
        }
      }),
      enabled: bit(state::SENSITIVE),
      focused: bit(state::FOCUSED),
      expanded: (role == Role::ComboBox).then(|| bit(state::EXPANDED)),
      has_popup: bit(state::HAS_POPUP),
      value: has("org.a11y.atspi.Value")
        .then(|| with_atspi(|atspi| atspi.value(object)).ok())
        .flatten(),
      text: text.map(Rc::from),
      selection,
      bounds: Self::bounds(found),
    }
  }

  fn act(&mut self, found: &Element, action: &Action) {
    match action {
      Action::Click => Self::click(found, (0.5, 0.5), input::PRIMARY),
      Action::ClickAt(x, y) => {
        Self::click(found, (f64::from(*x), f64::from(*y)), input::PRIMARY);
      }
      Action::ContextClick => Self::click(found, (0.5, 0.5), input::SECONDARY),
      Action::Hover => {
        let point = Self::point(found, 0.5, 0.5);
        let before = trace::last_pointer();

        with_input(|input| {
          input.move_to(point.0, point.1)?;
          input.sync()
        })
        .unwrap_or_else(|error| panic!("pointer: {error}"));

        let _ = pump_until(Duration::from_millis(300), || {
          trace::last_pointer().0 != before.0
        });

        settle();
      }
      Action::Drag(right, down) => {
        let from = Self::point(found, 0.5, 0.5);
        let unit = Self::pointer_unit();
        let to = (
          from.0 + f64::from(*right) * unit,
          from.1 + f64::from(*down) * unit,
        );

        Self::send(2, |input| {
          input.move_to(from.0, from.1)?;
          input.sync()?;
          input.button(input::PRIMARY, true)?;
          input.sync()?;

          for step in 1..=10 {
            let t = f64::from(step) / 10.;

            input.move_to(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)?;
            input.sync()?;
            std::thread::sleep(Duration::from_millis(5));
          }

          input.button(input::PRIMARY, false)
        });
      }
      Action::Scroll(right, down) => {
        let point = Self::point(found, 0.5, 0.5);

        Self::send(1, |input| {
          input.move_to(point.0, point.1)?;
          input.sync()?;
          input.wheel(*right, *down)
        });
      }
      Action::Fling(right, down) => Self::fling(found, f64::from(*right), f64::from(*down)),
      Action::Tap => Self::tap(found, (0.5, 0.5)),
      Action::TapAt(x, y) => Self::tap(found, (f64::from(*x), f64::from(*y))),
      Action::LongPress => Self::long_press(found),
      Action::Swipe(right, down) => Self::swipe(found, f64::from(*right), f64::from(*down)),
      Action::Access(request) => {
        let asked = with_atspi(|atspi| atspi.request(&found.object, *request));

        pump_until(Duration::from_secs(5), || asked.is_finished());
        asked
          .join()
          .unwrap_or_else(|_| Err("the reader's call panicked".to_owned()))
          .unwrap_or_else(|error| panic!("access: {error}"));
        settle();
      }
      Action::Press(..) | Action::Type(_) | Action::Compose(_) | Action::Commit => {
        self.act_at_focus(action);
      }
    }
  }

  fn probe(&mut self, name: &str) -> Option<Rc<str>> {
    skene_foundation::seam::testing::probe(name).map(Rc::from)
  }

  fn set_environment(&mut self, request: EnvironmentRequest) {
    match request {
      EnvironmentRequest::Scheme(scheme) => environment::set_scheme(scheme),
      EnvironmentRequest::Size(width, height) => {
        let header = Self::with_window(|window| window.shell().chrome.header());

        Self::with_window(|window| {
          window
            .resize(width.round() as u32, (height + header).round() as u32)
            .unwrap_or_else(|error| panic!("resize: {error}"));
        });

        let wanted = (width.round(), height.round());
        let _ = pump_until(Duration::from_secs(2), || {
          let (width, height) = Self::with_window(|window| window.shell().content_size());

          (width.round(), height.round()) == wanted
        });
      }
      EnvironmentRequest::ReducedMotion(reduced) => {
        Self::change(Setting::ReducedMotion(u32::from(reduced)));
      }
      EnvironmentRequest::ReaderScheme(scheme) => {
        Self::change(Setting::ColorScheme(match scheme {
          Scheme::Dark => 1,
          Scheme::Light => 2,
        }))
      }
      EnvironmentRequest::HighContrast(high) => {
        Self::change(Setting::Contrast(u32::from(high)));
      }
    }

    settle();
  }

  fn screenshot(&mut self) -> Screenshot {
    self.content()
  }

  fn set_recording(&mut self, recording: bool) {
    let painted = SHOWN.with(|shown| {
      shown
        .borrow()
        .as_ref()
        .map_or(0, |shown| shown.window.shell().frames)
    });

    RECORDING.with(|current| {
      *current.borrow_mut() = recording.then(|| Recording {
        started: Instant::now(),
        frames: Vec::new(),
        painted,
      });
    });
  }

  fn take_frames(&mut self) -> Vec<Frame> {
    RECORDING.with(|recording| {
      recording
        .borrow_mut()
        .as_mut()
        .map(|recording| std::mem::take(&mut recording.frames))
        .unwrap_or_default()
    })
  }

  fn wait(&mut self, duration: Duration) {
    pump(duration);
  }

  fn outline(&mut self) -> String {
    self
      .items()
      .iter()
      .map(|item| {
        let states = state::names(item.states);

        format!(
          "{}{} {:?}{}",
          "  ".repeat(item.depth),
          item.role,
          item.name,
          if states.is_empty() {
            String::new()
          } else {
            format!(" [{}]", states.join(", "))
          }
        )
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  fn audit(&mut self) -> Vec<Issue> {
    let items = self.items();
    let mut issues = Vec::new();

    for item in items {
      let role = role_of(&item.role, item.states);
      let control = matches!(
        role,
        Role::Button
          | Role::Switch
          | Role::Checkbox
          | Role::Radio
          | Role::Slider
          | Role::SpinButton
          | Role::TextField
          | Role::TextEditor
          | Role::ComboBox
          | Role::Link
          | Role::Row
      );
      let problem = if control && item.name.trim().is_empty() {
        Some("a control without a name")
      } else if role == Role::Image && item.name.trim().is_empty() {
        Some("a picture without a description")
      } else if item.states & (1 << state::SENSITIVE) == 0
        && item.states & (1 << state::FOCUSED) != 0
      {
        Some("a disabled control that has focus")
      } else {
        None
      };

      if let Some(problem) = problem {
        let element = Element {
          role,
          platform_role: item.role.clone(),
          name: item.name.clone(),
          object: item.object.clone(),
        };

        issues.push(Issue {
          snapshot: self.read(&element),
          problem: problem.into(),
        });
      }
    }

    issues
  }

  fn find(&mut self, query: &Query, timeout: Duration) -> Vec<Element> {
    let started = Instant::now();

    loop {
      let found = self.find_now(query);

      if !found.is_empty() || started.elapsed() >= timeout {
        return found;
      }

      pump(Duration::from_millis(10));
      settle();
    }
  }
}

/// The key binding a reader reads for `found`'s action (its keyboard shortcut), empty where
/// it has none.
#[must_use]
pub fn key_binding(found: &Element) -> String {
  with_atspi(|atspi| atspi.key_binding(&found.object)).unwrap_or_default()
}
