//! The identity a scope carries across two runs: its position, or the key anchoring it.

use std::{
  hash::{Hash, Hasher},
  rc::Rc,
};

use serde::{Deserialize, Serialize};

use crate::runtime::RUNTIME;

/// A scope's stable identity, naming its place in a state snapshot.
///
/// A key anchors the place its position would otherwise give it, so state under a
/// keyed scope survives an edit or a reorder around it. Keys are matched across runs
/// by a 128-bit hash of the value they are taken from, so two values hashing alike are
/// one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct Key(u128);

impl Key {
  /// The key of `value`.
  #[inline(always)]
  pub fn of(value: &impl Hash) -> Self {
    let mut hasher = Fnv128::default();

    value.hash(&mut hasher);

    Self(hasher.hash)
  }
}

/// The 128-bit FNV-1a hasher a key is taken with: wide enough that two keys colliding
/// is not a case to design for.
struct Fnv128 {
  hash: u128,
}

impl Default for Fnv128 {
  #[inline(always)]
  fn default() -> Self {
    Self {
      hash: 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d,
    }
  }
}

impl Hasher for Fnv128 {
  #[inline(always)]
  fn write(&mut self, bytes: &[u8]) {
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    for byte in bytes {
      self.hash ^= u128::from(*byte);
      self.hash = self.hash.wrapping_mul(PRIME);
    }
  }

  #[inline(always)]
  fn finish(&self) -> u64 {
    self.hash as u64
  }
}

/// One step of an owner path: a position among its siblings, or a [`Key`].
///
/// A keyed step takes no position, so keying a sibling shifts none of the positional
/// ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct Step(u128);

impl Step {
  /// The bit separating the keyed steps from the positional ones.
  const KEYED: u128 = 1 << 127;

  /// The step of the sibling at `ordinal`.
  #[inline(always)]
  pub(crate) const fn position(ordinal: u32) -> Self {
    Self(ordinal as u128)
  }

  /// The step `key` names.
  #[inline(always)]
  pub(crate) const fn key(key: Key) -> Self {
    Self(key.0 | Self::KEYED)
  }
}

/// A place in the state snapshot: an owner's path and an index in its sequence.
///
/// A snapshot signal takes one at creation, and so may anything else a platform carries
/// across runs by [`provide`](crate::provide)/[`staged`](crate::staged)—a focused or
/// scrolled node, say—so it is matched across runs exactly as a signal is, keys
/// included.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "Wire", into = "Wire")]
#[must_use]
pub struct Position {
  pub(crate) path: Rc<[Step]>,
  pub(crate) index: u32,
}

/// A [`Position`] as it crosses runs.
#[derive(Serialize, Deserialize)]
struct Wire(Vec<Step>, u32);

impl From<Wire> for Position {
  #[inline(always)]
  fn from(Wire(path, index): Wire) -> Self {
    Self {
      path: Rc::from(path),
      index,
    }
  }
}

impl From<Position> for Wire {
  #[inline(always)]
  fn from(position: Position) -> Self {
    Self(position.path.to_vec(), position.index)
  }
}

impl Position {
  /// Takes the next place in the current owner's sequence.
  pub fn next() -> Self {
    RUNTIME.with(|runtime| {
      let owner = runtime.current_owner.get();
      let mut graph = runtime.graph();

      Self {
        path: graph.owner_path(owner),
        index: graph.next_index(owner),
      }
    })
  }
}
