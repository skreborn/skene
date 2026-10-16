//! Real input, as a user's devices send it, injected through each display server's own path:
//!
//! - X11: XTest, from a connection of the test's own;
//! - mutter (headless, Wayland): a RemoteDesktop session's EIS socket, spoken through reis
//!   (libei's protocol), its absolute pointer aimed in the session's ScreenCast stream;
//! - sway (headless, Wayland): `zwlr_virtual_pointer_v1` and `zwp_virtual_keyboard_v1`.
//!
//! Every backend presses keys by keysym: the keymap the display server gives the device is
//! read, and a key whose keysym sits on a level of its own is pressed with that level's
//! modifier. Points are in the backend's pointer space; [`Input::window_origin`] says where a
//! window sits in it where the backend knows.

use std::{
  collections::HashMap,
  os::fd::{AsFd as _, FromRawFd as _, OwnedFd},
  thread::sleep,
  time::{Duration, Instant},
};

use xkbcommon::xkb;
use zbus::{
  blocking::Connection,
  zvariant::{OwnedObjectPath, OwnedValue, Value},
};

/// A failure, as a message.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The evdev code of the primary button.
pub const PRIMARY: u32 = 0x110;
/// The evdev code of the secondary button.
pub const SECONDARY: u32 = 0x111;
/// The evdev code of the middle button.
pub const MIDDLE: u32 = 0x112;

/// The evdev code of the left Shift key.
const SHIFT: u32 = 42;
/// The evdev code of the right Alt key: level 3 on layouts that have one.
const LEVEL3: u32 = 100;

/// Where a keysym is on a keymap: its evdev code and its level (0 plain, 1 Shift, 2 the third
/// level).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Place {
  /// The evdev code.
  pub code: u32,
  /// The level.
  pub level: u32,
}

/// Every keysym of `keymap`'s first layout, by where it is first found.
fn places(keymap: &xkb::Keymap) -> HashMap<u32, Place> {
  let mut map = HashMap::new();

  for level in 0..3 {
    for code in keymap.min_keycode().raw()..=keymap.max_keycode().raw() {
      let keycode = xkb::Keycode::new(code);

      for sym in keymap.key_get_syms_by_level(keycode, 0, level) {
        map.entry(sym.raw()).or_insert(Place {
          code: code - 8,
          level,
        });
      }
    }
  }

  map
}

/// The modifier keysyms a chord names, by their evdev codes.
fn modifier_code(keysym: u32) -> Option<u32> {
  match keysym {
    0xffe1 => Some(42),
    0xffe2 => Some(54),
    0xffe3 => Some(29),
    0xffe4 => Some(97),
    0xffe9 => Some(56),
    0xffea => Some(100),
    0xffeb => Some(125),
    0xffec => Some(126),
    _ => None,
  }
}

/// One way of sending real input.
pub trait Input {
  /// Moves the pointer to `(x, y)` in the backend's pointer space.
  fn move_to(&mut self, x: f64, y: f64) -> Result<()>;

  /// Presses or releases the evdev `button`.
  fn button(&mut self, button: u32, down: bool) -> Result<()>;

  /// Presses or releases the evdev key `code`.
  fn key(&mut self, code: u32, down: bool) -> Result<()>;

  /// Where `keysym` is on the device's keymap, where it is.
  fn place(&mut self, keysym: u32) -> Option<Place>;

  /// Turns the wheel by detents, right and down positive.
  fn wheel(&mut self, right: i32, down: i32) -> Result<()>;

  /// Waits until what was sent has reached the display server.
  fn sync(&mut self) -> Result<()>;

  /// Where the window's geometry starts in the pointer space, where the backend knows it
  /// without looking.
  fn window_origin(&mut self) -> Option<(f64, f64)>;

  /// The pointer space's size.
  fn extent(&self) -> (f64, f64);

  /// Presses and releases the key typing `keysym`, with the level's modifier held.
  ///
  /// # Errors
  ///
  /// Where no key of the device's keymap types it.
  fn tap(&mut self, keysym: u32) -> Result<()> {
    let place = self
      .place(keysym)
      .ok_or_else(|| format!("no key types keysym {keysym:#x} on the test keyboard"))?;
    let modifier = match place.level {
      0 => None,
      1 => Some(SHIFT),
      _ => Some(LEVEL3),
    };

    if let Some(modifier) = modifier {
      self.key(modifier, true)?;
    }

    self.key(place.code, true)?;
    self.key(place.code, false)?;

    if let Some(modifier) = modifier {
      self.key(modifier, false)?;
    }

    self.sync()
  }

  /// Presses `keysym` with the modifier keysyms `held` held down around it.
  ///
  /// # Errors
  ///
  /// Where no key types it.
  fn chord(&mut self, held: &[u32], keysym: u32) -> Result<()> {
    let codes: Vec<u32> = held.iter().filter_map(|sym| modifier_code(*sym)).collect();

    for code in &codes {
      self.key(*code, true)?;
    }

    let tapped = self.tap(keysym);

    for code in codes.iter().rev() {
      self.key(*code, false)?;
    }

    self.sync()?;

    tapped
  }
}

/// XTest on the test's own connection to `DISPLAY`.
pub struct X11 {
  conn: x11rb::rust_connection::RustConnection,
  root: u32,
  places: HashMap<u32, Place>,
  extent: (f64, f64),
}

impl X11 {
  /// Connects to `DISPLAY`.
  ///
  /// # Errors
  ///
  /// Where there is no display or it has no XTest.
  pub fn new() -> Result<Self> {
    use x11rb::connection::Connection as _;

    let (conn, screen) = x11rb::connect(None)?;
    let setup = conn.setup();
    let root = setup.roots[screen].root;
    let extent = (
      f64::from(setup.roots[screen].width_in_pixels),
      f64::from(setup.roots[screen].height_in_pixels),
    );
    let mut x11 = Self {
      conn,
      root,
      places: HashMap::new(),
      extent,
    };

    x11.read_keymap()?;

    Ok(x11)
  }

  /// Reads the core keymap's first group: plain and shifted levels, and the third level where
  /// the mapping lists one.
  fn read_keymap(&mut self) -> Result<()> {
    use x11rb::{connection::Connection as _, protocol::xproto::ConnectionExt as _};

    let setup = self.conn.setup();
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let map = self
      .conn
      .get_keyboard_mapping(min, max - min + 1)?
      .reply()?;
    let per = usize::from(map.keysyms_per_keycode);

    self.places.clear();

    // Core mapping columns: group 1 plain, group 1 shifted, group 2 plain, group 2 shifted,
    // then level 3 and 4 of group 1.
    for (column, level) in [(0, 0), (1, 1), (4, 2)] {
      for (index, syms) in map.keysyms.chunks(per).enumerate() {
        if let Some(sym) = syms.get(column)
          && *sym != 0
        {
          self.places.entry(*sym).or_insert(Place {
            code: u32::from(min) + index as u32 - 8,
            level,
          });
        }
      }
    }

    Ok(())
  }

  fn fake(&self, kind: u8, detail: u8, x: i16, y: i16) -> Result<()> {
    use x11rb::{connection::Connection as _, protocol::xtest::ConnectionExt as _};

    self
      .conn
      .xtest_fake_input(kind, detail, 0, self.root, x, y, 0)?;
    self.conn.flush()?;

    Ok(())
  }

  /// Maps `keysym` to a keycode no key uses, as a test keyboard with that key would have it:
  /// for a character the session's layout does not type.
  fn add_key(&mut self, keysym: u32) -> Result<Place> {
    use x11rb::{
      connection::Connection as _, protocol::xproto::ConnectionExt as _,
      wrapper::ConnectionExt as _,
    };

    let setup = self.conn.setup();
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let map = self
      .conn
      .get_keyboard_mapping(min, max - min + 1)?
      .reply()?;
    let per = usize::from(map.keysyms_per_keycode);
    let free = map
      .keysyms
      .chunks(per)
      .enumerate()
      .rev()
      .find(|(_, syms)| syms.iter().all(|sym| *sym == 0))
      .map(|(index, _)| min + index as u8)
      .ok_or("the keymap has no free keycode")?;

    self
      .conn
      .change_keyboard_mapping(1, free, 2, &[keysym, keysym])?;
    self.conn.sync()?;

    // The server announces the mapping before the key: the app reads it first.
    sleep(Duration::from_millis(20));

    let place = Place {
      code: u32::from(free) - 8,
      level: 0,
    };

    self.places.insert(keysym, place);

    Ok(place)
  }
}

impl Input for X11 {
  fn move_to(&mut self, x: f64, y: f64) -> Result<()> {
    self.fake(6, 0, x.round() as i16, y.round() as i16)
  }

  fn button(&mut self, button: u32, down: bool) -> Result<()> {
    let detail = match button {
      SECONDARY => 3,
      MIDDLE => 2,
      _ => 1,
    };

    self.fake(if down { 4 } else { 5 }, detail, 0, 0)
  }

  fn key(&mut self, code: u32, down: bool) -> Result<()> {
    self.fake(if down { 2 } else { 3 }, (code + 8) as u8, 0, 0)
  }

  fn place(&mut self, keysym: u32) -> Option<Place> {
    match self.places.get(&keysym) {
      Some(place) => Some(*place),
      None => self.add_key(keysym).ok(),
    }
  }

  fn wheel(&mut self, right: i32, down: i32) -> Result<()> {
    for (count, back, forward) in [(down, 4, 5), (right, 6, 7)] {
      let detail = if count < 0 { back } else { forward };

      for _ in 0..count.unsigned_abs() {
        self.fake(4, detail, 0, 0)?;
        self.fake(5, detail, 0, 0)?;
      }
    }

    Ok(())
  }

  fn sync(&mut self) -> Result<()> {
    use x11rb::wrapper::ConnectionExt as _;

    self.conn.sync()?;

    Ok(())
  }

  fn window_origin(&mut self) -> Option<(f64, f64)> {
    Some((0., 0.))
  }

  fn extent(&self) -> (f64, f64) {
    self.extent
  }
}

const REMOTE_DESKTOP: &str = "org.gnome.Mutter.RemoteDesktop";
const SCREEN_CAST: &str = "org.gnome.Mutter.ScreenCast";

/// mutter's RemoteDesktop session and its ScreenCast stream of the monitor.
struct Mutter {
  conn: Connection,
  session: OwnedObjectPath,
  mapping: Option<String>,
}

impl Mutter {
  fn new() -> Result<Self> {
    let conn = Connection::session()?;
    let session: OwnedObjectPath = conn
      .call_method(
        Some(REMOTE_DESKTOP),
        "/org/gnome/Mutter/RemoteDesktop",
        Some(REMOTE_DESKTOP),
        "CreateSession",
        &(),
      )?
      .body()
      .deserialize()?;
    let id: OwnedValue = conn
      .call_method(
        Some(REMOTE_DESKTOP),
        session.as_str(),
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &(format!("{REMOTE_DESKTOP}.Session"), "SessionId"),
      )?
      .body()
      .deserialize()?;
    let id = String::try_from(id)?;
    let mut properties: HashMap<&str, Value<'_>> = HashMap::new();

    properties.insert("remote-desktop-session-id", Value::from(id.as_str()));

    let cast: OwnedObjectPath = conn
      .call_method(
        Some(SCREEN_CAST),
        "/org/gnome/Mutter/ScreenCast",
        Some(SCREEN_CAST),
        "CreateSession",
        &(properties,),
      )?
      .body()
      .deserialize()?;
    let mut options: HashMap<&str, Value<'_>> = HashMap::new();

    options.insert("cursor-mode", Value::from(0u32));

    let stream: OwnedObjectPath = conn
      .call_method(
        Some(SCREEN_CAST),
        cast.as_str(),
        Some(&*format!("{SCREEN_CAST}.Session")),
        "RecordMonitor",
        &("", options),
      )?
      .body()
      .deserialize()?;

    conn.call_method(
      Some(REMOTE_DESKTOP),
      session.as_str(),
      Some(&*format!("{REMOTE_DESKTOP}.Session")),
      "Start",
      &(),
    )?;

    // The stream's parameters arrive once PipeWire has it.
    let started = Instant::now();
    let mapping = loop {
      let parameters = conn
        .call_method(
          Some(SCREEN_CAST),
          stream.as_str(),
          Some("org.freedesktop.DBus.Properties"),
          "Get",
          &(format!("{SCREEN_CAST}.Stream"), "Parameters"),
        )
        .map_err(Into::into)
        .and_then(|reply| -> Result<HashMap<String, OwnedValue>> {
          let value: OwnedValue = reply.body().deserialize()?;

          Ok(HashMap::<String, OwnedValue>::try_from(value)?)
        });

      if let Ok(parameters) = parameters
        && parameters.contains_key("size")
      {
        break parameters
          .get("mapping-id")
          .and_then(|value| String::try_from(value.try_clone().ok()?).ok());
      }

      if started.elapsed() > Duration::from_secs(5) {
        return Err("mutter's ScreenCast stream reported no size (is PipeWire running?)".into());
      }

      sleep(Duration::from_millis(20));
    };

    Ok(Self {
      conn,
      session,
      mapping,
    })
  }

  /// The session's EIS socket.
  fn connect_to_eis(&self) -> Result<OwnedFd> {
    let options: HashMap<&str, Value<'_>> = HashMap::new();
    let reply = self.conn.call_method(
      Some(REMOTE_DESKTOP),
      self.session.as_str(),
      Some(&*format!("{REMOTE_DESKTOP}.Session")),
      "ConnectToEIS",
      &(options,),
    )?;
    let fd: zbus::zvariant::OwnedFd = reply.body().deserialize()?;

    Ok(fd.into())
  }
}

/// A libei sender on mutter's EIS, its events pumped on the test's thread.
pub struct Ei {
  /// Keeps the RemoteDesktop session alive.
  mutter: Mutter,
  /// The libei context.
  context: reis::ei::Context,
  /// The events' converter.
  converter: reis::event::EiEventConverter,
  /// The absolute pointer device.
  absolute: Option<reis::event::Device>,
  /// The keyboard device.
  keyboard: Option<reis::event::Device>,
  /// The devices resumed.
  resumed: Vec<reis::event::Device>,
  /// Where each keysym is on the keymap.
  places: HashMap<u32, Place>,
  /// The absolute region aimed in, in the compositor's logical coordinates.
  region: (f64, f64, f64, f64),
  /// When the client started: the events' timestamps count from it.
  started: Instant,
  /// The last emulation sequence number.
  sequence: u32,
}

impl Ei {
  /// Opens a RemoteDesktop session on mutter and connects to its EIS socket, waiting for an
  /// absolute pointer and a keyboard.
  ///
  /// # Errors
  ///
  /// Where mutter has no RemoteDesktop service or EIS never offers the devices.
  pub fn new() -> Result<Self> {
    let mutter = Mutter::new()?;
    let stream = std::os::unix::net::UnixStream::from(mutter.connect_to_eis()?);
    let context = reis::ei::Context::new(stream)?;
    let response = reis::handshake::ei_handshake_blocking(
      &context,
      "skene-testing",
      reis::ei::handshake::ContextType::Sender,
    )?;
    let converter = reis::event::EiEventConverter::new(&context, response);
    let mut ei = Self {
      mutter,
      context,
      converter,
      absolute: None,
      keyboard: None,
      resumed: Vec::new(),
      places: HashMap::new(),
      region: (0., 0., 0., 0.),
      started: Instant::now(),
      sequence: 0,
    };
    let started = Instant::now();

    loop {
      ei.pump(20)?;

      let ready = |device: &Option<reis::event::Device>| {
        device
          .as_ref()
          .is_some_and(|device| ei.resumed.contains(device))
      };

      if ready(&ei.absolute) && ready(&ei.keyboard) {
        break;
      }

      if started.elapsed() > Duration::from_secs(5) {
        return Err("mutter's EIS offered no absolute pointer and keyboard".into());
      }
    }

    let keyboard = ei.keyboard.clone().ok_or("no keyboard")?;
    let keymap = keyboard.keymap().ok_or("EIS sent no keymap")?;
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    // SAFETY: the EIS server's keymap file, mapped read-only for its size.
    let keymap = unsafe {
      xkb::Keymap::new_from_fd(
        &context,
        keymap.fd.try_clone()?,
        keymap.size as usize,
        xkb::KEYMAP_FORMAT_TEXT_V1,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
      )?
    }
    .ok_or("EIS sent a keymap xkbcommon cannot read")?;
    let absolute = ei.absolute.clone().ok_or("no absolute pointer")?;
    let regions = absolute.regions();
    let region = regions
      .iter()
      .find(|region| region.mapping_id.is_some() && region.mapping_id == ei.mutter.mapping)
      .or_else(|| regions.first())
      .ok_or("the absolute pointer has no region")?;

    ei.places = places(&keymap);
    ei.region = (
      f64::from(region.x),
      f64::from(region.y),
      f64::from(region.width),
      f64::from(region.height),
    );
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
      let serial = self.converter.connection().serial();

      match &event {
        reis::event::EiEvent::SeatAdded(added) => {
          added.seat.bind_capabilities(
            reis::event::DeviceCapability::PointerAbsolute
              | reis::event::DeviceCapability::Keyboard
              | reis::event::DeviceCapability::Button
              | reis::event::DeviceCapability::Scroll
              | reis::event::DeviceCapability::Pointer,
          );
        }
        reis::event::EiEvent::DeviceAdded(added) => {
          let device = &added.device;

          if device.has_capability(reis::event::DeviceCapability::PointerAbsolute) {
            self.absolute = Some(device.clone());
          }

          if device.has_capability(reis::event::DeviceCapability::Keyboard) {
            self.keyboard = Some(device.clone());
          }
        }
        reis::event::EiEvent::DeviceResumed(resumed) => {
          self.resumed.push(resumed.device.clone());
          self.sequence += 1;
          resumed
            .device
            .device()
            .start_emulating(serial, self.sequence);
        }
        reis::event::EiEvent::DeviceRemoved(removed) => {
          self.resumed.retain(|device| device != &removed.device);

          if self.absolute.as_ref() == Some(&removed.device) {
            self.absolute = None;
          }

          if self.keyboard.as_ref() == Some(&removed.device) {
            self.keyboard = None;
          }
        }
        _ => {}
      }
    }

    self.context.flush()?;

    Ok(())
  }

  fn device(&self, absolute: bool) -> Result<reis::event::Device> {
    let device = if absolute {
      &self.absolute
    } else {
      &self.keyboard
    };

    device
      .clone()
      .ok_or_else(|| "mutter's EIS removed the device".into())
  }

  fn frame(&self, device: &reis::event::Device) -> Result<()> {
    device.device().frame(
      self.converter.connection().serial(),
      self.started.elapsed().as_micros() as u64,
    );
    self.context.flush()?;

    Ok(())
  }
}

impl Input for Ei {
  fn move_to(&mut self, x: f64, y: f64) -> Result<()> {
    self.pump(0)?;

    let device = self.device(true)?;
    let pointer = device
      .interface::<reis::ei::PointerAbsolute>()
      .ok_or("no absolute pointer")?;

    pointer.motion_absolute((self.region.0 + x) as f32, (self.region.1 + y) as f32);
    self.frame(&device)
  }

  fn button(&mut self, button: u32, down: bool) -> Result<()> {
    let device = self.device(true)?;
    let buttons = device.interface::<reis::ei::Button>().ok_or("no buttons")?;

    buttons.button(
      button,
      if down {
        reis::ei::button::ButtonState::Press
      } else {
        reis::ei::button::ButtonState::Released
      },
    );
    self.frame(&device)
  }

  fn key(&mut self, code: u32, down: bool) -> Result<()> {
    let device = self.device(false)?;
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
    self.frame(&device)
  }

  fn place(&mut self, keysym: u32) -> Option<Place> {
    self.places.get(&keysym).copied()
  }

  fn wheel(&mut self, right: i32, down: i32) -> Result<()> {
    let device = self.device(true)?;
    let scroll = device.interface::<reis::ei::Scroll>().ok_or("no scroll")?;

    scroll.scroll_discrete(right * 120, down * 120);
    self.frame(&device)
  }

  fn sync(&mut self) -> Result<()> {
    self.pump(0)
  }

  fn window_origin(&mut self) -> Option<(f64, f64)> {
    None
  }

  fn extent(&self) -> (f64, f64) {
    (self.region.2, self.region.3)
  }
}

/// sway's virtual pointer and keyboard on a connection of the test's own.
pub struct Sway {
  _conn: wayland_client::Connection,
  queue: wayland_client::EventQueue<Devices>,
  pointer: wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
  keyboard: wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
  places: HashMap<u32, Place>,
  extent: (u32, u32),
  started: Instant,
  modifiers: u32,
  _keymap: std::fs::File,
}

/// The virtual devices' connection state: nothing is read from it.
pub struct Devices;

impl
  wayland_client::Dispatch<
    wayland_client::protocol::wl_registry::WlRegistry,
    wayland_client::globals::GlobalListContents,
  > for Devices
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

wayland_client::delegate_noop!(Devices: ignore wayland_client::protocol::wl_seat::WlSeat);
wayland_client::delegate_noop!(Devices: ignore wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1);
wayland_client::delegate_noop!(Devices: ignore wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1);
wayland_client::delegate_noop!(Devices: ignore wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1);
wayland_client::delegate_noop!(Devices: ignore wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1);

impl Sway {
  /// Creates a virtual pointer and a virtual keyboard (a US layout, Compose on the right
  /// Control key) on `WAYLAND_DISPLAY`'s seat; `extent` is the output's size, which absolute
  /// motion spans.
  ///
  /// # Errors
  ///
  /// Where the compositor offers no virtual pointer or keyboard.
  pub fn new(extent: (u32, u32)) -> Result<Self> {
    use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
    use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;

    let conn = wayland_client::Connection::connect_to_env()?;
    let (globals, mut queue) = wayland_client::globals::registry_queue_init::<Devices>(&conn)?;
    let qh = queue.handle();
    let seat: wayland_client::protocol::wl_seat::WlSeat = globals.bind(&qh, 1..=1, ())?;
    let pointers: ZwlrVirtualPointerManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let keyboards: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let pointer = pointers.create_virtual_pointer(Some(&seat), &qh, ());
    let keyboard = keyboards.create_virtual_keyboard(&seat, &qh, ());
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let keymap = xkb::Keymap::new_from_names(
      &context,
      "",
      "",
      "us",
      "",
      Some("compose:rctrl".to_owned()),
      xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .ok_or("xkbcommon has no US keymap")?;
    let text = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
    let bytes = [text.as_bytes(), b"\0"].concat();
    // SAFETY: an anonymous file for the keymap, owned by `file` from here on.
    let file = unsafe {
      std::fs::File::from_raw_fd(libc::memfd_create(c"keymap".as_ptr(), libc::MFD_CLOEXEC))
    };

    std::io::Write::write_all(&mut &file, &bytes)?;
    keyboard.keymap(1, file.as_fd(), bytes.len() as u32);
    queue.roundtrip(&mut Devices)?;

    Ok(Self {
      _conn: conn,
      queue,
      pointer,
      keyboard,
      places: places(&keymap),
      extent,
      started: Instant::now(),
      modifiers: 0,
      _keymap: file,
    })
  }

  fn time(&self) -> u32 {
    self.started.elapsed().as_millis() as u32
  }
}

impl Input for Sway {
  fn move_to(&mut self, x: f64, y: f64) -> Result<()> {
    let time = self.time();

    self.pointer.motion_absolute(
      time,
      x.max(0.).round() as u32,
      y.max(0.).round() as u32,
      self.extent.0,
      self.extent.1,
    );
    self.pointer.frame();

    Ok(())
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

    Ok(())
  }

  fn key(&mut self, code: u32, down: bool) -> Result<()> {
    let time = self.time();

    self.keyboard.key(time, code, u32::from(down));

    // The virtual keyboard states its modifiers itself: Shift, Control, Alt (Mod1), Super
    // (Mod4) and the third level (Mod5) as the keymap's real modifiers.
    let bit = match code {
      42 | 54 => 0x1,
      29 | 97 => 0x4,
      56 => 0x8,
      125 | 126 => 0x40,
      LEVEL3 => 0x80,
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

    Ok(())
  }

  fn place(&mut self, keysym: u32) -> Option<Place> {
    self.places.get(&keysym).copied()
  }

  fn wheel(&mut self, right: i32, down: i32) -> Result<()> {
    use wayland_client::protocol::wl_pointer::{Axis, AxisSource};

    let time = self.time();

    self.pointer.axis_source(AxisSource::Wheel);

    for (axis, count) in [
      (Axis::VerticalScroll, down),
      (Axis::HorizontalScroll, right),
    ] {
      if count != 0 {
        self
          .pointer
          .axis_discrete(time, axis, f64::from(count) * 15., count);
      }
    }

    self.pointer.frame();

    Ok(())
  }

  fn sync(&mut self) -> Result<()> {
    self.queue.roundtrip(&mut Devices)?;

    Ok(())
  }

  fn window_origin(&mut self) -> Option<(f64, f64)> {
    None
  }

  fn extent(&self) -> (f64, f64) {
    (f64::from(self.extent.0), f64::from(self.extent.1))
  }
}
