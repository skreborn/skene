//! skene-parley's integration tests: the engine against the trait's contract, the editing
//! core over real shaping (bidirectional text, clusters), and Linux's fallback.

mod bidi;
mod clusters;
mod engine;
#[cfg(target_os = "linux")]
mod fallback;

use skene_foundation::text::{
  AttributedText, ParagraphStyle, RunStyle, TextEngine as _, editing::Editor,
};
use skene_parley::{ParleyEngine, ParleyLaid, ParleyParagraph};

/// The size the tests set text at.
const SIZE: f32 = 16.;

fn paragraph(text: &str, style: ParagraphStyle) -> ParleyParagraph {
  ParleyEngine::system().paragraph(&AttributedText::plain(text, RunStyle::new(SIZE)), &style)
}

fn laid(text: &str, width: Option<f32>) -> ParleyLaid {
  use skene_foundation::text::Paragraph as _;

  paragraph(text, ParagraphStyle::default()).layout(width)
}

fn editor(text: &str, style: ParagraphStyle) -> Editor<ParleyEngine> {
  let mut editor = Editor::new(ParleyEngine::system(), RunStyle::new(SIZE), style);

  editor.write(&skene_foundation::seam::content::Edit::text(text));

  editor
}
