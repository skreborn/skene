//! A navigation link: a box with a destination, upgraded to router navigation when
//! running.

use skene_foundation::{LinkBase, View, Wraps, routing::Route};

use crate::Active;

/// A link to a route.
///
/// An anchor whose destination is the route's path, so a page without JavaScript
/// navigates by full page load, and a running app navigates the
/// [`Router`](skene_foundation::routing::Router) instead, on a click or on `Enter`.
/// Without the one view [`set_child`](Self::set_child) supplies, it renders nothing.
///
/// A modified click or `Enter`—control, shift, alt, or command held—is left to the
/// browser, so open-in-new-tab and friends keep working.
///
/// ```
/// use skene_web::{prelude::*, router::Route};
///
/// #[derive(Clone, PartialEq)]
/// enum Page {
///   Home,
///   About,
/// }
///
/// impl Route for Page {
///   fn parse(path: &str) -> Option<Self> {
///     match path {
///       "/" => Some(Self::Home),
///       "/about" => Some(Self::About),
///       _ => None,
///     }
///   }
///
///   fn to_path(&self) -> String {
///     match self {
///       Self::Home => "/".to_owned(),
///       Self::About => "/about".to_owned(),
///     }
///   }
/// }
///
/// let about = Link::new(Page::About).set_child(Text::new("About"));
/// ```
#[derive(Debug, View, Wraps)]
#[view(core = skene_foundation::LinkCore<Active>)]
#[must_use]
pub struct Link<R: Route>(skene_foundation::Link<Active, R>);

impl<R: Route> LinkBase<Active, R> for Link<R> {}
