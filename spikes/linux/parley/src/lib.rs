//! # skene-parley
//!
//! Parley as Skene's [`TextEngine`](skene_foundation::text::TextEngine): a paragraph is
//! shaped once, broken per width into an immutable layout, and measured, hit-tested and
//! moved through with Parley's own editing primitives (bidirectional carets and visual
//! movement included). Complex-script line breaking (Thai, Khmer, Lao, Burmese) runs on ICU4X's
//! dictionaries, and a line breaking at a soft hyphen draws a hyphen.
//!
//! On Linux the engine stands on the system's layers rather than its own:
//!
//! - **fallback**: fontconfig resolves each character's family, as Pango does, from the
//!   user's configuration; an emoji-presentation cluster goes to the emoji family first
//!   ([`linux::Fallback`]);
//! - **rasterization**: FreeType draws each glyph under the antialiasing and hinting
//!   fontconfig gives its font (the desktop's settings beneath), into a mask cache Skene's
//!   renderer composites ([`render::MaskPainter`]); vello's outlines are the floor
//!   ([`render::paint_outlines`]).
//!
//! The terminal's engine is the same over a typeface of cells and no system font
//! ([`ParleyEngine::with_typefaces`]).

mod engine;
#[cfg(target_os = "linux")]
pub mod linux;
pub mod render;

pub use self::engine::{
  GlyphRun, Mark, PaintRuns, ParleyEngine, ParleyLaid, ParleyParagraph, PositionedGlyph, Typeface,
};
