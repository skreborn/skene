//! Suspense boundaries: a subtree whose async work is counted, so its completion can be
//! detected — and, on the server, so the render can flush the shell and stream the boundary's
//! content once it settles.

#[cfg(target_os = "wasi")]
use skene_reactive::SignalWith;
use skene_reactive::{Context, RwSignal, SignalUpdate};

/// The outstanding async work inside a boundary.
///
/// A task spawned within the boundary increments the count; it decrements when the task
/// finishes. The subtree is settled at zero. Threaded through the reactive [`Context`] exactly
/// as `TextTheme` threads inherited style, so [`spawn`](crate::spawn) reports to the nearest
/// enclosing boundary without any view being special.
#[derive(Clone, Copy)]
pub(crate) struct Boundary {
  pending: RwSignal<usize>,
}

impl Boundary {
  /// A fresh boundary with nothing pending.
  #[cfg(target_os = "wasi")]
  pub(crate) fn new() -> Self {
    Self {
      pending: RwSignal::new(0),
    }
  }

  /// The nearest enclosing boundary, if the current subtree is inside one.
  pub(crate) fn current() -> Option<Self> {
    Context::get::<Self>()
  }

  /// Records a task starting inside the boundary.
  pub(crate) fn enter(self) {
    self.pending.update(|pending| *pending += 1);
  }

  /// Records a task finishing inside the boundary.
  pub(crate) fn leave(self) {
    self.pending.update(|pending| *pending -= 1);
  }

  /// Whether all the boundary's work has settled. Does not track.
  #[cfg(target_os = "wasi")]
  pub(crate) fn is_settled(self) -> bool {
    self.pending.with_untracked(|pending| *pending == 0)
  }
}
