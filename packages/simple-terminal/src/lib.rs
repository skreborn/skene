//! # skene-simple-terminal
//!
//! The Simple component library on the terminal: its presets of the terminal's built-in
//! views ([`SimpleButton`], [`SimpleCard`], [`SimpleTooltip`], [`SimpleCheckbox`],
//! [`SimpleRadioGroup`] and [`SimpleRadio`], [`SimplePicker`] and
//! [`SimpleListboxOption`]), each returning the built-in styled from [`skene_simple`]'s
//! palette and shape in the terminal's unit; [`SimpleGlyphs`], the token set of the
//! glyphs its marks are drawn with; every other `Simple*` view of [`skene_simple`] as a
//! view of the terminal's own over its `Active`; the palette in the terminal's unit; and
//! a [`prelude`] an app imports beside the facade's own.

#![cfg(skene_platform = "terminal")]

mod button;
mod card;
mod checkbox;
mod context_menu;
pub mod glyph;
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
use skene_terminal::{
  Active,
  AnyView,
  Container,
  ContainerBase,
  DecorationBase,
  HasChild,
  HasChildren,
  NodeId,
  Part,
  RunBase,
  Scene,
  Stack,
  StackBase,
  State,
  Text,
  TextBase,
  View,
  When,
};

pub use self::{
  button::SimpleButton,
  card::SimpleCard,
  checkbox::SimpleCheckbox,
  context_menu::SimpleContextMenu,
  glyph::SimpleGlyphs,
  picker::{SimpleListboxOption, SimplePicker},
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

/// A control's mark drawn in glyphs: the one `glyph` gives a checked control while the
/// enclosing control is checked, and the unchecked one otherwise, stacked in one place,
/// inked in the foreground and the ring while focus shows.
fn glyph_mark(glyph: fn(&SimpleGlyphs, bool) -> String) -> Stack {
  let shown = |checked: bool| {
    Container::new()
      .set_hidden(checked.when(State::Checked, !checked))
      .set_child(
        Text::derive(move || glyph(&SimpleGlyphs::current(), checked))
          .set_color(SimplePalette::FOREGROUND.when(State::FocusVisible, SimplePalette::RING)),
      )
  };

  Stack::new().set_children((shown(true), shown(false)))
}

/// `view`, recorded once built as `part` of the control it is mounted under.
fn as_part(part: Part, view: impl View + 'static) -> AnyView {
  AnyView::new(move || {
    let node: NodeId = skene_foundation::View::build(view);

    Scene::<Active>::with(|scene| scene.set_part(node, part));

    node
  })
}

/// Everything above, for `use skene_simple_terminal::prelude::*` beside the
/// facade's own prelude.
pub mod prelude {
  pub use super::*;
}
