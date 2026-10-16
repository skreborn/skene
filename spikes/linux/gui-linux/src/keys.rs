//! The keyboard through xkbcommon, shared by both backends: a keymap and its state (from the
//! compositor's keymap on Wayland, from the server's XKB device on X11), the locale's compose
//! table for dead keys and `Multi_key` sequences, the repeat rate, and each key named as a
//! browser names it (`key` and `code`).

use std::time::Duration;

use skene_foundation::seam::report::{KeyEvent, Modifiers};
use xkbcommon::xkb::{self, compose, keysyms};

/// One key, after layout, modifiers and compose: a Skene key event and the text it types.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyInput {
  /// The event, named as a browser names it.
  pub event: KeyEvent,
  /// The text the press types, after compose; `None` for a control key, a dead key in
  /// progress or a cancelled sequence.
  pub text: Option<String>,
  /// The raw keycode (xkb numbering: evdev + 8).
  pub keycode: u32,
  /// Where the press continues a compose sequence (a dead key, `Multi_key`): the sequence so
  /// far as a composition shows it.
  pub composing: Option<String>,
  /// Whether the press ended a compose sequence, composed (its text in `text`) or cancelled.
  pub composed: bool,
}

/// A keymap, its state, the compose state and the repeat settings.
pub struct Keyboard {
  /// The xkb context.
  pub context: xkb::Context,
  keymap: Option<xkb::Keymap>,
  state: Option<xkb::State>,
  compose: Option<compose::State>,
  sequence: String,
  /// Keys per second a held key repeats at, and the delay before it starts; no repeat where
  /// the rate is 0.
  pub repeat: (u32, Duration),
}

impl Default for Keyboard {
  fn default() -> Self {
    Self::new()
  }
}

impl Keyboard {
  /// A keyboard with no keymap yet, and the compose table of the current locale.
  pub fn new() -> Self {
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
      .into_iter()
      .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
      .unwrap_or_else(|| "C".into());
    let table = compose::Table::new_from_locale(
      &context,
      std::ffi::OsStr::new(&locale),
      compose::COMPILE_NO_FLAGS,
    )
    .or_else(|()| {
      compose::Table::new_from_locale(
        &context,
        std::ffi::OsStr::new("en_US.UTF-8"),
        compose::COMPILE_NO_FLAGS,
      )
    })
    .ok();

    Self {
      context,
      keymap: None,
      state: None,
      compose: table.map(|table| compose::State::new(&table, compose::STATE_NO_FLAGS)),
      sequence: String::new(),
      repeat: (25, Duration::from_millis(600)),
    }
  }

  /// Installs a keymap and a fresh state for it.
  pub fn set_keymap(&mut self, keymap: xkb::Keymap, state: xkb::State) {
    self.keymap = Some(keymap);
    self.state = Some(state);
  }

  /// The names of the keymap's layouts.
  pub fn layouts(&self) -> Vec<String> {
    self.keymap.as_ref().map_or_else(Vec::new, |keymap| {
      (0..keymap.num_layouts())
        .map(|index| keymap.layout_get_name(index).to_owned())
        .collect()
    })
  }

  /// Applies a modifier and group update (Wayland's `modifiers`, X11's `XkbStateNotify`).
  pub fn update_mask(&mut self, mods: [u32; 3], groups: [u32; 3]) {
    if let Some(state) = &mut self.state {
      state.update_mask(mods[0], mods[1], mods[2], groups[0], groups[1], groups[2]);
    }
  }

  fn modifier(&self, name: &str) -> bool {
    self
      .state
      .as_ref()
      .is_some_and(|state| state.mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE))
  }

  /// The modifiers held now.
  pub fn modifiers(&self) -> Modifiers {
    Modifiers {
      shift: self.modifier(xkb::MOD_NAME_SHIFT),
      control: self.modifier(xkb::MOD_NAME_CTRL),
      alt: self.modifier(xkb::MOD_NAME_ALT),
      meta: self.modifier(xkb::MOD_NAME_LOGO),
    }
  }

  /// Whether a held `keycode` repeats.
  pub fn repeats(&self, keycode: u32) -> bool {
    self
      .keymap
      .as_ref()
      .is_some_and(|keymap| keymap.key_repeats(xkb::Keycode::new(keycode)))
  }

  /// Resolves `keycode` (xkb numbering) pressed or released into a key input, a press
  /// through compose.
  pub fn key(&mut self, keycode: u32, down: bool, repeat: bool) -> Option<KeyInput> {
    self.key_in(keycode, down, repeat, None)
  }

  /// Resolves `keycode` as [`Keyboard::key`] does, under the X core modifier mask `mask`
  /// where given (the mask its event carried: a key an input method gave back later reads the
  /// modifiers held when it was pressed, not those held now).
  pub fn key_in(
    &mut self,
    keycode: u32,
    down: bool,
    repeat: bool,
    mask: Option<u32>,
  ) -> Option<KeyInput> {
    let held = match (mask, &self.keymap) {
      (Some(mask), Some(keymap)) => {
        let mut state = xkb::State::new(keymap);

        // The core mask's modifier bits are XKB's real modifiers in order; bits 13 and 14
        // are the group.
        state.update_mask(mask & 0xff, 0, 0, 0, 0, (mask >> 13) & 0x3);

        Some(state)
      }
      _ => None,
    };
    let state = held.as_ref().or(self.state.as_ref())?;
    let code = xkb::Keycode::new(keycode);
    let keysym = state.key_get_one_sym(code);
    let mut text = Some(state.key_get_utf8(code)).filter(|text| !text.is_empty());

    let mut composing = None;
    let mut composed = false;

    if down && let Some(compose) = &mut self.compose {
      let accepted = compose.feed(keysym) == compose::FeedResult::Accepted;

      match compose.status() {
        compose::Status::Composing => {
          if accepted {
            self.sequence.push_str(&shown(keysym));
          }

          text = None;
          composing = Some(self.sequence.clone());
        }
        compose::Status::Composed => {
          text = compose.utf8();
          compose.reset();
          self.sequence.clear();
          composed = true;
        }
        compose::Status::Cancelled => {
          text = None;
          compose.reset();
          self.sequence.clear();
          composed = true;
        }
        compose::Status::Nothing => {}
      }
    }

    let text = text.filter(|text| !text.chars().any(char::is_control));
    let key = key_name(keysym.raw()).map_or_else(
      || {
        text
          .clone()
          .or_else(|| {
            let utf = xkb::keysym_to_utf8(keysym);

            Some(utf.trim_end_matches('\0').to_owned()).filter(|text| !text.is_empty())
          })
          .unwrap_or_else(|| "Unidentified".to_owned())
      },
      str::to_owned,
    );

    Some(KeyInput {
      event: KeyEvent {
        key,
        code: code_name(keycode.saturating_sub(8)).to_owned(),
        down,
        repeat,
        modifiers: match mask {
          Some(mask) => Modifiers {
            shift: mask & 0x1 != 0,
            control: mask & 0x4 != 0,
            alt: mask & 0x8 != 0,
            meta: mask & 0x40 != 0,
          },
          None => self.modifiers(),
        },
      },
      text: if down { text } else { None },
      keycode,
      composing,
      composed,
    })
  }

  /// The keysym `keycode` gives under the state in force, compose aside.
  pub fn keysym(&self, keycode: u32) -> u32 {
    self.state.as_ref().map_or(0, |state| {
      state.key_get_one_sym(xkb::Keycode::new(keycode)).raw()
    })
  }

  /// A keycode giving `keysym` under the state in force, where the keymap has one.
  pub fn keycode_for(&self, keysym: u32) -> Option<u32> {
    let keymap = self.keymap.as_ref()?;
    let state = self.state.as_ref()?;

    (keymap.min_keycode().raw()..=keymap.max_keycode().raw())
      .find(|code| state.key_get_one_sym(xkb::Keycode::new(*code)).raw() == keysym)
  }

  /// Updates the state for a key pressed or released (X11, where the server sends no
  /// modifier events of its own beside XKB's).
  pub fn update_key(&mut self, keycode: u32, down: bool) {
    if let Some(state) = &mut self.state {
      state.update_key(
        xkb::Keycode::new(keycode),
        if down {
          xkb::KeyDirection::Down
        } else {
          xkb::KeyDirection::Up
        },
      );
    }
  }
}

/// A non-printing keysym's name as a browser's `key` names it.
const fn key_name(keysym: u32) -> Option<&'static str> {
  Some(match keysym {
    keysyms::KEY_Return | keysyms::KEY_KP_Enter | keysyms::KEY_ISO_Enter => "Enter",
    keysyms::KEY_Tab | keysyms::KEY_ISO_Left_Tab | keysyms::KEY_KP_Tab => "Tab",
    keysyms::KEY_Escape => "Escape",
    keysyms::KEY_BackSpace => "Backspace",
    keysyms::KEY_Delete | keysyms::KEY_KP_Delete => "Delete",
    keysyms::KEY_Insert | keysyms::KEY_KP_Insert => "Insert",
    keysyms::KEY_Left | keysyms::KEY_KP_Left => "ArrowLeft",
    keysyms::KEY_Right | keysyms::KEY_KP_Right => "ArrowRight",
    keysyms::KEY_Up | keysyms::KEY_KP_Up => "ArrowUp",
    keysyms::KEY_Down | keysyms::KEY_KP_Down => "ArrowDown",
    keysyms::KEY_Home | keysyms::KEY_KP_Home => "Home",
    keysyms::KEY_End | keysyms::KEY_KP_End => "End",
    keysyms::KEY_Page_Up | keysyms::KEY_KP_Page_Up => "PageUp",
    keysyms::KEY_Page_Down | keysyms::KEY_KP_Page_Down => "PageDown",
    keysyms::KEY_Shift_L | keysyms::KEY_Shift_R => "Shift",
    keysyms::KEY_Control_L | keysyms::KEY_Control_R => "Control",
    keysyms::KEY_Alt_L | keysyms::KEY_Alt_R => "Alt",
    keysyms::KEY_Super_L | keysyms::KEY_Super_R | keysyms::KEY_Meta_L | keysyms::KEY_Meta_R => {
      "Meta"
    }
    keysyms::KEY_Caps_Lock => "CapsLock",
    keysyms::KEY_Num_Lock => "NumLock",
    keysyms::KEY_ISO_Level3_Shift => "AltGraph",
    keysyms::KEY_Menu => "ContextMenu",
    keysyms::KEY_space | keysyms::KEY_KP_Space => " ",
    keysyms::KEY_F1 => "F1",
    keysyms::KEY_F2 => "F2",
    keysyms::KEY_F3 => "F3",
    keysyms::KEY_F4 => "F4",
    keysyms::KEY_F5 => "F5",
    keysyms::KEY_F6 => "F6",
    keysyms::KEY_F7 => "F7",
    keysyms::KEY_F8 => "F8",
    keysyms::KEY_F9 => "F9",
    keysyms::KEY_F10 => "F10",
    keysyms::KEY_F11 => "F11",
    keysyms::KEY_F12 => "F12",
    keysyms::KEY_dead_acute
    | keysyms::KEY_dead_grave
    | keysyms::KEY_dead_circumflex
    | keysyms::KEY_dead_diaeresis
    | keysyms::KEY_dead_tilde => "Dead",
    _ => return None,
  })
}

/// An evdev keycode's physical key as a browser's `code` names it.
const fn code_name(evdev: u32) -> &'static str {
  const LETTERS: [(u32, &str); 26] = [
    (30, "KeyA"),
    (48, "KeyB"),
    (46, "KeyC"),
    (32, "KeyD"),
    (18, "KeyE"),
    (33, "KeyF"),
    (34, "KeyG"),
    (35, "KeyH"),
    (23, "KeyI"),
    (36, "KeyJ"),
    (37, "KeyK"),
    (38, "KeyL"),
    (50, "KeyM"),
    (49, "KeyN"),
    (24, "KeyO"),
    (25, "KeyP"),
    (16, "KeyQ"),
    (19, "KeyR"),
    (31, "KeyS"),
    (20, "KeyT"),
    (22, "KeyU"),
    (47, "KeyV"),
    (17, "KeyW"),
    (45, "KeyX"),
    (21, "KeyY"),
    (44, "KeyZ"),
  ];
  let mut index = 0;

  while index < LETTERS.len() {
    if LETTERS[index].0 == evdev {
      return LETTERS[index].1;
    }

    index += 1;
  }

  match evdev {
    2 => "Digit1",
    3 => "Digit2",
    4 => "Digit3",
    5 => "Digit4",
    6 => "Digit5",
    7 => "Digit6",
    8 => "Digit7",
    9 => "Digit8",
    10 => "Digit9",
    11 => "Digit0",
    1 => "Escape",
    14 => "Backspace",
    15 => "Tab",
    28 => "Enter",
    57 => "Space",
    29 => "ControlLeft",
    97 => "ControlRight",
    42 => "ShiftLeft",
    54 => "ShiftRight",
    56 => "AltLeft",
    100 => "AltRight",
    125 => "MetaLeft",
    126 => "MetaRight",
    58 => "CapsLock",
    103 => "ArrowUp",
    108 => "ArrowDown",
    105 => "ArrowLeft",
    106 => "ArrowRight",
    102 => "Home",
    107 => "End",
    104 => "PageUp",
    109 => "PageDown",
    110 => "Insert",
    111 => "Delete",
    12 => "Minus",
    13 => "Equal",
    26 => "BracketLeft",
    27 => "BracketRight",
    39 => "Semicolon",
    40 => "Quote",
    41 => "Backquote",
    43 => "Backslash",
    51 => "Comma",
    52 => "Period",
    53 => "Slash",
    127 => "ContextMenu",
    59 => "F1",
    60 => "F2",
    61 => "F3",
    62 => "F4",
    63 => "F5",
    64 => "F6",
    65 => "F7",
    66 => "F8",
    67 => "F9",
    68 => "F10",
    87 => "F11",
    88 => "F12",
    69 => "NumLock",
    70 => "ScrollLock",
    99 => "PrintScreen",
    119 => "Pause",
    71 => "Numpad7",
    72 => "Numpad8",
    73 => "Numpad9",
    74 => "NumpadSubtract",
    75 => "Numpad4",
    76 => "Numpad5",
    77 => "Numpad6",
    78 => "NumpadAdd",
    79 => "Numpad1",
    80 => "Numpad2",
    81 => "Numpad3",
    82 => "Numpad0",
    83 => "NumpadDecimal",
    96 => "NumpadEnter",
    98 => "NumpadDivide",
    55 => "NumpadMultiply",
    117 => "NumpadEqual",
    _ => "Unidentified",
  }
}

/// How a compose sequence shows `keysym` while it is composed: `Multi_key` as a middle dot, a
/// dead key as its spacing mark, any other key as the text it types.
fn shown(keysym: xkb::Keysym) -> String {
  let mark = match keysym.raw() {
    keysyms::KEY_Multi_key => '\u{b7}',
    keysyms::KEY_dead_grave => '`',
    keysyms::KEY_dead_acute => '\u{b4}',
    keysyms::KEY_dead_circumflex => '^',
    keysyms::KEY_dead_tilde => '~',
    keysyms::KEY_dead_macron => '\u{af}',
    keysyms::KEY_dead_breve => '\u{2d8}',
    keysyms::KEY_dead_abovedot => '\u{2d9}',
    keysyms::KEY_dead_diaeresis => '\u{a8}',
    keysyms::KEY_dead_abovering => '\u{2da}',
    keysyms::KEY_dead_doubleacute => '\u{2dd}',
    keysyms::KEY_dead_caron => '\u{2c7}',
    keysyms::KEY_dead_cedilla => '\u{b8}',
    keysyms::KEY_dead_ogonek => '\u{2db}',
    _ => {
      return xkb::keysym_to_utf8(keysym)
        .trim_end_matches('\0')
        .to_owned();
    }
  };

  mark.to_string()
}
