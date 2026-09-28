//! The per-thread reactive graph: the three arenas, the link pool, and the run queues.
//!
//! Every method here is borrow-only: none runs user code, so a caller holds the graph
//! for one call and releases it before invoking anything of the user's.

use std::{
  any::{Any, TypeId},
  cmp::Reverse,
  collections::{BinaryHeap, VecDeque},
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::{
  arena::Arena,
  computation::Computation,
  id::{Id, Kind, next_nonce},
  link::{Link, LinkIdx, LinkList, LinkPool, NONE},
  path::{Key, Step},
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
}

use flags::{DIRTY, MEMO, PAUSED, PENDING, QUEUED, RENDER, RUNNING};

/// Context values stored on one owner.
pub(crate) type Contexts = FxHashMap<TypeId, Rc<dyn Any>>;

/// The ownership facet owners and computations share.
#[derive(Default)]
pub(crate) struct OwnerData {
  pub(crate) parent: Option<Id>,
  pub(crate) depth: u32,
  pub(crate) children: Option<Box<FxHashSet<Id>>>,
  pub(crate) contexts: Option<Box<Contexts>>,
  pub(crate) cleanups: Vec<Box<dyn FnOnce()>>,
  /// The step from the parent, or from the runtime root for a detached owner.
  step: Option<Step>,
  /// The next positional owner child's ordinal.
  next_child: u32,
  /// The next index in the snapshot sequence.
  next_index: u32,
  /// The path of steps from the runtime root, cached once computed.
  path: Option<Rc<[Step]>>,
  /// The key the next child adopted takes instead of a position.
  pending_key: Option<Key>,
}

/// A local signal: its value cell and its subscriber list.
pub(crate) struct SignalRecord {
  pub(crate) value: Rc<dyn Any>,
  parent: Option<Id>,
  subs: LinkList,
  version: u32,
  /// Registered in the state snapshot, to be unregistered on removal.
  snapshotted: bool,
}

/// An effect or memo: its body, state bits, both link lists and its ownership facet.
pub(crate) struct ComputationRecord {
  pub(crate) flags: u8,
  deps: LinkList,
  subs: LinkList,
  version: u32,
  order: u64,
  body: Option<Rc<dyn Computation>>,
  pub(crate) owner: OwnerData,
}

struct OwnerRecord {
  owner: OwnerData,
}

/// A sync signal's subscribers on this thread, and the version they last saw.
struct SyncSubs {
  list: LinkList,
  version: u32,
}

/// What removing a signal leaves to finish once the graph is released.
pub(crate) struct RemovedSignal {
  pub(crate) value: Rc<dyn Any>,
  pub(crate) snapshotted: bool,
}

/// What removing an owner or computation leaves to finish once the graph is released.
pub(crate) struct RemovedNode {
  pub(crate) children: Option<Box<FxHashSet<Id>>>,
  pub(crate) cleanups: Vec<Box<dyn FnOnce()>>,
  pub(crate) contexts: Option<Box<Contexts>>,
  pub(crate) body: Option<Rc<dyn Computation>>,
}

/// What a computation's last run made, taken before it runs again.
pub(crate) struct RunRemains {
  pub(crate) children: Option<Box<FxHashSet<Id>>>,
  pub(crate) cleanups: Vec<Box<dyn FnOnce()>>,
  pub(crate) contexts: Option<Box<Contexts>>,
}

/// One thread's reactive graph.
pub(crate) struct Graph {
  nonce: u16,
  signals: Arena<SignalRecord>,
  computations: Arena<ComputationRecord>,
  owners: Arena<OwnerRecord>,
  links: LinkPool,
  sync_subs: FxHashMap<Id, SyncSubs>,
  next_order: u64,
  /// The next detached root's ordinal.
  next_root: u32,
  render_queue: BinaryHeap<Reverse<(u32, u64, Id)>>,
  queue: VecDeque<Id>,
}

impl Graph {
  pub(crate) fn new() -> Self {
    Self {
      nonce: next_nonce(),
      signals: Arena::new(),
      computations: Arena::new(),
      owners: Arena::new(),
      links: LinkPool::new(),
      sync_subs: FxHashMap::default(),
      next_order: 0,
      next_root: 0,
      render_queue: BinaryHeap::new(),
      queue: VecDeque::new(),
    }
  }

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
  fn signal_mut(&mut self, id: Id) -> Option<&mut SignalRecord> {
    let (generation, index) = self.slot(id, Kind::Signal)?;

    self.signals.get_mut(generation, index)
  }

  #[inline(always)]
  pub(crate) fn computation(&self, id: Id) -> Option<&ComputationRecord> {
    let (generation, index) = self.slot(id, Kind::Computation)?;

    self.computations.get(generation, index)
  }

  #[inline(always)]
  fn computation_mut(&mut self, id: Id) -> Option<&mut ComputationRecord> {
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
      Kind::Signal | Kind::Sync => None,
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
      Kind::Signal | Kind::Sync => None,
    }
  }

  pub(crate) fn allocate_signal(&mut self, value: Rc<dyn Any>) -> Id {
    let (generation, index) = self.signals.allocate(SignalRecord {
      value,
      parent: None,
      subs: LinkList::EMPTY,
      version: 0,
      snapshotted: false,
    });

    Id::local(Kind::Signal, self.nonce, generation, index)
  }

  /// Allocates the runtime's root owner, whose path is empty.
  pub(crate) fn allocate_root_owner(&mut self) -> Id {
    let id = self.allocate_owner();

    if let Some(data) = self.owner_data_mut(id) {
      data.path = Some(Rc::from([]));
    }

    id
  }

  /// Gives a detached owner its step among the detached roots, unless it has one.
  pub(crate) fn assign_root_ordinal(&mut self, id: Id) {
    let next = self.next_root;

    if let Some(data) = self.owner_data_mut(id)
      && data.step.is_none()
    {
      data.step = Some(Step::position(next));
      self.next_root += 1;
    }
  }

  /// Makes the next child `parent` adopts take `key` as its step instead of a
  /// position.
  pub(crate) fn key_next_child(&mut self, parent: Id, key: Key) {
    if let Some(data) = self.owner_data_mut(parent) {
      data.pending_key = Some(key);
    }
  }

  /// Allocates an owner in `previous`'s place: the same parent, depth and step, so its
  /// path is `previous`'s path and no sibling's position moves.
  pub(crate) fn allocate_replacement(&mut self, previous: Id) -> Id {
    let Some((parent, depth, step, path)) = self
      .owner_data(previous)
      .map(|data| (data.parent, data.depth, data.step, data.path.clone()))
    else {
      return self.allocate_owner();
    };
    let id = self.allocate_owner();

    if let Some(parent) = parent
      && let Some(parent_data) = self.owner_data_mut(parent)
    {
      parent_data.children.get_or_insert_default().insert(id);

      if let Some(data) = self.owner_data_mut(id) {
        data.parent = Some(parent);
      }
    }

    if let Some(data) = self.owner_data_mut(id) {
      data.depth = depth;
      data.step = step;
      data.path = path;
    }

    id
  }

  /// The path of steps from the runtime root to the owner or computation `id`,
  /// computed once and cached; a detached owner is a root of its own.
  pub(crate) fn owner_path(&mut self, id: Id) -> Rc<[Step]> {
    let Some(data) = self.owner_data(id)
    else {
      return Rc::from([]);
    };

    if let Some(path) = &data.path {
      return path.clone();
    }

    let parent = data.parent;
    let path: Rc<[Step]> = match parent {
      Some(parent) => {
        let prefix = self.owner_path(parent);
        let step = self.step_of(id);

        prefix.iter().copied().chain([step]).collect()
      }
      None => {
        self.assign_root_ordinal(id);

        Rc::from([self.step_of(id)])
      }
    };

    if let Some(data) = self.owner_data_mut(id) {
      data.path = Some(path.clone());
    }

    path
  }

  /// The step from `id`'s parent, or the first position for an owner without one.
  fn step_of(&self, id: Id) -> Step {
    self
      .owner_data(id)
      .and_then(|data| data.step)
      .unwrap_or(Step::position(0))
  }

  /// Takes the next snapshot sequence index of the owner or computation `id`.
  pub(crate) fn next_index(&mut self, id: Id) -> u32 {
    let Some(data) = self.owner_data_mut(id)
    else {
      return 0;
    };
    let index = data.next_index;

    data.next_index += 1;

    index
  }

  /// Rewinds `id`'s child ordinals and snapshot indexes, for a computation about to
  /// re-run after disposing its children.
  pub(crate) fn reset_creation_counters(&mut self, id: Id) {
    if let Some(data) = self.owner_data_mut(id) {
      data.next_child = 0;
      data.next_index = 0;
    }
  }

  /// Marks a signal as registered in the state snapshot.
  pub(crate) fn mark_snapshotted(&mut self, id: Id) {
    if let Some(signal) = self.signal_mut(id) {
      signal.snapshotted = true;
    }
  }

  /// Drops the cached paths of `id`'s owner subtree, after `id` moved.
  fn clear_paths(&mut self, id: Id) {
    let Some(data) = self.owner_data_mut(id)
    else {
      return;
    };

    data.path = None;

    let children: SmallVec<[Id; 8]> = data
      .children
      .as_ref()
      .map(|children| children.iter().copied().collect())
      .unwrap_or_default();

    for child in children {
      self.clear_paths(child);
    }
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

  /// Makes `child` a child of `parent`, unlinking it from a previous parent first.
  /// Does nothing when `parent` is gone.
  pub(crate) fn adopt(&mut self, child: Id, parent: Id) {
    let moved = self.orphan(child);

    let Some(parent_data) = self.owner_data_mut(parent)
    else {
      return;
    };

    let depth = parent_data.depth + 1;

    parent_data.children.get_or_insert_default().insert(child);

    match child.kind() {
      Kind::Signal => {
        if let Some(signal) = self.signal_mut(child) {
          signal.parent = Some(parent);
        }
      }
      Kind::Computation | Kind::Owner => {
        let step = match parent_data.pending_key.take() {
          Some(key) => Step::key(key),
          None => {
            let ordinal = parent_data.next_child;

            parent_data.next_child += 1;

            Step::position(ordinal)
          }
        };

        if let Some(data) = self.owner_data_mut(child) {
          data.parent = Some(parent);
          data.depth = depth;
          data.step = Some(step);
        }

        if moved {
          self.clear_paths(child);
        }
      }
      Kind::Sync => {}
    }
  }

  /// Removes `child` from its parent's children, keeping `child` itself; `true` when
  /// it had a parent.
  pub(crate) fn orphan(&mut self, child: Id) -> bool {
    let parent = match child.kind() {
      Kind::Signal => self
        .signal_mut(child)
        .and_then(|signal| signal.parent.take()),
      Kind::Computation | Kind::Owner => self
        .owner_data_mut(child)
        .and_then(|data| data.parent.take()),
      Kind::Sync => None,
    };

    if let Some(parent) = parent {
      self.detach_child(parent, child);

      return true;
    }

    false
  }

  /// Removes `child` from `parent`'s children.
  pub(crate) fn detach_child(&mut self, parent: Id, child: Id) {
    if let Some(data) = self.owner_data_mut(parent)
      && let Some(children) = data.children.as_mut()
    {
      children.remove(&child);
    }
  }

  /// The parent of an owner or computation.
  pub(crate) fn parent(&self, id: Id) -> Option<Id> {
    self.owner_data(id).and_then(|data| data.parent)
  }

  /// Takes what `id`'s last run made—its children, cleanups and contexts—leaving it
  /// as it was before that run.
  pub(crate) fn take_run_remains(&mut self, id: Id) -> Option<RunRemains> {
    let data = self.owner_data_mut(id)?;

    Some(RunRemains {
      children: data.children.take(),
      cleanups: std::mem::take(&mut data.cleanups),
      contexts: data.contexts.take(),
    })
  }

  /// Removes a local signal, unlinking it from every subscriber.
  pub(crate) fn remove_signal(&mut self, id: Id) -> Option<RemovedSignal> {
    self.orphan(id);

    let (generation, index) = self.slot(id, Kind::Signal)?;
    let record = self.signals.remove(generation, index)?;

    self.unlink_subs(record.subs);

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
          body: record.body,
        })
      }
      Kind::Signal | Kind::Sync => None,
    }
  }

  /// Drops this thread's subscriber list of a sync signal, unlinking every subscriber.
  pub(crate) fn remove_sync_subs(&mut self, id: Id) {
    if let Some(entry) = self.sync_subs.remove(&id) {
      self.unlink_subs(entry.list);
    }
  }

  /// The subscriber list of a dep, when the dep is known here.
  fn subs_mut(&mut self, dep: Id) -> Option<&mut LinkList> {
    match dep.kind() {
      Kind::Signal => self.signal_mut(dep).map(|signal| &mut signal.subs),
      Kind::Computation => self.computation_mut(dep).map(|memo| &mut memo.subs),
      Kind::Sync => self.sync_subs.get_mut(&dep).map(|entry| &mut entry.list),
      Kind::Owner => None,
    }
  }

  /// The subscriber list of a dep, created for a sync signal on first use.
  fn subs_for_link(&mut self, dep: Id) -> Option<&mut LinkList> {
    if dep.is_sync() {
      return Some(
        &mut self
          .sync_subs
          .entry(dep)
          .or_insert(SyncSubs {
            list: LinkList::EMPTY,
            version: 0,
          })
          .list,
      );
    }

    self.subs_mut(dep)
  }

  /// The version a dep is at: bumped on every value change.
  pub(crate) fn dep_version(&self, dep: Id) -> Option<u32> {
    match dep.kind() {
      Kind::Signal => self.signal(dep).map(|signal| signal.version),
      Kind::Computation => self.computation(dep).map(|memo| memo.version),
      Kind::Sync => Some(self.sync_subs.get(&dep).map_or(0, |entry| entry.version)),
      Kind::Owner => None,
    }
  }

  /// Records that `sub` read `dep`, reusing the edge from the previous run when it
  /// comes next in read order.
  pub(crate) fn link(&mut self, dep: Id, sub: Id) {
    let Some(version) = self.dep_version(dep)
    else {
      return;
    };

    let Some(computation) = self.computation(sub)
    else {
      return;
    };

    let tail = computation.deps.tail;
    let head = computation.deps.head;
    let next = if tail == NONE {
      head
    }
    else {
      self.links.get(tail).next_dep
    };

    if next != NONE {
      let candidate = self.links.get_mut(next);

      if candidate.dep == dep {
        candidate.version = version;
        self.computation_mut(sub).expect("checked above").deps.tail = next;

        return;
      }
    }

    if tail != NONE && self.links.get(tail).dep == dep {
      return;
    }

    let Some(subs) = self.subs_for_link(dep)
    else {
      return;
    };

    let prev_sub = subs.tail;
    let link = self.links.allocate(Link {
      dep,
      sub,
      prev_dep: tail,
      next_dep: next,
      prev_sub,
      next_sub: NONE,
      version,
    });

    if next != NONE {
      self.links.get_mut(next).prev_dep = link;
    }

    if tail != NONE {
      self.links.get_mut(tail).next_dep = link;
    }

    {
      let deps = &mut self.computation_mut(sub).expect("checked above").deps;

      if tail == NONE {
        deps.head = link;
      }

      deps.tail = link;
    }

    if prev_sub != NONE {
      self.links.get_mut(prev_sub).next_sub = link;
    }

    let subs = self.subs_for_link(dep).expect("checked above");

    if prev_sub == NONE {
      subs.head = link;
    }

    subs.tail = link;
  }

  /// Opens a run of `sub`: clears its invalidation, marks it running and rewinds its
  /// dependency cursor so [`link`](Self::link) can reuse last run's edges in order.
  pub(crate) fn start_tracking(&mut self, sub: Id) {
    if let Some(computation) = self.computation_mut(sub) {
      computation.deps.tail = NONE;
      computation.flags = (computation.flags & !(DIRTY | PENDING)) | RUNNING;
    }
  }

  /// Closes a run of `sub`: drops every edge it did not read this time.
  pub(crate) fn end_tracking(&mut self, sub: Id) {
    let Some(computation) = self.computation(sub)
    else {
      return;
    };

    let tail = computation.deps.tail;
    let mut stale = if tail == NONE {
      computation.deps.head
    }
    else {
      self.links.get(tail).next_dep
    };

    while stale != NONE {
      let next = self.links.get(stale).next_dep;

      self.unlink(stale);
      stale = next;
    }

    if let Some(computation) = self.computation_mut(sub) {
      computation.flags &= !RUNNING;
    }
  }

  /// Removes one edge from both of its lists.
  fn unlink(&mut self, index: LinkIdx) {
    let Link {
      dep,
      sub,
      prev_dep,
      next_dep,
      prev_sub,
      next_sub,
      ..
    } = self.links.get(index);

    if prev_sub != NONE {
      self.links.get_mut(prev_sub).next_sub = next_sub;
    }
    else if let Some(subs) = self.subs_mut(dep) {
      subs.head = next_sub;
    }

    if next_sub != NONE {
      self.links.get_mut(next_sub).prev_sub = prev_sub;
    }
    else if let Some(subs) = self.subs_mut(dep) {
      subs.tail = prev_sub;
    }

    if prev_dep != NONE {
      self.links.get_mut(prev_dep).next_dep = next_dep;
    }
    else if let Some(computation) = self.computation_mut(sub) {
      computation.deps.head = next_dep;
    }

    if next_dep != NONE {
      self.links.get_mut(next_dep).prev_dep = prev_dep;
    }

    if let Some(computation) = self.computation_mut(sub)
      && computation.deps.tail == index
    {
      computation.deps.tail = prev_dep;
    }

    self.links.free(index);
  }

  /// Unlinks every edge on a subscriber list.
  fn unlink_subs(&mut self, list: LinkList) {
    let mut index = list.head;

    while index != NONE {
      let next = self.links.get(index).next_sub;

      self.unlink(index);
      index = next;
    }
  }

  /// Unlinks every edge on a dependency list.
  fn unlink_deps(&mut self, list: LinkList) {
    let mut index = list.head;

    while index != NONE {
      let next = self.links.get(index).next_dep;

      self.unlink(index);
      index = next;
    }
  }

  /// The first edge of `sub`'s dependency list.
  pub(crate) fn first_dep(&self, sub: Id) -> LinkIdx {
    self
      .computation(sub)
      .map_or(NONE, |computation| computation.deps.head)
  }

  /// The edge at `index`; the caller checks it still names the pair it expects.
  #[inline(always)]
  pub(crate) fn link_at(&self, index: LinkIdx) -> Link {
    self.links.get(index)
  }

  /// A source changed: direct subscribers become dirty, memos downstream of them
  /// pending, and every affected effect joins its queue.
  pub(crate) fn propagate(&mut self, source: Id) {
    let head = match source.kind() {
      Kind::Signal => {
        let Some(signal) = self.signal_mut(source)
        else {
          return;
        };

        signal.version = signal.version.wrapping_add(1);
        signal.subs.head
      }
      Kind::Sync => {
        let Some(entry) = self.sync_subs.get_mut(&source)
        else {
          return;
        };

        entry.version = entry.version.wrapping_add(1);
        entry.list.head
      }
      Kind::Computation | Kind::Owner => return,
    };
    let mut stack: SmallVec<[(LinkIdx, bool); 8]> = SmallVec::new();

    stack.push((head, true));

    while let Some((mut index, mut dirty)) = stack.pop() {
      while index != NONE {
        let Link { sub, next_sub, .. } = self.links.get(index);

        index = next_sub;

        let Some(computation) = self.computation_mut(sub)
        else {
          continue;
        };

        let flags = computation.flags;

        if flags & MEMO != 0 {
          if flags & (DIRTY | PENDING) == 0 {
            computation.flags |= if dirty { DIRTY } else { PENDING };

            let downstream = computation.subs.head;

            if downstream != NONE {
              stack.push((index, dirty));
              index = downstream;
              dirty = false;
            }
          }
          else if dirty {
            computation.flags |= DIRTY;
          }

          continue;
        }

        computation.flags |= if dirty { DIRTY } else { PENDING };

        if flags & (QUEUED | PAUSED) == 0 {
          computation.flags |= QUEUED;

          let render = flags & RENDER != 0;
          let depth = computation.owner.depth;
          let order = computation.order;

          self.push_queue(sub, render, depth, order);
        }
      }
    }
  }

  fn push_queue(&mut self, id: Id, render: bool, depth: u32, order: u64) {
    if render {
      self.render_queue.push(Reverse((depth, order, id)));
    }
    else {
      self.queue.push_back(id);
    }
  }

  /// Queues an invalidated effect that is not queued yet; `false` when nothing is due.
  pub(crate) fn enqueue(&mut self, id: Id) -> bool {
    let Some(computation) = self.computation_mut(id)
    else {
      return false;
    };

    let flags = computation.flags;

    if flags & (DIRTY | PENDING) == 0 || flags & (QUEUED | MEMO) != 0 {
      return false;
    }

    computation.flags |= QUEUED;

    let render = flags & RENDER != 0;
    let depth = computation.owner.depth;
    let order = computation.order;

    self.push_queue(id, render, depth, order);

    true
  }

  /// The next effect due: render effects first, by owner depth then creation, then
  /// ordinary effects in propagation order.
  pub(crate) fn pop_queued(&mut self) -> Option<Id> {
    if let Some(Reverse((_, _, id))) = self.render_queue.pop() {
      return Some(id);
    }

    self.queue.pop_front()
  }

  pub(crate) fn has_queued(&self) -> bool {
    !self.render_queue.is_empty() || !self.queue.is_empty()
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

  /// Records that a memo's value changed.
  pub(crate) fn bump_version(&mut self, id: Id) {
    if let Some(computation) = self.computation_mut(id) {
      computation.version = computation.version.wrapping_add(1);
    }
  }
}
