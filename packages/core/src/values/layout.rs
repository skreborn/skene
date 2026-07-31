use super::Axis;

/// The layout mode of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Layout {
  /// Block flow: children flow in normal document order, contained within the box.
  Block,
  /// Flex: children laid out along `direction`.
  Flex {
    /// The main (layout) axis.
    direction: Axis,
  },
  /// Grid with `columns` equal-width columns.
  Grid {
    /// The number of columns.
    columns: u16,
  },
  /// Children overlap in one cell, stacked in order (last on top).
  Stack,
}
