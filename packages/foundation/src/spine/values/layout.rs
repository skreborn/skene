use super::Axis;

/// The layout mode of a box.
///
/// Grid tracks, implicit tracks, and flow live in the box's style (`grid_columns`
/// and friends), as every other layout property does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Layout {
  /// Children in document order, one after another, contained within the box.
  Document,
  /// Flex: children laid out one after another along `axis`.
  Flex {
    /// The main axis.
    axis: Axis,
    /// Whether the children run from the axis's end: right to left, or bottom to top.
    reversed: bool,
  },
  /// Grid: children placed into the style's tracks.
  Grid,
  /// Children overlap in one cell, stacked in order (last on top).
  Stack,
  /// One cell holding exactly one child: the child fills the box when the box is
  /// sized, and the box hugs the child when it is not.
  Single,
}
