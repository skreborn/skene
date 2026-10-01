use super::Dimension;

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
/// in proportion to the children's grow factors, and overflow reclaimed in proportion
/// to their shrink factors. A factor of zero opts out of that direction of flexing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Flexibility<T> {
  /// The main-axis size flexing starts from.
  pub initial: Dimension<T>,
  /// The child's weight when free space is handed out.
  pub grow_factor: f32,
  /// The child's weight when overflow is reclaimed.
  pub shrink_factor: f32,
}

impl<T> Flexibility<T> {
  /// Flexibility from all three components.
  #[inline(always)]
  pub const fn new(initial: Dimension<T>, grow_factor: f32, shrink_factor: f32) -> Self {
    Self {
      initial,
      grow_factor,
      shrink_factor,
    }
  }

  /// Starts from the content size and takes `weight` of the free space.
  #[inline(always)]
  pub const fn grow(weight: f32) -> Self {
    Self {
      initial: Dimension::Auto,
      grow_factor: weight,
      shrink_factor: 1.,
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
      grow_factor: weight,
      shrink_factor: 1.,
    }
  }
}

impl<T> Default for Flexibility<T> {
  /// Inflexible: the content's own size, shrinking only.
  fn default() -> Self {
    Self {
      initial: Dimension::Auto,
      grow_factor: 0.,
      shrink_factor: 1.,
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

/// Distribution of a set of things—children, wrapped lines, grid tracks—along an
/// axis with free space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Arrangement {
  /// Packs everything at the start of the axis.
  Start,
  /// Packs everything at the end of the axis.
  End,
  /// Packs everything at the center of the axis.
  Center,
  /// Stretches the stretchable to fill the axis; children pack at the start.
  Stretch,
  /// Distributes free space between the entries.
  SpaceBetween,
  /// Distributes free space around the entries.
  SpaceAround,
  /// Distributes free space evenly between and around the entries.
  SpaceEvenly,
}

/// Alignment of one item within the space its container gives it.
///
/// Which axis it applies to comes from the property carrying it: the container's
/// `item_alignment`, or a wrapper view's own alignment.
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

/// A two-axis alignment: where an item sits in the cell or region it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct Alignment {
  /// The horizontal placement.
  pub horizontal: ItemAlignment,
  /// The vertical placement.
  pub vertical: ItemAlignment,
}

impl Alignment {
  /// Centered on both axes.
  pub const CENTER: Self = Self::new(ItemAlignment::Center, ItemAlignment::Center);

  /// An alignment from its two axes.
  #[inline(always)]
  pub const fn new(horizontal: ItemAlignment, vertical: ItemAlignment) -> Self {
    Self {
      horizontal,
      vertical,
    }
  }
}
