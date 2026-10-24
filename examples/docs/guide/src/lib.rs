//! The guide's examples, compiled and run as doctests in the build of the facade each
//! chapter's examples use: a web chapter's on the web's server leaf, the terminal's in
//! the terminal's build, and the testing chapter's in both, each example in its own.

#[doc = include_str!("../../../../guide/src/introduction.md")]
pub mod introduction {}

#[doc = include_str!("../../../../guide/src/getting-started.md")]
#[cfg(skene_platform = "web")]
pub mod getting_started {}

#[doc = include_str!("../../../../guide/src/views.md")]
#[cfg(skene_platform = "web")]
pub mod views {}

#[doc = include_str!("../../../../guide/src/state.md")]
#[cfg(skene_platform = "web")]
pub mod state {}

#[doc = include_str!("../../../../guide/src/styling.md")]
#[cfg(skene_platform = "web")]
pub mod styling {}

#[doc = include_str!("../../../../guide/src/controls.md")]
#[cfg(skene_platform = "web")]
pub mod controls {}

#[doc = include_str!("../../../../guide/src/web.md")]
#[cfg(skene_platform = "web")]
pub mod web {}

#[doc = include_str!("../../../../guide/src/terminal.md")]
#[cfg(skene_platform = "terminal")]
pub mod terminal {}

#[doc = include_str!("../../../../guide/src/testing.md")]
pub mod testing {}

#[doc = include_str!("../../../../guide/src/libraries.md")]
#[cfg(skene_platform = "web")]
pub mod libraries {}
