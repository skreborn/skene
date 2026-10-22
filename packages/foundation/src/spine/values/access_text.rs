use super::Rect;

/// What a drawn text entry reports of its text to assistive technology, as AccessKit
/// models an editable text: a run per laid line, and the selection between two places in
/// them.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct AccessText {
  /// The laid lines in order; their texts joined are the text the entry shows.
  pub runs: Vec<AccessRun>,
  /// Where the selection began.
  pub anchor: AccessTextPosition,
  /// Where the selection extends to, and the caret.
  pub focus: AccessTextPosition,
}

/// One laid line of an [`AccessText`]: its text and the geometry of each of its
/// characters, a character being a grapheme cluster (or a piece of one, as
/// [`character_lengths`](Self::character_lengths) says).
///
/// Every length is in the units of the layout it is read from.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct AccessRun {
  /// The line's text, its line break included; for a secure entry, a U+2022 mark for each
  /// grapheme cluster, the line break's among them.
  pub text: String,
  /// The bytes of `text` each character takes, adding up to its length; a cluster longer
  /// than 255 bytes is as many characters as its code points fill in turn, the first
  /// drawing it and the rest of no width at its trailing edge.
  pub character_lengths: Vec<u8>,
  /// Where each character starts, from the run's leading edge: its left where it runs left
  /// to right, its right where it runs right to left.
  pub character_positions: Vec<f32>,
  /// How wide each character draws.
  pub character_widths: Vec<f32>,
  /// The characters a word starts at, in order, among the first 256.
  pub word_starts: Vec<u8>,
  /// The line's frame, in the coordinates of the layout it is read from.
  pub bounds: Rect<f32>,
  /// Whether the line runs right to left.
  pub right_to_left: bool,
}

/// A place in an [`AccessText`]: before a character of one of its runs, or at the end of
/// the run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct AccessTextPosition {
  /// The run, by its line.
  pub line: u16,
  /// The character it stands before, or the run's character count at its end.
  pub character: usize,
}

impl AccessTextPosition {
  /// The place before character `character` of the run of line `line`.
  #[inline(always)]
  pub const fn new(line: u16, character: usize) -> Self {
    Self { line, character }
  }
}
