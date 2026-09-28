//! The members a grouping control tracks, in tree order.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_core::{NodeId, Platform, Scene};
use skene_reactive::Trigger;

use super::TypeAhead;
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
  members: Rc<RefCell<Vec<Member>>>,
  root: Rc<Cell<Option<NodeId>>>,
  changed: Trigger,
}

/// One member: its node, the value it carries, the name it goes by when its control's
/// label supplied one, and whether its control is disabled.
#[derive(Clone)]
struct Member {
  node: NodeId,
  value: String,
  name: Option<String>,
  disabled: Rc<dyn Fn() -> bool>,
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

  /// Adds the member `node` holding `value`, going by `name` and disabled while
  /// `disabled` answers so, replacing any member already at that node.
  pub fn join(
    &self,
    node: NodeId,
    value: impl Into<String>,
    name: Option<String>,
    disabled: Rc<dyn Fn() -> bool>,
  ) {
    {
      let mut members = self.members.borrow_mut();

      members.retain(|member| member.node != node);
      members.push(Member {
        node,
        value: value.into(),
        name,
        disabled,
      });
    }

    self.changed.notify();
  }

  /// Removes the member at `node`.
  pub fn leave(&self, node: NodeId) {
    self
      .members
      .borrow_mut()
      .retain(|member| member.node != node);
    self.changed.notify();
  }

  /// Focuses the enabled member at `index` among the [`enabled`](Self::enabled), if
  /// there is one.
  pub fn focus_at<P: Platform>(&self, index: usize) {
    if let Some((node, _)) = self.enabled::<P>().get(index) {
      with_sink::<P, _>(*node, |sink| P::default().focus(sink));
    }
  }

  /// Focuses the enabled member holding `value`, or the first enabled member when none
  /// does.
  pub fn focus_value<P: Platform>(&self, value: Option<&str>) {
    let members = self.enabled::<P>();
    let index = value
      .and_then(|value| members.iter().position(|(_, held)| held == value))
      .unwrap_or(0);

    if let Some((node, _)) = members.get(index) {
      with_sink::<P, _>(*node, |sink| P::default().focus(sink));
    }
  }

  /// The members whose control is enabled, in tree order under `P`'s scene, tracked.
  #[must_use]
  pub fn enabled<P: Platform>(&self) -> Vec<(NodeId, String)> {
    self
      .walk::<P>()
      .into_iter()
      .filter(|member| !(member.disabled)())
      .map(|member| (member.node, member.value))
      .collect()
  }

  /// The members in tree order under `P`'s scene, tracked.
  #[must_use]
  pub fn ordered<P: Platform>(&self) -> Vec<(NodeId, String)> {
    self
      .walk::<P>()
      .into_iter()
      .map(|member| (member.node, member.value))
      .collect()
  }

  /// The members' values and names in tree order under `P`'s scene, tracked; a
  /// member's value is its name where its label supplied none.
  #[must_use]
  pub fn named<P: Platform>(&self) -> Vec<(String, String)> {
    self
      .walk::<P>()
      .into_iter()
      .map(|member| {
        let name = member.name.unwrap_or_else(|| member.value.clone());

        (member.value, name)
      })
      .collect()
  }

  /// The index among the [`enabled`](Self::enabled) of the member a type-ahead
  /// `buffer` picks by name (see [`TypeAhead::matching`]), given the member holding
  /// `current` has focus or is chosen; a member's value is its name where its label
  /// supplied none.
  #[must_use]
  pub fn typed<P: Platform>(&self, buffer: &str, current: Option<&str>) -> Option<usize> {
    let members: Vec<Member> = self
      .walk::<P>()
      .into_iter()
      .filter(|member| !(member.disabled)())
      .collect();
    let names: Vec<String> = members
      .iter()
      .map(|member| member.name.clone().unwrap_or_else(|| member.value.clone()))
      .collect();
    let current =
      current.and_then(|current| members.iter().position(|member| member.value == current));

    TypeAhead::matching(buffer, &names, current)
  }

  fn walk<P: Platform>(&self) -> Vec<Member> {
    self.changed.track();

    let members = self.members.borrow();
    let Some(root) = self.root.get()
    else {
      return members.clone();
    };

    Scene::<P>::with(|scene| {
      let mut ordered = Vec::with_capacity(members.len());
      let mut pending = vec![root];

      while let Some(node) = pending.pop() {
        if let Some(member) = members.iter().find(|member| member.node == node) {
          ordered.push(member.clone());
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
