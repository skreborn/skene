//! One of several branches, by key.

use skene_foundation::{AnyKind, NodeId, View};

use crate::ActivePlatform;

/// Shows the branch whose key is current, preserving every branch's state.
///
/// A branch is built the first time its key is current, under an owner of its own, then
/// attached and detached between a pair of anchors as the key changes; a branch not
/// shown pauses its effects, catching up once it shows again.
///
/// Every branch built is destroyed when the owner the switch was built under is
/// disposed. Nothing is shown while no key is set or no case has the current key.
#[must_use]
pub struct Switch<K>(skene_foundation::Switch<ActivePlatform, K>);

impl<K> std::fmt::Debug for Switch<K> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(formatter)
  }
}

impl<K: PartialEq + 'static> Default for Switch<K> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<K: PartialEq + 'static> Switch<K> {
  /// A switch with no key and no cases.
  #[inline(always)]
  pub const fn new() -> Self {
    Self(skene_foundation::Switch::new())
  }

  /// Sets the key of the branch shown.
  #[inline(always)]
  pub fn set_key(self, key: K) -> Self
  where
    K: Clone,
  {
    Self(self.0.set_key(key))
  }

  /// Shows the branch whose key `key` returns, re-evaluated whenever a signal it reads
  /// changes.
  #[inline(always)]
  pub fn derive_key(self, key: impl Fn() -> K + 'static) -> Self {
    Self(self.0.derive_key(key))
  }

  /// Adds the branch shown under `key`, built by `case` the first time the key is
  /// current.
  #[inline(always)]
  pub fn set_case<V: View<ActivePlatform>>(
    self,
    key: K,
    case: impl FnOnce() -> V + 'static,
  ) -> Self {
    Self(self.0.set_case(key, case))
  }
}

impl<K> AnyKind for Switch<K> {}

impl<K: PartialEq + 'static> View<ActivePlatform> for Switch<K> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
