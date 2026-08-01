//! Programmatic keyboard focus.

use std::{cell::Cell, rc::Rc};

use skene_core::NodeId;

use crate::with_state;

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
    if let Some(node) = self.0.get() {
      skene_reactive::Effect::batch(|| with_state(|state| state.set_focused(Some(node))));
    }
  }

  /// Removes keyboard focus from the bound view, when it holds it; a no-op otherwise.
  #[inline(always)]
  pub fn blur(&self) {
    if let Some(node) = self.0.get() {
      skene_reactive::Effect::batch(|| {
        with_state(|state| {
          if state.focused.get() == Some(node) {
            state.set_focused(None);
          }
        });
      });
    }
  }
}
