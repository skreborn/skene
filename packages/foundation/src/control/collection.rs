//! A collection's behavior and the behavior of each member in it: a radio group, a
//! listbox or a menu, told apart by a [`CollectionPolicy`].

use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use skene_reactive::{Memo, Signal, With, on_cleanup, provide_context, untrack, use_context};

use super::{
  AnyChoice,
  AnyChosen,
  Behavior,
  Choice,
  Chosen,
  ControlHandle,
  ControlledChoice,
  LabelPlacement,
  Roster,
  TypeAhead,
};
use crate::{
  AccessProp,
  CheckState,
  ControlState,
  Controller,
  EventKind,
  FocusOrder,
  Key,
  KeyEvent,
  Membership,
  NamedKey,
  NativeElement,
  Prevent,
  Realization,
  Role,
  contents,
};

/// How a [`Collection`] behaves: what it and its members are, which keys move between
/// the members, and what a move and a choice do.
///
/// Whatever the policy, the members are one tab stop—the chosen member where it is
/// enabled, else the first enabled one—and a move passes over the disabled ones.
pub trait CollectionPolicy: 'static {
  /// The collection's role.
  const ROLE: Role;
  /// The native element a member stands in, where a platform has one.
  const MEMBER_NATIVE: Option<NativeElement>;
  /// A member's role.
  const MEMBER_ROLE: Role;
  /// The events a member acts on.
  const MEMBER_EVENTS: &'static [EventKind];
  /// Whether a move past either end wraps to the other, rather than stopping there.
  const WRAPS: bool;
  /// Whether a move chooses the member it reaches, as a radio group's does, rather than
  /// only focusing it.
  const CHOOSES_ON_MOVE: bool;
  /// Whether the horizontal arrows move as the vertical ones do.
  const HORIZONTAL: bool;
  /// Whether `Home`, `End`, `Enter` and typing act, as they do in a list.
  const LISTS: bool;
  /// Whether a chosen member holds the control-scoped `Checked` state.
  const MARKS_CHOSEN: bool;

  /// What a member reports, `chosen` or not.
  fn member_access(chosen: bool) -> Vec<AccessProp>;
}

/// A radio group's policy: every arrow moves and chooses, wrapping at the ends, `Space`
/// chooses, and a member reports whether it is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RadioPolicy;

impl CollectionPolicy for RadioPolicy {
  const CHOOSES_ON_MOVE: bool = true;
  const HORIZONTAL: bool = true;
  const LISTS: bool = false;
  const MARKS_CHOSEN: bool = true;
  const MEMBER_EVENTS: &'static [EventKind] = &[EventKind::Click, EventKind::KeyDown];
  const MEMBER_NATIVE: Option<NativeElement> = Some(NativeElement::Radio);
  const MEMBER_ROLE: Role = Role::RadioButton;
  const ROLE: Role = Role::RadioGroup;
  const WRAPS: bool = true;

  fn member_access(chosen: bool) -> Vec<AccessProp> {
    vec![AccessProp::Checked(chosen.into())]
  }
}

/// A listbox's policy: the vertical arrows move focus and stop at the ends, `Home`,
/// `End` and typing move too, `Enter` and `Space` choose, and a member reports whether
/// it is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ListboxPolicy;

impl CollectionPolicy for ListboxPolicy {
  const CHOOSES_ON_MOVE: bool = false;
  const HORIZONTAL: bool = false;
  const LISTS: bool = true;
  const MARKS_CHOSEN: bool = true;
  const MEMBER_EVENTS: &'static [EventKind] = &[EventKind::Click, EventKind::KeyDown];
  const MEMBER_NATIVE: Option<NativeElement> = None;
  const MEMBER_ROLE: Role = Role::ListBoxOption;
  const ROLE: Role = Role::ListBox;
  const WRAPS: bool = false;

  fn member_access(chosen: bool) -> Vec<AccessProp> {
    vec![AccessProp::Selected(chosen)]
  }
}

/// A menu's policy: as a listbox's, but a move wraps at the ends, and a member reports
/// no choice: choosing it runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MenuPolicy;

impl CollectionPolicy for MenuPolicy {
  const CHOOSES_ON_MOVE: bool = false;
  const HORIZONTAL: bool = false;
  const LISTS: bool = true;
  const MARKS_CHOSEN: bool = false;
  const MEMBER_EVENTS: &'static [EventKind] = &[EventKind::Click, EventKind::KeyDown];
  const MEMBER_NATIVE: Option<NativeElement> = None;
  const MEMBER_ROLE: Role = Role::MenuItem;
  const ROLE: Role = Role::Menu;
  const WRAPS: bool = true;

  #[inline(always)]
  fn member_access(_chosen: bool) -> Vec<AccessProp> {
    Vec::new()
  }
}

/// What a [`Collection`] shares with the members inside it, through context.
///
/// Members may sit at any depth under the collection and still belong to it.
#[must_use]
pub struct CollectionContext<C: CollectionPolicy> {
  chosen: Rc<dyn Fn() -> AnyChosen>,
  choose: Rc<dyn Fn(AnyChosen)>,
  roster: Roster,
  tab_stop: Memo<Option<AnyChoice>>,
  type_ahead: Rc<TypeAhead>,
  commit_on_tab: bool,
  policy: PhantomData<C>,
}

impl<C: CollectionPolicy> Clone for CollectionContext<C> {
  fn clone(&self) -> Self {
    Self {
      chosen: Rc::clone(&self.chosen),
      choose: Rc::clone(&self.choose),
      roster: self.roster.clone(),
      tab_stop: self.tab_stop,
      type_ahead: Rc::clone(&self.type_ahead),
      commit_on_tab: self.commit_on_tab,
      policy: PhantomData,
    }
  }
}

impl<C: CollectionPolicy> std::fmt::Debug for CollectionContext<C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("CollectionContext")
      .finish_non_exhaustive()
  }
}

impl<C: CollectionPolicy> CollectionContext<C> {
  /// A group whose members join `roster` under `P`'s scene, answering `chosen` for what
  /// is chosen and asking `choose` for a choice; provided as context by a control
  /// holding members, its tab stop a memo owned by the current owner.
  pub fn new<P: Realization>(
    chosen: impl Fn() -> AnyChosen + 'static,
    choose: impl Fn(AnyChosen) + 'static,
    roster: Roster,
  ) -> Self {
    let chosen: Rc<dyn Fn() -> AnyChosen> = Rc::new(chosen);
    let tab_stop = roster.tab_stop::<P>({
      let chosen = Rc::clone(&chosen);

      move || chosen()
    });

    Self {
      chosen,
      choose: Rc::new(choose),
      roster,
      tab_stop,
      type_ahead: Rc::new(TypeAhead::new()),
      commit_on_tab: false,
      policy: PhantomData,
    }
  }

  /// Sets whether `Tab` from a focused member chooses it before focus moves on, as a
  /// picker's list does; off by default.
  #[inline(always)]
  pub const fn set_commit_on_tab(mut self, commit_on_tab: bool) -> Self {
    self.commit_on_tab = commit_on_tab;

    self
  }

  /// What is chosen, tracked.
  #[inline(always)]
  #[must_use]
  pub fn chosen(&self) -> AnyChosen {
    (self.chosen)()
  }

  /// Whether `value` is chosen, tracked.
  #[must_use]
  pub fn is_chosen(&self, value: &AnyChoice) -> bool {
    self.chosen().contains(value)
  }

  /// Whether `value` is of the type the collection's options carry.
  #[must_use]
  pub fn accepts(&self, value: &AnyChoice) -> bool {
    untrack(|| self.chosen()).accepts(value)
  }

  /// Asks for what is chosen once the reader chooses `value`.
  pub fn choose(&self, value: &AnyChoice) {
    (self.choose)(untrack(|| self.chosen()).choosing(value));
  }

  /// The members, in tree order.
  #[inline(always)]
  #[must_use]
  pub const fn roster(&self) -> &Roster {
    &self.roster
  }

  /// Whether `value` is the collection's one tab stop: the chosen member when it is
  /// enabled, else the first enabled one.
  #[must_use]
  pub fn is_tab_stop(&self, value: &AnyChoice) -> bool {
    self.tab_stop.with(|stop| stop.as_ref() == Some(value))
  }

  /// Moves to the enabled member `step` places from the one holding `value`, wrapping
  /// or stopping at the ends as the policy says, `isize::MIN` and `isize::MAX`
  /// reaching the first and the last: focuses it, and chooses it where the policy
  /// says a move does.
  pub fn step_from<P: Realization>(&self, value: &AnyChoice, step: isize) {
    let members = self.roster.enabled::<P>();
    let Some(index) = members.iter().position(|(_, held)| held == value)
    else {
      return;
    };
    let last = members.len().saturating_sub(1);
    let next = match step {
      isize::MIN => Some(0),
      isize::MAX => Some(last),
      _ if C::WRAPS => Ring::new(members.len()).step_from(index, step),
      _ => Some(index.saturating_add_signed(step).min(last)),
    };
    let Some((node, value)) = next.and_then(|next| members.get(next)).cloned()
    else {
      return;
    };

    if C::CHOOSES_ON_MOVE {
      self.choose(&value);
    }

    P::focus(node);
  }

  /// Feeds a key typed on the member holding `value` into the collection's type-ahead
  /// and focuses the member its buffer picks; `false` for a key that is no character.
  #[must_use]
  pub fn type_ahead_from<P: Realization>(&self, value: &AnyChoice, event: &KeyEvent) -> bool {
    let Some(buffer) = self.type_ahead.typed::<P>(event)
    else {
      return false;
    };
    if let Some(index) = self.roster.typed::<P>(&buffer, |held| held == value) {
      self.roster.focus_at::<P>(index);
    }

    true
  }
}

/// A collection of members under a [`CollectionPolicy`]: a radio group, a listbox or a
/// menu, its members one tab stop.
///
/// Controlled: what is chosen is what [`derive_chosen`](Self::derive_chosen) reads, and
/// choosing only calls [`handle_choose`](Self::handle_choose) with what a choice asks
/// for; [`bind_chosen`](Self::bind_chosen) wires both to one signal. Each takes a
/// [`Chosen`] of the members' values, an `Option<T>` for one choice, the members being
/// tagged with `T`s, and holds it erased, so the collection itself is one type whatever
/// the app's. Choosing a menu's member runs it. A collection whose members stand in a
/// native toggle, a radio group, writes the key of its chosen member as its
/// [`Chosen`](crate::Content::Chosen) content, which a [`Controller`] holds: a choice
/// the platform reports on a member asks for that member's value, a key no member goes
/// by asks nothing, and a choice the truth then refuses is undone by the write of the
/// chosen key. The collection takes no focus of its own; its members are [`Member`]
/// controls under the same policy, at any depth inside it, tracked by its
/// [`roster`](Self::roster).
#[must_use]
pub struct Collection<C: CollectionPolicy> {
  chosen: ControlledChoice,
  roster: Roster,
  commit_on_tab: bool,
  policy: PhantomData<C>,
}

impl<C: CollectionPolicy> std::fmt::Debug for Collection<C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Collection").finish_non_exhaustive()
  }
}

impl<C: CollectionPolicy> Collection<C> {
  /// A collection with nothing chosen and no handler, its roster owned by the current
  /// owner.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      chosen: ControlledChoice::new(),
      roster: Roster::new(),
      commit_on_tab: false,
      policy: PhantomData,
    }
  }

  /// Sets whether `Tab` from a focused member chooses it before focus moves on, as a
  /// picker's list does; off by default, where `Tab` only leaves the collection.
  #[inline(always)]
  pub const fn set_commit_on_tab(mut self, commit_on_tab: bool) -> Self {
    self.commit_on_tab = commit_on_tab;

    self
  }

  /// Shares `roster` with the collection, so members built under a group holding it
  /// belong to the collection as well.
  #[inline(always)]
  pub fn set_roster(mut self, roster: Roster) -> Self {
    self.roster = roster;

    self
  }

  /// The members the collection tracks, in tree order once the control builds.
  #[inline(always)]
  #[must_use]
  pub const fn roster(&self) -> &Roster {
    &self.roster
  }

  /// Sets what is chosen.
  pub fn set_chosen<K: Chosen>(mut self, chosen: K) -> Self {
    self.chosen.set(chosen);

    self
  }

  /// Derives what is chosen from the signals `chosen` reads.
  pub fn derive_chosen<K: Chosen>(mut self, chosen: impl Fn() -> K + 'static) -> Self {
    self.chosen.derive(chosen);

    self
  }

  /// Runs `action` with what a choice asks for.
  pub fn handle_choose<K: Chosen>(mut self, action: impl Fn(K) + 'static) -> Self {
    self.chosen.handle(action);

    self
  }

  /// Binds what is chosen to `signal` in both directions.
  #[inline(always)]
  pub fn bind_chosen<K: Chosen>(mut self, signal: Signal<K>) -> Self {
    self.chosen.bind(signal);

    self
  }
}

impl<C: CollectionPolicy> Default for Collection<C> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization, C: CollectionPolicy> Behavior<P> for Collection<C> {
  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(C::ROLE)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[]
  }

  #[inline(always)]
  fn label_placement(&self) -> LabelPlacement {
    LabelPlacement::Leading
  }

  fn attach(&self, control: &ControlHandle<P>) {
    let (chosen, choose) = (self.chosen.clone(), self.chosen.clone());
    let roster = self.roster.clone();

    roster.root_at(control.node());

    if C::MEMBER_NATIVE.is_some() {
      let (chosen, asks) = (self.chosen.clone(), self.chosen.clone());
      let (keys, values) = (roster.clone(), roster.clone());

      control.register(Controller::new(
        contents::CHOSEN,
        move || keys.key_of(&chosen.read()),
        move |proposed: &Option<Rc<str>>| {
          if let Some(value) = proposed.as_deref().and_then(|key| values.value_of(key)) {
            asks.choose(&value);
          }
        },
      ));
    }

    provide_context(
      CollectionContext::<C>::new::<P>(move || chosen.read(), move |next| choose.ask(next), roster)
        .set_commit_on_tab(self.commit_on_tab),
    );
  }

  #[inline(always)]
  fn focus_order(&self) -> Option<FocusOrder> {
    None
  }
}

/// One member of a [`Collection`] under the same policy: a radio choice, a listbox
/// option or a menu item, tagged with a value of the app's own type.
///
/// A click or `Space` chooses it, and `Enter` in a list; an arrow moves to an enabled
/// neighbour, choosing it where the policy says a move does; in a list `Home` and `End`
/// reach the ends and a typed character the member the type-ahead picks by the name its
/// label supplies; and it is the collection's tab stop while it is chosen or, with
/// nothing chosen, while first. A member tagged with a value of another type than its
/// collection's options carry is a debug assertion.
///
/// Its state reads its collection's choice, tracked. A member standing in a native
/// toggle writes its collection and the key it goes by there, its ordinal in the
/// collection's [`Roster`], as its [`Membership`](crate::Content::Membership) content,
/// so the platform groups the collection's members as one and checks the one its chosen
/// key names. Outside a collection it reports its role, is never chosen and takes focus
/// like any control, a native one holding its [`Checked`](crate::Content::Checked)
/// content unchecked.
#[must_use]
pub struct Member<C: CollectionPolicy> {
  value: AnyChoice,
  collection: RefCell<Option<CollectionContext<C>>>,
}

impl<C: CollectionPolicy> std::fmt::Debug for Member<C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Member").finish_non_exhaustive()
  }
}

impl<C: CollectionPolicy> Member<C> {
  /// A member tagged with `value`, what its collection is asked to choose when it is
  /// chosen.
  pub fn new(value: impl Choice) -> Self {
    Self {
      value: AnyChoice::new(value),
      collection: RefCell::new(None),
    }
  }

  /// The value this member is tagged with.
  #[inline(always)]
  #[must_use]
  pub const fn value(&self) -> &AnyChoice {
    &self.value
  }

  /// The collection this member found when it attached.
  #[inline(always)]
  fn collection(&self) -> Option<CollectionContext<C>> {
    self.collection.borrow().clone()
  }

  /// Whether this member is its collection's chosen one.
  fn is_chosen(&self) -> bool {
    self
      .collection()
      .is_some_and(|group| group.is_chosen(&self.value))
  }
}

impl<P: Realization, C: CollectionPolicy> Behavior<P> for Member<C> {
  #[inline(always)]
  fn native(&self) -> Option<NativeElement> {
    C::MEMBER_NATIVE
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(C::MEMBER_ROLE)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    C::MEMBER_EVENTS
  }

  fn attach(&self, control: &ControlHandle<P>) {
    let Some(group) = use_context::<CollectionContext<C>>()
    else {
      if C::MEMBER_NATIVE.is_some() {
        control.register(Controller::new(
          contents::CHECKED,
          || CheckState::Off,
          |_| {},
        ));
      }

      return;
    };
    let node = control.node();

    debug_assert!(
      group.accepts(&self.value),
      "a member is tagged with a value of the type its collection's options carry"
    );

    let key = group.roster.join(
      node,
      self.value.clone(),
      control.name_read(),
      control.disabled_read(),
    );
    let roster = group.roster.clone();

    on_cleanup(move || roster.leave(node));

    if C::MEMBER_NATIVE.is_some()
      && let Some(root) = group.roster.root()
    {
      contents::MEMBERSHIP.write::<P>(node, &Membership::new(root, key));
    }

    *self.collection.borrow_mut() = Some(group);
    control.refresh();
  }

  fn on_click(&self, _control: &ControlHandle<P>) -> Prevent {
    if let Some(group) = self.collection() {
      group.choose(&self.value);
    }

    Prevent::Bubbling
  }

  fn on_key(&self, event: &KeyEvent, _control: &ControlHandle<P>) -> Prevent {
    let Some(group) = self.collection()
    else {
      return Prevent::None;
    };
    let value = &self.value;
    let typed = || C::LISTS && group.type_ahead_from::<P>(value, event);

    match &event.key {
      Key::Named(NamedKey::ArrowDown) => group.step_from::<P>(value, 1),
      Key::Named(NamedKey::ArrowUp) => group.step_from::<P>(value, -1),
      Key::Named(NamedKey::ArrowRight) if C::HORIZONTAL => group.step_from::<P>(value, 1),
      Key::Named(NamedKey::ArrowLeft) if C::HORIZONTAL => group.step_from::<P>(value, -1),
      Key::Named(NamedKey::Home) if C::LISTS => group.step_from::<P>(value, isize::MIN),
      Key::Named(NamedKey::End) if C::LISTS => group.step_from::<P>(value, isize::MAX),
      Key::Named(NamedKey::Enter) if C::LISTS => group.choose(value),
      Key::Character(text) if text == " " && !typed() => group.choose(value),
      Key::Named(NamedKey::Tab) if group.commit_on_tab => {
        group.choose(value);

        return Prevent::None;
      }
      _ if typed() => {}
      _ => return Prevent::None,
    }

    Prevent::All
  }

  fn access(&self) -> Vec<AccessProp> {
    C::member_access(self.is_chosen())
  }

  fn focus_order(&self) -> Option<FocusOrder> {
    match self.collection() {
      Some(group) if !group.is_tab_stop(&self.value) => Some(FocusOrder::Skip),
      _ => Some(FocusOrder::Auto),
    }
  }

  fn state(&self) -> ControlState {
    ControlState {
      checked: (C::MARKS_CHOSEN && self.is_chosen()).into(),
      ..ControlState::DEFAULT
    }
  }
}

/// A radio group: a [`Collection`] under the [`RadioPolicy`].
pub type RadioGroup = Collection<RadioPolicy>;
/// One choice within a radio group: a [`Member`] under the [`RadioPolicy`].
pub type Radio = Member<RadioPolicy>;
/// What a radio group shares with its choices: a [`CollectionContext`] under the
/// [`RadioPolicy`].
pub type RadioGroupContext = CollectionContext<RadioPolicy>;
/// A listbox: a [`Collection`] under the [`ListboxPolicy`].
pub type Listbox = Collection<ListboxPolicy>;
/// One option within a listbox: a [`Member`] under the [`ListboxPolicy`].
pub type ListboxOption = Member<ListboxPolicy>;
/// What a listbox shares with its options: a [`CollectionContext`] under the
/// [`ListboxPolicy`].
pub type ListboxContext = CollectionContext<ListboxPolicy>;
/// A menu: a [`Collection`] under the [`MenuPolicy`].
pub type Menu = Collection<MenuPolicy>;
/// One item within a menu: a [`Member`] under the [`MenuPolicy`].
pub type MenuOption = Member<MenuPolicy>;
/// What a menu shares with its items: a [`CollectionContext`] under the [`MenuPolicy`].
pub type MenuContext = CollectionContext<MenuPolicy>;

/// A ring of positions stepped through by wrapping moves: a radio group's members, a
/// picker's options.
#[must_use]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ring(usize);

impl Ring {
  /// Creates a ring of `count` positions.
  #[inline(always)]
  pub const fn new(count: usize) -> Self {
    Self(count)
  }

  /// The position `step` places from `from`, wrapping at both ends.
  ///
  /// `None` when the ring is empty, or when the arithmetic does not fit.
  #[must_use]
  pub fn step_from(&self, from: usize, step: isize) -> Option<usize> {
    if self.0 == 0 {
      return None;
    }

    let count = isize::try_from(self.0).ok()?;
    let from = isize::try_from(from).ok()?;

    usize::try_from((from + step).rem_euclid(count)).ok()
  }
}

#[cfg(test)]
mod tests {
  use super::Ring;

  /// Stepping wraps at both ends, and an empty ring has nowhere to go.
  #[test]
  fn stepping_wraps_at_both_ends() {
    let ring = Ring::new(3);

    assert_eq!(ring.step_from(0, 1), Some(1));
    assert_eq!(ring.step_from(2, 1), Some(0), "forward past the end wraps");
    assert_eq!(ring.step_from(0, -1), Some(2), "back past the start wraps");
    assert_eq!(ring.step_from(0, 5), Some(2), "a long step still wraps");
    assert_eq!(Ring::new(0).step_from(0, 1), None);
  }
}
