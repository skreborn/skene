//! A view that renders one thing without script and another once the client owns the
//! page.

use std::{cell::Cell, rc::Rc};

use skene_core::{IntoView, NodeId, View};

/// A builder an enhancement can run more than once, since it renders one of its two
/// forms over a page's life.
type Build = Rc<dyn Fn() -> NodeId>;

thread_local! {
  /// Whether enhancements render a scriptless form at all; see
  /// [`set_scriptless_fallback`].
  static FALLBACK: Cell<bool> = const { Cell::new(true) };
}

/// Sets whether [`Enhanced`] renders a scriptless fallback at all.
///
/// On by default, because progressive enhancement is the framework's doctrine: a page
/// works without script and the client layers interactivity on top. An app that does
/// not care about a scriptless reader turns it off, and every enhancement then renders
/// its enhanced form everywhere—including on the server, where it is inert until the
/// client runs.
///
/// Turning it off also makes hydration cheaper: both sides build the same tree, so
/// nothing is built to be replaced a moment later.
///
/// Call it before rendering or launching; it applies to the current thread, which is
/// the one a render or a client runs on.
pub fn set_scriptless_fallback(emit: bool) {
  FALLBACK.with(|fallback| fallback.set(emit));
}

/// Whether an enhancement should render its scriptless form.
#[inline(always)]
fn wanted() -> bool {
  FALLBACK.with(Cell::get)
}

/// A view that renders its fallback without script and rebuilds into its child once the
/// client owns the page.
///
/// Progressive enhancement in one view: the server serializes the fallback, a scriptless
/// client keeps it and it works, and a client that runs replaces it with the child. The
/// pair a control uses is the browser's own control as the fallback and the drawn one as
/// the child—so the page is never gated on script, and nothing about the enhanced form
/// has to survive serialization.
///
/// The swap happens **after** hydration, not during it. Hydration matches the client's
/// tree against the server's by node id, and a drawn control allocates a different
/// number of nodes than a native one; rebuilding mid-hydration would shift every id
/// after it and let a later node adopt an unrelated element. Reproducing the fallback
/// first keeps that matching exact, and the rebuild runs in a tree nobody is matching
/// any more.
///
/// A fresh client-side launch never renders the fallback at all: there is no server tree
/// to reproduce, so the child builds directly. An app that does not want a scriptless
/// form anywhere turns it off with [`set_scriptless_fallback`].
#[must_use]
pub struct Enhanced<C = skene_core::NoChild> {
  fallback: Build,
  child: C,
}

impl Enhanced {
  /// Creates an enhancement over the view to render without script.
  #[inline(always)]
  pub fn new<V: IntoView>(fallback: impl Fn() -> V + 'static) -> Self {
    Self {
      fallback: Rc::new(move || fallback().into_view().build()),
      child: skene_core::NoChild,
    }
  }
}

impl<C> Enhanced<C> {
  /// Sets the view the client renders once it owns the page, which the enhancement
  /// requires before it can build.
  #[inline(always)]
  pub fn set_child<V: IntoView>(self, child: impl Fn() -> V + 'static) -> Enhanced<Build> {
    Enhanced {
      fallback: self.fallback,
      child: Rc::new(move || child().into_view().build()),
    }
  }
}

impl View for Enhanced<Build> {
  fn build(self) -> NodeId {
    let Self { fallback, child } = self;

    crate::derived(move || {
      if crate::driver::settled() || !wanted() {
        child()
      }
      else {
        fallback()
      }
    })
  }
}
