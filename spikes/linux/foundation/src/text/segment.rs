//! Logical boundaries by the Unicode segmentation rules (UAX #29), for an engine whose own
//! reading of the text has none to offer.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation as _;

use crate::text::{Direction, TextBoundary, TextIndex};

/// A text's word segments and hard lines, as UAX #29 divides it.
#[derive(Clone, Copy, Debug)]
pub struct Segments<'a> {
  text: &'a str,
}

impl<'a> Segments<'a> {
  /// The segments of `text`.
  #[inline(always)]
  #[must_use]
  pub const fn new(text: &'a str) -> Self {
    Self { text }
  }

  /// Each word segment's byte range, and whether it is blanks alone.
  fn words(self) -> impl Iterator<Item = (Range<usize>, bool)> + 'a {
    self
      .text
      .split_word_bound_indices()
      .map(|(start, segment)| {
        (
          start..start + segment.len(),
          segment.chars().all(char::is_whitespace),
        )
      })
  }

  /// The start of the word before `byte`: blanks skipped, then one segment.
  #[must_use]
  pub fn word_before(self, byte: usize) -> usize {
    self
      .words()
      .filter(|(range, blank)| range.start < byte && !blank)
      .last()
      .map_or(0, |(range, _)| range.start)
  }

  /// The end of the word after `byte`: blanks skipped, then one segment.
  #[must_use]
  pub fn word_after(self, byte: usize) -> usize {
    self
      .words()
      .find(|(range, blank)| range.end > byte && !blank)
      .map_or(self.text.len(), |(range, _)| range.end)
  }

  /// The segment holding `byte` (a word, a run of blanks, or a punctuation mark), the last
  /// one at the text's end, empty in an empty text.
  #[must_use]
  pub fn word_around(self, byte: usize) -> Range<usize> {
    let byte = byte.min(self.text.len().saturating_sub(1));

    self
      .words()
      .find(|(range, _)| range.contains(&byte))
      .map_or(0..0, |(range, _)| range)
  }

  /// The hard line holding `byte`: from after the newline before it to the next newline.
  #[must_use]
  pub fn paragraph_around(self, byte: usize) -> Range<usize> {
    let byte = byte.min(self.text.len());
    let start = self.text[..byte].rfind('\n').map_or(0, |at| at + 1);
    let end = self.text[byte..]
      .find('\n')
      .map_or(self.text.len(), |at| byte + at);

    start..end
  }

  /// The boundary of `unit` next to `byte` in `direction`, over `index`, the text's index.
  #[must_use]
  pub fn boundary(
    self,
    index: &TextIndex,
    byte: usize,
    unit: TextBoundary,
    direction: Direction,
  ) -> usize {
    match (unit, direction) {
      (TextBoundary::Grapheme, Direction::Forward) => index.next_grapheme(byte),
      (TextBoundary::Grapheme, Direction::Backward) => index.previous_grapheme(byte),
      (TextBoundary::Word, Direction::Forward) => self.word_after(byte),
      (TextBoundary::Word, Direction::Backward) => self.word_before(byte),
      (TextBoundary::Paragraph, Direction::Forward) => {
        let end = self.paragraph_around(byte).end;

        if end == byte && end < self.text.len() {
          self.paragraph_around(byte + 1).end
        } else {
          end
        }
      }
      (TextBoundary::Paragraph, Direction::Backward) => {
        let start = self.paragraph_around(byte).start;

        if start == byte && start > 0 {
          self.paragraph_around(byte - 1).start
        } else {
          start
        }
      }
      (TextBoundary::Text, Direction::Forward) => self.text.len(),
      (TextBoundary::Text, Direction::Backward) => 0,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::Segments;

  #[test]
  fn word_removal_follows_unicode_word_boundaries() {
    let mut text = String::from("say don't foo.bar_baz(");
    let mut seen = Vec::new();

    while !text.is_empty() {
      let start = Segments::new(&text).word_before(text.len());

      text.truncate(start);
      seen.push(text.clone());
    }

    assert_eq!(seen, ["say don't foo.bar_baz", "say don't ", "say ", ""]);
  }

  #[test]
  fn words_and_paragraphs_around_a_byte() {
    let segments = Segments::new("alpha beta\ngamma");

    assert_eq!(segments.word_around(7), 6..10);
    assert_eq!(segments.word_after(0), 5);
    assert_eq!(segments.word_after(5), 10);
    assert_eq!(segments.paragraph_around(13), 11..16);
    assert_eq!(segments.paragraph_around(3), 0..10);
  }
}
