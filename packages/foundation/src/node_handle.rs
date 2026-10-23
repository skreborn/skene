//! A handle naming the node a built view realized.

use std::rc::Rc;

use skene_reactive::{Get, Set, Signal, Update};

use crate::{NodeId, Realization, Size};

/// Whether a handle measures its node.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Measuring {
  /// Nothing asked for the size.
  Not,
  /// The size was asked for before a node was bound.
  Wanted,
  /// The bound node is measured.
  Node(NodeId),
}

/// A handle naming the node a view built: the box an overlay is placed against, the
/// view focus moves to, and the box whose measured size a view derives from.
///
/// `Copy`; bound to one view through its `bind_handle`, and holding nothing until that
/// view builds. It lives as long as the owner it was made under.
#[must_use]
pub struct NodeHandle<P: Realization> {
  node: Signal<Option<NodeId>>,
  size: Signal<Option<Size<P::Scalar>>>,
  measuring: Signal<Measuring>,
}

impl<P: Realization> NodeHandle<P> {
  /// An unbound handle.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      node: Signal::new(None),
      size: Signal::new(None),
      measuring: Signal::new(Measuring::Not),
    }
  }

  /// Binds the handle to `node`, a built view's root.
  pub fn assign(self, node: NodeId) {
    self.node.set(Some(node));

    if self.measuring.get_untracked() != Measuring::Not {
      self.measure(node);
    }
  }

  /// The bound node, tracked; `None` until the view builds.
  #[inline(always)]
  #[must_use]
  pub fn node(self) -> Option<NodeId> {
    self.node.get()
  }

  /// Moves focus to the bound view.
  pub fn focus(self) {
    if let Some(node) = self.node.get_untracked() {
      P::focus(node);
    }
  }

  /// Takes focus off the bound view.
  pub fn blur(self) {
    if let Some(node) = self.node.get_untracked() {
      P::blur(node);
    }
  }

  /// The bound box's last measured size, tracked; `None` before the first measurement.
  ///
  /// The box is measured from the first time its size is asked for.
  #[must_use]
  pub fn size(self) -> Option<Size<P::Scalar>> {
    if self.measuring.get_untracked() == Measuring::Not {
      match self.node.get_untracked() {
        Some(node) => self.measure(node),
        None => self.measuring.set(Measuring::Wanted),
      }
    }

    self.size.get()
  }

  /// Starts measuring `node`, unless it is measured already.
  fn measure(self, node: NodeId) {
    if self.measuring.get_untracked() == Measuring::Node(node) {
      return;
    }

    self.measuring.set(Measuring::Node(node));

    let size = self.size;
    let _observation = P::observe_size(
      node,
      Rc::new(move |measured| {
        let _ = size.try_update(|size| *size = Some(measured));
      }),
    );
  }
}

impl<P: Realization> Copy for NodeHandle<P> {}

#[expect(
  clippy::expl_impl_clone_on_copy,
  reason = "a derive would bound the platform on Copy"
)]
impl<P: Realization> Clone for NodeHandle<P> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<P: Realization> Default for NodeHandle<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> std::fmt::Debug for NodeHandle<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("NodeHandle").finish_non_exhaustive()
  }
}
