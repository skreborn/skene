use std::{
  cell::{Cell, RefCell},
  hash::Hash,
  rc::Rc,
};

use longest_increasing_subsequence::lis_with;
use rustc_hash::FxHashMap;
use skene_reactive::{Effect, Scope};

use crate::{Backend, NodeId, with_backend};

// TODO: Understand and fix up

struct Mounted {
  node: NodeId,
  scope: Scope,
}

struct Scratch<K> {
  old: FxHashMap<K, (usize, Mounted)>,
  old_positions: Vec<usize>,
  new_positions: Vec<usize>,
  subsequence: Vec<usize>,
  predecessors: Vec<usize>,
  starts: Vec<usize>,
}

impl<K> Default for Scratch<K> {
  fn default() -> Scratch<K> {
    Scratch {
      old: FxHashMap::default(),
      old_positions: Vec::new(),
      new_positions: Vec::new(),
      subsequence: Vec::new(),
      predecessors: Vec::new(),
      starts: Vec::new(),
    }
  }
}

/// Reactively reconciles a parent's children from a keyed collection.
///
/// `items` yields the items, `key` gives each a stable identity, `build` renders
/// one to a node. Each build runs in its own reactive child scope, disposed when
/// that item leaves. Reorder is minimal: kept children on the longest increasing
/// run of previous positions are left in place, and only displaced or new children
/// are re-inserted before their successor. A backend folds any per-child
/// preparation into `build`.
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
      subsequence,
      predecessors,
      starts,
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
        let node = scope.enter(|| build(item));

        Mounted { node, scope }
      };

      mounted.push((key, entry));
    }

    for (_, (_, entry)) in old.drain() {
      with_backend(|backend| {
        backend.detach_child(parent, entry.node);
        backend.destroy(entry.node);
      });

      entry.scope.dispose();
    }

    let kept = old_positions.len();

    subsequence.clear();
    predecessors.clear();
    predecessors.resize(kept, 0);
    starts.clear();
    starts.resize(kept, 0);

    lis_with(
      old_positions.as_slice(),
      subsequence,
      |a, b| a < b,
      predecessors.as_mut_slice(),
      starts.as_mut_slice(),
    );

    let mut stable = subsequence
      .iter()
      .map(|index| new_positions[*index])
      .peekable();

    for i in (0..mounted.len()).rev() {
      let before = mounted.get(i + 1).map(|(_, entry)| entry.node);

      if stable.next_if_eq(&i).is_none() {
        with_backend(|backend| {
          backend.attach_child(parent, mounted[i].1.node, before);
        });
      }
    }

    drop(stable);
    drop(scratch);

    mounted
  });
}

/// Anchors a reactive subtree under a fresh group node, returning that node.
///
/// `build` reruns whenever its reactive dependencies change; the previous subtree is removed,
/// destroyed, and its scope disposed before the new one is inserted.
pub fn dynamic(build: impl Fn() -> NodeId + 'static) -> NodeId {
  let container = with_backend(Backend::create_group);

  let parent_scope = Scope::current();
  let current = Rc::new(Cell::new(None::<(NodeId, Scope)>));

  Effect::new(move |_| {
    let child_scope = parent_scope.create_child();
    let new_node = child_scope.enter(&build);
    let previous = current.take();

    with_backend(|backend| {
      backend.attach_child(container, new_node, None);

      if let Some((old_node, _)) = &previous {
        backend.detach_child(container, *old_node);
        backend.destroy(*old_node);
      }
    });

    if let Some((_, old_scope)) = previous {
      old_scope.dispose();
    }

    current.set(Some((new_node, child_scope)));
  });

  container
}

#[cfg(test)]
mod tests {
  use skene_reactive::{Runtime, RwSignal, SignalGet, SignalUpdate};

  use crate::{
    Backend,
    MockBackend,
    MockOp,
    NodeId,
    dynamic,
    install_test_backend,
    reconcile,
    root_scope,
    with_backend,
  };

  #[test]
  fn reset_with_live_effects_does_not_panic() {
    let _backend = install_test_backend(MockBackend::new());

    let signal = RwSignal::new(vec![1, 2, 3]);
    let parent = with_backend(Backend::create_group);

    root_scope().enter(move || {
      reconcile(
        parent,
        move || signal.get(),
        |item| *item,
        |_item| with_backend(Backend::create_group),
      );
    });

    Runtime::reset();
  }

  #[test]
  fn reorder_is_minimal_and_preserves_nodes() {
    let backend = install_test_backend(MockBackend::new());
    let items = RwSignal::new(vec![1, 2, 3]);

    let parent = with_backend(Backend::create_group);

    root_scope().enter({
      move || {
        reconcile(
          parent,
          move || items.get(),
          |item| *item,
          |_| with_backend(Backend::create_group),
        );
      }
    });

    assert_eq!(
      backend.children_of(parent),
      vec![NodeId::new(2), NodeId::new(3), NodeId::new(4)],
    );

    items.set(vec![3, 1, 2]);

    assert_eq!(
      backend.children_of(parent),
      vec![NodeId::new(4), NodeId::new(2), NodeId::new(3)],
    );

    assert_eq!(
      backend.ops(),
      vec![
        MockOp::CreateGroup {
          node: NodeId::new(1),
        },
        MockOp::CreateGroup {
          node: NodeId::new(2),
        },
        MockOp::CreateGroup {
          node: NodeId::new(3),
        },
        MockOp::CreateGroup {
          node: NodeId::new(4),
        },
        MockOp::AttachChild {
          parent: NodeId::new(1),
          child: NodeId::new(4),
          before: None,
        },
        MockOp::AttachChild {
          parent: NodeId::new(1),
          child: NodeId::new(3),
          before: Some(NodeId::new(4)),
        },
        MockOp::AttachChild {
          parent: NodeId::new(1),
          child: NodeId::new(2),
          before: Some(NodeId::new(3)),
        },
        MockOp::AttachChild {
          parent: NodeId::new(1),
          child: NodeId::new(4),
          before: Some(NodeId::new(2)),
        },
      ],
    );
  }

  #[test]
  fn removed_item_is_destroyed() {
    let backend = install_test_backend(MockBackend::new());
    let items = RwSignal::new(vec![1, 2]);

    let parent = with_backend(Backend::create_group);

    root_scope().enter({
      move || {
        reconcile(
          parent,
          move || items.get(),
          |item| *item,
          |_| with_backend(Backend::create_group),
        );
      }
    });

    items.set(vec![1]);

    assert_eq!(backend.children_of(parent), vec![NodeId::new(2)]);

    assert_eq!(
      backend.ops(),
      vec![
        MockOp::CreateGroup {
          node: NodeId::new(1),
        },
        MockOp::CreateGroup {
          node: NodeId::new(2),
        },
        MockOp::CreateGroup {
          node: NodeId::new(3),
        },
        MockOp::AttachChild {
          parent: NodeId::new(1),
          child: NodeId::new(3),
          before: None,
        },
        MockOp::AttachChild {
          parent: NodeId::new(1),
          child: NodeId::new(2),
          before: Some(NodeId::new(3)),
        },
        MockOp::DetachChild {
          parent: NodeId::new(1),
          child: NodeId::new(3),
        },
        MockOp::Destroy {
          node: NodeId::new(3),
        },
      ],
    );
  }

  #[test]
  fn dynamic_swaps_and_destroys() {
    let backend = install_test_backend(MockBackend::new());
    let flag = RwSignal::new(true);

    let anchor = root_scope().enter(move || {
      dynamic(move || {
        flag.get();

        with_backend(Backend::create_group)
      })
    });

    assert_eq!(backend.children_of(anchor), vec![NodeId::new(2)]);

    flag.update(|value| *value = !*value);

    assert_eq!(backend.children_of(anchor), vec![NodeId::new(3)]);

    assert_eq!(
      backend.ops(),
      vec![
        MockOp::CreateGroup {
          node: NodeId::new(1),
        },
        MockOp::CreateGroup {
          node: NodeId::new(2),
        },
        MockOp::AttachChild {
          parent: NodeId::new(1),
          child: NodeId::new(2),
          before: None,
        },
        MockOp::CreateGroup {
          node: NodeId::new(3),
        },
        MockOp::AttachChild {
          parent: NodeId::new(1),
          child: NodeId::new(3),
          before: None,
        },
        MockOp::DetachChild {
          parent: NodeId::new(1),
          child: NodeId::new(2),
        },
        MockOp::Destroy {
          node: NodeId::new(2),
        },
      ],
    );
  }
}
