//! # skene-simple-web
//!
//! The Simple component library on the web: its presets of the web's built-in views
//! ([`SimpleButton`], [`SimpleCard`], [`SimpleTooltip`], [`SimpleCheckbox`],
//! [`SimpleRadioGroup`] and [`SimpleRadio`], [`SimplePicker`] and
//! [`SimpleListboxOption`], [`SimpleSlider`], [`SimpleTextField`], [`SimpleTextEditor`]
//! and [`SimpleContextMenu`]), each returning the built-in styled from [`skene_simple`]'s
//! palette and shape in the web's unit; the palette in the web's unit; and a [`prelude`]
//! an app imports beside the facade's own.

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

use skene_foundation::{Dimension, MouseCursor};
pub use skene_simple::SimpleVariant;
use skene_web::{
  AnyView,
  DecoratedTextEntry,
  DecorationWithCornerRadius,
  Inset,
  NodeId,
  Part,
  Scene,
  State,
  View,
  When,
};

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

/// `entry` in the Simple field chrome: full width, padded, on the card surface inside a
/// hairline in the border color that becomes the ring while focus shows, rounded, and on
/// the disabled surface under the default cursor while disabled; its value selected in
/// the background color on the foreground.
fn field_chrome<E: DecoratedTextEntry>(entry: E) -> E {
  let shape = shape();

  entry
    .set_width(Dimension::Fraction(1.))
    .set_padding(shape.control_inset)
    .set_background(SimplePalette::CARD.when(State::Disabled, SimplePalette::DISABLED_SURFACE))
    .set_border_width(Inset::all(shape.field_ring))
    .set_border_color(SimplePalette::BORDER.when(State::FocusVisible, SimplePalette::RING))
    .set_corner_radius(shape.control_radius)
    .set_cursor(MouseCursor::Text.when(State::Disabled, MouseCursor::Default))
    .set_selection_color(SimplePalette::BACKGROUND)
    .set_selection_background(SimplePalette::FOREGROUND)
}

/// `view`, recorded once built as `part` of the control it is mounted under.
fn as_part(part: Part, view: impl View + 'static) -> AnyView {
  AnyView::new(move || {
    let node: NodeId = skene_foundation::View::build(view);

    Scene::with(|scene| scene.set_part(node, part));

    node
  })
}

/// Everything above, for `use skene_simple_web::prelude::*` beside the
/// facade's own prelude.
pub mod prelude {
  pub use super::*;
}
