//! The portable event vocabulary: what a handler receives, with no platform types.

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
