//! The conformance suite in the terminal.

use std::time::Duration;

use skene_conformance::Driver;
use skene_core::{AnyView, KeyEvent, NodeId, Platform, Role, Size, View};
use skene_tui::{self as tui, Tui, testing};

/// The terminal driven as a user drives it, a frame painted at its size before every act
/// so what it acts on is on screen.
struct TerminalDriver {
  /// The terminal's size, in cells.
  size: (u16, u16),
}

impl TerminalDriver {
  /// A driver over a platform installed for it, 80 cells by 24.
  fn new() -> Self {
    tui::install();

    Self { size: (80, 24) }
  }

  /// Paints a frame.
  fn paint(&self) {
    let _ = testing::render_to_text(self.size.0, self.size.1);
  }
}

impl Drop for TerminalDriver {
  fn drop(&mut self) {
    tui::uninstall();
  }
}

impl Driver for TerminalDriver {
  type Platform = Tui;

  fn mount(&mut self, build: impl FnOnce() -> AnyView<Tui> + 'static) {
    let _ = tui::mount(move || build().build());

    self.paint();
  }

  fn find(&mut self, role: Role, name: &str) -> Option<NodeId> {
    self.paint();

    testing::find_by_role(role, name)
  }

  fn find_text(&mut self, text: &str) -> Option<NodeId> {
    self.paint();

    testing::find_by_text(text)
  }

  fn hover(&mut self, node: NodeId) {
    self.paint();
    testing::user::hover(node);
  }

  fn click(&mut self, node: NodeId) {
    self.paint();
    testing::user::click(node);
    self.paint();
  }

  fn press(&mut self, key: &KeyEvent) {
    testing::user::press_key(key);
    self.paint();
  }

  fn type_text(&mut self, node: NodeId, text: &str) {
    self.paint();
    testing::user::type_text(node, text);
    self.paint();
  }

  fn focus(&mut self, node: NodeId) {
    Tui::focus(node);
    self.paint();
  }

  fn resize(&mut self, size: Size<u32>) {
    self.size = (
      u16::try_from(size.width).unwrap_or(u16::MAX),
      u16::try_from(size.height).unwrap_or(u16::MAX),
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

  fn access_outline(&mut self) -> String {
    self.paint();

    testing::access_outline()
  }
}

#[test]
fn the_terminal_passes_the_suite() {
  skene_conformance::run(TerminalDriver::new);
}
