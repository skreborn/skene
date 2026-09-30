//! The dependency edges: linking what a run reads, dropping what it no longer reads, and
//! checking them for change.

use std::rc::Rc;

use super::{
  Graph,
  records::flags::{CHECKING, DIRTY, PENDING, RUNNING},
};
use crate::{
  id::{Id, Kind},
  link::{Link, LinkIdx, LinkList, NONE},
};

/// The step a dependency check takes at one edge.
pub(crate) enum CheckStep {
  /// The dependency changed since the edge read it, or the edge is gone.
  Changed,
  /// The dependency is unchanged; the check goes on at this edge.
  Unchanged(LinkIdx),
  /// The dependency is a dirty memo, to run before it is compared.
  Run(Id),
  /// The dependency is a pending memo, now on the check, whose own edges are checked
  /// first, from this one.
  Descend(Id, LinkIdx),
}

impl Graph {
  /// The subscriber list of a dep, when the dep is known here.
  fn subs_mut(&mut self, dep: Id) -> Option<&mut LinkList> {
    match dep.kind() {
      Kind::Signal => self.signal_mut(dep).map(|signal| &mut signal.subs),
      Kind::Computation => self.computation_mut(dep).map(|memo| &mut memo.subs),
      Kind::Owner => None,
    }
  }

  /// The version a dep is at: bumped on every value change.
  pub(crate) fn dep_version(&self, dep: Id) -> Option<u32> {
    match dep.kind() {
      Kind::Signal => self.signal(dep).map(|signal| signal.version),
      Kind::Computation => self.computation(dep).map(|memo| memo.version),
      Kind::Owner => None,
    }
  }

  /// Records that a memo's value changed.
  pub(crate) fn bump_version(&mut self, id: Id) {
    if let Some(computation) = self.computation_mut(id) {
      computation.version = computation.version.wrapping_add(1);
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

    let Some(subs) = self.subs_mut(dep)
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

    let subs = self.subs_mut(dep).expect("checked above");

    if prev_sub == NONE {
      subs.head = link;
    }

    subs.tail = link;
  }

  /// Opens a run of `sub`: clears its invalidation and any dependency check it is on,
  /// marks it running and rewinds its dependency cursor so [`link`](Self::link) can
  /// reuse last run's edges in order.
  pub(crate) fn start_tracking(&mut self, sub: Id) {
    if let Some(computation) = self.computation_mut(sub) {
      computation.deps.tail = NONE;
      computation.flags = (computation.flags & !(DIRTY | PENDING | CHECKING)) | RUNNING;
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

    if !self.unobserved_hooks.is_empty()
      && self.unobserved_hooks.contains_key(&dep)
      && self.subs_mut(dep).is_some_and(|subs| subs.head == NONE)
    {
      self.unobserved.push(dep);
    }
  }

  /// Unlinks every edge on a subscriber list.
  pub(super) fn unlink_subs(&mut self, list: LinkList) {
    let mut index = list.head;

    while index != NONE {
      let next = self.links.get(index).next_sub;

      self.unlink(index);
      index = next;
    }
  }

  /// Unlinks every edge on a dependency list.
  pub(super) fn unlink_deps(&mut self, list: LinkList) {
    let mut index = list.head;

    while index != NONE {
      let next = self.links.get(index).next_dep;

      self.unlink(index);
      index = next;
    }
  }

  /// Puts the computation `id` on a dependency check, returning its first edge.
  pub(crate) fn begin_check(&mut self, id: Id) -> LinkIdx {
    self.computation_mut(id).map_or(NONE, |computation| {
      computation.flags |= CHECKING;

      computation.deps.head
    })
  }

  /// The step a dependency check of `sub` takes at its edge `index`: a memo on the
  /// check already, or running, is only compared.
  pub(crate) fn check_edge(&mut self, sub: Id, index: LinkIdx) -> CheckStep {
    let link = self.links.get(index);

    if link.sub != sub {
      return CheckStep::Changed;
    }

    if let Some(dep) = self.computation_mut(link.dep)
      && dep.flags & (RUNNING | CHECKING) == 0
    {
      if dep.flags & DIRTY != 0 {
        return CheckStep::Run(link.dep);
      }

      if dep.flags & PENDING != 0 {
        dep.flags |= CHECKING;

        return CheckStep::Descend(link.dep, dep.deps.head);
      }
    }

    self
      .unchanged_edge(sub, index, link.dep)
      .map_or(CheckStep::Changed, CheckStep::Unchanged)
  }

  /// The edge after `sub`'s edge `index` when its dependency `dep` is unchanged since
  /// the edge read it; `None` when it changed or the edge no longer joins the two.
  pub(crate) fn unchanged_edge(&self, sub: Id, index: LinkIdx, dep: Id) -> Option<LinkIdx> {
    let link = self.links.get(index);

    (link.sub == sub
      && link.dep == dep
      && self
        .dep_version(dep)
        .is_some_and(|version| version == link.version))
    .then_some(link.next_dep)
  }

  /// Runs `hook`, once, when the signal `id` loses its last subscriber; see
  /// [`take_unobserved`](Self::take_unobserved).
  pub(crate) fn on_unobserved(&mut self, id: Id, hook: Rc<dyn Fn()>) {
    self.unobserved_hooks.insert(id, hook);
  }

  /// Whether a signal with a hook lost its last subscriber since the hooks were last
  /// taken.
  pub(crate) const fn has_unobserved(&self) -> bool {
    !self.unobserved.is_empty()
  }

  /// Takes the hooks of the signals that lost their last subscriber since the last call
  /// and have not regained one, for the caller to run once the graph is released.
  pub(crate) fn take_unobserved(&mut self) -> Vec<Rc<dyn Fn()>> {
    let pending = std::mem::take(&mut self.unobserved);
    let mut hooks = Vec::new();

    for id in pending {
      if self.subs_mut(id).is_some_and(|subs| subs.head == NONE)
        && let Some(hook) = self.unobserved_hooks.remove(&id)
      {
        hooks.push(hook);
      }
    }

    hooks
  }
}
