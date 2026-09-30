//! The graph's records: signals, computations and owners, allocated, found by id and
//! removed.

#[cfg(debug_assertions)]
use std::panic::Location;
use std::{
  any::{Any, TypeId},
  rc::Rc,
};

use rustc_hash::FxHashMap;

use super::Graph;
#[cfg(debug_assertions)]
use crate::obituary::{Disposing, Obituary};
use crate::{
  computation::Computation,
  id::{Id, Kind},
  link::LinkList,
  path::{Key, Step},
  spawn::TaskGuard,
};

/// A computation's state bits.
pub(crate) mod flags {
  /// A dependency changed; the next run is due.
  pub(crate) const DIRTY: u8 = 1;
  /// A transitive dependency may have changed; check before running.
  pub(crate) const PENDING: u8 = 2;
  /// Sitting in a run queue.
  pub(crate) const QUEUED: u8 = 4;
  /// Currently running.
  pub(crate) const RUNNING: u8 = 8;
  /// Paused: invalidations accumulate, runs wait for a resume.
  pub(crate) const PAUSED: u8 = 16;
  /// Flushes before every ordinary effect.
  pub(crate) const RENDER: u8 = 32;
  /// A memo: a lazy dep with a version, never queued.
  pub(crate) const MEMO: u8 = 64;
  /// On the stack of a dependency check, so a cycle back to it ends the descent.
  pub(crate) const CHECKING: u8 = 128;
}

/// Context values stored on one owner.
pub(crate) type Contexts = FxHashMap<TypeId, Rc<dyn Any>>;

/// The tasks running under one owner, by id.
pub(crate) type Tasks = FxHashMap<u64, TaskGuard>;

/// An owner's children in the order they were adopted: added at the end and removed
/// anywhere in amortized constant time.
#[derive(Default)]
pub(crate) struct Children {
  /// Each child as adopted, with the sequence number it was adopted under; an entry
  /// whose number is no longer its child's current one is stale.
  order: Vec<(Id, u32)>,
  /// Each child's current sequence number.
  live: FxHashMap<Id, u32>,
  /// The sequence number the next child adopted takes.
  next: u32,
}

impl Children {
  /// Adds `child` at the end, unless it is a child already.
  pub(super) fn insert(&mut self, child: Id) {
    if self.live.contains_key(&child) {
      return;
    }

    self.live.insert(child, self.next);
    self.order.push((child, self.next));
    self.next = self.next.wrapping_add(1);
  }

  /// Removes `child`, compacting once stale entries outnumber the live ones.
  pub(super) fn remove(&mut self, child: Id) {
    if self.live.remove(&child).is_none() {
      return;
    }

    if self.order.len() > 2 * self.live.len() + 16 {
      let live = &self.live;

      self
        .order
        .retain(|(child, sequence)| live.get(child) == Some(sequence));
    }
  }

  /// The children in the order they were adopted.
  pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = Id> + '_ {
    self
      .order
      .iter()
      .filter(|(child, sequence)| self.live.get(child) == Some(sequence))
      .map(|(child, _)| *child)
  }
}

/// The ownership facet owners and computations share.
#[derive(Default)]
pub(crate) struct OwnerData {
  /// The owner that adopted it.
  pub(crate) parent: Option<Id>,
  /// How many owners stand above it.
  pub(crate) depth: u32,
  /// The signals, owners and computations it adopted.
  pub(crate) children: Option<Box<Children>>,
  /// The context values it provides.
  pub(crate) contexts: Option<Box<Contexts>>,
  /// The cleanups registered on it, run in reverse when it is disposed or runs again.
  pub(crate) cleanups: Vec<Box<dyn FnOnce()>>,
  /// The tasks running under it, cancelled when it is disposed or runs again.
  pub(crate) tasks: Option<Box<Tasks>>,
  /// The step from the parent, or from the runtime root for a detached owner.
  pub(super) step: Option<Step>,
  /// The next positional owner child's ordinal.
  pub(super) next_child: u32,
  /// The next index in the snapshot sequence.
  pub(super) next_index: u32,
  /// The path of steps from the runtime root, cached once computed.
  pub(super) path: Option<Rc<[Step]>>,
  /// The key the next child adopted takes instead of a position.
  pub(super) pending_key: Option<Key>,
}

/// A local signal: its value cell and its subscriber list.
pub(crate) struct SignalRecord {
  /// The value cell.
  pub(crate) value: Rc<dyn Any>,
  /// The owner that adopted it.
  pub(super) parent: Option<Id>,
  /// The computations reading it.
  pub(super) subs: LinkList,
  /// Bumped on every write, which a subscriber's link is compared against.
  pub(super) version: u32,
  /// Registered in the state snapshot, to be unregistered on removal.
  snapshotted: bool,
  /// Where the signal was created, in a debug build.
  #[cfg(debug_assertions)]
  created_at: Option<&'static Location<'static>>,
}

/// An effect or memo: its body, state bits, both link lists and its ownership facet.
pub(crate) struct ComputationRecord {
  pub(crate) flags: u8,
  pub(super) deps: LinkList,
  pub(super) subs: LinkList,
  pub(super) version: u32,
  pub(super) order: u64,
  pub(super) body: Option<Rc<dyn Computation>>,
  pub(crate) owner: OwnerData,
}

pub(super) struct OwnerRecord {
  owner: OwnerData,
}

/// What removing a signal leaves to finish once the graph is released.
pub(crate) struct RemovedSignal {
  pub(crate) value: Rc<dyn Any>,
  pub(crate) snapshotted: bool,
}

/// What removing an owner or computation leaves to finish once the graph is released.
pub(crate) struct RemovedNode {
  pub(crate) children: Option<Box<Children>>,
  pub(crate) cleanups: Vec<Box<dyn FnOnce()>>,
  pub(crate) contexts: Option<Box<Contexts>>,
  pub(crate) tasks: Option<Box<Tasks>>,
  pub(crate) body: Option<Rc<dyn Computation>>,
}

/// What a computation's last run made, taken before it runs again.
pub(crate) struct RunRemains {
  pub(crate) children: Option<Box<Children>>,
  pub(crate) cleanups: Vec<Box<dyn FnOnce()>>,
  pub(crate) contexts: Option<Box<Contexts>>,
  pub(crate) tasks: Option<Box<Tasks>>,
}

impl Graph {
  #[inline(always)]
  fn slot(&self, id: Id, kind: Kind) -> Option<(u16, u32)> {
    if id.kind() != kind || id.nonce() != self.nonce {
      return None;
    }

    Some((id.generation(), id.index()))
  }

  #[inline(always)]
  pub(crate) fn signal(&self, id: Id) -> Option<&SignalRecord> {
    let (generation, index) = self.slot(id, Kind::Signal)?;

    self.signals.get(generation, index)
  }

  #[inline(always)]
  pub(super) fn signal_mut(&mut self, id: Id) -> Option<&mut SignalRecord> {
    let (generation, index) = self.slot(id, Kind::Signal)?;

    self.signals.get_mut(generation, index)
  }

  #[inline(always)]
  pub(crate) fn computation(&self, id: Id) -> Option<&ComputationRecord> {
    let (generation, index) = self.slot(id, Kind::Computation)?;

    self.computations.get(generation, index)
  }

  #[inline(always)]
  pub(super) fn computation_mut(&mut self, id: Id) -> Option<&mut ComputationRecord> {
    let (generation, index) = self.slot(id, Kind::Computation)?;

    self.computations.get_mut(generation, index)
  }

  /// The ownership facet of an owner or computation.
  pub(crate) fn owner_data(&self, id: Id) -> Option<&OwnerData> {
    match id.kind() {
      Kind::Computation => self.computation(id).map(|record| &record.owner),
      Kind::Owner => {
        let (generation, index) = self.slot(id, Kind::Owner)?;

        self
          .owners
          .get(generation, index)
          .map(|record| &record.owner)
      }
      Kind::Signal => None,
    }
  }

  /// The ownership facet of an owner or computation, mutably.
  pub(crate) fn owner_data_mut(&mut self, id: Id) -> Option<&mut OwnerData> {
    match id.kind() {
      Kind::Computation => self.computation_mut(id).map(|record| &mut record.owner),
      Kind::Owner => {
        let (generation, index) = self.slot(id, Kind::Owner)?;

        self
          .owners
          .get_mut(generation, index)
          .map(|record| &mut record.owner)
      }
      Kind::Signal => None,
    }
  }

  pub(crate) fn allocate_signal(&mut self, value: Rc<dyn Any>) -> Id {
    let (generation, index) = self.signals.allocate(SignalRecord {
      value,
      parent: None,
      subs: LinkList::EMPTY,
      version: 0,
      snapshotted: false,
      #[cfg(debug_assertions)]
      created_at: None,
    });

    Id::local(Kind::Signal, self.nonce, generation, index)
  }

  pub(crate) fn allocate_owner(&mut self) -> Id {
    let (generation, index) = self.owners.allocate(OwnerRecord {
      owner: OwnerData::default(),
    });

    Id::local(Kind::Owner, self.nonce, generation, index)
  }

  pub(crate) fn allocate_computation(&mut self, body: Rc<dyn Computation>, flags: u8) -> Id {
    let order = self.next_order;

    self.next_order += 1;

    let (generation, index) = self.computations.allocate(ComputationRecord {
      flags,
      deps: LinkList::EMPTY,
      subs: LinkList::EMPTY,
      version: 0,
      order,
      body: Some(body),
      owner: OwnerData::default(),
    });

    Id::local(Kind::Computation, self.nonce, generation, index)
  }

  /// Records where the signal `id` was created, for the obituary a debug build keeps.
  #[cfg(debug_assertions)]
  pub(crate) fn note_creation(&mut self, id: Id, site: &'static Location<'static>) {
    if let Some(signal) = self.signal_mut(id) {
      signal.created_at = Some(site);
    }
  }

  /// What a debug build remembers of the disposed signal `id`.
  #[cfg(debug_assertions)]
  pub(crate) fn obituary(&self, id: Id) -> Option<Obituary> {
    self.obituaries.get(id)
  }

  /// Marks a signal as registered in the state snapshot.
  pub(crate) fn mark_snapshotted(&mut self, id: Id) {
    if let Some(signal) = self.signal_mut(id) {
      signal.snapshotted = true;
    }
  }

  pub(crate) fn flags(&self, id: Id) -> Option<u8> {
    self.computation(id).map(|computation| computation.flags)
  }

  /// Sets then clears bits on a computation's flags, returning the flags before.
  pub(crate) fn update_flags(&mut self, id: Id, set: u8, clear: u8) -> Option<u8> {
    let computation = self.computation_mut(id)?;
    let previous = computation.flags;

    computation.flags = (previous | set) & !clear;

    Some(previous)
  }

  pub(crate) fn body(&self, id: Id) -> Option<Rc<dyn Computation>> {
    self.computation(id)?.body.clone()
  }

  /// Removes a local signal, unlinking it from every subscriber.
  pub(crate) fn remove_signal(&mut self, id: Id) -> Option<RemovedSignal> {
    self.orphan(id);
    self.unobserved_hooks.remove(&id);

    let (generation, index) = self.slot(id, Kind::Signal)?;
    let record = self.signals.remove(generation, index)?;

    self.unlink_subs(record.subs);

    #[cfg(debug_assertions)]
    self.obituaries.record(
      id,
      Obituary {
        created_at: record.created_at,
        disposed_at: Disposing::site(),
      },
    );

    Some(RemovedSignal {
      value: record.value,
      snapshotted: record.snapshotted,
    })
  }

  /// Removes an owner or computation, unlinking every edge it sits on.
  pub(crate) fn remove_node(&mut self, id: Id) -> Option<RemovedNode> {
    self.orphan(id);

    match id.kind() {
      Kind::Owner => {
        let (generation, index) = self.slot(id, Kind::Owner)?;
        let record = self.owners.remove(generation, index)?;

        Some(RemovedNode {
          children: record.owner.children,
          cleanups: record.owner.cleanups,
          contexts: record.owner.contexts,
          tasks: record.owner.tasks,
          body: None,
        })
      }
      Kind::Computation => {
        let (generation, index) = self.slot(id, Kind::Computation)?;
        let record = self.computations.remove(generation, index)?;

        self.unlink_deps(record.deps);
        self.unlink_subs(record.subs);

        Some(RemovedNode {
          children: record.owner.children,
          cleanups: record.owner.cleanups,
          contexts: record.owner.contexts,
          tasks: record.owner.tasks,
          body: record.body,
        })
      }
      Kind::Signal => None,
    }
  }
}
