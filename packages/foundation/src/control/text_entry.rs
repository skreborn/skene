//! A text field's and a text editor's behavior.

use skene_reactive::{Effect, Signal, untrack};

use super::{Behavior, ControlHandle, Controlled};
use crate::{Edit, EventKind, NativeElement, Realization, Role, TextSelection, Verdict};

/// A text entry: a single-line field, or an editor showing several lines at once.
///
/// Editing keys are the platform's; the behavior only reports each change. A drawn
/// entry reports a reader's edit as one [`Edit`], its text and selection together, with
/// whether its toolkit shows it already, and applies the [`Verdict`] it is answered with,
/// which is the only write-back: an edit the truth holds already is accepted unasked;
/// otherwise the value is asked for first, then the selection, and an edit the truth
/// then holds, or any edit while nothing is derived, is accepted, while any other is
/// answered with the truth.
///
/// Controlled: the text shown is what [`derive_value`](Self::derive_value) reads, and an
/// edit only calls [`handle_change`](Self::handle_change) with the text asked for;
/// [`bind_value`](Self::bind_value) wires both to one signal. The selection is
/// controlled the same way: [`derive_selection`](Self::derive_selection) places it,
/// [`handle_selection_change`](Self::handle_selection_change) hears every move of the
/// caret, and [`bind_selection`](Self::bind_selection) wires both to one signal, so
/// an app reads and writes the selection beside the value.
#[must_use]
pub struct TextEntry {
  value: Controlled<String>,
  selection: Controlled<TextSelection>,
  placeholder: Option<String>,
  lines: u16,
}

impl std::fmt::Debug for TextEntry {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("TextEntry").finish_non_exhaustive()
  }
}

impl TextEntry {
  /// The events the entry acts on: a change and an edit, and a move of the selection
  /// where something hears it.
  const fn events(&self) -> &'static [EventKind] {
    if self.selection.is_handled() {
      &[
        EventKind::Change,
        EventKind::SelectionChange,
        EventKind::Edit,
      ]
    }
    else {
      &[EventKind::Change, EventKind::Edit]
    }
  }

  /// An empty single-line field.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      value: Controlled::new(),
      selection: Controlled::new(),
      placeholder: None,
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
    self.placeholder = Some(placeholder.into());

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

  /// The text and selection the entry holds, read untracked; `None` while no text is
  /// derived.
  fn held(&self) -> Option<Edit> {
    untrack(|| {
      let text = self.value.read()?;
      let edit = Edit::new(text);

      Some(match self.selection.read() {
        Some(selection) => edit.with_selection(selection),
        None => edit,
      })
    })
  }
}

impl Default for TextEntry {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Behavior<P> for TextEntry {
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
    self.events()
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

    if let Some(value) = self.value.reader() {
      Effect::render(move |_: Option<()>| {
        let value = value();

        P::set_value(node, &value);
      });
    }

    if let Some(selection) = self.selection.reader() {
      Effect::render(move |_: Option<()>| {
        let selection = selection();

        P::set_selection(node, selection);
      });
    }
  }

  fn on_change(&self, value: &str, _control: &ControlHandle<P>) {
    self.value.ask(value.to_owned());
  }

  fn on_selection_change(&self, selection: TextSelection, _control: &ControlHandle<P>) {
    self.selection.ask(selection);
  }

  fn on_edit(&self, edit: &Edit, _applied: bool, _control: &ControlHandle<P>) -> Verdict {
    if self.held().is_some_and(|truth| truth.holds(edit)) {
      return Verdict::Accept;
    }

    self.value.ask(edit.text.to_string());

    if let Some(selection) = edit.selection {
      self.selection.ask(selection);
    }

    self
      .held()
      .filter(|truth| !truth.holds(edit))
      .map_or(Verdict::Accept, Verdict::Replace)
  }

  #[inline(always)]
  fn held_edit(&self) -> Option<Edit> {
    self.held()
  }

  fn reconcile(&self, control: &ControlHandle<P>) {
    if let Some(value) = untrack(|| self.value.read()) {
      P::set_value(control.node(), &value);
    }

    if let Some(selection) = untrack(|| self.selection.read()) {
      P::set_selection(control.node(), selection);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::TextEntry;
  use crate::{EventKind, TextSelection};

  /// An entry listens to its changes and edits, and to the selection's moves only where
  /// something hears them.
  #[test]
  fn an_entry_listens_to_the_selection_only_where_it_is_heard() {
    assert_eq!(
      TextEntry::new().events(),
      [EventKind::Change, EventKind::Edit]
    );
    assert_eq!(
      TextEntry::new()
        .handle_selection_change(|_: TextSelection| {})
        .events(),
      [
        EventKind::Change,
        EventKind::SelectionChange,
        EventKind::Edit,
      ]
    );
  }
}
