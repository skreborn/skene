/// Alignment of children along the main (layout) axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum MainAxisAlignment {
  /// Packs children at the start of the axis.
  Start,
  /// Packs children at the end of the axis.
  End,
  /// Packs children at the center of the axis.
  Center,
  /// Distributes free space between children.
  SpaceBetween,
  /// Distributes free space around children.
  SpaceAround,
  /// Distributes free space evenly between and around children.
  SpaceEvenly,
}

/// Alignment of children along the cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum CrossAxisAlignment {
  /// Aligns children to the start of the cross axis.
  Start,
  /// Aligns children to the end of the cross axis.
  End,
  /// Centers children on the cross axis.
  Center,
  /// Stretches children to fill the cross axis.
  Stretch,
  /// Aligns children to their text baseline.
  Baseline,
}
