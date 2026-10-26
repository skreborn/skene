//! # skene-simple-web
//!
//! The Simple component library on the web: its presets of the web's built-in views
//! ([`SimpleButton`], [`SimpleCard`], [`SimpleTooltip`], [`SimpleCheckbox`],
//! [`SimpleRadioGroup`] and [`SimpleRadio`], [`SimplePicker`] and
//! [`SimpleListboxOption`]), each returning the built-in styled from [`skene_simple`]'s
//! palette and shape in the web's unit; every other `Simple*` view of [`skene_simple`] as
//! a view of the web's own over its `Active`; the palette in the web's unit; and a
//! [`prelude`] an app imports beside the facade's own.

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
  SimpleContextMenuBase,
  SimpleSliderBase,
  SimpleTextEditorBase,
  SimpleTextFieldBase,
  SimpleVariant,
};
use skene_web::{Active, AnyView, NodeId, Part, Scene, View};

pub use self::{
  button::SimpleButton,
  card::SimpleCard,
  checkbox::SimpleCheckbox,
  context_menu::SimpleContextMenu,
  picker::{SimpleListboxOption, SimplePicker},
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

/// The Simple shape in the web's unit.
fn shape() -> skene_simple::SimpleShape<f32> {
  <f32 as skene_simple::SimpleUnit>::shape()
}

/// `view`, recorded once built as `part` of the control it is mounted under.
fn as_part(part: Part, view: impl View + 'static) -> AnyView {
  AnyView::new(move || {
    let node: NodeId = skene_foundation::View::build(view);

    Scene::<Active>::with(|scene| scene.set_part(node, part));

    node
  })
}

/// Everything above, for `use skene_simple_web::prelude::*` beside the
/// facade's own prelude.
pub mod prelude {
  pub use super::*;
}
