//! A radio group's behavior, and the behavior of each member in it.

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
  Ring,
  Role,
};
use skene_reactive::{Get, Memo, Set, Signal, With, on_cleanup, provide_context, use_context};

use super::{Behavior, ControlHandle, LabelPlacement, Roster};
use crate::realize::with_sink;

/// What a [`Roving`] group shares with the members inside it, through context.
///
/// Members may sit at any depth under the group and still belong to it.
#[derive(Clone)]
#[must_use]
pub struct RovingGroup {
  chosen: Rc<dyn Fn() -> Option<String>>,
  choose: Rc<dyn Fn(String)>,
  roster: Roster,
  tab_stop: Memo<Option<String>>,
}

impl std::fmt::Debug for RovingGroup {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("RovingGroup")
      .finish_non_exhaustive()
  }
}

impl RovingGroup {
  /// The chosen member's value, tracked.
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

  /// The members, in tree order.
  #[inline(always)]
  pub const fn roster(&self) -> &Roster {
    &self.roster
  }

  /// Whether `value` is the group's one tab stop: the chosen member when it is enabled,
  /// else the first enabled one.
  #[must_use]
  pub fn is_tab_stop(&self, value: &str) -> bool {
    self.tab_stop.with(|stop| stop.as_deref() == Some(value))
  }

  /// Chooses the enabled member `step` places from the one holding `value`, wrapping at
  /// both ends, and focuses it.
  pub fn step_from<P: Platform>(&self, value: &str, step: isize) {
    let members = self.roster.enabled::<P>();
    let Some(index) = members.iter().position(|(_, held)| held == value)
    else {
      return;
    };
    let Some(next) = Ring::new(members.len()).step_from(index, step)
    else {
      return;
    };
    let Some((node, value)) = members.get(next).cloned()
    else {
      return;
    };

    self.choose(value);
    with_sink::<P, _>(node, |sink| P::default().focus(sink));
  }
}

/// A radio group: the members inside it are one tab stop, the arrow keys move between
/// them choosing as they go, and exactly one is chosen.
///
/// Controlled: the chosen value is what [`derive_chosen`](Self::derive_chosen) reads,
/// and choosing only calls [`handle_choose`](Self::handle_choose) with the value asked
/// for; [`bind_chosen`](Self::bind_chosen) wires both to one signal. The group takes no
/// focus of its own; its members are [`RadioMember`] controls, at any depth inside it.
#[derive(Default)]
#[must_use]
pub struct Roving {
  chosen: Option<Rc<dyn Fn() -> Option<String>>>,
  on_choose: Option<Rc<dyn Fn(String)>>,
}

impl std::fmt::Debug for Roving {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Roving").finish_non_exhaustive()
  }
}

impl Roving {
  /// A group with nothing chosen and no handler.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets the chosen member's value.
  #[inline(always)]
  pub fn set_chosen(mut self, chosen: impl Into<String>) -> Self {
    let chosen = chosen.into();

    self.chosen = Some(Rc::new(move || Some(chosen.clone())));

    self
  }

  /// Derives the chosen member's value from the signals `chosen` reads.
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
}

impl<P: Platform> Behavior<P> for Roving {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    ControlKind::RadioGroup
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::RadioGroup)
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
    let chosen = self.chosen.clone();
    let on_choose = self.on_choose.clone();
    let roster = Roster::new();
    let chosen: Rc<dyn Fn() -> Option<String>> =
      Rc::new(move || chosen.as_ref().and_then(|chosen| chosen()));
    let tab_stop = roster.tab_stop::<P>({
      let chosen = Rc::clone(&chosen);

      move || chosen()
    });

    roster.root_at(control.node());
    provide_context(RovingGroup {
      chosen,
      choose: Rc::new(move |value| {
        if let Some(action) = &on_choose {
          action(value);
        }
      }),
      roster,
      tab_stop,
    });
  }

  #[inline(always)]
  fn focus_order(&self) -> Option<FocusOrder> {
    None
  }
}

/// One choice within a [`Roving`] group.
///
/// A click, `Space` or the platform's own check chooses it, the arrow keys move the
/// choice to a neighbour, and it is the group's tab stop while chosen or, with nothing
/// chosen, while first.
///
/// Outside a group it reports its role, is never checked and takes focus like any
/// control.
#[derive(Debug)]
#[must_use]
pub struct RadioMember {
  value: String,
  group: RefCell<Option<RovingGroup>>,
}

impl RadioMember {
  /// A choice carrying `value`, the string the group reports when it is chosen.
  #[inline(always)]
  pub fn new(value: impl Into<String>) -> Self {
    Self {
      value: value.into(),
      group: RefCell::new(None),
    }
  }

  /// The value this choice carries.
  #[inline(always)]
  #[must_use]
  pub fn value(&self) -> &str {
    &self.value
  }

  /// The group this member found when it attached.
  #[inline(always)]
  fn group(&self) -> Option<RovingGroup> {
    self.group.borrow().clone()
  }

  /// Whether this member is the group's chosen one.
  fn is_chosen(&self) -> bool {
    self
      .group()
      .is_some_and(|group| group.chosen().as_deref() == Some(self.value.as_str()))
  }
}

impl<P: Platform> Behavior<P> for RadioMember {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    ControlKind::Radio
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::Radio)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[EventKind::Click, EventKind::KeyDown, EventKind::Toggle]
  }

  fn attach(&self, control: &ControlHandle<P>) {
    let Some(group) = use_context::<RovingGroup>()
    else {
      return;
    };
    let node = control.node();

    group.roster.join(
      node,
      self.value.clone(),
      control.name_read(),
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
      "ArrowRight" | "ArrowDown" => group.step_from::<P>(&self.value, 1),
      "ArrowLeft" | "ArrowUp" => group.step_from::<P>(&self.value, -1),
      " " => group.choose(self.value.clone()),
      _ => return Prevent::None,
    }

    Prevent::All
  }

  fn on_toggle(&self, checked: bool, _control: &ControlHandle<P>) {
    if !checked {
      return;
    }

    if let Some(group) = self.group() {
      group.choose(self.value.clone());
    }
  }

  fn access(&self) -> Vec<AccessProp> {
    vec![AccessProp::Checked(self.is_chosen().into())]
  }

  fn focus_order(&self) -> Option<FocusOrder> {
    match self.group() {
      Some(group) if !group.is_tab_stop(&self.value) => Some(FocusOrder::Skip),
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
