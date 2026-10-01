//! One of several branches, by key.

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

skene_foundation::impl_switch!(Switch, ActivePlatform);
