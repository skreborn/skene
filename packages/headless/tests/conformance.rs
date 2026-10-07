//! The conformance suite on the headless platform.

use std::time::Duration;

use skene_conformance::{Driver, Probes, Reading};
use skene_foundation::{Container, HasChild, KeyEvent, NodeId, Role, Scene, Size, View};
use skene_headless::{
  Headless,
  access_tree,
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
  /// What the open page publishes.
  probes: Probes,
}

impl HeadlessDriver {
  /// A driver over a platform reset for it.
  fn new() -> Self {
    reset();

    Self {
      owner: None,
      probes: Probes::default(),
    }
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
  type Node = NodeId;
  type Scalar = f32;

  fn open(&mut self, page: &str) {
    if let Some(owner) = self.owner.take() {
      owner.dispose();
    }

    let page =
      skene_conformance::page::<Headless>(page).unwrap_or_else(|| panic!("no page named {page:?}"));
    let owner = Owner::new();
    let mut probes = Probes::default();
    let _root = owner.enter(|| {
      Container::<Headless>::new()
        .set_child((page.build)(&mut probes))
        .build()
    });

    self.owner = Some(owner);
    self.probes = probes;
  }

  fn probe(&mut self, name: &str) -> String {
    self
      .probes
      .read(name)
      .unwrap_or_else(|| panic!("the page publishes no probe {name:?}"))
  }

  fn find(&mut self, role: Role, name: &str) -> Option<NodeId> {
    find_by_role(role, name)
  }

  fn find_text(&mut self, text: &str) -> Option<NodeId> {
    find_by_text(text)
  }

  fn read(&mut self, node: &NodeId) -> Reading {
    Reading::of_tree(&access_tree(), *node)
  }

  fn hover(&mut self, node: &NodeId) {
    user::hover(*node);
  }

  fn click(&mut self, node: &NodeId) {
    user::click(*node);
  }

  fn hold(&mut self, node: &NodeId) {
    user::hold(*node);
  }

  fn release(&mut self, node: &NodeId) {
    user::release(*node);
  }

  fn press(&mut self, key: &KeyEvent) {
    user::press_key(key);
  }

  fn type_text(&mut self, node: &NodeId, text: &str) {
    user::type_text(*node, text);
  }

  fn focus(&mut self, node: &NodeId) {
    trigger_focus(*node);
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

  fn focus_shows(&mut self) -> bool {
    Scene::<Headless>::read(|scene| {
      scene
        .focused()
        .is_some_and(|focused| scene.focus_shows(focused))
    })
  }

  fn is_pressed(&mut self, node: &NodeId) -> bool {
    Scene::<Headless>::read(|scene| scene.is_pressed(*node))
  }
}

#[test]
fn the_headless_platform_passes_the_suite() {
  skene_conformance::run(HeadlessDriver::new);
}
