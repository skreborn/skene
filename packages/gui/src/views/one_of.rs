//! One of several branches, by key.

use skene_foundation::{OneOfBase, View, Wraps};

use crate::Active;

/// Shows the branch whose key is current, preserving every branch's state.
///
/// A branch is built the first time its key is current, under an owner of its own, then
/// attached and detached between a pair of anchors as the key changes; a branch not
/// shown pauses its effects, catching up once it shows again.
///
/// Every branch built is destroyed when the owner the switch was built under is
/// disposed. Nothing is shown while no key is set or no case has the current key.
///
/// ```
/// use skene_gui::prelude::*;
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// enum Tab {
///   Inbox,
///   Archive,
/// }
///
/// let tab = Signal::new(Tab::Inbox);
/// let pane = OneOf::new()
///   .derive_key(move || tab.get())
///   .add_case(Tab::Inbox, || Text::new("Inbox"))
///   .add_case(Tab::Archive, || Text::new("Archive"));
/// ```
#[derive(Debug, View, Wraps)]
#[view(core = skene_foundation::OneOfCore<Active>)]
#[must_use]
pub struct OneOf<K>(skene_foundation::OneOf<Active, K>);

impl<K: PartialEq + 'static> Default for OneOf<K> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<K: PartialEq + 'static> OneOfBase<Active, K> for OneOf<K> {}
