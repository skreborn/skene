//! Reactive subtrees between anchors: a tracked rebuild, and a keyed reconcile.

use std::{
  cell::{Cell, RefCell},
  collections::VecDeque,
  hash::Hash,
};

use rustc_hash::FxHashMap;
use skene_reactive::{Effect, Key, Owner, untrack};

use super::{AnyView, NodeId, Piece, Platform, Scene, Segment, View};
use crate::Reorder;

/// Mounts the node `build` returns between a fresh pair of anchors and returns the
/// start anchor; `build` reruns whenever a signal it read changes.
///
/// Each rebuild destroys the previous content and disposes its scope, so state inside
/// the content does not survive it. The annotation pending when the subtree is created
/// lands on the content of every rebuild.
pub fn derived<P: Platform>(build: impl Fn() -> NodeId + 'static) -> NodeId {
  let segment = Segment::<P>::new();
  let start = segment.start();
  let parent_scope = Owner::current();
  let current = Cell::new(None::<Piece>);

  Effect::render(move |_: Option<()>| {
    // The rebuilt content takes the previous content's place in the owner tree, so
    // state under it keeps its snapshot identity across the rebuild.
    let scope = current.get().map_or_else(
      || parent_scope.child(),
      |previous| previous.scope().replacement(),
    );
    let piece = segment.build(scope, &build);

    if let Some(previous) = current.replace(Some(piece)) {
      previous.discard();
    }

    segment.mount(piece);
  });

  start
}

/// A view that renders nothing: an empty segment.
#[inline(always)]
#[must_use]
pub fn empty<P: Platform>() -> impl View<P> {
  AnyView::new(|| Segment::<P>::new().start())
}

struct Mounted {
  node: NodeId,
  scope: Owner,
}

struct Scratch<K> {
  old: FxHashMap<K, VecDeque<(usize, Mounted)>>,
  occurrences: FxHashMap<Key, usize>,
  old_positions: Vec<usize>,
  new_positions: Vec<usize>,
  reorder: Reorder,
}

impl<K> Default for Scratch<K> {
  fn default() -> Self {
    Self {
      old: FxHashMap::default(),
      occurrences: FxHashMap::default(),
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
///
/// Keys should be unique, which a debug build warns about once; items sharing one
/// match the ones before them in order, each under a scope of its own.
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
      occurrences,
      old_positions,
      new_positions,
      reorder,
    } = &mut *scratch;

    for (index, (key, entry)) in mounted.drain(..).enumerate() {
      old.entry(key).or_default().push_back((index, entry));
    }

    occurrences.clear();
    old_positions.clear();
    new_positions.clear();

    for item in items() {
      let key = key(&item);
      let identity = Key::of(&key);
      let occurrence = occurrences.entry(identity).or_insert(0);
      let repeat = *occurrence;

      *occurrence += 1;

      #[cfg(debug_assertions)]
      if repeat == 1 {
        warn_repeated_key();
      }

      let kept = old.get_mut(&key).and_then(VecDeque::pop_front);
      let entry = if let Some((index, entry)) = kept {
        old_positions.push(index);
        new_positions.push(mounted.len());

        entry
      }
      else {
        let scope = parent_scope.keyed_child(if repeat == 0 {
          identity
        }
        else {
          Key::of(&(identity, repeat))
        });
        let node = scope.enter(|| untrack(|| build(item)));

        Mounted { node, scope }
      };

      mounted.push((key, entry));
    }

    // The last goes first, so each removal shifts as little as it can.
    let mut dropped: Vec<(usize, Mounted)> = old.drain().flat_map(|(_, entries)| entries).collect();

    dropped.sort_unstable_by_key(|(index, _)| std::cmp::Reverse(*index));

    for (_, entry) in dropped {
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

/// Warns, once, that a reconciled collection repeated a key.
#[cfg(debug_assertions)]
fn warn_repeated_key() {
  static WARNED: std::sync::Once = std::sync::Once::new();

  WARNED.call_once(|| {
    eprintln!(
      "skene-core: a reconciled collection repeated a key; keys should be unique, and \
       items sharing one match in order"
    );
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
    testing::{Call, TestBox, TestPlatform, child_kinds, children},
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
    assert!(TestPlatform::take_log().contains(&Call::Destroy(first)));

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
    let (start, end) = Scene::<TestPlatform>::create_segment();

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

  /// Items sharing a key match the ones before them in order, each its own node, and
  /// every one that leaves is destroyed, so no node outlives its item.
  #[test]
  fn reconcile_matches_repeated_keys_in_order() {
    TestPlatform::reset();

    let owner = Owner::new();
    let (items, set_items) = signal(vec![1, 1, 2]);
    let parent = TestBox::node();
    let (start, end) = Scene::<TestPlatform>::create_segment();

    with(|scene| scene.attach(parent, start, None));
    owner.enter(|| {
      reconcile::<TestPlatform, _, _, _>(
        start,
        move || items.get(),
        |item| *item,
        |_| TestBox::node(),
      );
    });

    let before = children(parent);

    assert_eq!(before.len(), 5);

    set_items.set(vec![2, 1]);

    let after = children(parent);

    assert_eq!(after, [start, before[3], before[1], end]);
    assert!(
      with(|scene| scene.record(before[2]).is_none()),
      "the second of the repeated items is gone"
    );

    set_items.set(vec![1, 1, 1]);

    assert_eq!(children(parent).len(), 5);
    assert_eq!(children(parent)[1], before[1], "the first keeps its node");

    owner.dispose();
  }
}
