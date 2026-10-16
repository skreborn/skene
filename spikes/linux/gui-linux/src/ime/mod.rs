//! Input methods: the text-input-v3 client on Wayland ([`wayland`]), and on X11 IBus and
//! Fcitx5 over D-Bus as their GTK modules talk to them ([`ibus`], [`fcitx`]) with XIM as the
//! fallback ([`xim`]), chosen by [`choose`] from `GTK_IM_MODULE`, `QT_IM_MODULE`, `XMODIFIERS`
//! and the D-Bus names present. Where none runs, the keyboard's own compose table types
//! ([`crate::keys`]).
//!
//! Every X11 client reports through [`ImeEvent`]s in protocol order: a D-Bus client answers each
//! key with [`ImeEvent::KeyDone`] after the signals the key caused, so the backend holds later
//! keys behind an unanswered one; XIM never answers a consumed key, so the keys it does not
//! consume come back as [`ImeEvent::Forward`].

pub mod dbus;
pub mod fcitx;
pub mod ibus;
pub mod wayland;
pub mod xim;

use std::time::Instant;

/// One raw X key event, as the backend saw it.
#[derive(Clone, Debug)]
pub struct RawKey {
  /// The X keycode (evdev + 8).
  pub keycode: u32,
  /// The keysym under the XKB state at the time of the event, without compose.
  pub keysym: u32,
  /// The X modifier mask (Shift 1, Lock 2, Control 4, Mod1 8, ...).
  pub state: u32,
  /// Whether the key was released.
  pub release: bool,
  /// The server time of the event.
  pub time: u32,
  /// The event itself, for XIM's forwarding.
  pub event: x11rb::protocol::xproto::KeyPressEvent,
}

/// What an input method tells the client, in the order it said it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeEvent {
  /// The client connected and has a context.
  Ready(String),
  /// Insert `text` at the caret, replacing the composition.
  Commit(String),
  /// Show `text` as the composition, the caret at a char offset into it; empty hides it.
  Preedit(String, Option<u32>),
  /// Delete `len` chars starting `offset` chars from the caret.
  DeleteSurrounding(i32, u32),
  /// A key the input method gives back for the client to handle: a keysym (D-Bus) or a raw
  /// keycode to resolve under the current XKB state (XIM).
  Forward {
    /// The keysym, where the input method names one.
    keysym: u32,
    /// The keycode, where the input method gives the event back whole.
    keycode: Option<u32>,
    /// The modifier mask.
    state: u32,
    /// Whether it is a release.
    release: bool,
  },
  /// The input method answered key `id`.
  KeyDone {
    /// The key.
    id: u64,
    /// Whether it consumed it.
    handled: bool,
  },
  /// The input method asks for the surrounding text.
  RequireSurrounding,
  /// Anything worth a log line.
  Note(String),
}

/// An input method connection on X11.
pub trait Ime {
  /// Its name in logs.
  fn name(&self) -> &'static str;

  /// Whether the input method answers every key ([`ImeEvent::KeyDone`]); XIM does not.
  fn answers_keys(&self) -> bool;

  /// Sends a key; returns whether it was sent.
  fn process_key(&mut self, id: u64, key: &RawKey) -> bool;

  /// The focused entry gained the keyboard.
  fn focus_in(&mut self);

  /// The focused entry lost the keyboard.
  fn focus_out(&mut self);

  /// Drops the input method's composition (after the client committed or discarded it).
  fn reset(&mut self);

  /// The caret: `root` in root-window coordinates, `window` in the client window's, each
  /// `x, y, width, height` in device pixels.
  fn set_cursor(&mut self, root: (i32, i32, i32, i32), window: (i32, i32, i32, i32));

  /// The text around the caret, offsets in chars.
  fn set_surrounding(&mut self, text: &str, cursor: u32, anchor: u32);

  /// What the focused entry's text is for: the input method offers what suits it.
  fn set_purpose(&mut self, _purpose: Purpose) {}

  /// Handles an X event meant for the input method (XIM's transport); returns whether it was.
  fn filter_event(&mut self, _event: &x11rb::protocol::Event) -> bool {
    false
  }

  /// Events produced while filtering X events (XIM), drained by the backend.
  fn take_events(&mut self) -> Vec<ImeEvent> {
    Vec::new()
  }
}

/// What an entry's text is for, as an input method is told it: the keyboard the app asked for
/// (`TextField::set_keyboard`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Purpose {
  /// Any text.
  #[default]
  Normal,
  /// An email address.
  Email,
  /// A number.
  Number,
  /// A phone number.
  Phone,
  /// A URL.
  Url,
}

impl Purpose {
  /// The purpose of the keyboard named `keyboard` (`email`, `number`, `phone`, `url`).
  #[must_use]
  pub fn of(keyboard: Option<&str>) -> Self {
    match keyboard {
      Some("email") => Self::Email,
      Some("number") => Self::Number,
      Some("phone") => Self::Phone,
      Some("url") => Self::Url,
      _ => Self::Normal,
    }
  }

  /// IBus's input purpose: free form 0, number 3, phone 4, URL 5, email 6.
  #[must_use]
  pub const fn ibus(self) -> u32 {
    match self {
      Self::Normal => 0,
      Self::Number => 3,
      Self::Phone => 4,
      Self::Url => 5,
      Self::Email => 6,
    }
  }

  /// Fcitx5's capability bits for it: email 1 << 7, URL 1 << 12, dialable 1 << 13, number
  /// 1 << 14.
  #[must_use]
  pub const fn fcitx(self) -> u64 {
    match self {
      Self::Normal => 0,
      Self::Email => 1 << 7,
      Self::Url => 1 << 12,
      Self::Phone => 1 << 13,
      Self::Number => 1 << 14,
    }
  }
}

/// Which input method to use: `SKENE_IME=ibus|fcitx|xim|none` forces one; otherwise the
/// variables GTK and Qt read decide, then `XMODIFIERS` with the D-Bus names present, then the
/// names alone.
pub fn choose() -> &'static str {
  let forced = std::env::var("SKENE_IME").unwrap_or_default();

  if let Some(chosen) = ["ibus", "fcitx", "xim", "none"]
    .into_iter()
    .find(|name| *name == forced)
  {
    crate::trace::log(|| format!("[ime] SKENE_IME={forced:?} -> {chosen}"));

    return chosen;
  }

  let var = |name: &str| std::env::var(name).unwrap_or_default().to_lowercase();
  let (gtk, qt, xmodifiers) = (var("GTK_IM_MODULE"), var("QT_IM_MODULE"), var("XMODIFIERS"));
  let chosen = if gtk == "ibus" || qt == "ibus" {
    "ibus"
  } else if gtk.starts_with("fcitx") || qt.starts_with("fcitx") {
    "fcitx"
  } else if gtk == "xim" || qt == "xim" {
    "xim"
  } else if xmodifiers.contains("@im=ibus") && ibus::address().is_some() {
    "ibus"
  } else if dbus::session_has_name("org.fcitx.Fcitx5") {
    "fcitx"
  } else if ibus::address().is_some() {
    "ibus"
  } else if xmodifiers.contains("@im=") {
    "xim"
  } else {
    "none"
  };

  crate::trace::log(|| {
    format!("[ime] GTK_IM_MODULE={gtk:?} QT_IM_MODULE={qt:?} XMODIFIERS={xmodifiers:?} -> {chosen}")
  });

  chosen
}

/// Logs how long an input method took to answer, where tracing is on.
pub fn log_latency(name: &str, what: &str, since: Instant) {
  crate::trace::log(|| format!("[latency] {name} {what} {}us", since.elapsed().as_micros()));
}

/// The byte offset of char `chars` in `text` (clamped to its end).
pub fn char_to_byte(text: &str, chars: usize) -> usize {
  text
    .char_indices()
    .nth(chars)
    .map_or(text.len(), |(index, _)| index)
}

/// The char offset of byte `byte` in `text`.
pub fn byte_to_char(text: &str, byte: usize) -> usize {
  text[..byte.min(text.len())].chars().count()
}
