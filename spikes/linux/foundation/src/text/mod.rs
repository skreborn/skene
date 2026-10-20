//! Text: what it says and how it is styled ([`AttributedText`], [`ParagraphStyle`]), the
//! contract a realization's engine implements to shape, break and measure it ([`TextEngine`]),
//! the layout engine's text leaf ([`TextLeaf`]), the offset index every protocol meets on
//! ([`TextIndex`]), and the editing core every drawn field shares ([`editing`]).
//!
//! The foundation owns the content and the style; the engine owns shaping, breaking,
//! rasterizing and measuring; everything acting on text (selection, editing, accessibility,
//! the decorations an engine lacks) is written once here over the geometry the engine reports.

mod decoration;
pub mod editing;
mod engine;
mod index;
mod leaf;
mod mask;
mod model;
mod segment;

#[cfg(test)]
mod grid;

pub use self::{
  decoration::DecorationLine,
  engine::{
    Affinity, Caret, Carets, Direction, Hit, Intrinsic, Laid, LineInfo, Made, Paragraph, Position,
    RunExtent, TextBoundary, TextEngine, TextSize, VisualMove,
  },
  index::{Round, TextIndex},
  leaf::{LeafMeasure, MeasureText, Space, TextLeaf},
  mask::{MaskState, Masked, MaskedLaid, MaskedParagraph},
  model::{
    AttributedText, BaseDirection, LineHeight, PaintStyle, ParagraphStyle, RunStyle, TextRun, Wrap,
  },
  segment::Segments,
};
