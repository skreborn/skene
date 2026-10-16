//! The Wayland and X11 input driver, out of process: what a user does, through each display
//! server's real input injection, at points of the app's window.
//!
//! ```text
//! drive <backend> <step>...
//! ```
//!
//! It refuses to run outside a private session (`SKENE_PRIVATE_SESSION`, which the session
//! scripts set to their own runtime directory).
//!
//! Backends:
//! - `x11`: XTest on `$DISPLAY`.
//! - `mutter-dbus`: org.gnome.Mutter.RemoteDesktop, keysyms and absolute motion on a
//!   ScreenCast stream (`STREAM=window|monitor`, PipeWire running).
//! - `mutter-ei`: the same session's ConnectToEIS, through reis (libei's protocol in Rust).
//! - `sway`: zwlr_virtual_pointer_v1 and zwp_virtual_keyboard_v1 on `$WAYLAND_DISPLAY`.
//!
//! The window's position in the compositor's pointer space is unknown to a Wayland client;
//! `calibrate` moves the pointer to a point, reads where the app saw it (`$SKENE_PROBE`, in the
//! window's geometry), and keeps the difference as the window's origin.
//!
//! Steps (points in the window's geometry): `calibrate`, `recalibrate`, `move X Y`,
//! `press [BUTTON]`, `release [BUTTON]`, `click-at X Y`, `right-click-at X Y`,
//! `double-click-at X Y`, `triple-click-at X Y`, `middle-click-at X Y`, `drag X0 Y0 X1 Y1`, `wheel X Y DX DY` (detents), `type TEXT`,
//! `run COMMAND` (a shell command, between steps), `key KEYSYM`, `keydown KEYSYM`, `keyup KEYSYM`, `keycode EVDEV`, `chord A+B`, `sleep MS`, `origin`.

use std::{
  collections::HashMap,
  os::fd::{AsFd as _, FromRawFd as _, OwnedFd},
  path::PathBuf,
  thread::sleep,
  time::{Duration, Instant},
};

use xkbcommon::xkb;
use zbus::{
  blocking::Connection,
  zvariant::{OwnedObjectPath, OwnedValue, Value},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// --- keys ---

/// Keysym -> (evdev keycode, needs Shift) for a keymap's first layout.
fn key_lookup(keymap: &xkb::Keymap) -> HashMap<u32, (u32, bool)> {
  let mut map = HashMap::new();

  for code in keymap.min_keycode().raw()..=keymap.max_keycode().raw() {
    let keycode = xkb::Keycode::new(code);

    for level in 0..2 {
      for sym in keymap.key_get_syms_by_level(keycode, 0, level) {
        map.entry(sym.raw()).or_insert((code - 8, level == 1));
      }
    }
  }

  map
}

const SHIFT: u32 = 42;

// --- input backends ---

trait Input {
  /// Moves the pointer to `(x, y)` in the backend's pointer space.
  fn move_to(&mut self, x: f64, y: f64) -> Result<()>;
  /// Presses or releases an evdev button (0x110 left, 0x111 right).
  fn button(&mut self, button: u32, down: bool) -> Result<()>;
  /// Presses and releases the key typing `keysym` (with Shift where the keymap needs it).
  fn keysym(&mut self, keysym: u32) -> Result<()>;
  /// Presses or releases the key of `keysym` alone (for chords).
  fn key_state(&mut self, keysym: u32, down: bool) -> Result<()>;
  /// The pointer space's origin of the window's geometry, where the backend knows it.
  fn window_origin(&mut self) -> Option<(f64, f64)> {
    None
  }
  /// A point inside the pointer space to calibrate at.
  fn center(&self) -> (f64, f64);
  /// Waits until the window system is done placing the window, where it says so.
  fn wait_placed(&mut self) -> Result<()> {
    Ok(())
  }
  /// Turns the wheel by detents, horizontally and vertically (down and right positive).
  fn wheel(&mut self, dx: i32, dy: i32) -> Result<()>;
  /// Presses and releases the key of evdev `code`, whatever the layout puts there.
  fn keycode(&mut self, code: u32) -> Result<()> {
    let _ = code;

    Err("raw keycodes are not supported on this backend".into())
  }
}

struct X11 {
  conn: x11rb::rust_connection::RustConnection,
  root: u32,
  keys: HashMap<u32, (u32, bool)>,
}

impl X11 {
  fn new() -> Result<Self> {
    use x11rb::{connection::Connection as _, protocol::xproto::ConnectionExt as _};

    let (conn, screen) = x11rb::connect(None)?;
    let setup = conn.setup();
    let root = setup.roots[screen].root;
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let map = conn.get_keyboard_mapping(min, max - min + 1)?.reply()?;
    let per = usize::from(map.keysyms_per_keycode);
    let mut keys = HashMap::new();

    for (index, syms) in map.keysyms.chunks(per).enumerate() {
      for (level, sym) in syms.iter().take(2).enumerate() {
        if *sym != 0 {
          keys
            .entry(*sym)
            .or_insert((u32::from(min) + index as u32 - 8, level == 1));
        }
      }
    }

    Ok(Self { conn, root, keys })
  }

  fn fake(&self, kind: u8, detail: u8, x: i16, y: i16) -> Result<()> {
    use x11rb::{connection::Connection as _, protocol::xtest::ConnectionExt as _};

    self
      .conn
      .xtest_fake_input(kind, detail, 0, self.root, x, y, 0)?;
    self.conn.flush()?;

    Ok(())
  }

  fn sync(&self) -> Result<()> {
    use x11rb::protocol::xproto::ConnectionExt as _;

    self.conn.get_input_focus()?.reply()?;

    Ok(())
  }
}

impl Input for X11 {
  fn move_to(&mut self, x: f64, y: f64) -> Result<()> {
    self.fake(6, 0, x as i16, y as i16)?;
    self.sync()
  }

  fn button(&mut self, button: u32, down: bool) -> Result<()> {
    let detail = match button {
      0x111 => 3,
      0x112 => 2,
      _ => 1,
    };

    self.fake(if down { 4 } else { 5 }, detail, 0, 0)?;
    self.sync()
  }

  fn keysym(&mut self, keysym: u32) -> Result<()> {
    let (code, shift) = *self.keys.get(&keysym).ok_or("keysym not in the keymap")?;
    let (code, shift_code) = ((code + 8) as u8, (SHIFT + 8) as u8);

    if shift {
      self.fake(2, shift_code, 0, 0)?;
    }

    self.fake(2, code, 0, 0)?;
    self.fake(3, code, 0, 0)?;

    if shift {
      self.fake(3, shift_code, 0, 0)?;
    }

    self.sync()
  }

  fn key_state(&mut self, keysym: u32, down: bool) -> Result<()> {
    let (code, _) = *self.keys.get(&keysym).ok_or("keysym not in the keymap")?;

    self.fake(if down { 2 } else { 3 }, (code + 8) as u8, 0, 0)?;
    self.sync()
  }

  fn window_origin(&mut self) -> Option<(f64, f64)> {
    Some((0.0, 0.0))
  }

  fn keycode(&mut self, code: u32) -> Result<()> {
    self.fake(2, (code + 8) as u8, 0, 0)?;
    self.fake(3, (code + 8) as u8, 0, 0)?;
    self.sync()
  }

  fn wheel(&mut self, dx: i32, dy: i32) -> Result<()> {
    for (count, negative, positive) in [(dy, 4, 5), (dx, 6, 7)] {
      let detail = if count < 0 { negative } else { positive };

      for _ in 0..count.unsigned_abs() {
        self.fake(4, detail, 0, 0)?;
        self.fake(5, detail, 0, 0)?;
      }
    }

    self.sync()
  }

  /// Under a window manager (`_NET_SUPPORTING_WM_CHECK` on the root), waits until it has
  /// framed the app's window (`_NET_FRAME_EXTENTS`) and focused it (`_NET_WM_STATE_FOCUSED`).
  fn wait_placed(&mut self) -> Result<()> {
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

    let atom = |name: &str| -> Result<u32> {
      Ok(self.conn.intern_atom(false, name.as_bytes())?.reply()?.atom)
    };
    let check = atom("_NET_SUPPORTING_WM_CHECK")?;
    let clients = atom("_NET_CLIENT_LIST")?;
    let extents = atom("_NET_FRAME_EXTENTS")?;
    let state = atom("_NET_WM_STATE")?;
    let focused = atom("_NET_WM_STATE_FOCUSED")?;

    if self
      .conn
      .get_property(false, self.root, check, AtomEnum::WINDOW, 0, 1)?
      .reply()?
      .value_len
      == 0
    {
      return Ok(());
    }

    let started = Instant::now();

    loop {
      let windows: Vec<u32> = self
        .conn
        .get_property(false, self.root, clients, AtomEnum::WINDOW, 0, 256)?
        .reply()?
        .value32()
        .map(Iterator::collect)
        .unwrap_or_default();

      for window in windows {
        let class = self
          .conn
          .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 64)?
          .reply()?;

        if !class.value.starts_with(b"dev.skene") {
          continue;
        }

        let framed = self
          .conn
          .get_property(false, window, extents, AtomEnum::CARDINAL, 0, 4)?
          .reply()?
          .value_len
          == 4;
        let is_focused = self
          .conn
          .get_property(false, window, state, AtomEnum::ATOM, 0, 32)?
          .reply()?
          .value32()
          .is_some_and(|mut atoms| atoms.any(|atom| atom == focused));

        if framed && is_focused {
          eprintln!(
            "[drive] x11: framed and focused after {:?}",
            started.elapsed()
          );

          return Ok(());
        }
      }

      if started.elapsed() > Duration::from_secs(3) {
        return Err("the window manager did not frame and focus the window".into());
      }

      sleep(Duration::from_millis(5));
    }
  }

  fn center(&self) -> (f64, f64) {
    (512.0, 384.0)
  }
}

/// mutter's RemoteDesktop session, with a ScreenCast stream for absolute motion.
struct Mutter {
  conn: Connection,
  session: OwnedObjectPath,
  stream: Option<OwnedObjectPath>,
  stream_size: (f64, f64),
  stream_mapping: Option<String>,
  #[expect(dead_code, reason = "kept for a window stream's mapping")]
  window_stream: bool,
}

const RD: &str = "org.gnome.Mutter.RemoteDesktop";
const SC: &str = "org.gnome.Mutter.ScreenCast";

impl Mutter {
  fn new_for_ei(stream: &str) -> Result<Self> {
    Self::create(stream, true)
  }

  fn new(stream: &str) -> Result<Self> {
    Self::create(stream, false)
  }

  fn create(kind: &str, ei: bool) -> Result<Self> {
    let conn = Connection::session()?;
    let reply = conn.call_method(
      Some(RD),
      "/org/gnome/Mutter/RemoteDesktop",
      Some(RD),
      "CreateSession",
      &(),
    )?;
    let session: OwnedObjectPath = reply.body().deserialize()?;
    let id: OwnedValue = conn
      .call_method(
        Some(RD),
        session.as_str(),
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &(format!("{RD}.Session"), "SessionId"),
      )?
      .body()
      .deserialize()?;
    let id = String::try_from(id)?;
    let mut properties: HashMap<&str, Value> = HashMap::new();

    properties.insert("remote-desktop-session-id", Value::from(id.as_str()));

    let screencast: OwnedObjectPath = conn
      .call_method(
        Some(SC),
        "/org/gnome/Mutter/ScreenCast",
        Some(SC),
        "CreateSession",
        &(properties,),
      )?
      .body()
      .deserialize()?;
    let window_stream = kind == "window";
    let mut options: HashMap<&str, Value> = HashMap::new();

    options.insert("cursor-mode", Value::from(0u32));

    let stream: OwnedObjectPath = if window_stream {
      conn
        .call_method(
          Some(SC),
          screencast.as_str(),
          Some(&*format!("{SC}.Session")),
          "RecordWindow",
          &(options,),
        )?
        .body()
        .deserialize()?
    } else {
      conn
        .call_method(
          Some(SC),
          screencast.as_str(),
          Some(&*format!("{SC}.Session")),
          "RecordMonitor",
          &("", options),
        )?
        .body()
        .deserialize()?
    };

    conn.call_method(
      Some(RD),
      session.as_str(),
      Some(&*format!("{RD}.Session")),
      "Start",
      &(),
    )?;

    // The stream's parameters arrive once PipeWire has it.
    let started = Instant::now();
    let (size, mapping) = loop {
      let parameters: Result<HashMap<String, OwnedValue>> = conn
        .call_method(
          Some(SC),
          stream.as_str(),
          Some("org.freedesktop.DBus.Properties"),
          "Get",
          &(format!("{SC}.Stream"), "Parameters"),
        )
        .map_err(Into::into)
        .and_then(|reply| {
          let value: OwnedValue = reply.body().deserialize()?;

          Ok(HashMap::<String, OwnedValue>::try_from(value)?)
        });

      if let Ok(parameters) = parameters
        && let Some(size) = parameters.get("size")
      {
        let (width, height) = <(i32, i32)>::try_from(size.try_clone()?)?;
        let mapping = parameters
          .get("mapping-id")
          .and_then(|value| String::try_from(value.try_clone().ok()?).ok());

        break ((f64::from(width), f64::from(height)), mapping);
      }

      if started.elapsed() > Duration::from_secs(5) {
        return Err("the ScreenCast stream reported no size".into());
      }

      sleep(Duration::from_millis(20));
    };

    eprintln!(
      "[drive] mutter session {} stream {} ({kind}) size {size:?} mapping {mapping:?}",
      session.as_str(),
      stream.as_str()
    );

    // mutter creates the virtual keyboard on the first keysym, and the seat gains its
    // keyboard then: that first key reaches no surface. A throwaway Shift creates it.
    if std::env::var_os("NO_WARMUP").is_none() && !ei {
      for down in [true, false] {
        conn.call_method(
          Some(RD),
          session.as_str(),
          Some(&*format!("{RD}.Session")),
          "NotifyKeyboardKeysym",
          &(0xffe1u32, down),
        )?;
      }

      sleep(Duration::from_millis(100));
    }

    Ok(Self {
      conn,
      session,
      stream: Some(stream),
      stream_size: size,
      stream_mapping: mapping,
      window_stream,
    })
  }

  fn session_call<B>(&self, method: &str, body: &B) -> Result<()>
  where
    B: serde::Serialize + zbus::zvariant::DynamicType,
  {
    self.conn.call_method(
      Some(RD),
      self.session.as_str(),
      Some(&*format!("{RD}.Session")),
      method,
      body,
    )?;

    Ok(())
  }

  /// The EIS socket of this session.
  fn connect_to_eis(&self) -> Result<OwnedFd> {
    let options: HashMap<&str, Value> = HashMap::new();
    let reply = self.conn.call_method(
      Some(RD),
      self.session.as_str(),
      Some(&*format!("{RD}.Session")),
      "ConnectToEIS",
      &(options,),
    )?;
    let fd: zbus::zvariant::OwnedFd = reply.body().deserialize()?;

    Ok(fd.into())
  }
}

impl Input for Mutter {
  fn move_to(&mut self, x: f64, y: f64) -> Result<()> {
    let stream = self.stream.clone().ok_or("no stream")?;

    self.session_call("NotifyPointerMotionAbsolute", &(stream.as_str(), x, y))
  }

  fn button(&mut self, button: u32, down: bool) -> Result<()> {
    self.session_call("NotifyPointerButton", &(button as i32, down))
  }

  fn keysym(&mut self, keysym: u32) -> Result<()> {
    self.session_call("NotifyKeyboardKeysym", &(keysym, true))?;
    self.session_call("NotifyKeyboardKeysym", &(keysym, false))
  }

  fn key_state(&mut self, keysym: u32, down: bool) -> Result<()> {
    self.session_call("NotifyKeyboardKeysym", &(keysym, down))
  }

  fn wheel(&mut self, dx: i32, dy: i32) -> Result<()> {
    if dy != 0 {
      self.session_call("NotifyPointerAxisDiscrete", &(0u32, dy))?;
    }

    if dx != 0 {
      self.session_call("NotifyPointerAxisDiscrete", &(1u32, dx))?;
    }

    Ok(())
  }

  fn window_origin(&mut self) -> Option<(f64, f64)> {
    None
  }

  fn center(&self) -> (f64, f64) {
    (self.stream_size.0 / 2.0, self.stream_size.1 / 2.0)
  }
}

/// The libei client of mutter's EIS, its events pumped on this thread.
struct Ei {
  /// Keeps the RemoteDesktop session alive.
  _mutter: Mutter,
  /// The libei context.
  context: reis::ei::Context,
  /// The events' converter.
  converter: reis::event::EiEventConverter,
  /// The devices offered.
  devices: EiDevices,
  /// Where each keysym is: its keycode and whether Shift reaches it.
  keys: HashMap<u32, (u32, bool)>,
  /// The absolute region aimed in, global logical coordinates.
  region: (f64, f64, f64, f64),
  /// When the client started: the events' timestamps count from it.
  started: Instant,
  /// The last emulation sequence number.
  sequence: u32,
}

#[derive(Default)]
struct EiDevices {
  absolute: Option<reis::event::Device>,
  keyboard: Option<reis::event::Device>,
  resumed: Vec<reis::event::Device>,
  log: Vec<String>,
}

impl Ei {
  fn new(stream_kind: &str) -> Result<Self> {
    let mutter = Mutter::new_for_ei(stream_kind)?;
    let fd = mutter.connect_to_eis()?;
    let stream = std::os::unix::net::UnixStream::from(fd);
    let context = reis::ei::Context::new(stream)?;
    let response = reis::handshake::ei_handshake_blocking(
      &context,
      "drive",
      reis::ei::handshake::ContextType::Sender,
    )?;
    let converter = reis::event::EiEventConverter::new(&context, response);
    let mut ei = Self {
      _mutter: mutter,
      context,
      converter,
      devices: EiDevices::default(),
      keys: HashMap::new(),
      region: (0.0, 0.0, 0.0, 0.0),
      started: Instant::now(),
      sequence: 0,
    };
    let started = Instant::now();

    loop {
      ei.pump(20)?;

      let devices = &ei.devices;
      let ready = |device: &Option<reis::event::Device>| {
        device
          .as_ref()
          .is_some_and(|device| devices.resumed.contains(device))
      };

      if ready(&devices.absolute) && ready(&devices.keyboard) {
        break;
      }

      if started.elapsed() > Duration::from_secs(5) {
        return Err(format!("EIS devices not ready: {:?}", devices.log).into());
      }
    }

    for line in &ei.devices.log {
      eprintln!("[drive] ei {line}");
    }

    let keyboard = ei.devices.keyboard.clone().unwrap();
    let keymap = keyboard.keymap().ok_or("no keymap")?;
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    // SAFETY: the EIS server's keymap fd, mapped read-only for its size.
    let keymap = unsafe {
      xkb::Keymap::new_from_fd(
        &context,
        keymap.fd.try_clone()?,
        keymap.size as usize,
        xkb::KEYMAP_FORMAT_TEXT_V1,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
      )?
    }
    .ok_or("keymap")?;
    let absolute = ei.devices.absolute.clone().unwrap();
    let regions = absolute.regions();
    let region = regions
      .iter()
      .find(|region| region.mapping_id.is_some() && region.mapping_id == ei._mutter.stream_mapping)
      .or_else(|| regions.first())
      .ok_or("no region")?;

    ei.keys = key_lookup(&keymap);
    ei.region = (
      f64::from(region.x),
      f64::from(region.y),
      f64::from(region.width),
      f64::from(region.height),
    );
    eprintln!("[drive] ei regions {regions:?}, aiming in {:?}", ei.region);

    ei.context.flush()?;

    Ok(ei)
  }

  /// Reads and handles what EIS sent, waiting up to `timeout_ms` for something to arrive.
  fn pump(&mut self, timeout_ms: i32) -> Result<()> {
    let mut fds = [libc::pollfd {
      fd: std::os::fd::AsRawFd::as_raw_fd(&self.context),
      events: libc::POLLIN,
      revents: 0,
    }];

    // SAFETY: polls one live descriptor.
    if unsafe { libc::poll(fds.as_mut_ptr(), 1, timeout_ms) } > 0 {
      self.context.read()?;
    }

    while let Some(result) = self.context.pending_event() {
      if let reis::PendingRequestResult::Request(request) = result {
        self.converter.handle_event(request)?;
      }
    }

    while let Some(event) = self.converter.next_event() {
      let devices = &mut self.devices;
      let sequence = &mut self.sequence;
      let serial = self.converter.connection().serial();

      match &event {
        reis::event::EiEvent::SeatAdded(added) => {
          added.seat.bind_capabilities(
            reis::event::DeviceCapability::PointerAbsolute
              | reis::event::DeviceCapability::Keyboard
              | reis::event::DeviceCapability::Button
              | reis::event::DeviceCapability::Pointer,
          );
          devices.log.push("seat added, bound".into());
        }
        reis::event::EiEvent::DeviceAdded(added) => {
          let device = &added.device;

          devices.log.push(format!(
            "device {:?} type {:?} regions {:?} keymap {}",
            device.name(),
            device.device_type(),
            device.regions(),
            device.keymap().is_some()
          ));

          if device.has_capability(reis::event::DeviceCapability::PointerAbsolute) {
            devices.absolute = Some(device.clone());
          }

          if device.has_capability(reis::event::DeviceCapability::Keyboard) {
            devices.keyboard = Some(device.clone());
          }
        }
        reis::event::EiEvent::DeviceResumed(resumed) => {
          devices
            .log
            .push(format!("resumed {:?}", resumed.device.name()));
          devices.resumed.push(resumed.device.clone());
          *sequence += 1;
          resumed.device.device().start_emulating(serial, *sequence);
        }
        reis::event::EiEvent::DeviceRemoved(removed) => {
          devices
            .log
            .push(format!("removed {:?}", removed.device.name()));
          devices.resumed.retain(|device| device != &removed.device);

          if devices.absolute.as_ref() == Some(&removed.device) {
            devices.absolute = None;
          }

          if devices.keyboard.as_ref() == Some(&removed.device) {
            devices.keyboard = None;
          }
        }
        reis::event::EiEvent::DevicePaused(paused) => {
          devices
            .log
            .push(format!("paused {:?}", paused.device.name()));
        }
        _ => {}
      }
    }

    self.context.flush()?;

    Ok(())
  }

  fn device(&self, absolute: bool) -> reis::event::Device {
    if absolute {
      self.devices.absolute.clone().unwrap()
    } else {
      self.devices.keyboard.clone().unwrap()
    }
  }

  fn frame(&self, device: &reis::event::Device) -> Result<()> {
    device.device().frame(
      self.converter.connection().serial(),
      self.started.elapsed().as_micros() as u64,
    );
    self.context.flush()?;

    Ok(())
  }

  /// Handles anything EIS sent meanwhile (pings, pauses).
  fn sync(&mut self) -> Result<()> {
    self.pump(0)
  }
}

impl Input for Ei {
  fn wheel(&mut self, dx: i32, dy: i32) -> Result<()> {
    let device = self.device(true);
    let scroll = device.interface::<reis::ei::Scroll>().ok_or("no scroll")?;

    scroll.scroll_discrete(dx * 120, dy * 120);
    self.frame(&device)?;
    self.sync()
  }

  fn move_to(&mut self, x: f64, y: f64) -> Result<()> {
    self.pump(0)?;

    let device = self.device(true);
    let pointer = device
      .interface::<reis::ei::PointerAbsolute>()
      .ok_or("no absolute pointer")?;

    pointer.motion_absolute((self.region.0 + x) as f32, (self.region.1 + y) as f32);
    self.frame(&device)?;
    self.sync()
  }

  fn button(&mut self, button: u32, down: bool) -> Result<()> {
    let device = self.device(true);
    let buttons = device.interface::<reis::ei::Button>().ok_or("no buttons")?;

    buttons.button(
      button,
      if down {
        reis::ei::button::ButtonState::Press
      } else {
        reis::ei::button::ButtonState::Released
      },
    );
    self.frame(&device)?;
    self.sync()
  }

  fn keysym(&mut self, keysym: u32) -> Result<()> {
    let (code, shift) = *self
      .keys
      .get(&keysym)
      .ok_or("keysym not in the EIS keymap")?;
    let device = self.device(false);
    let keyboard = device
      .interface::<reis::ei::Keyboard>()
      .ok_or("no keyboard")?;
    let key = |code: u32, down: bool| -> Result<()> {
      keyboard.key(
        code,
        if down {
          reis::ei::keyboard::KeyState::Press
        } else {
          reis::ei::keyboard::KeyState::Released
        },
      );
      self.frame(&device)
    };

    if shift {
      key(SHIFT, true)?;
    }

    key(code, true)?;
    key(code, false)?;

    if shift {
      key(SHIFT, false)?;
    }

    self.sync()
  }

  fn key_state(&mut self, keysym: u32, down: bool) -> Result<()> {
    let (code, _) = *self
      .keys
      .get(&keysym)
      .ok_or("keysym not in the EIS keymap")?;
    let device = self.device(false);
    let keyboard = device
      .interface::<reis::ei::Keyboard>()
      .ok_or("no keyboard")?;

    keyboard.key(
      code,
      if down {
        reis::ei::keyboard::KeyState::Press
      } else {
        reis::ei::keyboard::KeyState::Released
      },
    );
    self.frame(&device)?;
    self.sync()
  }

  fn center(&self) -> (f64, f64) {
    (self.region.2 / 2.0, self.region.3 / 2.0)
  }
}

/// sway's virtual pointer and keyboard, kept for the whole run.
struct Sway {
  #[expect(dead_code, reason = "the connection the virtual devices live on")]
  conn: wayland_client::Connection,
  queue: wayland_client::EventQueue<WlState>,
  pointer: wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
  keyboard: wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
  keys: HashMap<u32, (u32, bool)>,
  extent: (u32, u32),
  started: Instant,
  modifiers: u32,
  _keymap_file: std::fs::File,
}

struct WlState;

impl
  wayland_client::Dispatch<
    wayland_client::protocol::wl_registry::WlRegistry,
    wayland_client::globals::GlobalListContents,
  > for WlState
{
  fn event(
    _: &mut Self,
    _: &wayland_client::protocol::wl_registry::WlRegistry,
    _: wayland_client::protocol::wl_registry::Event,
    _: &wayland_client::globals::GlobalListContents,
    _: &wayland_client::Connection,
    _: &wayland_client::QueueHandle<Self>,
  ) {
  }
}

wayland_client::delegate_noop!(WlState: ignore wayland_client::protocol::wl_seat::WlSeat);
wayland_client::delegate_noop!(WlState: ignore wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1);
wayland_client::delegate_noop!(WlState: ignore wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1);
wayland_client::delegate_noop!(WlState: ignore wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1);
wayland_client::delegate_noop!(WlState: ignore wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1);

impl Sway {
  fn new() -> Result<Self> {
    use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
    use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;

    let conn = wayland_client::Connection::connect_to_env()?;
    let (globals, mut queue) = wayland_client::globals::registry_queue_init::<WlState>(&conn)?;
    let qh = queue.handle();
    let seat: wayland_client::protocol::wl_seat::WlSeat = globals.bind(&qh, 1..=1, ())?;
    let pointers: ZwlrVirtualPointerManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let keyboards: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let pointer = pointers.create_virtual_pointer(Some(&seat), &qh, ());
    let keyboard = keyboards.create_virtual_keyboard(&seat, &qh, ());
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let keymap =
      xkb::Keymap::new_from_names(&context, "", "", "us", "", None, 0).ok_or("keymap")?;
    let text = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
    let bytes = [text.as_bytes(), b"\0"].concat();
    // SAFETY: an anonymous file for the keymap, owned by `file` from here on.
    let file = unsafe {
      std::fs::File::from_raw_fd(libc::memfd_create(c"keymap".as_ptr(), libc::MFD_CLOEXEC))
    };

    std::io::Write::write_all(&mut &file, &bytes)?;
    keyboard.keymap(1, file.as_fd(), bytes.len() as u32);
    queue.roundtrip(&mut WlState)?;

    let extent = std::env::var("EXTENT")
      .ok()
      .and_then(|value| {
        let (w, h) = value.split_once('x')?;

        Some((w.parse().ok()?, h.parse().ok()?))
      })
      .or_else(|| {
        // The output's logical rectangle: what absolute motion spans.
        let output = std::process::Command::new("swaymsg")
          .args(["-t", "get_outputs", "-r"])
          .output()
          .ok()?;
        let outputs: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        let rect = &outputs.get(0)?["rect"];

        Some((
          rect["width"].as_u64()? as u32,
          rect["height"].as_u64()? as u32,
        ))
      })
      .unwrap_or((1024, 768));

    Ok(Self {
      conn,
      queue,
      pointer,
      keyboard,
      keys: key_lookup(&keymap),
      extent,
      modifiers: 0,
      started: Instant::now(),
      _keymap_file: file,
    })
  }

  fn time(&self) -> u32 {
    self.started.elapsed().as_millis() as u32
  }

  fn sync(&mut self) -> Result<()> {
    self.queue.roundtrip(&mut WlState)?;

    Ok(())
  }

  fn key(&mut self, code: u32, down: bool, shift: bool) {
    let time = self.time();

    self.keyboard.key(time, code, u32::from(down));
    self.keyboard.modifiers(u32::from(shift), 0, 0, 0);
  }
}

impl Input for Sway {
  fn keycode(&mut self, code: u32) -> Result<()> {
    self.key(code, true, false);
    self.key(code, false, false);
    self.sync()
  }

  fn wheel(&mut self, dx: i32, dy: i32) -> Result<()> {
    use wayland_client::protocol::wl_pointer::{Axis, AxisSource};

    let time = self.time();

    self.pointer.axis_source(AxisSource::Wheel);

    for (axis, count) in [(Axis::VerticalScroll, dy), (Axis::HorizontalScroll, dx)] {
      if count != 0 {
        self
          .pointer
          .axis_discrete(time, axis, f64::from(count) * 15.0, count);
      }
    }

    self.pointer.frame();
    self.sync()
  }

  fn move_to(&mut self, x: f64, y: f64) -> Result<()> {
    let time = self.time();

    self.pointer.motion_absolute(
      time,
      x.max(0.0) as u32,
      y.max(0.0) as u32,
      self.extent.0,
      self.extent.1,
    );
    self.pointer.frame();
    self.sync()
  }

  fn button(&mut self, button: u32, down: bool) -> Result<()> {
    use wayland_client::protocol::wl_pointer::ButtonState;

    let time = self.time();

    self.pointer.button(
      time,
      button,
      if down {
        ButtonState::Pressed
      } else {
        ButtonState::Released
      },
    );
    self.pointer.frame();
    self.sync()
  }

  fn keysym(&mut self, keysym: u32) -> Result<()> {
    let (code, shift) = *self.keys.get(&keysym).ok_or("keysym not in the keymap")?;

    if shift {
      self.key(SHIFT, true, true);
    }

    self.key(code, true, shift);
    self.key(code, false, shift);

    if shift {
      self.key(SHIFT, false, false);
    }

    self.sync()
  }

  fn key_state(&mut self, keysym: u32, down: bool) -> Result<()> {
    let (code, _) = *self.keys.get(&keysym).ok_or("keysym not in the keymap")?;
    let time = self.time();

    self.keyboard.key(time, code, u32::from(down));

    // The virtual keyboard states its modifiers itself: Shift, Control, Alt (Mod1) and Super
    // (Mod4) as the evdev keymap's real modifiers.
    let bit = match keysym {
      0xffe1 | 0xffe2 => 0x1,
      0xffe3 | 0xffe4 => 0x4,
      0xffe9 | 0xffea => 0x8,
      0xffeb | 0xffec => 0x40,
      _ => 0,
    };

    if bit != 0 {
      if down {
        self.modifiers |= bit;
      } else {
        self.modifiers &= !bit;
      }

      self.keyboard.modifiers(self.modifiers, 0, 0, 0);
    }

    self.sync()
  }

  fn window_origin(&mut self) -> Option<(f64, f64)> {
    let output = std::process::Command::new("swaymsg")
      .args(["-t", "get_tree", "-r"])
      .output()
      .ok()?;
    let tree: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let mut stack = vec![&tree];

    while let Some(node) = stack.pop() {
      if node["app_id"]
        .as_str()
        .is_some_and(|id| id.starts_with("dev.skene"))
      {
        let rect = &node["rect"];
        let window = &node["window_rect"];

        eprintln!(
          "[drive] sway node rect {rect} window_rect {window} geometry {}",
          node["geometry"]
        );

        return Some((
          rect["x"].as_f64()? + window["x"].as_f64()?,
          rect["y"].as_f64()? + window["y"].as_f64()?,
        ));
      }

      for key in ["nodes", "floating_nodes"] {
        if let Some(children) = node[key].as_array() {
          stack.extend(children);
        }
      }
    }

    None
  }

  fn center(&self) -> (f64, f64) {
    (
      f64::from(self.extent.0) / 2.0,
      f64::from(self.extent.1) / 2.0,
    )
  }
}

// --- the driver ---

struct Driver {
  /// The input injected.
  input: Box<dyn Input>,
  /// The backend's name.
  backend: String,
  /// The probe's report file, where one is read.
  probe: Option<PathBuf>,
  /// The window geometry's origin in the pointer space.
  origin: Option<(f64, f64)>,
}

impl Driver {
  fn read_probe(&self) -> Option<(f64, f64)> {
    let text = std::fs::read_to_string(self.probe.as_ref()?).ok()?;
    let mut parts = text.split_whitespace().map(str::parse::<f64>);

    Some((parts.next()?.ok()?, parts.next()?.ok()?))
  }

  /// Moves to `point` (pointer space) and waits until the app saw it; returns where.
  fn move_and_see(&mut self, point: (f64, f64)) -> Result<Option<(f64, f64)>> {
    if let Some(path) = &self.probe {
      let _ = std::fs::remove_file(path);
    }

    self.input.move_to(point.0, point.1)?;

    let started = Instant::now();

    while started.elapsed() < Duration::from_millis(300) {
      if let Some(seen) = self.read_probe() {
        return Ok(Some(seen));
      }

      sleep(Duration::from_millis(1));
    }

    Ok(None)
  }

  /// Finds the window's origin: by moving the pointer and reading where the app saw it, else
  /// where the backend knows it.
  fn calibrate(&mut self) -> Result<(f64, f64)> {
    self.input.wait_placed()?;

    let known = self.input.window_origin();
    let center = self.input.center();
    let mut tried = vec![center];

    for dx in [-0.25, 0.25] {
      for dy in [-0.25, 0.25] {
        tried.push((center.0 * (1.0 + 2.0 * dx), center.1 * (1.0 + 2.0 * dy)));
      }
    }

    tried.push((40.0, 40.0));

    let mut measured = None;

    if self.probe.is_some() {
      for point in tried {
        if let Some(seen) = self.move_and_see(point)? {
          let second = (point.0 + 7.0, point.1 + 5.0);
          let seen2 = self.move_and_see(second)?.unwrap_or_default();

          eprintln!(
            "[drive] calibrate: pointer {point:?} seen at {seen:?}; {second:?} seen at {seen2:?} (delta {:.2},{:.2})",
            seen2.0 - seen.0,
            seen2.1 - seen.1
          );
          measured = Some((point.0 - seen.0, point.1 - seen.1));

          break;
        }
      }
    }

    println!(
      "calibrate {}: backend-known origin {known:?}, measured origin {measured:?}",
      self.backend
    );

    let origin = measured.or(known).ok_or("no origin")?;

    self.origin = Some(origin);

    Ok(origin)
  }

  fn pointer_at(&mut self, x: f64, y: f64) -> Result<(f64, f64)> {
    let origin = match self.origin {
      Some(origin) => origin,
      None => self.calibrate()?,
    };

    Ok((origin.0 + x, origin.1 + y))
  }

  fn click_at_pointer(&mut self, point: (f64, f64), button: u32) -> Result<()> {
    self.input.move_to(point.0, point.1)?;
    self.input.button(button, true)?;
    self.input.button(button, false)
  }

  fn type_text(&mut self, text: &str) -> Result<()> {
    for character in text.chars() {
      self
        .input
        .keysym(xkb::utf32_to_keysym(u32::from(character)).raw())?;
    }

    Ok(())
  }

  fn button_of(name: Option<String>) -> u32 {
    match name.as_deref() {
      Some("right") => 0x111,
      Some("middle") => 0x112,
      _ => 0x110,
    }
  }

  fn step(&mut self, args: &mut std::collections::VecDeque<String>) -> Result<()> {
    let step = args.pop_front().unwrap();
    let started = Instant::now();
    let mut next = || args.pop_front().ok_or("missing argument");

    match step.as_str() {
      "calibrate" => {
        self.calibrate()?;
      }
      "recalibrate" => {
        self.origin = None;
        self.calibrate()?;
      }
      "origin" => println!("origin {:?}", self.origin),
      "sleep" => sleep(Duration::from_millis(next()?.parse()?)),
      "run" => {
        let command = next()?;
        let status = std::process::Command::new("sh")
          .args(["-c", &command])
          .status()?;

        println!("run {command:?} -> {status}");
      }
      "click-at" | "right-click-at" => {
        let (x, y) = (next()?.parse()?, next()?.parse()?);
        let point = self.pointer_at(x, y)?;
        let button = if step == "right-click-at" {
          0x111
        } else {
          0x110
        };

        self.click_at_pointer(point, button)?;
        println!("{step} {x},{y} -> pointer {point:?}");
      }
      "double-click-at" => {
        let (x, y) = (next()?.parse()?, next()?.parse()?);
        let point = self.pointer_at(x, y)?;

        self.click_at_pointer(point, 0x110)?;
        sleep(Duration::from_millis(60));
        self.click_at_pointer(point, 0x110)?;
        println!("double-click-at {x},{y} -> pointer {point:?}");
      }
      "triple-click-at" => {
        let (x, y) = (next()?.parse()?, next()?.parse()?);
        let point = self.pointer_at(x, y)?;

        for _ in 0..3 {
          self.click_at_pointer(point, 0x110)?;
          sleep(Duration::from_millis(60));
        }

        println!("triple-click-at {x},{y} -> pointer {point:?}");
      }
      "middle-click-at" => {
        let (x, y) = (next()?.parse()?, next()?.parse()?);
        let point = self.pointer_at(x, y)?;

        self.click_at_pointer(point, 0x112)?;
        println!("middle-click-at {x},{y} -> pointer {point:?}");
      }
      "move" => {
        let (x, y) = (next()?.parse()?, next()?.parse()?);
        let point = self.pointer_at(x, y)?;
        let seen = self.move_and_see(point)?;

        println!("move {x},{y} -> pointer {point:?}, app saw {seen:?}");
      }
      "press" | "release" => {
        let button = Self::button_of(
          args
            .front()
            .filter(|word| ["left", "right", "middle"].contains(&word.as_str()))
            .cloned(),
        );

        if args
          .front()
          .is_some_and(|word| ["left", "right", "middle"].contains(&word.as_str()))
        {
          args.pop_front();
        }

        self.input.button(button, step == "press")?;
        println!("{step} {button:#x}");
      }
      "drag" => {
        let (x0, y0, x1, y1): (f64, f64, f64, f64) = (
          next()?.parse()?,
          next()?.parse()?,
          next()?.parse()?,
          next()?.parse()?,
        );
        let from = self.pointer_at(x0, y0)?;

        self.input.move_to(from.0, from.1)?;
        self.input.button(0x110, true)?;
        sleep(Duration::from_millis(50));

        for i in 1..=10 {
          let t = f64::from(i) / 10.0;

          self
            .input
            .move_to(from.0 + (x1 - x0) * t, from.1 + (y1 - y0) * t)?;
          sleep(Duration::from_millis(20));
        }

        self.input.button(0x110, false)?;
        println!(
          "drag {x0},{y0} by {},{} (pointer {from:?})",
          x1 - x0,
          y1 - y0
        );
      }
      "wheel" => {
        let (x, y): (f64, f64) = (next()?.parse()?, next()?.parse()?);
        let (dx, dy): (i32, i32) = (next()?.parse()?, next()?.parse()?);
        let point = self.pointer_at(x, y)?;

        self.input.move_to(point.0, point.1)?;
        self.input.wheel(dx, dy)?;
        println!("wheel {dx},{dy} at {x},{y}");
      }
      "type" => {
        let text = next()?;

        self.type_text(&text)?;
        println!("type {text:?} ({:?})", started.elapsed());
      }
      "key" => {
        let name = next()?;
        let sym = xkb::keysym_from_name(&name, xkb::KEYSYM_NO_FLAGS);

        self.input.keysym(sym.raw())?;
        println!("key {name}");
      }
      "keycode" => {
        let code: u32 = next()?.parse()?;

        self.input.keycode(code)?;
        println!("keycode {code}");
      }
      "keydown" | "keyup" => {
        let name = next()?;
        let sym = xkb::keysym_from_name(&name, xkb::KEYSYM_NO_FLAGS);

        self.input.key_state(sym.raw(), step == "keydown")?;
        println!("{step} {name}");
      }
      "chord" => {
        let names: Vec<String> = next()?.split('+').map(str::to_owned).collect();
        let syms: Vec<u32> = names
          .iter()
          .map(|name| xkb::keysym_from_name(name, xkb::KEYSYM_NO_FLAGS).raw())
          .collect();

        for sym in &syms {
          self.input.key_state(*sym, true)?;
        }

        for sym in syms.iter().rev() {
          self.input.key_state(*sym, false)?;
        }

        println!("chord {names:?}");
      }
      other => return Err(format!("unknown step {other}").into()),
    }

    Ok(())
  }
}

/// Exits unless the session is private: `SKENE_PRIVATE_SESSION` names the runtime directory
/// a session script made (`tools/sessions/session.sh`, `tools/controls/session.sh`), so real
/// input never reaches the user's display.
fn require_private_session() {
  let marker = std::env::var_os("SKENE_PRIVATE_SESSION").filter(|marker| !marker.is_empty());
  let private = marker.is_some_and(|marker| {
    std::env::var_os("XDG_RUNTIME_DIR").as_ref() == Some(&marker)
      && std::path::Path::new(&marker).is_dir()
  });

  if !private {
    eprintln!(
      "drive: refusing to inject input outside a private session (run it through tools/sessions/session.sh or tools/controls/session.sh)"
    );
    std::process::exit(2);
  }
}

fn main() {
  require_private_session();

  let mut args: std::collections::VecDeque<String> = std::env::args().skip(1).collect();
  let backend = args.pop_front().expect("usage: drive <backend> <step>...");
  let stream = std::env::var("STREAM").unwrap_or_else(|_| "monitor".into());
  let input: Result<Box<dyn Input>> = match backend.as_str() {
    "x11" => X11::new().map(|input| Box::new(input) as Box<dyn Input>),
    "mutter-dbus" => Mutter::new(&stream).map(|input| Box::new(input) as Box<dyn Input>),
    "mutter-ei" => Ei::new(&stream).map(|input| Box::new(input) as Box<dyn Input>),
    "sway" => Sway::new().map(|input| Box::new(input) as Box<dyn Input>),
    other => Err(format!("unknown backend {other}").into()),
  };
  let input = input.unwrap_or_else(|error| {
    eprintln!("FAIL input setup: {error}");
    std::process::exit(2);
  });
  let mut driver = Driver {
    input,
    backend,
    probe: std::env::var_os("SKENE_PROBE").map(Into::into),
    origin: None,
  };

  while !args.is_empty() {
    if let Err(error) = driver.step(&mut args) {
      println!("FAIL {error}");
      std::process::exit(1);
    }
  }
}
