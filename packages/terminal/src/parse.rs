//! The terminal's input decoded: keys in the legacy and kitty encodings, mouse reports,
//! bracketed pastes, and the replies to the loop's queries.
//!
//! The parser is incremental: bytes arrive in chunks, a sequence cut at a chunk's end
//! completes with the next, and a sequence the loop never asked about is consumed
//! whole and dropped rather than read as keys. A string sequence (OSC, DCS, APC, PM,
//! SOS) reads as a reply only inside a reply window the loop opened; outside one its
//! introducer is the `Alt` chord that sends the same bytes. Each byte is scanned a
//! bounded number of times, and a sequence longer than it may be is dropped to its end
//! as it arrives, so the parser's time is linear in its input and its memory bounded.

use std::collections::VecDeque;

use skene_foundation::{Modifiers, PointerButton};

use crate::{
  device::{Note, WindowSize},
  escape::{Color, Sgr, Underline, base64},
};

/// The escape byte.
const ESC: u8 = 0x1b;

/// The bell, an OSC string's other terminator.
const BEL: u8 = 0x07;

/// Cancel and substitute, which abort a string sequence.
const CAN: u8 = 0x18;
const SUB: u8 = 0x1a;

/// The longest string sequence or bracketed paste read; a longer one is dropped.
const MAX_STRING: usize = 16 << 20;

/// The longest control sequence read; a longer one is dropped.
const MAX_CSI: usize = 4096;

/// The closing bracket of a bracketed paste.
const PASTE_END: &[u8] = b"\x1b[201~";

/// What reached the loop: the terminal's input or a reply to a query, a line from the
/// dev launcher, or a process signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
  /// A key pressed, repeated or released.
  Key(Key),
  /// A mouse report.
  Mouse(Mouse),
  /// A bracketed paste's text.
  Paste(String),
  /// A line the dev launcher wrote on its control pipe.
  Control(String),
  /// The window's new size: an in-band notification (mode 2048), or the platform's.
  Resized(WindowSize),
  /// A signal asked the process to end, by its number.
  EndRequested(i32),
  /// A signal asked the process to stop.
  StopRequested,
  /// The process continued after a stop.
  Continued,
  /// A DECRPM report of a DEC private mode.
  Mode { mode: u16, setting: ModeSetting },
  /// A color scheme report (mode 2031).
  Theme { dark: bool },
  /// A cursor position report, zero-based.
  CursorPosition { column: u16, row: u16 },
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
  ClipboardAnswer { read: bool, status: ClipboardStatus },
  /// The graphic rendition in force, a DECRPSS reply.
  GraphicRendition(Vec<Sgr>),
  /// The kitty graphics protocol's reply to a command on the image `id`: `OK`, or
  /// an error.
  Graphics { id: u32, ok: bool },
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

impl From<Note> for Event {
  fn from(note: Note) -> Self {
    match note {
      Note::EndRequested(signal) => Self::EndRequested(signal),
      Note::StopRequested => Self::StopRequested,
      Note::Continued => Self::Continued,
    }
  }
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

impl ModeSetting {
  /// Whether the mode is on: set, or permanently set.
  pub(crate) const fn enabled(self) -> bool {
    matches!(self, Self::Set | Self::PermanentlySet)
  }

  /// Whether the terminal can have the mode on: set, reset, or permanently set.
  pub(crate) const fn available(self) -> bool {
    matches!(self, Self::Set | Self::Reset | Self::PermanentlySet)
  }
}

/// A key event: the key, the modifiers held, and what the kitty protocol adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Key {
  /// The key.
  pub(crate) code: KeyCode,
  /// The modifiers held.
  pub(crate) modifiers: Modifiers,
  /// Whether the key was pressed, repeated or released.
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
  /// The text the key typed, as the layout and the modifiers made it, where the
  /// terminal reports it and it is one character.
  pub(crate) text: Option<char>,
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
      text: None,
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
  /// A character key, by the character it types.
  Char(char),
  /// Enter.
  Enter,
  /// Backspace.
  Backspace,
  /// Tab.
  Tab,
  /// Escape.
  Escape,
  /// The left arrow.
  Left,
  /// The right arrow.
  Right,
  /// The up arrow.
  Up,
  /// The down arrow.
  Down,
  /// Home.
  Home,
  /// End.
  End,
  /// Page Up.
  PageUp,
  /// Page Down.
  PageDown,
  /// Insert.
  Insert,
  /// Delete.
  Delete,
  /// The keypad's center key, Begin.
  KeypadBegin,
  /// A function key, by its number.
  Function(u8),
  /// A modifier key on its own.
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
  /// The bytes read and not yet parsed.
  buffer: Vec<u8>,
  /// The events parsed and not yet taken.
  events: VecDeque<Event>,
  /// The reply windows open.
  windows: u32,
  /// How much of the buffer's head the last scan read without finding its sequence's
  /// end, where the next one resumes.
  scanned: usize,
  /// The sequence being dropped to its end, too long to read.
  discarding: Option<Discard>,
}

impl Parser {
  /// Opens a reply window, in which a string sequence reads as a reply; the reply to
  /// the primary device attributes request written after the window's queries closes
  /// it.
  pub(crate) const fn open_reply_window(&mut self) {
    self.windows += 1;
  }

  /// Takes `bytes` in. An escape the input ends in waits for the rest of its sequence;
  /// see [`resolve_escape`](Self::resolve_escape).
  pub(crate) fn feed(&mut self, bytes: &[u8]) {
    self.buffer.extend_from_slice(bytes);

    while !self.buffer.is_empty() {
      if let Some(discard) = self.discarding {
        let Some(end) = discard.end(&self.buffer)
        else {
          self.buffer.drain(..discard.droppable(&self.buffer));

          break;
        };

        self.buffer.drain(..end);
        self.discarding = None;

        continue;
      }

      let (consumed, event) = match scan(&self.buffer, self.windows > 0, self.scanned) {
        Scan::Incomplete => {
          self.scanned = self.buffer.len();

          break;
        }
        Scan::Overlong(discard) => {
          self.discarding = Some(discard);

          (discard.droppable(&self.buffer), None)
        }
        Scan::Done(consumed, event) => (consumed, event),
        Scan::Fence(consumed) => {
          self.windows = self.windows.saturating_sub(1);

          (consumed, None)
        }
      };

      self.buffer.drain(..consumed);
      self.events.extend(event);
      self.scanned = 0;
    }
  }

  /// Whether the input ends in an escape a pause resolves: a lone `ESC`, or `ESC` with
  /// only `[` or `O` after it.
  pub(crate) const fn holds_escape(&self) -> bool {
    self.discarding.is_none() && matches!(self.buffer.as_slice(), [ESC] | [ESC, b'[' | b'O'])
  }

  /// Reads the escape the input ends in as a pause after it means: a lone `ESC` as the
  /// `Escape` key, and `ESC [` or `ESC O` as `Alt` with the key after it.
  pub(crate) fn resolve_escape(&mut self) {
    if !self.holds_escape() {
      return;
    }

    let event = match self.buffer.as_slice() {
      [_, second] => match scan_char(&[*second]) {
        Scan::Done(_, Some(Event::Key(mut key))) => {
          key.modifiers.alt = true;

          Some(Event::Key(key))
        }
        _ => None,
      },
      _ => Some(Event::Key(Key::new(KeyCode::Escape))),
    };

    self.buffer.clear();
    self.events.extend(event);
    self.scanned = 0;
  }

  /// The oldest event decoded and not yet taken.
  pub(crate) fn pop(&mut self) -> Option<Event> {
    self.events.pop_front()
  }
}

/// A sequence too long to read, dropped as it arrives up to its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Discard {
  /// A string sequence, ended as [`scan_string`] ends one; `bel` where `BEL` ends it
  /// too.
  String { bel: bool },
  /// A bracketed paste, ended by its closing bracket.
  Paste,
  /// A control sequence, ended by its final byte.
  Csi,
}

impl Discard {
  /// Where the sequence ends in `buffer`, the bytes up to it its own, when it does.
  fn end(self, buffer: &[u8]) -> Option<usize> {
    match self {
      Self::String { bel } => {
        let stop = string_stop(buffer, bel)?;

        match buffer[stop] {
          ESC => match buffer.get(stop + 1) {
            Some(b'\\') => Some(stop + 2),
            Some(_) => Some(stop),
            None => None,
          },
          _ => Some(stop + 1),
        }
      }
      Self::Paste => find(buffer, PASTE_END).map(|at| at + PASTE_END.len()),
      Self::Csi => buffer
        .iter()
        .position(|byte| !(0x20..=0x3f).contains(byte))
        .map(|at| at + 1),
    }
  }

  /// How much of `buffer`, holding no end of the sequence, can go: all but what may
  /// begin its end.
  fn droppable(self, buffer: &[u8]) -> usize {
    let kept = match self {
      Self::String { .. } => usize::from(buffer.last() == Some(&ESC)),
      Self::Paste => PASTE_END.len() - 1,
      Self::Csi => 0,
    };

    buffer.len().saturating_sub(kept)
  }
}

/// What one scan of the buffer's head found.
enum Scan {
  /// The head is a sequence still arriving.
  Incomplete,
  /// The head is a sequence longer than it may be, to be dropped to its end.
  Overlong(Discard),
  /// `consumed` bytes make one input, or nothing the loop reads.
  Done(usize, Option<Event>),
  /// `consumed` bytes are the reply to the primary device attributes request, which
  /// closes a reply window.
  Fence(usize),
}

/// Scans the buffer's head for one input, resuming a sequence's search for its end past
/// the `scanned` bytes a scan already read; a string sequence is a reply where
/// `replying`.
fn scan(buffer: &[u8], replying: bool, scanned: usize) -> Scan {
  let Some(first) = buffer.first()
  else {
    return Scan::Incomplete;
  };

  match *first {
    ESC => scan_escape(buffer, replying, scanned),
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
const fn key(consumed: usize, code: KeyCode) -> Scan {
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

/// A UTF-8 character's key; an uppercase letter carries `Shift`. A lead byte without
/// the continuation bytes it announces is dropped as soon as another byte takes their
/// place.
fn scan_char(buffer: &[u8]) -> Scan {
  let needed = match buffer[0] {
    0x00..=0x7f => 1,
    0xc0..=0xdf => 2,
    0xe0..=0xef => 3,
    0xf0..=0xf7 => 4,
    _ => return Scan::Done(1, None),
  };

  if let Some(stray) = buffer
    .iter()
    .take(needed)
    .skip(1)
    .position(|byte| !(0x80..=0xbf).contains(byte))
  {
    return Scan::Done(stray + 1, None);
  }

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

/// An escape sequence, or `Alt` with the key after a lone escape; a string sequence
/// only where `replying`, its introducer an `Alt` chord elsewhere. A lone escape waits
/// for what follows it.
fn scan_escape(buffer: &[u8], replying: bool, scanned: usize) -> Scan {
  let Some(second) = buffer.get(1)
  else {
    return Scan::Incomplete;
  };

  match *second {
    b'[' => scan_csi(buffer, replying, scanned),
    b']' if replying => scan_string(buffer, true, scanned, parse_osc),
    b'P' if replying => scan_string(buffer, false, scanned, parse_dcs),
    b'_' if replying => scan_string(buffer, false, scanned, parse_apc),
    b'^' | b'X' if replying => scan_string(buffer, false, scanned, |_| None),
    b'O' => scan_ss3(buffer),
    ESC => key(1, KeyCode::Escape),
    _ => match scan(&buffer[1..], replying, scanned.saturating_sub(1)) {
      Scan::Done(consumed, Some(Event::Key(mut key))) => {
        key.modifiers.alt = true;

        Scan::Done(consumed + 1, Some(Event::Key(key)))
      }
      Scan::Done(consumed, event) => Scan::Done(consumed + 1, event),
      Scan::Fence(consumed) => Scan::Fence(consumed + 1),
      scan => scan,
    },
  }
}

/// An `SS3` key: the cursor keys and `F1` to `F4` in application mode, a modifier
/// number between the introducer and the final where one is held. `ESC O` before
/// anything else is `Alt` with `O`.
fn scan_ss3(buffer: &[u8]) -> Scan {
  let digits = buffer[2..]
    .iter()
    .take(4)
    .take_while(|byte| byte.is_ascii_digit())
    .count();
  let Some(last) = buffer.get(2 + digits)
  else {
    return Scan::Incomplete;
  };
  let code = match *last {
    b'A' => KeyCode::Up,
    b'B' => KeyCode::Down,
    b'C' => KeyCode::Right,
    b'D' => KeyCode::Left,
    b'H' => KeyCode::Home,
    b'F' => KeyCode::End,
    byte @ b'P'..=b'S' => KeyCode::Function(1 + byte - b'P'),
    _ if digits == 0 => {
      return Scan::Done(
        2,
        Some(Event::Key(Key::new(KeyCode::Char('O')).with(Modifiers {
          alt: true,
          shift: true,
          ..Modifiers::default()
        }))),
      );
    }
    _ => return Scan::Done(3 + digits, None),
  };
  let mask = std::str::from_utf8(&buffer[2..2 + digits])
    .ok()
    .and_then(|mask| mask.parse().ok());

  Scan::Done(
    3 + digits,
    Some(Event::Key(
      Key::new(code).with(mask.map_or_else(Modifiers::default, modifiers_of)),
    )),
  )
}

/// A string sequence (OSC, DCS, APC, PM, SOS) up to its terminator—`ST`, or `BEL` too
/// where `bel` says the kind takes it—handed to `parse` without its introducer and
/// terminator; the search resumes past the `scanned` bytes a scan already read.
///
/// An escape starting anything but `ST` ends the string unread, and begins the next
/// sequence; `CAN` or `SUB` abort it; a string longer than [`MAX_STRING`] is dropped.
fn scan_string(
  buffer: &[u8],
  bel: bool,
  scanned: usize,
  parse: impl Fn(&[u8]) -> Option<Event>,
) -> Scan {
  let from = scanned.saturating_sub(1).clamp(2, buffer.len());

  if let Some(stop) = string_stop(&buffer[from..], bel).map(|stop| from + stop) {
    return match buffer[stop] {
      BEL => Scan::Done(stop + 1, parse(&buffer[2..stop])),
      ESC => match buffer.get(stop + 1) {
        Some(b'\\') => Scan::Done(stop + 2, parse(&buffer[2..stop])),
        Some(_) => Scan::Done(stop, None),
        None => Scan::Incomplete,
      },
      _ => Scan::Done(stop + 1, None),
    };
  }

  if buffer.len() > MAX_STRING {
    return Scan::Overlong(Discard::String { bel });
  }

  Scan::Incomplete
}

/// A CSI sequence: its parameters, intermediates and final byte, dispatched by the
/// final. A cursor position report reads as one only where `replying`, and as the
/// modified `F3` it shares its bytes with elsewhere; a bracketed paste's search for its
/// end resumes past the `scanned` bytes a scan already read.
fn scan_csi(buffer: &[u8], replying: bool, scanned: usize) -> Scan {
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
    return if index > MAX_CSI {
      Scan::Overlong(Discard::Csi)
    }
    else {
      Scan::Incomplete
    };
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
    let from = scanned.saturating_sub(PASTE_END.len() - 1).max(consumed);

    return match find(&buffer[from..], PASTE_END) {
      Some(end) => Scan::Done(
        from + end + PASTE_END.len(),
        Some(Event::Paste(
          String::from_utf8_lossy(&buffer[consumed..from + end]).into_owned(),
        )),
      ),
      None if buffer.len() - consumed > MAX_STRING => Scan::Overlong(Discard::Paste),
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
    (Some(b'?'), b'c') => return Scan::Fence(consumed),
    (Some(b'<'), b'M' | b'm') => parse_sgr_mouse(&parameters[1..], *last == b'm'),
    (Some(b'?' | b'<' | b'=' | b'>'), _) => None,
    (_, b'R') if replying => parse_cursor_position(parameters),
    (_, b't') => parse_window_report(parameters),
    (_, b'u') => parse_kitty_key(parameters),
    (_, b'~') => parse_special_key(parameters),
    (_, b'Z') => Some(Event::Key(Key::new(KeyCode::Tab).with(Modifiers {
      shift: true,
      ..Modifiers::default()
    }))),
    (_, b'A' | b'B' | b'C' | b'D' | b'F' | b'H' | b'P' | b'Q' | b'R' | b'S') => {
      parse_modified_key(parameters, *last)
    }
    _ => None,
  };

  Scan::Done(consumed, event)
}

/// Where `needle` first appears in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
  memchr::memmem::find(haystack, needle)
}

/// Where the first byte of `bytes` that stops a string sequence stands: `ESC`, `CAN` or
/// `SUB`, or `BEL` where `bel` says the kind ends on it.
fn string_stop(bytes: &[u8], bel: bool) -> Option<usize> {
  let control = memchr::memchr3(ESC, CAN, SUB, bytes);

  if !bel {
    return control;
  }

  let bell = memchr::memchr(BEL, &bytes[..control.unwrap_or(bytes.len())]);

  bell.or(control)
}

/// The modifiers a mask names: the held keys less one, bit by bit.
const fn modifiers_of(mask: u16) -> Modifiers {
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
  let mask: u16 = parts.next().and_then(|mask| mask.parse().ok()).unwrap_or(1);
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
        Some('\t') => KeyCode::Tab,
        Some('\x7f') => KeyCode::Backspace,
        Some(character) => KeyCode::Char(character),
        None => KeyCode::Unidentified,
      },
      false,
    )
  });

  // A modifier's own key holds it while pressed and has let it go once released, as
  // a browser's modifier state is after the key's event.
  if let KeyCode::Modifier(modifier) = code {
    let held = kind != KeyKind::Release;

    match modifier {
      ModifierKey::LeftAlt | ModifierKey::RightAlt => modifiers.alt = held,
      ModifierKey::LeftControl | ModifierKey::RightControl => modifiers.ctrl = held,
      ModifierKey::LeftShift | ModifierKey::RightShift => modifiers.shift = held,
      ModifierKey::LeftSuper
      | ModifierKey::RightSuper
      | ModifierKey::LeftMeta
      | ModifierKey::RightMeta => modifiers.meta = held,
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
  let mut typed = fields
    .next()
    .into_iter()
    .flat_map(|text| text.split(':'))
    .filter_map(|code| code.parse::<u32>().ok().and_then(char::from_u32));
  let text = typed
    .next()
    .filter(|character| !character.is_control())
    .filter(|_| typed.next().is_none());

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
    text,
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
    b'R' => KeyCode::Function(3),
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
const fn mouse_button(byte: u8) -> Option<(MouseKind, Modifiers)> {
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
    (8, false) => MouseKind::Press(PointerButton::X1),
    (9, false) => MouseKind::Press(PointerButton::X2),
    (8, true) => MouseKind::Drag(PointerButton::X1),
    (9, true) => MouseKind::Drag(PointerButton::X2),
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

/// An X color, `rgb:RR/GG/BB` or `rgba:RR/GG/BB/AA` with one to four hex digits per
/// component, or `#RGB` with one to four digits per component, each scaled to eight
/// bits; an alpha is read past.
fn parse_rgb(text: &str) -> Option<(u8, u8, u8)> {
  let scaled = |digits: &str| {
    if digits.is_empty() || digits.len() > 4 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
      return None;
    }

    let value = u32::from_str_radix(digits, 16).ok()?;
    let full = (1u32 << (4 * digits.len())) - 1;

    u8::try_from((value * 255 + full / 2) / full).ok()
  };

  if let Some(hex) = text.strip_prefix('#') {
    if hex.is_empty() || hex.len() % 3 != 0 || !hex.is_ascii() {
      return None;
    }

    let width = hex.len() / 3;

    return Some((
      scaled(&hex[..width])?,
      scaled(&hex[width..2 * width])?,
      scaled(&hex[2 * width..])?,
    ));
  }

  let (components, count) = match text.strip_prefix("rgba:") {
    Some(components) => (components, 4),
    None => (text.strip_prefix("rgb:")?, 3),
  };
  let parts: Vec<&str> = components.split('/').collect();

  if parts.len() != count {
    return None;
  }

  Some((scaled(parts[0])?, scaled(parts[1])?, scaled(parts[2])?))
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
const fn base_color(code: u8) -> u8 {
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

  /// Every event `bytes` decode to, fed as one chunk with a pause after it, inside a
  /// reply window.
  fn events(bytes: &[u8]) -> Vec<Event> {
    let mut parser = Parser::default();

    parser.open_reply_window();
    parser.feed(bytes);
    parser.resolve_escape();

    std::iter::from_fn(|| parser.pop()).collect()
  }

  /// Every event `bytes` decode to, with a pause after them and no reply window open.
  fn input(bytes: &[u8]) -> Vec<Event> {
    let mut parser = Parser::default();

    parser.feed(bytes);
    parser.resolve_escape();

    std::iter::from_fn(|| parser.pop()).collect()
  }

  /// `key` with `Alt` held, and `Shift` where `shift`.
  fn alt(code: KeyCode, shift: bool) -> Event {
    Event::Key(Key::new(code).with(Modifiers {
      alt: true,
      shift,
      ..Modifiers::default()
    }))
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
    assert!(key_of(b"\x1b[Z").modifiers.shift && key_of(b"\x1b[Z").code == KeyCode::Tab);
  }

  /// A lone escape waits for what follows it and is the key after a pause; a sequence
  /// cut across chunks completes with the next.
  #[test]
  fn sequences_complete_across_chunks() {
    let mut parser = Parser::default();

    parser.feed(b"\x1b");

    assert_eq!(parser.pop(), None);
    assert!(parser.holds_escape());

    parser.feed(b"[1;2");

    assert_eq!(parser.pop(), None);
    assert!(!parser.holds_escape());

    parser.feed(b"Hx");

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
  /// zero-based cells with their modifiers, the back and forward buttons among them,
  /// and the X10 form to the same.
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
      events(b"\x1b[<128;5;5M").as_slice(),
      [Event::Mouse(Mouse {
        kind: MouseKind::Press(PointerButton::X1),
        ..
      })]
    ));
    assert!(matches!(
      events(b"\x1b[<129;5;5m").as_slice(),
      [Event::Mouse(Mouse {
        kind: MouseKind::Release(PointerButton::X2),
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

    parser.feed(b"\x1b[200~hello\x1b[A");

    assert_eq!(parser.pop(), None);

    parser.feed(b"\x1b[201~");

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

  /// The text a kitty key reports it typed rides the key: `AltGr`'s `@` on `q`.
  #[test]
  fn a_kitty_key_carries_its_text() {
    let key = key_of(b"\x1b[113;;64u");

    assert_eq!(key.code, KeyCode::Char('q'));
    assert_eq!(key.text, Some('@'));
    assert_eq!(key_of(b"\x1b[113u").text, None);
  }

  /// Outside a reply window a string introducer is the `Alt` chord that sends it, so
  /// `Alt+Shift+P` never swallows the keys after it.
  #[test]
  fn an_introducer_outside_a_window_is_a_chord() {
    assert_eq!(
      input(b"\x1bPq\x11"),
      [
        alt(KeyCode::Char('P'), true),
        Event::Key(Key::new(KeyCode::Char('q'))),
        Event::Key(Key::new(KeyCode::Char('q')).with(Modifiers {
          ctrl: true,
          ..Modifiers::default()
        })),
      ]
    );
    assert_eq!(input(b"\x1b]"), [alt(KeyCode::Char(']'), false)]);
  }

  /// The primary device attributes reply closes the window its queries opened.
  #[test]
  fn the_attributes_reply_closes_a_window() {
    let mut parser = Parser::default();

    parser.open_reply_window();
    parser.feed(b"\x1b]11;rgb:0000/0000/0000\x1b\\\x1b[?62;22c\x1bP");
    parser.resolve_escape();

    assert_eq!(parser.pop(), Some(Event::Background(0, 0, 0)));
    assert_eq!(parser.pop(), Some(alt(KeyCode::Char('P'), true)));
    assert_eq!(parser.pop(), None);
  }

  /// Inside a window an escape that starts no terminator ends a string unread, and
  /// `CAN` aborts one, so a lost terminator costs only the string.
  #[test]
  fn a_string_ends_at_the_next_sequence() {
    assert_eq!(
      events(b"\x1bPhello\x1b[A"),
      [Event::Key(Key::new(KeyCode::Up))]
    );
    assert_eq!(
      events(b"\x1b]11;x\x18a"),
      [Event::Key(Key::new(KeyCode::Char('a')))]
    );
  }

  /// A held escape reads after a pause as the key it alone makes: `Escape`, or `Alt`
  /// with the `[` or `O` after it; `ESC O` before a byte no key ends with is the chord,
  /// that byte its own key.
  #[test]
  fn a_held_escape_resolves_after_a_pause() {
    assert_eq!(input(b"\x1b"), [Event::Key(Key::new(KeyCode::Escape))]);
    assert_eq!(input(b"\x1b["), [alt(KeyCode::Char('['), false)]);
    assert_eq!(input(b"\x1bO"), [alt(KeyCode::Char('O'), true)]);
    assert_eq!(
      input(b"\x1bOx"),
      [
        alt(KeyCode::Char('O'), true),
        Event::Key(Key::new(KeyCode::Char('x')))
      ]
    );

    let mut parser = Parser::default();

    parser.feed(b"\x1b[1");
    parser.resolve_escape();

    assert_eq!(parser.pop(), None, "a longer sequence waits on");
  }

  /// A modified `F3` shares its bytes with a cursor position report and reads as the
  /// key outside a reply window; an `SS3` key carries the modifier number before its
  /// final.
  #[test]
  fn modified_function_keys_decode() {
    let ctrl = Modifiers {
      ctrl: true,
      ..Modifiers::default()
    };

    assert_eq!(
      input(b"\x1b[1;5R"),
      [Event::Key(Key::new(KeyCode::Function(3)).with(ctrl))]
    );
    assert_eq!(
      events(b"\x1b[1;5R"),
      [Event::CursorPosition { column: 4, row: 0 }]
    );
    assert_eq!(
      input(b"\x1bO5P"),
      [Event::Key(Key::new(KeyCode::Function(1)).with(ctrl))]
    );
    assert_eq!(
      input(b"\x1bO2S"),
      [Event::Key(Key::new(KeyCode::Function(4)).with(Modifiers {
        shift: true,
        ..Modifiers::default()
      }))]
    );
  }

  /// A lead byte whose continuation bytes never come is dropped as soon as another
  /// byte takes their place, which then reads as it would alone.
  #[test]
  fn a_stray_lead_byte_drops_at_once() {
    assert_eq!(input(b"\xe0\x1b[A"), [Event::Key(Key::new(KeyCode::Up))]);
    assert_eq!(
      input(b"\xf0\x9fa"),
      [Event::Key(Key::new(KeyCode::Char('a')))]
    );
    assert_eq!(
      input("\u{e9}".as_bytes()),
      [Event::Key(Key::new(KeyCode::Char('\u{e9}')))]
    );
  }

  /// A modifier's own key holds it while pressed and has let it go once released, and
  /// the mask with every modifier set decodes whole.
  #[test]
  fn modifier_keys_and_masks_decode() {
    let shift = |bytes: &[u8]| match input(bytes).as_slice() {
      [Event::Key(key)] => key.modifiers.shift,
      other => panic!("{other:?}"),
    };

    assert!(shift(b"\x1b[57441;2u"));
    assert!(!shift(b"\x1b[57441;1:3u"));

    let every = key_of(b"\x1b[97;256u");

    assert_eq!(
      every.modifiers,
      Modifiers {
        ctrl: true,
        shift: true,
        alt: true,
        meta: true,
      }
    );
    assert!(every.caps_lock);
  }

  /// A color reply decodes in the `rgb:`, `rgba:` and `#` forms, each component at its
  /// own precision.
  #[test]
  fn colors_decode_in_every_form() {
    assert_eq!(parse_rgb("rgb:ffff/0/80"), Some((255, 0, 128)));
    assert_eq!(parse_rgb("rgba:ffff/0000/0000/ffff"), Some((255, 0, 0)));
    assert_eq!(parse_rgb("#ff0080"), Some((255, 0, 128)));
    assert_eq!(parse_rgb("#f08"), Some((255, 0, 136)));
    assert_eq!(parse_rgb("rgb:ff/00"), None);
    assert_eq!(parse_rgb("rgba:ff/00/00"), None);
    assert_eq!(parse_rgb("#ff00"), None);
  }

  /// A string, a paste or a control sequence longer than it may be is dropped to its end
  /// as it arrives, the buffer never holding more than the longest one read and a
  /// chunk, and what follows it reads as usual.
  #[test]
  fn overlong_sequences_drop_to_their_end() {
    const CHUNK: usize = 1 << 16;

    let overlong = |introducer: &[u8], body: u8, length: usize, end: &[u8]| {
      let mut parser = Parser::default();
      let mut bytes = introducer.to_vec();

      parser.open_reply_window();
      bytes.resize(introducer.len() + length, body);
      bytes.extend_from_slice(end);
      bytes.push(b'x');

      for chunk in bytes.chunks(CHUNK) {
        parser.feed(chunk);

        assert!(parser.buffer.len() <= MAX_STRING + CHUNK);
      }

      std::iter::from_fn(|| parser.pop()).collect::<Vec<_>>()
    };
    let x = [Event::Key(Key::new(KeyCode::Char('x')))];

    assert_eq!(
      overlong(b"\x1b]11;", b'a', MAX_STRING + CHUNK, b"\x1b\\"),
      x
    );
    assert_eq!(
      overlong(b"\x1b[200~", b'a', MAX_STRING + CHUNK, PASTE_END),
      x
    );
    assert_eq!(overlong(b"\x1b[", b'1', MAX_CSI * 4, b"m"), x);
  }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod properties {
  use proptest::prelude::*;

  use super::*;

  /// Input mostly of the bytes escape sequences are made of, any byte among them.
  fn arbitrary_input() -> impl Strategy<Value = Vec<u8>> {
    const ALPHABET: &[u8] =
      b"\x1b\x1b\x1b[[]]OP_^X\\\x07\x18\x1a0123456789;:<=>?$~uRMmcyAPZh\xe0\x80\xbfa ";

    proptest::collection::vec(
      prop_oneof![
        7 => proptest::sample::select(ALPHABET),
        1 => any::<u8>(),
      ],
      0..96,
    )
  }

  proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// Arbitrary input decodes without panicking, and to the same events fed whole or in
    /// chunks of any size, a scan resuming where the last left off.
    #[test]
    fn any_input_decodes_alike_in_any_chunks(
      bytes in arbitrary_input(),
      cuts in proptest::collection::vec(any::<proptest::sample::Index>(), 0..8),
      replying in any::<bool>(),
    ) {
      let decode = |chunks: &[&[u8]]| {
        let mut parser = Parser::default();

        if replying {
          parser.open_reply_window();
        }

        for chunk in chunks {
          parser.feed(chunk);
        }

        parser.resolve_escape();

        std::iter::from_fn(|| parser.pop()).collect::<Vec<_>>()
      };
      let mut points: Vec<usize> = cuts.iter().map(|cut| cut.index(bytes.len() + 1)).collect();

      points.sort_unstable();

      let mut chunks = Vec::new();
      let mut start = 0;

      for point in points {
        chunks.push(&bytes[start..point]);
        start = point;
      }

      chunks.push(&bytes[start..]);

      prop_assert_eq!(decode(&[&bytes]), decode(&chunks), "{:?} in {:?}", bytes, chunks);
    }
  }
}
