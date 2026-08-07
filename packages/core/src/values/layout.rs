use super::FlexDirection;

/// The layout mode of a box.
///
/// Grid templates, auto tracks, and flow live in the box's style
/// (`grid_template_columns` and friends), as every other layout property does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Layout {
  /// Block flow: children flow in normal document order, contained within the box.
  Block,
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
