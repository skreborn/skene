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
#[must_use]
pub fn empty<P: Platform>() -> impl View<P> {
  AnyView::new(|| Segment::<P>::new().start())
}

struct Mounted {
  node: NodeId,
  scope: Owner,
}

/// What a keyed reconcile keeps between its runs: the mounted items by key in order,
/// and the scratch each run reuses.
#[derive(Default)]
struct Reconciled {
  mounted: Vec<(Key, Mounted)>,
  old: FxHashMap<Key, VecDeque<(usize, Mounted)>>,
  occurrences: FxHashMap<Key, u32>,
  old_positions: Vec<usize>,
  new_positions: Vec<usize>,
  reorder: Reorder,
}

impl Reconciled {
  /// Brings the content of the segment `start` begins to the items `keys` names in
  /// order: a kept item stays, a new one is built by `build` from its index in its own
  /// scope under `parent_scope`, a gone one is destroyed, and only the displaced are
  /// re-inserted.
  fn update<P: Platform>(
    &mut self,
    start: NodeId,
    parent_scope: Owner,
    keys: &[Key],
    build: &mut dyn FnMut(usize) -> NodeId,
  ) {
    let Self {
      mounted,
      old,
      occurrences,
      old_positions,
      new_positions,
      reorder,
    } = self;

    for (index, (key, entry)) in mounted.drain(..).enumerate() {
      old.entry(key).or_default().push_back((index, entry));
    }

    occurrences.clear();
    old_positions.clear();
    new_positions.clear();

    for (index, &key) in keys.iter().enumerate() {
      let occurrence = occurrences.entry(key).or_insert(0);
      let repeat = *occurrence;

      *occurrence += 1;

      #[cfg(debug_assertions)]
      if repeat == 1 {
        warn_repeated_key();
      }

      let kept = old.get_mut(&key).and_then(VecDeque::pop_front);
      let entry = if let Some((position, entry)) = kept {
        old_positions.push(position);
        new_positions.push(mounted.len());

        entry
      }
      else {
        let scope = parent_scope.keyed_child(if repeat == 0 {
          key
        }
        else {
          Key::of(&(key, repeat))
        });
        let node = scope.enter(|| untrack(|| build(index)));

        Mounted { node, scope }
      };

      mounted.push((key, entry));
    }

    // The last goes first, so each removal shifts as little as it can.
    let mut dropped: Vec<(usize, Mounted)> = old.drain().flat_map(|(_, entries)| entries).collect();

    dropped.sort_unstable_by_key(|(index, _)| std::cmp::Reverse(*index));

    for (_, entry) in dropped {
      Scene::<P>::with(|scene| scene.destroy(entry.node));
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
  let reconciled = RefCell::new(Reconciled::default());

  Effect::render(move |_: Option<()>| {
    let mut pending = Vec::new();
    let keys: Vec<Key> = items()
      .into_iter()
      .map(|item| {
        let identity = Key::of(&key(&item));

        pending.push(Some(item));

        identity
      })
      .collect();

    reconciled
      .borrow_mut()
      .update::<P>(start, parent_scope, &keys, &mut |index| {
        let Some(item) = pending[index].take()
        else {
          unreachable!("a reconcile builds each new item once");
        };

        build(item)
      });
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
    testing::{Call, TestBox, TestPlatform, TestScene, child_kinds, children},
  };

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

    TestScene::with(|scene| scene.attach(parent, start, None));

    let end = TestScene::with(|scene| scene.segment_end(start)).unwrap();
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
    assert!(TestScene::with(|scene| scene.record(first).is_none()));
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

    TestScene::with(|scene| scene.set_pending_annotation(annotation.clone()));

    let start = owner.enter(|| {
      derived::<TestPlatform>(move || {
        version.track();

        TestBox::node()
      })
    });

    TestScene::with(|scene| scene.attach(parent, start, None));

    let content = |parent: NodeId| children(parent)[1];
    let annotation_of =
      |node: NodeId| TestScene::with(|scene| scene.record(node).unwrap().annotation().cloned());

    assert_eq!(annotation_of(content(parent)), Some(annotation.clone()));

    set_version.set(1);

    assert_eq!(annotation_of(content(parent)), Some(annotation));
    assert_eq!(TestScene::with(Scene::take_pending_annotation), None);

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

    TestScene::with(|scene| scene.attach(parent, start, None));
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
    assert!(TestScene::with(|scene| scene.record(second).is_none()));
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

    TestScene::with(|scene| scene.attach(parent, start, None));
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
      TestScene::with(|scene| scene.record(before[2]).is_none()),
      "the second of the repeated items is gone"
    );

    set_items.set(vec![1, 1, 1]);

    assert_eq!(children(parent).len(), 5);
    assert_eq!(children(parent)[1], before[1], "the first keeps its node");

    owner.dispose();
  }

  #[cfg(not(target_arch = "wasm32"))]
  use proptest::prelude::*;

  #[cfg(not(target_arch = "wasm32"))]
  proptest! {
    /// Any run of keyed lists, repeated keys among them, reconciles to nodes in the
    /// lists' order: an item kept from the list before keeps its node, the nth item of
    /// a key matching the nth before it, and every node left behind is destroyed.
    #[test]
    fn any_run_of_lists_reconciles_like_its_model(
      lists in proptest::collection::vec(
        proptest::collection::vec(0..6_u8, 0..10),
        1..8,
      ),
    ) {
      use std::collections::HashMap;

      TestPlatform::reset();

      let owner = Owner::new();
      let (items, set_items) = signal(Vec::<u8>::new());
      let built = Rc::new(std::cell::RefCell::new(HashMap::<NodeId, u8>::new()));
      let parent = TestBox::node();
      let (start, end) = Scene::<TestPlatform>::create_segment();

      TestScene::with(|scene| scene.attach(parent, start, None));
      owner.enter(|| {
        let built = Rc::clone(&built);

        reconcile::<TestPlatform, _, _, _>(
          start,
          move || items.get(),
          |item| *item,
          move |item| {
            let node = TestBox::node();

            built.borrow_mut().insert(node, item);

            node
          },
        );
      });

      let mut previous: Vec<(u8, NodeId)> = Vec::new();

      for list in lists {
        set_items.set(list.clone());

        let nodes = children(parent);

        prop_assert_eq!(nodes.first(), Some(&start));
        prop_assert_eq!(nodes.last(), Some(&end));

        let content = &nodes[1..nodes.len() - 1];
        let keys: Vec<u8> = content.iter().map(|node| built.borrow()[node]).collect();

        prop_assert_eq!(&keys, &list);

        // The nth occurrence of a key keeps the node of the nth occurrence before it.
        let mut before: HashMap<u8, Vec<NodeId>> = HashMap::new();

        for (key, node) in &previous {
          before.entry(*key).or_default().push(*node);
        }

        let mut seen: HashMap<u8, usize> = HashMap::new();

        for (key, node) in keys.iter().zip(content) {
          let occurrence = seen.entry(*key).or_insert(0);

          if let Some(kept) = before.get(key).and_then(|nodes| nodes.get(*occurrence)) {
            prop_assert_eq!(kept, node, "the item keeps its node");
          }

          *occurrence += 1;
        }

        for (_, node) in &previous {
          if !content.contains(node) {
            prop_assert!(
              TestScene::with(|scene| scene.record(*node).is_none()),
              "a node left behind is destroyed"
            );
          }
        }

        previous = keys.into_iter().zip(content.iter().copied()).collect();
      }

      owner.dispose();
    }
  }
}
