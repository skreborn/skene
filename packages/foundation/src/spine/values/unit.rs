/// The unit a platform's toolkit counts text offsets in, where they cross the seam.
///
/// Bytes of UTF-8 are the foundation's own index, so they are no unit a toolkit is told
/// about: a realization counting in them works in bytes directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Unit {
  /// Unicode scalar values (GTK, Pango, `IBus`, Fcitx).
  CodePoint,
  /// UTF-16 code units (the web's DOM, `UIKit` and `AppKit`'s `NSString`, Android's `String`,
  /// `WinRT`'s `HSTRING`).
  Utf16,
}

impl Unit {
  /// How many of this unit `character` takes.
  #[inline(always)]
  #[must_use]
  pub const fn len_of(self, character: char) -> usize {
    match self {
      Self::CodePoint => 1,
      Self::Utf16 => character.len_utf16(),
    }
  }

  /// The length of `text` in this unit.
  #[must_use]
  pub fn count(self, text: &str) -> usize {
    match self {
      Self::CodePoint => text.chars().count(),
      Self::Utf16 => text.chars().map(char::len_utf16).sum(),
    }
  }

  /// The byte of `offset` in `text`, clamped to the text; an offset inside a character (half
  /// a surrogate pair) goes to the character's start.
  #[must_use]
  pub fn byte(self, text: &str, offset: usize) -> usize {
    let mut units = 0;

    for (byte, character) in text.char_indices() {
      units += self.len_of(character);

      if units > offset {
        return byte;
      }
    }

    text.len()
  }

  /// The offset in this unit of byte `byte` of `text`, clamped to the text; a byte inside a
  /// character goes to the character's start.
  #[must_use]
  pub fn offset(self, text: &str, byte: usize) -> usize {
    let mut byte = byte.min(text.len());

    while !text.is_char_boundary(byte) {
      byte -= 1;
    }

    self.count(&text[..byte])
  }
}

#[cfg(test)]
mod tests {
  use super::Unit;

  /// An ASCII letter, an e with a combining acute, a face outside the Basic Multilingual Plane
  /// (a surrogate pair in UTF-16) and Hangul.
  const TEXT: &str = "ae\u{301}\u{1F600}\u{D55C}";

  #[test]
  fn every_character_boundary_round_trips_in_either_unit() {
    for unit in [Unit::CodePoint, Unit::Utf16] {
      for (byte, _) in TEXT.char_indices().chain([(TEXT.len(), ' ')]) {
        assert_eq!(
          unit.byte(TEXT, unit.offset(TEXT, byte)),
          byte,
          "{unit:?} at {byte}"
        );
      }
    }

    assert_eq!(Unit::CodePoint.count(TEXT), 5);
    assert_eq!(Unit::Utf16.count(TEXT), 6);
    assert_eq!(Unit::Utf16.offset(TEXT, 8), 5, "past the surrogate pair");
  }

  #[test]
  fn half_a_surrogate_pair_goes_to_its_characters_start() {
    // The face starts at byte 4, UTF-16 offset 3; offset 4 is its second half.
    assert_eq!(Unit::Utf16.byte(TEXT, 3), 4);
    assert_eq!(Unit::Utf16.byte(TEXT, 4), 4);
    assert_eq!(Unit::Utf16.byte(TEXT, 5), 8);
    assert_eq!(Unit::Utf16.byte(TEXT, 99), TEXT.len());
    assert_eq!(Unit::Utf16.offset(TEXT, 5), 3, "a byte inside the face");
  }
}
