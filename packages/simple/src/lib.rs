//! # skene-simple
//!
//! The core of the Simple component library, built once over [`skene_foundation`]: one
//! palette, one shape language, the marks a medium drawing with characters shows, and
//! the logic of every `Simple*` view, generic over the
//! [`Realization`](skene_foundation::Realization), its members reached through the
//! `Simple*Base` trait a platform's crate implements for a view of its own
//! (`skene-simple-web`, `skene-simple-terminal`).
//!
//! [`SimplePalette`] is a token set: `Theme::new(SimplePalette::default())` defines it
//! beneath the theme, every Simple view references its tokens by name, and a Simple
//! view with no theme above defines the default at the root. The palette also defines
//! the views' typography tokens, so a plain text under a Simple theme is inked in the
//! palette's foreground. Corners round where the medium has a
//! [`CornerRadius`](skene_foundation::CornerRadius) and stay square where it has none, and
//! the lengths come from the [`SimpleShape`] of the medium's unit.

mod button;
mod card;
mod checkbox;
mod context_menu;
pub mod glyphs;
mod palette;
mod picker;
mod radio;
mod shape;
mod slider;
mod text_editor;
mod text_field;
mod tooltip;

pub use self::{
  button::{SimpleButton, SimpleButtonBase},
  card::{SimpleCard, SimpleCardBase},
  checkbox::{SimpleCheckbox, SimpleCheckboxBase},
  context_menu::{SimpleContextMenu, SimpleContextMenuBase},
  glyphs::SimpleGlyphs,
  palette::{SimpleLook, SimplePalette, SimpleVariant},
  picker::{SimplePicker, SimplePickerBase},
  radio::{SimpleRadio, SimpleRadioBase, SimpleRadioGroup, SimpleRadioGroupBase},
  shape::{SimpleMarks, SimpleShape, SimpleUnit},
  slider::{SimpleSlider, SimpleSliderBase},
  text_editor::{SimpleTextEditor, SimpleTextEditorBase},
  text_field::{SimpleTextField, SimpleTextFieldBase},
  tooltip::{SimpleTooltip, SimpleTooltipBase},
};
