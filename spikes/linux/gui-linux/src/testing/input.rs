//! Real input, as a user's devices send it, injected through each display server's own path:
//!
//! - X11: XTest, from a connection of the test's own; touch through the touchscreen Xorg's
//!   inputtest driver offers on the X11 leg (`SKENE_TOUCH_SOCKET`), which posts it as a real
//!   touchscreen's XInput 2.2 touches;
//! - mutter (headless, Wayland): a RemoteDesktop session's EIS socket, spoken through reis
//!   (libei's protocol), its absolute pointer aimed in the session's ScreenCast stream; keys,
//!   and touch where its EIS offers no touchscreen, through the session's own D-Bus requests,
//!   since mutter 46 reports a modifier held on an EIS keyboard as released under a popup's
//!   grab;
//! - sway (headless, Wayland): `zwlr_virtual_pointer_v1` and `zwp_virtual_keyboard_v1`;
//! - KWin (windowed, Wayland): `org_kde_kwin_fake_input`, its pointer, keyboard and
//!   touchscreen;
//! - weston (Wayland, its X11 backend): XTest into the X window weston shows its output in, which
//!   weston takes its pointer and keyboard from.
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
pub const SHIFT: u32 = 42;
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
const fn modifier_code(keysym: u32) -> Option<u32> {
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

  /// Moves two fingers on a touchpad by `right` and `down` pixels of scrolling, as one frame
  /// of a touchpad's continuous scroll.
  ///
  /// # Errors
  ///
  /// Where the backend has no touchpad to send it from.
  fn finger_scroll(&mut self, right: f64, down: f64) -> Result<()> {
    let _ = (right, down);

    Err(format!("{} sends no touchpad scrolling", self.name()).into())
  }

  /// Lifts the fingers off the touchpad: the end of a continuous scroll.
  ///
  /// # Errors
  ///
  /// Where the backend has no touchpad to send it from.
  fn finger_scroll_stop(&mut self) -> Result<()> {
    Err(format!("{} sends no touchpad scrolling", self.name()).into())
  }

  /// Puts touch point `id` down on the touchscreen at `(x, y)` in the pointer space.
  ///
  /// # Errors
  ///
  /// Where the backend has no touchscreen to send it from.
  fn touch_down(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    let _ = (id, x, y);

    Err(format!("{} sends no touch", self.name()).into())
  }

  /// Moves touch point `id` to `(x, y)` in the pointer space.
  ///
  /// # Errors
  ///
  /// Where the backend has no touchscreen to send it from.
  fn touch_motion(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    let _ = (id, x, y);

    Err(format!("{} sends no touch", self.name()).into())
  }

  /// Lifts touch point `id`.
  ///
  /// # Errors
  ///
  /// Where the backend has no touchscreen to send it from.
  fn touch_up(&mut self, id: u32) -> Result<()> {
    let _ = id;

    Err(format!("{} sends no touch", self.name()).into())
  }

  /// Whether the backend sends touch.
  fn has_touch(&self) -> bool {
    false
  }

  /// Whether the backend sends a touchpad's continuous scrolling and its end.
  fn has_finger_scroll(&self) -> bool {
    false
  }

  /// The backend's name, as a failure names it.
  fn name(&self) -> &'static str;

  /// Waits until what was sent has reached the display server.
  fn sync(&mut self) -> Result<()>;

  /// Where the window's geometry starts in the pointer space, where the backend knows it
  /// without looking.
  fn window_origin(&mut self) -> Option<(f64, f64)>;

  /// The pointer space's size.
  fn extent(&self) -> (f64, f64);

  /// Where a move to `x`, `y` puts the pointer: there, or the nearest point the device can
  /// name.
  fn landing(&self, x: f64, y: f64) -> (f64, f64) {
    (x, y)
  }

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

  /// Presses the modifier keysyms `held`, then the key typing `keysym` (with its level's
  /// modifier), and leaves them all down: the keys pressed, in order, which
  /// [`Input::release`] lets go of.
  ///
  /// # Errors
  ///
  /// Where no key types it.
  fn chord_down(&mut self, held: &[u32], keysym: u32) -> Result<Vec<u32>> {
    let place = self
      .place(keysym)
      .ok_or_else(|| format!("no key types keysym {keysym:#x} on the test keyboard"))?;
    let mut codes: Vec<u32> = held.iter().filter_map(|sym| modifier_code(*sym)).collect();

    match place.level {
      0 => {}
      1 => codes.push(SHIFT),
      _ => codes.push(LEVEL3),
    }

    codes.push(place.code);

    for code in &codes {
      self.key(*code, true)?;
    }

    self.sync()?;

    Ok(codes)
  }

  /// Lets go of the keys `codes`, the last pressed first.
  ///
  /// # Errors
  ///
  /// Where the backend cannot send it.
  fn release(&mut self, codes: &[u32]) -> Result<()> {
    for code in codes.iter().rev() {
      self.key(*code, false)?;
    }

    self.sync()
  }
}

/// XTest on the test's own connection to an X display: the app's own (`DISPLAY`), or the one a
/// nested compositor shows its output in (`SKENE_INPUT_DISPLAY`), whose window the compositor
/// takes its pointer and keyboard from.
pub struct X11 {
  conn: x11rb::rust_connection::RustConnection,
  root: u32,
  places: HashMap<u32, Place>,
  extent: (f64, f64),
  nested: bool,
  touchscreen: Option<Touchscreen>,
}

impl X11 {
  /// Connects to `DISPLAY`, the display the app's window is on.
  ///
  /// # Errors
  ///
  /// Where there is no display or it has no XTest.
  pub fn new() -> Result<Self> {
    Self::connect(None, false)
  }

  /// Connects to `display`, where a nested compositor (weston's X11 backend) shows its output
  /// as a window: the pointer space is the X screen's, where the app's window is found as a
  /// Wayland window is, by moving the pointer onto it.
  ///
  /// # Errors
  ///
  /// Where there is no such display or it has no XTest.
  pub fn nested(display: &str) -> Result<Self> {
    Self::connect(Some(display), true)
  }

  fn connect(display: Option<&str>, nested: bool) -> Result<Self> {
    use x11rb::connection::Connection as _;

    let (conn, screen) = x11rb::connect(display)?;
    let setup = conn.setup();
    let root = setup.roots[screen].root;
    let extent = (
      f64::from(setup.roots[screen].width_in_pixels),
      f64::from(setup.roots[screen].height_in_pixels),
    );
    let touchscreen = match std::env::var_os("SKENE_TOUCH_SOCKET") {
      Some(path) if !nested => Some(Touchscreen::connect(path.as_ref(), extent)?),
      _ => None,
    };
    let mut x11 = Self {
      conn,
      root,
      places: HashMap::new(),
      extent,
      nested,
      touchscreen,
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

/// A backend whose pointer and touches are followed: where the last move sent the pointer,
/// and which touch points are down.
pub struct Tracked {
  /// The backend.
  input: Box<dyn Input>,
  /// Where the last move sent the pointer, in the pointer's space.
  pointer: Option<(f64, f64)>,
  /// The touch points put down and not yet lifted.
  touches: Vec<u32>,
}

impl Tracked {
  /// `input`, its pointer not yet moved and no touch down.
  #[must_use]
  pub const fn new(input: Box<dyn Input>) -> Self {
    Self {
      input,
      pointer: None,
      touches: Vec::new(),
    }
  }

  /// Where the last move sent the pointer, in the pointer's space; `None` before the first.
  #[must_use]
  pub const fn pointer(&self) -> Option<(f64, f64)> {
    self.pointer
  }

  /// Lifts every touch point put down and not yet lifted, so the compositor holds none.
  ///
  /// # Errors
  ///
  /// Where the backend cannot send it.
  pub fn lift_touches(&mut self) -> Result<()> {
    for id in std::mem::take(&mut self.touches) {
      self.input.touch_up(id)?;
    }

    self.input.sync()
  }
}

impl Input for Tracked {
  fn move_to(&mut self, x: f64, y: f64) -> Result<()> {
    self.input.move_to(x, y)?;
    self.pointer = Some((x, y));

    Ok(())
  }

  fn button(&mut self, button: u32, down: bool) -> Result<()> {
    self.input.button(button, down)
  }

  fn key(&mut self, code: u32, down: bool) -> Result<()> {
    self.input.key(code, down)
  }

  fn place(&mut self, keysym: u32) -> Option<Place> {
    self.input.place(keysym)
  }

  fn wheel(&mut self, right: i32, down: i32) -> Result<()> {
    self.input.wheel(right, down)
  }

  fn finger_scroll(&mut self, right: f64, down: f64) -> Result<()> {
    self.input.finger_scroll(right, down)
  }

  fn finger_scroll_stop(&mut self) -> Result<()> {
    self.input.finger_scroll_stop()
  }

  fn touch_down(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    self.input.touch_down(id, x, y)?;
    self.touches.push(id);

    Ok(())
  }

  fn touch_motion(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    self.input.touch_motion(id, x, y)
  }

  fn touch_up(&mut self, id: u32) -> Result<()> {
    self.input.touch_up(id)?;
    self.touches.retain(|down| *down != id);

    Ok(())
  }

  fn has_touch(&self) -> bool {
    self.input.has_touch()
  }

  fn has_finger_scroll(&self) -> bool {
    self.input.has_finger_scroll()
  }

  fn name(&self) -> &'static str {
    self.input.name()
  }

  fn sync(&mut self) -> Result<()> {
    self.input.sync()
  }

  fn window_origin(&mut self) -> Option<(f64, f64)> {
    self.input.window_origin()
  }

  fn extent(&self) -> (f64, f64) {
    self.input.extent()
  }

  fn landing(&self, x: f64, y: f64) -> (f64, f64) {
    self.input.landing(x, y)
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

  fn touch_down(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    self.touchscreen()?.touch(Touchscreen::BEGIN, id, (x, y))
  }

  fn touch_motion(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    self.touchscreen()?.touch(Touchscreen::UPDATE, id, (x, y))
  }

  fn touch_up(&mut self, id: u32) -> Result<()> {
    let touchscreen = self.touchscreen()?;
    let at = touchscreen.points.get(&id).copied().unwrap_or_default();

    touchscreen.touch(Touchscreen::END, id, at)
  }

  fn has_touch(&self) -> bool {
    self.touchscreen.is_some()
  }

  fn sync(&mut self) -> Result<()> {
    use x11rb::wrapper::ConnectionExt as _;

    if let Some(touchscreen) = self.touchscreen.as_mut() {
      touchscreen.sync()?;
    }

    self.conn.sync()?;

    Ok(())
  }

  fn window_origin(&mut self) -> Option<(f64, f64)> {
    (!self.nested).then_some((0., 0.))
  }

  fn extent(&self) -> (f64, f64) {
    self.extent
  }

  fn name(&self) -> &'static str {
    if self.nested {
      "XTest into a nested compositor's X11 window"
    } else if self.touchscreen.is_some() {
      "XTest with Xorg's inputtest touchscreen"
    } else {
      "XTest"
    }
  }
}

impl X11 {
  /// The X11 leg's touchscreen.
  fn touchscreen(&mut self) -> Result<&mut Touchscreen> {
    let name = self.name();

    self
      .touchscreen
      .as_mut()
      .ok_or_else(|| format!("{name} sends no touch").into())
  }
}

/// A touchscreen of Xorg's inputtest driver, spoken to over its socket (the protocol of
/// `xf86-input-inputtest-protocol.h`, version 1.1): a touch posted there reaches clients as a
/// real touchscreen's XInput 2.2 touch, the first one's pointer emulation included.
struct Touchscreen {
  /// The driver's socket.
  socket: std::os::unix::net::UnixStream,
  /// The screen's size, which the device's axes span.
  extent: (f64, f64),
  /// Where each touch point down is, by its id.
  points: HashMap<u32, (f64, f64)>,
  /// Whether an event was sent since the last sync.
  sent: bool,
}

impl Touchscreen {
  /// `XI_TouchBegin`.
  const BEGIN: u32 = 18;
  /// `XI_TouchUpdate`.
  const UPDATE: u32 = 19;
  /// `XI_TouchEnd`.
  const END: u32 = 20;
  /// The largest value of a touch axis, which spans the screen.
  const AXIS: f64 = 65535.;

  /// Connects to the driver's socket at `path` and agrees on the protocol's version; `extent`
  /// is the screen's size.
  fn connect(path: &std::path::Path, extent: (f64, f64)) -> Result<Self> {
    use std::io::{Read as _, Write as _};

    let mut socket = std::os::unix::net::UnixStream::connect(path)
      .map_err(|error| format!("the inputtest touchscreen at {}: {error}", path.display()))?;

    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut hello = Vec::with_capacity(12);

    // The client's version: the header (length, type 0), major 1, minor 1.
    hello.extend_from_slice(&12_u32.to_ne_bytes());
    hello.extend_from_slice(&0_u32.to_ne_bytes());
    hello.extend_from_slice(&1_u16.to_ne_bytes());
    hello.extend_from_slice(&1_u16.to_ne_bytes());
    socket.write_all(&hello)?;

    let mut answer = [0; 12];

    socket.read_exact(&mut answer)?;

    if u32::from_ne_bytes([answer[4], answer[5], answer[6], answer[7]]) != 0 {
      return Err("the inputtest driver did not answer with its version".into());
    }

    Ok(Self {
      socket,
      extent,
      points: HashMap::new(),
      sent: false,
    })
  }

  /// Posts a touch event of `kind` for touch point `id` at `at`, in the screen's pixels.
  fn touch(&mut self, kind: u32, id: u32, at: (f64, f64)) -> Result<()> {
    use std::io::Write as _;

    // The event: its header (length, type 6), the touch's id and type, then the valuator data
    // (whether unaccelerated values follow, a mask of 64 bits, four bytes of padding, 64
    // values, 64 unaccelerated ones): the axes 0 and 1 are the position.
    let mut event = Vec::with_capacity(1056);
    let mut values = [0_f64; 64];

    values[0] = at.0 / (self.extent.0 - 1.).max(1.) * Self::AXIS;
    values[1] = at.1 / (self.extent.1 - 1.).max(1.) * Self::AXIS;

    event.extend_from_slice(&1056_u32.to_ne_bytes());
    event.extend_from_slice(&6_u32.to_ne_bytes());
    event.extend_from_slice(&id.to_ne_bytes());
    event.extend_from_slice(&kind.to_ne_bytes());
    event.extend_from_slice(&0_u32.to_ne_bytes());
    event.extend_from_slice(&[0b11, 0, 0, 0, 0, 0, 0, 0]);
    event.extend_from_slice(&[0; 4]);

    for value in values.into_iter().chain([0.; 64]) {
      event.extend_from_slice(&value.to_ne_bytes());
    }

    self.socket.write_all(&event)?;
    self.sent = true;

    if kind == Self::END {
      self.points.remove(&id);
    } else {
      self.points.insert(id, at);
    }

    Ok(())
  }

  /// Waits until the server has posted every event sent (the driver answers a wait only once
  /// it has events to drain).
  fn sync(&mut self) -> Result<()> {
    use std::io::{Read as _, Write as _};

    if !std::mem::take(&mut self.sent) {
      return Ok(());
    }

    let mut wait = Vec::with_capacity(8);

    wait.extend_from_slice(&8_u32.to_ne_bytes());
    wait.extend_from_slice(&1_u32.to_ne_bytes());
    self.socket.write_all(&wait)?;

    let mut answer = [0; 8];

    self.socket.read_exact(&mut answer)?;

    Ok(())
  }
}

const REMOTE_DESKTOP: &str = "org.gnome.Mutter.RemoteDesktop";
const SCREEN_CAST: &str = "org.gnome.Mutter.ScreenCast";

/// mutter's RemoteDesktop session and its ScreenCast stream of the monitor.
struct Mutter {
  conn: Connection,
  session: OwnedObjectPath,
  stream: OwnedObjectPath,
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
      stream,
      mapping,
    })
  }

  /// Calls the RemoteDesktop session's `method` with `arguments`.
  fn notify<B: serde::Serialize + zbus::zvariant::DynamicType>(
    &self,
    method: &str,
    arguments: &B,
  ) -> Result<()> {
    self.conn.call_method(
      Some(REMOTE_DESKTOP),
      self.session.as_str(),
      Some(&*format!("{REMOTE_DESKTOP}.Session")),
      method,
      arguments,
    )?;

    Ok(())
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
  /// The keyboard device, whose keymap the keys pressed through the session follow.
  keyboard: Option<reis::event::Device>,
  /// The touchscreen device, where the compositor offers one.
  touch: Option<reis::event::Device>,
  /// The devices resumed.
  resumed: Vec<reis::event::Device>,
  /// Where each keysym is on the keymap.
  places: HashMap<u32, Place>,
  /// The absolute region aimed in, in the compositor's logical coordinates.
  region: (f64, f64, f64, f64),
  /// The scale of the monitor the region covers.
  region_scale: f64,
  /// When the client started: the events' timestamps count from it.
  started: Instant,
  /// The last emulation sequence number.
  sequence: u32,
  /// Whether events went out on EIS since mutter last said it had taken them.
  unsynced: bool,
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
      touch: None,
      resumed: Vec::new(),
      places: HashMap::new(),
      region: (0., 0., 0., 0.),
      region_scale: 1.,
      started: Instant::now(),
      sequence: 0,
      unsynced: false,
    };
    let started = Instant::now();

    loop {
      ei.pump(20)?;

      let ready = |device: &Option<reis::event::Device>| {
        device
          .as_ref()
          .is_some_and(|device| ei.resumed.contains(device))
      };

      if ready(&ei.absolute)
        && ready(&ei.keyboard)
        && (ready(&ei.touch) || started.elapsed() > Duration::from_millis(500))
      {
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
    ei.region_scale = f64::from(region.scale);
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
              | reis::event::DeviceCapability::Pointer
              | reis::event::DeviceCapability::Touch,
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

          if device.has_capability(reis::event::DeviceCapability::Touch) {
            self.touch = Some(device.clone());
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

          if self.touch.as_ref() == Some(&removed.device) {
            self.touch = None;
          }
        }
        _ => {}
      }
    }

    self.context.flush()?;

    Ok(())
  }

  fn pointer(&self) -> Result<reis::event::Device> {
    self
      .absolute
      .clone()
      .ok_or_else(|| "mutter's EIS removed the pointer".into())
  }

  fn frame(&mut self, device: &reis::event::Device) -> Result<()> {
    device.device().frame(
      self.converter.connection().serial(),
      self.started.elapsed().as_micros() as u64,
    );
    self.context.flush()?;
    self.unsynced = true;

    Ok(())
  }

  /// Waits until mutter has taken every event sent on EIS, so a request on the session's
  /// D-Bus after them arrives after them.
  fn roundtrip(&mut self) -> Result<()> {
    if !self.unsynced {
      return Ok(());
    }

    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let callback = self.converter.connection().connection().sync(1);

    {
      let done = std::rc::Rc::clone(&done);

      self
        .converter
        .add_callback_handler(callback, move |_| done.set(true));
    }

    self.context.flush()?;

    let started = Instant::now();

    while !done.get() {
      if started.elapsed() > Duration::from_secs(2) {
        return Err("mutter's EIS never answered a sync".into());
      }

      self.pump(20)?;
    }

    self.unsynced = false;

    Ok(())
  }

  /// The point `x`, `y` of the pointer's space (the monitor's logical pixels) as a touch
  /// request on the monitor's stream names it: mutter 46 divides a stream's touch position by
  /// the monitor's scale, though the stream it reports is the monitor's logical size.
  fn in_stream(&self, x: f64, y: f64) -> (f64, f64) {
    (x * self.region_scale, y * self.region_scale)
  }

  /// Calls the RemoteDesktop session's `method` with `arguments` once mutter has taken every
  /// event sent on EIS before it.
  fn notify<B: serde::Serialize + zbus::zvariant::DynamicType>(
    &mut self,
    method: &str,
    arguments: &B,
  ) -> Result<()> {
    self.roundtrip()?;
    self.mutter.notify(method, arguments)
  }
}

impl Input for Ei {
  fn move_to(&mut self, x: f64, y: f64) -> Result<()> {
    self.pump(0)?;

    let device = self.pointer()?;
    let pointer = device
      .interface::<reis::ei::PointerAbsolute>()
      .ok_or("no absolute pointer")?;

    pointer.motion_absolute((self.region.0 + x) as f32, (self.region.1 + y) as f32);
    self.frame(&device)
  }

  fn button(&mut self, button: u32, down: bool) -> Result<()> {
    let device = self.pointer()?;
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
    self.notify("NotifyKeyboardKeycode", &(code, down))
  }

  fn place(&mut self, keysym: u32) -> Option<Place> {
    self.places.get(&keysym).copied()
  }

  fn wheel(&mut self, right: i32, down: i32) -> Result<()> {
    let device = self.pointer()?;
    let scroll = device.interface::<reis::ei::Scroll>().ok_or("no scroll")?;

    scroll.scroll_discrete(right * 120, down * 120);
    self.frame(&device)
  }

  fn finger_scroll(&mut self, right: f64, down: f64) -> Result<()> {
    let device = self.pointer()?;
    let scroll = device.interface::<reis::ei::Scroll>().ok_or("no scroll")?;

    scroll.scroll(right as f32, down as f32);
    self.frame(&device)
  }

  fn finger_scroll_stop(&mut self) -> Result<()> {
    let device = self.pointer()?;
    let scroll = device.interface::<reis::ei::Scroll>().ok_or("no scroll")?;

    scroll.scroll_stop(1, 1, 0);
    self.frame(&device)
  }

  // mutter 46's EIS offers no touchscreen; its RemoteDesktop session's own touch requests,
  // aimed in the monitor's stream, stand in where it offers none.
  fn touch_down(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    self.pump(0)?;

    let Some(device) = self.touch.clone() else {
      let stream = self.mutter.stream.to_string();
      let (x, y) = self.in_stream(x, y);

      return self.notify("NotifyTouchDown", &(stream, id, x, y));
    };
    let touch = device
      .interface::<reis::ei::Touchscreen>()
      .ok_or("no touchscreen")?;

    touch.down(id, (self.region.0 + x) as f32, (self.region.1 + y) as f32);
    self.frame(&device)
  }

  fn touch_motion(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    let Some(device) = self.touch.clone() else {
      let stream = self.mutter.stream.to_string();
      let (x, y) = self.in_stream(x, y);

      return self.notify("NotifyTouchMotion", &(stream, id, x, y));
    };
    let touch = device
      .interface::<reis::ei::Touchscreen>()
      .ok_or("no touchscreen")?;

    touch.motion(id, (self.region.0 + x) as f32, (self.region.1 + y) as f32);
    self.frame(&device)
  }

  fn touch_up(&mut self, id: u32) -> Result<()> {
    let Some(device) = self.touch.clone() else {
      return self.notify("NotifyTouchUp", &(id,));
    };
    let touch = device
      .interface::<reis::ei::Touchscreen>()
      .ok_or("no touchscreen")?;

    touch.up(id);
    self.frame(&device)
  }

  fn has_touch(&self) -> bool {
    true
  }

  fn has_finger_scroll(&self) -> bool {
    true
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

  fn name(&self) -> &'static str {
    "mutter's EIS (keys, and touch where it has none, through its RemoteDesktop session)"
  }
}

/// sway's virtual pointer and keyboard on a connection of the test's own, and where sway runs
/// on its X11 backend inside the runner's Xorg, that server's inputtest touchscreen, whose
/// touches reach sway as its X11 backend's touch.
pub struct Sway {
  /// The connection the devices are made on.
  _conn: wayland_client::Connection,
  /// Its event queue, which a round trip waits on.
  queue: wayland_client::EventQueue<Devices>,
  /// The virtual pointer.
  pointer: wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
  /// The virtual keyboard.
  keyboard: wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
  /// Where each keysym is in the keyboard's keymap, by keysym.
  places: HashMap<u32, Place>,
  /// The output's size in device pixels, which absolute motion spans.
  extent: (u32, u32),
  /// The output's scale, from logical to device pixels.
  scale: f64,
  /// When the devices were made, which the events' times count from.
  started: Instant,
  /// The modifiers held, as the keyboard reports them.
  modifiers: u32,
  /// The keyboard's keymap, kept open while the compositor may read it.
  _keymap: std::fs::File,
  /// The touchscreen of the X server sway's output is a window of, filling its screen.
  touchscreen: Option<Touchscreen>,
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
wayland_client::delegate_noop!(Devices: ignore wayland_client::protocol::wl_output::WlOutput);
wayland_client::delegate_noop!(Devices: ignore wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1);
wayland_client::delegate_noop!(Devices: ignore wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1);
wayland_client::delegate_noop!(Devices: ignore wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1);
wayland_client::delegate_noop!(Devices: ignore wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1);

impl Sway {
  /// Creates a virtual pointer and a virtual keyboard (a US layout, Compose on the right
  /// Control key) on `WAYLAND_DISPLAY`'s seat; `extent` is the output's size in device pixels,
  /// which absolute motion spans, and `scale` its scale: the pointer moves in its logical
  /// pixels, sent in device pixels so a fractional logical position stays exact. The pointer is
  /// the session's first output's alone, so its motion spans that output wherever a test puts
  /// more beside it. Where the runner names an inputtest touchscreen (`SKENE_TOUCH_SOCKET`),
  /// sway's output is a window filling that X server's screen, and touches go there.
  ///
  /// # Errors
  ///
  /// Where the compositor offers no virtual pointer or keyboard, or the touchscreen named
  /// cannot be reached.
  pub fn new(extent: (u32, u32), scale: f64) -> Result<Self> {
    use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
    use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;

    let conn = wayland_client::Connection::connect_to_env()?;
    let (globals, mut queue) = wayland_client::globals::registry_queue_init::<Devices>(&conn)?;
    let qh = queue.handle();
    let seat: wayland_client::protocol::wl_seat::WlSeat = globals.bind(&qh, 1..=1, ())?;
    let pointers: ZwlrVirtualPointerManagerV1 = globals.bind(&qh, 1..=2, ())?;
    let keyboards: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let output: Option<wayland_client::protocol::wl_output::WlOutput> =
      globals.bind(&qh, 1..=1, ()).ok();
    let pointer = match &output {
      Some(output) if wayland_client::Proxy::version(&pointers) >= 2 => {
        pointers.create_virtual_pointer_with_output(Some(&seat), Some(output), &qh, ())
      }
      _ => pointers.create_virtual_pointer(Some(&seat), &qh, ()),
    };
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

    let touchscreen = match std::env::var_os("SKENE_TOUCH_SOCKET") {
      Some(path) => Some(Touchscreen::connect(
        path.as_ref(),
        (f64::from(extent.0), f64::from(extent.1)),
      )?),
      None => None,
    };

    Ok(Self {
      _conn: conn,
      queue,
      pointer,
      keyboard,
      places: places(&keymap),
      extent,
      scale,
      started: Instant::now(),
      modifiers: 0,
      _keymap: file,
      touchscreen,
    })
  }

  /// The touchscreen, where sway's output has one.
  fn touchscreen(&mut self) -> Result<&mut Touchscreen> {
    let name = self.name();

    self
      .touchscreen
      .as_mut()
      .ok_or_else(|| format!("{name} sends no touch").into())
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
      (x * self.scale).max(0.).round() as u32,
      (y * self.scale).max(0.).round() as u32,
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

  fn finger_scroll(&mut self, right: f64, down: f64) -> Result<()> {
    use wayland_client::protocol::wl_pointer::{Axis, AxisSource};

    let time = self.time();

    self.pointer.axis_source(AxisSource::Finger);

    for (axis, value) in [
      (Axis::VerticalScroll, down),
      (Axis::HorizontalScroll, right),
    ] {
      if value != 0. {
        self.pointer.axis(time, axis, value);
      }
    }

    self.pointer.frame();

    Ok(())
  }

  fn finger_scroll_stop(&mut self) -> Result<()> {
    use wayland_client::protocol::wl_pointer::{Axis, AxisSource};

    let time = self.time();

    self.pointer.axis_source(AxisSource::Finger);
    self.pointer.axis_stop(time, Axis::VerticalScroll);
    self.pointer.frame();

    Ok(())
  }

  fn has_finger_scroll(&self) -> bool {
    true
  }

  fn touch_down(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    let scale = self.scale;

    self
      .touchscreen()?
      .touch(Touchscreen::BEGIN, id, (x * scale, y * scale))
  }

  fn touch_motion(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    let scale = self.scale;

    self
      .touchscreen()?
      .touch(Touchscreen::UPDATE, id, (x * scale, y * scale))
  }

  fn touch_up(&mut self, id: u32) -> Result<()> {
    let touchscreen = self.touchscreen()?;
    let at = touchscreen.points.get(&id).copied().unwrap_or_default();

    touchscreen.touch(Touchscreen::END, id, at)
  }

  fn has_touch(&self) -> bool {
    self.touchscreen.is_some()
  }

  fn sync(&mut self) -> Result<()> {
    if let Some(touchscreen) = self.touchscreen.as_mut() {
      touchscreen.sync()?;
    }

    self.queue.roundtrip(&mut Devices)?;

    Ok(())
  }

  fn window_origin(&mut self) -> Option<(f64, f64)> {
    None
  }

  fn extent(&self) -> (f64, f64) {
    (
      f64::from(self.extent.0) / self.scale,
      f64::from(self.extent.1) / self.scale,
    )
  }

  fn landing(&self, x: f64, y: f64) -> (f64, f64) {
    (
      (x * self.scale).max(0.).round() / self.scale,
      (y * self.scale).max(0.).round() / self.scale,
    )
  }

  fn name(&self) -> &'static str {
    if self.touchscreen.is_some() {
      "sway's virtual pointer and keyboard, and Xorg's inputtest touchscreen under sway's X11 backend"
    } else {
      "sway's virtual pointer and keyboard"
    }
  }
}

/// KWin's fake input, through `kwin-fake-input`, a helper the driver starts (the runner names it,
/// `SKENE_KWIN_FAKE_INPUT`): KWin grants the restricted interface to that executable alone, so
/// the test's own connections are refused it unhashed. A pointer, a keyboard and a touchscreen,
/// aimed in KWin's logical coordinates.
pub struct Kwin {
  helper: std::process::Child,
  commands: std::process::ChildStdin,
  answers: std::io::BufReader<std::process::ChildStdout>,
  places: HashMap<u32, Place>,
  extent: (f64, f64),
  pointer: (f64, f64),
}

impl Kwin {
  /// Starts the helper on `WAYLAND_DISPLAY` and waits until KWin took its authentication;
  /// `extent` is the output's size in its logical pixels, which the pointer moves in. The
  /// keymap is KWin's: the US layout, Compose on the right Control key (the runner's
  /// configuration home).
  ///
  /// # Errors
  ///
  /// Where the runner names no helper, or the helper cannot reach KWin's fake input.
  pub fn new(extent: (f64, f64)) -> Result<Self> {
    use std::{
      io::BufRead as _,
      process::{Command, Stdio},
    };

    let program = std::env::var_os("SKENE_KWIN_FAKE_INPUT").ok_or(
      "the KWin legs act through kwin-fake-input, which the runner names (SKENE_KWIN_FAKE_INPUT)",
    )?;
    let mut helper = Command::new(&program)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .spawn()
      .map_err(|error| format!("{}: {error}", program.to_string_lossy()))?;
    let commands = helper.stdin.take().ok_or("the helper's input")?;
    let mut answers = std::io::BufReader::new(helper.stdout.take().ok_or("the helper's output")?);
    let mut ready = String::new();

    answers.read_line(&mut ready)?;

    if ready.trim() != "ready" {
      let _ = helper.wait();

      return Err(format!("{} did not start", program.to_string_lossy()).into());
    }

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

    Ok(Self {
      helper,
      commands,
      answers,
      places: places(&keymap),
      extent,
      pointer: (0., 0.),
    })
  }

  /// Sends the helper `command`.
  fn send(&mut self, command: std::fmt::Arguments<'_>) -> Result<()> {
    use std::io::Write as _;

    self.commands.write_fmt(command)?;
    self.commands.write_all(b"\n")?;

    Ok(())
  }
}

impl Drop for Kwin {
  fn drop(&mut self) {
    let _ = self.helper.kill();
    let _ = self.helper.wait();
  }
}

impl Input for Kwin {
  fn move_to(&mut self, x: f64, y: f64) -> Result<()> {
    self.pointer = (x, y);
    self.send(format_args!("motion {x} {y}"))
  }

  fn button(&mut self, button: u32, down: bool) -> Result<()> {
    self.send(format_args!("button {button} {}", u32::from(down)))
  }

  fn key(&mut self, code: u32, down: bool) -> Result<()> {
    self.send(format_args!("key {code} {}", u32::from(down)))
  }

  fn place(&mut self, keysym: u32) -> Option<Place> {
    self.places.get(&keysym).copied()
  }

  fn wheel(&mut self, right: i32, down: i32) -> Result<()> {
    // KWin passes a fake axis on as a wheel's: 15 pixels a detent, as libinput reports one, an
    // event a detent as a wheel turns.
    for (axis, count) in [(0, down), (1, right)] {
      for _ in 0..count.unsigned_abs() {
        self.send(format_args!(
          "axis {axis} {}",
          f64::from(count.signum()) * 15.
        ))?;
      }
    }

    Ok(())
  }

  fn touch_down(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    self.send(format_args!("touch-down {id} {x} {y}"))
  }

  fn touch_motion(&mut self, id: u32, x: f64, y: f64) -> Result<()> {
    self.send(format_args!("touch-motion {id} {x} {y}"))
  }

  fn touch_up(&mut self, id: u32) -> Result<()> {
    self.send(format_args!("touch-up {id}"))
  }

  fn has_touch(&self) -> bool {
    true
  }

  fn sync(&mut self) -> Result<()> {
    use std::io::BufRead as _;

    self.send(format_args!("sync"))?;

    let mut answer = String::new();

    self.answers.read_line(&mut answer)?;

    match answer.trim() {
      "ok" => Ok(()),
      "" => Err("kwin-fake-input ended".into()),
      other => Err(other.to_owned().into()),
    }
  }

  fn window_origin(&mut self) -> Option<(f64, f64)> {
    None
  }

  fn extent(&self) -> (f64, f64) {
    self.extent
  }

  fn name(&self) -> &'static str {
    "KWin's fake input"
  }
}
