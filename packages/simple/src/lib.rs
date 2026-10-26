//! # skene-simple
//!
//! The core of the Simple component library, built once over [`skene_foundation`]: one
//! palette with a look per emphasis variant, one shape language, and the logic of the
//! other `Simple*` views, generic over the
//! [`Realization`](skene_foundation::Realization), their members reached through the
//! `Simple*Base` trait a platform's crate implements for a view of its own. Each
//! platform's crate (`skene-simple-web`, `skene-simple-terminal`) also writes its
//! presets, a button, a card, a tooltip, a checkbox, a radio group and its radios, and a
//! picker and its options, returning the facade's built-in styled from the palette and
//! the shape of its unit, the terminal's marks drawn from its own glyph token set.
//!
//! [`SimplePalette`] is a token set: `Theme::new(SimplePalette::default())` defines it
//! beneath the theme, and every Simple view and preset references its tokens by name. A
//! Simple view with no theme above defines the default at the root; a preset leaves it
//! to a theme or a read of the palette. The palette also defines the views' typography
//! tokens, so a plain text under a Simple theme is inked in the palette's foreground.
//! Corners round where the medium has a
//! [`CornerRadius`](skene_foundation::CornerRadius) and stay square where it has none, and
//! the lengths come from the [`SimpleShape`] of the medium's unit.

mod context_menu;
mod palette;
mod shape;
mod slider;
mod text_editor;
mod text_field;

pub use self::{
  context_menu::{SimpleContextMenu, SimpleContextMenuBase},
  palette::{SimpleLook, SimplePalette, SimpleVariant},
  shape::{SimpleShape, SimpleUnit},
  slider::{SimpleSlider, SimpleSliderBase},
  text_editor::{SimpleTextEditor, SimpleTextEditorBase},
  text_field::{SimpleTextField, SimpleTextFieldBase},
};
