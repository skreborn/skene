//! One of several branches shown by key, each built once.

use std::{cell::RefCell, marker::PhantomData};

use skene_reactive::{Effect, Owner, untrack};

use crate::{AnyKind, AnyView, NodeId, Piece, Realization, Segment, View, Wraps};

/// One branch: its key, its builder until it is built, and its piece afterwards.
struct Case<P: Realization, K> {
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
pub struct OneOf<P: Realization, K> {
  key: Option<Box<dyn Fn() -> K>>,
  cases: Vec<Case<P, K>>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization, K> std::fmt::Debug for OneOf<P, K> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("OneOf").finish_non_exhaustive()
  }
}

impl<P: Realization, K: PartialEq + 'static> Default for OneOf<P, K> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization, K: PartialEq + 'static> OneOf<P, K> {
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
  pub fn set_key(mut self, key: K) -> Self
  where
    K: Clone,
  {
    self.key = Some(Box::new(move || key.clone()));

    self
  }

  /// Shows the branch whose key `key` returns, re-evaluated whenever a signal it reads
  /// changes.
  pub fn derive_key(mut self, key: impl Fn() -> K + 'static) -> Self {
    self.key = Some(Box::new(key));

    self
  }

  /// Adds the branch shown under `key`, built by `case` the first time the key is
  /// current.
  pub fn add_case<V: View<P>>(mut self, key: K, case: impl FnOnce() -> V + 'static) -> Self {
    self.cases.push(Case {
      key,
      build: Some(Box::new(move || case().into_any())),
      piece: None,
    });

    self
  }
}

impl<P: Realization, K> AnyKind for OneOf<P, K> {}

impl<P: Realization, K: PartialEq + 'static> View<P> for OneOf<P, K> {
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

/// What a facade's [`OneOf`] carries: its constructor, its key and its cases.
pub trait OneOfBase<P: Realization, K: PartialEq + 'static>: Wraps<Inner = OneOf<P, K>> {
  /// A switch with no key and no cases.
  #[must_use]
  fn new() -> Self {
    Self::wrap(OneOf::new())
  }

  /// Sets the key of the branch shown.
  #[must_use]
  fn set_key(self, key: K) -> Self
  where
    K: Clone,
  {
    Self::wrap(self.into_inner().set_key(key))
  }

  /// Shows the branch whose key `key` returns, re-evaluated whenever a signal it reads
  /// changes.
  #[must_use]
  fn derive_key(self, key: impl Fn() -> K + 'static) -> Self {
    Self::wrap(self.into_inner().derive_key(key))
  }

  /// Adds the branch shown under `key`, built by `case` the first time the key is
  /// current.
  #[must_use]
  fn add_case<V: View<P>>(self, key: K, case: impl FnOnce() -> V + 'static) -> Self {
    Self::wrap(self.into_inner().add_case(key, case))
  }
}
