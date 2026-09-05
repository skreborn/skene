//! Suspense boundaries: a subtree whose async work is counted, so its completion can be
//! detected—and, on a server, so a render can flush the shell and stream the boundary's
//! content once it settles.

use skene_reactive::{Signal, Update, With, use_context};

/// The outstanding async work inside a boundary.
///
/// A task spawned within the boundary increments the count; it decrements when the task
/// finishes. The subtree is settled at zero. Threaded through the reactive context
/// ([`use_context`]), so a task spawner reports to the nearest enclosing boundary without any view
/// being special.
#[derive(Clone, Copy)]
#[must_use]
pub struct Boundary {
  pending: Signal<usize>,
}

impl Boundary {
  /// A fresh boundary with nothing pending.
  pub fn new() -> Self {
    Self {
      pending: Signal::opaque(0),
    }
  }

  /// The nearest enclosing boundary, if the current subtree is inside one.
  #[must_use]
  pub fn current() -> Option<Self> {
    use_context::<Self>()
  }

  /// Records a task starting inside the boundary.
  pub fn enter(self) {
    self.pending.update(|pending| *pending += 1);
  }

  /// Records a task finishing inside the boundary.
  pub fn leave(self) {
    self.pending.update(|pending| *pending -= 1);
  }

  /// Whether all the boundary's work has settled. Tracks, so a reactive reader—a
  /// suspense swap, a derived placeholder—re-runs as work starts and finishes.
  #[must_use]
  pub fn is_settled(self) -> bool {
    self.pending.with(|pending| *pending == 0)
  }
}

impl Default for Boundary {
  fn default() -> Self {
    Self::new()
  }
}
