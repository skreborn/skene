//! The desktop's XSETTINGS in a test on X11: a stand-in manager on the test's X display, which
//! owns `_XSETTINGS_S<screen>` and publishes the settings it is given; the app reads and
//! follows them through its real path, as it reads a desktop's settings daemon's.
//!
//! A window opened while it serves reads its settings; a window open when they change follows
//! them. Dropping it gives the selection up, to the leg's desktop where it has one.
//!
//! A leg whose desktop publishes settings of its own ([`desktop`]: `Xft/DPI` where KWin's
//! Xwayland leaves X11 apps to scale themselves) has them served for the whole test process
//! ([`serve_desktop`]), as a desktop's settings daemon serves them; a test's own manager
//! publishes them beneath its own.

use std::cell::RefCell;

use x11rb::{
  connection::Connection as _,
  protocol::xproto::{
    ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, WindowClass,
  },
  rust_connection::RustConnection,
  wrapper::ConnectionExt as _,
};

use super::input::Result;

/// A setting's value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Value {
  /// An integer (`Xft/Antialias`, `Xft/DPI`).
  Integer(i32),
  /// A string (`Xft/HintStyle`, `Net/ThemeName`).
  Text(&'static str),
}

thread_local! {
  /// The leg's desktop's manager, serving while no test's own does.
  static DESKTOP: RefCell<Option<Xsettings>> = const { RefCell::new(None) };
}

/// The settings the leg's desktop publishes: on KWin's Xwayland scaled, `Xft/DPI` at the
/// leg's scale (X11 apps scale themselves there, as Plasma's settings daemon tells them);
/// none elsewhere.
#[must_use]
pub fn desktop() -> Vec<(&'static str, Value)> {
  let scale = super::Session::scale();

  if super::Session::current() == Some(super::Session::KwinX11) && scale != 1. {
    #[expect(clippy::cast_possible_truncation, reason = "a DPI in 1024ths")]
    let dpi = (scale * 96. * 1024.).round() as i32;

    vec![("Xft/DPI", Value::Integer(dpi))]
  } else {
    Vec::new()
  }
}

/// Serves the leg's desktop settings ([`desktop`]) where it has any and no manager of the
/// process serves them yet: before the test's first window opens, and again once a test's own
/// manager goes.
///
/// # Panics
///
/// Where the leg has settings and the X display cannot take them.
pub fn serve_desktop() {
  let settings = desktop();

  if settings.is_empty() {
    return;
  }

  DESKTOP.with(|desktop| {
    let mut desktop = desktop.borrow_mut();

    if desktop.is_none() {
      let mut manager = Xsettings::take(&settings)
        .unwrap_or_else(|error| panic!("the leg's desktop settings: {error}"));

      manager.desktop = true;
      *desktop = Some(manager);
    }
  });
}

/// The stand-in, serving while it lives.
pub struct Xsettings {
  conn: RustConnection,
  window: u32,
  property: u32,
  serial: u32,
  /// Whether it is the leg's desktop's own manager.
  desktop: bool,
}

impl Xsettings {
  /// Serves `settings` on `DISPLAY` as its XSETTINGS manager, above the leg's desktop's
  /// ([`desktop`]).
  ///
  /// # Errors
  ///
  /// Where there is no X display, or the selection cannot be taken.
  pub fn serve(settings: &[(&str, Value)]) -> Result<Self> {
    let mut all: Vec<(&str, Value)> = settings.to_vec();

    for (name, value) in desktop() {
      if !all.iter().any(|(set, _)| *set == name) {
        all.push((name, value));
      }
    }

    // The desktop's manager gives the selection up to this one.
    DESKTOP.with(|desktop| desktop.borrow_mut().take());

    Self::take(&all)
  }

  /// Takes the selection and publishes `settings` as they are.
  fn take(settings: &[(&str, Value)]) -> Result<Self> {
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let window = conn.generate_id()?;

    conn.create_window(
      0,
      window,
      root,
      0,
      0,
      1,
      1,
      0,
      WindowClass::INPUT_ONLY,
      0,
      &CreateWindowAux::new(),
    )?;

    let selection = conn
      .intern_atom(false, format!("_XSETTINGS_S{screen}").as_bytes())?
      .reply()?
      .atom;
    let property = conn
      .intern_atom(false, b"_XSETTINGS_SETTINGS")?
      .reply()?
      .atom;
    let manager = conn.intern_atom(false, b"MANAGER")?.reply()?.atom;
    let mut xsettings = Self {
      conn,
      window,
      property,
      serial: 0,
      desktop: false,
    };

    xsettings.set(settings)?;
    xsettings
      .conn
      .set_selection_owner(window, selection, x11rb::CURRENT_TIME)?;

    // The manager's arrival, as the XSETTINGS specification announces it.
    let announcement = ClientMessageEvent::new(32, root, manager, [0, selection, window, 0, 0]);

    xsettings
      .conn
      .send_event(false, root, EventMask::STRUCTURE_NOTIFY, announcement)?;
    xsettings.conn.sync()?;

    Ok(xsettings)
  }

  /// Publishes `settings` in place of those published before.
  ///
  /// # Errors
  ///
  /// Where the X server cannot be reached.
  pub fn set(&mut self, settings: &[(&str, Value)]) -> Result<()> {
    let data = Self::encode(settings, self.serial);

    self.serial += 1;
    self.conn.change_property8(
      PropMode::REPLACE,
      self.window,
      self.property,
      self.property,
      &data,
    )?;
    self.conn.sync()?;

    Ok(())
  }

  /// `settings` as the XSETTINGS property holds them, little-endian, at `serial`.
  fn encode(settings: &[(&str, Value)], serial: u32) -> Vec<u8> {
    let pad = |data: &mut Vec<u8>| {
      while !data.len().is_multiple_of(4) {
        data.push(0);
      }
    };
    let mut data = vec![0, 0, 0, 0];

    data.extend_from_slice(&serial.to_le_bytes());
    data.extend_from_slice(&(settings.len() as u32).to_le_bytes());

    for (name, value) in settings {
      data.push(match value {
        Value::Integer(_) => 0,
        Value::Text(_) => 1,
      });
      data.push(0);
      data.extend_from_slice(&(name.len() as u16).to_le_bytes());
      data.extend_from_slice(name.as_bytes());
      pad(&mut data);
      data.extend_from_slice(&0_u32.to_le_bytes());

      match value {
        Value::Integer(integer) => data.extend_from_slice(&integer.to_le_bytes()),
        Value::Text(text) => {
          data.extend_from_slice(&(text.len() as u32).to_le_bytes());
          data.extend_from_slice(text.as_bytes());
          pad(&mut data);
        }
      }
    }

    data
  }
}

impl Drop for Xsettings {
  fn drop(&mut self) {
    let _ = self.conn.destroy_window(self.window);
    let _ = self.conn.sync();

    // A test's own manager hands the selection back to the leg's desktop.
    if !self.desktop {
      let _ = DESKTOP.try_with(|_| serve_desktop());
    }
  }
}
