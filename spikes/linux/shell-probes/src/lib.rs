//! What the probes share: a row whose look follows the states the realization reports.

use skene_gui::prelude::*;

/// A row's fill at rest, under the pointer, pressed, and focused from the keyboard.
pub const REST: Color = Color::rgb(0xffffff);
/// Under the pointer.
pub const HOVER: Color = Color::rgb(0xdbeafe);
/// Pressed.
pub const PRESSED: Color = Color::rgb(0x93c5fd);
/// Focused from the keyboard.
pub const FOCUS: Color = Color::rgb(0xfde68a);

/// The fill of a row that answers hover, press and keyboard focus.
pub fn stateful() -> Value<Color> {
  REST
    .when(State::FocusVisible, FOCUS)
    .when(State::Hovered, HOVER)
    .when(State::Pressed, PRESSED)
}
