//! View transitions: the opt-in that animates a navigation between two states of the
//! page.

/// Opts the app into view transitions.
///
/// In the browser every navigation of the router installed by `install_router`
/// commits under
/// `document.startViewTransition` where the browser has it, and a box named through
/// `set_transition_name` is paired with its namesake across the change. A served or
/// generated page carries `@view-transition { navigation: auto }`, so the
/// cross-document navigations between pages animate the same way. A browser without
/// view transitions, and the host platform, cut.
#[cfg_attr(
  skene_gui_platform = "headless",
  expect(
    clippy::missing_const_for_fn,
    reason = "the web leaves' body is not const, and one function has one constness"
  )
)]
pub fn enable() {
  #[cfg(not(skene_gui_platform = "headless"))]
  skene_web::enable_view_transitions();
}
