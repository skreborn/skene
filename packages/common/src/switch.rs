//! One of several branches shown by key, each built once.

use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use skene_core::{AnyKind, AnyView, IntoView, NodeId, Platform, Scene, View};
use skene_reactive::{Effect, Owner, on_cleanup, untrack};

/// One branch: its key, its builder until it is built, and its node afterwards.
struct Case<P: Platform, K> {
  key: K,
  build: Option<Box<dyn FnOnce() -> AnyView<P>>>,
  node: Option<NodeId>,
}

/// Shows the branch whose key is current, preserving every branch's state: a branch
/// is built the first time its key is current, under an owner of its own, then
/// attached and detached between a pair of anchors as the key changes.
///
/// Every branch built is destroyed when the owner the switch was built under is
/// disposed. Nothing is shown while no key is set or no case has the current key.
#[must_use]
pub struct Switch<P: Platform, K> {
  key: Option<Box<dyn Fn() -> K>>,
  cases: Vec<Case<P, K>>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform, K: PartialEq + 'static> Default for Switch<P, K> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform, K: PartialEq + 'static> Switch<P, K> {
  /// A switch with no key and no cases.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      key: None,
      cases: Vec::new(),
      platform: PhantomData,
    }
  }

  /// Sets the key of the branch shown.
  #[inline(always)]
  pub fn set_key(mut self, key: K) -> Self
  where
    K: Clone,
  {
    self.key = Some(Box::new(move || key.clone()));

    self
  }

  /// Shows the branch whose key `key` returns, re-evaluated whenever a signal it reads
  /// changes.
  #[inline(always)]
  pub fn derive_key(mut self, key: impl Fn() -> K + 'static) -> Self {
    self.key = Some(Box::new(key));

    self
  }

  /// Adds the branch shown under `key`, built by `case` the first time the key is
  /// current.
  #[inline(always)]
  pub fn set_case<V: IntoView<P>>(mut self, key: K, case: impl FnOnce() -> V + 'static) -> Self {
    self.cases.push(Case {
      key,
      build: Some(Box::new(move || case().into_view())),
      node: None,
    });

    self
  }
}

impl<P: Platform, K> AnyKind for Switch<P, K> {}

impl<P: Platform, K: PartialEq + 'static> View<P> for Switch<P, K> {
  fn build(self) -> NodeId {
    let (start, annotation) =
      Scene::<P>::with(|scene| (scene.create_segment().0, scene.take_pending_annotation()));
    let Some(key) = self.key
    else {
      return start;
    };
    let owner = Owner::current();
    let cases = Rc::new(RefCell::new(self.cases));
    let built = Rc::clone(&cases);

    on_cleanup(move || {
      let nodes: Vec<NodeId> = built
        .borrow_mut()
        .iter_mut()
        .filter_map(|case| case.node.take())
        .collect();

      Scene::<P>::with(|scene| {
        for node in nodes {
          scene.destroy(node);
        }
      });
    });

    Effect::render(move |previous: Option<Option<NodeId>>| {
      let key = key();
      let index = cases.borrow().iter().position(|case| case.key == key);
      let node = index.and_then(|index| {
        let builder = cases.borrow_mut()[index].build.take();

        if let Some(builder) = builder {
          let scope = owner.child();
          let branch = || scope.enter(|| untrack(|| builder().build()));
          let node = match &annotation {
            Some(annotation) => Scene::<P>::annotate(annotation.clone(), branch),
            None => branch(),
          };

          cases.borrow_mut()[index].node = Some(node);
        }

        cases.borrow()[index].node
      });
      let previous = previous.flatten();

      if previous != node {
        Scene::<P>::with(|scene| {
          if let Some(previous) = previous {
            scene.remove_from_segment(start, previous);
          }

          if let Some(node) = node {
            scene.insert_in_segment(start, node, None);
          }
        });
      }

      node
    });

    start
  }
}
