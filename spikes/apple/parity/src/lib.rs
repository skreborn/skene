//! The Apple half of the text-engine revisit, on macOS and on the iOS simulator:
//!
//! - **A**, TextKit 2 standalone ([`textkit`]), read into the harness's record and drawn by its
//!   fragments into a Core Graphics bitmap;
//! - **B**, Parley and vello through the harness (`text_parity::run::parley`), and B tuned
//!   ([`tuned`]): Core Text's fonts and fallback registered with fontique, `opsz` at the size, Core
//!   Foundation's hyphenation points as soft hyphens;
//! - **C**, B's glyph runs drawn by Core Text's `CTFontDrawGlyphs` ([`raster`]).
//!
//! [`isolate`] draws TextKit's own glyphs by both rasterizers (layout held equal); [`suite`]
//! runs all of it with the probes ([`probes`], [`genmoji`], [`smoothing`], [`subset`], and on
//! iOS `ios`) and writes records, images, comparisons and a results summary to one directory;
//! [`ffi`] is the C entry the iOS test links.

pub mod cg;
pub mod coretext;
pub mod ffi;
pub mod genmoji;
#[cfg(target_os = "ios")]
pub mod ios;
pub mod isolate;
pub mod label;
pub mod platform;
pub mod probes;
pub mod raster;
pub mod sets;
pub mod smoothing;
pub mod subset;
pub mod suite;
pub mod textkit;
pub mod tuned;
