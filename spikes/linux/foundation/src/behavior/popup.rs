//! A closed face that opens a list of options: a picker.

use std::{cell::RefCell, rc::Rc};

use crate::{
  Signal,
  behavior::{
    Behavior, CollectionKeys, ControlHandle, Orientation, Response, Roster, TypeAhead, plain,
  },
  seam::{
    access::{AccessFacts, Role},
    control::{AccessRequest, ControlState, LabelPlacement, Prevent},
    report::KeyEvent,
    runtime::Cancel,
  },
  untrack,
};

/// What a closed popup's face does.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PopupAct {
  /// Open the list.
  Open,
  /// Choose an option, the list staying closed (a closed face's arrows).
  Choose(usize),
}

/// A closed popup face's key table: it opens on `Space`, `Alt` with an arrow down or up, and
/// `F4`; its arrows, `Home` and `End` choose without opening, as a Chromium `<select>` on
/// Linux and Windows does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[must_use]
pub struct PopupKeys {
  /// How many options.
  pub count: usize,
  /// The chosen option.
  pub chosen: Option<usize>,
}

impl PopupKeys {
  /// A closed face over `count` options with `chosen`.
  #[inline(always)]
  pub const fn new(count: usize, chosen: Option<usize>) -> Self {
    Self { count, chosen }
  }

  /// Whether `event` opens or closes an open list from anywhere in it: `Alt` with an arrow
  /// down or up, or `F4`.
  #[must_use]
  pub fn toggles(event: &KeyEvent) -> bool {
    (event.modifiers.alt && matches!(event.key.as_str(), "ArrowDown" | "ArrowUp"))
      || event.key == "F4"
  }

  /// A key pressed while the closed face holds focus.
  pub fn key(&self, event: &KeyEvent) -> Response<PopupAct> {
    let modifiers = event.modifiers;

    if !event.down || modifiers.control || modifiers.meta {
      return Response::IGNORED;
    }

    if Self::toggles(event) {
      return Response::act(PopupAct::Open);
    }

    if modifiers.alt {
      return Response::IGNORED;
    }

    match event.key.as_str() {
      " " => Response::act(PopupAct::Open),
      "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Home" | "End" | "PageUp"
      | "PageDown" => {
        let list = CollectionKeys::new(self.count, self.chosen, Orientation::Both).with_page(1);

        list.key(event).map(PopupAct::Choose)
      }
      _ => Response::IGNORED,
    }
  }
}

/// A popup face's behavior (a picker's): [`PopupKeys`] while closed, a click opening and
/// closing it, typed letters finding an option by its name ([`TypeAhead`]); while open, focus
/// is on its list's options, and `Escape` and `Tab` climbing from them close it and bring
/// focus back to the face, `Tab` then moving on.
///
/// Its list is a collection over the same [`Roster`], which commits a choice by closing it.
#[must_use]
pub struct Popup {
  roster: Rc<Roster>,
  names: Rc<dyn Fn() -> Vec<Rc<str>>>,
  open: Signal<bool>,
  type_ahead: Rc<RefCell<TypeAhead>>,
  reset: RefCell<Option<Cancel>>,
}

impl Popup {
  /// A closed popup choosing through `roster`, its options' names what `names` returns.
  pub fn new(roster: Rc<Roster>, names: impl Fn() -> Vec<Rc<str>> + 'static) -> Self {
    Self {
      roster,
      names: Rc::new(names),
      open: Signal::new(false),
      type_ahead: Rc::new(RefCell::new(TypeAhead::new())),
      reset: RefCell::new(None),
    }
  }

  /// Whether the list is open: what its overlay binds.
  #[must_use]
  pub const fn open(&self) -> Signal<bool> {
    self.open
  }

  /// Opens the list, focus on its chosen option (its first where none is chosen).
  pub fn show(&self) {
    if !self.open.get_untracked() {
      self.open.set(true);
    }

    let target = untrack(|| self.roster.chosen()).unwrap_or(0);

    self.roster.focus(target);
  }

  /// Closes the list, choosing nothing, and brings focus back to the face.
  pub fn hide(&self, control: &ControlHandle) {
    if self.open.get_untracked() {
      self.open.set(false);
      control.focus();
    }
  }

  /// The option `text` finds, typed into the face or the open list, from `current`.
  fn type_ahead(
    &self,
    text: &str,
    current: Option<usize>,
    control: &ControlHandle,
  ) -> Option<usize> {
    let names = untrack(|| (self.names)());
    let names: Vec<&str> = names.iter().map(|name| &**name).collect();
    let found = self
      .type_ahead
      .borrow_mut()
      .type_text(text, &names, current);
    let buffer = Rc::clone(&self.type_ahead);
    let reset = control.schedule(TypeAhead::PAUSE, move || buffer.borrow_mut().reset());

    if let Some(previous) = self.reset.replace(Some(reset)) {
      previous.cancel();
    }

    found
  }

  /// The text a key types into a search, where it types one: a printable character with no
  /// modifier but `Shift`, a space only while a search is under way.
  fn typed<'a>(&self, event: &'a KeyEvent) -> Option<&'a str> {
    let mut characters = event.key.chars();
    let (Some(character), None) = (characters.next(), characters.next()) else {
      return None;
    };

    (event.down
      && plain(event)
      && !character.is_control()
      && (character != ' ' || self.type_ahead.borrow().is_active()))
    .then_some(event.key.as_str())
  }
}

impl Behavior for Popup {
  fn role(&self) -> Role {
    Role::ComboBox
  }

  fn label_placement(&self) -> LabelPlacement {
    LabelPlacement::Beside
  }

  fn attach(&self, control: &ControlHandle) {
    let (open, control) = (self.open, control.clone());

    self.roster.set_commit(move |_| {
      if open.get_untracked() {
        open.set(false);
        control.focus();
      }
    });
  }

  fn on_click(&self, control: &ControlHandle) -> Prevent {
    if self.open.get_untracked() {
      self.hide(control);
    } else {
      self.show();
    }

    Prevent::All
  }

  fn on_key(&self, event: &KeyEvent, control: &ControlHandle) -> Prevent {
    if self.open.get_untracked() {
      if !event.down {
        return Prevent::None;
      }

      if let Some(text) = self.typed(event) {
        let current = untrack(|| self.roster.chosen());

        if let Some(found) = self.type_ahead(text, current, control) {
          self.roster.focus(found);
        }

        return Prevent::All;
      }

      return match event.key.as_str() {
        "Escape" => {
          self.hide(control);

          Prevent::All
        }
        "Tab" => {
          self.hide(control);

          Prevent::None
        }
        _ if PopupKeys::toggles(event) => {
          self.hide(control);

          Prevent::All
        }
        _ => Prevent::None,
      };
    }

    let chosen = untrack(|| self.roster.chosen());

    if let Some(text) = self.typed(event) {
      if let Some(found) = self.type_ahead(text, chosen, control) {
        self.roster.choose(found);
      }

      return Prevent::All;
    }

    let count = untrack(|| (self.names)().len());
    let response = PopupKeys::new(count, chosen).key(event);

    match response.act {
      Some(PopupAct::Open) => self.show(),
      Some(PopupAct::Choose(index)) => self.roster.choose(index),
      None => {}
    }

    response.verdict()
  }

  fn on_access(&self, request: AccessRequest, control: &ControlHandle) -> bool {
    match request {
      AccessRequest::Activate => {
        let _ = self.on_click(control);

        true
      }
      AccessRequest::Expand => {
        self.show();

        true
      }
      AccessRequest::Collapse => {
        self.hide(control);

        true
      }
      _ => false,
    }
  }

  fn access(&self) -> AccessFacts {
    let names = (self.names)();
    let chosen = self.roster.chosen();

    AccessFacts {
      expanded: Some(self.open.get()),
      has_popup: true,
      value_text: Some(
        chosen
          .and_then(|index| names.get(index).cloned())
          .unwrap_or_else(|| Rc::from("")),
      ),
      actions: vec![AccessRequest::Activate],
      ..AccessFacts::default()
    }
  }

  fn state(&self) -> ControlState {
    ControlState {
      open: self.open.get(),
      ..ControlState::DEFAULT
    }
  }
}
