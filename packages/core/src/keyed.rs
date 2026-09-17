//! A view's stable identity.

use std::marker::PhantomData;

use skene_reactive::{Key, Owner};

use super::{AnyKind, Children, HasChildren, NodeId, Platform, Seal, View};

/// The view a [`set_key`](View::set_key) named: built under a scope its key
/// identifies rather than its position.
///
/// Creates no node. State created while building the view—its children's included—
/// keeps its snapshot identity wherever the view sits and however its siblings change,
/// which is the same identity [`reconcile`](crate::reconcile) gives a keyed list item.
/// A container's setters carry through, so the children still come last.
#[must_use]
pub struct Keyed<P: Platform, V> {
  key: Key,
  view: V,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform, V> Keyed<P, V> {
  /// `view` under `key`.
  #[inline(always)]
  pub(crate) const fn new(key: Key, view: V) -> Self {
    Self {
      key,
      view,
      platform: PhantomData,
    }
  }
}

impl<P: Platform, V> AnyKind for Keyed<P, V> {}

impl<P: Platform, V: View<P>> View<P> for Keyed<P, V> {
  #[inline(always)]
  fn build(self) -> NodeId {
    Owner::current()
      .keyed_child(self.key)
      .enter(|| self.view.build())
  }
}

impl<P: Platform, V: HasChildren<P>> HasChildren<P> for Keyed<P, V> {
  type Kind = V::Kind;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<P, Self::Kind> {
    self.view.children_mut(seal)
  }
}
