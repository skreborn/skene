//! A menu's behavior, and the behavior of each item in it.

use std::{cell::RefCell, rc::Rc};

use skene_core::{
  ControlKind,
  EventKind,
  FocusOrder,
  KeyEvent,
  NodeId,
  Platform,
  Prevent,
  Role,
  Scene,
};
use skene_reactive::{on_cleanup, provide_context, use_context};

use super::{Behavior, ControlHandle, Roster, TypeAhead};

/// What a [`Menu`] shares with the items inside it, through context.
///
/// Items may sit at any depth under the menu and still belong to it.
#[derive(Clone)]
#[must_use]
pub struct MenuGroup {
  activate: Rc<dyn Fn(String)>,
  roster: Roster,
  type_ahead: Rc<TypeAhead>,
}

impl std::fmt::Debug for MenuGroup {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("MenuGroup").finish_non_exhaustive()
  }
}

impl MenuGroup {
  /// Asks for the item holding `value` to run.
  #[inline(always)]
  pub fn activate(&self, value: impl Into<String>) {
    (self.activate)(value.into());
  }

  /// The items, in tree order.
  #[inline(always)]
  pub const fn roster(&self) -> &Roster {
    &self.roster
  }

  /// The enabled items in tree order: those whose control is not disabled; tracked.
  #[must_use]
  pub fn enabled<P: Platform>(&self) -> Vec<(NodeId, String)> {
    self.roster.enabled::<P>()
  }

  /// Whether `value` is the menu's one tab stop: the first enabled item.
  #[must_use]
  pub fn is_tab_stop<P: Platform>(&self, value: &str) -> bool {
    self
      .enabled::<P>()
      .first()
      .is_some_and(|(_, first)| first == value)
  }

  /// Focuses the first enabled item, if there is one.
  pub fn focus_first<P: Platform>(&self) {
    let members = self.enabled::<P>();

    if let Some((node, _)) = members.first() {
      focus::<P>(*node);
    }
  }

  /// Focuses the enabled item `step` places from the one holding `value`, wrapping at
  /// both ends as a browser's menu does, running nothing.
  pub fn step_from<P: Platform>(&self, value: &str, step: isize) {
    let members = self.enabled::<P>();
    let Some(index) = members.iter().position(|(_, held)| held == value)
    else {
      return;
    };
    let count = members.len();
    let next = match step {
      isize::MIN => 0,
      isize::MAX => count.saturating_sub(1),
      _ => (index.cast_signed() + step)
        .rem_euclid(count.cast_signed())
        .cast_unsigned(),
    };

    if let Some((node, _)) = members.get(next) {
      focus::<P>(*node);
    }
  }

  /// Feeds a key typed on the item holding `value` into the menu's type-ahead and
  /// focuses the item its buffer picks; `false` for a key that is no character.
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

/// Moves focus to `node`.
fn focus<P: Platform>(node: NodeId) {
  let sink = Scene::<P>::with(|scene| scene.sink(node));

  if let Some(sink) = sink {
    P::default().focus(&sink);
  }
}

/// A menu: a list of commands whose items are one tab stop.
///
/// The arrow keys move focus between the enabled items with wrapping, `Home` and `End`
/// reach the ends, typing moves focus to the item whose name the characters begin, and
/// `Enter`, `Space` or a click runs the focused one.
///
/// Running an item only calls [`handle_activate`](Self::handle_activate) with the
/// item's value; what happens next, the menu closing included, is its user's. The
/// menu takes no focus of its own; its items are [`MenuOption`] controls, at any
/// depth inside it, tracked by its [`roster`](Self::roster).
#[must_use]
pub struct Menu {
  on_activate: Option<Rc<dyn Fn(String)>>,
  roster: Roster,
}

impl std::fmt::Debug for Menu {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Menu").finish_non_exhaustive()
  }
}

impl Menu {
  /// A menu with no handler, its roster owned by the current owner.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      on_activate: None,
      roster: Roster::new(),
    }
  }

  /// Runs `action` with the value of the item run.
  #[inline(always)]
  pub fn handle_activate(mut self, action: impl Fn(String) + 'static) -> Self {
    self.on_activate = Some(Rc::new(action));

    self
  }

  /// The items the menu tracks, in tree order once the control builds.
  #[inline(always)]
  pub const fn roster(&self) -> &Roster {
    &self.roster
  }
}

impl Default for Menu {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Behavior<P> for Menu {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    ControlKind::Menu
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::Menu)
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
    let on_activate = self.on_activate.clone();
    let roster = self.roster.clone();

    roster.root_at(control.node());
    provide_context(MenuGroup {
      activate: Rc::new(move |value| {
        if let Some(action) = &on_activate {
          action(value);
        }
      }),
      roster,
      type_ahead: Rc::new(TypeAhead::new()),
    });
  }

  #[inline(always)]
  fn focus_order(&self) -> Option<FocusOrder> {
    None
  }
}

/// One item within a [`Menu`].
///
/// A click, `Enter` or `Space` runs it, the arrow keys move focus to an enabled
/// neighbour, `Home` and `End` to the ends, a typed character to the item the menu's
/// type-ahead picks, and it is the menu's tab stop while it is the first enabled item.
///
/// Outside a menu it reports its role and takes focus like any control.
#[derive(Debug)]
#[must_use]
pub struct MenuOption {
  value: String,
  group: RefCell<Option<MenuGroup>>,
}

impl MenuOption {
  /// An item carrying `value`, the string the menu reports when it runs.
  #[inline(always)]
  pub fn new(value: impl Into<String>) -> Self {
    Self {
      value: value.into(),
      group: RefCell::new(None),
    }
  }

  /// The value this item carries.
  #[inline(always)]
  #[must_use]
  pub fn value(&self) -> &str {
    &self.value
  }

  /// The menu this item found when it attached.
  #[inline(always)]
  fn group(&self) -> Option<MenuGroup> {
    self.group.borrow().clone()
  }
}

impl<P: Platform> Behavior<P> for MenuOption {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    ControlKind::MenuItem
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::MenuItem)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[EventKind::Click, EventKind::KeyDown]
  }

  fn attach(&self, control: &ControlHandle<P>) {
    let Some(group) = use_context::<MenuGroup>()
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
      group.activate(self.value.clone());
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
      "Enter" => group.activate(self.value.clone()),
      " " if !group.type_ahead_from::<P>(&self.value, event) => {
        group.activate(self.value.clone());
      }
      _ if group.type_ahead_from::<P>(&self.value, event) => {}
      _ => return Prevent::None,
    }

    Prevent::All
  }

  fn focus_order(&self) -> Option<FocusOrder> {
    match self.group() {
      Some(group) if !group.is_tab_stop::<P>(&self.value) => Some(FocusOrder::Skip),
      _ => Some(FocusOrder::Auto),
    }
  }
}
