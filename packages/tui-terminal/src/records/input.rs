//! The input state: focus as the scene holds it, the press awaiting its release, the
//! drag in progress, the wheel's latched target and the hover path.

use skene_foundation::{FocusOrder, NodeId, Scene};

use super::{Kind, Records};
use crate::Terminal;

/// The center of a cell, where a pointer reported by cell alone is taken to be.
pub(crate) const CELL_CENTER: (f32, f32) = (0.5, 0.5);

/// A press-initiated drag in progress, followed by pointer motion until release.
#[derive(Clone, Copy)]
pub(crate) enum Drag {
  /// A scrollbar's thumb: motion scrolls so the grabbed spot follows the pointer.
  Scrollbar {
    node: NodeId,
    axis: skene_foundation::Axis,
    /// The grabbed position within the thumb, in track cells, exact where the
    /// terminal reports pixels.
    grab: f32,
  },
  /// A text selection: motion extends it to the position under the pointer.
  Selection,
  /// A text entry's caret: motion moves it to the position under the pointer, the
  /// anchor held.
  Caret(NodeId),
  /// A press under a node offering drag data: a drag of what the node offers starts
  /// once the pointer leaves the press's cell `from`, and has once `started`.
  Item {
    node: NodeId,
    from: (u16, u16),
    started: bool,
  },
}

/// The pause that ends a wheel gesture: the next wheel event re-targets, as on the
/// web.
pub(crate) const SCROLL_GESTURE: std::time::Duration = std::time::Duration::from_millis(500);

/// The interval between the steps of a caret drag's autoscroll while the pointer is
/// held past the entry's edge; each step scrolls by the pointer's distance past it.
pub(crate) const AUTOSCROLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

/// One wheel gesture's scroll target: the cell it hit, the box it scrolls, and the
/// clock reading of the last wheel event.
pub(crate) struct ScrollLatch {
  pub(crate) cell: (u16, u16),
  pub(crate) target: NodeId,
  pub(crate) last: f64,
}

/// The delivered left press awaiting its release: the node it landed on with its
/// ancestors, the root first, the node it landed on, the node declaring drag axes that
/// captured it, if any, the focusable it landed on, and its place in a run of quick
/// presses at one cell.
pub(crate) struct Press {
  pub(crate) path: Vec<NodeId>,
  pub(crate) pressed: NodeId,
  pub(crate) captured: Option<NodeId>,
  pub(crate) target: Option<NodeId>,
  pub(crate) count: u8,
}

impl Records {
  /// `node`'s place in focus traversal, as the scene holds it.
  #[expect(
    clippy::unused_self,
    reason = "every reader holds the records, and focus is asked of it as of the rest"
  )]
  pub(crate) fn focus_order(&self, node: NodeId) -> Option<FocusOrder> {
    Scene::<Terminal>::read(|scene| scene.focus_order(node))
  }

  /// Whether `node` takes focus at all: by traversal, by pointer, or programmatically.
  pub(crate) fn is_focusable(&self, node: NodeId) -> bool {
    self.focus_order(node).is_some()
  }

  /// Whether a press's pointer drags `node`: a box or a control declaring the axes it
  /// drags along, which keeps a press's pointer until the release.
  pub(crate) fn drags(&self, node: NodeId) -> bool {
    self
      .runtime
      .record(node)
      .and_then(|record| record.kind.box_record())
      .is_some_and(|inner| inner.drag.is_some())
  }

  /// Whether `node` is a control a press's pointer drags.
  pub(crate) fn drags_as_control(&self, node: NodeId) -> bool {
    matches!(
      self.runtime.record(node).map(|record| &record.kind),
      Some(Kind::Control { .. })
    ) && self.drags(node)
  }

  /// The focused link run's node and destination, while a link holds focus.
  pub(crate) fn focused_link(&self) -> Option<(NodeId, String)> {
    let node = self.focused()?;
    let record = self.runtime.record(node)?;

    match &record.kind {
      Kind::Run {
        facts, destination, ..
      } if facts.link => Some((node, destination.clone()?)),
      _ => None,
    }
  }

  /// The node holding focus, as the scene holds it.
  #[expect(
    clippy::unused_self,
    reason = "every reader holds the records, and focus is asked of it as of the rest"
  )]
  pub(crate) fn focused(&self) -> Option<NodeId> {
    Scene::<Terminal>::read(Scene::focused)
  }

  /// Whether `node` holds focus that shows: after a key, or as a text entry, and not
  /// since a pointer press.
  pub(crate) fn focus_shows(&self, node: NodeId) -> bool {
    self.runtime.contains(node) && Scene::<Terminal>::read(|scene| scene.focus_shows(node))
  }

  /// Records whether focus shows, repainting the focused node when that changes.
  pub(crate) fn set_focus_visible(&mut self, visible: bool) {
    let changed = Scene::<Terminal>::with(|scene| {
      let changed = scene.focus_visible() != visible;

      scene.set_focus_visible(visible);

      changed
    });

    if changed && let Some(focused) = self.focused() {
      self.runtime.damage(focused);
    }
  }

  /// The node a press or focus on `node` lands on: itself when focusable, else its
  /// enclosing control when that is; `None` where neither takes focus, as for a disabled
  /// control.
  pub(crate) fn focus_holder(&self, node: NodeId) -> Option<NodeId> {
    if self.is_focusable(node) {
      return Some(node);
    }

    self
      .runtime
      .enclosing_control(node)
      .filter(|control| self.is_focusable(*control))
  }

  /// Whether the app holds the mouse now: taken at launch and not handed to the
  /// terminal since.
  #[inline(always)]
  pub(crate) const fn mouse_taken(&self) -> bool {
    self.mouse && !self.mouse_released
  }

  /// The hover path, the deepest node under the mouse first, up to the root.
  pub(crate) fn hovered(&self) -> Vec<NodeId> {
    Scene::<Terminal>::read(Scene::hovered)
      .map_or_else(Vec::new, |deepest| self.runtime.ancestors(deepest))
  }
}
