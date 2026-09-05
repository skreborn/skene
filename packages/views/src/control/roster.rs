//! The members a grouping control tracks, in tree order.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_core::{Driver, NodeId, Scene};
use skene_reactive::Trigger;

use crate::realize::with_sink;

/// The members a grouping control tracks: a radio group's choices, a listbox's options.
///
/// A member joins as it builds and leaves as it is disposed; order comes from the scene,
/// not from the order members joined, so a set that reconciles stays in tree order and a
/// destroyed member is absent because the walk cannot reach it. Membership is reactive:
/// [`ordered`](Self::ordered) tracks every join, leave and rooting, so a focus order or
/// state derived from it re-evaluates when the set changes.
#[derive(Clone)]
#[must_use]
pub struct Roster {
  members: Rc<RefCell<Vec<(NodeId, String)>>>,
  root: Rc<Cell<Option<NodeId>>>,
  changed: Trigger,
}

impl Roster {
  /// A roster with no members and no root, its trigger owned by the current owner.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      members: Rc::new(RefCell::new(Vec::new())),
      root: Rc::new(Cell::new(None)),
      changed: Trigger::new(),
    }
  }

  /// Sets the node whose subtree holds the members, which orders them; until one is set
  /// the members come in join order.
  pub fn root_at(&self, node: NodeId) {
    self.root.set(Some(node));
    self.changed.notify();
  }

  /// Adds the member `node` holding `value`, replacing any member already at that node.
  pub fn join(&self, node: NodeId, value: impl Into<String>) {
    {
      let mut members = self.members.borrow_mut();

      members.retain(|(held, _)| *held != node);
      members.push((node, value.into()));
    }

    self.changed.notify();
  }

  /// Removes the member at `node`.
  pub fn leave(&self, node: NodeId) {
    self.members.borrow_mut().retain(|(held, _)| *held != node);
    self.changed.notify();
  }

  /// Focuses the member at `index` in tree order, if there is one.
  pub fn focus_at<D: Driver>(&self, index: usize) {
    if let Some((node, _)) = self.ordered::<D>().get(index) {
      with_sink::<D, _>(*node, |sink| D::default().focus(sink));
    }
  }

  /// Focuses the member holding `value`, or the first member when none holds it.
  pub fn focus_value<D: Driver>(&self, value: Option<&str>) {
    let members = self.ordered::<D>();
    let index = value
      .and_then(|value| members.iter().position(|(_, held)| held == value))
      .unwrap_or(0);

    if let Some((node, _)) = members.get(index) {
      with_sink::<D, _>(*node, |sink| D::default().focus(sink));
    }
  }

  /// The members in tree order under `D`'s scene, tracked.
  #[must_use]
  pub fn ordered<D: Driver>(&self) -> Vec<(NodeId, String)> {
    self.changed.track();

    let members = self.members.borrow();
    let Some(root) = self.root.get()
    else {
      return members.clone();
    };

    Scene::<D>::with(|scene| {
      let mut ordered = Vec::with_capacity(members.len());
      let mut pending = vec![root];

      while let Some(node) = pending.pop() {
        if let Some((_, value)) = members.iter().find(|(held, _)| *held == node) {
          ordered.push((node, value.clone()));
        }

        let children: Vec<NodeId> = scene.children(node).collect();

        pending.extend(children.into_iter().rev());
      }

      ordered
    })
  }
}

impl Default for Roster {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}
