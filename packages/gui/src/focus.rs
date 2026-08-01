//! Programmatic keyboard focus.

// The bound node is read only by a platform's realization; a target with no platform
// compiles the handle but never reads it.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::{cell::Cell, rc::Rc};

use skene_core::NodeId;

/// A handle that moves keyboard focus to a view programmatically.
///
/// Create one, attach it to a focusable view with its `bind_focus`, and call
/// [`focus`](Self::focus) or [`blur`](Self::blur) from any handler. Calls are no-ops
/// until the bound view builds.
#[derive(Clone, Default)]
#[must_use]
pub struct FocusHandle(Rc<Cell<Option<NodeId>>>);

impl FocusHandle {
  /// Creates an unbound handle.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Binds the handle to a built view's node.
  pub(crate) fn assign(&self, node: NodeId) {
    self.0.set(Some(node));
  }

  /// Moves keyboard focus to the bound view; a no-op while unbound.
  #[inline(always)]
  pub fn focus(&self) {
    #[cfg(target_arch = "wasm32")]
    if let Some(node) = self.0.get() {
      crate::driver::focus_node(node);
    }
  }

  /// Removes keyboard focus from the bound view; a no-op while unbound.
  #[inline(always)]
  pub fn blur(&self) {
    #[cfg(target_arch = "wasm32")]
    if let Some(node) = self.0.get() {
      crate::driver::blur_node(node);
    }
  }
}
