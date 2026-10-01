//! A view in one of two forms: a fallback until a signal says the environment is
//! ready, and the child from then on.

use std::cell::RefCell;

use skene_reactive::{Get, Memo, ReadSignal, untrack};

use crate::{
  AnyKind,
  AnyView,
  NoChild,
  NodeId,
  Platform,
  Scene,
  View,
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
/// A form builds untracked, so a signal it reads while building never rebuilds it.
/// With no fallback the segment stays empty until the signal turns. Holds [`NoChild`]
/// until [`set_child_builder`](Self::set_child_builder) supplies the child; both forms
/// are builder closures because only one of them is built.
///
/// The signals the built-ins wait on are a platform's own: the web facade's
/// `media::hydrated` for a render made without script, and
/// [`Painting::available`](crate::Painting::available) for a painted box's pixels.
#[derive(Debug)]
#[must_use]
pub struct Enhance<P: Platform, C = NoChild> {
  live: ReadSignal<bool>,
  fallback: Option<ChildBuilder<P>>,
  child: C,
}

impl<P: Platform> Enhance<P> {
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

impl<P: Platform> Enhance<P> {
  /// Sets the form built while `live` is false; nothing by default.
  pub fn set_fallback<V: View<P>>(mut self, fallback: impl FnOnce() -> V + 'static) -> Self {
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

impl<P: Platform, C> AnyKind for Enhance<P, C> {}

impl<P: Platform> View<P> for Enhance<P, ChildBuilder<P>> {
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

/// Implements a facade's newtype `$name<C>` over the foundation's [`Enhance`].
///
/// The newtype holds the view on the platform `$platform` in its field; the macro
/// implements the constructor, both forms and the traits an enhancement meets.
#[macro_export]
macro_rules! impl_enhance {
  ($name:ident, $platform:ty) => {
    impl<C: ::core::fmt::Debug> ::core::fmt::Debug for $name<C> {
      fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        ::core::fmt::Debug::fmt(&self.0, formatter)
      }
    }
    impl $name {
      /// An enhancement waiting on `live`, with no forms yet.
      #[inline(always)]
      pub const fn new(live: $crate::reactive::ReadSignal<bool>) -> Self {
        Self($crate::Enhance::new(live))
      }

      /// Sets the form built while `live` is false; nothing by default.
      #[inline(always)]
      pub fn set_fallback<V: $crate::View<$platform>>(
        self,
        fallback: impl FnOnce() -> V + 'static,
      ) -> Self {
        Self(self.0.set_fallback(fallback))
      }

      /// Sets the form built once `live` is true.
      #[inline(always)]
      pub fn set_child_builder<V: $crate::View<$platform>>(
        self,
        child: impl FnOnce() -> V + 'static,
      ) -> $name<$crate::ChildBuilder<$platform>> {
        $name(self.0.set_child_builder(child))
      }
    }

    impl<C> $crate::AnyKind for $name<C> {}

    impl $crate::View<$platform> for $name<$crate::ChildBuilder<$platform>> {
      #[inline(always)]
      fn build(self) -> $crate::NodeId {
        $crate::View::build(self.0)
      }
    }
  };
}
