//! In-app text selection: source-anchored ranges over selectable runs, and the
//! clipboard actions over them.
//!
//! A selection's ends live in source space—a run and a character offset into its
//! source text—so it survives reflow and copies as the text appears in the source, soft
//! wraps excluded. A press anchors it, a drag extends it across runs in document order,
//! a double click takes the word, a triple click the run. A text entry's selection lives
//! in its [`Field`](crate::store::Field) instead, over its value, and holds while a
//! popup takes focus from the entry, so the context menu acts on it. `Copy` stages the
//! text for the terminal loop's OSC 52 write, `Cut` removes it as well, and `Paste`
//! stages an OSC 52 read whose reply lands in the entry that asked. The launch's
//! [`Clipboard`](crate::Clipboard) says which actions exist; within it every action is
//! offered on every terminal, since OSC 52 has no capability query, and a terminal
//! lacking it ignores the write and answers no read.

use std::time::{Duration, Instant};

use skene_core::ClipboardAction;

use crate::{
  field::word_bounds,
  layout::{is_paragraph, paragraph_runs, paragraph_text},
  paint::{line_at_row, line_holding, paragraph_scales, run_scale, wrap_sized},
  store::{Drag, Handle, Kind, Store},
  text::{self, Scales},
};

/// One end of a selection: a run and a cluster offset into its source text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionPoint {
  pub(crate) node: Handle,
  pub(crate) offset: usize,
}

/// A selection: the pressed end and the moving end; document order between the two is
/// resolved at use.
#[derive(Clone, Copy)]
pub(crate) struct Selection {
  pub(crate) anchor: SelectionPoint,
  pub(crate) focus: SelectionPoint,
}

/// The window between two presses of one multi-click sequence.
const MULTI_CLICK: Duration = Duration::from_millis(500);

impl Store {
  /// Counts a press at (`x`, `y`) into its multi-click sequence: 1 for a fresh press,
  /// 2 for a double click, 3 for a triple; a fourth begins a new sequence.
  pub(crate) fn register_press(&mut self, x: u16, y: u16) -> u8 {
    let now = Instant::now();
    let count = match self.last_press {
      Some((at, press_x, press_y, count))
        if press_x == x && press_y == y && now.duration_since(at) <= MULTI_CLICK && count < 3 =>
      {
        count + 1
      }
      _ => 1,
    };

    self.last_press = Some((now, x, y, count));

    count
  }

  /// The node's source text, when it is a selectable, non-empty run or paragraph
  /// outside every control and `Copy` is offered: a control's label is chrome, whatever
  /// it opts into, and without `Copy` nothing in the document is selectable.
  pub(crate) fn selectable_text(&self, handle: Handle) -> Option<String> {
    let record = self.record(handle)?;

    if !self.offers(ClipboardAction::Copy) || self.enclosing_control(handle).is_some() {
      return None;
    }

    match &record.kind {
      Kind::Run { props, .. } if props.selectable && !record.text.is_empty() => {
        Some(record.text.clone())
      }
      Kind::Box(_) if is_paragraph(self, handle) => {
        let selectable = paragraph_runs(self, handle).iter().any(|run| {
          matches!(
            self.record(*run).map(|record| &record.kind),
            Some(Kind::Run { props, .. }) if props.selectable
          )
        });
        let text = paragraph_text(self, handle);

        (selectable && !text.is_empty()).then_some(text)
      }
      _ => None,
    }
  }

  /// The scales of the node's text: its runs' for a paragraph, its own for a run.
  fn text_scales(&self, handle: Handle) -> Scales {
    if is_paragraph(self, handle) {
      paragraph_scales(self, handle)
    }
    else {
      Scales::uniform(run_scale(self, handle))
    }
  }

  /// The character offset within the node's source at the node-local cell, clamped
  /// into the nearest wrapped line and its ends, against the last layout.
  fn run_offset(&self, handle: Handle, local_x: i64, local_y: i64) -> Option<usize> {
    let text = self.selectable_text(handle)?;
    let (width, _) = self.node_size(handle);
    let scales = self.text_scales(handle);
    let lines = wrap_sized(&text, usize::try_from(width).unwrap_or(1), &scales);
    let row = usize::try_from(local_y.max(0)).unwrap_or(0);
    let line = &lines[line_at_row(&lines, row)];
    let column = usize::try_from(local_x.max(0)).unwrap_or(0);

    Some(line.start + line.index_at_column(column, &scales))
  }

  /// The source position at the cell (`x`, `y`), when it lies on a selectable run.
  pub(crate) fn text_position_at(&self, x: u16, y: u16) -> Option<SelectionPoint> {
    let path = self.hit_test(x, y);
    let (node, (origin_x, origin_y)) = *path.last()?;
    let offset = self.run_offset(node, i64::from(x) - origin_x, i64::from(y) - origin_y)?;

    Some(SelectionPoint { node, offset })
  }

  /// The linked run at the cell (`x`, `y`) and its destination, when the cell carries
  /// the run's character—exact hits only, never clamped into a neighboring line.
  pub(crate) fn link_at(&self, x: u16, y: u16) -> Option<(Handle, String)> {
    let path = self.hit_test(x, y);
    let (node, (origin_x, origin_y)) = *path.last()?;
    let (width, _) = self.node_size(node);
    let record = self.record(node)?;
    let row = usize::try_from(i64::from(y) - origin_y).ok()?;
    let column = usize::try_from(i64::from(x) - origin_x).ok()?;
    let scales = self.text_scales(node);

    if let Kind::Run { props, .. } = &record.kind {
      let lines = wrap_sized(&record.text, usize::try_from(width).unwrap_or(1), &scales);
      let line = &lines[line_holding(&lines, row)?];

      return line
        .cluster_at(column, &scales)
        .and_then(|_| props.link.clone().map(|url| (node, url)));
    }

    if !is_paragraph(self, node) {
      return None;
    }

    let text = paragraph_text(self, node);
    let lines = wrap_sized(&text, usize::try_from(width).unwrap_or(1), &scales);
    let line = &lines[line_holding(&lines, row)?];
    let offset = line.start + line.cluster_at(column, &scales)?;
    let mut cursor = 0;

    paragraph_runs(self, node).into_iter().find_map(|run| {
      let record = self.record(run)?;
      let end = cursor + text::count(&record.text);
      let hit = (cursor..end).contains(&offset);

      cursor = end;

      match &record.kind {
        Kind::Run { props, .. } if hit => props.link.clone().map(|url| (run, url)),
        _ => None,
      }
    })
  }

  /// The source position nearest the cell (`x`, `y`) over every selectable run—by
  /// vertical distance first, then horizontal, earlier runs winning ties. Runs a modal
  /// makes unreachable are ignored.
  fn nearest_text_position(&self, x: u16, y: u16) -> Option<SelectionPoint> {
    let (pointer_x, pointer_y) = (i64::from(x), i64::from(y));
    let mut best: Option<(i64, i64)> = None;
    let mut nearest: Option<(Handle, (i64, i64))> = None;

    for node in self.document_runs() {
      if !self.reachable_under_modal(node) {
        continue;
      }

      let (origin_x, origin_y) = self.screen_origin(node);
      let (width, height) = self.node_size(node);
      let distance = (
        axis_distance(pointer_y, origin_y, height),
        axis_distance(pointer_x, origin_x, width),
      );

      if best.is_none_or(|best| distance < best) {
        best = Some(distance);
        nearest = Some((node, (origin_x, origin_y)));
      }
    }

    let (node, (origin_x, origin_y)) = nearest?;
    let offset = self.run_offset(node, pointer_x - origin_x, pointer_y - origin_y)?;

    Some(SelectionPoint { node, offset })
  }

  /// Every selectable run in document order, the subtrees of closed overlays and the
  /// runs inside a paragraph excluded.
  fn document_runs(&self) -> Vec<Handle> {
    let mut order = Vec::new();
    let mut stack = vec![self.root];

    while let Some(current) = stack.pop() {
      let Some(record) = self.record(current)
      else {
        continue;
      };

      if record.is_overlay() && !record.is_open_overlay() {
        continue;
      }

      if self.selectable_text(current).is_some() {
        order.push(current);

        if is_paragraph(self, current) {
          continue;
        }
      }

      stack.extend(record.children.iter().rev());
    }

    order
  }

  /// The selection's ends in document order, with every selectable run it spans.
  fn selection_span(&self) -> Option<(SelectionPoint, SelectionPoint, Vec<Handle>)> {
    let selection = self.selection?;
    let runs = self.document_runs();
    let anchor_at = runs
      .iter()
      .position(|node| *node == selection.anchor.node)?;
    let focus_at = runs.iter().position(|node| *node == selection.focus.node)?;
    let (start, end) = if (anchor_at, selection.anchor.offset) <= (focus_at, selection.focus.offset)
    {
      (selection.anchor, selection.focus)
    }
    else {
      (selection.focus, selection.anchor)
    };
    let (from, to) = (anchor_at.min(focus_at), anchor_at.max(focus_at));

    Some((start, end, runs[from..=to].to_vec()))
  }

  /// The selected character range within the node's source, when the selection touches
  /// it; a range running past the node's end is `usize::MAX`, clamped by the caller.
  pub(crate) fn selection_range(&self, handle: Handle) -> Option<(usize, usize)> {
    let (start, end, runs) = self.selection_span()?;

    if !runs.contains(&handle) {
      return None;
    }

    let from = if handle == start.node {
      start.offset
    }
    else {
      0
    };
    let to = if handle == end.node {
      end.offset
    }
    else {
      usize::MAX
    };

    (from < to).then_some((from, to))
  }

  /// The focused text entry, when one holds focus.
  pub(crate) fn focused_entry(&self) -> Option<Handle> {
    self.focused.filter(|focused| self.is_field(*focused))
  }

  /// The entry the clipboard acts on: the focused entry, or the entry focus left for
  /// the open popup it now sits in.
  pub(crate) fn clipboard_entry(&self) -> Option<Handle> {
    self.focused_entry().or_else(|| {
      let focused = self.focused?;
      let ancestors = self.ancestors(focused);
      let popup = self
        .open_overlays()
        .into_iter()
        .rev()
        .find(|overlay| ancestors.contains(overlay))?;

      match self.record(popup).map(|record| &record.kind) {
        Some(Kind::Overlay {
          return_focus: Some(entry),
          ..
        }) if self.is_field(*entry) => Some(*entry),
        _ => None,
      }
    })
  }

  /// Whether the entry `entry` holds a selection.
  pub(crate) fn entry_has_selection(&self, entry: Handle) -> bool {
    self
      .record(entry)
      .and_then(|record| record.kind.field()?.selection())
      .is_some()
  }

  /// Whether anything is selected: a range over the document's runs, or one inside the
  /// clipboard's entry.
  pub(crate) fn has_selection(&self) -> bool {
    self.selection.is_some()
      || self
        .clipboard_entry()
        .is_some_and(|entry| self.entry_has_selection(entry))
  }

  /// The selected text: the focused entry's, or the document's as it appears in the
  /// source—each run's slice, runs joined by newlines—or `None` when nothing is
  /// selected.
  pub(crate) fn selected_text(&self) -> Option<String> {
    if let Some(entry) = self.clipboard_entry() {
      return self.record(entry)?.kind.field()?.selected_text();
    }

    let (start, end, runs) = self.selection_span()?;
    let mut parts = Vec::new();

    for node in runs {
      let Some(text) = self.selectable_text(node)
      else {
        continue;
      };
      let count = text::count(&text);
      let from = if node == start.node {
        start.offset.min(count)
      }
      else {
        0
      };
      let to = if node == end.node {
        end.offset.min(count)
      }
      else {
        count
      };

      if from < to {
        parts.push(text::slice(&text, from, to).to_owned());
      }
    }

    (!parts.is_empty()).then(|| parts.join("\n"))
  }

  /// Anchors a fresh selection at `point` and starts its drag.
  pub(crate) fn anchor_selection(&mut self, point: SelectionPoint) {
    self.selection = Some(Selection {
      anchor: point,
      focus: point,
    });
    self.drag = Some(Drag::Selection);
    self.paint_dirty = true;
  }

  /// Follows a selection drag to (`x`, `y`): the exact position under the pointer, or
  /// the nearest one.
  pub(crate) fn extend_selection(&mut self, x: u16, y: u16) {
    let Some(point) = self
      .text_position_at(x, y)
      .or_else(|| self.nearest_text_position(x, y))
    else {
      return;
    };
    let Some(mut selection) = self.selection
    else {
      return;
    };

    if selection.focus != point {
      selection.focus = point;
      self.selection = Some(selection);
      self.paint_dirty = true;
    }
  }

  /// Selects the word segment around `point`, as Unicode word boundaries divide the
  /// text: a word, a whitespace run, or a punctuation mark.
  pub(crate) fn select_word(&mut self, point: SelectionPoint) {
    let Some(text) = self.selectable_text(point.node)
    else {
      return;
    };
    let (from, to) = word_bounds(&text, point.offset);

    self.selection = Some(Selection {
      anchor: SelectionPoint {
        node: point.node,
        offset: from,
      },
      focus: SelectionPoint {
        node: point.node,
        offset: to,
      },
    });
    self.paint_dirty = true;
  }

  /// Selects the whole run at `point`.
  pub(crate) fn select_run(&mut self, point: SelectionPoint) {
    let Some(text) = self.selectable_text(point.node)
    else {
      return;
    };

    self.selection = Some(Selection {
      anchor: SelectionPoint {
        node: point.node,
        offset: 0,
      },
      focus: SelectionPoint {
        node: point.node,
        offset: text::count(&text),
      },
    });
    self.paint_dirty = true;
  }

  /// Clears the document's selection.
  pub(crate) fn clear_selection(&mut self) {
    if self.selection.take().is_some() {
      self.paint_dirty = true;
    }
  }

  /// Collapses the selection inside `entry` at its caret.
  pub(crate) fn collapse_entry_selection(&mut self, entry: Handle) {
    let collapsed = self
      .record_mut(entry)
      .and_then(|record| record.kind.field_mut())
      .is_some_and(|field| {
        let selected = field.selection().is_some();

        field.collapse();

        selected
      });

    if collapsed {
      self.damage(entry);
    }
  }

  /// Drops the selection when one of its ends lives in `handle`, on the node's text
  /// change or removal.
  pub(crate) fn invalidate_selection(&mut self, handle: Handle) {
    if let Some(selection) = self.selection
      && (selection.anchor.node == handle || selection.focus.node == handle)
    {
      self.clear_selection();
    }
  }

  /// Whether the clipboard `action` is offered: every one where the terminal's
  /// clipboard answers, else those the launch chose.
  pub(crate) const fn offers(&self, action: ClipboardAction) -> bool {
    self.clipboard_answers
      || match action {
        ClipboardAction::Copy | ClipboardAction::Cut => self.clipboard.copy,
        ClipboardAction::Paste => self.clipboard.paste,
      }
  }

  /// Stages the selected text for the terminal loop's clipboard write; nothing is
  /// staged when `Copy` is not offered.
  pub(crate) fn copy_selection(&mut self) {
    if self.offers(ClipboardAction::Copy)
      && let Some(text) = self.selected_text()
    {
      self.pending_copy = Some(text);
    }
  }

  /// Stages a clipboard read for the terminal loop, its reply bound for the clipboard's
  /// entry; nothing is staged without one that takes edits, or when `Paste` is not
  /// offered.
  pub(crate) fn request_paste(&mut self) {
    if self.offers(ClipboardAction::Paste)
      && let Some(entry) = self
        .clipboard_entry()
        .filter(|entry| self.is_editable(*entry))
    {
      self.paste_target = Some(entry);
      self.pending_paste = true;
    }
  }
}

/// A point's distance to the span from `start` over `length` along one axis, zero
/// inside it.
const fn axis_distance(point: i64, start: i64, length: i64) -> i64 {
  if point < start {
    start - point
  }
  else if point >= start + length {
    point - (start + length - 1)
  }
  else {
    0
  }
}
