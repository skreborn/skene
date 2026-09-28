use std::sync::atomic::{AtomicU32, Ordering};

/// Identifies one reactive entity: a signal, a computation (an effect or a memo), or an
/// [`Owner`](crate::Owner).
///
/// An id names a generational slot in the runtime of the thread that made it. A stale
/// id (its slot freed or reused, or another runtime's) resolves to nothing rather than
/// to a different entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct Id(u64);

/// The arena an id addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
  Signal = 0,
  Computation = 1,
  Owner = 2,
}

/// How many bits a slot index takes: an arena holds under `2^INDEX_BITS` slots.
pub(crate) const INDEX_BITS: u32 = 28;
const GENERATION_SHIFT: u32 = INDEX_BITS;
const NONCE_SHIFT: u32 = GENERATION_SHIFT + 16;
const NONCE_MASK: u32 = (1 << 18) - 1;
const KIND_SHIFT: u32 = 62;

/// The next runtime's nonce; distinguishes ids across threads and resets.
static NEXT_NONCE: AtomicU32 = AtomicU32::new(0);

/// Allocates a nonce for a fresh runtime.
pub(crate) fn next_nonce() -> u32 {
  NEXT_NONCE.fetch_add(1, Ordering::Relaxed) & NONCE_MASK
}

impl Id {
  /// Packs an id from its arena, its runtime's nonce, slot generation, and slot index.
  #[inline(always)]
  pub(crate) const fn local(kind: Kind, nonce: u32, generation: u16, index: u32) -> Self {
    Self(
      ((kind as u64) << KIND_SHIFT)
        | (((nonce & NONCE_MASK) as u64) << NONCE_SHIFT)
        | ((generation as u64) << GENERATION_SHIFT)
        | index as u64,
    )
  }

  /// The arena this id addresses.
  #[inline(always)]
  pub(crate) const fn kind(self) -> Kind {
    match self.0 >> KIND_SHIFT {
      0 => Kind::Signal,
      1 => Kind::Computation,
      _ => Kind::Owner,
    }
  }

  /// The allocating runtime's nonce.
  #[inline(always)]
  pub(crate) const fn nonce(self) -> u32 {
    ((self.0 >> NONCE_SHIFT) as u32) & NONCE_MASK
  }

  /// The slot generation this id was minted for.
  #[inline(always)]
  #[expect(
    clippy::cast_possible_truncation,
    reason = "the generation is these bits of the id"
  )]
  pub(crate) const fn generation(self) -> u16 {
    (self.0 >> GENERATION_SHIFT) as u16
  }

  /// The slot index in the allocating runtime.
  #[inline(always)]
  #[expect(
    clippy::cast_possible_truncation,
    reason = "the index is the id's low bits"
  )]
  pub(crate) const fn index(self) -> u32 {
    (self.0 as u32) & ((1 << INDEX_BITS) - 1)
  }
}
