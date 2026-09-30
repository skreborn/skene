//! One of several branches shown by key, each built once.

use std::{cell::RefCell, marker::PhantomData};

use skene_core::{AnyKind, AnyView, NodeId, Piece, Platform, Segment, View};
use skene_reactive::{Effect, Owner, untrack};

/// One branch: its key, its builder until it is built, and its piece afterwards.
struct Case<P: Platform, K> {
  key: K,
  build: Option<Box<dyn FnOnce() -> AnyView<P>>>,
  piece: Option<Piece>,
}

/// Shows the branch whose key is current, preserving every branch's state.
///
/// A branch is built the first time its key is current, under an owner of its own, then
/// attached and detached between a pair of anchors as the key changes; a branch not
/// shown pauses its effects, catching up once it shows again.
///
/// Every branch built is destroyed when the owner the switch was built under is
/// disposed. Nothing is shown while no key is set or no case has the current key.
#[must_use]
pub struct Switch<P: Platform, K> {
  key: Option<Box<dyn Fn() -> K>>,
  cases: Vec<Case<P, K>>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform, K> std::fmt::Debug for Switch<P, K> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Switch").finish_non_exhaustive()
  }
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
  pub fn set_case<V: View<P>>(mut self, key: K, case: impl FnOnce() -> V + 'static) -> Self {
    self.cases.push(Case {
      key,
      build: Some(Box::new(move || case().into_any())),
      piece: None,
    });

    self
  }
}

impl<P: Platform, K> AnyKind for Switch<P, K> {}

impl<P: Platform, K: PartialEq + 'static> View<P> for Switch<P, K> {
  fn build(self) -> NodeId {
    let segment = Segment::<P>::new();
    let start = segment.start();
    let Some(key) = self.key
    else {
      return start;
    };
    let owner = Owner::current();
    let cases = RefCell::new(self.cases);

    Effect::render(move |previous: Option<Option<usize>>| {
      let key = key();
      let index = cases.borrow().iter().position(|case| case.key == key);
      let previous = previous.flatten();

      if previous == index {
        return index;
      }

      if let Some(shown) = previous.and_then(|previous| cases.borrow()[previous].piece) {
        shown.pause();
        segment.unmount(shown);
      }

      if let Some(index) = index {
        let held = cases.borrow()[index].piece;

        if let Some(held) = held {
          segment.mount(held);
          held.resume();
        }
        else {
          let builder = cases.borrow_mut()[index].build.take();

          if let Some(builder) = builder {
            let built = segment.build(owner.child(), || untrack(|| builder().build()));

            cases.borrow_mut()[index].piece = Some(built);
            segment.mount(built);
          }
        }
      }

      index
    });

    start
  }
}
