//! # text-parity
//!
//! One corpus of texts, laid out and drawn by Parley and vello on one side and by a platform's
//! own engine on the other, and a comparison of the two.
//!
//! - [`corpus`]: the cases (`corpus.json`) and the variants (a size and a width) each expands to,
//!   the same list on every platform.
//! - [`record`]: the record an engine writes per variant (lines, runs, clusters, metrics, timings,
//!   images), the schema a platform spike fills from its own engine.
//! - [`fonts`]: the font collection Parley shapes with, fontique's system backend or an explicit
//!   [`FontSource`](fonts::FontSource) (Android's `ASystemFontIterator` fonts).
//! - [`layout`]: Parley's layout of a variant, its record, and its glyph runs for a platform
//!   rasterizer (option C).
//! - [`render`]: a layout drawn with vello_cpu into a PNG at a scale.
//! - [`compare`]: two record sets and their images compared, as markdown and JSON.
//! - [`bench`](mod@bench): the timings: font collection start-up, cold and warm layout, 1000
//!   labels.
//! - [`run`]: one Parley run over the corpus, as the CLI and an embedding spike start it.

pub mod bench;
pub mod compare;
pub mod corpus;
pub mod emoji;
pub mod fonts;
pub mod image;
pub mod layout;
pub mod record;
pub mod render;
pub mod run;
pub mod text;
