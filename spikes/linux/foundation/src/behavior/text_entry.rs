//! A text entry.

use std::rc::Rc;

use crate::{
  behavior::{Behavior, ControlHandle, Response},
  seam::{
    access::{AccessFacts, Role},
    content::{Edit, TextSelection, contents},
    control::{AccessRequest, Element, LabelPlacement, Prevent},
    report::KeyEvent,
  },
  text::editing::{Binding, Command},
  untrack,
};

/// What a text entry's key table does with a key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EntryAct {
  /// A key bound to an editing command, or a paste.
  Edit(Binding),
  /// `Enter` in an entry of one line: the entry is submitted.
  Activate,
}

/// A text entry's key table: the editing core's ([`Binding::for_key`]), `Enter` submitting an
/// entry of one line, and a read-only entry keeping its moves, its selection and its copy but
/// no edit. A secure entry (a password) offers neither copy nor cut.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[must_use]
pub struct EntryKeys {
  /// Whether it holds several lines.
  pub multiline: bool,
  /// Whether its text can be selected but not changed.
  pub read_only: bool,
  /// Whether it hides what is typed.
  pub secure: bool,
}

impl EntryKeys {
  /// An entry of several lines where `multiline`.
  #[inline(always)]
  pub const fn new(multiline: bool) -> Self {
    Self {
      multiline,
      read_only: false,
      secure: false,
    }
  }

  /// This entry, read only where `read_only`.
  #[inline(always)]
  pub const fn with_read_only(mut self, read_only: bool) -> Self {
    self.read_only = read_only;

    self
  }

  /// This entry, hiding what is typed where `secure`.
  #[inline(always)]
  pub const fn with_secure(mut self, secure: bool) -> Self {
    self.secure = secure;

    self
  }

  /// Whether the entry runs `command`: a secure entry neither copies nor cuts.
  #[must_use]
  pub const fn allows(&self, command: &Command) -> bool {
    !(self.secure && matches!(command, Command::Copy | Command::Cut))
  }

  /// A key pressed or released while the entry holds focus.
  pub fn key(&self, event: &KeyEvent) -> Response<EntryAct> {
    let modifiers = event.modifiers;

    if event.down
      && event.key == "Enter"
      && !self.multiline
      && !modifiers.control
      && !modifiers.alt
      && !modifiers.meta
    {
      return Response::act(EntryAct::Activate);
    }

    match Binding::for_key(event, self.multiline) {
      Some(Binding::Command(command)) if !self.allows(&command) => Response::CONSUMED,
      Some(binding) => Response::act(EntryAct::Edit(binding)),
      None => Response::IGNORED,
    }
  }
}

/// A text entry's behavior: the realization's own text view ([`Element::Editable`]) holding a
/// controlled text and selection, its editing the platform's default for every key no
/// handler prevents; `Enter` in an entry of one line runs its activation. Its label sits
/// beside it.
#[must_use]
pub struct TextEntry {
  lines: u16,
  text: Rc<dyn Fn() -> String>,
  change: Option<Rc<dyn Fn(String)>>,
  selection: Option<Rc<dyn Fn() -> TextSelection>>,
  select: Option<Rc<dyn Fn(TextSelection)>>,
  read_only: Option<Rc<dyn Fn() -> bool>>,
  placeholder: Option<Rc<dyn Fn() -> Rc<str>>>,
  secure: Option<Rc<dyn Fn() -> bool>>,
  keyboard: Option<Rc<dyn Fn() -> Rc<str>>>,
  activate: Option<Rc<dyn Fn()>>,
}

impl TextEntry {
  /// An empty entry showing `lines` lines at once: one a field, more an editor.
  pub fn new(lines: u16) -> Self {
    Self {
      lines: lines.max(1),
      text: Rc::new(String::new),
      change: None,
      selection: None,
      select: None,
      read_only: None,
      placeholder: None,
      secure: None,
      keyboard: None,
      activate: None,
    }
  }

  /// Whether it holds several lines.
  #[must_use]
  pub const fn is_multiline(&self) -> bool {
    self.lines > 1
  }

  /// This entry, showing what `text` returns, kept current.
  pub fn derive_text(mut self, text: impl Fn() -> String + 'static) -> Self {
    self.text = Rc::new(text);

    self
  }

  /// This entry, asking `change` for the whole text each edit proposes.
  pub fn handle_change(mut self, change: impl Fn(String) + 'static) -> Self {
    self.change = Some(Rc::new(change));

    self
  }

  /// This entry, its selection at what `selection` returns (in grapheme clusters), kept
  /// current: the selection is then controlled.
  pub fn derive_selection(mut self, selection: impl Fn() -> TextSelection + 'static) -> Self {
    self.selection = Some(Rc::new(selection));

    self
  }

  /// This entry, asking `select` for each move of the caret or the selection where the
  /// selection is controlled.
  pub fn handle_selection_change(mut self, select: impl Fn(TextSelection) + 'static) -> Self {
    self.select = Some(Rc::new(select));

    self
  }

  /// This entry, read only while `read_only` returns true.
  pub fn derive_read_only(mut self, read_only: impl Fn() -> bool + 'static) -> Self {
    self.read_only = Some(Rc::new(read_only));

    self
  }

  /// This entry, showing what `placeholder` returns while it is empty.
  pub fn derive_placeholder(mut self, placeholder: impl Fn() -> Rc<str> + 'static) -> Self {
    self.placeholder = Some(Rc::new(placeholder));

    self
  }

  /// This entry, hiding what is typed while `secure` returns true.
  pub fn derive_secure(mut self, secure: impl Fn() -> bool + 'static) -> Self {
    self.secure = Some(Rc::new(secure));

    self
  }

  /// This entry, asking the platform's input for the keyboard `keyboard` names (`text`,
  /// `email`, `number`, `phone`, `url`).
  pub fn derive_keyboard(mut self, keyboard: impl Fn() -> Rc<str> + 'static) -> Self {
    self.keyboard = Some(Rc::new(keyboard));

    self
  }

  /// This entry, running `activate` when the reader submits it (`Enter` in a field).
  pub fn handle_activate(mut self, activate: impl Fn() + 'static) -> Self {
    self.activate = Some(Rc::new(activate));

    self
  }
}

impl Behavior for TextEntry {
  fn role(&self) -> Role {
    if self.is_multiline() {
      Role::TextEditor
    } else {
      Role::TextField
    }
  }

  fn element(&self) -> Element {
    Element::Editable { lines: self.lines }
  }

  fn label_placement(&self) -> LabelPlacement {
    LabelPlacement::Beside
  }

  fn attach(&self, control: &ControlHandle) {
    let (text, selection) = (Rc::clone(&self.text), self.selection.clone());
    let truth = {
      let (text, selection) = (Rc::clone(&text), selection.clone());

      move || Edit {
        text: text().into(),
        selection: selection.as_ref().map(|selection| selection()),
      }
    };
    let (change, select) = (self.change.clone(), self.select.clone());

    control.control(contents::EDIT, truth, move |edit: Edit| {
      let (now, selected) = untrack(|| (text(), selection.as_ref().map(|selection| selection())));

      if let Some(change) = &change
        && *now != *edit.text
      {
        change(edit.text.to_string());
      }

      if let (Some(select), Some(moved)) = (&select, edit.selection)
        && selected != Some(moved)
      {
        select(moved);
      }
    });

    if let Some(read_only) = self.read_only.clone() {
      control.bind(contents::READ_ONLY, move || read_only());
    }

    if let Some(placeholder) = self.placeholder.clone() {
      control.bind(contents::PLACEHOLDER, move || placeholder());
    }

    if !self.is_multiline() {
      if let Some(secure) = self.secure.clone() {
        control.bind(contents::SECURE, move || secure());
      }

      if let Some(keyboard) = self.keyboard.clone() {
        control.bind(contents::KEYBOARD, move || keyboard());
      }
    }
  }

  fn on_key(&self, event: &KeyEvent, _: &ControlHandle) -> Prevent {
    let modifiers = event.modifiers;

    if let Some(activate) = &self.activate
      && event.down
      && event.key == "Enter"
      && !self.is_multiline()
      && !modifiers.control
      && !modifiers.alt
      && !modifiers.meta
    {
      activate();

      return Prevent::All;
    }

    Prevent::None
  }

  fn on_access(&self, request: AccessRequest, control: &ControlHandle) -> bool {
    match request {
      AccessRequest::Activate | AccessRequest::Focus => {
        control.focus();

        true
      }
      _ => false,
    }
  }

  fn access(&self) -> AccessFacts {
    AccessFacts {
      read_only: self.read_only.as_ref().is_some_and(|read_only| read_only()),
      ..AccessFacts::default()
    }
  }
}
