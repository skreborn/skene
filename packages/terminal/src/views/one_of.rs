//! One of several branches, by key.

use skene_foundation::{OneOfBase, View, Wraps};

use crate::ActivePlatform;

/// Shows the branch whose key is current, preserving every branch's state.
///
/// A branch is built the first time its key is current, under an owner of its own, then
/// attached and detached between a pair of anchors as the key changes; a branch not
/// shown pauses its effects, catching up once it shows again.
///
/// Every branch built is destroyed when the owner the switch was built under is
/// disposed. Nothing is shown while no key is set or no case has the current key.
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct OneOf<K>(skene_foundation::OneOf<ActivePlatform, K>);

impl<K: PartialEq + 'static> Default for OneOf<K> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<K: PartialEq + 'static> OneOfBase<ActivePlatform, K> for OneOf<K> {}
