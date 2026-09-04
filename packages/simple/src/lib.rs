//! # skene-simple
//!
//! The Simple component library's shared design language: the palette and the emphasis
//! variants, expressed over the platform-agnostic color vocabulary and generic over the
//! condition vocabulary each facade owns. Each facade builds its own `Simple*` views
//! over its raw controls and imports this crate for the colors, so the GUI and the
//! terminal read as one design wherever the medium allows.
//!
//! A palette is data—a struct of [`Token`](skene_core::Token)s—and this crate holds no
//! machinery a third-party component library could not write for its own palette type.
//!
//! [`next`] is the library rebuilt over the generic views: the palette as a token set
//! and every `Simple*` view once, generic over the driver a facade names. It replaces
//! the flat exports once the facades build on the generic views.

pub mod next;
mod palette;

pub use self::palette::{SimpleLook, SimplePalette, SimpleVariant};
