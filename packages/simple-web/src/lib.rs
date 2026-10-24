//! # skene-simple-web
//!
//! The Simple component library on the web: every `Simple*` view of
//! [`skene_simple`] as a view of the web's own over its `Active`, the
//! palette in the web's unit, and a [`prelude`] an app imports beside the facade's
//! own.

#![cfg(skene_platform = "web")]

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
  SimpleButtonBase,
  SimpleCardBase,
  SimpleCheckboxBase,
  SimpleContextMenuBase,
  SimplePickerBase,
  SimpleRadioBase,
  SimpleRadioGroupBase,
  SimpleSliderBase,
  SimpleTextEditorBase,
  SimpleTextFieldBase,
  SimpleTooltipBase,
  SimpleVariant,
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

/// The Simple palette in the web's unit; see [`skene_simple::SimplePalette`].
pub type SimplePalette = skene_simple::SimplePalette<f32>;
/// One emphasis variant's colors; see [`skene_simple::SimpleLook`].
pub type SimpleLook = skene_simple::SimpleLook<f32>;

/// Everything above, for `use skene_simple_web::prelude::*` beside the
/// facade's own prelude.
pub mod prelude {
  pub use super::*;
}
