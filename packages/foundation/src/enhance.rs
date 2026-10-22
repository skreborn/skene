//! A view in one of two forms: a fallback until a signal says the environment is
//! ready, and the child from then on.

use std::cell::RefCell;

use skene_reactive::{Get, Memo, ReadSignal, untrack};

use crate::{
  AnyKind,
  AnyView,
  NoChild,
  NodeId,
  Realization,
  Scene,
  View,
  Wraps,
  derived,
  realize::ChildBuilder,
};

/// Builds one of two forms between its anchors: the fallback while `live` is false,
/// the child once it is true.
///
/// Only the form in force is built, and each at most once: the fallback stands where
/// the signal never turns, and where it turns later the fallback is destroyed and the
/// child built in its place, under a replacement owner, so state bound beneath keeps
/// its snapshot identity. Once built, the child stands whatever the signal does after.
/// A form builds untracked, so a signal it reads while building never rebuilds it. With
/// no fallback the segment stays empty until the signal turns. Holds [`NoChild`] until
/// [`set_child_builder`](Self::set_child_builder) supplies the child; both forms are
/// builder closures, and only one of them is built.
///
/// The signals the built-ins wait on are a platform's own: the web facade's
/// `media::hydrated` for a render made without script, and
/// [`Painting::available`](crate::Painting::available) for a painted box's pixels.
#[derive(Debug)]
#[must_use]
pub struct Enhance<P: Realization, C = NoChild> {
  live: ReadSignal<bool>,
  fallback: Option<ChildBuilder<P>>,
  child: C,
}

impl<P: Realization> Enhance<P> {
  /// An enhancement waiting on `live`, with no forms yet.
  #[inline(always)]
  pub const fn new(live: ReadSignal<bool>) -> Self {
    Self {
      live,
      fallback: None,
      child: NoChild,
    }
  }
}

impl<P: Realization> Enhance<P> {
  /// Sets the form built while `live` is false; nothing by default.
  pub fn set_fallback_builder<V: View<P>>(
    mut self,
    fallback: impl FnOnce() -> V + 'static,
  ) -> Self {
    self.fallback = Some(ChildBuilder::new(fallback));

    self
  }

  /// Sets the form built once `live` is true.
  pub fn set_child_builder<V: View<P>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Enhance<P, ChildBuilder<P>> {
    Enhance {
      live: self.live,
      fallback: self.fallback,
      child: ChildBuilder::new(child),
    }
  }
}

impl<P: Realization, C> AnyKind for Enhance<P, C> {}

impl<P: Realization> View<P> for Enhance<P, ChildBuilder<P>> {
  fn build(self) -> NodeId {
    let signal = self.live;
    let live = Memo::new(move |_| signal.get());
    let fallback = RefCell::new(self.fallback);
    let child = RefCell::new(Some(self.child));

    derived::<P>(move || {
      // Once live, the signal is read untracked, so nothing rebuilds the child.
      let form = if untrack(|| live.get()) {
        child.borrow_mut().take()
      }
      else {
        let _ = live.get();

        fallback.borrow_mut().take()
      };

      untrack(|| {
        form.map_or_else(
          || AnyView::<P>::new(|| Scene::<P>::create_segment().0).build(),
          ChildBuilder::build,
        )
      })
    })
  }
}

/// What a facade's [`Enhance`] carries: its constructor and its fallback.
pub trait EnhanceBase<P: Realization>: Wraps<Inner = Enhance<P>> {
  /// An enhancement waiting on `live`, with no forms yet.
  #[must_use]
  fn new(live: ReadSignal<bool>) -> Self {
    Self::wrap(Enhance::new(live))
  }

  /// Sets the form built while `live` is false; nothing by default.
  #[must_use]
  fn set_fallback_builder<V: View<P>>(self, fallback: impl FnOnce() -> V + 'static) -> Self {
    Self::wrap(self.into_inner().set_fallback_builder(fallback))
  }
}
