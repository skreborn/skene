//! A navigation link: a box navigating the router when followed.

use skene_foundation::{LinkBase, View, Wraps, routing::Route};

use crate::Active;

/// A link to a route.
///
/// A tab stop that navigates the [`Router`](skene_foundation::routing::Router) on a
/// click or on `Enter`, as a browser's link does. Without the one view
/// [`set_child`](Self::set_child) supplies, it renders nothing.
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
pub struct Link<R: Route>(skene_foundation::Link<Active, R>);

impl<R: Route> LinkBase<Active, R> for Link<R> {}
