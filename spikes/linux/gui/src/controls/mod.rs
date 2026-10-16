//! The built-in controls: each a [`crate::Control`] over a shipped behavior, composed of
//! Skene's views and styled by ordinary values, so every default is a value its user
//! overrides and every part a view its user replaces (`set_<part>`), starting from the
//! default the control hands back (`default_<part>()`).
//!
//! Colors are [`skene_foundation::SystemColors`] tokens, which the platform defines at the
//! root and any `Theme` beneath redefines; lengths are the literals each default's
//! documentation states. No default changes a layout property with a state or a value: a
//! press, a check, a flip or a drag changes paint alone (a switch's thumb and a slider's fill
//! move by `offset`), so a change costs no layout.

mod button;
mod checkbox;
pub mod marks;
mod picker;
mod radio;
mod slider;
mod text_field;
mod toggle;

#[cfg(test)]
mod tests;

use skene_foundation::{Ring, State, Value, When as _};

pub use self::{
  button::Button,
  checkbox::Checkbox,
  picker::{BasicPicker, Picker, PickerItem},
  radio::{Radio, RadioGroup},
  slider::Slider,
  text_field::{Keyboard, TextEditor, TextField},
  toggle::Toggle,
};

/// The ring a built-in shows while focus shows on it: two pixels wide, at its edge.
fn focus_ring() -> Value<Ring> {
  Ring::NONE.when(State::FocusVisible, Ring::new(2))
}
