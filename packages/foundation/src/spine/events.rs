//! The portable event vocabulary: what a handler receives, with no platform types—and
//! the dispatch contract every facade honors.
//!
//! Dispatch is deterministic and identical on every facade. For each event, the user
//! handlers run first, in a fixed order: deepest-to-shallowest along the hit path for
//! pointer events and clicks, the focused node then its ancestors for keys. Every
//! handler on one node always runs; bubbling is between nodes. After the handlers, the
//! platform's own response runs—the browser's default action, the terminal's focus
//! movement on `Tab` and activation of the control it realizes (a toggle, check, cycle,
//! or jump), input editing, `Escape` dismissal—unless a handler prevented it.
//!
//! A handler's closure returns its verdict as anything `Into<Prevent>`: `()` to
//! observe, or a [`Prevent`] to suppress the platform's response, stop bubbling, or
//! both; prevention only accumulates. Notification events—change, toggle, scroll
//! offsets, focus and blur—report what already happened and take no verdict, and a
//! terminal's `Ctrl-Q` stays unconditional.
//!
//! A view that composes its own behavior out of these events—every control that draws
//! itself—decides its own verdict where that behavior lives, and consumes the events it
//! acts on. A handler installed on such a view only observes; above a platform, nothing
//! states a verdict.

use std::{borrow::Cow, str::FromStr};

use keyboard_types::{Code, Key, NamedKey, webdriver::KeyInputState};
use ui_events::pointer::{PointerButton, PointerButtons, PointerId, PointerInfo, PointerType};

use crate::spine::Axes;

/// What a handler prevents of the dispatch that invoked it.
///
/// A handler returns anything `Into<Prevent>`; `()` converts to
/// [`None`](Prevent::None), so an observing handler needs no return at all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub enum Prevent {
  /// Prevents nothing: the platform's response runs and bubbling continues.
  #[default]
  None,
  /// Suppresses the platform's own response; shallower handlers still run.
  Platform,
  /// Skips the shallower handlers; the platform's response still runs.
  Bubbling,
  /// Both: no platform response, no shallower handlers.
  All,
}

impl From<()> for Prevent {
  #[inline(always)]
  fn from((): ()) -> Self {
    Self::None
  }
}

/// What one dispatch has had prevented so far.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub struct Prevented {
  /// Whether the platform's own response is suppressed.
  pub platform: bool,
  /// Whether bubbling stops after the current node.
  pub bubbling: bool,
}

impl Prevented {
  /// Folds one handler's verdict in; prevention only accumulates.
  pub const fn absorb(&mut self, verdict: Prevent) {
    match verdict {
      Prevent::None => {}
      Prevent::Platform => self.platform = true,
      Prevent::Bubbling => self.bubbling = true,
      Prevent::All => {
        self.platform = true;
        self.bubbling = true;
      }
    }
  }
}

/// Whether an event carries on to the ancestors after a handler has seen it.
///
/// The one decision a view's user makes about dispatch, and it is only about onward
/// travel: a view may handle an event and still let it climb. What the platform is told
/// belongs to the layer that owns the behavior, so it never appears in a handler's
/// signature.
///
/// A handler returns anything `Into<Bubbling>`; `()` converts to
/// [`Continue`](Bubbling::Continue), so an ordinary handler needs no return at all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub enum Bubbling {
  /// The event carries on to the ancestors.
  #[default]
  Continue,
  /// The event stops here; no ancestor sees it.
  Stop,
}

impl From<()> for Bubbling {
  #[inline(always)]
  fn from((): ()) -> Self {
    Self::Continue
  }
}

impl From<Bubbling> for Prevent {
  #[inline(always)]
  fn from(bubbling: Bubbling) -> Self {
    match bubbling {
      Bubbling::Continue => Self::None,
      Bubbling::Stop => Self::Bubbling,
    }
  }
}

/// A click handler as dispatch stores it: the closure plus its verdict.
pub type ClickHandler = dyn Fn(&ClickEvent) -> Prevent;
/// A key handler as dispatch stores it: the closure plus its verdict.
pub type KeyHandler = dyn Fn(&KeyEvent) -> Prevent;
/// A pointer handler as dispatch stores it: the closure plus its verdict.
pub type PointerHandler = dyn Fn(&PointerEvent) -> Prevent;
/// A wheel handler as dispatch stores it: the closure plus its verdict.
pub type WheelHandler = dyn Fn(&WheelEvent) -> Prevent;

/// The modifier-key state accompanying an input event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
#[expect(
  clippy::struct_excessive_bools,
  reason = "one bool per modifier key is the domain shape"
)]
pub struct Modifiers {
  /// Whether Control was held.
  pub ctrl: bool,
  /// Whether Shift was held.
  pub shift: bool,
  /// Whether Alt was held.
  pub alt: bool,
  /// Whether the platform command/meta key was held.
  pub meta: bool,
}

impl Modifiers {
  /// No modifier held.
  pub const NONE: Self = Self {
    ctrl: false,
    shift: false,
    alt: false,
    meta: false,
  };

  /// Control alone.
  pub const CTRL: Self = Self {
    ctrl: true,
    ..Self::NONE
  };

  /// Shift alone.
  pub const SHIFT: Self = Self {
    shift: true,
    ..Self::NONE
  };

  /// Alt alone.
  pub const ALT: Self = Self {
    alt: true,
    ..Self::NONE
  };

  /// The platform command/meta key alone.
  pub const META: Self = Self {
    meta: true,
    ..Self::NONE
  };

  /// These modifiers with `other`'s held as well.
  #[inline(always)]
  pub const fn with(self, other: Self) -> Self {
    Self {
      ctrl: self.ctrl || other.ctrl,
      shift: self.shift || other.shift,
      alt: self.alt || other.alt,
      meta: self.meta || other.meta,
    }
  }
}

/// A click: a press and release on one target, or a control's activation by a key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct ClickEvent {
  /// The modifier state.
  pub modifiers: Modifiers,
  /// The click's place in a run of quick clicks at one spot: `1` for a single click,
  /// `2` for the second of a double, and so on; `0` for a click no pointer made, as a
  /// key's.
  pub count: u8,
}

impl ClickEvent {
  /// A click of `count` quick clicks with no modifier held.
  #[inline(always)]
  pub const fn new(count: u8) -> Self {
    Self {
      modifiers: Modifiers::NONE,
      count,
    }
  }
}

/// A key pressed with exactly a set of modifiers held: what a shortcut answers to,
/// matched by [`KeyEvent::matches`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct Shortcut {
  modifiers: Modifiers,
  key: &'static str,
}

impl Shortcut {
  /// `key` pressed with `modifiers` held; `key` names it as
  /// [`KeyEvent::shortcut_key`] does: a lowercase letter or digit, or a key's W3C name
  /// (`"F9"`, `"Enter"`).
  #[inline(always)]
  pub const fn new(modifiers: Modifiers, key: &'static str) -> Self {
    Self { modifiers, key }
  }

  /// The modifiers held.
  #[inline(always)]
  pub const fn modifiers(&self) -> Modifiers {
    self.modifiers
  }

  /// The key.
  #[inline(always)]
  #[must_use]
  pub const fn key(&self) -> &'static str {
    self.key
  }
}

impl std::fmt::Display for Shortcut {
  /// The shortcut as a hint reads it: its modifiers, then its key, a letter capital,
  /// joined by `+` (`Ctrl+Shift+S`, `F2`).
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let held = [
      (self.modifiers.ctrl, "Ctrl"),
      (self.modifiers.alt, "Alt"),
      (self.modifiers.shift, "Shift"),
      (self.modifiers.meta, "Meta"),
    ];

    for (_, name) in held.into_iter().filter(|(held, _)| *held) {
      write!(formatter, "{name}+")?;
    }

    let mut characters = self.key.chars();

    match (characters.next(), characters.next()) {
      (Some(character), None) => write!(formatter, "{}", character.to_uppercase()),
      _ => formatter.write_str(self.key),
    }
  }
}

/// A keyboard event: the logical key, the physical key, and the modifier state.
///
/// `key` is the W3C key value (`Key::Character("a")`, `Key::Named(NamedKey::Enter)`, a
/// modifier's name for a modifier pressed alone) and `code` the W3C code of the key's
/// position on the standard layout (`Code::KeyA`, `Code::Digit1`, `Code::ShiftLeft`), so
/// a shortcut on `code` holds across keyboard layouts; every platform maps its own keys
/// onto them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct KeyEvent {
  /// The logical key value.
  pub key: Key,
  /// The physical key, `Code::Unidentified` where the platform does not know it.
  pub code: Code,
  /// The modifier state.
  pub modifiers: Modifiers,
  /// Whether the press is the platform's repeat of a key held down.
  pub repeat: bool,
  /// Whether the press is part of an input method's composition, which a control's
  /// behavior leaves to the input method.
  pub composing: bool,
}

/// What a key does once its handlers leave it to the platform, where the scene decides
/// it; a platform's own keys decide the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum KeyDefault {
  /// `Tab` moves focus to the next tab stop, or the previous one with `Shift`.
  Traverse {
    /// Whether focus moves back.
    back: bool,
  },
  /// `Escape` dismisses the topmost open overlay.
  Dismiss(crate::spine::NodeId),
  /// The menu key or `Shift-F10` asks for the context menu where focus is.
  ContextMenu,
}

impl KeyEvent {
  /// A press of the key its W3C key value `key` names (`"a"`, `" "`, `"Enter"`), with no
  /// modifier held, not repeating and outside a composition, at the position the key has
  /// on the standard US layout; a value naming no key is the text it types.
  #[inline(always)]
  pub fn new(key: &str) -> Self {
    let key = Key::from_str(key).unwrap_or_else(|_| Key::Character(key.to_owned()));
    let code = Self::code_of(&key);

    Self {
      key,
      code,
      modifiers: Modifiers::default(),
      repeat: false,
      composing: false,
    }
  }

  /// The W3C code of `key`'s position on the standard US layout: `Code::KeyA` for `"a"`
  /// or `"A"`, `Code::Digit1` for `"1"` or `"!"`, `Code::Space` for `" "`, the left
  /// modifier for a modifier, the code of its name for another named key, and
  /// `Code::Unidentified` for a key the layout does not have.
  #[must_use]
  pub fn code_of(key: &Key) -> Code {
    match key {
      Key::Character(text) => {
        let mut characters = text.chars();

        match (characters.next(), characters.next()) {
          (Some(character), None) => KeyInputState::new().dispatch_keydown(character).code,
          _ => Code::Unidentified,
        }
      }
      Key::Named(NamedKey::Shift) => Code::ShiftLeft,
      Key::Named(NamedKey::Control) => Code::ControlLeft,
      Key::Named(NamedKey::Alt) => Code::AltLeft,
      Key::Named(NamedKey::Meta) => Code::MetaLeft,
      Key::Named(NamedKey::AltGraph) => Code::AltRight,
      Key::Named(named) => Code::from_str(&named.to_string()).unwrap_or(Code::Unidentified),
    }
  }

  /// Whether the key is `named`.
  #[inline(always)]
  #[must_use]
  pub fn is(&self, named: NamedKey) -> bool {
    self.key == Key::Named(named)
  }

  /// Whether the key types `text`, as `" "` for the space bar.
  #[inline(always)]
  #[must_use]
  pub fn types(&self, text: &str) -> bool {
    matches!(&self.key, Key::Character(typed) if typed == text)
  }

  /// The key a shortcut names this one by: a letter lowercase whatever case it was
  /// typed in, the letter or digit on the physical key where the key typed is no ASCII
  /// character (one of a non-Latin layout), and the key value otherwise.
  #[must_use]
  pub fn shortcut_key(&self) -> Cow<'_, str> {
    let Key::Character(text) = &self.key
    else {
      return Cow::Owned(self.key.to_string());
    };
    let mut characters = text.chars();

    match (characters.next(), characters.next()) {
      (Some(character), None) if character.is_ascii_uppercase() => {
        Cow::Owned(character.to_ascii_lowercase().to_string())
      }
      (Some(character), None) if !character.is_ascii() => {
        let code = self.code.to_string();

        match code
          .strip_prefix("Key")
          .or_else(|| code.strip_prefix("Digit"))
        {
          Some(physical) if physical.len() == 1 => Cow::Owned(physical.to_ascii_lowercase()),
          _ => Cow::Borrowed(text),
        }
      }
      _ => Cow::Borrowed(text),
    }
  }

  /// Whether the event presses `shortcut`: its key, as
  /// [`shortcut_key`](Self::shortcut_key) names it, with exactly its modifiers held,
  /// whether or not the press repeats.
  #[must_use]
  pub fn matches(&self, shortcut: &Shortcut) -> bool {
    self.modifiers == shortcut.modifiers && self.shortcut_key() == shortcut.key
  }

  /// Whether the key shows focus, as `:focus-visible` holds after it: any key but a
  /// modifier pressed alone.
  #[inline(always)]
  #[must_use]
  pub const fn shows_focus(&self) -> bool {
    !self.is_modifier()
  }

  /// Whether the key is a modifier pressed alone.
  #[expect(
    deprecated,
    reason = "the legacy Hyper and Super keys still arrive from terminals and engines"
  )]
  #[inline(always)]
  #[must_use]
  pub const fn is_modifier(&self) -> bool {
    matches!(
      self.key,
      Key::Named(
        NamedKey::Shift
          | NamedKey::Control
          | NamedKey::Alt
          | NamedKey::Meta
          | NamedKey::AltGraph
          | NamedKey::Hyper
          | NamedKey::Super
      )
    )
  }
}

/// A pointer event: where, which pointer, its buttons and pressure, and the modifier
/// state.
///
/// Its buttons, pointer and pressure follow the W3C pointer events: `buttons` holds
/// every button down once the event took effect, and a device reporting no pressure
/// presses at `0.5` while a button is held and at `0` otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct PointerEvent {
  /// The pointer's horizontal position, in viewport logical pixels.
  pub x: f32,
  /// The pointer's vertical position, in viewport logical pixels.
  pub y: f32,
  /// The pointer's horizontal position, relative to the handling view's top-left
  /// corner.
  pub local_x: f32,
  /// The pointer's vertical position, relative to the handling view's top-left corner.
  pub local_y: f32,
  /// The button that changed state, or `None` for a move.
  pub button: Option<PointerButton>,
  /// The buttons held once the event took effect: with `button` for a press, without it
  /// for a release.
  pub buttons: PointerButtons,
  /// The modifier state.
  pub modifiers: Modifiers,
  /// The pointer and the kind of device it is; its id is `None` for an event no pointing
  /// device raised, as a context menu the keyboard opens.
  pub pointer: PointerInfo,
  /// The pressure on the pointer, from `0` to `1`.
  pub pressure: f32,
}

impl PointerEvent {
  /// The primary mouse at (`x`, `y`) in the viewport, local to a view at the viewport's
  /// origin, with no button held and no modifier: a move.
  pub fn mouse(x: f32, y: f32) -> Self {
    Self {
      x,
      y,
      local_x: x,
      local_y: y,
      button: None,
      buttons: PointerButtons::new(),
      modifiers: Modifiers::NONE,
      pointer: PointerInfo {
        pointer_id: Some(PointerId::PRIMARY),
        persistent_device_id: None,
        pointer_type: PointerType::Mouse,
      },
      pressure: 0.,
    }
  }

  /// The event as the press of `button` on a device reporting no pressure: `button`
  /// changed and is held beside the others.
  pub fn with_press(mut self, button: PointerButton) -> Self {
    self.button = Some(button);
    self.buttons.insert(button);
    self.pressure = 0.5;

    self
  }

  /// A wheel turned by `delta` in `unit`s where the pointer is.
  #[inline(always)]
  pub const fn wheel(&self, delta: Axes<f32>, unit: WheelUnit) -> WheelEvent {
    WheelEvent {
      x: self.x,
      y: self.y,
      local_x: self.local_x,
      local_y: self.local_y,
      delta,
      unit,
      modifiers: self.modifiers,
      pointer: self.pointer,
    }
  }

  /// The event as the release of `button` on a device reporting no pressure: `button`
  /// changed and the others stay held.
  pub fn with_release(mut self, button: PointerButton) -> Self {
    self.button = Some(button);
    self.buttons.remove(button);
    self.pressure = if self.buttons.is_empty() { 0. } else { 0.5 };

    self
  }
}

/// The unit a wheel's turn counts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum WheelUnit {
  /// The platform's own lengths: pixels on the web, cells in the terminal.
  Lengths,
  /// Lines of text: rows in the terminal.
  Lines,
  /// Pages of the scrolled box.
  Pages,
}

/// A wheel turned over a node, before the platform scrolls by it.
///
/// A behavior's verdict that suppresses the platform's response keeps the scroll from
/// happening.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct WheelEvent {
  /// The pointer's horizontal position, in viewport logical pixels.
  pub x: f32,
  /// The pointer's vertical position, in viewport logical pixels.
  pub y: f32,
  /// The pointer's horizontal position, relative to the handling view's top-left
  /// corner.
  pub local_x: f32,
  /// The pointer's vertical position, relative to the handling view's top-left corner.
  pub local_y: f32,
  /// How far the wheel turned along each axis, in `unit`s: positive toward the
  /// content's end, where the scroll it asks for moves the view.
  pub delta: Axes<f32>,
  /// The unit `delta` counts in.
  pub unit: WheelUnit,
  /// The modifier state.
  pub modifiers: Modifiers,
  /// The pointer the wheel turned under.
  pub pointer: PointerInfo,
}

impl WheelEvent {
  /// The pointer event at the wheel's position, for localizing it.
  pub fn position(&self) -> PointerEvent {
    PointerEvent {
      local_x: self.local_x,
      local_y: self.local_y,
      modifiers: self.modifiers,
      pointer: self.pointer,
      ..PointerEvent::mouse(self.x, self.y)
    }
  }
}

/// A scroll: the scrolled container's new offsets.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct ScrollEvent {
  /// The horizontal scroll offset, in logical pixels.
  pub x: f32,
  /// The vertical scroll offset, in logical pixels.
  pub y: f32,
}

#[cfg(test)]
mod tests {
  use keyboard_types::{Code, Key, NamedKey};
  use ui_events::pointer::{PointerButton, PointerButtons};

  use super::{KeyEvent, Modifiers, PointerEvent, Shortcut};

  /// A key's code is its position on the US layout, whatever the shifted form.
  #[test]
  fn the_code_is_the_keys_position_on_the_us_layout() {
    let code = |key: &str| KeyEvent::new(key).code;

    assert_eq!(code("a"), Code::KeyA);
    assert_eq!(code("A"), Code::KeyA);
    assert_eq!(code("1"), Code::Digit1);
    assert_eq!(code("!"), Code::Digit1);
    assert_eq!(code(" "), Code::Space);
    assert_eq!(code("?"), Code::Slash);
    assert_eq!(code("Shift"), Code::ShiftLeft);
    assert_eq!(code("ArrowLeft"), Code::ArrowLeft);
    assert_eq!(code("F12"), Code::F12);
    assert_eq!(code("ф"), Code::Unidentified);
    assert_eq!(KeyEvent::new("Enter").key, Key::Named(NamedKey::Enter));
    assert_eq!(KeyEvent::new("z").key, Key::Character("z".to_owned()));
  }

  /// A shortcut names a letter in lowercase, a non-Latin layout's key by the letter on
  /// its physical key, and a named key by its name.
  #[test]
  fn a_shortcut_names_a_key_across_case_and_layout() {
    let cyrillic = KeyEvent {
      code: Code::KeyC,
      ..KeyEvent::new("\u{441}")
    };
    let unplaced = KeyEvent::new("\u{441}");

    assert_eq!(KeyEvent::new("C").shortcut_key(), "c");
    assert_eq!(KeyEvent::new("c").shortcut_key(), "c");
    assert_eq!(cyrillic.shortcut_key(), "c");
    assert_eq!(unplaced.shortcut_key(), "\u{441}");
    assert_eq!(KeyEvent::new("Enter").shortcut_key(), "Enter");
    assert_eq!(KeyEvent::new("?").shortcut_key(), "?");
  }

  /// A press holds its button beside the others at a mouse's pressure, and a release
  /// keeps the others held, pressing at `0` once none is.
  #[test]
  fn a_press_and_a_release_change_the_buttons_held() {
    let both = PointerEvent::mouse(1., 2.)
      .with_press(PointerButton::Primary)
      .with_press(PointerButton::Secondary);

    assert_eq!(both.button, Some(PointerButton::Secondary));
    assert_eq!(
      both.buttons,
      PointerButton::Primary | PointerButton::Secondary
    );
    assert!((both.pressure - 0.5).abs() < f32::EPSILON);

    let one = both.with_release(PointerButton::Primary);

    assert_eq!(one.button, Some(PointerButton::Primary));
    assert_eq!(one.buttons, PointerButtons::from(PointerButton::Secondary));
    assert!((one.pressure - 0.5).abs() < f32::EPSILON);

    let none = one.with_release(PointerButton::Secondary);

    assert!(none.buttons.is_empty());
    assert!(none.pressure.abs() < f32::EPSILON);
  }

  /// A shortcut matches its key whatever the case or the repeat, with exactly its
  /// modifiers held.
  #[test]
  fn a_shortcut_matches_exactly_its_modifiers() {
    let alt_m = Shortcut::new(Modifiers::ALT, "m");
    let press = |key: &str, modifiers: Modifiers, repeat: bool| KeyEvent {
      modifiers,
      repeat,
      ..KeyEvent::new(key)
    };

    assert!(press("m", Modifiers::ALT, false).matches(&alt_m));
    assert!(press("M", Modifiers::ALT, true).matches(&alt_m));
    assert!(!press("m", Modifiers::ALT.with(Modifiers::SHIFT), false).matches(&alt_m));
    assert!(!press("m", Modifiers::NONE, false).matches(&alt_m));
    assert!(!press("n", Modifiers::ALT, false).matches(&alt_m));
  }
}
