use std::ops::Range;

use skene_reactive::{Recorder, Snapshot, Value, key};

/// The selection inside a text entry, in grapheme clusters of its value: `start` is
/// the cluster the selection began from and `end` the one it extends to, where the
/// caret is, the two equal while collapsed and `end` before `start` when the
/// selection was made backward.
///
/// A platform clamps each index into the value; nothing can name the inside of a
/// cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub struct TextSelection {
  /// The cluster the selection began from.
  pub start: usize,
  /// The cluster the selection extends to, and the insertion point.
  pub end: usize,
}

impl TextSelection {
  /// A selection from `start` to `end`.
  #[inline(always)]
  pub const fn new(start: usize, end: usize) -> Self {
    Self { start, end }
  }

  /// A collapsed selection: the caret at `at`, nothing selected.
  #[inline(always)]
  pub const fn collapsed(at: usize) -> Self {
    Self { start: at, end: at }
  }

  /// The lower index.
  #[inline(always)]
  #[must_use]
  pub const fn min(&self) -> usize {
    if self.start < self.end {
      self.start
    }
    else {
      self.end
    }
  }

  /// The higher index.
  #[inline(always)]
  #[must_use]
  pub const fn max(&self) -> usize {
    if self.start < self.end {
      self.end
    }
    else {
      self.start
    }
  }

  /// The selected clusters, in document order; empty while collapsed.
  #[inline(always)]
  #[must_use]
  pub const fn range(&self) -> Range<usize> {
    self.min()..self.max()
  }

  /// Whether nothing is selected.
  #[inline(always)]
  #[must_use]
  pub const fn is_collapsed(&self) -> bool {
    self.start == self.end
  }

  /// Whether the selection was made backward: its end before its start.
  #[inline(always)]
  #[must_use]
  pub const fn is_reversed(&self) -> bool {
    self.end < self.start
  }

  /// This selection with each index clamped to `count`, a value's cluster count.
  #[inline(always)]
  pub const fn clamped(self, count: usize) -> Self {
    Self {
      start: if self.start < count {
        self.start
      }
      else {
        count
      },
      end: if self.end < count { self.end } else { count },
    }
  }
}

impl Snapshot for TextSelection {
  fn record(&self, recorder: &mut Recorder) {
    recorder.record(|fields| {
      fields.field(key("start"), &self.start);
      fields.field(key("end"), &self.end);
    });
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    let Value::Record(record) = value
    else {
      return None;
    };

    Some(Self {
      start: record.field(key("start")).and_then(usize::restore)?,
      end: record.field(key("end")).and_then(usize::restore)?,
    })
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(Self::default())
  }
}
