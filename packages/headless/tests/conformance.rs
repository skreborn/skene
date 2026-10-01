//! The conformance suite on the headless platform.

use std::time::Duration;

use skene_conformance::Driver;
use skene_foundation::{AnyView, Container, HasChild, KeyEvent, NodeId, Role, Size, View};
use skene_headless::{
  Headless,
  access_outline,
  find_by_role,
  find_by_text,
  focused_node,
  reset,
  trigger_focus,
  trigger_frame,
  trigger_resize,
  user,
};
use skene_reactive::Owner;

/// The headless platform driven as a user drives it, the page mounted under a root box as
/// every platform mounts it.
struct HeadlessDriver {
  /// The owner of what is mounted.
  owner: Option<Owner>,
}

impl HeadlessDriver {
  /// A driver over a platform reset for it.
  fn new() -> Self {
    reset();

    Self { owner: None }
  }
}

impl Drop for HeadlessDriver {
  fn drop(&mut self) {
    if let Some(owner) = self.owner.take() {
      owner.dispose();
    }
  }
}

impl Driver for HeadlessDriver {
  type Platform = Headless;

  fn mount(&mut self, build: impl FnOnce() -> AnyView<Headless> + 'static) {
    if let Some(owner) = self.owner.take() {
      owner.dispose();
    }

    let owner = Owner::new();
    let _root = owner.enter(|| Container::<Headless>::new().set_child(build()).build());

    self.owner = Some(owner);
  }

  fn find(&mut self, role: Role, name: &str) -> Option<NodeId> {
    find_by_role(role, name)
  }

  fn find_text(&mut self, text: &str) -> Option<NodeId> {
    find_by_text(text)
  }

  fn hover(&mut self, node: NodeId) {
    user::hover(node);
  }

  fn click(&mut self, node: NodeId) {
    user::click(node);
  }

  fn press(&mut self, key: &KeyEvent) {
    user::press_key(key);
  }

  fn type_text(&mut self, node: NodeId, text: &str) {
    user::type_text(node, text);
  }

  fn focus(&mut self, node: NodeId) {
    trigger_focus(node);
  }

  fn resize(&mut self, size: Size<f32>) {
    trigger_resize(size);
  }

  fn advance(&mut self, duration: Duration) {
    trigger_frame(duration);
  }

  fn focused(&mut self) -> Option<NodeId> {
    focused_node()
  }

  fn access_outline(&mut self) -> String {
    access_outline()
  }
}

#[test]
fn the_headless_platform_passes_the_suite() {
  skene_conformance::run(HeadlessDriver::new);
}
