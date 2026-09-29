//! A text field's and a text editor's behavior.

use std::rc::Rc;

use skene_core::{
  ChangeHandler,
  ControlKind,
  EventKind,
  Platform,
  Role,
  SelectionHandler,
  TextSelection,
};
use skene_reactive::{Effect, Get, Set, Signal, untrack};

use super::{Behavior, ControlHandle};
use crate::realize::with_sink;

/// A text entry: a single-line field, or an editor showing several lines at once.
///
/// Editing keys are the platform's; the behavior only reports each change. Controlled:
/// the text shown is what [`derive_value`](Self::derive_value) reads, and an edit
/// only calls [`handle_change`](Self::handle_change) with the text asked for;
/// [`bind_value`](Self::bind_value) wires both to one signal. The selection is
/// controlled the same way: [`derive_selection`](Self::derive_selection) places it,
/// [`handle_selection_change`](Self::handle_selection_change) hears every move of the
/// caret, and [`bind_selection`](Self::bind_selection) wires both to one signal, so
/// an app reads and writes the selection beside the value.
#[must_use]
pub struct TextEntry {
  value: Option<Rc<dyn Fn() -> String>>,
  selection: Option<Rc<dyn Fn() -> TextSelection>>,
  placeholder: Option<String>,
  lines: u16,
  on_change: Option<Rc<ChangeHandler>>,
  on_selection_change: Option<Rc<SelectionHandler>>,
}

impl std::fmt::Debug for TextEntry {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("TextEntry").finish_non_exhaustive()
  }
}

impl TextEntry {
  /// An empty single-line field.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      value: None,
      selection: None,
      placeholder: None,
      lines: 1,
      on_change: None,
      on_selection_change: None,
    }
  }

  /// Sets the lines shown at once; more than one makes the entry an editor.
  #[inline(always)]
  pub const fn set_lines(mut self, lines: u16) -> Self {
    self.lines = if lines == 0 { 1 } else { lines };

    self
  }

  /// Sets the hint shown while the value is empty.
  #[inline(always)]
  pub fn set_placeholder(mut self, placeholder: impl Into<String>) -> Self {
    self.placeholder = Some(placeholder.into());

    self
  }

  /// Sets the text.
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    let value = value.into();

    self.value = Some(Rc::new(move || value.clone()));

    self
  }

  /// Derives the text from the signals `value` reads.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.value = Some(Rc::new(value));

    self
  }

  /// Runs `action` with the text an edit asks for.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.on_change = Some(Rc::new(action));

    self
  }

  /// Binds the text to `signal` in both directions.
  #[inline(always)]
  pub fn bind_value(self, signal: Signal<String>) -> Self {
    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value.to_owned()))
  }

  /// Sets the selection, in grapheme clusters of the text.
  #[inline(always)]
  pub fn set_selection(mut self, selection: TextSelection) -> Self {
    self.selection = Some(Rc::new(move || selection));

    self
  }

  /// Derives the selection from the signals `selection` reads.
  #[inline(always)]
  pub fn derive_selection(mut self, selection: impl Fn() -> TextSelection + 'static) -> Self {
    self.selection = Some(Rc::new(selection));

    self
  }

  /// Runs `action` with the selection after every move of the caret or its anchor.
  #[inline(always)]
  pub fn handle_selection_change(mut self, action: impl Fn(TextSelection) + 'static) -> Self {
    self.on_selection_change = Some(Rc::new(action));

    self
  }

  /// Binds the selection to `signal` in both directions.
  #[inline(always)]
  pub fn bind_selection(self, signal: Signal<TextSelection>) -> Self {
    self
      .derive_selection(move || signal.get())
      .handle_selection_change(move |selection| signal.set(selection))
  }
}

impl Default for TextEntry {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Behavior<P> for TextEntry {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    if self.lines > 1 {
      ControlKind::TextEditor
    }
    else {
      ControlKind::TextField
    }
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    None
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    if self.on_selection_change.is_some() {
      &[EventKind::Change, EventKind::SelectionChange]
    }
    else {
      &[EventKind::Change]
    }
  }

  #[inline(always)]
  fn placeholder(&self) -> Option<String> {
    self.placeholder.clone()
  }

  #[inline(always)]
  fn lines(&self) -> u16 {
    self.lines
  }

  fn attach(&self, control: &ControlHandle<P>) {
    let node = control.node();

    if let Some(value) = self.value.clone() {
      Effect::render(move |_: Option<()>| {
        let value = value();

        with_sink::<P, _>(node, |sink| P::set_value(sink, &value));
      });
    }

    if let Some(selection) = self.selection.clone() {
      Effect::render(move |_: Option<()>| {
        let selection = selection();

        with_sink::<P, _>(node, |sink| P::set_selection(sink, selection));
      });
    }
  }

  fn on_change(&self, value: &str, _control: &ControlHandle<P>) {
    if let Some(action) = &self.on_change {
      action(value);
    }
  }

  fn on_selection_change(&self, selection: TextSelection, _control: &ControlHandle<P>) {
    if let Some(action) = &self.on_selection_change {
      action(selection);
    }
  }

  fn reconcile(&self, control: &ControlHandle<P>) {
    if let Some(value) = &self.value {
      let value = untrack(|| value());

      control.with_sink(|sink| P::set_value(sink, &value));
    }

    if let Some(selection) = &self.selection {
      let selection = untrack(|| selection());

      control.with_sink(|sink| P::set_selection(sink, selection));
    }
  }
}
