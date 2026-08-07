/// How a box is positioned in its parent's layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Position {
  /// Laid out in flow; the box's inset offsets it from its flow position.
  Relative,
  /// Taken out of flow and placed by its inset against the nearest positioned
  /// ancestor.
  Absolute,
}

/// The main axis of a flex container, and whether it runs reversed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum FlexDirection {
  /// Left to right.
  Row,
  /// Top to bottom.
  Column,
  /// Right to left.
  RowReverse,
  /// Bottom to top.
  ColumnReverse,
}

/// Whether and how a flex container wraps its children onto multiple lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum FlexWrap {
  /// One line, children shrinking as needed.
  NoWrap,
  /// Wraps onto further lines below (or after) the first.
  Wrap,
  /// Wraps onto further lines above (or before) the first.
  WrapReverse,
}

/// Distribution of children along a container's main axis.
///
/// The main axis is the flex direction; in a grid it is the horizontal (row) axis,
/// where this distributes the column tracks.
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

/// Alignment of one item within the space its container gives it.
///
/// Which axis it applies to is named by the property carrying it
/// (`cross_axis_alignment`, `main_axis_item_alignment`, and the `self_` overrides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum ItemAlignment {
  /// Aligns the item to the start of the axis.
  Start,
  /// Aligns the item to the end of the axis.
  End,
  /// Centers the item on the axis.
  Center,
  /// Aligns the item to its text baseline.
  Baseline,
  /// Stretches the item to fill the axis.
  Stretch,
}

/// Distribution of wrapped flex lines—or a grid's row tracks—along the cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum LineAlignment {
  /// Packs lines at the start of the axis.
  Start,
  /// Packs lines at the end of the axis.
  End,
  /// Packs lines at the center of the axis.
  Center,
  /// Stretches lines to fill the axis.
  Stretch,
  /// Distributes free space between lines.
  SpaceBetween,
  /// Distributes free space around lines.
  SpaceAround,
  /// Distributes free space evenly between and around lines.
  SpaceEvenly,
}
