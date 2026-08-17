//! # skene-simple
//!
//! The Simple component library's shared design language: the palette and the emphasis
//! variants, expressed over the platform-agnostic color vocabulary and generic over the
//! condition vocabulary each facade owns. Each facade builds its own `Simple*` views
//! over its raw controls and imports this crate for the colors, so the GUI and the
//! terminal read as one design wherever the medium allows.
//!
//! A palette is data—a struct of [`Token`]s—and this crate holds no machinery a
//! third-party component library could not write for its own palette type.

mod palette;

pub use self::palette::{SimpleLook, SimplePalette, SimpleVariant};
