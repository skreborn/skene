//! The members a grouping control tracks, in tree order.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use rustc_hash::FxHashMap;
use skene_reactive::{Memo, Trigger};

use super::{AnyChoice, AnyChosen, TypeAhead};
use crate::{ChoiceOption, NodeId, Realization, Scene};

/// The members a grouping control tracks: a radio group's choices, a listbox's options.
///
/// A member joins as it builds and leaves as it is disposed; order comes from the
/// scene, not from the order members joined, so a set that reconciles stays in tree
/// order and a destroyed member is absent. Each member goes by a key, its ordinal: the
/// order it joined the roster in, which two builds of one tree give alike, so a
/// platform's scriptless form names an option by it; it never changes while the member
/// stays. Membership is reactive:
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
/// label supplied one, whether its control is disabled, and its ordinal.
#[derive(Clone)]
struct Member {
  node: NodeId,
  value: AnyChoice,
  name: Option<Rc<dyn Fn() -> String>>,
  disabled: Rc<dyn Fn() -> bool>,
  joined: u64,
}

impl Member {
  /// The name the member goes by now: its label's, else none.
  fn name(&self) -> Option<String> {
    self.name.as_ref().map(|name| name())
  }

  /// The key the member goes by: its ordinal.
  fn key(&self) -> Rc<str> {
    Rc::from(self.joined.to_string())
  }
}

impl Roster {
  /// A roster with no members and no root, its trigger owned by the current owner.
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

  /// The node whose subtree holds the members, once one is set.
  #[inline(always)]
  #[must_use]
  pub fn root(&self) -> Option<NodeId> {
    self.root.get()
  }

  /// Adds the member `node` holding `value`, going by the name `name` reads and
  /// disabled while `disabled` answers so, replacing any member already at that node;
  /// answers the key it goes by.
  pub fn join(
    &self,
    node: NodeId,
    value: AnyChoice,
    name: Option<Rc<dyn Fn() -> String>>,
    disabled: Rc<dyn Fn() -> bool>,
  ) -> Rc<str> {
    let joined = self.joins.get();
    let member = Member {
      node,
      value,
      name,
      disabled,
      joined,
    };
    let key = member.key();

    self.joins.set(joined + 1);
    self.members.borrow_mut().insert(node, member);
    self.changed.notify();

    key
  }

  /// Removes the member at `node`.
  pub fn leave(&self, node: NodeId) {
    self.members.borrow_mut().remove(&node);
    self.changed.notify();
  }

  /// Focuses the enabled member at `index` among the [`enabled`](Self::enabled), if
  /// there is one.
  pub fn focus_at<P: Realization>(&self, index: usize) {
    if let Some((node, _)) = self.enabled::<P>().get(index) {
      P::focus(*node);
    }
  }

  /// Focuses the first enabled member `chosen` holds, or the first enabled member when
  /// it holds none.
  pub fn focus_chosen<P: Realization>(&self, chosen: &AnyChosen) {
    let members = self.enabled::<P>();
    let index = members
      .iter()
      .position(|(_, held)| chosen.contains(held))
      .unwrap_or(0);

    if let Some((node, _)) = members.get(index) {
      P::focus(*node);
    }
  }

  /// The members whose control is enabled, in tree order under `P`'s scene, tracked.
  #[must_use]
  pub fn enabled<P: Realization>(&self) -> Vec<(NodeId, AnyChoice)> {
    self
      .walk::<P>()
      .into_iter()
      .filter(|member| !(member.disabled)())
      .map(|member| (member.node, member.value))
      .collect()
  }

  /// The members in tree order under `P`'s scene, tracked.
  #[must_use]
  pub fn ordered<P: Realization>(&self) -> Vec<(NodeId, AnyChoice)> {
    self
      .walk::<P>()
      .into_iter()
      .map(|member| (member.node, member.value))
      .collect()
  }

  /// The members as the options of a choice, each keyed by its ordinal and shown by its
  /// name (empty where its label supplied none), in tree order under `P`'s scene,
  /// tracked.
  #[must_use]
  pub fn options<P: Realization>(&self) -> Vec<ChoiceOption> {
    self
      .walk::<P>()
      .into_iter()
      .map(|member| {
        ChoiceOption::new(member.key(), member.name().unwrap_or_default())
          .set_disabled((member.disabled)())
      })
      .collect()
  }

  /// The key of the first member to join whose value `chosen` holds, tracked; none where
  /// it holds no member's.
  #[must_use]
  pub fn key_of(&self, chosen: &AnyChosen) -> Option<Rc<str>> {
    self.changed.track();

    self
      .members
      .borrow()
      .values()
      .filter(|member| chosen.contains(&member.value))
      .min_by_key(|member| member.joined)
      .map(Member::key)
  }

  /// The value of the member going by `key`, tracked; none where no member does.
  #[must_use]
  pub fn value_of(&self, key: &str) -> Option<AnyChoice> {
    self.changed.track();

    let joined = key.parse::<u64>().ok()?;

    self
      .members
      .borrow()
      .values()
      .find(|member| member.joined == joined)
      .map(|member| member.value.clone())
  }

  /// The index among the [`enabled`](Self::enabled) of the member a type-ahead
  /// `buffer` picks by name (see [`TypeAhead::matching`]), given the first member whose
  /// value `current` answers for has focus or is chosen; a member whose label supplied
  /// no name is passed over.
  #[must_use]
  pub fn typed<P: Realization>(
    &self,
    buffer: &str,
    current: impl Fn(&AnyChoice) -> bool,
  ) -> Option<usize> {
    let members: Vec<Member> = self
      .walk::<P>()
      .into_iter()
      .filter(|member| !(member.disabled)())
      .collect();
    let names: Vec<Option<String>> = members.iter().map(Member::name).collect();
    let current = members.iter().position(|member| current(&member.value));

    TypeAhead::matching(buffer, &names, current)
  }

  /// The group's one tab stop as a memo: the first enabled member whose value `chosen`
  /// holds, else the first enabled member, in tree order under `P`'s scene; a member
  /// compares its value against it.
  pub fn tab_stop<P: Realization>(
    &self,
    chosen: impl Fn() -> AnyChosen + 'static,
  ) -> Memo<Option<AnyChoice>> {
    let roster = self.clone();

    Memo::new(move |_| {
      let enabled = roster.enabled::<P>();
      let chosen = chosen();

      enabled
        .iter()
        .find(|(_, value)| chosen.contains(value))
        .or_else(|| enabled.first())
        .map(|(_, value)| value.clone())
    })
  }

  fn walk<P: Realization>(&self) -> Vec<Member> {
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
