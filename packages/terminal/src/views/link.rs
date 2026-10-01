//! A navigation link: a box navigating the router when followed.

use skene_foundation::{LinkBase, View, Wraps, routing::Route};

use crate::ActivePlatform;

/// A link to a route.
///
/// A tab stop that navigates the [`Router`](skene_foundation::routing::Router) on a
/// click or on `Enter`, as a browser's link does. Takes its content through
/// [`add_child`](skene_foundation::HasChildren::add_child).
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Link<R: Route>(skene_foundation::Link<ActivePlatform, R>);

impl<R: Route> LinkBase<ActivePlatform, R> for Link<R> {}
