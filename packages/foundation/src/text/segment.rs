//! Logical boundaries by the Unicode segmentation rules (UAX #29), for an engine whose own
//! reading of the text has none to offer.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation as _;

use crate::text::{Direction, TextBoundary, TextIndex};

/// A text's word segments and hard lines, as UAX #29 divides it.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Segments<'a> {
  text: &'a str,
}

impl<'a> Segments<'a> {
  /// The segments of `text`.
  #[inline(always)]
  pub const fn new(text: &'a str) -> Self {
    Self { text }
  }

  /// Each word segment's byte range, and whether it is blanks alone: UAX #29's, with a full
  /// stop between two letters a segment of its own, as Chromium tailors the rules.
  fn words(self) -> impl Iterator<Item = (Range<usize>, bool)> + 'a {
    let text = self.text;
    let mut segments = text.split_word_bound_indices();
    let mut stop = None;
    let mut rest = None;

    let blank = move |range: Range<usize>| {
      let blank = text[range.clone()].chars().all(char::is_whitespace);

      (range, blank)
    };

    std::iter::from_fn(move || {
      if let Some(stop) = stop.take() {
        return Some(blank(stop));
      }

      let (start, segment): (usize, &str) = rest.take().or_else(|| segments.next())?;
      let cut = segment
        .char_indices()
        .find(|&(at, character)| character == '.' && Self::between_letters(segment, at));
      let Some((at, _)) = cut
      else {
        return Some(blank(start..start + segment.len()));
      };

      stop = Some(start + at..start + at + 1);
      rest = Some((start + at + 1, &segment[at + 1..]));

      Some(blank(start..start + at))
    })
  }

  /// Whether byte `at` of `segment` lies between two letters.
  fn between_letters(segment: &str, at: usize) -> bool {
    let before = segment[..at].chars().next_back();
    let after = segment[at..].chars().nth(1);

    before.is_some_and(char::is_alphabetic) && after.is_some_and(char::is_alphabetic)
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
        }
        else {
          end
        }
      }
      (TextBoundary::Paragraph, Direction::Backward) => {
        let start = self.paragraph_around(byte).start;

        if start == byte && start > 0 {
          self.paragraph_around(byte - 1).start
        }
        else {
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
  fn word_removal_follows_chromiums_word_boundaries() {
    let mut text = String::from("say don't foo.bar_baz(");
    let mut seen = Vec::new();

    while !text.is_empty() {
      let start = Segments::new(&text).word_before(text.len());

      text.truncate(start);
      seen.push(text.clone());
    }

    assert_eq!(
      seen,
      [
        "say don't foo.bar_baz",
        "say don't foo.",
        "say don't foo",
        "say don't ",
        "say ",
        ""
      ]
    );
  }

  #[test]
  fn a_full_stop_between_letters_is_a_word_of_its_own() {
    let segments = Segments::new("say foo.bar_baz end 3.14");

    assert_eq!(segments.word_around(9), 8..15, "the word after the stop");
    assert_eq!(segments.word_around(7), 7..8, "the stop");
    assert_eq!(segments.word_around(5), 4..7, "the word before it");
    assert_eq!(segments.word_around(21), 20..24, "a decimal stays one word");

    let mut at = 0;
    let mut stops = Vec::new();

    while at < 19 {
      at = segments.word_after(at);
      stops.push(at);
    }

    assert_eq!(stops, [3, 7, 8, 15, 19]);
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
