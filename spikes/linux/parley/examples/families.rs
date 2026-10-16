//! Prints each run's font family, and every cluster boundary's caret, for a few texts.

use skene_foundation::text::{
  AttributedText, Laid as _, Paragraph as _, ParagraphStyle, Position, RunStyle, TextEngine as _,
};
use skene_parley::ParleyEngine;

fn main() {
  let engine = ParleyEngine::system();

  for text in std::env::args().skip(1) {
    let paragraph = engine.paragraph(
      &AttributedText::plain(text.as_str(), RunStyle::new(13.)),
      &ParagraphStyle::default(),
    );
    let laid = paragraph.layout(None);

    let families: Vec<(&str, String)> = laid
      .run_families()
      .into_iter()
      .map(|(range, family)| (&text[range], family))
      .collect();

    println!("{families:?}");

    for (at, _) in text.char_indices() {
      let caret = laid.caret(Position::new(at));

      print!("{at}:{:.1} ", caret.primary.x);
    }

    println!();
  }
}
