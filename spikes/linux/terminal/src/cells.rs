//! The terminal's text engine: Parley over a typeface of cells, so the terminal lays its text
//! out, breaks it and moves through it by the same engine and rules as every pixel platform,
//! in units of cells.

use skene_foundation::{
  text::{AttributedText, ParagraphStyle, RunStyle, TextEngine, TextRun},
  value::FontFamily,
};
use skene_parley::{ParleyEngine, ParleyParagraph, TabSize, Typeface};

/// The family the cell typeface is known by.
pub const FAMILY: &str = "Skene Cells";

/// The typeface the terminal measures its text in, generated at build time: every code point
/// a glyph as many cells wide as the terminal draws it on its own (zero, one or two), a cell
/// an em, its ascent the whole cell.
pub const CELLS: Typeface = Typeface {
  family: FAMILY,
  data: include_bytes!(concat!(env!("OUT_DIR"), "/cells.otf")),
};

/// The terminal's text engine: one unit is one cell, one line one row.
///
/// Every run is set in the cell typeface at a size of one cell, whatever its style names:
/// the terminal draws glyphs in its own font, and only their cells are laid out here.
#[derive(Clone)]
pub struct Cells {
  engine: ParleyEngine,
}

impl Cells {
  /// The engine, over the cell typeface alone, a tab one blank cell: its stops a cell apart.
  #[must_use]
  pub fn new() -> Self {
    let engine = ParleyEngine::with_typefaces(&[CELLS]);

    engine.set_tab_size(TabSize::Spaces(1.));

    Self { engine }
  }

  /// `style` as the cell typeface sets it: one cell a em, whole cells of spacing.
  fn cell_style(style: &RunStyle) -> RunStyle {
    RunStyle {
      family: FontFamily::Named(FAMILY.into()),
      size: 1.,
      letter_spacing: style.letter_spacing.round(),
      ..style.clone()
    }
  }
}

impl Default for Cells {
  fn default() -> Self {
    Self::new()
  }
}

impl TextEngine for Cells {
  type Paragraph = ParleyParagraph;

  fn paragraph(&self, text: &AttributedText, style: &ParagraphStyle) -> ParleyParagraph {
    let text = AttributedText::new(
      text.shared_value(),
      Self::cell_style(text.base()),
      text
        .runs()
        .iter()
        .map(|run| TextRun {
          range: run.range.clone(),
          style: Self::cell_style(&run.style),
        })
        .collect(),
    );

    self.engine.paragraph(&text, style)
  }
}

#[cfg(test)]
mod tests {
  use skene_foundation::text::{
    AttributedText, Intrinsic, Laid as _, Paragraph as _, ParagraphStyle, RunStyle, TextEngine as _,
  };

  use super::Cells;

  fn widths(text: &str) -> Intrinsic {
    Cells::new()
      .paragraph(
        &AttributedText::plain(text, RunStyle::new(13.)),
        &ParagraphStyle::default(),
      )
      .intrinsic()
  }

  #[test]
  fn text_is_measured_in_cells() {
    assert_eq!(widths("abc").max, 3.);
    assert_eq!(widths("日本").max, 4., "a wide cluster takes two cells");
    assert_eq!(widths("e\u{301}").max, 1., "a combining mark takes none");
    assert_eq!(widths("a\tb").max, 3., "a tab is one cell");
    assert_eq!(widths("alpha beta").min, 5.);
  }

  #[test]
  fn a_line_is_one_row() {
    let laid = Cells::new()
      .paragraph(
        &AttributedText::plain("ab\ncd", RunStyle::new(13.)),
        &ParagraphStyle::default(),
      )
      .layout(None);

    assert_eq!(laid.size().height, 2.);
    assert_eq!(laid.lines()[1].top, 1.);
  }
}
