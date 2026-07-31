//! The portable event vocabulary: what a handler receives, with no platform types.

/// A keyboard event: the logical key plus modifier state.
///
/// `key` follows the W3C key-value names (`"a"`, `"Enter"`, `"ArrowLeft"`, …); every
/// driver maps its platform's keys onto them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
// One bool per modifier key is the domain shape.
#[expect(clippy::struct_excessive_bools)]
pub struct KeyEvent {
  /// The logical key value.
  pub key: String,
  /// Whether Control was held.
  pub ctrl: bool,
  /// Whether Shift was held.
  pub shift: bool,
  /// Whether Alt was held.
  pub alt: bool,
  /// Whether the platform command/meta key was held.
  pub meta: bool,
}
