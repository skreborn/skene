//! Key mapping: the terminal's keys as the portable vocabulary a browser's events carry.

use std::str::FromStr;

use skene_foundation::{Code, Key as PortableKey, KeyEvent, Modifiers, NamedKey};

use crate::{
  escape,
  interaction,
  parse::{Key, KeyCode, KeyKind, ModifierKey},
  with_records,
};

/// The kitty keyboard flags asked of a terminal that answers the flags query: the
/// disambiguated escape codes, the event types (a release, a repeat), the alternate
/// keys (the shifted key under `Shift`), every key as an escape code (a modifier
/// alone), and the text a key typed (through `AltGr` or `Option` too). A terminal keeps
/// the flags it has.
pub(super) const KEYBOARD_FLAGS: u8 = escape::DISAMBIGUATE_ESCAPE_CODES
  | escape::REPORT_EVENT_TYPES
  | escape::REPORT_ALTERNATE_KEYS
  | escape::REPORT_ALL_KEYS_AS_ESCAPE_CODES
  | escape::REPORT_ASSOCIATED_TEXT;

/// Dispatches one keyboard press or repeat; see [`press`].
pub(super) fn dispatch_key(key: &Key) {
  press(&portable_key(key));
}

/// Dispatches one key press or repeat through core's dispatch, then the terminal's own
/// response unless a handler prevented it; `Ctrl-Q` quits unconditionally.
///
/// Where the terminal does not report releases, a release follows every press at once.
pub(crate) fn press(event: &KeyEvent) {
  if event.modifiers.ctrl && event.shortcut_key() == "q" {
    with_records(|records| records.quit = true);

    return;
  }

  interaction::key(event);

  if !with_records(|records| records.key_releases) {
    interaction::key_up(&KeyEvent {
      repeat: false,
      ..event.clone()
    });
  }
}

/// Maps a terminal key to the portable W3C key value and code: a key carries `Shift`
/// when it arrives with its shifted alternate applied, or uppercase, and Caps Lock
/// flips a letter's case, so a shortcut reads the modifiers and a text entry the case
/// as a browser's `key` gives them; the code is the key's position on the standard
/// layout, from the base-layout alternate where the terminal reports one.
///
/// Where the terminal reports the text a key typed, the text is the key, as a
/// browser's `key` is, and `Ctrl` and `Alt` are spent in typing it (`AltGr`, `Option`), so
/// a text entry inserts it.
pub(super) fn portable_key(key: &Key) -> KeyEvent {
  let mut modifiers = key.modifiers;
  let keypad = key.keypad;
  let key_value = match (key.text, key.code) {
    (Some(text), _) => {
      modifiers.ctrl = false;
      modifiers.alt = false;

      PortableKey::Character(text.to_string())
    }
    (None, code) => portable_key_value(key, code, &mut modifiers),
  };
  let code = match key.code {
    KeyCode::Char(character) if keypad => keypad_code(character),
    KeyCode::Char(character) => KeyEvent::code_of(&PortableKey::Character(
      key.base_layout.unwrap_or(character).to_string(),
    )),
    KeyCode::Enter if keypad => Code::NumpadEnter,
    KeyCode::KeypadBegin => Code::Numpad5,
    KeyCode::Modifier(modifier) => modifier_code(modifier),
    _ => KeyEvent::code_of(&key_value),
  };

  KeyEvent {
    key: key_value,
    code,
    modifiers,
    repeat: key.kind == KeyKind::Repeat,
    composing: false,
  }
}

/// The W3C key value of `code`, a key reporting no text, marking `Shift` in `modifiers`
/// where the key arrived shifted.
#[expect(
  deprecated,
  reason = "the terminal reports a Hyper key, which the W3C names by its legacy value"
)]
fn portable_key_value(key: &Key, code: KeyCode, modifiers: &mut Modifiers) -> PortableKey {
  let named = match code {
    KeyCode::Char(character) => {
      modifiers.shift |= key.shifted.is_some() || character.is_uppercase();

      return PortableKey::Character(if key.caps_lock && character.is_alphabetic() {
        if character.is_uppercase() {
          character.to_lowercase().collect()
        }
        else {
          character.to_uppercase().collect()
        }
      }
      else {
        character.to_string()
      });
    }
    KeyCode::Modifier(modifier) => match modifier {
      ModifierKey::LeftShift | ModifierKey::RightShift => NamedKey::Shift,
      ModifierKey::LeftControl | ModifierKey::RightControl => NamedKey::Control,
      ModifierKey::LeftAlt | ModifierKey::RightAlt => NamedKey::Alt,
      ModifierKey::LeftSuper
      | ModifierKey::RightSuper
      | ModifierKey::LeftMeta
      | ModifierKey::RightMeta => NamedKey::Meta,
      ModifierKey::LeftHyper | ModifierKey::RightHyper => NamedKey::Hyper,
      ModifierKey::IsoLevel3Shift => NamedKey::AltGraph,
      ModifierKey::IsoLevel5Shift => NamedKey::Unidentified,
    },
    KeyCode::Enter => NamedKey::Enter,
    KeyCode::Backspace => NamedKey::Backspace,
    KeyCode::Tab => NamedKey::Tab,
    KeyCode::Escape => NamedKey::Escape,
    KeyCode::Delete => NamedKey::Delete,
    KeyCode::Left => NamedKey::ArrowLeft,
    KeyCode::Right => NamedKey::ArrowRight,
    KeyCode::Up => NamedKey::ArrowUp,
    KeyCode::Down => NamedKey::ArrowDown,
    KeyCode::Home => NamedKey::Home,
    KeyCode::End => NamedKey::End,
    KeyCode::PageUp => NamedKey::PageUp,
    KeyCode::PageDown => NamedKey::PageDown,
    KeyCode::Insert => NamedKey::Insert,
    KeyCode::Function(number) => {
      NamedKey::from_str(&format!("F{number}")).unwrap_or(NamedKey::Unidentified)
    }
    _ => NamedKey::Unidentified,
  };

  PortableKey::Named(named)
}

/// The W3C code of the keypad key producing `character`.
fn keypad_code(character: char) -> Code {
  match character {
    '0'..='9' => Code::from_str(&format!("Numpad{character}")).unwrap_or(Code::Unidentified),
    '.' => Code::NumpadDecimal,
    '/' => Code::NumpadDivide,
    '*' => Code::NumpadMultiply,
    '-' => Code::NumpadSubtract,
    '+' => Code::NumpadAdd,
    '=' => Code::NumpadEqual,
    ',' => Code::NumpadComma,
    _ => Code::Unidentified,
  }
}

/// The W3C code of the modifier key `modifier`, by its side.
const fn modifier_code(modifier: ModifierKey) -> Code {
  match modifier {
    ModifierKey::LeftShift => Code::ShiftLeft,
    ModifierKey::RightShift => Code::ShiftRight,
    ModifierKey::LeftControl => Code::ControlLeft,
    ModifierKey::RightControl => Code::ControlRight,
    ModifierKey::LeftAlt => Code::AltLeft,
    ModifierKey::RightAlt | ModifierKey::IsoLevel3Shift => Code::AltRight,
    ModifierKey::LeftSuper | ModifierKey::LeftMeta => Code::MetaLeft,
    ModifierKey::RightSuper | ModifierKey::RightMeta => Code::MetaRight,
    ModifierKey::LeftHyper | ModifierKey::RightHyper | ModifierKey::IsoLevel5Shift => {
      Code::Unidentified
    }
  }
}

/// The key event `spec` names: a portable W3C key value (`"a"`, `"Enter"`, `"Tab"`,
/// `"Backspace"`, …) after the `"Ctrl+"`, `"Alt+"` and `"Shift+"` prefixes adding
/// its modifiers (`"Ctrl+ArrowLeft"`, `"Shift+Tab"`).
pub(crate) fn key_event(spec: &str) -> KeyEvent {
  let mut modifiers = Modifiers::default();
  let mut key = spec;

  while let Some((modifier, rest)) = key.split_once('+') {
    match modifier {
      "Ctrl" => modifiers.ctrl = true,
      "Alt" => modifiers.alt = true,
      "Shift" => modifiers.shift = true,
      _ => break,
    }

    key = rest;
  }

  KeyEvent {
    modifiers,
    ..KeyEvent::new(key)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A modifier pressed alone maps to its W3C key value, and a repeat carries the
  /// flag.
  #[test]
  fn modifier_keys_and_repeats_map_to_the_portable_vocabulary() {
    let shift = portable_key(&Key::new(KeyCode::Modifier(ModifierKey::LeftShift)).with(
      Modifiers {
        shift: true,
        ..Modifiers::default()
      },
    ));
    let meta = portable_key(&Key::new(KeyCode::Modifier(ModifierKey::RightSuper)));
    let mut held = Key::new(KeyCode::Char('a'));

    held.kind = KeyKind::Repeat;

    let held = portable_key(&held);

    assert!(shift.is(NamedKey::Shift));
    assert!(shift.is_modifier());
    assert!(meta.is(NamedKey::Meta));
    assert!(held.repeat && !held.is_modifier());
  }

  /// An uppercase letter carries `Shift` even where the terminal dropped it for the
  /// shifted key, and Caps Lock flips a letter's case without touching the modifiers.
  #[test]
  fn letter_case_follows_shift_and_caps_lock() {
    let shifted = portable_key(&Key::new(KeyCode::Char('Z')).with(Modifiers {
      ctrl: true,
      ..Modifiers::default()
    }));
    let mut locked = Key::new(KeyCode::Char('z')).with(Modifiers {
      ctrl: true,
      ..Modifiers::default()
    });

    locked.caps_lock = true;

    let mut locked_shifted = locked;

    locked_shifted.code = KeyCode::Char('Z');

    let locked_shifted = portable_key(&locked_shifted);
    let locked = portable_key(&locked);

    assert!(shifted.types("Z"));
    assert!(shifted.modifiers.shift && shifted.modifiers.ctrl);
    assert!(locked.types("Z"));
    assert!(!locked.modifiers.shift && locked.modifiers.ctrl);
    assert!(locked_shifted.types("z"));
    assert!(locked_shifted.modifiers.shift);
  }

  /// A shifted alternate the terminal applied restores `Shift` for any key, and the
  /// code is the base-layout alternate's position where one is reported, else the
  /// key's own on the standard layout, the keypad's keys and the modifiers' sides by
  /// their own names.
  #[test]
  fn alternate_keys_restore_shift_and_name_the_physical_key() {
    let mut bang = Key::new(KeyCode::Char('!'));

    bang.shifted = Some('!');

    let bang = portable_key(&bang);

    assert!(bang.types("!"));
    assert_eq!(bang.code, Code::Digit1);
    assert!(bang.modifiers.shift);

    let mut cyrillic = Key::new(KeyCode::Char('\u{444}')).with(Modifiers {
      ctrl: true,
      ..Modifiers::default()
    });

    cyrillic.base_layout = Some('a');

    let cyrillic = portable_key(&cyrillic);

    assert!(cyrillic.types("\u{444}"));
    assert_eq!(cyrillic.code, Code::KeyA);
    assert!(cyrillic.modifiers.ctrl && !cyrillic.modifiers.shift);

    let mut keypad = Key::new(KeyCode::Char('7'));

    keypad.keypad = true;

    assert_eq!(portable_key(&keypad).code, Code::Numpad7);
    assert_eq!(
      portable_key(&Key::new(KeyCode::Modifier(ModifierKey::RightShift))).code,
      Code::ShiftRight
    );
    assert_eq!(portable_key(&Key::new(KeyCode::Left)).code, Code::ArrowLeft);
    assert_eq!(key_event("Ctrl+z").code, Code::KeyZ);
  }

  /// A key reporting the text it typed is that text, the `Ctrl` and `Alt` spent in
  /// typing it cleared, so an entry inserts what `AltGr` or `Option` typed.
  #[test]
  fn a_typed_text_is_the_key() {
    let mut option = Key::new(KeyCode::Char('2')).with(Modifiers {
      alt: true,
      ..Modifiers::default()
    });

    option.text = Some('\u{2122}');

    let option = portable_key(&option);

    assert!(option.types("\u{2122}"));
    assert_eq!(option.code, Code::Digit2);
    assert!(!option.modifiers.alt && !option.modifiers.ctrl);
  }
}
