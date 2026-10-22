//! # skene-parley
//!
//! Parley as skene's [`TextEngine`](skene_foundation::text::TextEngine), over typefaces an
//! engine is given and no system font ([`ParleyEngine::with_typefaces`]): a paragraph is
//! shaped once, broken per width into a layout whose geometry never changes and whose paint
//! is the paragraph's, and measured, hit-tested and moved through with Parley's own editing
//! primitives, bidirectional carets and visual movement included. Complex-script line
//! breaking (Thai, Lao, Khmer, Burmese) runs on ICU4X's dictionaries, and a line breaking at
//! a soft hyphen draws a hyphen.
//!
//! Off the web, a batch of [`PARALLEL`] paragraphs or more shapes on threads beside the
//! engine's own, each over contexts of its own on the engine's fonts. A laid paragraph draws
//! through vello as outlines with the shared decoration painter's lines
//! ([`render::paint_outlines`]).

mod engine;
pub mod render;
mod shaping;

/// How far apart a tab's stops are, which an engine sets for its paragraphs
/// ([`ParleyEngine::set_tab_size`]).
pub use parley::TabSize;

pub use self::{
  engine::{
    GlyphRun,
    ParleyEngine,
    ParleyLaid,
    ParleyParagraph,
    PositionedGlyph,
    SHAPE_CACHE,
    Typeface,
  },
  shaping::PARALLEL,
};
