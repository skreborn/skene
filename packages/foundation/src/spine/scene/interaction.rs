//! The interaction the scene holds: the hovered, pressed and focused nodes, the
//! handlers nodes carry, and what a key does once its handlers leave it.

use smallvec::SmallVec;

use super::{Mirror, NodeRecord, Scene};
use crate::spine::{
  Focus,
  Handler,
  Interaction,
  KeyDefault,
  KeyEvent,
  NamedKey,
  NodeId,
  Platform,
  handler::HandlerSet,
};

impl<P: Platform> Scene<P> {
  /// A count that moves whenever the interaction a node resolves against may have
  /// changed: its hover, press, focus or drag, or its control's state.
  #[inline(always)]
  #[must_use]
  pub const fn interaction_generation(&self) -> u64 {
    self.interaction_generation
  }

  /// The deepest node under the pointer.
  #[inline(always)]
  #[must_use]
  pub const fn hovered(&self) -> Option<NodeId> {
    self.hovered
  }

  /// Moves the pointer over `next`, the deepest node under it, returning the nodes it
  /// left, innermost first, and those it entered, outermost first.
  pub fn move_pointer(
    &mut self,
    next: Option<NodeId>,
  ) -> (SmallVec<[NodeId; 16]>, SmallVec<[NodeId; 16]>) {
    let previous = std::mem::replace(&mut self.hovered, next);

    if previous != next {
      self.interaction_generation += 1;
    }

    let left = self.crossed(previous, next);
    let mut entered = self.crossed(next, previous);

    entered.reverse();

    (left, entered)
  }

  /// Whether `node` is under the pointer or holds the node that is, as `:hover` holds.
  #[must_use]
  pub fn is_hovered(&self, node: NodeId) -> bool {
    self
      .hovered
      .is_some_and(|hovered| self.ancestry(hovered).contains(&node))
  }

  /// The deepest node a primary press landed on, until its release.
  #[inline(always)]
  #[must_use]
  pub const fn pressed(&self) -> Option<NodeId> {
    self.pressed
  }

  /// Holds a primary press on `node`, or ends the press for `None`.
  pub fn set_pressed(&mut self, node: Option<NodeId>) {
    if self.pressed != node {
      self.pressed = node;
      self.interaction_generation += 1;
    }
  }

  /// Whether a primary press is held on `node` or a node beneath it, as `:active` holds.
  #[must_use]
  pub fn is_pressed(&self, node: NodeId) -> bool {
    self
      .pressed
      .is_some_and(|pressed| self.ancestry(pressed).contains(&node))
  }

  /// The node holding focus.
  #[inline(always)]
  #[must_use]
  pub const fn focused(&self) -> Option<NodeId> {
    self.focused
  }

  /// Moves focus to `next`, returning the nodes whose subtree it left, innermost first,
  /// and those whose subtree it reached, outermost first.
  pub fn move_focus(
    &mut self,
    next: Option<NodeId>,
  ) -> (SmallVec<[NodeId; 16]>, SmallVec<[NodeId; 16]>) {
    let previous = std::mem::replace(&mut self.focused, next);

    if previous != next {
      self.interaction_generation += 1;
    }

    let left = self.crossed(previous, next);
    let mut reached = self.crossed(next, previous);

    reached.reverse();

    (left, reached)
  }

  /// Whether focus shows while held: set by a key, cleared by a pointer press.
  #[inline(always)]
  #[must_use]
  pub const fn focus_visible(&self) -> bool {
    self.focus_visible
  }

  /// Records whether focus shows while held.
  #[inline(always)]
  pub const fn set_focus_visible(&mut self, visible: bool) {
    if self.focus_visible != visible {
      self.focus_visible = visible;
      self.interaction_generation += 1;
    }
  }

  /// Records that the control `node` is a text entry, whose focus shows however it was
  /// reached, as `:focus-visible` does on an `<input>`.
  pub fn note_entry(&mut self, node: NodeId) {
    if let Some(record) = self.record_mut(node) {
      record.entry = true;
      self.interaction_generation += 1;
    }
  }

  /// Whether `node` holds focus that shows: reached by a key and not since a pointer
  /// press, or held by a text entry.
  #[must_use]
  pub fn focus_shows(&self, node: NodeId) -> bool {
    self.focused == Some(node)
      && (self.focus_visible || self.record(node).is_some_and(|record| record.entry))
  }

  /// The interaction `node`'s conditions resolve against: its own hover, the focus and
  /// press of the control enclosing it or of itself, and that control's state.
  pub fn interaction(&self, node: NodeId) -> Interaction {
    let control = self.enclosing_control(node);
    let holder = control.unwrap_or(node);

    Interaction {
      hovered: self.is_hovered(node),
      focus: Focus::new(self.focused == Some(holder), self.focus_shows(holder)),
      pressed: self.is_pressed(holder),
      dragged: self.dragged() == Some(node),
      targeted: self.drag_target() == Some(node),
      control: control.and_then(|control| self.control_state(control)),
    }
  }

  /// What `event` does once its handlers leave it to the platform, as a browser's
  /// defaults do: `Tab` traverses, `Escape` dismisses the topmost open overlay, and the
  /// menu key or `Shift-F10` asks for a context menu; `None` for every other key.
  pub fn key_default(&self, event: &KeyEvent) -> Option<KeyDefault> {
    if event.is(NamedKey::Tab) {
      return Some(KeyDefault::Traverse {
        back: event.modifiers.shift,
      });
    }

    if event.is(NamedKey::Escape) {
      return self.open_overlays.last().copied().map(KeyDefault::Dismiss);
    }

    (event.is(NamedKey::ContextMenu) || (event.is(NamedKey::F10) && event.modifiers.shift))
      .then_some(KeyDefault::ContextMenu)
  }

  /// Adds `handler` to `node`: it runs as the current owner when an event of its kind
  /// reaches the node, after the node's earlier handlers of that kind.
  pub fn handle(&mut self, node: NodeId, handler: Handler) {
    let Some(record) = self.record_mut(node)
    else {
      return;
    };
    let kind = handler.kind();

    record.handlers.get_or_insert_default().push(handler);
    self.mirrors.push(Mirror::Listen { node, kind });
  }

  /// The handlers `node` carries.
  #[inline(always)]
  #[must_use]
  pub(in crate::spine) fn handlers(&self, node: NodeId) -> Option<&HandlerSet> {
    self.record(node).and_then(NodeRecord::handlers)
  }
}

#[cfg(test)]
mod tests {
  use crate::spine::testing::{TestBox, TestPlatform, TestScene};

  /// Focus shows after a key and always on a text entry, a press holds along its
  /// ancestors, and a destroyed node's focus is gone, the scene resolving each node's
  /// interaction from them.
  #[test]
  fn the_scene_holds_focus_press_and_their_rules() {
    TestPlatform::reset();

    let root = TestBox::node();
    let [button, entry] = std::array::from_fn(|_| TestBox::node());

    TestScene::with(|scene| {
      scene.attach(root, button, None);
      scene.attach(root, entry, None);
      scene.note_entry(entry);
      scene.set_focus_visible(false);

      let _ = scene.move_focus(Some(button));
    });

    assert!(
      TestScene::with(|scene| !scene.focus_shows(button)),
      "focus a press gave"
    );

    TestScene::with(|scene| {
      let _ = scene.move_focus(Some(entry));

      scene.set_pressed(Some(entry));
    });

    assert!(
      TestScene::with(|scene| scene.focus_shows(entry)),
      "an entry's focus shows"
    );
    assert!(TestScene::with(|scene| scene.interaction(root).pressed));
    assert!(TestScene::with(|scene| !scene.interaction(button).pressed));

    TestScene::with(|scene| scene.destroy(entry));

    assert_eq!(TestScene::with(|scene| scene.focused()), None);
    assert_eq!(TestScene::with(|scene| scene.pressed()), None);
  }
}
