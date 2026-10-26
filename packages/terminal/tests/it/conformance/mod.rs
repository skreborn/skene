//! The conformance suite in the terminal: its driver, and the suite's pages in
//! skene-terminal's vocabulary ([`pages`]).

mod pages;

use std::time::Duration;

use skene_conformance::{Driver, PAGES, Probes, Reading};
use skene_foundation::{KeyEvent, NodeId, Realization, Role, Scene, Size, View};
use skene_terminal::{
  platform::terminal::{Terminal, install, mount, uninstall},
  testing,
};

/// The terminal driven as a user drives it, a frame painted at its size before every act
/// so what it acts on is on screen.
struct TerminalDriver {
  /// The terminal's size, in cells.
  size: (u16, u16),
  /// What the open page publishes.
  probes: Probes,
}

impl TerminalDriver {
  /// A driver over a platform installed for it, 80 cells by 24.
  fn new() -> Self {
    install();

    Self {
      size: (80, 24),
      probes: Probes::default(),
    }
  }

  /// Paints a frame.
  fn paint(&self) {
    let _ = testing::render_to_text(self.size.0, self.size.1);
  }
}

impl Drop for TerminalDriver {
  fn drop(&mut self) {
    uninstall();
  }
}

impl Driver for TerminalDriver {
  type Node = NodeId;
  type Scalar = i32;

  fn open(&mut self, page: &str) {
    let page = pages::page(page).unwrap_or_else(|| panic!("no page named {page:?}"));
    let mut probes = Probes::default();
    let _ = mount(|| (page.build)(&mut probes).build());

    self.probes = probes;
    self.paint();
  }

  fn probe(&mut self, name: &str) -> String {
    self
      .probes
      .read(name)
      .unwrap_or_else(|| panic!("the page publishes no probe {name:?}"))
  }

  fn find(&mut self, role: Role, name: &str) -> Option<NodeId> {
    self.paint();

    testing::find_by_role(role, name)
  }

  fn find_text(&mut self, text: &str) -> Option<NodeId> {
    self.paint();

    testing::find_by_text(text)
  }

  fn read(&mut self, node: &NodeId) -> Reading {
    self.paint();

    Reading::of_tree(&testing::access_tree(), *node)
  }

  fn hover(&mut self, node: &NodeId) {
    self.paint();
    testing::user::hover(*node);
  }

  fn click(&mut self, node: &NodeId) {
    self.paint();
    testing::user::click(*node);
    self.paint();
  }

  fn hold(&mut self, node: &NodeId) {
    self.paint();
    testing::user::hold(*node);
    self.paint();
  }

  fn release(&mut self, node: &NodeId) {
    self.paint();
    testing::user::release(*node);
    self.paint();
  }

  fn press(&mut self, key: &KeyEvent) {
    testing::user::press_key(key);
    self.paint();
  }

  fn type_text(&mut self, node: &NodeId, text: &str) {
    self.paint();
    testing::user::type_text(*node, text);
    self.paint();
  }

  fn focus(&mut self, node: &NodeId) {
    Terminal::focus(*node);
    self.paint();
  }

  fn resize(&mut self, size: Size<i32>) {
    self.size = (
      u16::try_from(size.width.max(0)).unwrap_or(u16::MAX),
      u16::try_from(size.height.max(0)).unwrap_or(u16::MAX),
    );
    testing::trigger_resize(self.size.0, self.size.1);
    self.paint();
  }

  fn advance(&mut self, duration: Duration) {
    testing::trigger_frame(duration);
  }

  fn focused(&mut self) -> Option<NodeId> {
    testing::focused_node()
  }

  fn focus_shows(&mut self) -> bool {
    Scene::<Terminal>::read(|scene| {
      scene
        .focused()
        .is_some_and(|focused| scene.focus_shows(focused))
    })
  }

  fn is_pressed(&mut self, node: &NodeId) -> bool {
    Scene::<Terminal>::read(|scene| scene.is_pressed(*node))
  }
}

#[test]
fn the_pages_are_the_suites() {
  let names: Vec<&str> = pages::pages().iter().map(|page| page.name).collect();

  assert_eq!(names, PAGES);
}

#[test]
fn the_terminal_passes_the_suite() {
  skene_conformance::run(TerminalDriver::new);
}
