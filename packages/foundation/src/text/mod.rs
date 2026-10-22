//! Text, and the contract a realization's engine lays it out under.
//!
//! What a text says and how it is styled ([`AttributedText`], [`ParagraphStyle`]), the
//! contract a realization's engine implements to shape, break and measure it ([`TextEngine`]),
//! the layout engine's text leaf ([`TextLeaf`]), and the offset index every protocol meets on
//! ([`TextIndex`]).
//!
//! The foundation owns the content and the style; the engine owns shaping, breaking,
//! rasterizing and measuring; everything acting on text (selection, editing, accessibility,
//! the decorations an engine lacks) is written once here over the geometry the engine reports.

mod decoration;
mod engine;
mod index;
mod leaf;
mod mask;
mod model;
mod segment;
mod view;

#[cfg(test)]
mod grid;

pub use self::{
  decoration::DecorationLine,
  engine::{
    Affinity,
    Caret,
    Carets,
    Direction,
    Hit,
    Intrinsic,
    Laid,
    LineInfo,
    Made,
    Paragraph,
    Position,
    RunExtent,
    TextBoundary,
    TextEngine,
    TextSize,
    VisualMove,
  },
  index::{Round, TextIndex},
  leaf::{LeafMeasure, MeasureText, Space, TextLeaf},
  mask::{MaskState, Masked, Substitute, Substituted, SubstitutedLaid, SubstitutedParagraph},
  model::{
    AttributedText,
    BaseDirection,
    FontFamily,
    FontSlant,
    FontWeight,
    InlineBox,
    LineHeight,
    PaintStyle,
    ParagraphStyle,
    TextAlignment,
    TextRun,
    TextStyle,
    TextWrap,
  },
  segment::Segments,
  view::{Text, TextBase},
};
