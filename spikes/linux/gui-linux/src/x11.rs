//! The X11 backend over x11rb (libxcb underneath, so xkbcommon reads the keymap from the
//! server's XKB device):
//!
//! - a window the window manager decorates (or, where `SKENE_X11_CSD` is set, one asking for
//!   no decorations through `_MOTIF_WM_HINTS`, its title bar drawn by Skene and moved and
//!   resized through `_NET_WM_MOVERESIZE`);
//! - frames through MIT-SHM where the server offers it (a memfd segment, `ShmPutImage` of the
//!   damaged rectangle), `PutImage` in bands otherwise;
//! - the pointer through XInput 2 where the server has it (smooth scrolling from the scroll
//!   valuators, emulated wheel buttons ignored), core events otherwise; the keyboard through
//!   XKB, with detectable auto-repeat;
//! - XSETTINGS: `Xft/DPI` and `Gdk/WindowScalingFactor` for the scale, the `Xft/*` rendering
//!   settings beneath fontconfig, `Net/ThemeName` and `Gtk/EnableAnimations` where no
//!   settings portal answers;
//! - the `CLIPBOARD` and `PRIMARY` selections, owned and converted to UTF-8;
//! - the cursor from the reader's cursor theme;
//! - the open overlay (a picker's list) in an override-redirect window holding the pointer
//!   grab, a press outside it and its anchor dismissing it;
//! - the input method ([`crate::ime`]): IBus or Fcitx5 over D-Bus, or XIM, each key sent to it
//!   while a text entry has focus and handled here only where it gives the key back.

use std::{
  collections::{HashMap, HashSet, VecDeque},
  error::Error,
  os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd},
  rc::Rc,
  sync::Arc,
  time::{Duration, Instant},
};

use skene_parley::linux::xsettings::{self, Setting};
use x11rb::{
  connection::{Connection as _, RequestConnection as _},
  protocol::{
    Event,
    shm::{self, ConnectionExt as _},
    xinput::{self, ConnectionExt as _},
    xkb::{self as xkbproto, ConnectionExt as _},
    xproto::{
      AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt as _, CreateGCAux,
      CreateWindowAux, EventMask, GrabMode, ImageFormat, KeyPressEvent, PropMode,
      SelectionNotifyEvent, Window, WindowClass,
    },
  },
  wrapper::ConnectionExt as _,
  xcb_ffi::XCBConnection,
};
use xkbcommon::xkb;

use crate::{
  clipboard::{self, Op, Selection, Then},
  environment,
  ime::{self, Ime, ImeEvent, RawKey},
  keys::Keyboard,
  runtime::Message,
  shell::{Cursor, Request, Shell, Wheel},
  trace,
};

/// How long a key waits for the input method's answer before the window handles it.
const IME_PATIENCE: Duration = Duration::from_millis(800);

/// The open overlay on its own window.
struct PopupWindow {
  window: Window,
  size: (u32, u32),
}

/// A key sent to the input method and not answered yet.
struct Waiting {
  id: u64,
  key: RawKey,
  repeat: bool,
  sent: Instant,
}

x11rb::atom_manager! {
  Atoms: AtomsCookie {
    WM_PROTOCOLS,
    WM_DELETE_WINDOW,
    WM_CHANGE_STATE,
    _NET_WM_NAME,
    _NET_WM_PID,
    _NET_SUPPORTING_WM_CHECK,
    _NET_WM_MOVERESIZE,
    _NET_WM_STATE,
    _NET_WM_STATE_MAXIMIZED_VERT,
    _NET_WM_STATE_MAXIMIZED_HORZ,
    _NET_WM_STATE_FULLSCREEN,
    _MOTIF_WM_HINTS,
    UTF8_STRING,
    CLIPBOARD,
    PRIMARY,
    TARGETS,
    TEXT,
    STRING,
    TEXT_PLAIN: b"text/plain;charset=utf-8",
    SKENE_SELECTION,
    _XSETTINGS_SETTINGS,
  }
}

/// The keymap parts whose change means reloading the keymap: selecting them is what makes
/// the server send `XkbMapNotify`.
fn map_parts() -> xkbproto::MapPart {
  xkbproto::MapPart::KEY_TYPES
    | xkbproto::MapPart::KEY_SYMS
    | xkbproto::MapPart::MODIFIER_MAP
    | xkbproto::MapPart::EXPLICIT_COMPONENTS
    | xkbproto::MapPart::KEY_ACTIONS
    | xkbproto::MapPart::VIRTUAL_MODS
    | xkbproto::MapPart::VIRTUAL_MOD_MAP
}

/// A shared memory segment the server reads frames from.
struct Segment {
  seg: shm::Seg,
  memory: *mut u8,
  length: usize,
}

impl Drop for Segment {
  fn drop(&mut self) {
    // SAFETY: unmaps the mapping made for this segment, once.
    unsafe {
      libc::munmap(self.memory.cast(), self.length);
    }
  }
}

/// What the input method was last told: the surrounding text, the caret and anchor bytes, the
/// caret's rectangle on the screen.
type ImeSent = (String, u32, u32, (i32, i32, i32, i32));

/// A scroll valuator of a pointer device: which axis, how much one detent is, and its last
/// value.
struct ScrollAxis {
  vertical: bool,
  increment: f64,
  last: Option<f64>,
}

struct X11 {
  conn: Rc<XCBConnection>,
  screen: usize,
  root: Window,
  window: Window,
  gc: u32,
  depth: u8,
  atoms: Atoms,
  keyboard: Keyboard,
  device: i32,
  xi: bool,
  scroll: HashMap<(u16, u16), ScrollAxis>,
  segment: Option<Segment>,
  shm: bool,
  buffer: Vec<u8>,
  cursors: Option<x11rb::cursor::Handle>,
  cursor_ids: HashMap<&'static str, u32>,
  xsettings: Option<(Window, u32)>,
  owned: HashMap<Selection, String>,
  reads: Vec<(Selection, Then)>,
  time: u32,
  last_press: (i16, i16),
  keys_down: HashSet<u32>,
  scale_override: Option<f64>,
  window_size: (u32, u32),
  window_origin: (i32, i32),
  popup: Option<PopupWindow>,
  ime: Option<Box<dyn Ime>>,
  ime_waiting: Option<Waiting>,
  ime_queue: VecDeque<(RawKey, bool)>,
  ime_next: u64,
  ime_focused: bool,
  ime_stale: bool,
  ime_sent: Option<ImeSent>,
  ime_purpose: Option<ime::Purpose>,
}

fn fixed(value: xinput::Fp1616) -> f32 {
  value as f32 / 65536.
}

fn fixed64(value: xinput::Fp3232) -> f64 {
  f64::from(value.integral) + f64::from(value.frac) / 4_294_967_296.
}

/// A window open on `DISPLAY`, its loop turned a step at a time.
pub struct Open {
  x11: X11,
  pending: Option<Event>,
}

/// Opens the window on `DISPLAY`.
pub fn open(shell: &mut Shell, app_id: &str) -> Result<Open, Box<dyn Error>> {
  Ok(Open {
    x11: X11::open(shell, app_id)?,
    pending: None,
  })
}

impl Open {
  /// Turns the loop once: handles what arrived, paints what changed, then waits for more,
  /// `wait` at most where given.
  pub fn step(&mut self, shell: &mut Shell, wait: Option<Duration>) -> Result<(), Box<dyn Error>> {
    self.x11.step(shell, &mut self.pending, wait)
  }

  /// Asks for the window's geometry to be `width` by `height` pixels.
  pub fn resize(&mut self, shell: &Shell, width: u32, height: u32) -> Result<(), Box<dyn Error>> {
    use x11rb::protocol::xproto::ConfigureWindowAux;

    let (width, height) = shell.chrome.surface_size(width, height);
    let scale = shell.scale;

    self.x11.conn.configure_window(
      self.x11.window,
      &ConfigureWindowAux::new()
        .width((f64::from(width) * scale).round() as u32)
        .height((f64::from(height) * scale).round() as u32),
    )?;
    self.x11.conn.flush()?;

    Ok(())
  }
}

impl X11 {
  fn open(shell: &mut Shell, app_id: &str) -> Result<Self, Box<dyn Error>> {
    let (conn, screen) = XCBConnection::connect(None)?;
    let setup_screen = conn.setup().roots[screen].clone();
    let atoms = Atoms::new(&conn)?.reply()?;

    conn.xkb_use_extension(1, 0)?.reply()?;

    let (mut major, mut minor, mut base_event, mut base_error) = (0, 0, 0, 0);

    if !xkb::x11::setup_xkb_extension(
      &conn,
      1,
      0,
      xkb::x11::SetupXkbExtensionFlags::NoFlags,
      &mut major,
      &mut minor,
      &mut base_event,
      &mut base_error,
    ) {
      return Err("the X server has no XKB".into());
    }

    let device = xkb::x11::get_core_keyboard_device_id(&conn);
    let mut keyboard = Keyboard::new();

    load_keymap(&mut keyboard, &conn, device);
    conn.xkb_select_events(
      device as u16,
      xkbproto::EventType::from(0u16),
      xkbproto::EventType::NEW_KEYBOARD_NOTIFY
        | xkbproto::EventType::MAP_NOTIFY
        | xkbproto::EventType::STATE_NOTIFY,
      map_parts(),
      map_parts(),
      &xkbproto::SelectEventsAux::new(),
    )?;

    // A held key repeats as presses alone, so a repeat is told from a new press.
    let _ = conn.xkb_per_client_flags(
      xkbproto::ID::USE_CORE_KBD.into(),
      xkbproto::PerClientFlag::DETECTABLE_AUTO_REPEAT,
      xkbproto::PerClientFlag::DETECTABLE_AUTO_REPEAT,
      xkbproto::BoolCtrl::from(0u32),
      xkbproto::BoolCtrl::from(0u32),
      xkbproto::BoolCtrl::from(0u32),
    )?;

    let mut x11 = Self {
      root: setup_screen.root,
      window: 0,
      gc: 0,
      depth: setup_screen.root_depth,
      conn: Rc::new(conn),
      screen,
      atoms,
      keyboard,
      device,
      xi: false,
      scroll: HashMap::new(),
      segment: None,
      shm: false,
      buffer: Vec::new(),
      cursors: None,
      cursor_ids: HashMap::new(),
      xsettings: None,
      owned: HashMap::new(),
      reads: Vec::new(),
      time: x11rb::CURRENT_TIME,
      last_press: (0, 0),
      keys_down: HashSet::new(),
      scale_override: std::env::var("SKENE_SCALE")
        .ok()
        .and_then(|scale| scale.parse().ok()),
      window_size: (0, 0),
      window_origin: (0, 0),
      popup: None,
      ime: None,
      ime_waiting: None,
      ime_queue: VecDeque::new(),
      ime_next: 1,
      ime_focused: false,
      ime_stale: true,
      ime_sent: None,
      ime_purpose: None,
    };

    x11.read_xsettings(shell);

    if let Some(scale) = x11.scale_override {
      shell.set_scale(scale);
    }

    x11.create_window(shell, app_id)?;
    x11.select_input()?;
    x11.start_ime();

    // On a bare X server (no window manager) the keyboard follows the pointer root: the window
    // has it from the start.
    if let Ok(focus) = x11.conn.get_input_focus()?.reply()
      && (focus.focus == x11.window || focus.focus == 1)
    {
      shell.keyboard_focus(true);
    }
    x11.shm = x11
      .conn
      .extension_information(shm::X11_EXTENSION_NAME)?
      .is_some()
      && std::env::var_os("SKENE_X11_NO_SHM").is_none();

    if let Ok(database) = x11rb::resource_manager::new_from_default(&x11.conn)
      && let Ok(cookie) = x11rb::cursor::Handle::new(&x11.conn, x11.screen, &database)
    {
      x11.cursors = cookie.reply().ok();
    }

    trace::log(|| {
      format!(
        "[x11] window 0x{:x} depth {} xi2 {} shm {} scale {} layouts {:?}",
        x11.window,
        x11.depth,
        x11.xi,
        x11.shm,
        shell.scale,
        x11.keyboard.layouts()
      )
    });

    Ok(x11)
  }

  fn create_window(&mut self, shell: &mut Shell, app_id: &str) -> Result<(), Box<dyn Error>> {
    let conn = &self.conn;
    let window = conn.generate_id()?;
    let csd = std::env::var_os("SKENE_X11_CSD").is_some();

    if csd {
      shell.chrome.csd = true;
      shell.chrome.shadow = false;
      shell.chrome.states.activated = true;
    }

    let (width, height) = shell.device_size();

    self.window_size = (width, height);
    conn.create_window(
      x11rb::COPY_DEPTH_FROM_PARENT,
      window,
      self.root,
      0,
      0,
      width as u16,
      height as u16,
      0,
      WindowClass::INPUT_OUTPUT,
      0,
      &CreateWindowAux::new()
        .background_pixel(0)
        .bit_gravity(x11rb::protocol::xproto::Gravity::NORTH_WEST)
        .event_mask(
          EventMask::EXPOSURE
            | EventMask::STRUCTURE_NOTIFY
            | EventMask::KEY_PRESS
            | EventMask::KEY_RELEASE
            | EventMask::BUTTON_PRESS
            | EventMask::BUTTON_RELEASE
            | EventMask::POINTER_MOTION
            | EventMask::ENTER_WINDOW
            | EventMask::LEAVE_WINDOW
            | EventMask::PROPERTY_CHANGE
            | EventMask::FOCUS_CHANGE,
        ),
    )?;

    let title = shell.title().as_bytes().to_vec();
    let class = format!("{app_id}\0{app_id}\0");

    conn.change_property8(
      PropMode::REPLACE,
      window,
      self.atoms._NET_WM_NAME,
      self.atoms.UTF8_STRING,
      &title,
    )?;
    conn.change_property8(
      PropMode::REPLACE,
      window,
      AtomEnum::WM_NAME,
      AtomEnum::STRING,
      &title,
    )?;
    conn.change_property8(
      PropMode::REPLACE,
      window,
      AtomEnum::WM_CLASS,
      AtomEnum::STRING,
      class.as_bytes(),
    )?;
    conn.change_property32(
      PropMode::REPLACE,
      window,
      self.atoms.WM_PROTOCOLS,
      AtomEnum::ATOM,
      &[self.atoms.WM_DELETE_WINDOW],
    )?;
    conn.change_property32(
      PropMode::REPLACE,
      window,
      self.atoms._NET_WM_PID,
      AtomEnum::CARDINAL,
      &[std::process::id()],
    )?;

    if csd {
      conn.change_property32(
        PropMode::REPLACE,
        window,
        self.atoms._MOTIF_WM_HINTS,
        self.atoms._MOTIF_WM_HINTS,
        &[2, 0, 0, 0, 0],
      )?;
    }

    conn.map_window(window)?;

    let gc = conn.generate_id()?;

    conn.create_gc(gc, window, &CreateGCAux::new().graphics_exposures(0))?;
    conn.flush()?;
    self.window = window;
    self.gc = gc;
    shell.configure(shell.size.0, shell.size.1);

    Ok(())
  }

  /// Selects XInput 2's pointer events where the server has them, and learns the scroll
  /// valuators of every pointer.
  fn select_input(&mut self) -> Result<(), Box<dyn Error>> {
    if std::env::var_os("SKENE_X11_NO_XI2").is_some()
      || self
        .conn
        .extension_information(xinput::X11_EXTENSION_NAME)?
        .is_none()
    {
      return Ok(());
    }

    let Ok(version) = self.conn.xinput_xi_query_version(2, 2)?.reply() else {
      return Ok(());
    };

    if (version.major_version, version.minor_version) < (2, 1) {
      return Ok(());
    }

    self.conn.xinput_xi_select_events(
      self.window,
      &[xinput::EventMask {
        deviceid: xinput::Device::ALL_MASTER.into(),
        mask: vec![
          xinput::XIEventMask::BUTTON_PRESS
            | xinput::XIEventMask::BUTTON_RELEASE
            | xinput::XIEventMask::MOTION
            | xinput::XIEventMask::ENTER
            | xinput::XIEventMask::LEAVE,
        ],
      }],
    )?;
    self.xi = true;
    self.learn_devices()?;

    Ok(())
  }

  fn learn_devices(&mut self) -> Result<(), Box<dyn Error>> {
    self.scroll.clear();

    let devices = self
      .conn
      .xinput_xi_query_device(xinput::Device::ALL_MASTER)?
      .reply()?;

    for info in devices.infos {
      for class in info.classes {
        if let xinput::DeviceClassData::Scroll(scroll) = class.data {
          self.scroll.insert(
            (info.deviceid, scroll.number),
            ScrollAxis {
              vertical: scroll.scroll_type == xinput::ScrollType::VERTICAL,
              increment: fixed64(scroll.increment).abs().max(f64::EPSILON)
                * fixed64(scroll.increment).signum(),
              last: None,
            },
          );
        }
      }
    }

    trace::log(|| format!("[x11] scroll valuators {}", self.scroll.len()));

    Ok(())
  }

  /// Reads the XSETTINGS manager's settings and follows its changes.
  fn read_xsettings(&mut self, shell: &mut Shell) {
    let Ok(name) = self
      .conn
      .intern_atom(false, format!("_XSETTINGS_S{}", self.screen).as_bytes())
      .map_err(|_| ())
      .and_then(|cookie| cookie.reply().map_err(|_| ()))
    else {
      return;
    };
    let Ok(owner) = self
      .conn
      .get_selection_owner(name.atom)
      .map_err(|_| ())
      .and_then(|cookie| cookie.reply().map_err(|_| ()))
    else {
      return;
    };

    if owner.owner == x11rb::NONE {
      trace::log(|| "[xsettings] no manager".to_owned());

      return;
    }

    let _ = self.conn.change_window_attributes(
      owner.owner,
      &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    );
    self.xsettings = Some((owner.owner, self.atoms._XSETTINGS_SETTINGS));
    self.apply_xsettings(shell);
  }

  fn apply_xsettings(&mut self, shell: &mut Shell) {
    let Some((owner, atom)) = self.xsettings else {
      return;
    };
    let Ok(reply) = self
      .conn
      .get_property(false, owner, atom, AtomEnum::ANY, 0, u32::MAX / 4)
      .map_err(|_| ())
      .and_then(|cookie| cookie.reply().map_err(|_| ()))
    else {
      return;
    };
    let Some(settings) = xsettings::parse(&reply.value) else {
      return;
    };
    let integer = |name: &str| {
      settings.iter().find_map(|(key, value)| match value {
        Setting::Integer(value) if key == name => Some(*value),
        _ => None,
      })
    };
    let text = |name: &str| {
      settings.iter().find_map(|(key, value)| match value {
        Setting::Text(value) if key == name => Some(value.clone()),
        _ => None,
      })
    };
    let dpi = integer("Xft/DPI").filter(|dpi| *dpi > 0);
    let factor = integer("Gdk/WindowScalingFactor").filter(|factor| *factor > 0);
    let scale = match (factor, dpi) {
      (Some(factor), _) => f64::from(factor),
      (None, Some(dpi)) => f64::from(dpi) / 1024. / 96.,
      (None, None) => 1.,
    };

    trace::log(|| {
      format!(
        "[xsettings] {} settings, dpi {dpi:?}, scale {scale}",
        settings.len()
      )
    });

    if self.scale_override.is_none() && (scale - shell.scale).abs() > f64::EPSILON {
      shell.set_scale(scale);

      // The window keeps its size on the screen; the content takes the new scale's.
      if self.window_size != (0, 0) {
        shell.configure(
          (f64::from(self.window_size.0) / scale).round() as u32,
          (f64::from(self.window_size.1) / scale).round() as u32,
        );
      }
    }

    environment::xft(xsettings::desktop_settings(&settings));
    environment::xsettings(
      text("Net/ThemeName").as_deref(),
      integer("Gtk/EnableAnimations").map(|enabled| enabled != 0),
      integer("Net/CursorBlink").map(|blinks| blinks != 0),
    );
  }

  fn step(
    &mut self,
    shell: &mut Shell,
    pending: &mut Option<Event>,
    wait: Option<Duration>,
  ) -> Result<(), Box<dyn Error>> {
    let fd = self.conn.as_raw_fd();
    let inbox = crate::runtime::inbox();
    let deadline = wait.map(|wait| Instant::now() + wait);

    loop {
      while let Some(event) = match pending.take() {
        Some(event) => Some(event),
        None => self.conn.poll_for_event()?,
      } {
        self.event(shell, event)?;
      }

      for message in crate::turn(shell) {
        if let Message::Ime(event) = message {
          self.ime_event(shell, event);
        }
      }

      if let Some(ime) = &mut self.ime {
        let events = ime.take_events();

        for event in events {
          self.ime_event(shell, event);
        }
      }

      self.ime_timeout(shell);
      self.clipboard()?;

      let asked = shell.take_requests();

      self.requests(shell, asked)?;
      self.sync_ime(shell);

      if shell.quit {
        return Ok(());
      }

      if shell.wants_frame() {
        self.present(shell)?;
      }

      self.present_popup(shell)?;

      self.conn.flush()?;

      // libxcb reads incoming events into its queue while it waits to write a large
      // request, where polling the socket no longer sees them.
      if let Some(event) = self.conn.poll_for_event()? {
        *pending = Some(event);

        continue;
      }

      if shell.wants_frame() || shell.popup_wants_frame() {
        continue;
      }

      let mut fds = [
        libc::pollfd {
          fd,
          events: libc::POLLIN,
          revents: 0,
        },
        libc::pollfd {
          fd: inbox.read,
          events: libc::POLLIN,
          revents: 0,
        },
      ];
      let ime_deadline = self
        .ime_waiting
        .as_ref()
        .map(|waiting| (waiting.sent + IME_PATIENCE).saturating_duration_since(Instant::now()));
      let left = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
      let timeout = [crate::runtime::timeout(), ime_deadline]
        .into_iter()
        .flatten()
        .min()
        .map(|timeout| timeout + Duration::from_millis(1))
        .into_iter()
        .chain(left)
        .min()
        .map_or(-1, |timeout| {
          i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX)
        });

      // SAFETY: polls two live descriptors.
      unsafe {
        libc::poll(fds.as_mut_ptr(), 2, timeout);
      }

      return Ok(());
    }
  }

  fn requests(&mut self, shell: &mut Shell, requests: Vec<Request>) -> Result<(), Box<dyn Error>> {
    for request in requests {
      trace::log(|| format!("[x11] request {request:?}"));

      match request {
        Request::Close => shell.quit = true,
        Request::Move => self.move_resize(8)?,
        Request::Resize(edge) => {
          use crate::chrome::Edge;

          self.move_resize(match edge {
            Edge::TopLeft => 0,
            Edge::Top => 1,
            Edge::TopRight => 2,
            Edge::Right => 3,
            Edge::BottomRight => 4,
            Edge::Bottom => 5,
            Edge::BottomLeft => 6,
            Edge::Left => 7,
          })?;
        }
        Request::ToggleMaximize => self.wm_state(2)?,
        Request::Minimize => {
          self.conn.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            ClientMessageEvent::new(32, self.window, self.atoms.WM_CHANGE_STATE, [3, 0, 0, 0, 0]),
          )?;
        }
        Request::ShowMenu(..) => {}
        Request::Cursor(cursor) => self.set_cursor(cursor)?,
        Request::Popup(placement) => self.place_popup(shell, placement)?,
      }
    }

    Ok(())
  }

  fn move_resize(&self, direction: u32) -> Result<(), Box<dyn Error>> {
    self.conn.ungrab_pointer(x11rb::CURRENT_TIME)?;
    self.conn.send_event(
      false,
      self.root,
      EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
      ClientMessageEvent::new(
        32,
        self.window,
        self.atoms._NET_WM_MOVERESIZE,
        [
          self.last_press.0 as u32,
          self.last_press.1 as u32,
          direction,
          1,
          1,
        ],
      ),
    )?;

    Ok(())
  }

  fn wm_state(&self, action: u32) -> Result<(), Box<dyn Error>> {
    self.conn.send_event(
      false,
      self.root,
      EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
      ClientMessageEvent::new(
        32,
        self.window,
        self.atoms._NET_WM_STATE,
        [
          action,
          self.atoms._NET_WM_STATE_MAXIMIZED_VERT,
          self.atoms._NET_WM_STATE_MAXIMIZED_HORZ,
          1,
          0,
        ],
      ),
    )?;

    Ok(())
  }

  fn set_cursor(&mut self, cursor: Cursor) -> Result<(), Box<dyn Error>> {
    let Some(handle) = &self.cursors else {
      return Ok(());
    };
    let name = cursor.name();
    let id = match self.cursor_ids.get(name) {
      Some(id) => *id,
      None => {
        let id = handle.load_cursor(&self.conn, name)?;

        self.cursor_ids.insert(name, id);

        id
      }
    };

    self
      .conn
      .change_window_attributes(self.window, &ChangeWindowAttributesAux::new().cursor(id))?;

    Ok(())
  }

  fn pointer_at(&self, x: f32, y: f32) -> (f32, f32) {
    (x, y)
  }

  fn button(
    &mut self,
    shell: &mut Shell,
    detail: u32,
    down: bool,
    at: (f32, f32),
    emulated: bool,
  ) -> Result<(), Box<dyn Error>> {
    let scale = shell.scale as f32;
    let (x, y) = (at.0 / scale, at.1 / scale);

    match detail {
      1..=3 => {
        let button = match detail {
          1 => 0,
          2 => 1,
          _ => 2,
        };
        let requests = shell.pointer_button(button, down, self.pointer_at(x, y));

        self.requests(shell, requests)?;
      }
      4..=7 if down && !emulated => {
        let (dx, dy) = match detail {
          4 => (0., -1.),
          5 => (0., 1.),
          6 => (-1., 0.),
          _ => (1., 0.),
        };

        shell.wheel(Wheel::Lines(dx, dy));
      }
      _ => {}
    }

    Ok(())
  }

  fn motion(&mut self, shell: &mut Shell, at: (f32, f32)) -> Result<(), Box<dyn Error>> {
    let scale = shell.scale as f32;
    let (x, y) = (at.0 / scale, at.1 / scale);
    let (ox, oy) = shell.chrome.origin();

    trace::pointer(x - ox, y - oy);

    let requests = shell.pointer_motion(x, y);

    self.requests(shell, requests)
  }

  #[expect(clippy::too_many_lines, reason = "one arm per event")]
  fn event(&mut self, shell: &mut Shell, event: Event) -> Result<(), Box<dyn Error>> {
    if let Some(ime) = &mut self.ime
      && ime.filter_event(&event)
    {
      return Ok(());
    }

    match event {
      Event::Expose(event)
        if self
          .popup
          .as_ref()
          .is_some_and(|popup| popup.window == event.window) =>
      {
        if let Some(list) = shell.popup_mut() {
          list.invalidate();
        }
      }
      Event::Expose(_) => crate::shell::invalidate_all(),
      Event::ConfigureNotify(event) if event.window == self.window => {
        let scale = shell.scale;

        self.window_size = (u32::from(event.width), u32::from(event.height));

        let width = (f64::from(event.width) / scale).round() as u32;
        let height = (f64::from(event.height) / scale).round() as u32;

        if (width, height) != shell.size {
          trace::log(|| format!("[x11] configure {width}x{height}"));
          shell.configure(width, height);
        }

        self.locate_window(shell);
      }
      Event::MapNotify(event) if event.window == self.window => {
        // Without a window manager nothing gives the keyboard to a window: it takes it, as
        // GTK's windows do there.
        let managed = self
          .conn
          .get_property(
            false,
            self.root,
            self.atoms._NET_SUPPORTING_WM_CHECK,
            AtomEnum::WINDOW,
            0,
            1,
          )?
          .reply()?
          .value_len
          > 0;

        if !managed {
          self.conn.set_input_focus(
            x11rb::protocol::xproto::InputFocus::PARENT,
            self.window,
            x11rb::CURRENT_TIME,
          )?;
        }
      }
      Event::FocusIn(_) => {
        shell.keyboard_focus(true);
        self.ime_stale = true;
      }
      Event::FocusOut(event) => {
        // The popup's pointer grab moves no keyboard focus; a grab's notification is not a
        // loss of focus.
        if event.mode != x11rb::protocol::xproto::NotifyMode::GRAB
          && event.mode != x11rb::protocol::xproto::NotifyMode::UNGRAB
        {
          shell.keyboard_focus(false);

          if let Some(ime) = &mut self.ime
            && self.ime_focused
          {
            ime.focus_out();
            self.ime_focused = false;
          }
        }
      }
      Event::XkbStateNotify(event) => self.keyboard.update_mask(
        [
          u32::from(event.base_mods),
          u32::from(event.latched_mods),
          u32::from(event.locked_mods),
        ],
        [
          event.base_group as u32,
          event.latched_group as u32,
          u32::from(event.locked_group),
        ],
      ),
      Event::XkbNewKeyboardNotify(_) | Event::XkbMapNotify(_) | Event::MappingNotify(_) => {
        load_keymap(&mut self.keyboard, &self.conn, self.device);
      }
      Event::KeyPress(event) => {
        self.time = event.time;

        let keycode = u32::from(event.detail);
        let repeat = !self.keys_down.insert(keycode);

        self.key(shell, event, false, repeat);
      }
      Event::KeyRelease(event) => {
        self.time = event.time;

        let keycode = u32::from(event.detail);

        self.keys_down.remove(&keycode);
        self.key(shell, event, true, false);
      }
      Event::ButtonPress(event) | Event::ButtonRelease(event)
        if self
          .popup
          .as_ref()
          .is_some_and(|popup| popup.window == event.event) =>
      {
        let down = event.response_type & 0x7f == x11rb::protocol::xproto::BUTTON_PRESS_EVENT;
        let scale = shell.scale as f32;
        let (x, y) = (
          f32::from(event.event_x) / scale,
          f32::from(event.event_y) / scale,
        );

        match event.detail {
          4 | 5 if down => {
            shell.popup_wheel(Wheel::Lines(0., if event.detail == 4 { -1. } else { 1. }))
          }
          1..=3 => shell.popup_button(down, x, y),
          _ => {}
        }

        let pending = shell.take_requests();

        self.requests(shell, pending)?;
      }
      Event::MotionNotify(event)
        if self
          .popup
          .as_ref()
          .is_some_and(|popup| popup.window == event.event) =>
      {
        let scale = shell.scale as f32;

        shell.popup_motion(
          f32::from(event.event_x) / scale,
          f32::from(event.event_y) / scale,
        );
      }

      Event::XinputButtonPress(event) | Event::XinputButtonRelease(event) => {
        self.time = event.time;

        let down = event.event_type == xinput::BUTTON_PRESS_EVENT;
        let emulated = event
          .flags
          .contains(xinput::PointerEventFlags::POINTER_EMULATED);

        if down {
          self.last_press = ((fixed(event.root_x)) as i16, (fixed(event.root_y)) as i16);
        }

        self.button(
          shell,
          event.detail,
          down,
          (fixed(event.event_x), fixed(event.event_y)),
          emulated,
        )?;
      }
      Event::XinputMotion(event) => {
        self.time = event.time;
        self.smooth_scroll(
          shell,
          event.deviceid,
          &event.valuator_mask,
          &event.axisvalues,
        );
        self.motion(shell, (fixed(event.event_x), fixed(event.event_y)))?;
      }
      Event::XinputEnter(event) => {
        for axis in self.scroll.values_mut() {
          axis.last = None;
        }

        self.motion(shell, (fixed(event.event_x), fixed(event.event_y)))?;
      }
      Event::XinputLeave(_) | Event::LeaveNotify(_) => shell.pointer_leave(),
      Event::ButtonPress(event) | Event::ButtonRelease(event) if !self.xi => {
        self.time = event.time;

        let down = event.response_type & 0x7f == x11rb::protocol::xproto::BUTTON_PRESS_EVENT;

        if down {
          self.last_press = (event.root_x, event.root_y);
        }

        self.button(
          shell,
          u32::from(event.detail),
          down,
          (f32::from(event.event_x), f32::from(event.event_y)),
          false,
        )?;
      }
      Event::MotionNotify(event) if !self.xi => {
        self.motion(shell, (f32::from(event.event_x), f32::from(event.event_y)))?;
      }
      Event::EnterNotify(event) if !self.xi => {
        self.motion(shell, (f32::from(event.event_x), f32::from(event.event_y)))?;
      }
      Event::PropertyNotify(event)
        if event.window == self.window && event.atom == self.atoms._NET_WM_STATE =>
      {
        let states = self
          .conn
          .get_property(
            false,
            self.window,
            self.atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            0,
            64,
          )?
          .reply()?;
        let held: Vec<u32> = states.value32().map(Iterator::collect).unwrap_or_default();

        shell.chrome.states.maximized = held.contains(&self.atoms._NET_WM_STATE_MAXIMIZED_VERT);
        shell.chrome.states.fullscreen = held.contains(&self.atoms._NET_WM_STATE_FULLSCREEN);
        trace::log(|| format!("[x11] maximized {}", shell.chrome.states.maximized));
        crate::shell::invalidate_layout();
        crate::shell::invalidate_all();
      }
      Event::PropertyNotify(event)
        if self
          .xsettings
          .is_some_and(|(owner, atom)| event.window == owner && event.atom == atom) =>
      {
        self.apply_xsettings(shell);
      }
      Event::ClientMessage(event) if event.format == 32 => {
        if event.data.as_data32()[0] == self.atoms.WM_DELETE_WINDOW {
          shell.quit = true;
        }
      }
      Event::SelectionRequest(request) => self.answer_selection(&request)?,
      Event::SelectionClear(event) => {
        if let Some(selection) = self.selection_of(event.selection) {
          self.owned.remove(&selection);
        }
      }
      Event::SelectionNotify(event) => self.selection_arrived(&event)?,
      Event::XinputDeviceChanged(_) | Event::XinputHierarchy(_) => self.learn_devices()?,
      _ => {}
    }

    Ok(())
  }

  /// Turns the scroll valuators that moved into wheel motion.
  fn smooth_scroll(
    &mut self,
    shell: &mut Shell,
    device: u16,
    mask: &[u32],
    values: &[xinput::Fp3232],
  ) {
    let mut values = values.iter();
    let (mut dx, mut dy) = (0., 0.);

    for (word, bits) in mask.iter().enumerate() {
      for bit in 0..32 {
        if bits & (1 << bit) == 0 {
          continue;
        }

        let number = (word * 32 + bit) as u16;
        let Some(value) = values.next() else {
          return;
        };
        let value = fixed64(*value);

        if let Some(axis) = self.scroll.get_mut(&(device, number)) {
          if let Some(last) = axis.last {
            let delta = ((value - last) / axis.increment) as f32;

            if axis.vertical {
              dy += delta;
            } else {
              dx += delta;
            }
          }

          axis.last = Some(value);
        }
      }
    }

    if dx != 0. || dy != 0. {
      shell.wheel(Wheel::Lines(dx, dy));
    }
  }

  fn selection_atom(&self, selection: Selection) -> u32 {
    match selection {
      Selection::Clipboard => self.atoms.CLIPBOARD,
      Selection::Primary => self.atoms.PRIMARY,
    }
  }

  fn selection_of(&self, atom: u32) -> Option<Selection> {
    if atom == self.atoms.CLIPBOARD {
      Some(Selection::Clipboard)
    } else if atom == self.atoms.PRIMARY {
      Some(Selection::Primary)
    } else {
      None
    }
  }

  /// Carries out the clipboard operations queued.
  fn clipboard(&mut self) -> Result<(), Box<dyn Error>> {
    for op in clipboard::take() {
      match op {
        Op::Write(selection, text) => {
          let atom = self.selection_atom(selection);

          self
            .conn
            .set_selection_owner(self.window, atom, self.time)?;
          trace::log(|| format!("[clipboard] own {selection:?} ({} bytes)", text.len()));
          self.owned.insert(selection, text);
        }
        Op::Read(selection, then) => {
          if let Some(text) = self.owned.get(&selection) {
            then(Some(text.clone()));

            continue;
          }

          self.conn.convert_selection(
            self.window,
            self.selection_atom(selection),
            self.atoms.UTF8_STRING,
            self.atoms.SKENE_SELECTION,
            self.time,
          )?;
          self.reads.push((selection, then));
        }
      }
    }

    Ok(())
  }

  fn answer_selection(
    &self,
    request: &x11rb::protocol::xproto::SelectionRequestEvent,
  ) -> Result<(), Box<dyn Error>> {
    let text = self
      .selection_of(request.selection)
      .and_then(|selection| self.owned.get(&selection));
    let property = if request.property == x11rb::NONE {
      request.target
    } else {
      request.property
    };
    let answered = match text {
      Some(_) if request.target == self.atoms.TARGETS => {
        self.conn.change_property32(
          PropMode::REPLACE,
          request.requestor,
          property,
          AtomEnum::ATOM,
          &[
            self.atoms.TARGETS,
            self.atoms.UTF8_STRING,
            self.atoms.TEXT_PLAIN,
            self.atoms.STRING,
            self.atoms.TEXT,
          ],
        )?;

        true
      }
      Some(text)
        if [
          self.atoms.UTF8_STRING,
          self.atoms.TEXT_PLAIN,
          self.atoms.STRING,
          self.atoms.TEXT,
        ]
        .contains(&request.target) =>
      {
        let kind = if request.target == self.atoms.STRING {
          self.atoms.STRING
        } else {
          self.atoms.UTF8_STRING
        };

        self.conn.change_property8(
          PropMode::REPLACE,
          request.requestor,
          property,
          kind,
          text.as_bytes(),
        )?;

        true
      }
      _ => false,
    };

    self.conn.send_event(
      false,
      request.requestor,
      EventMask::NO_EVENT,
      SelectionNotifyEvent {
        response_type: x11rb::protocol::xproto::SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time: request.time,
        requestor: request.requestor,
        selection: request.selection,
        target: request.target,
        property: if answered { property } else { x11rb::NONE },
      },
    )?;

    Ok(())
  }

  fn selection_arrived(&mut self, event: &SelectionNotifyEvent) -> Result<(), Box<dyn Error>> {
    let Some(selection) = self.selection_of(event.selection) else {
      return Ok(());
    };
    let Some(at) = self.reads.iter().position(|(read, _)| *read == selection) else {
      return Ok(());
    };
    let (_, then) = self.reads.remove(at);

    if event.property == x11rb::NONE {
      then(None);

      return Ok(());
    }

    let reply = self
      .conn
      .get_property(
        true,
        self.window,
        event.property,
        AtomEnum::ANY,
        0,
        u32::MAX / 4,
      )?
      .reply()?;

    then(String::from_utf8(reply.value).ok());

    Ok(())
  }

  fn segment(&mut self, length: usize) -> Result<Option<&Segment>, Box<dyn Error>> {
    if self
      .segment
      .as_ref()
      .is_some_and(|segment| segment.length >= length)
    {
      return Ok(self.segment.as_ref());
    }

    if let Some(old) = self.segment.take() {
      self.conn.shm_detach(old.seg)?;
    }

    // SAFETY: creates an anonymous memory file; the name is a C string.
    let fd = unsafe { libc::memfd_create(c"skene-frame".as_ptr(), libc::MFD_CLOEXEC) };

    if fd < 0 {
      return Ok(None);
    }

    // SAFETY: `fd` was just created and is owned here.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };

    // SAFETY: sizes the file this process owns.
    if unsafe { libc::ftruncate(owned.as_raw_fd(), length as libc::off_t) } != 0 {
      return Ok(None);
    }

    // SAFETY: maps the whole file, read and write, shared with the server.
    let memory = unsafe {
      libc::mmap(
        std::ptr::null_mut(),
        length,
        libc::PROT_READ | libc::PROT_WRITE,
        libc::MAP_SHARED,
        owned.as_raw_fd(),
        0,
      )
    };

    if memory == libc::MAP_FAILED {
      return Ok(None);
    }

    let seg = self.conn.generate_id()?;

    self.conn.shm_attach_fd(seg, owned, false)?;
    self.segment = Some(Segment {
      seg,
      memory: memory.cast(),
      length,
    });

    Ok(self.segment.as_ref())
  }

  /// Connects to the input method `ime::choose` names, where one answers.
  fn start_ime(&mut self) {
    let chosen = ime::choose();
    let inbox = crate::runtime::inbox();
    let post: ime::dbus::Post = Arc::new(move |event| inbox.send(Message::Ime(event)));
    let connected: Result<Box<dyn Ime>, Box<dyn Error>> = match chosen {
      "ibus" => ime::ibus::IBus::connect(post).map(|ime| Box::new(ime) as Box<dyn Ime>),
      "fcitx" => ime::fcitx::Fcitx::connect(post).map(|ime| Box::new(ime) as Box<dyn Ime>),
      "xim" => ime::xim::Xim::connect(Rc::clone(&self.conn), self.screen, self.window)
        .map(|ime| Box::new(ime) as Box<dyn Ime>),
      _ => Err("no input method".into()),
    };

    match connected {
      Ok(ime) => {
        trace::log(|| format!("[ime] {} connected", ime.name()));
        self.ime = Some(ime);
      }
      Err(error) => trace::log(|| format!("[ime] {chosen}: {error}")),
    }
  }

  /// Whether keys go to the input method first: one is connected and a text entry that
  /// shows what is typed has focus.
  const fn ime_routes(&self) -> bool {
    self.ime.is_some() && self.ime_focused
  }

  /// A key pressed or released: through the input method where it routes keys, to the
  /// window otherwise.
  fn key(&mut self, shell: &mut Shell, event: KeyPressEvent, release: bool, repeat: bool) {
    let keycode = u32::from(event.detail);

    if !self.ime_routes() {
      self.deliver(shell, keycode, release, repeat);

      return;
    }

    let key = RawKey {
      keycode,
      keysym: self.keyboard.keysym(keycode),
      state: u32::from(u16::from(event.state)),
      release,
      time: event.time,
      event,
    };
    let answers = self.ime.as_ref().is_some_and(|ime| ime.answers_keys());

    if !answers {
      let sent = self
        .ime
        .as_mut()
        .is_some_and(|ime| ime.process_key(0, &key));

      if !sent {
        self.deliver(shell, keycode, release, repeat);
      }

      return;
    }

    if self.ime_waiting.is_some() {
      self.ime_queue.push_back((key, repeat));
    } else {
      self.send_key(shell, key, repeat);
    }
  }

  fn send_key(&mut self, shell: &mut Shell, key: RawKey, repeat: bool) {
    let id = self.ime_next;

    self.ime_next += 1;

    let sent = self
      .ime
      .as_mut()
      .is_some_and(|ime| ime.process_key(id, &key));

    if sent {
      self.ime_waiting = Some(Waiting {
        id,
        key,
        repeat,
        sent: Instant::now(),
      });
    } else {
      self.deliver(shell, key.keycode, key.release, repeat);
    }
  }

  /// The window handles a key itself.
  fn deliver(&mut self, shell: &mut Shell, keycode: u32, release: bool, repeat: bool) {
    if let Some(input) = self.keyboard.key(keycode, !release, repeat) {
      shell.key(&input);
    }
  }

  /// The window handles a key the input method gave back, under the modifiers held when it
  /// was pressed.
  fn deliver_late(&mut self, shell: &mut Shell, key: &RawKey, repeat: bool) {
    if let Some(input) = self
      .keyboard
      .key_in(key.keycode, !key.release, repeat, Some(key.state))
    {
      shell.key(&input);
    }
  }

  /// Sends the next held key once the input method answered the last.
  fn next_key(&mut self, shell: &mut Shell) {
    while self.ime_waiting.is_none()
      && let Some((key, repeat)) = self.ime_queue.pop_front()
    {
      if self.ime_routes() {
        self.send_key(shell, key, repeat);
      } else {
        self.deliver_late(shell, &key, repeat);
      }
    }
  }

  /// Gives a key back to the window where the input method has not answered it in time.
  fn ime_timeout(&mut self, shell: &mut Shell) {
    if self
      .ime_waiting
      .as_ref()
      .is_some_and(|waiting| waiting.sent.elapsed() > IME_PATIENCE)
      && let Some(waiting) = self.ime_waiting.take()
    {
      trace::log(|| format!("[ime] key {} unanswered: handled here", waiting.id));
      self.deliver_late(shell, &waiting.key, waiting.repeat);
      self.next_key(shell);
    }
  }

  /// Applies what the input method said.
  fn ime_event(&mut self, shell: &mut Shell, event: ImeEvent) {
    trace::log(|| format!("[ime] {event:?}"));

    match event {
      ImeEvent::Commit(text) => shell.ime_commit(&text),
      ImeEvent::Preedit(text, cursor) => {
        if text.is_empty() {
          shell.ime_preedit("", None, Vec::new());
        } else {
          let cursor = cursor.map(|cursor| {
            let byte = ime::char_to_byte(&text, cursor as usize);

            byte..byte
          });

          shell.ime_preedit(&text, cursor, Vec::new());
        }
      }
      ImeEvent::DeleteSurrounding(offset, length) => {
        if let Some(state) = shell.ime_state() {
          let cursor = ime::byte_to_char(&state.surrounding, state.cursor) as i64;
          let start = (cursor + i64::from(offset)).max(0) as usize;
          let end = start + length as usize;
          let (start, end) = (
            ime::char_to_byte(&state.surrounding, start),
            ime::char_to_byte(&state.surrounding, end),
          );

          shell.ime_delete(start..end);
        }
      }
      ImeEvent::Forward {
        keysym,
        keycode,
        state,
        release,
      } => {
        let keycode = keycode.or_else(|| self.keyboard.keycode_for(keysym));

        if let Some(keycode) = keycode
          && let Some(input) = self.keyboard.key_in(keycode, !release, false, Some(state))
        {
          shell.key(&input);
        }
      }
      ImeEvent::KeyDone { id, handled } => {
        if self
          .ime_waiting
          .as_ref()
          .is_some_and(|waiting| waiting.id == id)
          && let Some(waiting) = self.ime_waiting.take()
        {
          if !handled {
            self.deliver_late(shell, &waiting.key, waiting.repeat);
          }

          self.next_key(shell);
        }
      }
      ImeEvent::RequireSurrounding => {
        self.ime_sent = None;
        self.ime_stale = true;
      }
      ImeEvent::Ready(_) | ImeEvent::Note(_) => {}
    }
  }

  /// Tells the input method about the focused entry: focus, its surrounding text and where
  /// its caret is.
  fn sync_ime(&mut self, shell: &Shell) {
    if crate::entry::take_ime_dirty().is_some() {
      self.ime_stale = true;
    }

    if !self.ime_stale || self.ime.is_none() {
      return;
    }

    self.ime_stale = false;

    let state = shell.ime_state().filter(|state| !state.secure);
    let scale = shell.scale;
    let (ox, oy) = shell.chrome.origin();
    let origin = self.window_origin;
    let Some(ime) = &mut self.ime else {
      return;
    };
    let Some(state) = state else {
      if self.ime_focused {
        ime.reset();
        ime.focus_out();
        self.ime_focused = false;
        self.ime_sent = None;
        self.ime_purpose = None;
      }

      return;
    };

    if !self.ime_focused {
      ime.focus_in();
      self.ime_focused = true;
    }

    if self.ime_purpose != Some(state.purpose) {
      ime.set_purpose(state.purpose);
      self.ime_purpose = Some(state.purpose);
    }

    let device = |value: f32, offset: f32| (f64::from(value + offset) * scale).round() as i32;
    let window = (
      device(state.rect.x, ox),
      device(state.rect.y, oy),
      ((f64::from(state.rect.width) * scale).round() as i32).max(1),
      ((f64::from(state.rect.height) * scale).round() as i32).max(1),
    );
    let cursor = ime::byte_to_char(&state.surrounding, state.cursor) as u32;
    let anchor = ime::byte_to_char(&state.surrounding, state.anchor) as u32;
    let sent = (state.surrounding.clone(), cursor, anchor, window);

    if self.ime_sent.as_ref() == Some(&sent) {
      return;
    }

    ime.set_surrounding(&state.surrounding, cursor, anchor);
    ime.set_cursor(
      (window.0 + origin.0, window.1 + origin.1, window.2, window.3),
      window,
    );
    self.ime_sent = Some(sent);
  }

  /// Learns where the window is on the screen: for the input method's candidates and the
  /// reader.
  fn locate_window(&mut self, shell: &Shell) {
    let Ok(reply) = self
      .conn
      .translate_coordinates(self.window, self.root, 0, 0)
      .map_err(|_| ())
      .and_then(|cookie| cookie.reply().map_err(|_| ()))
    else {
      return;
    };
    let origin = (i32::from(reply.dst_x), i32::from(reply.dst_y));

    if origin != self.window_origin {
      self.window_origin = origin;
      self.ime_sent = None;
      self.ime_stale = true;
    }

    let (ox, oy) = shell.chrome.origin();
    let (width, height) = shell.size;
    let rect = skene_foundation::layout::Rect {
      x: origin.0 as f32 + ox,
      y: origin.1 as f32 + oy,
      width: width as f32,
      height: height as f32,
    };

    crate::access::set_window_bounds(rect, rect);
  }

  /// Opens the open overlay's window at `placement` (window pixels), or closes it.
  fn place_popup(
    &mut self,
    shell: &mut Shell,
    placement: Option<crate::overlay::Placement>,
  ) -> Result<(), Box<dyn Error>> {
    if let Some(popup) = self.popup.take() {
      self.conn.ungrab_pointer(x11rb::CURRENT_TIME)?;
      self.conn.destroy_window(popup.window)?;
      self.conn.flush()?;
    }

    let Some(placement) = placement else {
      return Ok(());
    };

    self.locate_window(shell);

    let scale = shell.scale;
    let (ox, oy) = shell.chrome.origin();
    let anchor = placement.anchor;
    let device = |value: f32| (f64::from(value) * scale).round() as i32;
    let (width, height) = (
      (f64::from(placement.size.0) * scale).round() as u32,
      (f64::from(placement.size.1) * scale).round() as u32,
    );
    let screen = &self.conn.setup().roots[self.screen];
    let x = (self.window_origin.0 + device(anchor.x + ox))
      .clamp(0, (i32::from(screen.width_in_pixels) - width as i32).max(0));
    let below = self.window_origin.1 + device(anchor.y + anchor.height + 2. + oy);
    let y = if below + height as i32 > i32::from(screen.height_in_pixels) {
      (self.window_origin.1 + device(anchor.y + oy) - height as i32 - 2).max(0)
    } else {
      below
    };
    let window = self.conn.generate_id()?;

    self.conn.create_window(
      x11rb::COPY_DEPTH_FROM_PARENT,
      window,
      self.root,
      x as i16,
      y as i16,
      width as u16,
      height as u16,
      0,
      WindowClass::INPUT_OUTPUT,
      0,
      &CreateWindowAux::new()
        .override_redirect(1)
        .background_pixel(0)
        .event_mask(
          EventMask::EXPOSURE
            | EventMask::BUTTON_PRESS
            | EventMask::BUTTON_RELEASE
            | EventMask::POINTER_MOTION,
        ),
    )?;
    self.conn.map_window(window)?;
    self.conn.flush()?;

    let grabbed = self
      .conn
      .grab_pointer(
        false,
        window,
        EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
        x11rb::NONE,
        x11rb::NONE,
        x11rb::CURRENT_TIME,
      )?
      .reply()
      .map(|reply| reply.status);

    trace::log(|| format!("[x11] popup 0x{window:x} at {x},{y} {width}x{height} grab {grabbed:?}"));
    self.popup = Some(PopupWindow {
      window,
      size: (width, height),
    });
    shell.popup_placed(
      (f64::from(x - self.window_origin.0) / scale) as f32 - ox,
      (f64::from(y - self.window_origin.1) / scale) as f32 - oy,
    );

    Ok(())
  }

  /// Paints the open overlay where it changed.
  fn present_popup(&mut self, shell: &mut Shell) -> Result<(), Box<dyn Error>> {
    let Some(popup) = &self.popup else {
      return Ok(());
    };
    let window = popup.window;
    let Some(frame) = shell.popup_frame() else {
      return Ok(());
    };
    let pixmap = frame.pixmap;
    let (width, height) = (usize::from(pixmap.width()), usize::from(pixmap.height()));
    let mut bytes = Vec::with_capacity(width * height * 4);

    for pixel in pixmap.data() {
      bytes.extend_from_slice(&[pixel.b, pixel.g, pixel.r, 255]);
    }

    let row = width * 4;
    let rows = ((self.conn.maximum_request_bytes() - 64) / row.max(1)).clamp(1, height.max(1));
    let mut top = 0;

    while top < height {
      let count = rows.min(height - top);

      self.conn.put_image(
        ImageFormat::Z_PIXMAP,
        window,
        self.gc,
        width as u16,
        count as u16,
        0,
        top as i16,
        0,
        self.depth,
        &bytes[top * row..(top + count) * row],
      )?;
      top += count;
    }

    if let Some(list) = shell.popup() {
      trace::shot_popup(list.pixmap());
    }

    let _ = popup.size;

    Ok(())
  }

  /// Paints what changed and shows it.
  fn present(&mut self, shell: &mut Shell) -> Result<(), Box<dyn Error>> {
    let started = std::time::Instant::now();
    let Some(frame) = shell.frame() else {
      return Ok(());
    };
    let pixmap = frame.pixmap;
    let (width, height) = (u32::from(pixmap.width()), u32::from(pixmap.height()));
    let (dx, dy, dw, dh) = frame.damage;

    if dw == 0 || dh == 0 {
      return Ok(());
    }

    let stride = width as usize * 4;
    let pixels = pixmap.data();
    let copy = |target: &mut [u8]| {
      for row in dy..dy + dh {
        let start = row as usize * width as usize;

        for column in dx..dx + dw {
          let pixel = pixels[start + column as usize];
          let at = (start + column as usize) * 4;

          target[at..at + 4].copy_from_slice(&[pixel.b, pixel.g, pixel.r, 255]);
        }
      }
    };

    if self.shm {
      let length = stride * height as usize;

      if let Some(segment) = self.segment(length)? {
        // SAFETY: the mapping is `segment.length` bytes, at least `length`, and only this
        // thread writes it.
        let target = unsafe { std::slice::from_raw_parts_mut(segment.memory, length) };
        let seg = segment.seg;

        copy(target);
        self.conn.shm_put_image(
          self.window,
          self.gc,
          width as u16,
          height as u16,
          dx as u16,
          dy as u16,
          dw as u16,
          dh as u16,
          dx as i16,
          dy as i16,
          self.depth,
          ImageFormat::Z_PIXMAP.into(),
          false,
          seg,
          0,
        )?;
        // The server reads the segment before the next frame overwrites it.
        self.conn.sync()?;
        trace::log(|| {
          format!(
            "[x11] present shm {dw}x{dh}+{dx}+{dy} in {:.2} ms",
            started.elapsed().as_secs_f64() * 1000.
          )
        });
        trace::shot(pixmap);

        return Ok(());
      }

      self.shm = false;
    }

    self.buffer.resize(stride * height as usize, 0);

    let mut buffer = std::mem::take(&mut self.buffer);

    copy(&mut buffer);

    let row_bytes = dw as usize * 4;
    let rows_per_request =
      ((self.conn.maximum_request_bytes() - 64) / row_bytes).clamp(1, dh as usize);
    let mut band = Vec::with_capacity(row_bytes * rows_per_request);
    let mut row = dy as usize;

    while row < (dy + dh) as usize {
      let rows = rows_per_request.min((dy + dh) as usize - row);

      band.clear();

      for line in row..row + rows {
        let start = line * stride + dx as usize * 4;

        band.extend_from_slice(&buffer[start..start + row_bytes]);
      }

      self.conn.put_image(
        ImageFormat::Z_PIXMAP,
        self.window,
        self.gc,
        dw as u16,
        rows as u16,
        dx as i16,
        row as i16,
        0,
        self.depth,
        &band,
      )?;
      row += rows;
    }

    self.buffer = buffer;
    trace::log(|| {
      format!(
        "[x11] present put {dw}x{dh}+{dx}+{dy} in {:.2} ms",
        started.elapsed().as_secs_f64() * 1000.
      )
    });
    trace::shot(pixmap);

    Ok(())
  }
}

fn load_keymap(keyboard: &mut Keyboard, conn: &XCBConnection, device: i32) {
  let keymap = xkb::x11::keymap_new_from_device(
    &keyboard.context,
    conn,
    device,
    xkb::KEYMAP_COMPILE_NO_FLAGS,
  );
  let state = xkb::x11::state_new_from_device(&keymap, conn, device);

  keyboard.set_keymap(keymap, state);
}
