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

/// Alignment of items on the cross axis (flex) or block axis (grid).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum AlignItems {
  /// Aligns items to the start of the axis.
  Start,
  /// Aligns items to the end of the axis.
  End,
  /// Centers items on the axis.
  Center,
  /// Aligns items to their text baseline.
  Baseline,
  /// Stretches items to fill the axis.
  Stretch,
}

/// One item's own cross-axis alignment, overriding the container's `align_items`.
pub type AlignSelf = AlignItems;

/// Alignment of grid items on the inline axis.
pub type JustifyItems = AlignItems;

/// One item's own inline-axis alignment, overriding the container's `justify_items`.
pub type JustifySelf = AlignItems;

/// Distribution of content along an axis with free space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum AlignContent {
  /// Packs content at the start of the axis.
  Start,
  /// Packs content at the end of the axis.
  End,
  /// Packs content at the center of the axis.
  Center,
  /// Stretches content to fill the axis.
  Stretch,
  /// Distributes free space between items.
  SpaceBetween,
  /// Distributes free space around items.
  SpaceAround,
  /// Distributes free space evenly between and around items.
  SpaceEvenly,
}

/// Distribution of children along the main axis (flex) or inline axis (grid).
pub type JustifyContent = AlignContent;
