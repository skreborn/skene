//! The one hash the framework takes of names, keys and content: xxh3, 128 bits wide
//! where two inputs colliding must not be a case to design for.

use std::hash::{Hash, Hasher};

use xxhash_rust::{const_xxh3, xxh3};

/// The key a field or a variant is recorded under: the low half of the 64-bit xxh3 of
/// its name, stable under reordering.
#[inline(always)]
#[must_use]
#[expect(
  clippy::cast_possible_truncation,
  reason = "a key is the hash's low half"
)]
pub const fn field_key(name: &str) -> u32 {
  const_xxh3::xxh3_64(name.as_bytes()) as u32
}

/// The 128-bit xxh3 of `bytes`.
#[inline(always)]
#[must_use]
pub fn wide(bytes: &[u8]) -> u128 {
  xxh3::xxh3_128(bytes)
}

/// The 128-bit xxh3 of `value`, as it hashes, alike on every target: an integer counts as
/// its little-endian bytes, a `usize` as a `u64` and an `isize` as an `i64`.
#[must_use]
pub fn wide_of(value: &impl Hash) -> u128 {
  let mut hasher = Stable(xxh3::Xxh3::new());

  value.hash(&mut hasher);

  hasher.0.digest128()
}

struct Stable(xxh3::Xxh3);

impl Hasher for Stable {
  fn finish(&self) -> u64 {
    self.0.digest()
  }

  #[inline(always)]
  fn write(&mut self, bytes: &[u8]) {
    self.0.update(bytes);
  }

  #[inline(always)]
  fn write_u8(&mut self, value: u8) {
    self.write(&[value]);
  }

  #[inline(always)]
  fn write_u16(&mut self, value: u16) {
    self.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_u32(&mut self, value: u32) {
    self.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_u64(&mut self, value: u64) {
    self.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_u128(&mut self, value: u128) {
    self.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_usize(&mut self, value: usize) {
    self.write_u64(value as u64);
  }

  #[inline(always)]
  fn write_i8(&mut self, value: i8) {
    self.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_i16(&mut self, value: i16) {
    self.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_i32(&mut self, value: i32) {
    self.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_i64(&mut self, value: i64) {
    self.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_i128(&mut self, value: i128) {
    self.write(&value.to_le_bytes());
  }

  #[inline(always)]
  fn write_isize(&mut self, value: isize) {
    self.write_i64(value as i64);
  }
}

#[cfg(test)]
mod tests {
  use super::wide_of;

  #[test]
  fn a_value_hashes_to_what_it_hashed_to() {
    assert_eq!(wide_of(&42u32), 0x9a45_5182_d724_f034_1ad2_93a7_1185_8e8f);
    assert_eq!(wide_of(&7usize), 0xdeb6_d224_c549_cbd3_e8c0_d782_b2f6_1276);
    assert_eq!(wide_of(&-1isize), 0xdc6b_20d2_0742_5aa5_8b32_49d3_4c2e_f0b0);
    assert_eq!(wide_of(&"row"), 0xd2f2_8282_435c_c6ac_7651_138b_515a_7219);
    assert_eq!(
      wide_of(&(3usize, "a")),
      0x0225_5803_5bfe_01d1_ef69_cd80_5749_b47a
    );
    assert_eq!(
      wide_of(&vec![1u16, 2]),
      0x0758_c67a_c51a_33c6_6944_4642_ed1f_5182
    );
  }
}
