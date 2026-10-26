//! # skene-simple-terminal
//!
//! The Simple component library on the terminal: its presets of the terminal's built-in
//! views ([`SimpleButton`], [`SimpleCard`], [`SimpleTooltip`]), each returning the
//! built-in styled from [`skene_simple`]'s palette and shape in the terminal's unit;
//! every other `Simple*` view of [`skene_simple`] as a view of the terminal's own over
//! its `Active`; the palette in the terminal's unit; and a [`prelude`] an app imports
//! beside the facade's own.

#![cfg(skene_platform = "terminal")]

mod button;
mod card;
mod checkbox;
mod context_menu;
mod picker;
mod radio;
mod slider;
mod text_editor;
mod text_field;
mod tooltip;

pub use skene_simple::{
  SimpleCheckboxBase,
  SimpleContextMenuBase,
  SimpleGlyphs,
  SimplePickerBase,
  SimpleRadioBase,
  SimpleRadioGroupBase,
  SimpleSliderBase,
  SimpleTextEditorBase,
  SimpleTextFieldBase,
  SimpleVariant,
  glyphs as glyph,
};

pub use self::{
  button::SimpleButton,
  card::SimpleCard,
  checkbox::SimpleCheckbox,
  context_menu::SimpleContextMenu,
  picker::SimplePicker,
  radio::{SimpleRadio, SimpleRadioGroup},
  slider::SimpleSlider,
  text_editor::SimpleTextEditor,
  text_field::SimpleTextField,
  tooltip::SimpleTooltip,
};

/// The Simple palette in the terminal's unit; see [`skene_simple::SimplePalette`].
pub type SimplePalette = skene_simple::SimplePalette<i32>;
/// One emphasis variant's colors; see [`skene_simple::SimpleLook`].
pub type SimpleLook = skene_simple::SimpleLook<i32>;

/// The Simple shape in the terminal's unit.
fn shape() -> skene_simple::SimpleShape<i32> {
  <i32 as skene_simple::SimpleUnit>::shape()
}

/// Everything above, for `use skene_simple_terminal::prelude::*` beside the
/// facade's own prelude.
pub mod prelude {
  pub use super::*;
}
