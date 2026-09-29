//! A handle moving keyboard focus to a built view.

use std::{cell::Cell, marker::PhantomData, rc::Rc};

use skene_core::{NodeId, Platform};

use crate::realize::with_sink;

/// A handle moving keyboard focus to a view programmatically.
///
/// Bound to a focusable view through its `bind_focus`; [`focus`](Self::focus) and
/// [`blur`](Self::blur) go through the platform and do nothing until that view builds.
#[derive(Debug)]
#[must_use]
pub struct FocusHandle<P: Platform> {
  node: Rc<Cell<Option<NodeId>>>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> FocusHandle<P> {
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
      with_sink::<P, _>(node, |sink| P::focus(sink));
    }
  }

  /// Takes focus off the bound view.
  #[inline(always)]
  pub fn blur(&self) {
    if let Some(node) = self.node.get() {
      with_sink::<P, _>(node, |sink| P::blur(sink));
    }
  }
}

impl<P: Platform> Clone for FocusHandle<P> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      node: Rc::clone(&self.node),
      platform: PhantomData,
    }
  }
}

impl<P: Platform> Default for FocusHandle<P> {
  #[inline(always)]
  fn default() -> Self {
    Self {
      node: Rc::new(Cell::new(None)),
      platform: PhantomData,
    }
  }
}
