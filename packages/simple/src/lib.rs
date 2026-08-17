//! # skene-simple
//!
//! The Simple component library's shared design language: the palette pair, the
//! emphasis variants with their resolved colors, and the scheme resolution every
//! facade's theme view runs on, expressed over the platform-agnostic color
//! vocabulary. Each facade builds its own `Simple*` views over its raw controls and
//! imports this crate for the colors, so the GUI and the terminal read as one design
//! wherever the medium allows.

mod palette;
mod theme;

pub use self::{
  palette::{Look, Palette, SimpleVariant},
  theme::{Palettes, SchemeCondition},
};
