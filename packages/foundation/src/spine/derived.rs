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
