//! A navigation link: a real anchor, upgraded to router navigation when running.

use std::{cell::Cell, rc::Rc};

use skene_core::{IntoView, Modifiers, NodeId, Prevent, View};
use skene_router::{Route, Router};

use crate::driver;

/// A link to a route: a real anchor carrying the route's path, so a page without
/// JavaScript navigates by full page load, and a running app intercepts the click
/// into [`Router::navigate`] instead.
///
/// A modified click—control, shift, alt, or command held—is left to the browser, so
/// open-in-new-tab and friends keep working.
#[must_use]
pub struct Link<R: Route> {
  route: R,
  child: Option<Box<dyn FnOnce() -> NodeId>>,
}

impl<R: Route> Link<R> {
  /// Creates a link to `route`; the anchor shows [`set_child`](Self::set_child).
  #[inline(always)]
  pub fn new(route: R) -> Self {
    Self { route, child: None }
  }

  /// Sets the link's content.
  #[inline(always)]
  pub fn set_child<V: IntoView>(mut self, child: impl FnOnce() -> V + 'static) -> Self {
    self.child = Some(Box::new(move || child().into_view().build()));

    self
  }
}

impl<R: Route> View for Link<R> {
  fn build(self) -> NodeId {
    let node = driver::create_element("a");

    driver::set_attribute(node, "href", &self.route.to_path());

    if let Some(child) = self.child {
      driver::attach(node, child(), None);
    }

    let held = Rc::new(Cell::new(Modifiers::default()));

    driver::handle_pointer_down(node, {
      let held = Rc::clone(&held);

      Rc::new(move |event: &skene_core::PointerEvent| {
        held.set(event.modifiers);

        Prevent::None
      })
    });

    let route = self.route;
    // Captured while building, where an ancestor's `provide` is in scope; a click
    // arrives outside any scope. No router leaves the anchor plain.
    let router = Router::<R>::get();

    driver::handle_click(
      node,
      Rc::new(move || {
        let modifiers = held.replace(Modifiers::default());

        if modifiers.ctrl || modifiers.shift || modifiers.alt || modifiers.meta {
          return Prevent::None;
        }

        let Some(router) = router.clone()
        else {
          return Prevent::None;
        };

        router.navigate(route.clone());

        Prevent::Default
      }),
    );

    node
  }
}
