use std::{ops::Range, rc::Rc};

use crate::spine::{TextSelection, Unit};

/// A text entry's text with its selection, as one value: a selection is never applied to a
/// text it does not index.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use]
pub struct Edit {
  /// The text.
  pub text: Rc<str>,
  /// The selection, in grapheme clusters of the text, where it is given; `None` leaves it
  /// where it was, mapped through the change.
  pub selection: Option<TextSelection>,
}

impl Edit {
  /// `text`, leaving the selection where it was.
  pub fn new(text: impl Into<Rc<str>>) -> Self {
    Self {
      text: text.into(),
      selection: None,
    }
  }

  /// This edit, the selection placed at `selection`.
  #[inline(always)]
  pub const fn with_selection(mut self, selection: TextSelection) -> Self {
    self.selection = Some(selection);
    self
  }

  /// Whether this edit, as the truth, already holds `proposed`: the same text, and the
  /// same selection where this edit gives one.
  #[must_use]
  pub fn holds(&self, proposed: &Self) -> bool {
    self.text == proposed.text
      && self
        .selection
        .is_none_or(|selection| proposed.selection == Some(selection))
  }

  /// The smallest edit turning `old` into this edit's text, in `unit`: the range of `old`
  /// replaced and the text replacing it.
  #[must_use]
  pub fn splice(&self, old: &str, unit: Unit) -> (Range<usize>, &str) {
    let (replaced, inserted) = self.splice_bytes(old);
    let units =
      |text: &str| -> usize { text.chars().map(|character| unit.len_of(character)).sum() };
    let start = units(&old[..replaced.start]);

    (start..start + units(&old[replaced]), inserted)
  }

  /// The smallest edit turning `old` into this edit's text, in bytes on character
  /// boundaries: the range of `old` replaced and the text replacing it.
  #[must_use]
  pub fn splice_bytes(&self, old: &str) -> (Range<usize>, &str) {
    let new = &*self.text;
    let (before, after) = (old.as_bytes(), new.as_bytes());
    let mut prefix = before
      .iter()
      .zip(after)
      .take_while(|(left, right)| left == right)
      .count();

    while !old.is_char_boundary(prefix) {
      prefix -= 1;
    }

    let room = before.len().min(after.len()) - prefix;
    let mut suffix = before
      .iter()
      .rev()
      .zip(after.iter().rev())
      .take(room)
      .take_while(|(left, right)| left == right)
      .count();

    while !old.is_char_boundary(before.len() - suffix)
      || !new.is_char_boundary(after.len() - suffix)
    {
      suffix -= 1;
    }

    (
      prefix..before.len() - suffix,
      &new[prefix..after.len() - suffix],
    )
  }

  /// `offset` mapped through a splice of `replaced` by `inserted` units: before the range it
  /// stays, after it it moves with it, and strictly inside it it stays as far into the
  /// replacement as it was into the range, clamped to the replacement's end.
  #[must_use]
  pub const fn map_through(offset: usize, replaced: Range<usize>, inserted: usize) -> usize {
    if offset <= replaced.start {
      offset
    }
    else if offset >= replaced.end {
      offset - (replaced.end - replaced.start) + inserted
    }
    else {
      let into = offset - replaced.start;

      replaced.start + if into < inserted { into } else { inserted }
    }
  }
}

/// The answer to an edit a reader proposes to a controlled text entry.
///
/// A proposal the truth already holds is answered [`Verdict::Accept`]: it is no change.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use]
pub enum Verdict {
  /// The proposal stands: the toolkit applies it natively, or keeps it where it applied it
  /// already, so its caret, its undo grouping and its input method stay intact.
  Accept,
  /// The truth is this edit instead: a toolkit that asks before applying applies it in the
  /// proposal's place, and one that applied first is repaired to it as the smallest edit,
  /// the selection mapped through it where the edit leaves it ([`Edit::map_through`]: an
  /// offset inside the replaced range keeps its distance into the replacement, clamped to
  /// its end).
  Replace(Edit),
}

#[cfg(test)]
mod tests {
  use super::Edit;
  use crate::spine::{TextSelection, Unit};

  #[test]
  fn the_smallest_edit_keeps_the_common_ends_on_character_boundaries() {
    let edit = Edit::new("abXYef");

    assert_eq!(edit.splice_bytes("abcdef"), (2..4, "XY"));
    assert_eq!(Edit::new("abc").splice_bytes("abc"), (3..3, ""));
    assert_eq!(Edit::new("").splice_bytes("abc"), (0..3, ""));

    // Both texts share the first byte of `é` and `è`: the edit starts at the character.
    assert_eq!(
      Edit::new("a\u{e8}b").splice_bytes("a\u{e9}b"),
      (1..3, "\u{e8}")
    );

    // A UTF-16 count of the prefix and the range.
    let emoji = Edit::new("\u{1F600}x");

    assert_eq!(emoji.splice("\u{1F600}yz", Unit::Utf16), (2..4, "x"));
    assert_eq!(emoji.splice("\u{1F600}yz", Unit::CodePoint), (1..3, "x"));
  }

  #[test]
  fn an_offset_maps_through_a_splice() {
    assert_eq!(Edit::map_through(1, 2..4, 1), 1);
    assert_eq!(Edit::map_through(2, 2..4, 1), 2);
    assert_eq!(Edit::map_through(3, 2..4, 1), 3);
    assert_eq!(Edit::map_through(4, 2..4, 1), 3);
    assert_eq!(Edit::map_through(6, 2..4, 5), 9);
  }

  #[test]
  fn an_offset_inside_the_replaced_range_keeps_its_distance_into_the_replacement() {
    // An equal-length replacement keeps the offset.
    assert_eq!(Edit::map_through(2, 0..3, 3), 2);
    assert_eq!(Edit::map_through(1, 0..3, 3), 1);

    // A shrinking one clamps it to the replacement's end.
    assert_eq!(Edit::map_through(3, 1..4, 1), 2);
    assert_eq!(Edit::map_through(2, 1..4, 1), 2);
    assert_eq!(Edit::map_through(2, 1..4, 0), 1);

    // A growing one keeps the offset.
    assert_eq!(Edit::map_through(2, 1..3, 5), 2);
  }

  #[test]
  fn an_edit_carries_its_selection_with_its_text() {
    let edit = Edit::new("ab").with_selection(TextSelection::collapsed(1));

    assert_eq!(edit.selection, Some(TextSelection::collapsed(1)));
    assert_eq!(&*edit.text, "ab");
  }

  #[test]
  fn a_truth_holds_its_text_and_the_selection_it_gives() {
    let truth = Edit::new("ab").with_selection(TextSelection::collapsed(1));

    assert!(truth.holds(&Edit::new("ab").with_selection(TextSelection::collapsed(1))));
    assert!(
      !truth.holds(&Edit::new("ab")),
      "a proposal leaving the selection where it was may not land on the truth's"
    );
    assert!(!truth.holds(&Edit::new("ab").with_selection(TextSelection::collapsed(2))));
    assert!(!truth.holds(&Edit::new("abc")));
    assert!(Edit::new("ab").holds(&Edit::new("ab").with_selection(TextSelection::collapsed(2))));
  }
}
