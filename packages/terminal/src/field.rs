//! The terminal's text entries as clients of the shared editing core: each operation on an
//! entry's editor runs with neither the records nor the scene borrowed, and what it changed is
//! settled after; the keys bound to the core's commands, the clipboard the launch offers, the
//! pointer, and the window over the value.

use std::rc::Rc;

use skene_foundation::{
  ClipboardAction,
  Edit,
  Key,
  KeyEvent,
  Modifiers,
  NamedKey,
  Scene,
  TextSelection,
  text::{
    Direction,
    TextBoundary,
    editing::{Binding, Command, Movement},
  },
};

use crate::{
  Terminal,
  layout::padding_cells,
  records::{AUTOSCROLL_INTERVAL, Drag, EntryEditor, Field, Handle, Records},
  with_records,
};

/// A window of `span` over `extent`, starting at `window` and settled around `at`: held
/// where `at` stays in view, moved the least that brings it in, and never past the extent's
/// end.
pub(crate) const fn settled_window(window: usize, at: usize, extent: usize, span: usize) -> usize {
  let last = extent.saturating_sub(span);
  let window = if window > last { last } else { window };

  if at < window {
    at
  }
  else if at + 1 > window + span {
    at + 1 - span
  }
  else {
    window
  }
}

/// The cells an entry shows its value in: inside its border and padding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Area {
  pub(crate) columns: usize,
  pub(crate) rows: usize,
}

impl Records {
  /// The cells an entry shows its value in: inside its border and padding.
  pub(crate) fn field_area(&self, handle: Handle) -> Area {
    let (width, height) = self.node_size(handle);
    let border = self.node_border(handle);
    let padding = self
      .record(handle)
      .and_then(|record| record.kind.box_style())
      .map(padding_cells)
      .unwrap_or_default();

    Area {
      columns: usize::try_from(
        width - border.0 - border.2 - i64::from(padding.start) - i64::from(padding.end),
      )
      .unwrap_or(0),
      rows: usize::try_from(
        height - border.1 - border.3 - i64::from(padding.top) - i64::from(padding.bottom),
      )
      .unwrap_or(0),
    }
  }

  /// Whether the entry at `handle` takes edits to its value: it is neither disabled nor
  /// read-only.
  #[expect(
    clippy::unused_self,
    reason = "every reader holds the records, and an entry's editability is asked of it as of the rest"
  )]
  pub(crate) fn is_editable(&self, handle: Handle) -> bool {
    Scene::<Terminal>::read(|scene| scene.is_editable(handle))
  }

  /// The inset from an entry's edge to its value: border plus padding.
  fn field_inset(&self, handle: Handle) -> (i64, i64) {
    let border = self.node_border(handle);
    let padding = self
      .record(handle)
      .and_then(|record| record.kind.box_style())
      .map(padding_cells)
      .unwrap_or_default();

    (
      border.0 + i64::from(padding.start),
      border.1 + i64::from(padding.top),
    )
  }

  /// An entry's local cell as a column and row of its value: past the inset, clamped
  /// at its start.
  fn entry_cell(&self, handle: Handle, local_x: i64, local_y: i64) -> (usize, usize) {
    let (inset_x, inset_y) = self.field_inset(handle);

    (
      usize::try_from(local_x - inset_x).unwrap_or(0),
      usize::try_from(local_y - inset_y).unwrap_or(0),
    )
  }

  /// The layout position an entry's local cell stands for in its value as it shows:
  /// mapped through the inset and the window, clamped into the visible area.
  fn entry_point(&self, handle: Handle, local_x: i64, local_y: i64) -> Option<(f32, f32)> {
    let area = self.field_area(handle);
    let (column, row) = self.entry_cell(handle, local_x, local_y);
    let field = self.field(handle)?;
    let entry = self.entry(handle)?;

    Some(field.point_at(field.visible_window(&entry, area), area, column, row))
  }

  /// Whether the node is an editor, whose window scrolls down its wrapped rows.
  pub(crate) fn is_editor(&self, handle: Handle) -> bool {
    self.field(handle).is_some_and(Field::is_editor)
  }

  /// An editor's window as it shows in its laid-out area; `None` for any other node.
  pub(crate) fn editor_window(&self, handle: Handle) -> Option<u32> {
    let field = self.field(handle).filter(|field| field.is_editor())?;
    let entry = self.entry(handle)?;

    u32::try_from(field.visible_window(&entry, self.field_area(handle))).ok()
  }

  /// How far an editor's window can scroll in its laid-out area; `None` for any other
  /// node.
  pub(crate) fn editor_window_limit(&self, handle: Handle) -> Option<u32> {
    let field = self.field(handle).filter(|field| field.is_editor())?;
    let entry = self.entry(handle)?;

    u32::try_from(field.scroll_limit(&entry, self.field_area(handle))).ok()
  }

  /// Moves an editor's window to `window`, clamped, in its laid-out area.
  pub(crate) fn set_editor_window(&mut self, handle: Handle, window: u32) {
    let area = self.field_area(handle);
    let Some(entry) = self.entry(handle)
    else {
      return;
    };

    if let Some(field) = self.field_mut(handle).filter(|field| field.is_editor()) {
      let window = usize::try_from(window).unwrap_or(usize::MAX);

      field.window = window.min(field.scroll_limit(&entry, area));
      field.window_area = Some(area);
    }
  }

  /// Scrolls the single-line field at `handle` by `delta` cells in its laid-out area,
  /// damaging it: `None` when the node is no field or `delta` is zero, else whether it
  /// moved.
  pub(crate) fn scroll_field_by(&mut self, handle: Handle, delta: i64) -> Option<bool> {
    if delta == 0 {
      return None;
    }

    let area = self.field_area(handle);
    let entry = self.entry(handle)?;
    let field = self.field_mut(handle).filter(|field| !field.is_editor())?;
    let moved = field.scroll_window(&entry, area, delta);

    if moved {
      self.damage(handle);
    }

    Some(moved)
  }

  /// Settles an entry's window around its caret, in its laid-out area.
  pub(crate) fn settle_entry(&mut self, handle: Handle) {
    let area = self.field_area(handle);
    let Some(entry) = self.entry(handle)
    else {
      return;
    };

    if let Some(field) = self.field_mut(handle) {
      field.settle(&entry, area);
    }
  }

  /// Lays every editor's value out at the width of its laid-out area; an editor an
  /// operation holds is left to it.
  pub(crate) fn fit_entries(&self) {
    for entry in &self.entries {
      let (Some(field), Some(state)) = (self.field(*entry), self.entry(*entry))
      else {
        continue;
      };

      if let Ok(mut editor) = state.editor().try_borrow_mut() {
        Field::fit(&mut editor, field.lines, self.field_area(*entry));
      }
    }
  }

  /// Settles what an entry's editor changed: a changed value restyles and lays the entry
  /// out again, and a changed value or selection settles its window and repaints it.
  pub(crate) fn settle_changes(&mut self, handle: Handle) {
    let area = self.field_area(handle);
    let Some(entry) = self.entry(handle)
    else {
      return;
    };
    let Some(field) = self.field_mut(handle)
    else {
      return;
    };
    let changed = entry.take_changed();

    if !(changed.text || changed.selection) {
      return;
    }

    field.settle(&entry, area);
    self.damage(handle);

    if changed.text {
      self.queue_restyle(handle);
      self.layout_dirty = true;
    }
  }

  /// Runs `operation` on the editor of the entry at `handle` now, where no operation holds
  /// it, and settles what it changed; else once the operation holding it ends.
  pub(crate) fn with_entry(
    &mut self,
    handle: Handle,
    operation: impl FnOnce(&mut EntryEditor) + 'static,
  ) {
    let Some(entry) = self.entry(handle)
    else {
      return;
    };
    let Ok(mut editor) = entry.editor().try_borrow_mut()
    else {
      entry.defer(operation);

      return;
    };

    operation(&mut editor);
    drop(editor);
    self.settle_changes(handle);
  }

  /// Collapses the selection inside `entry` at its caret.
  pub(crate) fn collapse_entry_selection(&mut self, entry: Handle) {
    self.with_entry(entry, |editor| {
      let caret = editor.selection().end;

      let _ = editor.set_selection(TextSelection::collapsed(caret));
    });
  }

  /// Takes focus into `entry`: a field selects its whole value and an editor puts its
  /// caret at the end, as a browser's `<input>` and `<textarea>` do.
  pub(crate) fn focus_entry(&mut self, entry: Handle) {
    let Some(multiline) = self.field(entry).map(Field::is_editor)
    else {
      return;
    };

    self.with_entry(entry, move |editor| {
      let command = if multiline {
        Command::Move {
          movement: Movement::Logical(TextBoundary::Text, Direction::Forward),
          extend: false,
        }
      }
      else {
        Command::SelectAll
      };

      let _ = editor.perform(command);
    });
  }

  /// Places `entry`'s selection from outside its editing, each index clamped into the
  /// value; it is not reported back.
  pub(crate) fn place_entry_selection(&mut self, entry: Handle, selection: TextSelection) {
    let Some(reported) = self.entry(entry).map(|state| state.reported())
    else {
      return;
    };

    self.with_entry(entry, move |editor| {
      let _ = editor.set_selection(selection);

      reported.set(editor.selection());
    });
  }

  /// Shows `edit` in `entry` from outside its editing: its text written as
  /// [`write_entry`](Self::write_entry) writes it, then its selection placed where it gives
  /// one.
  pub(crate) fn show_entry(&mut self, entry: Handle, edit: &Edit) {
    self.write_entry(entry, &edit.text);

    if let Some(selection) = edit.selection {
      self.place_entry_selection(entry, selection);
    }
  }

  /// Writes `value` into `entry` from outside its editing, as the entry holds it: one undo
  /// step but for an entry's first value, the caret mapped through the change and the
  /// selection collapsed at it; nothing happens where it is shown already.
  pub(crate) fn write_entry(&mut self, entry: Handle, value: &str) {
    let value = value.to_owned();

    self.with_entry(entry, move |editor| {
      if editor.write(&Edit::new(value)) {
        let caret = editor.selection().end;

        let _ = editor.set_selection(TextSelection::collapsed(caret));
      }
    });
  }

  /// Makes `entry` secure where `secure`: its value drawn masked, a mark a cluster, its
  /// selection kept from the clipboard and its history dropped, as the editing core keeps
  /// a secure text.
  pub(crate) fn set_entry_secure(&mut self, entry: Handle, secure: bool) {
    let Some(mask) = self.entry(entry).map(|state| Rc::clone(state.mask()))
    else {
      return;
    };

    self.with_entry(entry, move |editor| {
      mask.set_masked(secure);
      editor.set_secure(secure);
      editor.relayout();
    });

    self.queue_restyle(entry);
    self.layout_dirty = true;
    self.damage(entry);
  }

  /// Whether `entry` is a secure text entry.
  pub(crate) fn is_secure_entry(&self, entry: Handle) -> bool {
    self.entry(entry).is_some_and(|state| state.is_secure())
  }

  /// Whether the caret position at an entry's local cell lies inside its selection.
  pub(crate) fn entry_selection_contains(
    &self,
    handle: Handle,
    local_x: i64,
    local_y: i64,
  ) -> bool {
    let Some((x, y)) = self.entry_point(handle, local_x, local_y)
    else {
      return false;
    };

    self.entry(handle).is_some_and(|entry| {
      entry.editor().try_borrow().is_ok_and(|editor| {
        let selected = editor.selected();
        let at = editor.byte_at(x, y);

        selected.contains(&at)
      })
    })
  }

  /// The caret drag's overshoot: how far past the entry's visible value the pointer is
  /// held, in cells of a field and rows of an editor, negative before its start; `None`
  /// while the pointer is inside it or the window can scroll no further that way.
  fn caret_drag_overshoot(&self) -> Option<(Handle, i64)> {
    let Some(Drag::Caret(handle)) = self.drag
    else {
      return None;
    };
    let field = self.field(handle)?;
    let entry = self.entry(handle)?;
    let area = self.field_area(handle);
    let (origin_x, origin_y) = self.screen_origin(handle);
    let (inset_x, inset_y) = self.field_inset(handle);
    let (x, y) = self.last_pointer;
    let (along, span) = if field.is_editor() {
      (i64::from(y) - origin_y - inset_y, area.rows.max(1))
    }
    else {
      (i64::from(x) - origin_x - inset_x, area.columns.max(1))
    };
    let span = i64::try_from(span).unwrap_or(i64::MAX);
    let overshoot = if along < 0 {
      along
    }
    else if along >= span {
      along - span + 1
    }
    else {
      return None;
    };
    let window = field.visible_window(&entry, area);
    let room = if overshoot < 0 {
      window > 0
    }
    else {
      window < field.scroll_limit(&entry, area)
    };

    room.then_some((handle, overshoot))
  }

  /// Whether a caret drag is held past its entry's edge with room to scroll.
  pub(crate) fn autoscrolls(&self) -> bool {
    self.caret_drag_overshoot().is_some()
  }

  /// Steps the caret drag's autoscroll on the frame clock: once per interval while the
  /// pointer is held past the entry's visible value, the window scrolls toward it by the
  /// overshoot; the entry and the point its caret follows to, at the edge, are returned.
  fn autoscroll_step(&mut self) -> Option<(Handle, (f32, f32))> {
    let Some((handle, overshoot)) = self.caret_drag_overshoot()
    else {
      self.autoscroll_at = None;

      return None;
    };
    let now = self.now();
    let interval = AUTOSCROLL_INTERVAL.as_secs_f64();
    let Some(at) = self.autoscroll_at
    else {
      self.autoscroll_at = Some(now);

      return None;
    };
    let steps = ((now - at) / interval + 1e-9).floor();

    if steps < 1. {
      return None;
    }

    self.autoscroll_at = Some(at + steps * interval);

    let area = self.field_area(handle);
    let (origin_x, origin_y) = self.screen_origin(handle);
    let (x, y) = self.last_pointer;
    let (column, row) = self.entry_cell(handle, i64::from(x) - origin_x, i64::from(y) - origin_y);
    let entry = self.entry(handle)?;
    let field = self.field_mut(handle)?;
    #[expect(clippy::cast_possible_truncation, reason = "at least one whole step")]
    let delta = overshoot.saturating_mul(steps as i64);

    field.scroll_window(&entry, area, delta);

    let point = field.point_at(field.window, area, column, row);

    self.damage(handle);

    Some((handle, point))
  }
}

/// Runs `operation` on the editor of the entry at `entry` with no records borrowed: the
/// editor laid out at the entry's width (the scene laid out first where `lay_out`) and
/// taking edits only where the entry is editable; what it changed, and what waited for
/// it, settle after, and moved selections are reported. `None` where the entry is gone or
/// an operation holds its editor already.
fn operate<R>(
  entry: Handle,
  lay_out: bool,
  operation: impl FnOnce(&mut EntryEditor) -> R,
) -> Option<R> {
  if lay_out {
    skene_foundation::lay_out::<Terminal>();
  }

  let (state, lines, area, editable) = with_records(|records| {
    let field = records.field(entry)?;

    Some((
      records.entry(entry)?,
      field.lines,
      records.field_area(entry),
      records.is_editable(entry),
    ))
  })?;
  let result = {
    let mut editor = state.editor().try_borrow_mut().ok()?;

    Field::fit(&mut editor, lines, area);
    editor.set_read_only(!editable);
    operation(&mut editor)
  };

  finish(entry);

  Some(result)
}

/// Settles an operation on the editor of `entry` once it has ended: the operations that
/// waited for it run, what they and it changed settles, and moved selections are reported.
fn finish(entry: Handle) {
  if let Some(state) = with_records(|records| records.entry(entry)) {
    while state.has_deferred() {
      let Ok(mut editor) = state.editor().try_borrow_mut()
      else {
        break;
      };

      for operation in state.take_deferred() {
        operation(&mut editor);
      }
    }
  }

  with_records(|records| records.settle_changes(entry));
  report_selections();
}

/// Reports every entry's selection that moved since it was last reported, at the
/// entry's node as a change of what it shows that its toolkit applied, and shows the
/// truth the verdict answers with.
pub(crate) fn report_selections() {
  let entries = with_records(|records| records.entry_states());

  for entry in entries {
    if let Some(truth) = entry.report_selection() {
      with_records(|records| records.show_entry(entry.node(), &truth));
    }
  }
}

/// `event` as the key the terminal's legacy encodings stand for: `Ctrl+W` for
/// `Ctrl+Backspace`, which some keyboard encodings cannot express and readline binds
/// alike, and `Ctrl+H` (`0x08`, the legacy byte of `Ctrl+Backspace`) for `Backspace`,
/// deleting one character as both readings allow.
fn legacy(event: &KeyEvent) -> KeyEvent {
  let modifiers = event.modifiers;
  let control = modifiers.ctrl && !modifiers.alt && !modifiers.meta && !modifiers.shift;
  let word = match (&event.key, event.shortcut_key().as_ref()) {
    (Key::Character(_), "w") if control => true,
    (Key::Character(_), "h") if control => false,
    _ => return event.clone(),
  };

  KeyEvent {
    key: Key::Named(NamedKey::Backspace),
    modifiers: Modifiers {
      ctrl: word,
      ..modifiers
    },
    ..event.clone()
  }
}

/// The terminal's response to a key the handlers let stand, through the editing core's key
/// table: in the focused entry, the binding's command; the clipboard's bindings only where
/// the launch offers their operation, `Shift+Delete` deleting forward where it offers no
/// `Cut`; and outside an entry, the copy of the document's selection, or the cut of the
/// entry a popup was opened from.
pub(crate) fn edit_focused(event: &KeyEvent) {
  let (entry, multiline) = with_records(|records| {
    let entry = records.focused_entry();

    (entry, entry.is_some_and(|entry| records.is_editor(entry)))
  });
  let event = legacy(event);
  let offers = |action| with_records(|records| records.offers(action));

  match Binding::for_key(&event, multiline) {
    Some(Binding::Command(Command::Copy)) if offers(ClipboardAction::Copy) => {
      with_records(Records::copy_selection);
    }
    Some(Binding::Command(Command::Cut)) if offers(ClipboardAction::Cut) => cut_focused(),
    Some(Binding::Command(Command::Cut)) if matches!(event.key, Key::Named(NamedKey::Delete)) => {
      if let Some(entry) = entry {
        operate(entry, true, |editor| {
          editor.perform(Command::Delete(TextBoundary::Grapheme, Direction::Forward))
        });
      }
    }
    Some(Binding::Paste) if offers(ClipboardAction::Paste) => {
      with_records(|records| {
        if records.focused_entry().is_some() {
          records.request_paste();
        }
      });
    }
    Some(Binding::Command(Command::Copy | Command::Cut | Command::Paste(_)) | Binding::Paste)
    | None => {}
    Some(Binding::Command(command)) => {
      if let Some(entry) = entry {
        operate(entry, true, |editor| editor.perform(command));
      }
    }
  }
}

/// Places the caret of `entry` at its local cell, the `clicks`th press in a row (a double
/// press takes the word, a triple the line of an editor and the value of a field),
/// extending the selection where `extend`, and starts the drag that follows the press.
pub(crate) fn press_entry(entry: Handle, local_x: i64, local_y: i64, clicks: u8, extend: bool) {
  let Some((x, y)) = with_records(|records| records.entry_point(entry, local_x, local_y))
  else {
    return;
  };

  if operate(entry, false, |editor| editor.press(x, y, clicks, extend)).is_some() {
    with_records(|records| records.drag = Some(Drag::Caret(entry)));
  }
}

/// Follows a caret drag to the screen cell (`x`, `y`): the selection extends to the
/// position under the pointer, clamped to the visible value's edge, by the press's unit;
/// past the edge, the autoscroll carries it on from there.
pub(crate) fn drag_entry(entry: Handle, x: u16, y: u16) {
  let point = with_records(|records| {
    let (origin_x, origin_y) = records.screen_origin(entry);

    records.entry_point(entry, i64::from(x) - origin_x, i64::from(y) - origin_y)
  });

  if let Some((x, y)) = point {
    operate(entry, false, |editor| editor.drag(x, y));
  }
}

/// Steps the caret drag's autoscroll on the frame clock: the window scrolls toward the
/// pointer held past the entry's edge, and the selection follows to the edge.
pub(crate) fn autoscroll() {
  if let Some((entry, (x, y))) = with_records(Records::autoscroll_step) {
    operate(entry, false, |editor| editor.drag(x, y));
  }
}

/// Inserts `text` into the focused entry in place of its selection.
pub(crate) fn paste_focused(text: &str) {
  let target = with_records(|records| records.focused());

  paste_into(target, text);
}

/// Inserts `text`, the terminal's answer to the last clipboard read, into the entry
/// that asked for it; nothing happens when no entry asked or it is gone.
pub(crate) fn paste_requested(text: &str) {
  let target = with_records(|records| records.paste_target.take());

  paste_into(target, text);
}

/// Inserts `text` into the entry at `target`, in place of its selection, where the
/// entry is editable.
fn paste_into(target: Option<Handle>, text: &str) {
  let Some(target) = target.filter(|target| with_records(|records| records.is_field(*target)))
  else {
    return;
  };

  operate(target, false, |editor| {
    editor.perform(Command::Paste(text.to_owned()))
  });
}

/// Stages the clipboard entry's selection for the clipboard and removes it; nothing
/// happens when `Cut` is not offered or the entry is not editable.
pub(crate) fn cut_focused() {
  let Some(entry) = with_records(|records| {
    if !records.offers(ClipboardAction::Cut) {
      return None;
    }

    records
      .clipboard_entry()
      .filter(|entry| records.is_editable(*entry))
  })
  else {
    return;
  };
  let cut = operate(entry, false, |editor| editor.perform(Command::Cut));

  if let Some(text) = cut.and_then(|outcome| outcome.clipboard) {
    with_records(|records| records.pending_copy = Some(text));
  }
}
