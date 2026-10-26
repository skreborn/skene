//! # skene-simple
//!
//! The core of the Simple component library, built once over [`skene_foundation`] and
//! naming no platform: one palette with a look per emphasis variant, the variants
//! themselves, and the sizes every Simple preset is drawn with in each medium's unit.
//! Each facade's crate (`skene-simple-gui`, `skene-simple-terminal`) writes the presets,
//! each returning the facade's built-in view styled from this palette and the
//! [`SimpleShape`] of its unit, exactly as a third party's library writes its own.
//!
//! [`SimplePalette`] is a token set, and every Simple preset references its tokens by
//! name. Each reference carries the palette's default, so a preset is inked in it with
//! no theme above it and nothing to read: the default reaches the root the first time a
//! node references one of the tokens, and `Theme::new(palette)` redefines them beneath
//! the theme. The palette also defines the typography tokens, so a plain text wherever
//! the palette is defined is inked in its foreground. Corners round where the medium has
//! a [`CornerRadius`](skene_foundation::CornerRadius) and stay square where it has none.

mod palette;
mod shape;

pub use self::{
  palette::{SimpleLook, SimplePalette, SimpleVariant},
  shape::{SimpleShape, SimpleUnit},
};
