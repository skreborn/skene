//! # skene-simple
//!
//! The Simple component library built once over [`skene_views`], generic over the
//! [`Driver`](skene_views::Driver) a facade names: one palette, one shape language, and
//! every `Simple*` view composed from the public views alone, so a third-party library
//! sits on the same footing.
//!
//! [`SimplePalette`] is a token set: `Theme::new(SimplePalette::default())` defines it
//! beneath the theme, every Simple view references its tokens by name, and a Simple
//! view with no theme above defines the default at the root. The palette also defines
//! the views' typography tokens, so a plain text under a Simple theme is inked in the
//! palette's foreground. Corners round where the medium has a
//! [`CornerRadius`](skene_views::CornerRadius) and stay square where it has none, and
//! the lengths come from the [`SimpleShape`] of the medium's unit.

mod button;
mod card;
mod checkbox;
mod palette;
mod picker;
mod radio;
mod shape;
mod slider;
mod text_editor;
mod text_field;

pub use self::{
  button::SimpleButton,
  card::SimpleCard,
  checkbox::SimpleCheckbox,
  palette::{SimpleLook, SimplePalette, SimpleVariant},
  picker::SimplePicker,
  radio::{SimpleRadio, SimpleRadioGroup},
  shape::{SimpleShape, SimpleUnit},
  slider::SimpleSlider,
  text_editor::SimpleTextEditor,
  text_field::SimpleTextField,
};
