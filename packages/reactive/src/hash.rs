//! The one hash the framework takes of names, keys and content: xxh3, 128 bits wide
//! where two inputs colliding must not be a case to design for.

use std::hash::Hash;

use xxhash_rust::{const_xxh3, xxh3};

/// The key a field or a variant is recorded under: the low half of the 64-bit xxh3 of
/// its name, stable under reordering.
#[inline(always)]
#[must_use]
#[expect(
  clippy::cast_possible_truncation,
  reason = "a key is the hash's low half"
)]
pub const fn key(name: &str) -> u32 {
  const_xxh3::xxh3_64(name.as_bytes()) as u32
}

/// The 128-bit xxh3 of `bytes`.
#[inline(always)]
#[must_use]
pub fn wide(bytes: &[u8]) -> u128 {
  xxh3::xxh3_128(bytes)
}

/// The 128-bit xxh3 of `value`, as it hashes.
#[must_use]
pub fn wide_of(value: &impl Hash) -> u128 {
  let mut hasher = xxh3::Xxh3::new();

  value.hash(&mut hasher);

  hasher.digest128()
}
