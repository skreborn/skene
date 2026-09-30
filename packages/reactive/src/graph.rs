//! The per-thread reactive graph: the three arenas, the link pool, and the run queues.
//!
//! Every method of the graph is borrow-only: none runs user code, so a caller holds the
//! graph for one call and releases it before invoking anything of the user's.

mod edges;
mod ownership;
mod propagation;
mod records;

use std::{
  cmp::Reverse,
  collections::{BinaryHeap, VecDeque},
  rc::Rc,
};

use rustc_hash::FxHashMap;

use self::records::{ComputationRecord, OwnerRecord, SignalRecord};
pub(crate) use self::{
  edges::CheckStep,
  records::{RunRemains, flags},
};
#[cfg(debug_assertions)]
use crate::obituary::Obituaries;
use crate::{
  arena::Arena,
  id::{Id, next_nonce},
  link::LinkPool,
};

/// One thread's reactive graph.
pub(crate) struct Graph {
  /// The graph's own mark on every id it mints, telling them from another thread's.
  nonce: u32,
  /// The local signals.
  signals: Arena<SignalRecord>,
  /// The effects and memos.
  computations: Arena<ComputationRecord>,
  /// The owners that are not computations.
  owners: Arena<OwnerRecord>,
  /// The dependency links between the computations and what they read.
  links: LinkPool,
  /// The creation order the next computation takes.
  next_order: u64,
  /// The next detached root's ordinal.
  next_root: u32,
  /// The queued render effects, by owner depth, then creation order.
  render_queue: BinaryHeap<Reverse<(u32, u64, Id)>>,
  /// The queued ordinary effects, in propagation order.
  queue: VecDeque<Id>,
  /// The hooks run once their signal has lost its last subscriber.
  unobserved_hooks: FxHashMap<Id, Rc<dyn Fn()>>,
  /// The hooked signals that lost their last subscriber, their hooks not yet run.
  unobserved: Vec<Id>,
  /// The next task's id.
  next_task: u64,
  /// What a debug build remembers of the recently disposed signals.
  #[cfg(debug_assertions)]
  obituaries: Obituaries,
}

impl Graph {
  pub(crate) fn new() -> Self {
    Self {
      nonce: next_nonce(),
      signals: Arena::new(),
      computations: Arena::new(),
      owners: Arena::new(),
      links: LinkPool::new(),
      next_order: 0,
      next_root: 0,
      render_queue: BinaryHeap::new(),
      queue: VecDeque::new(),
      unobserved_hooks: FxHashMap::default(),
      unobserved: Vec::new(),
      next_task: 0,
      #[cfg(debug_assertions)]
      obituaries: Obituaries::default(),
    }
  }
}
