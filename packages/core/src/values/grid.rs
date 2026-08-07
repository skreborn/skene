/// One grid track's size, in a template or an auto-track list.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum GridTrack<T> {
  /// A fixed length.
  Length(T),
  /// A percentage of the container's corresponding extent (`0.` to `100.`).
  Percent(f32),
  /// A fraction of the free space.
  Fr(f32),
  /// Sized to the content.
  Auto,
  /// The content's minimum size.
  MinContent,
  /// The content's maximum size.
  MaxContent,
  /// The content's maximum size, clamped to the given length.
  FitContent(T),
  /// Sized between the two limits.
  Minmax(TrackLimit<T>, TrackLimit<T>),
}

impl<T: Copy> GridTrack<T> {
  /// This track repeated `count` times, for building templates.
  #[inline(always)]
  pub fn repeated(self, count: u16) -> impl Iterator<Item = Self> {
    std::iter::repeat_n(self, count as usize)
  }
}

/// One limit of a [`GridTrack::Minmax`] track.
///
/// A fraction is meaningful only as the maximum; as a minimum it behaves as `Auto`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum TrackLimit<T> {
  /// A fixed length.
  Length(T),
  /// A percentage of the container's corresponding extent (`0.` to `100.`).
  Percent(f32),
  /// A fraction of the free space.
  Fr(f32),
  /// Sized to the content.
  Auto,
  /// The content's minimum size.
  MinContent,
  /// The content's maximum size.
  MaxContent,
}

/// The direction auto-placed grid items fill, and whether gaps are backfilled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum GridAutoFlow {
  /// Fills row by row.
  Row,
  /// Fills column by column.
  Column,
  /// Fills row by row, backfilling earlier gaps.
  RowDense,
  /// Fills column by column, backfilling earlier gaps.
  ColumnDense,
}

/// One end of a grid item's placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum GridPlacement {
  /// Placed by the auto-placement algorithm.
  Auto,
  /// The grid line with this index (1-based; negative counts from the end).
  Line(i16),
  /// Spans this many tracks from the other end.
  Span(u16),
}

/// A start and an end on one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct Line<T> {
  /// The start.
  pub start: T,
  /// The end.
  pub end: T,
}

impl Line<GridPlacement> {
  /// A placement starting at `line`, ending by auto-placement.
  #[inline(always)]
  pub const fn at(line: i16) -> Self {
    Self {
      start: GridPlacement::Line(line),
      end: GridPlacement::Auto,
    }
  }

  /// A placement spanning `count` tracks from wherever auto-placement starts it.
  #[inline(always)]
  pub const fn span(count: u16) -> Self {
    Self {
      start: GridPlacement::Auto,
      end: GridPlacement::Span(count),
    }
  }

  /// A placement from `start` to `end`, as 1-based grid lines.
  #[inline(always)]
  pub const fn between(start: i16, end: i16) -> Self {
    Self {
      start: GridPlacement::Line(start),
      end: GridPlacement::Line(end),
    }
  }
}
