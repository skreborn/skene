//! A text field's and a text editor's behavior.

use std::rc::Rc;

use skene_reactive::Signal;

use super::{Behavior, ControlHandle, Controlled};
use crate::{
  EditController,
  EditTruth,
  EventKind,
  Keyboard,
  NativeElement,
  Role,
  TextSelection,
  contents,
  realize::Source,
};

/// A text entry: a single-line field, or an editor showing several lines at once.
///
/// Editing keys are the platform's; the behavior only controls the entry's [`Edit`]
/// content through an [`EditController`]. A reader's change is reported with whether its
/// toolkit shows it already, and answered with the verdict, which is the only
/// write-back: a change the truth holds already is accepted unasked; otherwise the text
/// it changes is asked for first, then the selection it moves, and a change the truth
/// then holds, or any change while nothing is derived, is accepted, while any other is
/// answered with the truth.
///
/// Controlled: the text shown is what [`derive_value`](Self::derive_value) reads, and an
/// edit only calls [`handle_change`](Self::handle_change) with the text asked for;
/// [`bind_value`](Self::bind_value) wires both to one signal. The selection is
/// controlled the same way: [`derive_selection`](Self::derive_selection) places it,
/// [`handle_selection_change`](Self::handle_selection_change) hears every move of the
/// caret, and [`bind_selection`](Self::bind_selection) wires both to one signal, so
/// an app reads and writes the selection beside the value.
///
/// The text and the selection cross to the platform as one [`Edit`]: a change of the
/// text alone leaves a selection nothing derives to the platform, mapped through the
/// change, and a derived selection is placed in the text it is written with, or in the
/// text the entry shows where no text is derived.
///
/// A secure field ([`set_secure`](Self::set_secure)) masks its text, each cluster drawn
/// as a mark, and keeps it from the clipboard: copying and cutting are disabled in it,
/// pasting is not. The flag holds on a single-line field alone; an editor is never
/// secure. The keyboard ([`set_keyboard`](Self::set_keyboard)) names what the entry takes
/// and how the input method treats what is typed, for the platform's input protocol to
/// carry; a platform without one, as the terminal, has no keyboard to offer.
///
/// [`Edit`]: crate::Edit
#[must_use]
pub struct TextEntry {
  value: Controlled<String>,
  selection: Controlled<TextSelection>,
  placeholder: Option<Source<Rc<str>>>,
  secure: Option<Source<bool>>,
  keyboard: Option<Source<Keyboard>>,
  lines: u16,
}

impl std::fmt::Debug for TextEntry {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("TextEntry").finish_non_exhaustive()
  }
}

impl TextEntry {
  /// An empty single-line field.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      value: Controlled::new(),
      selection: Controlled::new(),
      placeholder: None,
      secure: None,
      keyboard: None,
      lines: 1,
    }
  }

  /// Sets the lines shown at once; more than one makes the entry an editor.
  #[inline(always)]
  pub const fn set_lines(mut self, lines: u16) -> Self {
    self.lines = if lines == 0 { 1 } else { lines };

    self
  }

  /// Sets the hint shown while the value is empty.
  pub fn set_placeholder(mut self, placeholder: impl Into<String>) -> Self {
    self.placeholder = Some(Source::Fixed(Rc::from(placeholder.into())));

    self
  }

  /// Derives the hint shown while the value is empty from the signals `placeholder`
  /// reads.
  pub fn derive_placeholder(mut self, placeholder: impl Fn() -> String + 'static) -> Self {
    self.placeholder = Some(Source::derive(move || Rc::from(placeholder())));

    self
  }

  /// Makes the field secure where `secure`: its text masked, and kept from the
  /// clipboard.
  #[inline(always)]
  pub fn set_secure(mut self, secure: bool) -> Self {
    self.secure = Some(Source::Fixed(secure));

    self
  }

  /// Derives whether the field is secure from the signals `secure` reads.
  pub fn derive_secure(mut self, secure: impl Fn() -> bool + 'static) -> Self {
    self.secure = Some(Source::derive(secure));

    self
  }

  /// Sets the keyboard the entry asks the platform for.
  #[inline(always)]
  pub fn set_keyboard(mut self, keyboard: Keyboard) -> Self {
    self.keyboard = Some(Source::Fixed(keyboard));

    self
  }

  /// Derives the keyboard the entry asks the platform for from the signals `keyboard`
  /// reads.
  pub fn derive_keyboard(mut self, keyboard: impl Fn() -> Keyboard + 'static) -> Self {
    self.keyboard = Some(Source::derive(keyboard));

    self
  }

  /// Sets the text.
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.value.set(value.into());

    self
  }

  /// Derives the text from the signals `value` reads.
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.value.derive(value);

    self
  }

  /// Runs `action` with the text an edit asks for.
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.value.handle(move |value: String| action(&value));

    self
  }

  /// Binds the text to `signal` in both directions.
  #[inline(always)]
  pub fn bind_value(mut self, signal: Signal<String>) -> Self {
    self.value.bind(signal);

    self
  }

  /// Sets the selection, in grapheme clusters of the text.
  #[inline(always)]
  pub fn set_selection(mut self, selection: TextSelection) -> Self {
    self.selection.set(selection);

    self
  }

  /// Derives the selection from the signals `selection` reads.
  pub fn derive_selection(mut self, selection: impl Fn() -> TextSelection + 'static) -> Self {
    self.selection.derive(selection);

    self
  }

  /// Runs `action` with the selection after every move of the caret or its anchor.
  pub fn handle_selection_change(mut self, action: impl Fn(TextSelection) + 'static) -> Self {
    self.selection.handle(action);

    self
  }

  /// Binds the selection to `signal` in both directions.
  #[inline(always)]
  pub fn bind_selection(mut self, signal: Signal<TextSelection>) -> Self {
    self.selection.bind(signal);

    self
  }
}

impl Default for TextEntry {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Behavior for TextEntry {
  #[inline(always)]
  fn native(&self) -> Option<NativeElement> {
    Some(if self.lines > 1 {
      NativeElement::TextEditor
    }
    else {
      NativeElement::TextField
    })
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    None
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[]
  }

  #[inline(always)]
  fn lines(&self) -> u16 {
    self.lines
  }

  fn attach(&self, control: &ControlHandle) {
    let node = control.node();
    let ops = control.ops().content();

    if let Some(placeholder) = self.placeholder.clone() {
      placeholder.bind(node, move |node, hint| {
        contents::PLACEHOLDER.write_with(ops, node, hint);
      });
    }

    if let Some(secure) = self.secure.clone().filter(|_| self.lines == 1) {
      secure.bind(node, move |node, secure| {
        contents::SECURE.write_with(ops, node, secure);
      });
    }

    if let Some(keyboard) = self.keyboard.clone() {
      keyboard.bind(node, move |node, keyboard| {
        contents::KEYBOARD.write_with(ops, node, keyboard);
      });
    }

    let (value, selection) = (self.value.clone(), self.selection.clone());
    let (asks_value, asks_selection) = (self.value.clone(), self.selection.clone());

    control.register(EditController::new(
      move || EditTruth::new(value.read().map(Rc::from), selection.read()),
      move |asked| {
        if let Some(text) = &asked.text {
          asks_value.ask(text.to_string());
        }

        if let Some(selection) = asked.selection {
          asks_selection.ask(selection);
        }
      },
    ));
  }
}
