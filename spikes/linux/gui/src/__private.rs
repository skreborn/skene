//! What `#[skene_gui::main]` and `#[skene_gui::test]` expand to call. Not part of the API.

use std::process::ExitCode;

use skene_foundation::seam::{
  Realization as _,
  node::{LayoutKind, NodeKind, NodeSpec},
  style::ScopeChain,
  tree,
};

use crate::{
  Active, Theme,
  view::{Cx, View},
};

/// Checks, at compile time, that the app's root returns a view.
pub const fn assert_app<V: View>(_: fn() -> V) {}

/// Builds `app` under a root box, the node a window shows, beneath the platform's colors
/// ([`skene_foundation::SystemColors`]) as the root theme.
pub(crate) fn root<V: View>(app: impl FnOnce() -> V) -> skene_foundation::NodeId {
  let root = tree::create::<Active>(
    NodeSpec::new(NodeKind::Layout(LayoutKind::Box)),
    &ScopeChain::root(),
    None,
  );
  let node = Theme::derive(Active::system_colors)
    .set_child(app())
    .build(&mut Cx::root());

  tree::attach::<Active>(root, node.0, 0);

  root
}

/// Runs the app on this build's platform, as `app_id`.
pub fn start<V: View>(app_id: &str, app: fn() -> V) -> ExitCode {
  Active::start(app_id, Box::new(move || root(app)))
}

/// Runs a test's body on the realization's window thread, under an owner of its own, the
/// environment and the reader's settings reset first.
pub fn test(name: &'static str, body: fn()) {
  skene_gui_linux::testing::run_test(name, body);
}
