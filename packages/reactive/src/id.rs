use std::sync::atomic::{AtomicU16, Ordering};

/// Identifies one reactive entity: a signal, a computation (an effect or a memo), or an
/// [`Owner`](crate::Owner).
///
/// An id names a generational slot in its allocating runtime—the calling thread's for
/// local entities, the process-wide registry for sync signals. A stale id (its slot
/// freed or reused, or another thread's runtime) resolves to nothing rather than to a
/// different entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct Id(u64);

/// The arena an id addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
  Signal = 0,
  Computation = 1,
  Owner = 2,
  Sync = 3,
}

const SYNC_BIT: u64 = 1 << 63;
const KIND_SHIFT: u32 = 61;
const KIND_MASK: u64 = 0b11;
const NONCE_SHIFT: u32 = 48;
const NONCE_MASK: u16 = 0x1FFF;

/// The next local runtime's nonce; distinguishes ids across threads and resets.
static NEXT_NONCE: AtomicU16 = AtomicU16::new(0);

/// Allocates a nonce for a fresh local runtime.
pub(crate) fn next_nonce() -> u16 {
  NEXT_NONCE.fetch_add(1, Ordering::Relaxed) & NONCE_MASK
}

impl Id {
  /// Packs a local id from its arena, its runtime's nonce, slot generation, and slot
  /// index.
  #[inline(always)]
  pub(crate) const fn local(kind: Kind, nonce: u16, generation: u16, index: u32) -> Self {
    Self(
      ((kind as u64) << KIND_SHIFT)
        | (((nonce & NONCE_MASK) as u64) << NONCE_SHIFT)
        | ((generation as u64) << 32)
        | index as u64,
    )
  }

  /// Packs a sync id from its slot generation and slot index.
  #[inline(always)]
  pub(crate) const fn sync(generation: u16, index: u32) -> Self {
    Self(SYNC_BIT | ((generation as u64) << 32) | index as u64)
  }

  /// Whether this id names a sync signal rather than a local entity.
  #[inline(always)]
  pub(crate) const fn is_sync(self) -> bool {
    self.0 & SYNC_BIT != 0
  }

  /// The arena this id addresses.
  #[inline(always)]
  pub(crate) const fn kind(self) -> Kind {
    if self.is_sync() {
      return Kind::Sync;
    }

    match (self.0 >> KIND_SHIFT) & KIND_MASK {
      0 => Kind::Signal,
      1 => Kind::Computation,
      _ => Kind::Owner,
    }
  }

  /// The allocating local runtime's nonce.
  #[inline(always)]
  pub(crate) const fn nonce(self) -> u16 {
    ((self.0 >> NONCE_SHIFT) as u16) & NONCE_MASK
  }

  /// The slot generation this id was minted for.
  #[inline(always)]
  #[expect(
    clippy::cast_possible_truncation,
    reason = "the generation is these bits of the id"
  )]
  pub(crate) const fn generation(self) -> u16 {
    (self.0 >> 32) as u16
  }

  /// The slot index in the allocating runtime.
  #[inline(always)]
  #[expect(
    clippy::cast_possible_truncation,
    reason = "the index is the id's low bits"
  )]
  pub(crate) const fn index(self) -> u32 {
    self.0 as u32
  }
}
