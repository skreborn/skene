//! skene-parley's integration tests: the engine against the trait's contract and the
//! guaranteed core, the editing core over real shaping (bidirectional text, clusters), and
//! the changes Skene carries in Parley, and drawing through vello.
//!
//! Every test shapes in the pinned Inter face kept under `fonts/`, subset to Latin-1 and the
//! combining acute: a Thai, Hebrew or Arabic case tests the engine's analysis (breaks,
//! direction, clusters) over `.notdef` glyphs only, never how a font draws the script.

#![cfg(not(target_arch = "wasm32"))]

mod bidi;
mod clusters;
mod engine;
mod patched;
mod render;

use skene_foundation::{
  Edit,
  text::{AttributedText, ParagraphStyle, TextEngine as _, TextStyle, editing::Editor},
};
use skene_parley::{ParleyEngine, ParleyLaid, ParleyParagraph, Typeface};

/// The pinned face.
const INTER: Typeface = Typeface::new(
  "Inter",
  include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fonts/inter-regular.otf"
  )),
);

/// The size the tests set text at.
const SIZE: f32 = 16.;

/// An engine over the pinned face alone.
fn inter() -> ParleyEngine {
  ParleyEngine::with_typefaces(&[INTER])
}

/// `text` as a paragraph in `style`, at the tests' size.
fn paragraph(text: &str, style: ParagraphStyle) -> ParleyParagraph {
  inter().paragraph(&AttributedText::plain(text, TextStyle::new(SIZE)), &style)
}

/// `text` laid out within `width`, at the tests' size.
fn laid(text: &str, width: Option<f32>) -> ParleyLaid {
  use skene_foundation::text::Paragraph as _;

  paragraph(text, ParagraphStyle::default()).layout(width)
}

/// An editor holding `text` in `style`, at the tests' size.
fn editor(text: &str, style: ParagraphStyle) -> Editor<ParleyEngine> {
  let mut editor = Editor::new(inter(), TextStyle::new(SIZE), style);

  editor.write(&Edit::new(text));

  editor
}
