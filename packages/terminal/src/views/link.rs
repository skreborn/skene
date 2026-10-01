//! A navigation link: a box navigating the router when followed.

use skene_foundation::{AnyView, LinkBase, NoChild, View, Wraps, routing::Route};

use crate::ActivePlatform;

/// A link to a route.
///
/// A tab stop that navigates the [`Router`](skene_foundation::routing::Router) on a
/// click or on `Enter`, as a browser's link does. Holds [`NoChild`] until
/// [`set_child`](Self::set_child) supplies the one view it links.
///
/// ```
/// use skene_terminal::{prelude::*, router::Route};
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
