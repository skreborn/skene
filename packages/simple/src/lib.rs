//! # skene-simple
//!
//! The core of the Simple component library, built once over [`skene_foundation`] and
//! naming no platform: one palette with a look per emphasis variant, the variants
//! themselves, and the sizes every Simple preset is drawn with in each medium's unit.
//! Each facade's crate (`skene-simple-web`, `skene-simple-terminal`) writes the presets,
//! each returning the facade's built-in view styled from this palette and the
//! [`SimpleShape`] of its unit, exactly as a third party's library writes its own.
//!
//! [`SimplePalette`] is a token set: `Theme::new(SimplePalette::default())` defines it
//! beneath the theme, and every Simple preset references its tokens by name, leaving the
//! definition to a theme or a read of the palette, which defines the default at the root
//! where no theme provides one. The palette also defines the typography tokens, so a
//! plain text under a Simple theme is inked in the palette's foreground. Corners round
//! where the medium has a [`CornerRadius`](skene_foundation::CornerRadius) and stay
//! square where it has none.

mod palette;
mod shape;

pub use self::{
  palette::{SimpleLook, SimplePalette, SimpleVariant},
  shape::{SimpleShape, SimpleUnit},
};
