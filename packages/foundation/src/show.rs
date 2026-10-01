//! A child built once and shown or hidden without rebuilding.

use std::{cell::Cell, marker::PhantomData};

use skene_reactive::{Effect, Owner, untrack};

use crate::{AnyKind, AnyView, NoChild, NodeId, Piece, Platform, Segment, View, Wraps};

/// Shows or hides one child, preserving its state.
///
/// The child is built the first time it shows, then attached or detached between a
/// pair of anchors when the condition changes, and left where it stands while it
/// holds. While hidden its effects pause, catching up once it shows again.
///
/// Holds [`NoChild`] until `set_child` supplies the child; only then is it a view.
#[must_use]
pub struct Show<P: Platform, C = NoChild> {
  shown: Box<dyn Fn() -> bool>,
  child: C,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform, C> std::fmt::Debug for Show<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Show").finish_non_exhaustive()
  }
}

impl<P: Platform> Show<P> {
  /// A `Show` with no child yet, shown until told otherwise.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      shown: Box::new(|| true),
      child: NoChild,
      platform: PhantomData,
    }
  }
}

impl<P: Platform> Default for Show<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Show<P> {
  /// Sets whether the child is shown.
  #[inline(always)]
  pub fn set_shown(mut self, shown: bool) -> Self {
    self.shown = Box::new(move || shown);

    self
  }

  /// Shows the child while `shown` returns `true`, re-evaluated whenever a signal it
  /// reads changes.
  pub fn derive_shown(mut self, shown: impl Fn() -> bool + 'static) -> Self {
    self.shown = Box::new(shown);

    self
  }

  /// Sets the child.
  pub fn set_child(self, child: impl View<P>) -> Show<P, AnyView<P>> {
    Show {
      shown: self.shown,
      child: child.into_any(),
      platform: PhantomData,
    }
  }
}

impl<P: Platform, C> AnyKind for Show<P, C> {}

impl<P: Platform> View<P> for Show<P, AnyView<P>> {
  fn build(self) -> NodeId {
    let segment = Segment::<P>::new();
    let start = segment.start();
    let scope = Owner::current().child();
    let shown = self.shown;
    let child = Cell::new(Some(self.child));
    let piece = Cell::new(None::<Piece>);

    Effect::render(move |previous: Option<bool>| {
      let visible = shown();

      if previous == Some(visible) {
        return visible;
      }

      match (visible, piece.get()) {
        (true, Some(held)) => {
          segment.mount(held);
          held.resume();
        }
        (true, None) => {
          if let Some(child) = child.take() {
            let built = segment.build(scope, || untrack(|| child.build()));

            piece.set(Some(built));
            segment.mount(built);
          }
        }
        (false, Some(shown)) => {
          shown.pause();
          segment.unmount(shown);
        }
        (false, None) => {}
      }

      visible
    });

    start
  }
}

/// What a facade's [`Show`] carries: its constructor and its condition.
pub trait ShowBase<P: Platform>: Wraps<Inner = Show<P>> {
  /// A `Show` with no child yet, shown until told otherwise.
  #[must_use]
  fn new() -> Self {
    Self::wrap(Show::new())
  }

  /// Sets whether the child is shown.
  #[must_use]
  fn set_shown(self, shown: bool) -> Self {
    Self::wrap(self.into_inner().set_shown(shown))
  }

  /// Shows the child while `shown` returns `true`, re-evaluated whenever a signal it
  /// reads changes.
  #[must_use]
  fn derive_shown(self, shown: impl Fn() -> bool + 'static) -> Self {
    Self::wrap(self.into_inner().derive_shown(shown))
  }
}
