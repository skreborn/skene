//! The ownership tree: adoption, depths, snapshot paths, a run's creation counters and
//! the tasks an owner runs.

use std::rc::Rc;

use smallvec::SmallVec;

use super::{
  Graph,
  records::{RunRemains, flags::MEMO},
};
use crate::{
  computation::Computation,
  id::{Id, Kind},
  path::{Key, Step},
  spawn::TaskGuard,
};

impl Graph {
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
    let path: Rc<[Step]> = if let Some(parent) = parent {
      let prefix = self.owner_path(parent);
      let step = self.step_of(id);

      prefix.iter().copied().chain([step]).collect()
    }
    else {
      self.assign_root_ordinal(id);

      Rc::from([self.step_of(id)])
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
      .map(|children| children.iter().collect())
      .unwrap_or_default();

    for child in children {
      self.clear_paths(child);
    }
  }

  /// Sets the depth of the owner or computation `id` to `depth`, and its subtree's to
  /// follow.
  fn set_depths(&mut self, id: Id, depth: u32) {
    let Some(data) = self.owner_data_mut(id)
    else {
      return;
    };

    if data.depth == depth {
      return;
    }

    data.depth = depth;

    let children: SmallVec<[Id; 8]> = data
      .children
      .as_ref()
      .map(|children| children.iter().collect())
      .unwrap_or_default();

    for child in children {
      self.set_depths(child, depth + 1);
    }
  }

  /// Makes `child` a child of `parent`, unlinking it from a previous parent first; a
  /// moved subtree takes the depths of its new place. Does nothing when `parent` is
  /// gone.
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
        let step = if let Some(key) = parent_data.pending_key.take() {
          Step::key(key)
        }
        else {
          let ordinal = parent_data.next_child;

          parent_data.next_child += 1;

          Step::position(ordinal)
        };

        if let Some(data) = self.owner_data_mut(child) {
          data.parent = Some(parent);
          data.step = Some(step);
        }

        self.set_depths(child, depth);

        if moved {
          self.clear_paths(child);
        }
      }
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
      children.remove(child);
    }
  }

  /// The parent of an owner or computation.
  pub(crate) fn parent(&self, id: Id) -> Option<Id> {
    self.owner_data(id).and_then(|data| data.parent)
  }

  /// Starts a run of the computation `id`: resets its creation counters and returns its
  /// body, with what its last run made when that was anything.
  pub(crate) fn begin_run(&mut self, id: Id) -> Option<(Rc<dyn Computation>, Option<RunRemains>)> {
    let computation = self.computation_mut(id)?;
    let body = computation.body.clone()?;
    let data = &mut computation.owner;

    data.next_child = 0;
    data.next_index = 0;

    let made = data.children.is_some()
      || !data.cleanups.is_empty()
      || data.contexts.is_some()
      || data.tasks.is_some();
    let remains = made.then(|| RunRemains {
      children: data.children.take(),
      cleanups: std::mem::take(&mut data.cleanups),
      contexts: data.contexts.take(),
      tasks: data.tasks.take(),
    });

    Some((body, remains))
  }

  /// The next task's id.
  pub(crate) const fn next_task(&mut self) -> u64 {
    let id = self.next_task;

    self.next_task += 1;

    id
  }

  /// Adds `guard`, the running task `id`, to the tasks of the owner `owner`; hands the
  /// guard back when the owner is gone, for the caller to cancel the task by dropping
  /// it.
  pub(crate) fn adopt_task(&mut self, owner: Id, id: u64, guard: TaskGuard) -> Option<TaskGuard> {
    let Some(data) = self.owner_data_mut(owner)
    else {
      return Some(guard);
    };

    data.tasks.get_or_insert_default().insert(id, guard);

    None
  }

  /// Takes the task `id` out of the tasks of the owner `owner`, for the caller to drop
  /// once the graph is released; the last one leaves the owner with none.
  pub(crate) fn take_task(&mut self, owner: Id, id: u64) -> Option<TaskGuard> {
    let tasks = self.owner_data_mut(owner)?.tasks.as_mut()?;
    let guard = tasks.remove(&id);

    if tasks.is_empty()
      && let Some(data) = self.owner_data_mut(owner)
    {
      data.tasks = None;
    }

    guard
  }

  /// The effects at or under `id` in the ownership tree.
  pub(crate) fn owned_effects(&self, id: Id) -> Vec<Id> {
    let mut effects = Vec::new();
    let mut stack = vec![id];

    while let Some(current) = stack.pop() {
      if self
        .computation(current)
        .is_some_and(|computation| computation.flags & MEMO == 0)
      {
        effects.push(current);
      }

      if let Some(children) = self
        .owner_data(current)
        .and_then(|data| data.children.as_deref())
      {
        stack.extend(children.iter().filter(|child| child.kind() != Kind::Signal));
      }
    }

    effects
  }
}
