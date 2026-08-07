//! A suspense view: fallback content while a subtree's async work is pending.

use std::{cell::Cell, rc::Rc};

use skene_reactive::{Effect, Owner, provide_context};

use crate::{Boundary, IntoView, NodeId, View, tree::TreeOps};

/// A view showing its content once that subtree's async work settles, and the
/// [fallback](Self::set_fallback) until then.
///
/// The content builds immediately under a fresh [`Boundary`], so its tasks start right
/// away and report there; the view swaps between the two subtrees as the boundary
/// empties and refills, keeping both alive, so state survives the swaps. Toward an
/// enclosing boundary the whole subtree counts as one pending task, so nesting
/// composes, and a server render settles through it to the content.
///
/// A facade aliases this with its own ops type, so app code sees a plain `Suspense`.
#[must_use]
pub struct Suspense<Ops> {
  content: Box<dyn FnOnce() -> NodeId>,
  fallback: Box<dyn FnOnce() -> NodeId>,
  ops: std::marker::PhantomData<Ops>,
}

impl<Ops: TreeOps> Suspense<Ops> {
  /// Creates a suspense view around `content`; resources and tasks created inside the
  /// closure report to this view's boundary.
  #[inline(always)]
  pub fn new<V: IntoView>(content: impl FnOnce() -> V + 'static) -> Self {
    Self {
      content: Box::new(move || content().into_view().build()),
      fallback: Box::new(move || Ops::default().create_group()),
      ops: std::marker::PhantomData,
    }
  }

  /// Sets the fallback shown while the content's work is pending; nothing by default.
  #[inline(always)]
  pub fn set_fallback<V: IntoView>(mut self, fallback: impl FnOnce() -> V + 'static) -> Self {
    self.fallback = Box::new(move || fallback().into_view().build());

    self
  }
}

impl<Ops: TreeOps> View for Suspense<Ops> {
  fn build(self) -> NodeId {
    let container = Ops::default().create_group();
    let boundary = Boundary::new();
    let parent = Boundary::current();
    // A child scope so the boundary is in `Context` for the content's tasks only, not
    // its siblings; it holds no state of its own and disposes with the view.
    let scope = Owner::current().child();

    let content = scope.enter(|| {
      provide_context(boundary);

      (self.content)()
    });

    let fallback = (self.fallback)();
    // Whether this subtree currently counts as pending toward the enclosing boundary,
    // so a disposal mid-pend still balances the count.
    let entered = Rc::new(Cell::new(false));

    if let Some(parent) = parent {
      let entered = Rc::clone(&entered);

      Owner::current().on_cleanup(move || {
        if entered.get() {
          parent.leave();
        }
      });
    }

    Effect::new(move |previous: Option<bool>| {
      let settled = boundary.is_settled();

      if previous == Some(settled) {
        return settled;
      }

      if settled {
        if previous.is_some() {
          Ops::default().detach(container, fallback);
        }

        Ops::default().attach(container, content, None);

        if entered.replace(false)
          && let Some(parent) = parent
        {
          parent.leave();
        }
      }
      else {
        if previous.is_some() {
          Ops::default().detach(container, content);
        }

        Ops::default().attach(container, fallback, None);

        if !entered.replace(true)
          && let Some(parent) = parent
        {
          parent.enter();
        }
      }

      settled
    });

    container
  }
}
