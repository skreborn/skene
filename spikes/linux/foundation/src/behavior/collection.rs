//! Members of a group, each carrying a value of the app's own type: a radio group's radios, a
//! list box's options, a menu's items.

use std::{
  cell::RefCell,
  collections::{BTreeSet, HashMap, HashSet},
  hash::Hash,
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

/// What a choice group's options carry and what it has chosen: a value of the app's own type,
/// compared to find the chosen option and hashed to name it.
///
/// Its `Hash` agrees with its `Eq` and depends on nothing of the build (no pointer, no
/// `TypeId`), since a platform may name an option by it across two builds of one app.
#[diagnostic::on_unimplemented(
  message = "`{Self}` cannot be a choice's value",
  note = "a choice's value is `Clone`, `Eq` and `Hash`, as SwiftUI's tag is `Hashable`: derive them, or choose by an id"
)]
pub trait Choice: Clone + Eq + Hash + 'static {}

#[diagnostic::do_not_recommend]
impl<T: Clone + Eq + Hash + 'static> Choice for T {}

/// What a list box has chosen: one value or none (`Option<T>`), or a set of values
/// (`HashSet<T>`, `BTreeSet<T>`).
#[diagnostic::on_unimplemented(
  message = "a list box chooses an `Option` or a set of its options' values, not `{Self}`",
  note = "bind an `Option<T>` for one choice, a `HashSet<T>` or a `BTreeSet<T>` for several"
)]
pub trait Chosen: Clone + PartialEq + 'static {
  /// What each option carries.
  type Value: Choice;

  /// Whether `value` is chosen.
  fn contains(&self, value: &Self::Value) -> bool;

  /// What is chosen once the reader chooses `value` from `chosen` (nothing where none
  /// reads): `value` alone for one choice, `value` added or taken away for a set.
  fn choosing(chosen: Option<&Self>, value: &Self::Value) -> Self;
}

impl<T: Choice> Chosen for Option<T> {
  type Value = T;

  fn contains(&self, value: &T) -> bool {
    self.as_ref() == Some(value)
  }

  fn choosing(_: Option<&Self>, value: &T) -> Self {
    Some(value.clone())
  }
}

impl<T: Choice> Chosen for HashSet<T> {
  type Value = T;

  fn contains(&self, value: &T) -> bool {
    Self::contains(self, value)
  }

  fn choosing(chosen: Option<&Self>, value: &T) -> Self {
    let mut chosen = chosen.cloned().unwrap_or_default();

    if !chosen.remove(value) {
      chosen.insert(value.clone());
    }

    chosen
  }
}

impl<T: Choice + Ord> Chosen for BTreeSet<T> {
  type Value = T;

  fn contains(&self, value: &T) -> bool {
    Self::contains(self, value)
  }

  fn choosing(chosen: Option<&Self>, value: &T) -> Self {
    let mut chosen = chosen.cloned().unwrap_or_default();

    if !chosen.remove(value) {
      chosen.insert(value.clone());
    }

    chosen
  }
}

/// What a collection holds: a value always chosen ([`One`]: a radio group's, a picker's), or a
/// list box's choice ([`Chosen`]), or a menu's place.
pub trait Held: 'static {
  /// What each member carries.
  type Option: Choice;
  /// What its choice is bound as: `T` for `One<T>`, the choice itself for a list box's.
  type Bound: Clone + PartialEq + 'static;

  /// Whether `bound` chooses `value`.
  fn holds(bound: &Self::Bound, value: &Self::Option) -> bool;

  /// What is chosen once the reader chooses `value` from `bound` (nothing where none reads).
  fn choosing(bound: Option<&Self::Bound>, value: &Self::Option) -> Self::Bound;
}

/// A value always chosen: what a radio group and a picker hold, each option carrying a `T`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct One<T>(pub T);

impl<T: Choice> Held for One<T> {
  type Option = T;
  type Bound = T;

  fn holds(bound: &T, value: &T) -> bool {
    bound == value
  }

  fn choosing(_: Option<&T>, value: &T) -> T {
    value.clone()
  }
}

impl<C: Chosen> Held for C {
  type Option = C::Value;
  type Bound = C;

  fn holds(bound: &C, value: &C::Value) -> bool {
    bound.contains(value)
  }

  fn choosing(bound: Option<&C>, value: &C::Value) -> C {
    C::choosing(bound, value)
  }
}

/// The options a collection moves through, built or not, in order: what its keys, its
/// type-ahead and a closed face read. A styled group's are its segments; a group whose members
/// a third party places by hand moves through its registered members in the scene's order.
pub trait Options<T> {
  /// How many options, tracking what they are read from.
  fn count(&self) -> usize;

  /// The value of the option at `index`.
  fn value(&self, index: usize) -> Option<T>;

  /// The place of the option carrying `value`.
  fn index_of(&self, value: &T) -> Option<usize>;

  /// The option's name where its label states one; none where its content names it.
  fn name(&self, index: usize) -> Option<Rc<str>>;
}

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

/// What a choice commits: run with the value of the member chosen.
type Commit<T> = Rc<dyn Fn(&T)>;

/// The members of a collection as they register, the options it moves through, its controlled
/// choice, and what a choice commits: what a group and its members share.
pub struct Roster<H: Held> {
  chosen: RefCell<Controlled<H::Bound>>,
  members: RefCell<HashMap<H::Option, ControlHandle>>,
  options: RefCell<Option<Rc<dyn Options<H::Option>>>>,
  commit: RefCell<Option<Commit<H::Option>>>,
  dismiss: RefCell<Option<Rc<dyn Fn()>>>,
  changed: Signal<u32>,
  active: Signal<Option<H::Option>>,
}

impl<H: Held> std::fmt::Debug for Roster<H> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Roster")
      .field("members", &self.members.borrow().len())
      .finish_non_exhaustive()
  }
}

impl<H: Held> Roster<H> {
  /// A roster with no member, its choice `chosen`.
  #[must_use]
  pub fn new(chosen: Controlled<H::Bound>) -> Rc<Self> {
    Rc::new(Self {
      chosen: RefCell::new(chosen),
      members: RefCell::new(HashMap::new()),
      options: RefCell::new(None),
      commit: RefCell::new(None),
      dismiss: RefCell::new(None),
      changed: Signal::new(0),
      active: Signal::new(None),
    })
  }

  /// Runs `dismiss` when `Escape` is pressed on a committing member: a menu closing without a
  /// command.
  pub fn set_dismiss(&self, dismiss: impl Fn() + 'static) {
    *self.dismiss.borrow_mut() = Some(Rc::new(dismiss));
  }

  /// Runs what dismisses the collection, where something does: whether it did.
  #[must_use]
  pub fn dismiss(&self) -> bool {
    let dismiss = self.dismiss.borrow().clone();

    dismiss.is_some_and(|dismiss| {
      dismiss();

      true
    })
  }

  /// Runs `commit` after each choice that commits (an option chosen in a popup's list).
  pub fn set_commit(&self, commit: impl Fn(&H::Option) + 'static) {
    *self.commit.borrow_mut() = Some(Rc::new(commit));
  }

  /// The options it moves through: a styled group's segments. Without them it moves through
  /// its registered members in the scene's order.
  pub fn set_options(&self, options: Rc<dyn Options<H::Option>>) {
    *self.options.borrow_mut() = Some(options);
  }

  /// The options it moves through, as [`Roster::set_options`] says.
  #[must_use]
  pub fn options(self: &Rc<Self>) -> Rc<dyn Options<H::Option>> {
    let options = self.options.borrow().clone();

    options.unwrap_or_else(|| Rc::new(Registered(Rc::clone(self))))
  }

  /// Changes the controlled choice before the collection is built.
  pub fn change_chosen(&self, change: impl FnOnce(&mut Controlled<H::Bound>)) {
    change(&mut self.chosen.borrow_mut());
  }

  /// What is chosen as it reads now, tracking it; none where nothing is derived.
  #[must_use]
  pub fn chosen(&self) -> Option<H::Bound> {
    let chosen = self.chosen.borrow().clone();

    chosen.read()
  }

  /// Whether the member carrying `value` is chosen, tracking it.
  #[must_use]
  pub fn is_chosen(&self, value: &H::Option) -> bool {
    self.chosen().is_some_and(|chosen| H::holds(&chosen, value))
  }

  /// Asks for `value` to be chosen: chosen alone, or, in a set, added or taken away; nothing
  /// where that changes nothing.
  pub fn choose(&self, value: &H::Option) {
    let now = untrack(|| self.chosen());
    let next = H::choosing(now.as_ref(), value);

    if now.as_ref() != Some(&next) {
      let chosen = self.chosen.borrow().clone();

      chosen.ask(next);
    }
  }

  /// Chooses `value` and runs what a choice commits.
  pub fn commit(&self, value: &H::Option) {
    self.choose(value);

    let commit = self.commit.borrow().clone();

    if let Some(commit) = commit {
      commit(value);
    }
  }

  /// Registers the control of the member carrying `value` until the current owner is
  /// disposed.
  ///
  /// # Panics
  ///
  /// In a debug build, where another live member already carries `value`: two options of one
  /// choice carry one value.
  pub fn register(self: &Rc<Self>, value: H::Option, member: ControlHandle) {
    let node = member.node();
    let previous = self.members.borrow_mut().insert(value.clone(), member);

    debug_assert!(
      previous.is_none_or(|previous| previous.node() == node),
      "two options of one choice carry one value"
    );
    self.changed.update(|count| *count = count.wrapping_add(1));

    let roster: Weak<Self> = Rc::downgrade(self);

    if Owner::current().is_some() {
      Owner::on_cleanup(move || {
        if let Some(roster) = roster.upgrade() {
          let mut members = roster.members.borrow_mut();

          if members
            .get(&value)
            .is_some_and(|member| member.node() == node)
          {
            members.remove(&value);
          }

          drop(members);
          roster
            .changed
            .update(|count| *count = count.wrapping_add(1));
        }
      });
    }
  }

  /// The control of the member carrying `value`, where it is registered.
  #[must_use]
  pub fn member(&self, value: &H::Option) -> Option<ControlHandle> {
    self.members.borrow().get(value).cloned()
  }

  /// The value the member `node` carries, where it is registered.
  #[must_use]
  pub fn value_of(&self, node: NodeId) -> Option<H::Option> {
    self
      .members
      .borrow()
      .iter()
      .find(|(_, member)| member.node() == node)
      .map(|(value, _)| value.clone())
  }

  /// Whether the member carrying `value` is registered and enabled, tracking both.
  #[must_use]
  pub fn is_enabled(&self, value: &H::Option) -> bool {
    let _ = self.changed.get();

    self
      .member(value)
      .is_some_and(|member| !member.is_disabled())
  }

  /// Moves focus to the member carrying `value`, which becomes the active one.
  pub fn focus(&self, value: &H::Option) {
    if self
      .active
      .with_untracked(|active| active.as_ref() != Some(value))
    {
      self.active.set(Some(value.clone()));
    }

    if let Some(member) = self.member(value) {
      member.focus();
    }
  }

  /// The member focus was last moved to through the roster, where it is registered and
  /// enabled, tracking it.
  #[must_use]
  pub fn active(&self) -> Option<H::Option> {
    self.active.get().filter(|active| self.is_enabled(active))
  }

  /// The member a group's one tab stop is on: the first chosen one that is enabled, else the
  /// first enabled; tracking both.
  #[must_use]
  pub fn stop(self: &Rc<Self>) -> Option<H::Option> {
    let options = self.options();
    let values: Vec<H::Option> = (0..options.count())
      .filter_map(|index| options.value(index))
      .collect();
    let chosen = self.chosen();

    values
      .iter()
      .find(|value| {
        chosen
          .as_ref()
          .is_some_and(|chosen| H::holds(chosen, value))
          && self.is_enabled(value)
      })
      .or_else(|| values.iter().find(|value| self.is_enabled(value)))
      .cloned()
  }
}

/// A roster's registered members in the scene's order: the options of a group whose members a
/// third party places by hand.
struct Registered<H: Held>(Rc<Roster<H>>);

impl<H: Held> Registered<H> {
  /// The registered members' values in the scene's order, tracking registrations.
  fn values(&self) -> Vec<H::Option> {
    let _ = self.0.changed.get();
    let members = self.0.members.borrow();
    let mut nodes: Vec<(NodeId, H::Option)> = members
      .iter()
      .map(|(value, member)| (member.node(), value.clone()))
      .collect();
    let order = members.values().next().map(ControlHandle::scene_order);

    drop(members);

    if let Some(order) = order {
      nodes.sort_by(|a, b| order(a.0, b.0));
    }

    nodes.into_iter().map(|(_, value)| value).collect()
  }
}

impl<H: Held> Options<H::Option> for Registered<H> {
  fn count(&self) -> usize {
    self.values().len()
  }

  fn value(&self, index: usize) -> Option<H::Option> {
    self.values().into_iter().nth(index)
  }

  fn index_of(&self, value: &H::Option) -> Option<usize> {
    self.values().iter().position(|at| at == value)
  }

  fn name(&self, index: usize) -> Option<Rc<str>> {
    self
      .value(index)
      .and_then(|value| self.0.member(&value))
      .and_then(|member| member.name())
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
  /// Whether a committing member reports itself selected while chosen (a list box's option);
  /// a menu's item reports neither selection nor check.
  const SELECTS: bool;
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
  const SELECTS: bool = false;
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
  const SELECTS: bool = true;
}

/// A menu of commands: the arrows, `Home` and `End` move focus without choosing, wrapping;
/// `Enter`, `Space` and a click run the focused item and commit; a pointer moving over an item
/// focuses it; `Escape` dismisses the menu ([`Roster::set_dismiss`]).
#[derive(Clone, Copy, Debug)]
pub struct MenuPolicy;

impl CollectionPolicy for MenuPolicy {
  const GROUP: Role = Role::Menu;
  const MEMBER: Role = Role::MenuItem;
  const ORIENTATION: Orientation = Orientation::Vertical;
  const WRAPS: bool = true;
  const PAGE: Option<usize> = None;
  const CHOOSE_ON_MOVE: bool = false;
  const COMMITS: bool = true;
  const SELECTS: bool = false;
}

/// A collection's behavior: the group around its members, each carrying a value, what it
/// holds chosen. It takes no focus of its own: its members do ([`Collection::member`]).
#[must_use]
pub struct Collection<P: CollectionPolicy, H: Held> {
  roster: Rc<Roster<H>>,
  policy: PhantomData<P>,
}

/// A radio group's behavior: one `T` always chosen.
pub type RadioGroup<T> = Collection<RadioPolicy, One<T>>;
/// A radio's behavior.
pub type Radio<T> = Member<RadioPolicy, One<T>>;
/// A list box's behavior, choosing `C`: an `Option` of its options' values, or a set of them.
pub type Listbox<C> = Collection<ListboxPolicy, C>;
/// A list box option's behavior.
pub type ListboxOption<C> = Member<ListboxPolicy, C>;
/// A menu's behavior: its commands, by their place among its entries.
pub type Menu = Collection<MenuPolicy, Option<usize>>;
/// A menu item's behavior.
pub type MenuOption = Member<MenuPolicy, Option<usize>>;

impl<P: CollectionPolicy, H: Held> Default for Collection<P, H> {
  fn default() -> Self {
    Self::new()
  }
}

impl<P: CollectionPolicy, H: Held> Collection<P, H> {
  /// A collection with nothing chosen and no handler.
  pub fn new() -> Self {
    Self::with_roster(Roster::new(Controlled::new()))
  }

  /// A collection over `roster`, shared with what else reads its choice (a popup's face).
  pub const fn with_roster(roster: Rc<Roster<H>>) -> Self {
    Self {
      roster,
      policy: PhantomData,
    }
  }

  /// Its roster.
  #[must_use]
  pub fn roster(&self) -> Rc<Roster<H>> {
    Rc::clone(&self.roster)
  }

  /// This collection, `chosen` chosen.
  pub fn set_chosen(self, chosen: H::Bound) -> Self {
    self
      .roster
      .change_chosen(|controlled| controlled.set(chosen));

    self
  }

  /// This collection, what `chosen` returns chosen, kept current.
  pub fn derive_chosen(self, chosen: impl Fn() -> H::Bound + 'static) -> Self {
    self
      .roster
      .change_chosen(|controlled| controlled.derive(chosen));

    self
  }

  /// This collection, asking `choose` for each choice the reader makes.
  pub fn handle_choose(self, choose: impl Fn(H::Bound) + 'static) -> Self {
    self
      .roster
      .change_chosen(|controlled| controlled.handle(choose));

    self
  }

  /// This collection, showing `chosen` and writing each choice into it.
  pub fn bind_chosen(self, chosen: Signal<H::Bound>) -> Self {
    self
      .roster
      .change_chosen(|controlled| controlled.bind_signal(chosen));

    self
  }

  /// The behavior of its member carrying `value`.
  pub fn member(&self, value: H::Option) -> Member<P, H> {
    Member {
      value,
      roster: Rc::clone(&self.roster),
      pressed: Signal::new(false),
      policy: PhantomData,
    }
  }
}

impl<P: CollectionPolicy, H: Held> Behavior for Collection<P, H> {
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

/// A member's behavior: one of its collection's, carrying a value, chosen through the roster
/// it shares with the group.
#[must_use]
pub struct Member<P: CollectionPolicy, H: Held> {
  value: H::Option,
  roster: Rc<Roster<H>>,
  pressed: Signal<bool>,
  policy: PhantomData<P>,
}

impl<P: CollectionPolicy, H: Held> Member<P, H> {
  /// The value it carries.
  #[must_use]
  pub const fn value(&self) -> &H::Option {
    &self.value
  }

  fn is_chosen(&self) -> bool {
    self.roster.is_chosen(&self.value)
  }

  fn choose(&self) {
    if P::COMMITS {
      self.roster.commit(&self.value);
    } else {
      self.roster.choose(&self.value);
    }
  }
}

impl<P: CollectionPolicy, H: Held> Behavior for Member<P, H> {
  fn role(&self) -> Role {
    P::MEMBER
  }

  fn attach(&self, control: &ControlHandle) {
    self.roster.register(self.value.clone(), control.clone());
  }

  fn on_click(&self, _: &ControlHandle) -> Prevent {
    self.roster.focus(&self.value);
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
        "Escape" if plain(event) && roster.dismiss() => return Prevent::All,
        "Enter" | " " if plain(event) => {
          self.choose();

          return Prevent::All;
        }
        _ if toggles => {
          self.choose();

          return Prevent::All;
        }
        "Tab" => {
          roster.choose(&self.value);

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

    let options = roster.options();
    let (count, current) = untrack(|| (options.count(), options.index_of(&self.value)));
    let enabled = |index: usize| {
      untrack(|| {
        options
          .value(index)
          .is_some_and(|value| roster.is_enabled(&value))
      })
    };
    let mut keys = CollectionKeys::new(count, current, P::ORIENTATION)
      .with_wrap(P::WRAPS)
      .with_enabled(&enabled);

    if let Some(page) = P::PAGE {
      keys = keys.with_page(page);
    }

    let response = keys.key(event);

    if let Some(target) = response
      .act
      .and_then(|target| untrack(|| options.value(target)))
    {
      roster.focus(&target);

      if P::CHOOSE_ON_MOVE {
        roster.choose(&target);
      }
    }

    response.verdict()
  }

  fn on_pointer(&self, event: &PointerEvent, _: &ControlHandle) -> Prevent {
    if P::COMMITS && event.phase == PointerPhase::Move {
      self.roster.focus(&self.value);
    }

    Prevent::None
  }

  fn on_blur(&self, _: &ControlHandle) {
    if self.pressed.get_untracked() {
      self.pressed.set(false);
    }
  }

  fn on_access(&self, request: AccessRequest, _: &ControlHandle) -> bool {
    match request {
      AccessRequest::Activate | AccessRequest::Toggle => {
        self.roster.focus(&self.value);
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
      selected: P::SELECTS.then_some(chosen),
      actions: vec![AccessRequest::Activate],
      ..AccessFacts::default()
    }
  }

  fn focusable(&self) -> bool {
    let stop = if P::CHOOSE_ON_MOVE {
      self.roster.stop()
    } else {
      self.roster.active().or_else(|| self.roster.stop())
    };

    stop.as_ref() == Some(&self.value)
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
