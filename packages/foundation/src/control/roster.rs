//! The members a grouping control tracks, in tree order.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use rustc_hash::FxHashMap;
use skene_reactive::{Memo, Trigger};

use super::TypeAhead;
use crate::{NodeId, PickerOption, Platform, Scene};

/// The members a grouping control tracks: a radio group's choices, a listbox's options.
///
/// A member joins as it builds and leaves as it is disposed; order comes from the
/// scene, not from the order members joined, so a set that reconciles stays in tree
/// order and a destroyed member is absent. Membership is reactive:
/// [`ordered`](Self::ordered) tracks every join, leave and rooting, so a focus order or
/// state derived from it re-evaluates when the set changes.
#[derive(Clone)]
#[must_use]
pub struct Roster {
  members: Rc<RefCell<FxHashMap<NodeId, Member>>>,
  joins: Rc<Cell<u64>>,
  root: Rc<Cell<Option<NodeId>>>,
  changed: Trigger,
}

impl std::fmt::Debug for Roster {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Roster").finish_non_exhaustive()
  }
}

/// One member: its node, the value it carries, the name it goes by when its control's
/// label supplied one, whether its control is disabled, and when it joined.
#[derive(Clone)]
struct Member {
  node: NodeId,
  value: String,
  name: Option<Rc<dyn Fn() -> String>>,
  disabled: Rc<dyn Fn() -> bool>,
  joined: u64,
}

impl Member {
  /// The name the member goes by now: its label's, else its value.
  fn name(&self) -> String {
    self
      .name
      .as_ref()
      .map_or_else(|| self.value.clone(), |name| name())
  }
}

impl Roster {
  /// A roster with no members and no root, its trigger owned by the current owner.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      members: Rc::new(RefCell::new(FxHashMap::default())),
      joins: Rc::new(Cell::new(0)),
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

  /// Adds the member `node` holding `value`, going by the name `name` reads and
  /// disabled while `disabled` answers so, replacing any member already at that node.
  pub fn join(
    &self,
    node: NodeId,
    value: impl Into<String>,
    name: Option<Rc<dyn Fn() -> String>>,
    disabled: Rc<dyn Fn() -> bool>,
  ) {
    let joined = self.joins.get();

    self.joins.set(joined + 1);
    self.members.borrow_mut().insert(
      node,
      Member {
        node,
        value: value.into(),
        name,
        disabled,
        joined,
      },
    );
    self.changed.notify();
  }

  /// Removes the member at `node`.
  pub fn leave(&self, node: NodeId) {
    self.members.borrow_mut().remove(&node);
    self.changed.notify();
  }

  /// Focuses the enabled member at `index` among the [`enabled`](Self::enabled), if
  /// there is one.
  pub fn focus_at<P: Platform>(&self, index: usize) {
    if let Some((node, _)) = self.enabled::<P>().get(index) {
      P::focus(*node);
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
      P::focus(*node);
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

  /// The members as a platform-drawn picker's options, in tree order under `P`'s scene,
  /// tracked; a member's value is its name where its label supplied none.
  #[must_use]
  pub fn options<P: Platform>(&self) -> Vec<PickerOption> {
    self
      .walk::<P>()
      .into_iter()
      .map(|member| PickerOption {
        name: member.name(),
        disabled: (member.disabled)(),
        value: member.value,
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
    let names: Vec<String> = members.iter().map(Member::name).collect();
    let current =
      current.and_then(|current| members.iter().position(|member| member.value == current));

    TypeAhead::matching(buffer, &names, current)
  }

  /// The group's one tab stop as a memo: the member holding the value `chosen` names
  /// where that member is enabled, else the first enabled member, in tree order under
  /// `P`'s scene; a member compares its value against it.
  pub fn tab_stop<P: Platform>(
    &self,
    chosen: impl Fn() -> Option<String> + 'static,
  ) -> Memo<Option<String>> {
    let roster = self.clone();

    Memo::new(move |_| {
      let enabled = roster.enabled::<P>();

      chosen()
        .filter(|chosen| enabled.iter().any(|(_, value)| value == chosen))
        .or_else(|| enabled.into_iter().next().map(|(_, value)| value))
    })
  }

  fn walk<P: Platform>(&self) -> Vec<Member> {
    self.changed.track();

    let members = self.members.borrow();
    let Some(root) = self.root.get()
    else {
      let mut joined: Vec<Member> = members.values().cloned().collect();

      joined.sort_by_key(|member| member.joined);

      return joined;
    };

    Scene::<P>::with(|scene| {
      let mut ordered = Vec::with_capacity(members.len());
      let mut pending = vec![root];

      while let Some(node) = pending.pop() {
        if let Some(member) = members.get(&node) {
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
