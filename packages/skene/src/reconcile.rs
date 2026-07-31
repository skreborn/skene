//! Keyed reconciliation, shared across every platform.
//!
//! The tree mutations resolve through the active driver, which the
//! target selects at compile time, so this is written once and monomorphizes onto whichever
//! platform the build targets. The minimal-reorder decision itself is platform-agnostic and
//! lives in the spine ([`Reorder`]).

use std::{
  cell::{Cell, RefCell},
  hash::Hash,
  rc::Rc,
};

use rustc_hash::FxHashMap;
use skene_core::{NodeId, Reorder, View};
use skene_reactive::{Effect, Scope};

use crate::driver as tree;

struct Mounted {
  node: NodeId,
  scope: Scope,
}

struct Scratch<K> {
  old: FxHashMap<K, (usize, Mounted)>,
  old_positions: Vec<usize>,
  new_positions: Vec<usize>,
  reorder: Reorder,
}

impl<K> Default for Scratch<K> {
  fn default() -> Scratch<K> {
    Scratch {
      old: FxHashMap::default(),
      old_positions: Vec::new(),
      new_positions: Vec::new(),
      reorder: Reorder::default(),
    }
  }
}

/// Reactively reconciles a parent's children from a keyed collection.
///
/// `items` yields the items, `key` gives each a stable identity, `build` renders one to a
/// node. Each build runs in its own reactive child scope, disposed when that item leaves.
/// Reorder is minimal: kept children on the longest increasing run of previous positions
/// are left in place, and only displaced or new children are re-inserted before their
/// successor. A platform folds any per-child preparation into `build`.
///
/// Only `items` is tracked: it alone decides when the list is diffed again. `build` runs
/// untracked, so an item that reads a signal while building neither re-runs this diff nor
/// rebuilds itself — express an item's own reactivity with a `derive_` property or a nested
/// [`DynamicView`], which set up their own effects.
pub fn reconcile<T, I, K>(
  parent: NodeId,
  items: impl Fn() -> I + 'static,
  key: impl Fn(&T) -> K + 'static,
  build: impl Fn(T) -> NodeId + 'static,
) where
  I: IntoIterator<Item = T>,
  K: Eq + Hash + 'static,
{
  let parent_scope = Scope::current();
  let scratch = RefCell::new(Scratch::<K>::default());

  Effect::new(move |prev: Option<Vec<(K, Mounted)>>| {
    let mut mounted = prev.unwrap_or_default();
    let mut scratch = scratch.borrow_mut();

    let Scratch {
      old,
      old_positions,
      new_positions,
      reorder,
    } = &mut *scratch;

    for (index, (key, entry)) in mounted.drain(..).enumerate() {
      old.insert(key, (index, entry));
    }

    old_positions.clear();
    new_positions.clear();

    for item in items() {
      let key = key(&item);

      let entry = if let Some((index, entry)) = old.remove(&key) {
        old_positions.push(index);
        new_positions.push(mounted.len());

        entry
      }
      else {
        let scope = parent_scope.create_child();
        // Untracked: an item's build is one-shot, so a signal it happens to read must not
        // subscribe *this* effect and re-run the whole list diff. Reactivity inside an item
        // is its own — a `derive_` property or a nested `DynamicView` — and those set up
        // their own effects, which track normally.
        let node = scope.enter(|| Effect::untrack(|| build(item)));

        Mounted { node, scope }
      };

      mounted.push((key, entry));
    }

    for (_, (_, entry)) in old.drain() {
      tree::detach(parent, entry.node);
      tree::destroy(entry.node);

      entry.scope.dispose();
    }

    let settled = reorder.settled_positions(old_positions.as_slice(), new_positions.as_slice());
    let mut stable = settled.iter().copied().peekable();

    for i in (0..mounted.len()).rev() {
      let before = mounted.get(i + 1).map(|(_, entry)| entry.node);

      if stable.next_if_eq(&i).is_none() {
        tree::attach(parent, mounted[i].1.node, before);
      }
    }

    drop(stable);
    drop(scratch);

    mounted
  });
}

/// Anchors a reactive subtree under a fresh group node, returning that node.
///
/// `build` reruns whenever its reactive dependencies change; the previous subtree is
/// removed, destroyed, and its scope disposed before the new one is inserted.
pub fn dynamic(build: impl Fn() -> NodeId + 'static) -> NodeId {
  let container = tree::create_group();

  let parent_scope = Scope::current();
  let current = Rc::new(Cell::new(None::<(NodeId, Scope)>));

  Effect::new(move |_| {
    let child_scope = parent_scope.create_child();
    let new_node = child_scope.enter(&build);
    let previous = current.take();

    tree::attach(container, new_node, None);

    if let Some((old_node, _)) = &previous {
      tree::detach(container, *old_node);
      tree::destroy(*old_node);
    }

    if let Some((_, old_scope)) = previous {
      old_scope.dispose();
    }

    current.set(Some((new_node, child_scope)));
  });

  container
}

/// A reactive subtree.
#[must_use]
pub struct DynamicView {
  build: Box<dyn Fn() -> NodeId>,
}

impl DynamicView {
  /// Creates a reactive subtree.
  #[inline(always)]
  pub fn new<V: View>(build: impl Fn() -> V + 'static) -> Self {
    Self {
      build: Box::new(move || build().build()),
    }
  }
}

impl View for DynamicView {
  fn build(self) -> NodeId {
    dynamic(self.build)
  }
}
