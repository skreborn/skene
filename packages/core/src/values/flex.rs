use super::Dimension;

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
pub enum Wrap {
  /// One line, children shrinking as needed.
  None,
  /// Wraps onto further lines after the first.
  Forward,
  /// Wraps onto further lines before the first.
  Reverse,
}

/// A child's flexibility: the size it starts from, and its weights for taking free
/// space and giving up overflow.
///
/// The child starts at `initial` on the main axis; leftover space is then handed out
/// in proportion to the children's `grow` weights, and overflow reclaimed in
/// proportion to their `shrink` weights. A weight of zero opts out of that direction
/// of flexing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Flexibility<T> {
  /// The main-axis size flexing starts from.
  pub initial: Dimension<T>,
  /// The child's weight when free space is handed out.
  pub grow: f32,
  /// The child's weight when overflow is reclaimed.
  pub shrink: f32,
}

impl<T> Flexibility<T> {
  /// Flexibility from all three components.
  #[inline(always)]
  pub const fn new(initial: Dimension<T>, grow: f32, shrink: f32) -> Self {
    Self {
      initial,
      grow,
      shrink,
    }
  }

  /// Starts from the content size and takes `weight` of the free space.
  #[inline(always)]
  pub const fn grow(weight: f32) -> Self {
    Self {
      initial: Dimension::Auto,
      grow: weight,
      shrink: 1.,
    }
  }

  /// Splits the whole axis by weight: starts from zero, so siblings sharing an axis
  /// divide it in proportion to their weights.
  #[inline(always)]
  pub const fn weight(weight: f32) -> Self
  where
    T: Copy,
  {
    Self {
      initial: Dimension::Fraction(0.),
      grow: weight,
      shrink: 1.,
    }
  }
}

impl<T> Default for Flexibility<T> {
  /// Inflexible: the content's own size, shrinking only.
  fn default() -> Self {
    Self {
      initial: Dimension::Auto,
      grow: 0.,
      shrink: 1.,
    }
  }
}

/// The spacing between a container's children, per axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct Spacing<T> {
  /// The space between rows.
  pub row: T,
  /// The space between columns.
  pub column: T,
}

impl<T: Copy> Spacing<T> {
  /// The same spacing on both axes.
  #[inline(always)]
  pub const fn all(value: T) -> Self {
    Self {
      row: value,
      column: value,
    }
  }
}

impl<T: Copy> From<T> for Spacing<T> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::all(value)
  }
}

/// Distribution of a container's children along its main axis.
///
/// The main axis is the flex direction; in a grid it is the horizontal (row) axis,
/// where this distributes the column tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Arrangement {
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

/// Distribution of wrapped flex lines—or a grid's row tracks—along the cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum LineArrangement {
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
