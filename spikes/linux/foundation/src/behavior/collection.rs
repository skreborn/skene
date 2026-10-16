//! Members of a group, one chosen: a radio group's radios, a list box's options.

use std::{
  cell::RefCell,
  marker::PhantomData,
  rc::{Rc, Weak},
};

use crate::{
  NodeId, Owner, Signal,
  behavior::{Behavior, ControlHandle, Controlled, Response, ToggleKeys, ToggleKind, plain},
  seam::{
    access::{AccessFacts, Checked, Role},
    control::{AccessRequest, ControlState, LabelPlacement, Prevent},
    report::{KeyEvent, PointerEvent, PointerPhase},
  },
  untrack,
};

/// The axes a collection's arrows move along.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Orientation {
  /// The vertical arrows (a list).
  Vertical,
  /// The horizontal arrows.
  Horizontal,
  /// Both: down and right move forward, up and left backward (a radio group).
  Both,
}

/// A collection's key table over `count` members, one of them current: the arrows move to
/// the next enabled member along its orientation (wrapping past either end where it wraps),
/// and, where it pages, `Home` and `End` to the first and last enabled member, `PageUp` and
/// `PageDown` by a page. Its act is the member to move to.
#[derive(Clone, Copy)]
#[must_use]
pub struct CollectionKeys<'a> {
  /// How many members.
  pub count: usize,
  /// The current member.
  pub current: Option<usize>,
  /// Which arrows move.
  pub orientation: Orientation,
  /// Whether moving past an end goes round to the other.
  pub wraps: bool,
  /// The members a page key moves by; none where the collection does not page.
  pub page: Option<usize>,
  /// Whether a member can be moved to.
  pub enabled: &'a dyn Fn(usize) -> bool,
}

impl<'a> CollectionKeys<'a> {
  /// A collection of `count` members with `current`, moving along `orientation`, every
  /// member enabled, neither wrapping nor paging.
  pub fn new(count: usize, current: Option<usize>, orientation: Orientation) -> Self {
    Self {
      count,
      current,
      orientation,
      wraps: false,
      page: None,
      enabled: &|_| true,
    }
  }

  /// This collection, wrapping past its ends where `wraps`.
  pub const fn with_wrap(mut self, wraps: bool) -> Self {
    self.wraps = wraps;

    self
  }

  /// This collection, paging by `page` members.
  pub const fn with_page(mut self, page: usize) -> Self {
    self.page = Some(page);

    self
  }

  /// This collection, a member moved to only where `enabled` says.
  pub const fn with_enabled(mut self, enabled: &'a dyn Fn(usize) -> bool) -> Self {
    self.enabled = enabled;

    self
  }

  /// The next enabled member from `from` (exclusive) going forward or backward, wrapping
  /// where the collection wraps; from before the first (or after the last) where `from` is
  /// `None`.
  #[must_use]
  pub fn step(&self, from: Option<usize>, forward: bool) -> Option<usize> {
    if self.count == 0 {
      return None;
    }

    let count = self.count;
    let mut at = from;

    for _ in 0..count {
      let next = match (at, forward) {
        (None, true) => 0,
        (None, false) => count - 1,
        (Some(at), true) if at + 1 < count => at + 1,
        (Some(_), true) if self.wraps => 0,
        (Some(at), false) if at > 0 => at - 1,
        (Some(_), false) if self.wraps => count - 1,
        (Some(_), _) => return None,
      };

      if (self.enabled)(next) {
        return Some(next);
      }

      at = Some(next);
    }

    None
  }

  /// The first enabled member, or the last where `!first`.
  #[must_use]
  pub fn end(&self, first: bool) -> Option<usize> {
    Self {
      wraps: false,
      ..*self
    }
    .step(None, first)
  }

  /// The member a page moves to from the current one, forward or backward: the farthest
  /// enabled member within a page, else the end.
  #[must_use]
  pub fn paged(&self, page: usize, forward: bool) -> Option<usize> {
    let Some(current) = self.current else {
      return self.end(forward);
    };
    let target = if forward {
      (current + page).min(self.count.saturating_sub(1))
    } else {
      current.saturating_sub(page)
    };
    let mut at = target;

    loop {
      if (self.enabled)(at) {
        return (at != current).then_some(at);
      }

      if at == current {
        return None;
      }

      at = if forward { at - 1 } else { at + 1 };
    }
  }

  /// A key pressed while the collection (or its current member) holds focus.
  pub fn key(&self, event: &KeyEvent) -> Response<usize> {
    if !event.down || !plain(event) || event.modifiers.shift {
      return Response::IGNORED;
    }

    let vertical = matches!(self.orientation, Orientation::Vertical | Orientation::Both);
    let horizontal = matches!(
      self.orientation,
      Orientation::Horizontal | Orientation::Both
    );
    let target = match event.key.as_str() {
      "ArrowDown" if vertical => self.step(self.current, true),
      "ArrowUp" if vertical => self.step(self.current, false),
      "ArrowRight" if horizontal => self.step(self.current, true),
      "ArrowLeft" if horizontal => self.step(self.current, false),
      "Home" if self.page.is_some() => self.end(true),
      "End" if self.page.is_some() => self.end(false),
      "PageDown" => match self.page {
        Some(page) => self.paged(page, true),
        None => return Response::IGNORED,
      },
      "PageUp" => match self.page {
        Some(page) => self.paged(page, false),
        None => return Response::IGNORED,
      },
      _ => return Response::IGNORED,
    };

    Response::maybe(target.filter(|target| Some(*target) != self.current))
  }
}

/// What a choice commits: run with the member chosen.
type Commit = Rc<dyn Fn(usize)>;

/// The members of a collection as they register, its controlled choice, and what a choice
/// commits: what a group and its members share.
pub struct Roster {
  chosen: RefCell<Controlled<Option<usize>>>,
  members: RefCell<Vec<(usize, ControlHandle)>>,
  commit: RefCell<Option<Commit>>,
}

impl std::fmt::Debug for Roster {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Roster")
      .field("members", &self.members.borrow().len())
      .finish_non_exhaustive()
  }
}

impl Roster {
  /// A roster with no member, its choice `chosen`.
  #[must_use]
  pub fn new(chosen: Controlled<Option<usize>>) -> Rc<Self> {
    Rc::new(Self {
      chosen: RefCell::new(chosen),
      members: RefCell::new(Vec::new()),
      commit: RefCell::new(None),
    })
  }

  /// Runs `commit` after each choice that commits (an option chosen in a popup's list).
  pub fn set_commit(&self, commit: impl Fn(usize) + 'static) {
    *self.commit.borrow_mut() = Some(Rc::new(commit));
  }

  /// Changes the controlled choice before the collection is built.
  pub fn change_chosen(&self, change: impl FnOnce(&mut Controlled<Option<usize>>)) {
    change(&mut self.chosen.borrow_mut());
  }

  /// The chosen member as it reads now, tracking it.
  #[must_use]
  pub fn chosen(&self) -> Option<usize> {
    let chosen = self.chosen.borrow().clone();

    chosen.read().flatten()
  }

  /// Asks for member `index` to be chosen, unless it is already.
  pub fn choose(&self, index: usize) {
    if untrack(|| self.chosen()) != Some(index) {
      let chosen = self.chosen.borrow().clone();

      chosen.ask(Some(index));
    }
  }

  /// Chooses member `index` and runs what a choice commits.
  pub fn commit(&self, index: usize) {
    self.choose(index);

    let commit = self.commit.borrow().clone();

    if let Some(commit) = commit {
      commit(index);
    }
  }

  /// Registers member `index`'s control until the current owner is disposed.
  pub fn register(self: &Rc<Self>, index: usize, member: ControlHandle) {
    let node = member.node();

    self.members.borrow_mut().push((index, member));

    let roster: Weak<Self> = Rc::downgrade(self);

    if Owner::current().is_some() {
      Owner::on_cleanup(move || {
        if let Some(roster) = roster.upgrade() {
          roster
            .members
            .borrow_mut()
            .retain(|(_, member)| member.node() != node);
        }
      });
    }
  }

  /// How many members it has: one past the greatest index registered.
  #[must_use]
  pub fn count(&self) -> usize {
    self
      .members
      .borrow()
      .iter()
      .map(|(index, _)| index + 1)
      .max()
      .unwrap_or(0)
  }

  /// Member `index`'s control, where it is registered.
  #[must_use]
  pub fn member(&self, index: usize) -> Option<ControlHandle> {
    self
      .members
      .borrow()
      .iter()
      .find(|(at, _)| *at == index)
      .map(|(_, member)| member.clone())
  }

  /// Member `node`'s index, where it is registered.
  #[must_use]
  pub fn index_of(&self, node: NodeId) -> Option<usize> {
    self
      .members
      .borrow()
      .iter()
      .find(|(_, member)| member.node() == node)
      .map(|(index, _)| *index)
  }

  /// Whether member `index` is registered and enabled, tracking it.
  #[must_use]
  pub fn is_enabled(&self, index: usize) -> bool {
    self
      .member(index)
      .is_some_and(|member| !member.is_disabled())
  }

  /// Moves focus to member `index`.
  pub fn focus(&self, index: usize) {
    if let Some(member) = self.member(index) {
      member.focus();
    }
  }

  /// The member a group's one tab stop is on: the chosen one where it is enabled, else the
  /// first enabled; tracking both.
  #[must_use]
  pub fn stop(&self) -> Option<usize> {
    let chosen = self.chosen().filter(|chosen| self.is_enabled(*chosen));

    chosen.or_else(|| (0..self.count()).find(|index| self.is_enabled(*index)))
  }
}

/// How a collection's members move and choose: a radio group's or a list box's.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a collection's policy",
  note = "a policy names a collection's roles and how its arrows move and choose"
)]
pub trait CollectionPolicy: 'static {
  /// What a reader takes the group for.
  const GROUP: Role;
  /// What a reader takes a member for.
  const MEMBER: Role;
  /// Which arrows move between members.
  const ORIENTATION: Orientation;
  /// Whether moving past an end goes round to the other.
  const WRAPS: bool;
  /// The members a page key moves by; none where the collection does not page.
  const PAGE: Option<usize>;
  /// Whether an arrow chooses the member it moves to (a radio group), or only focuses it.
  const CHOOSE_ON_MOVE: bool;
  /// Whether `Enter`, `Space` and a click choose and commit (a popup's list), or `Space`
  /// alone chooses as a radio does.
  const COMMITS: bool;
}

/// A radio group: `Space` and a click choose a radio, the arrows move to the next and choose
/// it, wrapping, and the group is one tab stop (its chosen radio, else its first enabled).
#[derive(Clone, Copy, Debug)]
pub struct RadioPolicy;

impl CollectionPolicy for RadioPolicy {
  const GROUP: Role = Role::RadioGroup;
  const MEMBER: Role = Role::Radio;
  const ORIENTATION: Orientation = Orientation::Both;
  const WRAPS: bool = true;
  const PAGE: Option<usize> = None;
  const CHOOSE_ON_MOVE: bool = true;
  const COMMITS: bool = false;
}

/// A list box in a popup: the arrows, the page keys, `Home` and `End` move focus without
/// choosing, not wrapping; `Enter`, `Space` and a click choose the focused option and commit;
/// a pointer moving over an option focuses it.
#[derive(Clone, Copy, Debug)]
pub struct ListboxPolicy;

impl CollectionPolicy for ListboxPolicy {
  const GROUP: Role = Role::ListBox;
  const MEMBER: Role = Role::Option;
  const ORIENTATION: Orientation = Orientation::Vertical;
  const WRAPS: bool = false;
  const PAGE: Option<usize> = Some(10);
  const CHOOSE_ON_MOVE: bool = false;
  const COMMITS: bool = true;
}

/// A collection's behavior: the group around its members, one of them chosen. It takes no
/// focus of its own: its members do ([`Collection::member`]).
#[must_use]
pub struct Collection<P: CollectionPolicy> {
  roster: Rc<Roster>,
  policy: PhantomData<P>,
}

/// A radio group's behavior.
pub type RadioGroup = Collection<RadioPolicy>;
/// A list box's behavior.
pub type Listbox = Collection<ListboxPolicy>;
/// A radio's behavior.
pub type Radio = Member<RadioPolicy>;
/// A list box option's behavior.
pub type ListboxOption = Member<ListboxPolicy>;

impl<P: CollectionPolicy> Default for Collection<P> {
  fn default() -> Self {
    Self::new()
  }
}

impl<P: CollectionPolicy> Collection<P> {
  /// A collection with nothing chosen and no handler.
  pub fn new() -> Self {
    Self::with_roster(Roster::new(Controlled::new()))
  }

  /// A collection over `roster`, shared with what else reads its choice (a popup's face).
  pub fn with_roster(roster: Rc<Roster>) -> Self {
    Self {
      roster,
      policy: PhantomData,
    }
  }

  /// Its roster.
  #[must_use]
  pub fn roster(&self) -> Rc<Roster> {
    Rc::clone(&self.roster)
  }

  /// This collection, member `chosen` chosen.
  pub fn set_chosen(self, chosen: Option<usize>) -> Self {
    self
      .roster
      .change_chosen(|controlled| controlled.set(chosen));

    self
  }

  /// This collection, the member `chosen` returns chosen, kept current.
  pub fn derive_chosen(self, chosen: impl Fn() -> Option<usize> + 'static) -> Self {
    self
      .roster
      .change_chosen(|controlled| controlled.derive(chosen));

    self
  }

  /// This collection, asking `choose` for each member the reader chooses.
  pub fn handle_choose(self, choose: impl Fn(Option<usize>) + 'static) -> Self {
    self
      .roster
      .change_chosen(|controlled| controlled.handle(choose));

    self
  }

  /// This collection, showing `chosen` and writing each choice into it.
  pub fn bind_chosen(self, chosen: Signal<Option<usize>>) -> Self {
    self
      .roster
      .change_chosen(|controlled| controlled.bind_signal(chosen));

    self
  }

  /// The behavior of its member `index`.
  pub fn member(&self, index: usize) -> Member<P> {
    Member {
      index,
      roster: Rc::clone(&self.roster),
      pressed: Signal::new(false),
      policy: PhantomData,
    }
  }
}

impl<P: CollectionPolicy> Behavior for Collection<P> {
  fn role(&self) -> Role {
    P::GROUP
  }

  fn label_placement(&self) -> LabelPlacement {
    LabelPlacement::Leading
  }

  fn focusable(&self) -> bool {
    false
  }
}

/// A member's behavior: one of its collection's, by index, chosen through the roster it
/// shares with the group.
#[must_use]
pub struct Member<P: CollectionPolicy> {
  index: usize,
  roster: Rc<Roster>,
  pressed: Signal<bool>,
  policy: PhantomData<P>,
}

impl<P: CollectionPolicy> Member<P> {
  /// Its index in its collection.
  #[must_use]
  pub const fn index(&self) -> usize {
    self.index
  }

  fn is_chosen(&self) -> bool {
    self.roster.chosen() == Some(self.index)
  }

  fn choose(&self) {
    if P::COMMITS {
      self.roster.commit(self.index);
    } else {
      self.roster.choose(self.index);
    }
  }

  fn keys(&self) -> (usize, Option<usize>) {
    (self.roster.count(), Some(self.index))
  }
}

impl<P: CollectionPolicy> Behavior for Member<P> {
  fn role(&self) -> Role {
    P::MEMBER
  }

  fn attach(&self, control: &ControlHandle) {
    self.roster.register(self.index, control.clone());
  }

  fn on_click(&self, control: &ControlHandle) -> Prevent {
    control.focus();
    self.choose();

    Prevent::All
  }

  fn on_key(&self, event: &KeyEvent, _: &ControlHandle) -> Prevent {
    let roster = &self.roster;
    let modifiers = event.modifiers;

    if event.down && P::COMMITS {
      let toggles = (modifiers.alt && matches!(event.key.as_str(), "ArrowDown" | "ArrowUp"))
        || event.key == "F4";

      match event.key.as_str() {
        "Enter" | " " if plain(event) => {
          self.choose();

          return Prevent::All;
        }
        _ if toggles => {
          self.choose();

          return Prevent::All;
        }
        "Tab" => {
          roster.choose(self.index);

          return Prevent::None;
        }
        _ => {}
      }
    }

    if !P::COMMITS {
      let keys = ToggleKeys::new(
        ToggleKind::Radio,
        untrack(|| self.is_chosen()),
        false,
        self.pressed.get_untracked(),
      );
      let response = keys.key(event);

      if let Some(pressed) = response.pressed {
        self.pressed.set(pressed);
      }

      if response.act.is_some() {
        self.choose();
      }

      if response.consumed {
        return Prevent::All;
      }
    }

    let (count, current) = untrack(|| self.keys());
    let enabled = |index: usize| untrack(|| roster.is_enabled(index));
    let mut keys = CollectionKeys::new(count, current, P::ORIENTATION)
      .with_wrap(P::WRAPS)
      .with_enabled(&enabled);

    if let Some(page) = P::PAGE {
      keys = keys.with_page(page);
    }

    let response = keys.key(event);

    if let Some(target) = response.act {
      roster.focus(target);

      if P::CHOOSE_ON_MOVE {
        roster.choose(target);
      }
    }

    response.verdict()
  }

  fn on_pointer(&self, event: &PointerEvent, control: &ControlHandle) -> Prevent {
    if P::COMMITS && event.phase == PointerPhase::Move {
      control.focus();
    }

    Prevent::None
  }

  fn on_blur(&self, _: &ControlHandle) {
    if self.pressed.get_untracked() {
      self.pressed.set(false);
    }
  }

  fn on_access(&self, request: AccessRequest, control: &ControlHandle) -> bool {
    match request {
      AccessRequest::Activate | AccessRequest::Toggle => {
        control.focus();
        self.choose();

        true
      }
      _ => false,
    }
  }

  fn access(&self) -> AccessFacts {
    let chosen = self.is_chosen();

    AccessFacts {
      checked: (!P::COMMITS).then_some(if chosen { Checked::On } else { Checked::Off }),
      selected: P::COMMITS.then_some(chosen),
      actions: vec![AccessRequest::Activate],
      ..AccessFacts::default()
    }
  }

  fn focusable(&self) -> bool {
    P::COMMITS || self.roster.stop() == Some(self.index)
  }

  fn state(&self) -> ControlState {
    ControlState {
      checked: if self.is_chosen() {
        Checked::On
      } else {
        Checked::Off
      },
      pressed: self.pressed.get(),
      ..ControlState::DEFAULT
    }
  }
}
