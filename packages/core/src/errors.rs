//! The errors a subtree carries to its nearest enclosing boundary.

use std::{error::Error, rc::Rc};

use skene_reactive::{Signal, Update, With, use_context};

/// The errors a subtree carried to its nearest error boundary.
///
/// Capture the handle with [`current`](Self::current) while building—handlers and
/// tasks run outside the build scope, where the lookup no longer resolves—then
/// [`report`](Self::report) into it from anywhere. The boundary presents its fallback
/// while any errors are held; [`clear`](Self::clear) dismisses them.
#[derive(Clone, Copy)]
#[must_use]
pub struct Errors {
  errors: Signal<Vec<Rc<dyn Error>>>,
}

impl Default for Errors {
  fn default() -> Self {
    Self::new()
  }
}

impl Errors {
  /// A fresh handle holding no errors.
  pub fn new() -> Self {
    Self {
      errors: Signal::opaque(Vec::new()),
    }
  }

  /// The nearest enclosing error boundary's handle, if the current subtree is inside
  /// one.
  #[must_use]
  pub fn current() -> Option<Self> {
    use_context::<Self>()
  }

  /// Reports an error; the boundary swaps to its fallback.
  pub fn report(&self, error: impl Error + 'static) {
    self.errors.update(|errors| errors.push(Rc::new(error)));
  }

  /// Drops every held error; the boundary swaps back to its content.
  pub fn clear(&self) {
    self.errors.update(Vec::clear);
  }

  /// Whether no errors are held. Tracks.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.errors.with(Vec::is_empty)
  }

  /// Runs `apply` on the held errors, oldest first. Tracks.
  pub fn with<R>(&self, apply: impl FnOnce(&[Rc<dyn Error>]) -> R) -> R {
    self.errors.with(|errors| apply(errors))
  }
}
