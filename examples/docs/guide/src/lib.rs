//! The guide's examples, compiled and run as doctests on the web's server leaf.

#[doc = include_str!("../../../../guide/src/introduction.md")]
pub mod introduction {}

#[doc = include_str!("../../../../guide/src/getting-started.md")]
pub mod getting_started {}

#[doc = include_str!("../../../../guide/src/views.md")]
pub mod views {}

#[doc = include_str!("../../../../guide/src/state.md")]
pub mod state {}

#[doc = include_str!("../../../../guide/src/styling.md")]
pub mod styling {}

#[doc = include_str!("../../../../guide/src/controls.md")]
pub mod controls {}

#[doc = include_str!("../../../../guide/src/web.md")]
pub mod web {}

#[doc = include_str!("../../../../guide/src/terminal.md")]
pub mod terminal {}

#[doc = include_str!("../../../../guide/src/testing.md")]
pub mod testing {}

#[doc = include_str!("../../../../guide/src/libraries.md")]
pub mod libraries {}
