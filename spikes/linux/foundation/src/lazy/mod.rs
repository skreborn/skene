//! Lists that realize only the items in view: the values a lazy list takes and the engine
//! that windows it, written once for every platform ([`engine::Engine`]); the view that drives
//! it over the seam is [`crate::view::Lazy`].
//!
//! A list's items are identified by key ([`Key`]), laid out in lines (one item a line in a
//! column or a row, several in a grid, [`GridCells`]), each line's extent fixed or measured as
//! it is realized and estimated before ([`ItemExtent`]); a viewport realizes the lines it
//! shows and a few beyond each edge, and an anchor keeps what the reader sees still while
//! extents and items change.

pub mod engine;
pub mod extents;

use std::hash::{DefaultHasher, Hash, Hasher};

use crate::{AxisAlignment, IntoUnit};

/// What identifies a list's item across changes: the hash of the value it was keyed by and of
/// its segment's place among the list's segments, 128 bits wide, so two keys of one list
/// collide only by a chance no list meets.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Key(u128);

impl Key {
  /// The key of an item keyed by `value` in the list's segment at `segment`.
  #[must_use]
  pub fn of(segment: usize, value: &impl Hash) -> Self {
    let half = |salt: u8| {
      let mut hasher = DefaultHasher::new();

      salt.hash(&mut hasher);
      segment.hash(&mut hasher);
      value.hash(&mut hasher);

      hasher.finish()
    };

    Self(u128::from(half(0)) << 64 | u128::from(half(1)))
  }

  /// The key of the item the segment at `segment` holds alone: a single item or a header,
  /// keyed by its place.
  #[must_use]
  pub fn place(segment: usize) -> Self {
    Self::of(segment, &"place")
  }
}

/// How long a list's items are along its axis.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ItemExtent {
  /// Every item this long: none is measured (Flutter's `itemExtent`).
  Fixed(f32),
  /// Each item measured as it is realized and its measure kept by key; until then this long,
  /// then the mean of the measured once the list is at rest.
  Estimated(f32),
}

impl ItemExtent {
  /// The length it names: the fixed one, or the estimate.
  #[must_use]
  pub const fn length(self) -> f32 {
    match self {
      Self::Fixed(length) | Self::Estimated(length) => length,
    }
  }
}

impl From<i32> for ItemExtent {
  /// A fixed extent: `set_item_height(48)`.
  fn from(length: i32) -> Self {
    Self::Fixed(length.into_unit())
  }
}

impl From<f32> for ItemExtent {
  /// A fixed extent: `set_item_height(48.5)`.
  fn from(length: f32) -> Self {
    Self::Fixed(length)
  }
}

/// How many items a lazy grid's line holds, across its axis: Compose's `GridCells`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GridCells {
  /// This many, sharing the line's room equally.
  Count(u16),
  /// As many as fit at this length or more, sharing the rest.
  Adaptive(f32),
  /// As many as fit at exactly this length.
  FixedSize(f32),
}

impl GridCells {
  /// How many items a line `across` long holds, `spacing` between neighbors: one at least.
  #[must_use]
  pub fn per_line(self, across: f32, spacing: f32) -> usize {
    let fits = |length: f32| {
      if length <= 0. {
        return 1;
      }

      (((across + spacing) / (length + spacing)).floor() as usize).max(1)
    };

    match self {
      Self::Count(count) => usize::from(count.max(1)),
      Self::Adaptive(length) | Self::FixedSize(length) => fits(length),
    }
  }
}

/// Where a lazy list is scrolled to: the item at `index` (its place among the list's items and
/// headers) with its point at `alignment` (start, center or end) `offset` pixels from the
/// viewport's matching point. One value, as the seam's contents are.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScrollPosition {
  /// The item's place among the list's items.
  pub index: usize,
  /// Which point of the item and of the viewport meet.
  pub alignment: AxisAlignment,
  /// How far the item's point is from the viewport's, along the axis, in pixels: positive
  /// toward the end.
  pub offset: f32,
}

impl ScrollPosition {
  /// The item at `index` at the viewport's start.
  #[must_use]
  pub const fn at(index: usize) -> Self {
    Self {
      index,
      alignment: AxisAlignment::Start,
      offset: 0.,
    }
  }

  /// The fraction along the axis its alignment names: `0` the start, `0.5` the center, `1`
  /// the end.
  #[must_use]
  pub const fn fraction(self) -> f64 {
    match self.alignment {
      AxisAlignment::Center => 0.5,
      AxisAlignment::End => 1.,
      AxisAlignment::Start | AxisAlignment::Stretch | AxisAlignment::Baseline => 0.,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{GridCells, Key};

  #[test]
  fn keys_part_by_segment_and_value() {
    assert_eq!(Key::of(0, &7_u32), Key::of(0, &7_u32));
    assert_ne!(Key::of(0, &7_u32), Key::of(1, &7_u32));
    assert_ne!(Key::of(0, &7_u32), Key::of(0, &8_u32));
    assert_ne!(Key::place(0), Key::place(1));
  }

  #[test]
  fn a_grid_line_holds_what_fits() {
    assert_eq!(GridCells::Count(3).per_line(100., 8.), 3);
    assert_eq!(GridCells::Adaptive(160.).per_line(700., 8.), 4);
    assert_eq!(GridCells::Adaptive(160.).per_line(100., 8.), 1);
    assert_eq!(GridCells::FixedSize(120.).per_line(500., 10.), 3);
  }
}
