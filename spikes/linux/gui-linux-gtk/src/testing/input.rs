//! Real input through the X server: XTest events from a connection of the test process's own
//! (the same trusted events `xdotool` sends), at root coordinates.

use std::collections::HashMap;

use x11rb::{
  connection::Connection as _,
  protocol::{
    xproto::{
      BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, ConnectionExt as _, KEY_PRESS_EVENT,
      KEY_RELEASE_EVENT, MOTION_NOTIFY_EVENT,
    },
    xtest::ConnectionExt as _,
  },
  rust_connection::RustConnection,
};

/// A failed request, as a message.
pub type Result<T> = std::result::Result<T, String>;

/// The keysyms of the modifiers, in the order a chord holds them.
pub mod keysym {
  /// `Shift_L`.
  pub const SHIFT: u32 = 0xffe1;
  /// `Control_L`.
  pub const CONTROL: u32 = 0xffe3;
  /// `Alt_L`.
  pub const ALT: u32 = 0xffe9;
  /// `Super_L`.
  pub const SUPER: u32 = 0xffeb;
}

/// An X connection that sends XTest input.
pub struct Input {
  connection: RustConnection,
  root: u32,
  keys: HashMap<u32, (u8, bool)>,
}

impl Input {
  /// Connects to `$DISPLAY` and reads its keyboard map.
  ///
  /// # Errors
  ///
  /// Where the display cannot be reached or has no XTest.
  pub fn connect() -> Result<Self> {
    let (connection, screen) = x11rb::connect(None).map_err(|error| error.to_string())?;
    let setup = connection.setup();
    let root = setup.roots[screen].root;
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let map = connection
      .get_keyboard_mapping(min, max - min + 1)
      .map_err(|error| error.to_string())?
      .reply()
      .map_err(|error| error.to_string())?;
    let per = usize::from(map.keysyms_per_keycode);
    let mut keys = HashMap::new();

    for (index, syms) in map.keysyms.chunks(per).enumerate() {
      let code = min + u8::try_from(index).unwrap_or(0);

      for (level, sym) in syms.iter().take(2).enumerate() {
        if *sym != 0 {
          keys.entry(*sym).or_insert((code, level == 1));
        }
      }
    }

    connection
      .xtest_get_version(2, 2)
      .map_err(|error| error.to_string())?
      .reply()
      .map_err(|error| format!("the display has no XTest: {error}"))?;

    Ok(Self {
      connection,
      root,
      keys,
    })
  }

  /// The root coordinates of the X window `window`'s origin.
  ///
  /// # Errors
  ///
  /// Where the window is gone.
  pub fn origin(&self, window: u32) -> Result<(i32, i32)> {
    let reply = self
      .connection
      .translate_coordinates(window, self.root, 0, 0)
      .map_err(|error| error.to_string())?
      .reply()
      .map_err(|error| error.to_string())?;

    Ok((i32::from(reply.dst_x), i32::from(reply.dst_y)))
  }

  /// Waits until the server has handled every request sent so far.
  ///
  /// # Errors
  ///
  /// Where the connection broke.
  pub fn sync(&self) -> Result<()> {
    self
      .connection
      .get_input_focus()
      .map_err(|error| error.to_string())?
      .reply()
      .map(drop)
      .map_err(|error| error.to_string())
  }

  fn fake(&self, kind: u8, detail: u8, x: i32, y: i32) -> Result<()> {
    let clamp = |value: i32| i16::try_from(value).unwrap_or(0);

    self
      .connection
      .xtest_fake_input(kind, detail, 0, self.root, clamp(x), clamp(y), 0)
      .map(drop)
      .map_err(|error| error.to_string())
  }

  /// Moves the pointer to root `(x, y)`.
  ///
  /// # Errors
  ///
  /// Where the connection broke.
  pub fn move_to(&self, x: i32, y: i32) -> Result<()> {
    self.fake(MOTION_NOTIFY_EVENT, 0, x, y)?;
    self.sync()
  }

  /// Moves the pointer to root `(x, y)` and presses and releases `button` there (1 the
  /// primary, 4 and 5 the wheel up and down, 6 and 7 left and right).
  ///
  /// # Errors
  ///
  /// Where the connection broke.
  pub fn click(&self, x: i32, y: i32, button: u8) -> Result<()> {
    self.fake(MOTION_NOTIFY_EVENT, 0, x, y)?;
    self.fake(BUTTON_PRESS_EVENT, button, 0, 0)?;
    self.fake(BUTTON_RELEASE_EVENT, button, 0, 0)?;
    self.sync()
  }

  /// Presses the primary button at root `from`, moves the pointer to root `to` in steps, and
  /// releases it there.
  ///
  /// # Errors
  ///
  /// Where the connection broke.
  pub fn drag(&self, from: (i32, i32), to: (i32, i32)) -> Result<()> {
    const STEPS: i32 = 8;

    self.fake(MOTION_NOTIFY_EVENT, 0, from.0, from.1)?;
    self.fake(BUTTON_PRESS_EVENT, 1, 0, 0)?;
    self.sync()?;

    for step in 1..=STEPS {
      self.fake(
        MOTION_NOTIFY_EVENT,
        0,
        from.0 + (to.0 - from.0) * step / STEPS,
        from.1 + (to.1 - from.1) * step / STEPS,
      )?;
      self.sync()?;
    }

    self.fake(BUTTON_RELEASE_EVENT, 1, 0, 0)?;
    self.sync()
  }

  /// The key producing `sym`, and whether it takes Shift.
  fn code(&self, sym: u32) -> Result<(u8, bool)> {
    self
      .keys
      .get(&sym)
      .copied()
      .ok_or_else(|| format!("no key on this keyboard types keysym {sym:#x}"))
  }

  /// Whether a key types `sym`.
  #[must_use]
  pub fn types(&self, sym: u32) -> bool {
    self.keys.contains_key(&sym)
  }

  /// Presses and releases the key for keysym `sym`, the keys of `modifiers` held (and Shift
  /// where the keysym is on the key's shifted level).
  ///
  /// # Errors
  ///
  /// Where no key produces `sym`.
  pub fn key(&self, sym: u32, modifiers: &[u32]) -> Result<()> {
    let (code, shifted) = self.code(sym)?;
    let mut held = Vec::new();

    for modifier in modifiers {
      held.push(self.code(*modifier)?.0);
    }

    if shifted {
      let shift = self.code(keysym::SHIFT)?.0;

      if !held.contains(&shift) {
        held.push(shift);
      }
    }

    for key in &held {
      self.fake(KEY_PRESS_EVENT, *key, 0, 0)?;
    }

    self.fake(KEY_PRESS_EVENT, code, 0, 0)?;
    self.fake(KEY_RELEASE_EVENT, code, 0, 0)?;

    for key in held.iter().rev() {
      self.fake(KEY_RELEASE_EVENT, *key, 0, 0)?;
    }

    self.sync()
  }
}
