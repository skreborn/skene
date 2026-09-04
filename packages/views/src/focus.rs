//! A handle moving keyboard focus to a built view.

use std::{cell::Cell, marker::PhantomData, rc::Rc};

use skene_core::next::{Driver, NodeId};

use crate::realize::with_sink;

/// A handle moving keyboard focus to a view programmatically.
///
/// Bound to a focusable view through its `bind_focus`; [`focus`](Self::focus) and
/// [`blur`](Self::blur) go through the driver and do nothing until that view builds.
#[must_use]
pub struct FocusHandle<D: Driver> {
  node: Rc<Cell<Option<NodeId>>>,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> FocusHandle<D> {
  /// An unbound handle.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Binds the handle to `node`, a built view's root.
  #[inline(always)]
  pub fn assign(&self, node: NodeId) {
    self.node.set(Some(node));
  }

  /// The bound node, once the view built.
  #[inline(always)]
  #[must_use]
  pub fn node(&self) -> Option<NodeId> {
    self.node.get()
  }

  /// Moves focus to the bound view.
  #[inline(always)]
  pub fn focus(&self) {
    if let Some(node) = self.node.get() {
      with_sink::<D, _>(node, |sink| D::default().focus(sink));
    }
  }

  /// Takes focus off the bound view.
  #[inline(always)]
  pub fn blur(&self) {
    if let Some(node) = self.node.get() {
      with_sink::<D, _>(node, |sink| D::default().blur(sink));
    }
  }
}

impl<D: Driver> Clone for FocusHandle<D> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      node: Rc::clone(&self.node),
      driver: PhantomData,
    }
  }
}

impl<D: Driver> Default for FocusHandle<D> {
  #[inline(always)]
  fn default() -> Self {
    Self {
      node: Rc::new(Cell::new(None)),
      driver: PhantomData,
    }
  }
}
