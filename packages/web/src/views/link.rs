//! A navigation link: a box with a destination, upgraded to router navigation when
//! running.

use skene_foundation::{AnyView, LinkBase, NoChild, View, Wraps, routing::Route};

use crate::ActivePlatform;

/// A link to a route.
///
/// An anchor whose destination is the route's path, so a page without JavaScript
/// navigates by full page load, and a running app navigates the
/// [`Router`](skene_foundation::routing::Router) instead, on a click or on `Enter`.
/// Holds [`NoChild`] until [`set_child`](Self::set_child) supplies the one view it
/// links.
///
/// A modified click or `Enter`—control, shift, alt, or command held—is left to the
/// browser, so open-in-new-tab and friends keep working.
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Link<R: Route, C = NoChild>(skene_foundation::Link<ActivePlatform, R, C>);

impl<R: Route> LinkBase<ActivePlatform, R> for Link<R> {}

impl<R: Route> Link<R> {
  /// Sets the view the link holds.
  #[inline(always)]
  pub fn set_child(self, child: impl View<ActivePlatform>) -> Link<R, AnyView<ActivePlatform>> {
    Link(self.0.set_child(child))
  }
}
