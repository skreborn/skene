//! The identity a scope carries across two runs: its position, or the key anchoring it.

use std::{hash::Hash, rc::Rc};

use crate::{
  encode::{Recorder, Snapshot, Value},
  hash,
  runtime::RUNTIME,
};

/// A scope's stable identity, naming its place in a state snapshot.
///
/// A key anchors the place its position would otherwise give it, so state under a
/// keyed scope survives an edit or a reorder around it. Keys are matched across runs
/// by a 128-bit hash of the value they are taken from ([`hash::wide_of`], alike on every
/// target), so two values hashing alike are one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct Key(u128);

impl Key {
  /// The key of `value`.
  pub fn of(value: &impl Hash) -> Self {
    Self(hash::wide_of(value))
  }
}

/// One step of an owner path: a position among its siblings, or a [`Key`].
///
/// A keyed step takes no position, so keying a sibling shifts none of the positional
/// ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Step(u128);

impl Snapshot for Step {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.wide(self.0);
  }

  #[inline(always)]
  fn restore(value: Value<'_>) -> Option<Self> {
    u128::restore(value).map(Self)
  }
}

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
/// across runs by [`provide_extra`](crate::provide_extra)/[`take_extra`](crate::take_extra)—a
/// focused or scrolled node, say—so it is matched across runs exactly as a signal is, keys
/// included.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use]
pub struct SnapshotPlace {
  pub(crate) path: Rc<[Step]>,
  pub(crate) index: u32,
}

impl Snapshot for SnapshotPlace {
  fn record(&self, recorder: &mut Recorder) {
    recorder.seq(2, |recorder| {
      Step::record_slice(&self.path, recorder);
      self.index.record(recorder);
    });
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    let Value::Seq(mut items) = value
    else {
      return None;
    };
    let path = Step::restore_slice(items.next()?)?;
    let index = u32::restore(items.next()?)?;

    Some(Self {
      path: Rc::from(path),
      index,
    })
  }
}

impl SnapshotPlace {
  /// Takes the next place in the current owner's sequence while snapshots are
  /// recorded, and the root's first place, shared by everything, while they are not;
  /// see [`record_snapshots`](crate::record_snapshots).
  pub fn next() -> Self {
    if !crate::snapshot::recording() {
      return Self {
        path: UNRECORDED.with(Rc::clone),
        index: 0,
      };
    }

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

thread_local! {
  /// The path every place shares while nothing records.
  static UNRECORDED: Rc<[Step]> = Rc::from([]);
}
