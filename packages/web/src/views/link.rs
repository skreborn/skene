//! A navigation link: a box with a destination, upgraded to router navigation when
//! running.

use skene_foundation::{LinkBase, View, Wraps, routing::Route};

use crate::ActivePlatform;

/// A link to a route.
///
/// An anchor whose destination is the route's path, so a page without JavaScript
/// navigates by full page load, and a running app navigates the
/// [`Router`](skene_foundation::routing::Router) instead, on a click or on `Enter`.
/// Takes its content through
/// [`add_child`](skene_foundation::HasChildren::add_child).
///
/// A modified click or `Enter`—control, shift, alt, or command held—is left to the
/// browser, so open-in-new-tab and friends keep working.
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Link<R: Route>(skene_foundation::Link<ActivePlatform, R>);

impl<R: Route> LinkBase<ActivePlatform, R> for Link<R> {}
