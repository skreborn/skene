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
  NoChild,
  NodeId,
  Prevent,
  Realization,
  Scene,
  View,
  Wraps,
  routing::{Route, Router},
};

/// A link to a route.
///
/// A box whose destination is the route's path, so where the platform has real links a
/// page without script follows it by loading that path, and a running app navigates
/// the [`Router`] instead: on a click, or on `Enter` while it holds focus, the link
/// being a tab stop. Holds [`NoChild`] until [`set_child`](Self::set_child) supplies the
/// one view it links.
///
/// A modified click or `Enter`—control, shift, alt, or command held—is left to the
/// platform, so open-in-new-tab and friends keep working.
#[derive(Debug)]
#[must_use]
pub struct Link<P: Realization, R: Route, C = NoChild> {
  route: R,
  inner: LayoutBox<P>,
  child: C,
}

impl<P: Realization, R: Route> Link<P, R> {
  /// A link to `route`.
  #[inline(always)]
  pub fn new(route: R) -> Self {
    let inner = LayoutBox::new(Layout::Document).set_destination(route.to_path());

    Self {
      route,
      inner,
      child: NoChild,
    }
  }

  /// Sets the view the link holds.
  pub fn set_child(self, child: impl View<P>) -> Link<P, R, AnyView<P>> {
    Link {
      route: self.route,
      inner: self.inner,
      child: child.into_any(),
    }
  }
}

impl<P: Realization, R: Route> AnyKind for Link<P, R, AnyView<P>> {}

impl<P: Realization, R: Route> View<P> for Link<P, R, AnyView<P>> {
  fn build(self) -> NodeId {
    let node = self.inner.add_child(self.child).build();
    // Captured while building, where an ancestor's `provide` is in scope; an event
    // arrives outside any scope. No router leaves the link to the platform.
    let router = Router::<R>::current();
    let follow = Rc::new(move |modifiers: Modifiers| {
      let Some(router) = router
      else {
        return Prevent::None;
      };

      if modifiers != Modifiers::NONE {
        return Prevent::None;
      }

      router.navigate(self.route.clone());

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

/// What a facade's [`Link`] carries: its constructor.
pub trait LinkBase<P: Realization, R: Route>: Wraps<Inner = Link<P, R>> {
  /// A link to `route`.
  #[must_use]
  fn new(route: R) -> Self {
    Self::wrap(Link::new(route))
  }
}
