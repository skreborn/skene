//! Reactive subtrees between anchors: a tracked rebuild, and a keyed reconcile.

use std::{
  cell::{Cell, RefCell},
  hash::Hash,
  rc::Rc,
};

use rustc_hash::FxHashMap;
use skene_reactive::{Effect, Key, Owner, untrack};

use super::{AnyView, NodeId, Platform, Scene, View};
use crate::Reorder;

/// Mounts the node `build` returns between a fresh pair of anchors and returns the
/// start anchor; `build` reruns whenever a signal it read changes.
///
/// Each rebuild destroys the previous content and disposes its scope, so state inside
/// the content does not survive it. The annotation pending when the subtree is created
/// lands on the content of every rebuild.
pub fn derived<P: Platform>(build: impl Fn() -> NodeId + 'static) -> NodeId {
  let (start, annotation) =
    Scene::<P>::with(|scene| (scene.create_segment().0, scene.take_pending_annotation()));
  let parent_scope = Owner::current();
  let current = Rc::new(Cell::new(None::<Owner>));

  Effect::render(move |_: Option<()>| {
    // The rebuilt content takes the previous content's place in the owner tree, so
    // state under it keeps its snapshot identity across the rebuild.
    let scope = current
      .get()
      .map_or_else(|| parent_scope.child(), |previous| previous.replacement());

    let node = match &annotation {
      Some(annotation) => Scene::<P>::annotate(annotation.clone(), || scope.enter(&build)),
      None => scope.enter(&build),
    };

    Scene::<P>::with(|scene| {
      scene.clear_segment(start);
      scene.insert_in_segment(start, node, None);
    });

    if let Some(previous) = current.replace(Some(scope)) {
      previous.dispose();
    }
  });

  start
}

/// A view that renders nothing: an empty segment.
#[inline(always)]
#[must_use]
pub fn empty<P: Platform>() -> impl View<P> {
  AnyView::new(|| Scene::<P>::with(|scene| scene.create_segment().0))
}

struct Mounted {
  node: NodeId,
  scope: Owner,
}

struct Scratch<K> {
  old: FxHashMap<K, (usize, Mounted)>,
  old_positions: Vec<usize>,
  new_positions: Vec<usize>,
  reorder: Reorder,
}

impl<K> Default for Scratch<K> {
  fn default() -> Self {
    Self {
      old: FxHashMap::default(),
      old_positions: Vec::new(),
      new_positions: Vec::new(),
      reorder: Reorder::default(),
    }
  }
}

/// Reactively reconciles the content of the segment `start` begins from a keyed
/// collection.
///
/// `items` yields the items, `key` gives each a stable identity, `build` renders one to
/// a node. Only `items` is tracked. Each build runs untracked in its own scope keyed by
/// the item's key, so the state under it keeps its snapshot identity across a reorder,
/// and disposed when the item leaves; kept items on the longest increasing run of
/// previous positions stay in place, and only displaced or new ones are re-inserted
/// before their successor.
pub fn reconcile<P: Platform, T, I, K>(
  start: NodeId,
  items: impl Fn() -> I + 'static,
  key: impl Fn(&T) -> K + 'static,
  build: impl Fn(T) -> NodeId + 'static,
) where
  I: IntoIterator<Item = T>,
  K: Eq + Hash + 'static,
{
  let parent_scope = Owner::current();
  let scratch = RefCell::new(Scratch::<K>::default());

  Effect::render(move |previous: Option<Vec<(K, Mounted)>>| {
    let mut mounted = previous.unwrap_or_default();
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
        let scope = parent_scope.keyed_child(Key::of(&key));
        let node = scope.enter(|| untrack(|| build(item)));

        Mounted { node, scope }
      };

      mounted.push((key, entry));
    }

    for (_, (_, entry)) in old.drain() {
      Scene::<P>::with(|scene| {
        scene.remove_from_segment(start, entry.node);
        scene.destroy(entry.node);
      });
      entry.scope.dispose();
    }

    let settled = reorder.settled_positions(old_positions.as_slice(), new_positions.as_slice());
    let mut stable = settled.iter().copied().peekable();

    Scene::<P>::with(|scene| {
      for index in (0..mounted.len()).rev() {
        let before = mounted.get(index + 1).map(|(_, entry)| entry.node);

        if stable.next_if_eq(&index).is_none() {
          scene.insert_in_segment(start, mounted[index].1.node, before);
        }
      }
    });

    drop(stable);
    drop(scratch);

    mounted
  });
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use skene_reactive::{Get, Owner, Set, Signal, Track, signal};

  use super::{derived, reconcile};
  use crate::{
    Annotation,
    Flexibility,
    NodeId,
    NodeKind,
    Scene,
    testing::{Call, TestBox, TestPlatform, child_kinds, children, sink},
  };

  fn with<R>(operation: impl FnOnce(&mut Scene<TestPlatform>) -> R) -> R {
    Scene::<TestPlatform>::with(operation)
  }

  /// A derived subtree contributes its content directly to its parent's children,
  /// with nothing between the anchors but the content, and rebuilds in place.
  #[test]
  fn derived_mounts_only_its_content_between_the_anchors() {
    TestPlatform::reset();

    let owner = Owner::new();
    let (version, set_version) = signal(0);
    let built = Rc::new(Cell::new(0));
    let parent = TestBox::node();
    let start = owner.enter(|| {
      let built = Rc::clone(&built);

      derived::<TestPlatform>(move || {
        version.track();
        built.set(built.get() + 1);

        TestBox::node()
      })
    });

    with(|scene| scene.attach(parent, start, None));

    let end = with(|scene| scene.segment_end(start)).unwrap();
    let first = children(parent)[1];

    assert_eq!(children(parent), [start, first, end]);
    assert_eq!(
      child_kinds(parent),
      [NodeKind::Anchor, NodeKind::Box, NodeKind::Anchor]
    );
    assert_eq!(built.get(), 1);

    TestPlatform::take_log();
    set_version.set(1);

    let second = children(parent)[1];

    assert_eq!(built.get(), 2);
    assert_ne!(second, first);
    assert_eq!(children(parent), [start, second, end]);
    assert!(with(|scene| scene.record(first).is_none()));
    assert!(TestPlatform::take_log().contains(&Call::Destroy(sink(second) - 1)));

    owner.dispose();
  }

  /// The annotation pending when a derived subtree is created lands on the content of
  /// every rebuild.
  #[test]
  fn derived_forwards_the_pending_annotation_to_each_rebuild() {
    TestPlatform::reset();

    let owner = Owner::new();
    let (version, set_version) = signal(0);
    let parent = TestBox::node();
    let annotation = Annotation::<TestPlatform> {
      flex: Some(Flexibility::grow(1.).into()),
      ..Annotation::default()
    };

    with(|scene| scene.set_pending_annotation(annotation.clone()));

    let start = owner.enter(|| {
      derived::<TestPlatform>(move || {
        version.track();

        TestBox::node()
      })
    });

    with(|scene| scene.attach(parent, start, None));

    let content = |parent: NodeId| children(parent)[1];
    let annotation_of =
      |node: NodeId| with(|scene| scene.record(node).unwrap().annotation().cloned());

    assert_eq!(annotation_of(content(parent)), Some(annotation.clone()));

    set_version.set(1);

    assert_eq!(annotation_of(content(parent)), Some(annotation));
    assert_eq!(with(Scene::take_pending_annotation), None);

    owner.dispose();
  }

  /// A reconciled item that leaves is destroyed and its scope disposed, so a signal
  /// created while building it is dead; kept items keep their nodes.
  #[test]
  fn reconcile_disposes_the_owner_of_a_removed_item() {
    TestPlatform::reset();

    let owner = Owner::new();
    let (items, set_items) = signal(vec![1, 2, 3]);
    let inner = Rc::new(Cell::new(None::<Signal<i32>>));
    let parent = TestBox::node();
    let (start, end) = with(Scene::create_segment);

    with(|scene| scene.attach(parent, start, None));
    owner.enter(|| {
      let inner = Rc::clone(&inner);

      reconcile::<TestPlatform, _, _, _>(
        start,
        move || items.get(),
        |item| *item,
        move |item| {
          if item == 2 {
            inner.set(Some(Signal::new(item)));
          }

          TestBox::node()
        },
      );
    });

    let before = children(parent);
    let second = before[2];
    let inner = inner.get().unwrap();

    assert_eq!(before.len(), 5);
    assert_eq!(inner.try_get(), Some(2));

    set_items.set(vec![3, 1]);

    let after = children(parent);

    assert_eq!(after, [start, before[3], before[1], end]);
    assert!(with(|scene| scene.record(second).is_none()));
    assert_eq!(inner.try_get(), None);

    owner.dispose();
  }
}
