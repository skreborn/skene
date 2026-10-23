//! A view in one of two forms: a fallback until a signal says the environment is
//! ready, and the child from then on.

use std::cell::RefCell;

use skene_reactive::{Get, Memo, ReadSignal, untrack};

use crate::{
  AnyKind,
  AnyView,
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
/// no fallback the segment stays empty until the signal turns, and with no child
/// [`set_child_builder`](Self::set_child_builder) supplies it stays empty from then on;
/// both forms are builder closures, and only one of them is built.
///
/// The signals the built-ins wait on are a platform's own: the web facade's
/// `media::hydrated` for a render made without script, and
/// [`Painting::available`](crate::Painting::available) for a painted box's pixels.
#[derive(Debug)]
#[must_use]
pub struct Enhance<P: Realization> {
  live: ReadSignal<bool>,
  fallback: Option<ChildBuilder<P>>,
  child: Option<ChildBuilder<P>>,
}

impl<P: Realization> Enhance<P> {
  /// An enhancement waiting on `live`, with no forms yet.
  #[inline(always)]
  pub const fn new(live: ReadSignal<bool>) -> Self {
    Self {
      live,
      fallback: None,
      child: None,
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
  pub fn set_child_builder<V: View<P>>(mut self, child: impl FnOnce() -> V + 'static) -> Self {
    self.child = Some(ChildBuilder::new(child));

    self
  }
}

impl<P: Realization> AnyKind for Enhance<P> {}

impl<P: Realization> View<P> for Enhance<P> {
  fn build(self) -> NodeId {
    let signal = self.live;
    let live = Memo::new(move |_| signal.get());
    let fallback = RefCell::new(self.fallback);
    let child = RefCell::new(self.child);

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

/// What a facade's [`Enhance`] carries: its constructor and its two forms.
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

  /// Sets the form built once `live` is true.
  #[must_use]
  fn set_child_builder<V: View<P>>(self, child: impl FnOnce() -> V + 'static) -> Self {
    Self::wrap(self.into_inner().set_child_builder(child))
  }
}
