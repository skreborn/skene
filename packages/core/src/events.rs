//! The portable event vocabulary: what a handler receives, with no platform types—and
//! the dispatch contract every facade honors.
//!
//! Dispatch is deterministic and identical on every facade. For each event, the user
//! handlers run first, in a fixed order: deepest-to-shallowest along the hit path for
//! pointer events and clicks, the focused node then its ancestors for keys. Every
//! handler on one node always runs; bubbling is between nodes. After the handlers, the
//! facade's built-in default runs—focus movement on `Tab`, activation on `Enter`/
//! `Space` and click (a control's toggle, check, cycle, or jump), control arrows,
//! input editing, `Escape` dismissal—unless a handler prevented it.
//!
//! A handler's closure returns its verdict through [`IntoPrevent`]: nothing (`()`) to
//! observe, a [`Prevent`] to suppress the default, stop bubbling, or both; prevention
//! only accumulates. Notification events—change, toggle, scroll offsets, focus and
//! blur—report what already happened and take no verdict, and a terminal's `Ctrl-C`
//! stays unconditional.

/// What a handler prevents of the dispatch that invoked it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Prevent {
  /// Suppresses the built-in default; shallower handlers still run.
  Default,
  /// Skips the shallower handlers; the built-in default still runs.
  Bubbling,
  /// Both: no default, no shallower handlers.
  All,
}

/// A handler's verdict, converted from whatever its closure returns.
///
/// Implemented for `()` (an observing handler needs no return), for [`Prevent`]
/// (returned directly), and for `Option<Prevent>` (a conditional verdict).
pub trait IntoPrevent {
  /// The prevention this value requests, if any.
  fn into_prevent(self) -> Option<Prevent>;
}

impl IntoPrevent for () {
  #[inline(always)]
  fn into_prevent(self) -> Option<Prevent> {
    None
  }
}

impl IntoPrevent for Prevent {
  #[inline(always)]
  fn into_prevent(self) -> Option<Prevent> {
    Some(self)
  }
}

impl IntoPrevent for Option<Prevent> {
  #[inline(always)]
  fn into_prevent(self) -> Option<Prevent> {
    self
  }
}

/// The accumulated prevention of one dispatch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub struct Prevention {
  /// Whether the built-in default is suppressed.
  pub default: bool,
  /// Whether bubbling stops after the current node.
  pub bubbling: bool,
}

impl Prevention {
  /// Folds one handler's verdict in; prevention only accumulates.
  pub fn absorb(&mut self, verdict: Option<Prevent>) {
    match verdict {
      Some(Prevent::Default) => self.default = true,
      Some(Prevent::Bubbling) => self.bubbling = true,
      Some(Prevent::All) => {
        self.default = true;
        self.bubbling = true;
      }
      None => {}
    }
  }

  /// Folds another accumulated prevention in.
  pub fn merge(&mut self, other: Self) {
    self.default |= other.default;
    self.bubbling |= other.bubbling;
  }
}

/// A click handler as dispatch stores it: the closure plus its verdict.
pub type ClickHandler = dyn Fn() -> Option<Prevent>;
/// A key handler as dispatch stores it: the closure plus its verdict.
pub type KeyHandler = dyn Fn(&KeyEvent) -> Option<Prevent>;
/// A pointer handler as dispatch stores it: the closure plus its verdict.
pub type PointerHandler = dyn Fn(&PointerEvent) -> Option<Prevent>;

/// The modifier-key state accompanying an input event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
// One bool per modifier key is the domain shape.
#[expect(clippy::struct_excessive_bools)]
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

/// A keyboard event: the logical key plus modifier state.
///
/// `key` follows the W3C key-value names (`"a"`, `"Enter"`, `"ArrowLeft"`, …); every
/// driver maps its platform's keys onto them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct KeyEvent {
  /// The logical key value.
  pub key: String,
  /// The modifier state.
  pub modifiers: Modifiers,
}

/// A pointer button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum PointerButton {
  /// The primary button (usually the left).
  Primary,
  /// The secondary button (usually the right).
  Secondary,
  /// The auxiliary button (usually the wheel).
  Auxiliary,
}

/// A pointer event: where, which button, and the modifier state.
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
  /// The modifier state.
  pub modifiers: Modifiers,
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
