//! Text offsets in both units a record carries: UTF-8 bytes and UTF-16 code units.

/// The two units' offsets of one text, at every character boundary.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Offsets {
  utf16_of_utf8: Vec<usize>,
  utf8_of_utf16: Vec<usize>,
}

impl Offsets {
  /// The offsets of `text`.
  #[must_use]
  pub fn new(text: &str) -> Self {
    let mut utf16_of_utf8 = vec![0; text.len() + 1];
    let mut utf8_of_utf16 = Vec::with_capacity(text.len() + 1);
    let mut utf16 = 0;

    for (at, character) in text.char_indices() {
      utf16_of_utf8[at..at + character.len_utf8()].fill(utf16);

      for _ in 0..character.len_utf16() {
        utf8_of_utf16.push(at);
      }

      utf16 += character.len_utf16();
    }

    utf16_of_utf8[text.len()] = utf16;
    utf8_of_utf16.push(text.len());

    Self {
      utf16_of_utf8,
      utf8_of_utf16,
    }
  }

  /// The UTF-16 offset of byte `utf8`, the character's start where it falls inside one.
  #[must_use]
  pub fn utf16(&self, utf8: usize) -> usize {
    self.utf16_of_utf8[utf8.min(self.utf16_of_utf8.len() - 1)]
  }

  /// The byte offset of UTF-16 unit `utf16`, the character's start where it falls inside one.
  #[must_use]
  pub fn utf8(&self, utf16: usize) -> usize {
    self.utf8_of_utf16[utf16.min(self.utf8_of_utf16.len() - 1)]
  }

  /// A UTF-8 range in UTF-16.
  #[must_use]
  pub fn range_utf16(&self, utf8: [usize; 2]) -> [usize; 2] {
    [self.utf16(utf8[0]), self.utf16(utf8[1])]
  }

  /// A UTF-16 range in UTF-8.
  #[must_use]
  pub fn range_utf8(&self, utf16: [usize; 2]) -> [usize; 2] {
    [self.utf8(utf16[0]), self.utf8(utf16[1])]
  }
}

/// Whether `character` draws nothing a reader sees: a blank, a line break, a format character
/// (ZWJ, a soft hyphen, a bidi mark), a variation selector or an emoji tag.
#[must_use]
pub fn is_invisible(character: char) -> bool {
  character.is_whitespace()
    || matches!(
      character,
      '\u{AD}'
        | '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{FE00}'..='\u{FE0F}'
        | '\u{FEFF}'
        | '\u{E0000}'..='\u{E007F}'
        | '\u{E0100}'..='\u{E01EF}'
    )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn both_units() {
    let text = "a😀é\n";
    let offsets = Offsets::new(text);

    assert_eq!(offsets.utf16(0), 0);
    assert_eq!(offsets.utf16(1), 1);
    assert_eq!(offsets.utf16(5), 3);
    assert_eq!(offsets.utf16(7), 4);
    assert_eq!(offsets.utf16(8), 5);
    assert_eq!(
      offsets.utf8(2),
      1,
      "inside a surrogate pair: the character's start"
    );
    assert_eq!(offsets.utf8(3), 5);
    assert_eq!(offsets.range_utf8([1, 4]), [1, 7]);
  }

  #[test]
  fn empty() {
    let offsets = Offsets::new("");

    assert_eq!(offsets.utf16(0), 0);
    assert_eq!(offsets.utf8(0), 0);
  }
}
