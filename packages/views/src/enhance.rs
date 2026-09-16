//! A view in one of two forms: a fallback until a signal says the environment is
//! ready, and the child from then on.

use std::cell::RefCell;

use skene_core::{AnyKind, AnyView, Driver, IntoView, NoChild, NodeId, Scene, View, derived};
use skene_reactive::{Get, ReadSignal};

use crate::realize::ChildBuilder;

/// Builds one of two forms between its anchors: the fallback while `live` is false,
/// the child once it is true.
///
/// Only the form in force is built, and each at most once: the fallback stands where
/// the signal never turns, and where it turns later the fallback is destroyed and the
/// child built in its place, under a replacement owner, so state bound beneath keeps
/// its snapshot identity. With no fallback the segment stays empty until the signal
/// turns. Holds [`NoChild`] until [`set_child_builder`](Self::set_child_builder)
/// supplies the child; both forms are builder closures because only one of them is
/// built.
///
/// The signals the built-ins wait on are a driver's own: the GUI facade's
/// `media::hydrated` for a render made without script, and
/// [`Painting::available`](crate::Painting::available) for a painted box's pixels.
#[must_use]
pub struct Enhance<D: Driver, C = NoChild> {
  live: ReadSignal<bool>,
  fallback: Option<ChildBuilder<D>>,
  child: C,
}

impl<D: Driver> Enhance<D> {
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

impl<D: Driver, C> Enhance<D, C> {
  /// Sets the form built while `live` is false; nothing by default.
  #[inline(always)]
  pub fn set_fallback<V: IntoView<D>>(mut self, fallback: impl FnOnce() -> V + 'static) -> Self {
    self.fallback = Some(ChildBuilder::new(fallback));

    self
  }

  /// Sets the form built once `live` is true.
  #[inline(always)]
  pub fn set_child_builder<V: IntoView<D>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Enhance<D, ChildBuilder<D>> {
    Enhance {
      live: self.live,
      fallback: self.fallback,
      child: ChildBuilder::new(child),
    }
  }
}

impl<D: Driver, C> AnyKind for Enhance<D, C> {}

impl<D: Driver> View<D> for Enhance<D, ChildBuilder<D>> {
  fn build(self) -> NodeId {
    let live = self.live;
    let fallback = RefCell::new(self.fallback);
    let child = RefCell::new(Some(self.child));

    derived::<D>(move || {
      let form = if live.get() {
        child.borrow_mut().take()
      }
      else {
        fallback.borrow_mut().take()
      };

      form.map_or_else(
        || AnyView::<D>::new(|| Scene::<D>::with(|scene| scene.create_segment().0)).build(),
        ChildBuilder::build,
      )
    })
  }
}
