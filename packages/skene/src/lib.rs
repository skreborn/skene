//! Re-exports the shared foundation every core and shell builds on: the reactive
//! engine ([`reactive`]) and the framework core ([`core`]).

pub use skene_core::{self as core, *};
pub use skene_reactive::{self as reactive, *};

// TODO: Add global property editing (styles, scripts, title, meta, etc.)
// TODO: Add file generation to backends (manifests, configs, etc.)

pub mod prelude;
