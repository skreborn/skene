//! The terminal's input decoded: keys in the legacy and kitty encodings, mouse reports,
//! bracketed pastes, and the replies to the loop's queries.
//!
//! The parser is incremental: bytes arrive in chunks, a sequence cut at a chunk's end
//! completes with the next, and a sequence the loop never asked about is consumed
//! whole and dropped rather than read as keys.

use std::collections::VecDeque;

use skene_core::{Modifiers, PointerButton};

use crate::escape::{Color, Sgr, Underline, base64};

/// The escape byte.
const ESC: u8 = 0x1b;

/// The bell, an OSC string's other terminator.
const BEL: u8 = 0x07;

/// What the terminal sent: an input, or a reply to a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
  Key(Key),
  Mouse(Mouse),
  /// A bracketed paste's text.
  Paste(String),
  /// A line the dev launcher wrote on its control pipe.
  Control(String),
  /// An in-band resize notification (mode 2048).
  Resized(WindowSize),
  /// A DECRPM report of a DEC private mode.
  Mode {
    mode: u16,
    setting: ModeSetting,
  },
  /// A color scheme report (mode 2031).
  Theme {
    dark: bool,
  },
  /// A cursor position report, zero-based.
  CursorPosition {
    column: u16,
    row: u16,
  },
  /// The kitty keyboard flags in force.
  KeyboardFlags(u8),
  /// The foreground color, an OSC 10 reply.
  Foreground(u8, u8, u8),
  /// The background color, an OSC 11 reply.
  Background(u8, u8, u8),
  /// Base colors by index, an OSC 4 reply.
  Palette(Vec<(u8, (u8, u8, u8))>),
  /// The clipboard's text, an OSC 52 reply.
  Clipboard(String),
  /// The answering clipboard's reply (OSC 5522) to a read, or to a write when not
  /// `read`.
  ClipboardAnswer {
    read: bool,
    status: ClipboardStatus,
  },
  /// The graphic rendition in force, a DECRPSS reply.
  GraphicRendition(Vec<Sgr>),
  /// The kitty graphics protocol's reply to a command on the image `id`: `OK`, or
  /// an error.
  Graphics {
    id: u32,
    ok: bool,
  },
}

/// What the answering clipboard said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClipboardStatus {
  /// The request is taken; a read's data follows.
  Ok,
  /// A chunk of the text read.
  Data(Vec<u8>),
  /// The request is complete.
  Done,
  /// The request was refused: denied, busy, or not understood.
  Refused,
}

/// The window's size in cells, and in pixels where the terminal knows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowSize {
  pub(crate) columns: u16,
  pub(crate) rows: u16,
  pub(crate) pixel_width: Option<u16>,
  pub(crate) pixel_height: Option<u16>,
}

/// What a DECRPM report says of a mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModeSetting {
  NotRecognized,
  Set,
  Reset,
  PermanentlySet,
  PermanentlyReset,
}

/// A key event: the key, the modifiers held, and what the kitty protocol adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Key {
  pub(crate) code: KeyCode,
  pub(crate) modifiers: Modifiers,
  pub(crate) kind: KeyKind,
  /// Whether the key is on the keypad.
  pub(crate) keypad: bool,
  /// Whether Caps Lock is on.
  pub(crate) caps_lock: bool,
  /// The shifted key the terminal applied, where it reports alternate keys.
  pub(crate) shifted: Option<char>,
  /// The key at the same position on the standard layout, where the active layout
  /// differs there.
  pub(crate) base_layout: Option<char>,
}

impl Key {
  /// A press of `code` with no modifier held.
  pub(crate) const fn new(code: KeyCode) -> Self {
    Self {
      code,
      modifiers: Modifiers {
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
      },
      kind: KeyKind::Press,
      keypad: false,
      caps_lock: false,
      shifted: None,
      base_layout: None,
    }
  }

  /// This key with `modifiers` held.
  pub(crate) const fn with(mut self, modifiers: Modifiers) -> Self {
    self.modifiers = modifiers;

    self
  }
}

/// Whether a key was pressed, repeated while held, or released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyKind {
  Press,
  Repeat,
  Release,
}

/// The key itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyCode {
  Char(char),
  Enter,
  Backspace,
  Tab,
  BackTab,
  Escape,
  Left,
  Right,
  Up,
  Down,
  Home,
  End,
  PageUp,
  PageDown,
  Insert,
  Delete,
  KeypadBegin,
  Function(u8),
  Modifier(ModifierKey),
  /// A key the loop has no name for.
  Unidentified,
}

/// A modifier key pressed on its own, by its side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModifierKey {
  LeftShift,
  LeftControl,
  LeftAlt,
  LeftSuper,
  LeftHyper,
  LeftMeta,
  RightShift,
  RightControl,
  RightAlt,
  RightSuper,
  RightHyper,
  RightMeta,
  IsoLevel3Shift,
  IsoLevel5Shift,
}

/// A mouse report at a zero-based cell, or in pixels while pixel reports are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Mouse {
  pub(crate) kind: MouseKind,
  pub(crate) column: u16,
  pub(crate) row: u16,
  pub(crate) modifiers: Modifiers,
}

/// What the mouse did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MouseKind {
  Press(PointerButton),
  Release(PointerButton),
  Drag(PointerButton),
  Move,
  ScrollUp,
  ScrollDown,
  ScrollLeft,
  ScrollRight,
}

/// The incremental parser: bytes in, events out.
#[derive(Debug, Default)]
pub(crate) struct Parser {
  buffer: Vec<u8>,
  events: VecDeque<Event>,
}

impl Parser {
  /// Takes `bytes` in; `more` says the chunk may have been cut short of a sequence's
  /// end, so a lone escape at its end waits rather than reading as the `Escape` key.
  pub(crate) fn feed(&mut self, bytes: &[u8], more: bool) {
    self.buffer.extend_from_slice(bytes);

    loop {
      match scan(&self.buffer, more) {
        Scan::Incomplete => break,
        Scan::Done(consumed, event) => {
          self.buffer.drain(..consumed);
          self.events.extend(event);

          if self.buffer.is_empty() {
            break;
          }
        }
      }
    }
  }

  /// The oldest event decoded and not yet taken.
  pub(crate) fn pop(&mut self) -> Option<Event> {
    self.events.pop_front()
  }
}

/// What one scan of the buffer's head found.
enum Scan {
  /// The head is a sequence still arriving.
  Incomplete,
  /// `consumed` bytes make one input, or nothing the loop reads.
  Done(usize, Option<Event>),
}

/// Scans the buffer's head for one input.
fn scan(buffer: &[u8], more: bool) -> Scan {
  let Some(first) = buffer.first()
  else {
    return Scan::Incomplete;
  };

  match *first {
    ESC => scan_escape(buffer, more),
    b'\r' => key(1, KeyCode::Enter),
    b'\t' => key(1, KeyCode::Tab),
    0x7f => key(1, KeyCode::Backspace),
    0x00 => control(1, ' '),
    byte @ 0x01..=0x1a => control(1, char::from(byte - 0x01 + b'a')),
    byte @ 0x1c..=0x1f => control(1, char::from(byte - 0x1c + b'4')),
    _ => scan_char(buffer),
  }
}

/// One byte's key.
fn key(consumed: usize, code: KeyCode) -> Scan {
  Scan::Done(consumed, Some(Event::Key(Key::new(code))))
}

/// A control byte's key: `Ctrl` with `character`.
fn control(consumed: usize, character: char) -> Scan {
  Scan::Done(
    consumed,
    Some(Event::Key(Key::new(KeyCode::Char(character)).with(
      Modifiers {
        ctrl: true,
        ..Modifiers::default()
      },
    ))),
  )
}

/// A UTF-8 character's key; an uppercase letter carries `Shift`.
fn scan_char(buffer: &[u8]) -> Scan {
  let needed = match buffer[0] {
    0x00..=0x7f => 1,
    0xc0..=0xdf => 2,
    0xe0..=0xef => 3,
    0xf0..=0xf7 => 4,
    _ => return Scan::Done(1, None),
  };

  if buffer.len() < needed {
    return Scan::Incomplete;
  }

  let Ok(text) = std::str::from_utf8(&buffer[..needed])
  else {
    return Scan::Done(1, None);
  };
  let character = text.chars().next().expect("one character");

  Scan::Done(
    needed,
    Some(Event::Key(Key::new(KeyCode::Char(character)).with(
      Modifiers {
        shift: character.is_uppercase(),
        ..Modifiers::default()
      },
    ))),
  )
}

/// An escape sequence, or `Alt` with the key after a lone escape.
fn scan_escape(buffer: &[u8], more: bool) -> Scan {
  let Some(second) = buffer.get(1)
  else {
    return if more {
      Scan::Incomplete
    }
    else {
      key(1, KeyCode::Escape)
    };
  };

  match *second {
    b'[' => scan_csi(buffer),
    b']' => scan_string(buffer, true, parse_osc),
    b'P' => scan_string(buffer, false, parse_dcs),
    b'_' => scan_string(buffer, false, parse_apc),
    b'^' | b'X' => scan_string(buffer, false, |_| None),
    b'O' => scan_ss3(buffer),
    ESC => key(1, KeyCode::Escape),
    _ => match scan(&buffer[1..], more) {
      Scan::Incomplete => Scan::Incomplete,
      Scan::Done(consumed, Some(Event::Key(mut key))) => {
        key.modifiers.alt = true;

        Scan::Done(consumed + 1, Some(Event::Key(key)))
      }
      Scan::Done(consumed, event) => Scan::Done(consumed + 1, event),
    },
  }
}

/// An `SS3` key: the cursor keys and `F1` to `F4` in application mode.
fn scan_ss3(buffer: &[u8]) -> Scan {
  let Some(third) = buffer.get(2)
  else {
    return Scan::Incomplete;
  };
  let code = match *third {
    b'A' => KeyCode::Up,
    b'B' => KeyCode::Down,
    b'C' => KeyCode::Right,
    b'D' => KeyCode::Left,
    b'H' => KeyCode::Home,
    b'F' => KeyCode::End,
    byte @ b'P'..=b'S' => KeyCode::Function(1 + byte - b'P'),
    _ => return Scan::Done(3, None),
  };

  key(3, code)
}

/// A string sequence (OSC, DCS, APC, PM, SOS) up to its terminator—`ST`, or `BEL` too
/// where `bel` says the kind takes it—handed to `parse` without its introducer and
/// terminator.
fn scan_string(buffer: &[u8], bel: bool, parse: impl Fn(&[u8]) -> Option<Event>) -> Scan {
  let mut index = 2;

  while index < buffer.len() {
    match buffer[index] {
      BEL if bel => return Scan::Done(index + 1, parse(&buffer[2..index])),
      ESC if buffer.get(index + 1) == Some(&b'\\') => {
        return Scan::Done(index + 2, parse(&buffer[2..index]));
      }
      _ => index += 1,
    }
  }

  Scan::Incomplete
}

/// A CSI sequence: its parameters, intermediates and final byte, dispatched by the
/// final.
fn scan_csi(buffer: &[u8]) -> Scan {
  // The Linux console's `F1` to `F5`, `CSI [ A` to `CSI [ E`.
  if buffer.get(2) == Some(&b'[') {
    return match buffer.get(3) {
      None => Scan::Incomplete,
      Some(byte @ b'A'..=b'E') => key(4, KeyCode::Function(1 + byte - b'A')),
      Some(_) => Scan::Done(4, None),
    };
  }

  let mut index = 2;

  while buffer
    .get(index)
    .is_some_and(|byte| (0x30..=0x3f).contains(byte))
  {
    index += 1;
  }

  let parameters_end = index;

  while buffer
    .get(index)
    .is_some_and(|byte| (0x20..=0x2f).contains(byte))
  {
    index += 1;
  }

  let Some(last) = buffer.get(index)
  else {
    return Scan::Incomplete;
  };

  if !(0x40..=0x7e).contains(last) {
    return Scan::Done(index + 1, None);
  }

  let Ok(parameters) = std::str::from_utf8(&buffer[2..parameters_end])
  else {
    return Scan::Done(index + 1, None);
  };
  let intermediates = &buffer[parameters_end..index];
  let consumed = index + 1;

  // A bracketed paste runs to its closing bracket.
  if parameters == "200" && *last == b'~' {
    return match find(&buffer[consumed..], b"\x1b[201~") {
      Some(end) => Scan::Done(
        consumed + end + 6,
        Some(Event::Paste(
          String::from_utf8_lossy(&buffer[consumed..consumed + end]).into_owned(),
        )),
      ),
      None => Scan::Incomplete,
    };
  }

  // The X10 mouse encoding carries three bytes after its final.
  if parameters.is_empty() && intermediates.is_empty() && *last == b'M' {
    let Some(report) = buffer.get(3..6)
    else {
      return Scan::Incomplete;
    };

    return Scan::Done(6, parse_x10_mouse(report));
  }

  let event = match (parameters.as_bytes().first(), *last) {
    (Some(b'?'), b'y') if intermediates == b"$" => parse_mode_report(&parameters[1..]),
    (Some(b'?'), b'n') => parse_theme(&parameters[1..]),
    (Some(b'?'), b'u') => parameters[1..].parse().ok().map(Event::KeyboardFlags),
    (Some(b'<'), b'M' | b'm') => parse_sgr_mouse(&parameters[1..], *last == b'm'),
    (Some(b'?' | b'<' | b'=' | b'>'), _) => None,
    (_, b'R') => parse_cursor_position(parameters),
    (_, b't') => parse_window_report(parameters),
    (_, b'u') => parse_kitty_key(parameters),
    (_, b'~') => parse_special_key(parameters),
    (_, b'Z') => Some(Event::Key(Key::new(KeyCode::BackTab).with(Modifiers {
      shift: true,
      ..Modifiers::default()
    }))),
    (_, b'A' | b'B' | b'C' | b'D' | b'F' | b'H' | b'P' | b'Q' | b'S') => {
      parse_modified_key(parameters, *last)
    }
    _ => None,
  };

  Scan::Done(consumed, event)
}

/// Where `needle` first appears in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
  haystack
    .windows(needle.len())
    .position(|window| window == needle)
}

/// The modifiers a mask names: the held keys less one, bit by bit.
fn modifiers_of(mask: u8) -> Modifiers {
  let bits = mask.saturating_sub(1);

  Modifiers {
    shift: bits & 1 != 0,
    alt: bits & 2 != 0,
    ctrl: bits & 4 != 0,
    meta: bits & (8 | 32) != 0,
  }
}

/// A `modifiers[:kind]` parameter: the modifiers, the event kind, and Caps Lock.
fn parse_modifier_parameter(parameter: Option<&str>) -> (Modifiers, KeyKind, bool) {
  let mut parts = parameter.unwrap_or_default().split(':');
  let mask: u8 = parts.next().and_then(|mask| mask.parse().ok()).unwrap_or(1);
  let kind = match parts.next().and_then(|kind| kind.parse().ok()) {
    Some(2) => KeyKind::Repeat,
    Some(3) => KeyKind::Release,
    _ => KeyKind::Press,
  };

  (modifiers_of(mask), kind, mask.saturating_sub(1) & 64 != 0)
}

/// A kitty `CSI u` key: `code[:shifted[:base]] ; modifiers[:kind] ; text`.
fn parse_kitty_key(parameters: &str) -> Option<Event> {
  let mut fields = parameters.split(';');
  let mut codes = fields.next()?.split(':');
  let code_point: u32 = codes.next()?.parse().ok()?;
  let (mut modifiers, kind, caps_lock) = parse_modifier_parameter(fields.next());
  let (mut code, keypad) = functional_key(code_point).unwrap_or_else(|| {
    (
      match char::from_u32(code_point) {
        Some('\x1b') => KeyCode::Escape,
        Some('\r') => KeyCode::Enter,
        Some('\t') if modifiers.shift => KeyCode::BackTab,
        Some('\t') => KeyCode::Tab,
        Some('\x7f') => KeyCode::Backspace,
        Some(character) => KeyCode::Char(character),
        None => KeyCode::Unidentified,
      },
      false,
    )
  });

  if let KeyCode::Modifier(modifier) = code {
    match modifier {
      ModifierKey::LeftAlt | ModifierKey::RightAlt => modifiers.alt = true,
      ModifierKey::LeftControl | ModifierKey::RightControl => modifiers.ctrl = true,
      ModifierKey::LeftShift | ModifierKey::RightShift => modifiers.shift = true,
      ModifierKey::LeftSuper
      | ModifierKey::RightSuper
      | ModifierKey::LeftMeta
      | ModifierKey::RightMeta => modifiers.meta = true,
      _ => {}
    }
  }

  let mut alternate = || {
    codes
      .next()
      .and_then(|code| code.parse::<u32>().ok())
      .and_then(char::from_u32)
  };
  let shifted = alternate();
  let base_layout = alternate();

  if modifiers.shift
    && let Some(shifted) = shifted
  {
    code = KeyCode::Char(shifted);
    modifiers.shift = false;
  }

  Some(Event::Key(Key {
    code,
    modifiers,
    kind,
    keypad,
    caps_lock,
    shifted,
    base_layout,
  }))
}

/// The key a kitty functional code point names, and whether it is on the keypad.
fn functional_key(code_point: u32) -> Option<(KeyCode, bool)> {
  let keypad = match code_point {
    57399..=57408 => KeyCode::Char(char::from(u8::try_from(code_point - 57399).ok()? + b'0')),
    57409 => KeyCode::Char('.'),
    57410 => KeyCode::Char('/'),
    57411 => KeyCode::Char('*'),
    57412 => KeyCode::Char('-'),
    57413 => KeyCode::Char('+'),
    57414 => KeyCode::Enter,
    57415 => KeyCode::Char('='),
    57416 => KeyCode::Char(','),
    57417 => KeyCode::Left,
    57418 => KeyCode::Right,
    57419 => KeyCode::Up,
    57420 => KeyCode::Down,
    57421 => KeyCode::PageUp,
    57422 => KeyCode::PageDown,
    57423 => KeyCode::Home,
    57424 => KeyCode::End,
    57425 => KeyCode::Insert,
    57426 => KeyCode::Delete,
    57427 => KeyCode::KeypadBegin,
    _ => {
      let code = match code_point {
        57358..=57363 | 57428..=57440 => KeyCode::Unidentified,
        57376..=57398 => KeyCode::Function(u8::try_from(code_point - 57376).ok()? + 13),
        57441 => KeyCode::Modifier(ModifierKey::LeftShift),
        57442 => KeyCode::Modifier(ModifierKey::LeftControl),
        57443 => KeyCode::Modifier(ModifierKey::LeftAlt),
        57444 => KeyCode::Modifier(ModifierKey::LeftSuper),
        57445 => KeyCode::Modifier(ModifierKey::LeftHyper),
        57446 => KeyCode::Modifier(ModifierKey::LeftMeta),
        57447 => KeyCode::Modifier(ModifierKey::RightShift),
        57448 => KeyCode::Modifier(ModifierKey::RightControl),
        57449 => KeyCode::Modifier(ModifierKey::RightAlt),
        57450 => KeyCode::Modifier(ModifierKey::RightSuper),
        57451 => KeyCode::Modifier(ModifierKey::RightHyper),
        57452 => KeyCode::Modifier(ModifierKey::RightMeta),
        57453 => KeyCode::Modifier(ModifierKey::IsoLevel3Shift),
        57454 => KeyCode::Modifier(ModifierKey::IsoLevel5Shift),
        _ => return None,
      };

      return Some((code, false));
    }
  };

  Some((keypad, true))
}

/// A `CSI number ; modifiers ~` key.
fn parse_special_key(parameters: &str) -> Option<Event> {
  let mut fields = parameters.split(';');
  let number: u8 = fields.next()?.parse().ok()?;
  let (modifiers, kind, caps_lock) = parse_modifier_parameter(fields.next());
  let code = match number {
    1 | 7 => KeyCode::Home,
    2 => KeyCode::Insert,
    3 => KeyCode::Delete,
    4 | 8 => KeyCode::End,
    5 => KeyCode::PageUp,
    6 => KeyCode::PageDown,
    11..=15 => KeyCode::Function(number - 10),
    17..=21 => KeyCode::Function(number - 11),
    23..=26 => KeyCode::Function(number - 12),
    28..=29 => KeyCode::Function(number - 15),
    31..=34 => KeyCode::Function(number - 17),
    _ => return None,
  };

  Some(Event::Key(Key {
    modifiers,
    kind,
    caps_lock,
    ..Key::new(code)
  }))
}

/// A `CSI 1 ; modifiers X` key, or `CSI X` bare.
fn parse_modified_key(parameters: &str, last: u8) -> Option<Event> {
  let mut fields = parameters.split(';');
  let _ = fields.next();
  let (modifiers, kind, caps_lock) = parse_modifier_parameter(fields.next());
  let code = match last {
    b'A' => KeyCode::Up,
    b'B' => KeyCode::Down,
    b'C' => KeyCode::Right,
    b'D' => KeyCode::Left,
    b'F' => KeyCode::End,
    b'H' => KeyCode::Home,
    b'P' => KeyCode::Function(1),
    b'Q' => KeyCode::Function(2),
    b'S' => KeyCode::Function(4),
    _ => return None,
  };

  Some(Event::Key(Key {
    modifiers,
    kind,
    caps_lock,
    ..Key::new(code)
  }))
}

/// An SGR mouse report, `CSI < button ; column ; row M` or `m` for a release.
fn parse_sgr_mouse(parameters: &str, release: bool) -> Option<Event> {
  let mut fields = parameters.split(';');
  let button: u8 = fields.next()?.parse().ok()?;
  let column: u16 = fields.next()?.parse().ok()?;
  let row: u16 = fields.next()?.parse().ok()?;
  let (kind, modifiers) = mouse_button(button)?;
  let kind = match kind {
    MouseKind::Press(button) if release => MouseKind::Release(button),
    kind => kind,
  };

  Some(Event::Mouse(Mouse {
    kind,
    column: column.saturating_sub(1),
    row: row.saturating_sub(1),
    modifiers,
  }))
}

/// An X10 mouse report's three bytes, each offset by 32.
fn parse_x10_mouse(report: &[u8]) -> Option<Event> {
  let (kind, modifiers) = mouse_button(report[0].checked_sub(32)?)?;

  Some(Event::Mouse(Mouse {
    kind,
    column: u16::from(report[1].saturating_sub(33)),
    row: u16::from(report[2].saturating_sub(33)),
    modifiers,
  }))
}

/// What a mouse report's button byte says: the action and the modifiers held.
fn mouse_button(byte: u8) -> Option<(MouseKind, Modifiers)> {
  let number = (byte & 0b11) | ((byte & 0b1100_0000) >> 4);
  let dragging = byte & 0b0010_0000 != 0;
  let kind = match (number, dragging) {
    (0, false) => MouseKind::Press(PointerButton::Primary),
    (1, false) => MouseKind::Press(PointerButton::Auxiliary),
    (2, false) => MouseKind::Press(PointerButton::Secondary),
    (0, true) => MouseKind::Drag(PointerButton::Primary),
    (1, true) => MouseKind::Drag(PointerButton::Auxiliary),
    (2, true) => MouseKind::Drag(PointerButton::Secondary),
    (3, false) => MouseKind::Release(PointerButton::Primary),
    (3..=5, true) => MouseKind::Move,
    (4, false) => MouseKind::ScrollUp,
    (5, false) => MouseKind::ScrollDown,
    (6, false) => MouseKind::ScrollLeft,
    (7, false) => MouseKind::ScrollRight,
    _ => return None,
  };

  Some((
    kind,
    Modifiers {
      shift: byte & 0b100 != 0,
      alt: byte & 0b1000 != 0,
      ctrl: byte & 0b1_0000 != 0,
      meta: false,
    },
  ))
}

/// A DECRPM report, `CSI ? mode ; setting $ y`.
fn parse_mode_report(parameters: &str) -> Option<Event> {
  let (mode, setting) = parameters.split_once(';')?;
  let mode: u16 = mode.parse().ok()?;
  let setting = match setting.parse::<u8>().ok()? {
    0 => ModeSetting::NotRecognized,
    1 => ModeSetting::Set,
    2 => ModeSetting::Reset,
    3 => ModeSetting::PermanentlySet,
    4 => ModeSetting::PermanentlyReset,
    _ => return None,
  };

  Some(Event::Mode { mode, setting })
}

/// A color scheme report, `CSI ? 997 ; 1 n` for dark and `2` for light.
fn parse_theme(parameters: &str) -> Option<Event> {
  let (code, mode) = parameters.split_once(';')?;

  if code != "997" {
    return None;
  }

  match mode {
    "1" => Some(Event::Theme { dark: true }),
    "2" => Some(Event::Theme { dark: false }),
    _ => None,
  }
}

/// A cursor position report, `CSI row ; column R`, one-based.
fn parse_cursor_position(parameters: &str) -> Option<Event> {
  let (row, column) = parameters.split_once(';')?;
  let row: u16 = row.parse().ok()?;
  let column: u16 = column.parse().ok()?;

  Some(Event::CursorPosition {
    column: column.checked_sub(1)?,
    row: row.checked_sub(1)?,
  })
}

/// An in-band resize notification, `CSI 48 ; rows ; columns ; height ; width t`; a
/// terminal that does not know its pixel size reports zero.
fn parse_window_report(parameters: &str) -> Option<Event> {
  let mut fields = parameters.split(';');

  if fields.next()? != "48" {
    return None;
  }

  let rows: u16 = fields.next()?.parse().ok()?;
  let columns: u16 = fields.next()?.parse().ok()?;
  let mut pixels = || {
    fields
      .next()
      .and_then(|pixels| pixels.parse::<u16>().ok())
      .filter(|pixels| *pixels > 0)
  };
  let pixel_height = pixels();
  let pixel_width = pixels();

  Some(Event::Resized(WindowSize {
    columns,
    rows,
    pixel_width,
    pixel_height,
  }))
}

/// An OSC string's body: the background color (11) or the clipboard's text (52).
fn parse_osc(body: &[u8]) -> Option<Event> {
  let body = std::str::from_utf8(body).ok()?;
  let (number, rest) = body.split_once(';')?;

  match number {
    "4" => {
      let mut fields = rest.split(';');
      let mut colors = Vec::new();

      while let (Some(index), Some(color)) = (fields.next(), fields.next()) {
        if let (Ok(index), Some(color)) = (index.parse::<u8>(), parse_rgb(color)) {
          colors.push((index, color));
        }
      }

      (!colors.is_empty()).then_some(Event::Palette(colors))
    }
    "10" => {
      let (red, green, blue) = parse_rgb(rest)?;

      Some(Event::Foreground(red, green, blue))
    }
    "11" => {
      let (red, green, blue) = parse_rgb(rest)?;

      Some(Event::Background(red, green, blue))
    }
    "52" => {
      let (_, payload) = rest.split_once(';')?;

      if payload == "?" {
        return None;
      }

      Some(Event::Clipboard(
        String::from_utf8(base64::decode(payload)?).ok()?,
      ))
    }
    "5522" => {
      let (metadata, payload) = rest.split_once(';').unwrap_or((rest, ""));
      let value = |key: &str| {
        metadata
          .split(':')
          .find_map(|pair| pair.strip_prefix(key)?.strip_prefix('='))
      };
      let status = match value("status")? {
        "OK" => ClipboardStatus::Ok,
        "DATA" => ClipboardStatus::Data(base64::decode(payload)?),
        "DONE" => ClipboardStatus::Done,
        _ => ClipboardStatus::Refused,
      };

      Some(Event::ClipboardAnswer {
        read: value("type") == Some("read"),
        status,
      })
    }
    _ => None,
  }
}

/// An X color, `rgb:RR/GG/BB` with one to four hex digits per component, each scaled
/// to eight bits.
fn parse_rgb(text: &str) -> Option<(u8, u8, u8)> {
  let mut parts = text.strip_prefix("rgb:")?.split('/');
  let mut component = || {
    let digits = parts.next()?;

    if digits.is_empty() || digits.len() > 4 {
      return None;
    }

    let value = u32::from_str_radix(digits, 16).ok()?;
    let full = (1u32 << (4 * digits.len())) - 1;

    u8::try_from((value * 255 + full / 2) / full).ok()
  };
  let red = component()?;
  let green = component()?;
  let blue = component()?;

  Some((red, green, blue))
}

/// An APC string's body: the graphics protocol's reply, `G` then `key=value` pairs
/// naming the image, `;` and `OK` or the error.
fn parse_apc(body: &[u8]) -> Option<Event> {
  let body = std::str::from_utf8(body).ok()?.strip_prefix('G')?;
  let (control, message) = body.split_once(';')?;
  let id = control
    .split(',')
    .find_map(|pair| pair.strip_prefix("i="))?
    .parse()
    .ok()?;

  Some(Event::Graphics {
    id,
    ok: message == "OK",
  })
}

/// A DCS string's body: a DECRPSS report of the graphic rendition, `1 $ r ... m` (or
/// `0` where the terminal did not know the request).
fn parse_dcs(body: &[u8]) -> Option<Event> {
  let body = std::str::from_utf8(body).ok()?;
  let attributes = body
    .strip_prefix("1$r")
    .or_else(|| body.strip_prefix("0$r"))?
    .strip_suffix('m')?;

  Some(Event::GraphicRendition(parse_sgr_list(attributes)))
}

/// The attributes an SGR parameter list names; a parameter the loop does not know
/// is skipped, and a color introducer takes its subparameters after colons or
/// semicolons alike.
fn parse_sgr_list(list: &str) -> Vec<Sgr> {
  let mut attributes = Vec::new();
  let mut fields = list.split(';').peekable();

  while let Some(field) = fields.next() {
    let mut parts = field.split(':').filter(|part| !part.is_empty());
    let Some(code) = parts.next()
    else {
      attributes.push(Sgr::Reset);

      continue;
    };
    let mut subparameters: Vec<&str> = parts.collect();

    if subparameters.is_empty() && matches!(code, "38" | "48" | "58") {
      let take = match fields.peek() {
        Some(&"2") => 4,
        Some(&"5") => 2,
        _ => 0,
      };

      subparameters.extend(fields.by_ref().take(take));
    }

    let attribute = match (code, subparameters.as_slice()) {
      ("0", []) => Sgr::Reset,
      ("1", []) => Sgr::Bold(true),
      ("22", []) => Sgr::Bold(false),
      ("4", [] | ["1"]) => Sgr::Underline(Underline::Single),
      ("4", ["0"]) | ("24", []) => Sgr::Underline(Underline::None),
      ("4", ["2"]) | ("21", []) => Sgr::Underline(Underline::Double),
      ("4", ["3"]) => Sgr::Underline(Underline::Curly),
      ("4", ["4"]) => Sgr::Underline(Underline::Dotted),
      ("4", ["5"]) => Sgr::Underline(Underline::Dashed),
      ("7", []) => Sgr::Reverse(true),
      ("27", []) => Sgr::Reverse(false),
      ("9", []) => Sgr::StrikeThrough(true),
      ("29", []) => Sgr::StrikeThrough(false),
      ("53", []) => Sgr::Overline(true),
      ("55", []) => Sgr::Overline(false),
      ("39", []) => Sgr::Foreground(Color::Reset),
      ("49", []) => Sgr::Background(Color::Reset),
      ("59", []) => Sgr::UnderlineColor(Color::Reset),
      ("38", parts) => match parse_color(parts) {
        Some(color) => Sgr::Foreground(color),
        None => continue,
      },
      ("48", parts) => match parse_color(parts) {
        Some(color) => Sgr::Background(color),
        None => continue,
      },
      ("58", parts) => match parse_color(parts) {
        Some(color) => Sgr::UnderlineColor(color),
        None => continue,
      },
      (code, []) => match code.parse::<u8>().ok() {
        Some(code @ (30..=37 | 90..=97)) => Sgr::Foreground(Color::Palette(base_color(code))),
        Some(code @ (40..=47 | 100..=107)) => Sgr::Background(Color::Palette(base_color(code))),
        _ => continue,
      },
      _ => continue,
    };

    attributes.push(attribute);
  }

  attributes
}

/// The palette index of a base color code: `30` to `37` and `40` to `47` the first
/// eight, `90` to `97` and `100` to `107` the bright eight.
fn base_color(code: u8) -> u8 {
  match code {
    30..=37 => code - 30,
    40..=47 => code - 40,
    90..=97 => code - 82,
    _ => code - 92,
  }
}

/// A color's subparameters after `38`, `48` or `58`: `2` then RGB, with or without a
/// color space before them, or `5` then a palette index.
fn parse_color(parts: &[&str]) -> Option<Color> {
  let number = |part: &str| part.parse::<u8>().ok();

  match parts {
    ["2", red, green, blue] | ["2", _, red, green, blue] => {
      Some(Color::Rgb(number(red)?, number(green)?, number(blue)?))
    }
    ["5", index] => Some(Color::Palette(number(index)?)),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Every event `bytes` decode to, fed as one chunk with nothing after it.
  fn events(bytes: &[u8]) -> Vec<Event> {
    let mut parser = Parser::default();

    parser.feed(bytes, false);

    std::iter::from_fn(|| parser.pop()).collect()
  }

  /// The one key `bytes` decode to.
  fn key_of(bytes: &[u8]) -> Key {
    match events(bytes).as_slice() {
      [Event::Key(key)] => *key,
      other => panic!("{other:?}"),
    }
  }

  /// Plain bytes, control bytes, escape prefixes and the legacy sequences decode to
  /// their keys, an uppercase letter with `Shift` and a control byte with `Ctrl`.
  #[test]
  fn legacy_keys_decode() {
    assert_eq!(key_of(b"a").code, KeyCode::Char('a'));
    assert!(key_of(b"A").modifiers.shift);
    assert_eq!(key_of("é".as_bytes()).code, KeyCode::Char('é'));
    assert_eq!(key_of(b"\r").code, KeyCode::Enter);
    assert_eq!(key_of(b"\x7f").code, KeyCode::Backspace);
    assert_eq!(key_of(b"\x1b").code, KeyCode::Escape);
    assert!(key_of(b"\x01").modifiers.ctrl && key_of(b"\x01").code == KeyCode::Char('a'));
    assert!(key_of(b"\x1bx").modifiers.alt && key_of(b"\x1bx").code == KeyCode::Char('x'));
    assert_eq!(key_of(b"\x1b[A").code, KeyCode::Up);
    assert_eq!(key_of(b"\x1bOD").code, KeyCode::Left);
    assert_eq!(key_of(b"\x1b[3~").code, KeyCode::Delete);
    assert_eq!(key_of(b"\x1b[15~").code, KeyCode::Function(5));
    assert!(key_of(b"\x1b[1;5C").modifiers.ctrl);
    assert_eq!(key_of(b"\x1b[1;5C").code, KeyCode::Right);
    assert_eq!(key_of(b"\x1b[[B").code, KeyCode::Function(2));
    assert!(key_of(b"\x1b[Z").modifiers.shift && key_of(b"\x1b[Z").code == KeyCode::BackTab);
  }

  /// A lone escape waits while more may come and is the key when nothing does; a
  /// sequence cut across chunks completes with the next.
  #[test]
  fn sequences_complete_across_chunks() {
    let mut parser = Parser::default();

    parser.feed(b"\x1b", true);

    assert_eq!(parser.pop(), None);

    parser.feed(b"[1;2", true);

    assert_eq!(parser.pop(), None);

    parser.feed(b"Hx", false);

    assert_eq!(
      parser.pop(),
      Some(Event::Key(Key::new(KeyCode::Home).with(Modifiers {
        shift: true,
        ..Modifiers::default()
      })))
    );
    assert_eq!(parser.pop(), Some(Event::Key(Key::new(KeyCode::Char('x')))));
    assert_eq!(parser.pop(), None);
  }

  /// A kitty key carries its modifiers, its event kind, Caps Lock, the keypad, and
  /// its alternates: a shifted key replaces the key with `Shift` dropped, a base
  /// layout key rides along, and a modifier alone sets its own flag.
  #[test]
  fn kitty_keys_decode() {
    let released = key_of(b"\x1b[97;1:3u");

    assert_eq!(
      (released.code, released.kind),
      (KeyCode::Char('a'), KeyKind::Release)
    );

    let held = key_of(b"\x1b[97;69:2u");

    assert!(held.caps_lock && held.kind == KeyKind::Repeat);
    assert!(key_of(b"\x1b[57400u").keypad);
    assert_eq!(key_of(b"\x1b[57400u").code, KeyCode::Char('1'));
    assert_eq!(key_of(b"\x1b[27u").code, KeyCode::Escape);
    assert_eq!(key_of(b"\x1b[57376u").code, KeyCode::Function(13));

    let bang = key_of(b"\x1b[49:33;2u");

    assert_eq!(
      (bang.code, bang.shifted, bang.modifiers.shift),
      (KeyCode::Char('!'), Some('!'), false)
    );

    let cyrillic = key_of(b"\x1b[1092::97;5u");

    assert_eq!(
      (cyrillic.code, cyrillic.base_layout),
      (KeyCode::Char('\u{444}'), Some('a'))
    );
    assert!(cyrillic.modifiers.ctrl);

    let shift = key_of(b"\x1b[57441;2u");

    assert_eq!(shift.code, KeyCode::Modifier(ModifierKey::LeftShift));
    assert!(shift.modifiers.shift);
    assert!(key_of(b"\x1b[97;9u").modifiers.meta, "super is meta");
    assert!(key_of(b"\x1b[97;33u").modifiers.meta, "meta is meta");
  }

  /// SGR mouse reports decode to presses, releases, drags, motion and wheels at
  /// zero-based cells with their modifiers, and the X10 form to the same.
  #[test]
  fn mouse_reports_decode() {
    assert_eq!(
      events(b"\x1b[<0;3;2M"),
      [Event::Mouse(Mouse {
        kind: MouseKind::Press(PointerButton::Primary),
        column: 2,
        row: 1,
        modifiers: Modifiers::default(),
      })]
    );
    assert!(matches!(
      events(b"\x1b[<2;1;1m").as_slice(),
      [Event::Mouse(Mouse {
        kind: MouseKind::Release(PointerButton::Secondary),
        ..
      })]
    ));
    assert!(matches!(
      events(b"\x1b[<32;5;5M").as_slice(),
      [Event::Mouse(Mouse {
        kind: MouseKind::Drag(PointerButton::Primary),
        ..
      })]
    ));
    assert!(matches!(
      events(b"\x1b[<35;5;5M").as_slice(),
      [Event::Mouse(Mouse {
        kind: MouseKind::Move,
        ..
      })]
    ));
    assert!(matches!(
      events(b"\x1b[<69;5;5M").as_slice(),
      [Event::Mouse(Mouse {
        kind: MouseKind::ScrollDown,
        modifiers: Modifiers { shift: true, .. },
        ..
      })]
    ));
    assert!(matches!(
      events(b"\x1b[M !!").as_slice(),
      [Event::Mouse(Mouse {
        kind: MouseKind::Press(PointerButton::Primary),
        column: 0,
        row: 0,
        ..
      })]
    ));
  }

  /// A bracketed paste is one event holding its text, waiting for its closing
  /// bracket.
  #[test]
  fn pastes_decode_whole() {
    let mut parser = Parser::default();

    parser.feed(b"\x1b[200~hello\x1b[A", true);

    assert_eq!(parser.pop(), None);

    parser.feed(b"\x1b[201~", false);

    assert_eq!(parser.pop(), Some(Event::Paste("hello\x1b[A".to_owned())));
    assert_eq!(events(b"\x1b[200~\x1b[201~"), [Event::Paste(String::new())]);
  }

  /// The replies the loop asks for decode: mode reports, the theme, the cursor
  /// position, the keyboard flags, the resize notification, the background, the
  /// clipboard and the graphic rendition; a query echoed and a string unknown are
  /// dropped whole.
  #[test]
  fn replies_decode() {
    assert_eq!(
      events(b"\x1b[?2026;2$y"),
      [Event::Mode {
        mode: 2026,
        setting: ModeSetting::Reset
      }]
    );
    assert_eq!(events(b"\x1b[?997;1n"), [Event::Theme { dark: true }]);
    assert_eq!(
      events(b"\x1b[1;3R"),
      [Event::CursorPosition { column: 2, row: 0 }]
    );
    assert_eq!(events(b"\x1b[?15u"), [Event::KeyboardFlags(15)]);
    assert_eq!(
      events(b"\x1b[48;24;80;0;800t"),
      [Event::Resized(WindowSize {
        columns: 80,
        rows: 24,
        pixel_width: Some(800),
        pixel_height: None,
      })]
    );
    assert_eq!(
      events(b"\x1b]11;rgb:2828/2828/2828\x1b\\"),
      [Event::Background(40, 40, 40)]
    );
    assert_eq!(
      events(b"\x1b]11;rgb:f/80/0\x07"),
      [Event::Background(255, 128, 0)]
    );
    assert_eq!(
      events(b"\x1b]10;rgb:ffff/ffff/ffff\x07"),
      [Event::Foreground(255, 255, 255)]
    );
    assert_eq!(
      events(b"\x1b]4;0;rgb:2828/2c2c/3434;9;rgb:ffff/5555/5555\x1b\\"),
      [Event::Palette(vec![(0, (40, 44, 52)), (9, (255, 85, 85))])]
    );
    assert_eq!(events(b"\x1b]4;0;?\x07"), []);
    assert_eq!(
      events(b"\x1b]52;c;aGVsbG8=\x1b\\"),
      [Event::Clipboard("hello".to_owned())]
    );
    assert_eq!(events(b"\x1b]52;c;?\x07"), []);
    assert_eq!(
      events(b"\x1b]5522;type=read:status=OK\x1b\\\x1b]5522;type=read:status=DATA:mime=dGV4dC9wbGFpbg==;aGk=\x1b\\\x1b]5522;type=read:status=DONE\x1b\\"),
      [
        Event::ClipboardAnswer {
          read: true,
          status: ClipboardStatus::Ok
        },
        Event::ClipboardAnswer {
          read: true,
          status: ClipboardStatus::Data(b"hi".to_vec())
        },
        Event::ClipboardAnswer {
          read: true,
          status: ClipboardStatus::Done
        },
      ]
    );
    assert_eq!(
      events(b"\x1b]5522;type=write:status=EPERM\x1b\\"),
      [Event::ClipboardAnswer {
        read: false,
        status: ClipboardStatus::Refused
      }]
    );
    assert_eq!(events(b"\x1b]777;notify;hi\x07"), []);
    assert_eq!(
      events(b"\x1b_Gi=31;OK\x1b\\"),
      [Event::Graphics { id: 31, ok: true }]
    );
    assert_eq!(
      events(b"\x1b_Gi=7,I=2;ENOENT: no such\x1b\\"),
      [Event::Graphics { id: 7, ok: false }]
    );
    assert_eq!(events(b"\x1b_Xsomething\x1b\\"), []);
    assert_eq!(events(b"\x1b[?64;4;22c"), []);
    assert_eq!(
      events(b"\x1bP1$r0;4:3;48;2;18;52;86;58:2::1:2:3;38:5:200m\x1b\\"),
      [Event::GraphicRendition(vec![
        Sgr::Reset,
        Sgr::Underline(Underline::Curly),
        Sgr::Background(Color::Rgb(18, 52, 86)),
        Sgr::UnderlineColor(Color::Rgb(1, 2, 3)),
        Sgr::Foreground(Color::Palette(200)),
      ])]
    );
    assert_eq!(
      events(b"\x1bP0$r0;1;101;5m\x1b\\"),
      [Event::GraphicRendition(vec![
        Sgr::Reset,
        Sgr::Bold(true),
        Sgr::Background(Color::Palette(9)),
      ])]
    );
  }
}
