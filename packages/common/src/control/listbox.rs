//! A listbox's behavior, and the behavior of each option in it.

use std::{cell::RefCell, rc::Rc};

use skene_core::{
  AccessProp,
  ControlKind,
  ControlState,
  EventKind,
  FocusOrder,
  KeyEvent,
  Platform,
  Prevent,
  Role,
};
use skene_reactive::{Get, Set, Signal, on_cleanup, provide_context, use_context};

use super::{Behavior, ControlHandle, Roster, TypeAhead};

/// What a [`Listbox`] shares with the options inside it, through context.
///
/// Options may sit at any depth under the listbox and still belong to it.
#[derive(Clone)]
#[must_use]
pub struct ListboxGroup {
  chosen: Rc<dyn Fn() -> Option<String>>,
  choose: Rc<dyn Fn(String)>,
  roster: Roster,
  type_ahead: Rc<TypeAhead>,
  commit_on_tab: bool,
}

impl ListboxGroup {
  /// A group whose options join `roster`, answering `chosen` for the chosen value and
  /// asking `choose` for a choice; provided as context by a control listing options.
  #[inline(always)]
  pub fn new(
    chosen: impl Fn() -> Option<String> + 'static,
    choose: impl Fn(String) + 'static,
    roster: Roster,
  ) -> Self {
    Self {
      chosen: Rc::new(chosen),
      choose: Rc::new(choose),
      roster,
      type_ahead: Rc::new(TypeAhead::new()),
      commit_on_tab: false,
    }
  }

  /// Sets whether `Tab` from a focused option chooses it before focus moves on, as a
  /// picker's list does; off by default.
  #[inline(always)]
  pub const fn set_commit_on_tab(mut self, commit_on_tab: bool) -> Self {
    self.commit_on_tab = commit_on_tab;

    self
  }

  /// The chosen option's value, tracked.
  #[inline(always)]
  #[must_use]
  pub fn chosen(&self) -> Option<String> {
    (self.chosen)()
  }

  /// Asks for `value` to be chosen.
  #[inline(always)]
  pub fn choose(&self, value: impl Into<String>) {
    (self.choose)(value.into());
  }

  /// The options, in tree order.
  #[inline(always)]
  pub const fn roster(&self) -> &Roster {
    &self.roster
  }

  /// Whether `value` is the list's one tab stop: the chosen option when it is enabled,
  /// else the first enabled one.
  #[must_use]
  pub fn is_tab_stop<P: Platform>(&self, value: &str) -> bool {
    let enabled = self.roster.enabled::<P>();

    match self
      .chosen()
      .filter(|chosen| enabled.iter().any(|(_, held)| held == chosen))
    {
      Some(chosen) => chosen == value,
      None => enabled.first().is_some_and(|(_, first)| first == value),
    }
  }

  /// Focuses the enabled option `step` places from the one holding `value`, clamped at
  /// both ends, choosing nothing.
  pub fn step_from<P: Platform>(&self, value: &str, step: isize) {
    let members = self.roster.enabled::<P>();
    let Some(index) = members.iter().position(|(_, held)| held == value)
    else {
      return;
    };
    let Some(last) = members.len().checked_sub(1)
    else {
      return;
    };

    self
      .roster
      .focus_at::<P>(index.saturating_add_signed(step).min(last));
  }

  /// Feeds a key typed on the option holding `value` into the list's type-ahead and
  /// focuses the option its buffer picks; `false` for a key that is no character.
  #[must_use]
  pub fn type_ahead_from<P: Platform>(&self, value: &str, event: &KeyEvent) -> bool {
    let Some(buffer) = self.type_ahead.typed::<P>(event)
    else {
      return false;
    };

    if let Some(index) = self.roster.typed::<P>(&buffer, Some(value)) {
      self.roster.focus_at::<P>(index);
    }

    true
  }
}

/// A listbox: the options inside it are one tab stop, the arrow keys move focus
/// between them without wrapping, `Home` and `End` reach the ends, typing moves focus
/// to the option whose name the characters begin (a character repeated cycling through
/// the options starting with it), and `Enter`, `Space` or a click chooses the focused
/// one.
///
/// Controlled: the chosen value is what [`derive_chosen`](Self::derive_chosen) reads,
/// and choosing only calls [`handle_choose`](Self::handle_choose) with the value asked
/// for; [`bind_chosen`](Self::bind_chosen) wires both to one signal. The listbox takes
/// no focus of its own; its options are [`ListItem`] controls, at any depth inside it,
/// tracked by its [`roster`](Self::roster).
#[must_use]
pub struct Listbox {
  chosen: Option<Rc<dyn Fn() -> Option<String>>>,
  on_choose: Option<Rc<dyn Fn(String)>>,
  roster: Roster,
  commit_on_tab: bool,
}

impl Listbox {
  /// A listbox with nothing chosen and no handler, its roster owned by the current
  /// owner.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      chosen: None,
      on_choose: None,
      roster: Roster::new(),
      commit_on_tab: false,
    }
  }

  /// Sets whether `Tab` from a focused option chooses it before focus moves on, as a
  /// picker's list does; off by default, where `Tab` only leaves the list.
  #[inline(always)]
  pub const fn set_commit_on_tab(mut self, commit_on_tab: bool) -> Self {
    self.commit_on_tab = commit_on_tab;

    self
  }

  /// Shares `roster` with the listbox, so options built under a group holding it
  /// belong to the listbox as well.
  #[inline(always)]
  pub fn set_roster(mut self, roster: Roster) -> Self {
    self.roster = roster;

    self
  }

  /// Sets the chosen option's value.
  #[inline(always)]
  pub fn set_chosen(mut self, chosen: impl Into<String>) -> Self {
    let chosen = chosen.into();

    self.chosen = Some(Rc::new(move || Some(chosen.clone())));

    self
  }

  /// Derives the chosen option's value from the signals `chosen` reads.
  #[inline(always)]
  pub fn derive_chosen(mut self, chosen: impl Fn() -> Option<String> + 'static) -> Self {
    self.chosen = Some(Rc::new(chosen));

    self
  }

  /// Runs `action` with the value a choice asks for.
  #[inline(always)]
  pub fn handle_choose(mut self, action: impl Fn(String) + 'static) -> Self {
    self.on_choose = Some(Rc::new(action));

    self
  }

  /// Binds the chosen value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_chosen(self, signal: Signal<Option<String>>) -> Self {
    self
      .derive_chosen(move || signal.get())
      .handle_choose(move |value| signal.set(Some(value)))
  }

  /// The options the listbox tracks, in tree order once the control builds.
  #[inline(always)]
  pub const fn roster(&self) -> &Roster {
    &self.roster
  }
}

impl Default for Listbox {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Behavior<P> for Listbox {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    ControlKind::Listbox
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::Listbox)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[]
  }

  #[inline(always)]
  fn label_leads(&self) -> bool {
    true
  }

  fn attach(&self, control: &ControlHandle<P>) {
    let chosen = self.chosen.clone();
    let on_choose = self.on_choose.clone();
    let roster = self.roster.clone();

    roster.root_at(control.node());
    provide_context(ListboxGroup {
      chosen: Rc::new(move || chosen.as_ref().and_then(|chosen| chosen())),
      choose: Rc::new(move |value| {
        if let Some(action) = &on_choose {
          action(value);
        }
      }),
      roster,
      type_ahead: Rc::new(TypeAhead::new()),
      commit_on_tab: self.commit_on_tab,
    });
  }

  #[inline(always)]
  fn focus_order(&self) -> Option<FocusOrder> {
    None
  }
}

/// One option within a [`Listbox`]: a click, `Enter` or `Space` chooses it, the arrow
/// keys move focus to a neighbour, `Home` and `End` to the ends, a typed character
/// to the option the list's type-ahead picks, and it is the list's tab stop while
/// chosen or, with nothing chosen, while first.
///
/// Outside a listbox it reports its role, is never selected and takes focus like any
/// control.
#[must_use]
pub struct ListItem {
  value: String,
  group: RefCell<Option<ListboxGroup>>,
}

impl ListItem {
  /// An option carrying `value`, the string the listbox reports when it is chosen.
  #[inline(always)]
  pub fn new(value: impl Into<String>) -> Self {
    Self {
      value: value.into(),
      group: RefCell::new(None),
    }
  }

  /// The value this option carries.
  #[inline(always)]
  #[must_use]
  pub fn value(&self) -> &str {
    &self.value
  }

  /// The listbox this option found when it attached.
  #[inline(always)]
  fn group(&self) -> Option<ListboxGroup> {
    self.group.borrow().clone()
  }

  /// Whether this option is the listbox's chosen one.
  fn is_chosen(&self) -> bool {
    self
      .group()
      .is_some_and(|group| group.chosen().as_deref() == Some(self.value.as_str()))
  }
}

impl<P: Platform> Behavior<P> for ListItem {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    ControlKind::ListboxOption
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::ListboxOption)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[EventKind::Click, EventKind::KeyDown]
  }

  fn attach(&self, control: &ControlHandle<P>) {
    let Some(group) = use_context::<ListboxGroup>()
    else {
      return;
    };
    let node = control.node();

    group.roster.join(
      node,
      self.value.clone(),
      control.name().map(str::to_owned),
      control.disabled_read(),
    );

    let roster = group.roster.clone();

    on_cleanup(move || roster.leave(node));
    *self.group.borrow_mut() = Some(group);
    control.refresh();
  }

  fn on_click(&self, _control: &ControlHandle<P>) -> Prevent {
    if let Some(group) = self.group() {
      group.choose(self.value.clone());
    }

    Prevent::Bubbling
  }

  fn on_key(&self, event: &KeyEvent, _control: &ControlHandle<P>) -> Prevent {
    let Some(group) = self.group()
    else {
      return Prevent::None;
    };

    match event.key.as_str() {
      "ArrowDown" => group.step_from::<P>(&self.value, 1),
      "ArrowUp" => group.step_from::<P>(&self.value, -1),
      "Home" => group.step_from::<P>(&self.value, isize::MIN),
      "End" => group.step_from::<P>(&self.value, isize::MAX),
      "Enter" => group.choose(self.value.clone()),
      " " if !group.type_ahead_from::<P>(&self.value, event) => {
        group.choose(self.value.clone());
      }
      "Tab" if group.commit_on_tab => {
        group.choose(self.value.clone());

        return Prevent::None;
      }
      _ if group.type_ahead_from::<P>(&self.value, event) => {}
      _ => return Prevent::None,
    }

    Prevent::All
  }

  fn access(&self) -> Vec<AccessProp> {
    vec![AccessProp::Selected(self.is_chosen())]
  }

  fn focus_order(&self) -> Option<FocusOrder> {
    match self.group() {
      Some(group) if !group.is_tab_stop::<P>(&self.value) => Some(FocusOrder::Skip),
      _ => Some(FocusOrder::Auto),
    }
  }

  fn state(&self) -> ControlState {
    ControlState {
      checked: self.is_chosen().into(),
      ..ControlState::DEFAULT
    }
  }
}
