//! One index over a text's offsets: UTF-8 bytes, UTF-16 code units, code points and
//! grapheme clusters, so a protocol in any unit and Skene's grapheme-indexed public selection
//! meet on the same positions.

use unicode_segmentation::UnicodeSegmentation as _;

use crate::seam::content::Unit;

/// Which way an offset inside a character or a cluster goes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Round {
  /// To the start of what it falls inside.
  Down,
  /// To the end of what it falls inside.
  Up,
}

/// A text's offset tables: the byte start of each character and of each grapheme cluster,
/// and the UTF-16 offset of each character. An ASCII text keeps no tables.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[must_use]
pub struct TextIndex {
  len: usize,
  ascii: bool,
  chars: Vec<u32>,
  utf16: Vec<u32>,
  graphemes: Vec<u32>,
}

impl TextIndex {
  /// The index of `text`.
  pub fn new(text: &str) -> Self {
    // ASCII's one cluster of two characters is a carriage return and a line feed.
    if text.is_ascii() && !text.contains("\r\n") {
      return Self {
        len: text.len(),
        ascii: true,
        chars: Vec::new(),
        utf16: Vec::new(),
        graphemes: Vec::new(),
      };
    }

    let graphemes = text
      .grapheme_indices(true)
      .map(|(start, _)| Self::narrow(start))
      .collect::<Vec<_>>();

    if text.is_ascii() && graphemes.len() == text.len() {
      return Self {
        len: text.len(),
        ascii: true,
        chars: Vec::new(),
        utf16: Vec::new(),
        graphemes: Vec::new(),
      };
    }

    let mut chars = Vec::with_capacity(text.len());
    let mut utf16 = Vec::with_capacity(text.len());
    let mut units = 0;

    for (start, character) in text.char_indices() {
      chars.push(Self::narrow(start));
      utf16.push(units);
      units += Self::narrow(character.len_utf16());
    }

    utf16.push(units);

    Self {
      len: text.len(),
      ascii: false,
      chars,
      utf16,
      graphemes,
    }
  }

  #[expect(
    clippy::cast_possible_truncation,
    reason = "a text an index covers is under four gigabytes"
  )]
  const fn narrow(value: usize) -> u32 {
    value as u32
  }

  /// The text's length in bytes.
  #[inline(always)]
  #[must_use]
  pub const fn len(&self) -> usize {
    self.len
  }

  /// Whether the text is empty.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.len == 0
  }

  /// The number of grapheme clusters.
  #[must_use]
  pub const fn grapheme_count(&self) -> usize {
    if self.ascii {
      self.len
    } else {
      self.graphemes.len()
    }
  }

  /// The text's length in `unit`.
  #[must_use]
  pub fn count(&self, unit: Unit) -> usize {
    if self.ascii {
      return self.len;
    }

    match unit {
      Unit::Utf8 => self.len,
      Unit::CodePoint => self.chars.len(),
      Unit::Utf16 => self.utf16.last().map_or(0, |units| *units as usize),
    }
  }

  /// The byte start of grapheme cluster `index`, the text's end past the last.
  #[must_use]
  pub fn byte_of_grapheme(&self, index: usize) -> usize {
    if self.ascii {
      return index.min(self.len);
    }

    self
      .graphemes
      .get(index)
      .map_or(self.len, |start| *start as usize)
  }

  /// The grapheme cluster holding byte `byte`, or the count at the end; a byte inside a
  /// cluster goes to the cluster's start (`Down`) or its end (`Up`).
  #[must_use]
  pub fn grapheme_at_byte(&self, byte: usize, round: Round) -> usize {
    if self.ascii {
      return byte.min(self.len);
    }

    if byte >= self.len {
      return self.graphemes.len();
    }

    let byte = Self::narrow(byte);

    match self.graphemes.binary_search(&byte) {
      Ok(index) => index,
      Err(index) => match round {
        Round::Down => index.saturating_sub(1),
        Round::Up => index,
      },
    }
  }

  /// Whether `byte` starts a grapheme cluster or ends the text.
  #[must_use]
  pub fn is_grapheme_boundary(&self, byte: usize) -> bool {
    self.ascii || byte == self.len || self.graphemes.binary_search(&Self::narrow(byte)).is_ok()
  }

  /// `byte` moved to a grapheme boundary as `round` says.
  #[must_use]
  pub fn snap_to_grapheme(&self, byte: usize, round: Round) -> usize {
    self.byte_of_grapheme(self.grapheme_at_byte(byte, round))
  }

  /// The grapheme boundary after `byte`, the text's end at the last.
  #[must_use]
  pub fn next_grapheme(&self, byte: usize) -> usize {
    self.byte_of_grapheme(self.grapheme_at_byte(byte, Round::Down) + 1)
  }

  /// The grapheme boundary before `byte`, the text's start at the first.
  #[must_use]
  pub fn previous_grapheme(&self, byte: usize) -> usize {
    self.byte_of_grapheme(self.grapheme_at_byte(byte, Round::Up).saturating_sub(1))
  }

  /// The byte of `offset` in `unit`, clamped to the text; an offset inside a character
  /// (half a surrogate pair, a byte inside a UTF-8 sequence) goes as `round` says.
  #[must_use]
  pub fn byte(&self, unit: Unit, offset: usize, round: Round) -> usize {
    if self.ascii {
      return offset.min(self.len);
    }

    let offset = Self::narrow(offset.min(u32::MAX as usize));
    let found = match unit {
      Unit::Utf8 => {
        if offset as usize >= self.len {
          return self.len;
        }

        self.chars.binary_search(&offset)
      }
      Unit::CodePoint => {
        return self
          .chars
          .get(offset as usize)
          .map_or(self.len, |start| *start as usize);
      }
      Unit::Utf16 => self.utf16.binary_search(&offset),
    };
    let character = match found {
      Ok(index) => index,
      Err(index) => match round {
        Round::Down => index.saturating_sub(1),
        Round::Up => index,
      },
    };

    self
      .chars
      .get(character)
      .map_or(self.len, |start| *start as usize)
  }

  /// The offset in `unit` of byte `byte`, which lies on a character boundary (else it goes
  /// to its character's start).
  #[must_use]
  pub fn offset(&self, unit: Unit, byte: usize) -> usize {
    if self.ascii || unit == Unit::Utf8 {
      return byte.min(self.len);
    }

    let byte = Self::narrow(byte.min(self.len));
    let character = match self.chars.binary_search(&byte) {
      Ok(index) => index,
      Err(index) if byte as usize == self.len => index,
      Err(index) => index.saturating_sub(1),
    };

    match unit {
      Unit::CodePoint => character,
      _ => self.utf16[character] as usize,
    }
  }

  /// A range in `unit` as bytes, widened to whole characters: its start down, its end up.
  #[must_use]
  pub fn byte_range(&self, unit: Unit, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
    let start = self.byte(unit, range.start, Round::Down);
    let end = self.byte(unit, range.end.max(range.start), Round::Up);

    start..end.max(start)
  }

  /// A byte range in `unit`.
  #[must_use]
  pub fn unit_range(&self, unit: Unit, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
    self.offset(unit, range.start)..self.offset(unit, range.end)
  }
}

#[cfg(test)]
mod tests {
  use super::{Round, TextIndex};
  use crate::seam::content::Unit;

  #[test]
  fn an_ascii_text_keeps_no_tables() {
    let index = TextIndex::new("hello");

    assert!(index.chars.is_empty());
    assert_eq!(index.grapheme_count(), 5);
    assert_eq!(index.byte(Unit::Utf16, 3, Round::Down), 3);
    assert_eq!(index.offset(Unit::CodePoint, 9), 5);
  }

  #[test]
  fn offsets_meet_across_units() {
    // e + combining acute (one cluster, two code points), a family emoji (one cluster,
    // five code points, eight UTF-16 units), Hangul.
    let text = "e\u{301}\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{D55C}";
    let index = TextIndex::new(text);

    assert_eq!(index.grapheme_count(), 3);
    assert_eq!(index.byte_of_grapheme(1), 3);
    assert_eq!(index.byte_of_grapheme(2), 3 + 18);
    assert_eq!(index.count(Unit::CodePoint), 8);
    assert_eq!(index.count(Unit::Utf16), 2 + 8 + 1);
    assert_eq!(index.offset(Unit::Utf16, 3), 2);
    assert_eq!(index.offset(Unit::Utf16, 21), 10);
    assert_eq!(
      index.byte(Unit::CodePoint, 2, Round::Down),
      3,
      "a code point's start"
    );
    // UTF-16 offset 3 falls inside the first surrogate pair: widened to the code point.
    assert_eq!(index.byte(Unit::Utf16, 3, Round::Down), 3);
    assert_eq!(index.byte(Unit::Utf16, 3, Round::Up), 7);
    assert_eq!(index.byte_range(Unit::Utf16, 3..3), 3..7);
    // A byte inside the cluster rounds to its ends.
    assert_eq!(index.grapheme_at_byte(7, Round::Down), 1);
    assert_eq!(index.grapheme_at_byte(7, Round::Up), 2);
    assert_eq!(index.next_grapheme(0), 3);
    assert_eq!(index.previous_grapheme(text.len()), 21);
    assert!(!index.is_grapheme_boundary(1));
  }
}
