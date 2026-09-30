//! A navigation link: a box with a destination, upgraded to router navigation when
//! running.

use std::rc::Rc;

use skene_common::{
  LayoutBox,
  routing::{Route, Router},
};
use skene_core::{
  AnyKind,
  Children,
  ClickEvent,
  Handler,
  HasChildren,
  Layout,
  Modifiers,
  NodeId,
  PlainChild,
  Prevent,
  Scene,
  Seal,
  View,
};

use crate::ActivePlatform;

/// A link to a route.
///
/// A box whose destination is the route's path, so a page without JavaScript navigates
/// by full page load, and a running app intercepts the click into [`Router::navigate`]
/// instead. Takes its content through [`add_child`](HasChildren::add_child).
///
/// A modified click—control, shift, alt, or command held—is left to the platform, so
/// open-in-new-tab and friends keep working.
#[derive(Debug)]
#[must_use]
pub struct Link<R: Route> {
  route: R,
  inner: LayoutBox<ActivePlatform>,
}

impl<R: Route> Link<R> {
  /// Creates a link to `route`.
  #[inline(always)]
  pub fn new(route: R) -> Self {
    let inner = LayoutBox::new(Layout::Flow).set_destination(route.to_path());

    Self { route, inner }
  }
}

impl<R: Route> HasChildren<ActivePlatform> for Link<R> {
  type Kind = PlainChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<ActivePlatform, PlainChild> {
    self.inner.children_mut(seal)
  }
}

impl<R: Route> AnyKind for Link<R> {}

impl<R: Route> View<ActivePlatform> for Link<R> {
  fn build(self) -> NodeId {
    let node = self.inner.build();
    let route = self.route;
    // Captured while building, where an ancestor's `provide` is in scope; a click
    // arrives outside any scope. No router leaves the anchor plain.
    let router = Router::<R>::current();

    Scene::<ActivePlatform>::with(|scene| {
      scene.handle(
        node,
        Handler::Click(Rc::new(move |event: &ClickEvent| {
          if event.modifiers != Modifiers::NONE {
            return Prevent::None;
          }

          let Some(router) = router
          else {
            return Prevent::None;
          };

          router.navigate(route.clone());

          Prevent::Platform
        })),
      );
    });

    node
  }
}
