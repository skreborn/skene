//! The terminal's own vocabulary, in [`terminal`].
//!
//! A platform's module holds what only that platform has, in its own build: the terminal
//! is this facade's one platform, so what [`terminal`] holds is there in every build
//! the facade compiles in. The foundation names no platform: everything here belongs to
//! the facade.

pub mod terminal;
