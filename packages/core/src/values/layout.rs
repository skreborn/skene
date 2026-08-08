use super::FlexDirection;

/// The layout mode of a box.
///
/// Grid tracks, implicit tracks, and flow live in the box's style (`grid_columns`
/// and friends), as every other layout property does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Layout {
  /// Normal flow: children stack in document order, contained within the box.
  Flow,
  /// Flex: children laid out along `direction`.
  Flex {
    /// The main axis, and whether it runs reversed.
    direction: FlexDirection,
  },
  /// Grid: children placed into the style's tracks.
  Grid,
  /// Children overlap in one cell, stacked in order (last on top).
  Stack,
}
