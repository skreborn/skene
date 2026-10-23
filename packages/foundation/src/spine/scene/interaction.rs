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
  Realization,
  dispatch::Crossing,
  handler::HandlerSet,
};

/// The interaction the scene holds, as plain state: the hovered, pressed and focused
/// nodes, whether focus shows, and a generation that moves whenever what a node resolves
/// its interaction to may have changed.
///
/// What holds along a node's ancestors reads them through an `ancestry`, a node and the
/// nodes it stands in, itself first, as [`Scene::ancestry`] gives it.
#[derive(Debug, Clone)]
pub(in crate::spine) struct Held {
  hovered: Option<NodeId>,
  pressed: Option<NodeId>,
  focused: Option<NodeId>,
  focus_visible: bool,
  generation: u64,
}

impl Held {
  /// Nothing held, focus showing once held.
  pub(in crate::spine) const fn new() -> Self {
    Self {
      hovered: None,
      pressed: None,
      focused: None,
      focus_visible: true,
      generation: 0,
    }
  }

  /// The generation of the interaction held.
  #[inline(always)]
  pub(in crate::spine) const fn generation(&self) -> u64 {
    self.generation
  }

  /// Moves the generation, for a change outside what this holds: a drag, a control's
  /// state, a node becoming an entry.
  #[inline(always)]
  pub(in crate::spine) const fn touch(&mut self) {
    self.generation += 1;
  }

  /// The deepest node under the pointer.
  #[inline(always)]
  pub(in crate::spine) const fn hovered(&self) -> Option<NodeId> {
    self.hovered
  }

  /// The deepest node a primary press holds.
  #[inline(always)]
  pub(in crate::spine) const fn pressed(&self) -> Option<NodeId> {
    self.pressed
  }

  /// The node holding focus.
  #[inline(always)]
  pub(in crate::spine) const fn focused(&self) -> Option<NodeId> {
    self.focused
  }

  /// Whether focus shows while held.
  #[inline(always)]
  pub(in crate::spine) const fn focus_visible(&self) -> bool {
    self.focus_visible
  }

  /// Moves the pointer over `next`, returning the node it was over.
  pub(in crate::spine) fn hover(&mut self, next: Option<NodeId>) -> Option<NodeId> {
    self.replace(|held| &mut held.hovered, next)
  }

  /// Holds a primary press on `node`, or ends it for `None`.
  pub(in crate::spine) fn press(&mut self, node: Option<NodeId>) {
    self.replace(|held| &mut held.pressed, node);
  }

  /// Moves focus to `next`, returning the node that held it.
  pub(in crate::spine) fn focus(&mut self, next: Option<NodeId>) -> Option<NodeId> {
    self.replace(|held| &mut held.focused, next)
  }

  /// Records whether focus shows while held.
  pub(in crate::spine) const fn set_focus_visible(&mut self, visible: bool) {
    if self.focus_visible != visible {
      self.focus_visible = visible;
      self.generation += 1;
    }
  }

  /// Forgets `node`, destroyed: it is no longer hovered, pressed or focused.
  pub(in crate::spine) fn forget(&mut self, node: NodeId) {
    for held in [&mut self.hovered, &mut self.pressed, &mut self.focused] {
      if *held == Some(node) {
        *held = None;
        self.generation += 1;
      }
    }
  }

  /// Whether `node` is under the pointer or holds the node that is, as `:hover` holds;
  /// `ancestry` gives the hovered node's.
  pub(in crate::spine) fn hovers(
    &self,
    node: NodeId,
    ancestry: impl FnOnce(NodeId) -> SmallVec<[NodeId; 16]>,
  ) -> bool {
    self
      .hovered
      .is_some_and(|hovered| ancestry(hovered).contains(&node))
  }

  /// Whether a primary press is held on `node` or a node beneath it, as `:active` holds;
  /// `ancestry` gives the pressed node's.
  pub(in crate::spine) fn presses(
    &self,
    node: NodeId,
    ancestry: impl FnOnce(NodeId) -> SmallVec<[NodeId; 16]>,
  ) -> bool {
    self
      .pressed
      .is_some_and(|pressed| ancestry(pressed).contains(&node))
  }

  /// Whether `node` holds focus that shows: reached by a key and not since a pointer
  /// press, or held by a text entry, which `entry` says it is.
  pub(in crate::spine) fn focus_shows(&self, node: NodeId, entry: bool) -> bool {
    self.focused == Some(node) && (self.focus_visible || entry)
  }

  /// Puts `next` in the slot `slot` picks, moving the generation where it changes, and
  /// returns what it held.
  fn replace(
    &mut self,
    slot: impl FnOnce(&mut Self) -> &mut Option<NodeId>,
    next: Option<NodeId>,
  ) -> Option<NodeId> {
    let previous = std::mem::replace(slot(self), next);

    if previous != next {
      self.generation += 1;
    }

    previous
  }
}

impl<P: Realization> Scene<P> {
  /// A count that moves whenever the interaction a node resolves against may have
  /// changed: its hover, press, focus or drag, or its control's state.
  #[inline(always)]
  #[must_use]
  pub const fn interaction_generation(&self) -> u64 {
    self.held.generation()
  }

  /// The deepest node under the pointer.
  #[inline(always)]
  #[must_use]
  pub const fn hovered(&self) -> Option<NodeId> {
    self.held.hovered()
  }

  /// Moves the pointer over `next`, the deepest node under it, returning the nodes it
  /// left, innermost first, and those it entered, outermost first.
  pub fn move_pointer(
    &mut self,
    next: Option<NodeId>,
  ) -> (SmallVec<[NodeId; 16]>, SmallVec<[NodeId; 16]>) {
    let previous = self.held.hover(next);

    self.crossing(previous, next)
  }

  /// Whether `node` is under the pointer or holds the node that is, as `:hover` holds.
  #[must_use]
  pub fn is_hovered(&self, node: NodeId) -> bool {
    self.held.hovers(node, |hovered| self.ancestry(hovered))
  }

  /// The deepest node a primary press landed on, until its release.
  #[inline(always)]
  #[must_use]
  pub const fn pressed(&self) -> Option<NodeId> {
    self.held.pressed()
  }

  /// Holds a primary press on `node`, or ends the press for `None`.
  pub fn set_pressed(&mut self, node: Option<NodeId>) {
    self.held.press(node);
  }

  /// Whether a primary press is held on `node` or a node beneath it, as `:active` holds.
  #[must_use]
  pub fn is_pressed(&self, node: NodeId) -> bool {
    self.held.presses(node, |pressed| self.ancestry(pressed))
  }

  /// The node holding focus.
  #[inline(always)]
  #[must_use]
  pub const fn focused(&self) -> Option<NodeId> {
    self.held.focused()
  }

  /// Moves focus to `next`, returning the nodes whose subtree it left, innermost first,
  /// and those whose subtree it reached, outermost first.
  pub fn move_focus(
    &mut self,
    next: Option<NodeId>,
  ) -> (SmallVec<[NodeId; 16]>, SmallVec<[NodeId; 16]>) {
    let previous = self.held.focus(next);

    self.crossing(previous, next)
  }

  /// The nodes a move from `previous` to `next` leaves, innermost first, and those it
  /// reaches, outermost first.
  fn crossing(
    &self,
    previous: Option<NodeId>,
    next: Option<NodeId>,
  ) -> (SmallVec<[NodeId; 16]>, SmallVec<[NodeId; 16]>) {
    let ancestry = |node: Option<NodeId>| node.map(|node| self.ancestry(node)).unwrap_or_default();
    let crossing = Crossing::between(&ancestry(previous), &ancestry(next));

    (crossing.left, crossing.reached)
  }

  /// Whether focus shows while held: set by a key, cleared by a pointer press.
  #[inline(always)]
  #[must_use]
  pub const fn focus_visible(&self) -> bool {
    self.held.focus_visible()
  }

  /// Records whether focus shows while held.
  #[inline(always)]
  pub const fn set_focus_visible(&mut self, visible: bool) {
    self.held.set_focus_visible(visible);
  }

  /// Records that the control `node` is a text entry, whose focus shows however it was
  /// reached, as `:focus-visible` does on an `<input>`.
  pub fn note_entry(&mut self, node: NodeId) {
    if let Some(record) = self.record_mut(node) {
      record.entry = true;
      self.held.touch();
    }
  }

  /// Whether `node` holds focus that shows: reached by a key and not since a pointer
  /// press, or held by a text entry.
  #[must_use]
  pub fn focus_shows(&self, node: NodeId) -> bool {
    self
      .held
      .focus_shows(node, self.record(node).is_some_and(|record| record.entry))
  }

  /// The interaction `node`'s conditions resolve against: its own hover, the focus and
  /// press of the control enclosing it or of itself, and that control's state.
  pub fn interaction(&self, node: NodeId) -> Interaction {
    let control = self.enclosing_control(node);
    let holder = control.unwrap_or(node);

    Interaction {
      hovered: self.is_hovered(node),
      focus: Focus::new(self.focused() == Some(holder), self.focus_shows(holder)),
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
  use smallvec::SmallVec;

  use super::Held;
  use crate::spine::NodeId;

  /// Focus shows after a key and always on a text entry, a press holds along its
  /// ancestors, and a destroyed node's focus and press are gone, each change moving the
  /// generation once.
  #[test]
  fn the_scene_holds_focus_press_and_their_rules() {
    let [root, button, entry] =
      std::array::from_fn(|index| NodeId::new(u32::try_from(index).unwrap(), 0));
    let ancestry = |node: NodeId| -> SmallVec<[NodeId; 16]> {
      if node == root {
        [root].into_iter().collect()
      }
      else {
        [node, root].into_iter().collect()
      }
    };
    let mut held = Held::new();

    held.set_focus_visible(false);

    assert_eq!(held.focus(Some(button)), None);
    assert!(!held.focus_shows(button, false), "focus a press gave");
    assert!(held.focus_shows(button, true), "an entry's focus shows");
    assert_eq!(held.focus(Some(entry)), Some(button));
    assert!(
      !held.focus_shows(button, false),
      "focus elsewhere shows nowhere"
    );

    let generation = held.generation();

    held.press(Some(entry));
    held.press(Some(entry));

    assert_eq!(
      held.generation(),
      generation + 1,
      "an unchanged press moves nothing"
    );
    assert!(held.presses(root, ancestry));
    assert!(held.presses(entry, ancestry));
    assert!(!held.presses(button, ancestry));

    held.forget(entry);

    assert_eq!(held.focused(), None);
    assert_eq!(held.pressed(), None);
    assert_eq!(held.generation(), generation + 3);
  }
}
