//! Error boundaries: carried errors collected from a subtree, presented by the nearest
//! enclosing boundary.

use std::{error::Error, rc::Rc};

use skene_reactive::{Context, Effect, RwSignal, Scope, SignalUpdate, SignalWith};

use crate::{IntoView, NodeId, View, reconcile::dynamic, tree::TreeOps};

/// The errors a subtree carried to its nearest [`ErrorBoundary`].
///
/// Capture the handle with [`current`](Self::current) while building—handlers and
/// tasks run outside the build scope, where the lookup no longer resolves—then
/// [`report`](Self::report) into it from anywhere. The boundary presents its fallback
/// while any errors are held; [`clear`](Self::clear) dismisses them.
#[derive(Clone, Copy)]
#[must_use]
pub struct Errors {
  errors: RwSignal<Vec<Rc<dyn Error>>>,
}

impl Errors {
  fn new() -> Self {
    Self {
      errors: RwSignal::new(Vec::new()),
    }
  }

  /// The nearest enclosing error boundary's handle, if the current subtree is inside
  /// one.
  #[must_use]
  pub fn current() -> Option<Self> {
    Context::get::<Self>()
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

/// A view showing its content while no errors are held, and the
/// [fallback](Self::set_fallback) once one is reported.
///
/// The boundary provides an [`Errors`] handle to its content through the reactive
/// context; the fallback receives the same handle, to present the errors
/// ([`Errors::with`]) and dismiss them ([`Errors::clear`]). The content stays alive
/// behind the fallback, so clearing restores it as it was; the fallback rebuilds
/// whenever the held errors change. Failure carried in values stays the default—this
/// boundary is for errors with no render site of their own: handlers, tasks, and
/// components that do not own their error presentation.
///
/// A facade aliases this with its own ops type, so app code sees a plain
/// `ErrorBoundary`.
#[must_use]
pub struct ErrorBoundary<Ops> {
  content: Box<dyn FnOnce() -> NodeId>,
  fallback: Rc<dyn Fn(Errors) -> NodeId>,
  ops: std::marker::PhantomData<Ops>,
}

impl<Ops: TreeOps> ErrorBoundary<Ops> {
  /// Creates an error boundary around `content`; [`Errors::current`] resolves to this
  /// boundary inside the closure.
  #[inline(always)]
  pub fn new<V: IntoView>(content: impl FnOnce() -> V + 'static) -> Self {
    Self {
      content: Box::new(move || content().into_view().build()),
      fallback: Rc::new(|_| Ops::default().create_group()),
      ops: std::marker::PhantomData,
    }
  }

  /// Sets the fallback shown while errors are held; nothing by default.
  #[inline(always)]
  pub fn set_fallback<V: IntoView>(mut self, fallback: impl Fn(Errors) -> V + 'static) -> Self {
    self.fallback = Rc::new(move |errors| fallback(errors).into_view().build());

    self
  }
}

impl<Ops: TreeOps> View for ErrorBoundary<Ops> {
  fn build(self) -> NodeId {
    let container = Ops::default().create_group();
    let errors = Errors::new();
    // A child scope so the handle is in `Context` for the content only, not its
    // siblings; it holds no state of its own and disposes with the view.
    let scope = Scope::current().create_child();

    let content = scope.enter(|| {
      Context::provide(errors);

      (self.content)()
    });

    Effect::new(move |previous: Option<bool>| {
      let empty = errors.is_empty();

      if previous == Some(empty) {
        return empty;
      }

      if empty {
        Ops::default().attach(container, content, None);
      }
      else if previous.is_some() {
        Ops::default().detach(container, content);
      }

      empty
    });

    let fallback = self.fallback;
    let anchor = dynamic::<Ops>(move || {
      if errors.is_empty() {
        Ops::default().create_group()
      }
      else {
        fallback(errors)
      }
    });

    Ops::default().attach(container, anchor, None);

    container
  }
}
