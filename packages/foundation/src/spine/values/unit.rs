/// The unit a platform's toolkit counts text offsets in, where they cross the seam.
///
/// Bytes of UTF-8 are the foundation's own index, so they are no unit a toolkit is told
/// about: a realization counting in them works in bytes directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Unit {
  /// Unicode scalar values.
  CodePoint,
  /// UTF-16 code units, a surrogate pair two.
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
}

#[cfg(test)]
mod tests {
  use super::Unit;

  #[test]
  fn a_character_outside_the_basic_multilingual_plane_is_two_utf16_units() {
    assert_eq!(Unit::CodePoint.len_of('\u{1F600}'), 1);
    assert_eq!(Unit::Utf16.len_of('\u{1F600}'), 2);
    assert_eq!(Unit::Utf16.len_of('\u{D55C}'), 1);
  }
}
