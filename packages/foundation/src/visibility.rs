//! A child built once and shown or hidden without rebuilding.

use std::{cell::Cell, marker::PhantomData};

use skene_reactive::{Effect, Owner, untrack};

use crate::{AnyKind, AnyView, NodeId, Piece, Realization, Segment, View, Wraps, empty};

/// Shows or hides one child, preserving its state.
///
/// The child is built the first time it shows, then attached or detached between a
/// pair of anchors when the condition changes, and left where it stands while it
/// holds. While hidden its effects pause, catching up once it shows again.
///
/// Without the child [`set_child`](Self::set_child) supplies, it renders nothing.
#[must_use]
pub struct Visibility<P: Realization> {
  shown: Box<dyn Fn() -> bool>,
  child: Option<AnyView<P>>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization> std::fmt::Debug for Visibility<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Visibility").finish_non_exhaustive()
  }
}

impl<P: Realization> Visibility<P> {
  /// A `Visibility` with no child yet, shown until told otherwise.
  pub fn new() -> Self {
    Self {
      shown: Box::new(|| true),
      child: None,
      platform: PhantomData,
    }
  }
}

impl<P: Realization> Default for Visibility<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Visibility<P> {
  /// Sets whether the child is shown.
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
  pub fn set_child(mut self, child: impl View<P>) -> Self {
    self.child = Some(child.into_any());

    self
  }
}

impl<P: Realization> AnyKind for Visibility<P> {}

impl<P: Realization> View<P> for Visibility<P> {
  fn build(self) -> NodeId {
    let Some(child) = self.child
    else {
      return empty::<P>().build();
    };
    let segment = Segment::<P>::new();
    let start = segment.start();
    let scope = Owner::current().child();
    let shown = self.shown;
    let child = Cell::new(Some(child));
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

/// What a facade's [`Visibility`] carries: its constructor, its condition and its child.
pub trait VisibilityBase<P: Realization>: Wraps<Inner = Visibility<P>> {
  /// A `Visibility` with no child yet, shown until told otherwise.
  #[must_use]
  fn new() -> Self {
    Self::wrap(Visibility::new())
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

  /// Sets the child.
  #[must_use]
  fn set_child(self, child: impl View<P>) -> Self {
    Self::wrap(self.into_inner().set_child(child))
  }
}
