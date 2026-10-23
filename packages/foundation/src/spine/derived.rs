//! Reactive subtrees between anchors: a tracked rebuild, and a keyed reconcile.

use std::{
  cell::{Cell, RefCell},
  hash::Hash,
};

use skene_reactive::{Effect, Key, Owner, untrack};

use super::{AnyView, NodeId, Piece, Realization, Scene, Segment, View, reorder::KeyedItems};

/// Mounts the node `build` returns between a fresh pair of anchors and returns the
/// start anchor; `build` reruns whenever a signal it read changes.
///
/// Each rebuild destroys the previous content and disposes its scope, so state inside
/// the content does not survive it. The annotation pending when the subtree is created
/// lands on the content of every rebuild.
pub fn derived<P: Realization>(build: impl Fn() -> NodeId + 'static) -> NodeId {
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
pub fn empty<P: Realization>() -> impl View<P> {
  AnyView::new(|| Segment::<P>::new().start())
}

/// What a reconciled item mounted: its node, and the scope it was built in.
struct Mounted {
  node: NodeId,
  scope: Owner,
}

impl KeyedItems<Mounted> {
  /// Brings the content of the segment `start` begins to the items `keys` names in
  /// order: a kept item stays, a new one is built by `build` from its index in its own
  /// scope under `parent_scope`, a gone one is destroyed, and only the displaced are
  /// re-inserted.
  fn reconcile<P: Realization>(
    &mut self,
    start: NodeId,
    parent_scope: Owner,
    keys: &[Key],
    build: &mut dyn FnMut(usize) -> NodeId,
  ) {
    let dropped = self.update(keys, |index, key| {
      let scope = parent_scope.keyed_child(key);
      let node = scope.enter(|| untrack(|| build(index)));

      Mounted { node, scope }
    });

    for entry in dropped {
      Scene::<P>::with(|scene| scene.destroy(entry.node));
      entry.scope.dispose();
    }

    Scene::<P>::with(|scene| {
      self.place_displaced(|entry, before| {
        scene.insert_in_segment(start, entry.node, before.map(|before| before.node));
      });
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
pub fn reconcile<P: Realization, T, I, K>(
  start: NodeId,
  items: impl Fn() -> I + 'static,
  key: impl Fn(&T) -> K + 'static,
  build: impl Fn(T) -> NodeId + 'static,
) where
  I: IntoIterator<Item = T>,
  K: Eq + Hash + 'static,
{
  let parent_scope = Owner::current();
  let reconciled = RefCell::new(KeyedItems::default());

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
      .reconcile::<P>(start, parent_scope, &keys, &mut |index| {
        let Some(item) = pending[index].take()
        else {
          unreachable!("a reconcile builds each new item once");
        };

        build(item)
      });
  });
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use skene_reactive::{Owner, Set, Track, signal};

  use super::derived;
  use crate::spine::{
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
}
