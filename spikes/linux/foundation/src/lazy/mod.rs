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

use std::{
  collections::{HashMap, HashSet},
  hash::{BuildHasherDefault, DefaultHasher, Hash, Hasher},
};

use crate::{AxisAlignment, IntoUnit, Property, TokenValue};

/// What identifies a list's item across changes: the hash of the value it was keyed by and of
/// its segment's place among the list's segments, 128 bits wide, so two keys of one list
/// collide only by a chance no list meets, and the same on every target one toolchain builds
/// for ([`WideHasher`]): the web's server leaf and browser leaf agree on it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
#[must_use]
pub struct Key(u128);

impl Key {
  /// The key of an item keyed by `value` in the list's segment at `segment`.
  pub fn of(segment: usize, value: &impl Hash) -> Self {
    Self::hashed(segment, &|mut state: &mut dyn Hasher| {
      value.hash(&mut state)
    })
  }

  /// The key of an item in the list's segment at `segment` whose value `value` hashes.
  fn hashed(segment: usize, value: &dyn Fn(&mut dyn Hasher)) -> Self {
    let half = |salt: u8| {
      let mut hasher = WideHasher::default();

      salt.hash(&mut hasher);
      segment.hash(&mut hasher);
      value(&mut hasher);

      hasher.finish()
    };

    Self(u128::from(half(0)) << 64 | u128::from(half(1)))
  }

  /// The key of the item the segment at `segment` holds alone: a single item or a header,
  /// keyed by its place.
  pub fn place(segment: usize) -> Self {
    Self::of(segment, &"place")
  }
}

/// std's hasher writing the same bytes on every target: integers little-endian, `usize` and
/// `isize` widened to 64 bits. Its hashes are one toolchain's, as std promises no more.
#[derive(Clone, Default, Debug)]
pub struct WideHasher(DefaultHasher);

impl Hasher for WideHasher {
  #[inline(always)]
  fn finish(&self) -> u64 {
    self.0.finish()
  }

  #[inline(always)]
  fn write(&mut self, bytes: &[u8]) {
    self.0.write(bytes);
  }

  #[inline(always)]
  fn write_u8(&mut self, value: u8) {
    self.0.write(&[value]);
  }

  #[inline(always)]
  fn write_u16(&mut self, value: u16) {
    self.0.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_u32(&mut self, value: u32) {
    self.0.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_u64(&mut self, value: u64) {
    self.0.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_u128(&mut self, value: u128) {
    self.0.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_usize(&mut self, value: usize) {
    self.write_u64(value as u64);
  }

  #[inline(always)]
  fn write_i8(&mut self, value: i8) {
    self.0.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_i16(&mut self, value: i16) {
    self.0.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_i32(&mut self, value: i32) {
    self.0.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_i64(&mut self, value: i64) {
    self.0.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_i128(&mut self, value: i128) {
    self.0.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_isize(&mut self, value: isize) {
    self.write_i64(value as i64);
  }
}

/// Hashes a [`Key`], itself a wide hash, as its low 64 bits; anything else byte by byte.
#[derive(Clone, Copy, Default, Debug)]
pub struct KeyHasher(u64);

impl Hasher for KeyHasher {
  #[inline(always)]
  fn finish(&self) -> u64 {
    self.0
  }

  fn write(&mut self, bytes: &[u8]) {
    for byte in bytes {
      self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
    }
  }

  #[inline(always)]
  #[expect(
    clippy::cast_possible_truncation,
    reason = "the low half of a wide hash"
  )]
  fn write_u128(&mut self, value: u128) {
    self.0 = value as u64;
  }
}

/// A map from a list's items' keys, hashed by [`KeyHasher`].
pub type KeyMap<V> = HashMap<Key, V, BuildHasherDefault<KeyHasher>>;

/// A set of a list's items' keys, hashed by [`KeyHasher`].
pub type KeySet = HashSet<Key, BuildHasherDefault<KeyHasher>>;

/// How long a list's items are along its axis.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub enum ItemExtent {
  /// Every item this long: none is measured (Flutter's `itemExtent`).
  Fixed(f32),
  /// Each item measured as it is realized and its measure kept by key; until then this long,
  /// then the mean of the measured once the list is at rest.
  Estimated(f32),
}

impl ItemExtent {
  /// Every item `length` long, none measured: an integer or a float.
  #[inline(always)]
  pub fn fixed(length: impl IntoUnit<f32>) -> Self {
    Self::Fixed(length.into_unit())
  }

  /// Each item measured as it is realized, `length` long until then: an integer or a float.
  #[inline(always)]
  pub fn estimated(length: impl IntoUnit<f32>) -> Self {
    Self::Estimated(length.into_unit())
  }

  /// The length it names: the fixed one, or the estimate.
  #[must_use]
  pub const fn length(self) -> f32 {
    match self {
      Self::Fixed(length) | Self::Estimated(length) => length,
    }
  }
}

impl From<f32> for ItemExtent {
  /// A fixed extent: `set_item_height(48.5)`.
  fn from(length: f32) -> Self {
    Self::Fixed(length)
  }
}

/// A length token stands for a fixed extent of its length.
impl Property for ItemExtent {
  #[inline(always)]
  fn from_token(value: &TokenValue) -> Option<Self> {
    f32::from_token(value).map(Self::Fixed)
  }
}

/// How many items a lazy grid's line holds, across its axis: Compose's `GridCells`.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub enum GridCells {
  /// This many, sharing the line's room equally.
  Count(u16),
  /// As many as fit at this length or more, sharing the rest.
  Adaptive(f32),
  /// As many as fit at exactly this length.
  FixedSize(f32),
}

impl GridCells {
  /// As many as fit at `length` or more, sharing the rest: an integer or a float.
  #[inline(always)]
  pub fn adaptive(length: impl IntoUnit<f32>) -> Self {
    Self::Adaptive(length.into_unit())
  }

  /// As many as fit at exactly `length`: an integer or a float.
  #[inline(always)]
  pub fn fixed_size(length: impl IntoUnit<f32>) -> Self {
    Self::FixedSize(length.into_unit())
  }

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
#[must_use]
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
  use std::hash::{Hash, Hasher};

  use super::{GridCells, Key, WideHasher};

  #[test]
  fn keys_part_by_segment_and_value() {
    assert_eq!(Key::of(0, &7_u32), Key::of(0, &7_u32));
    assert_ne!(Key::of(0, &7_u32), Key::of(1, &7_u32));
    assert_ne!(Key::of(0, &7_u32), Key::of(0, &8_u32));
    assert_ne!(Key::place(0), Key::place(1));
  }

  #[test]
  fn a_key_hashes_the_same_bytes_on_every_target() {
    let hash = |value: &dyn Fn(&mut WideHasher)| {
      let mut hasher = WideHasher::default();

      value(&mut hasher);
      hasher.finish()
    };

    assert_eq!(
      hash(&|hasher| 7_usize.hash(hasher)),
      hash(&|hasher| 7_u64.hash(hasher)),
      "a usize is 64 bits wide"
    );
    assert_eq!(
      hash(&|hasher| (-3_isize).hash(hasher)),
      hash(&|hasher| (-3_i64).hash(hasher)),
      "an isize is 64 bits wide"
    );
    assert_eq!(
      hash(&|hasher| 0x0102_u16.hash(hasher)),
      hash(&|hasher| hasher.write(&[2, 1])),
      "an integer is little-endian"
    );
    // The value a wasm32 build of the same toolchain computes (1.99.1, run under Node 22).
    assert_eq!(
      format!("{:032x}", Key::of(0, &7_usize).0),
      "dbc83ab89a2d4178da8e5897d5cc9594"
    );
  }

  #[test]
  fn a_grid_line_holds_what_fits() {
    assert_eq!(GridCells::Count(3).per_line(100., 8.), 3);
    assert_eq!(GridCells::Adaptive(160.).per_line(700., 8.), 4);
    assert_eq!(GridCells::Adaptive(160.).per_line(100., 8.), 1);
    assert_eq!(GridCells::FixedSize(120.).per_line(500., 10.), 3);
  }
}
