//! A link to a route: a box with a destination, navigating the router when followed.

use std::rc::Rc;

use crate::{
  AnyKind,
  AnyView,
  ClickEvent,
  Handler,
  HasChildren,
  KeyEvent,
  Layout,
  LayoutBox,
  Modifiers,
  NamedKey,
  NodeId,
  Prevent,
  Realization,
  Scene,
  Thin,
  View,
  Wraps,
  empty,
  routing::{Route, Router},
};

/// A link to a route.
///
/// A box whose destination is the route's path, so where the platform has real links a
/// page without script follows it by loading that path, and a running app navigates
/// the [`Router`] instead: on a click, or on `Enter` while it holds focus, the link
/// being a tab stop. Without the one view [`set_child`](Self::set_child) supplies, it
/// renders nothing.
///
/// A modified click or `Enter`—control, shift, alt, or command held—is left to the
/// platform, so open-in-new-tab and friends keep working.
#[derive(Debug)]
#[must_use]
pub struct Link<P: Realization, R: Route> {
  route: R,
  inner: LayoutBox<P>,
  child: Option<AnyView<P>>,
}

impl<P: Realization, R: Route> Link<P, R> {
  /// A link to `route`.
  #[inline(always)]
  pub fn new(route: R) -> Self {
    let inner = LayoutBox::new(Layout::Document).set_destination(route.to_path());

    Self {
      route,
      inner,
      child: None,
    }
  }

  /// Sets the view the link holds.
  pub fn set_child(mut self, child: impl View<P>) -> Self {
    self.child = Some(child.into_any());

    self
  }
}

impl<P: Realization, R: Route> AnyKind for Link<P, R> {}

impl<P: Realization, R: Route> Thin<P> for Link<P, R> {
  type Core = LinkCore<P>;

  fn into_core(self) -> LinkCore<P> {
    let route = self.route;

    LinkCore {
      inner: self.inner,
      child: self.child,
      follow: Box::new(move || {
        let router = Router::<R>::current()?;

        Some(Box::new(move || router.navigate(route.clone())))
      }),
    }
  }
}

impl<P: Realization, R: Route> View<P> for Link<P, R> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.into_core().build()
  }
}

/// How a link follows its destination: called while it builds, where an ancestor's
/// router is in scope, it returns the navigation, or `None` without a router.
type Follow = Box<dyn FnOnce() -> Option<Box<dyn Fn()>>>;

/// A [`Link`] with its route's type erased: what every link builds, whatever its
/// route, the link thin over it.
///
/// It holds the box carrying the destination's path, the view inside it, and the
/// navigation to the destination through the router in scope when it builds.
#[must_use]
pub struct LinkCore<P: Realization> {
  inner: LayoutBox<P>,
  child: Option<AnyView<P>>,
  follow: Follow,
}

impl<P: Realization> std::fmt::Debug for LinkCore<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("LinkCore").finish_non_exhaustive()
  }
}

impl<P: Realization> AnyKind for LinkCore<P> {}

impl<P: Realization> View<P> for LinkCore<P> {
  fn build(self) -> NodeId {
    let Some(child) = self.child
    else {
      return empty::<P>().build();
    };
    let node = self.inner.add_child(child).build();
    // Captured while building, where an ancestor's `provide` is in scope; an event
    // arrives outside any scope. No router leaves the link to the platform.
    let navigate = (self.follow)();
    let follow = Rc::new(move |modifiers: Modifiers| {
      let Some(navigate) = &navigate
      else {
        return Prevent::None;
      };

      if modifiers != Modifiers::NONE {
        return Prevent::None;
      }

      navigate();

      Prevent::Realization
    });

    Scene::<P>::with(|scene| {
      scene.handle(
        node,
        Handler::Click(Rc::new({
          let follow = Rc::clone(&follow);

          move |event: &ClickEvent| follow(event.modifiers)
        })),
      );
      scene.handle(
        node,
        Handler::KeyDown(Rc::new(move |event: &KeyEvent| {
          if event.is(NamedKey::Enter) && !event.composing {
            follow(event.modifiers)
          }
          else {
            Prevent::None
          }
        })),
      );
    });

    node
  }
}

/// What a facade's [`Link`] carries: its constructor and the view it holds.
pub trait LinkBase<P: Realization, R: Route>: Wraps<Inner = Link<P, R>> {
  /// A link to `route`.
  #[must_use]
  fn new(route: R) -> Self {
    Self::wrap(Link::new(route))
  }

  /// Sets the view the link holds.
  #[must_use]
  fn set_child(self, child: impl View<P>) -> Self {
    Self::wrap(self.into_inner().set_child(child))
  }
}
