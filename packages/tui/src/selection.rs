//! In-app text selection: source-anchored ranges over selectable runs, and the copy
//! menu.
//!
//! A selection's ends live in source space—a run node and a character offset into its
//! source text—so it survives reflow and copies as the text appears in the source, soft
//! wraps excluded. A press anchors it, a drag extends it across runs in document order,
//! a double click takes the word, a triple click the run. Right-clicking while a
//! selection exists opens the copy menu at the pressed cell; `Copy` stages the text for
//! the terminal loop's OSC 52 write.

use std::time::{Duration, Instant};

use skene_core::NodeId;

use crate::{
  paint::wrap_spans,
  state::{NodeKind, ROOT_ID, State},
};

/// One end of a selection: a run and a character offset into its source text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionPoint {
  pub(crate) node: NodeId,
  pub(crate) offset: usize,
}

/// A selection: the pressed end and the moving end; document order between the two is
/// resolved at use.
#[derive(Clone, Copy)]
pub(crate) struct Selection {
  pub(crate) anchor: SelectionPoint,
  pub(crate) focus: SelectionPoint,
}

/// The copy menu's sole item label.
pub(crate) const MENU_ITEM: &str = "Copy";

/// The disabled menu's hint line, shown when the terminal discards clipboard writes.
pub(crate) const MENU_HINT: &str = "no terminal support";

/// The window between two presses of one multi-click sequence.
const MULTI_CLICK: Duration = Duration::from_millis(500);

/// The class a character groups with under a double click.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CharacterClass {
  Word,
  Space,
  Other,
}

/// The class of one character: word characters, whitespace, or punctuation.
fn character_class(character: char) -> CharacterClass {
  if character.is_alphanumeric() || character == '_' {
    CharacterClass::Word
  }
  else if character.is_whitespace() {
    CharacterClass::Space
  }
  else {
    CharacterClass::Other
  }
}

impl State {
  /// Counts a press at (`x`, `y`) into its multi-click sequence: 1 for a fresh press,
  /// 2 for a double click, 3 for a triple; a fourth begins a new sequence.
  pub(crate) fn register_press(&self, x: u16, y: u16) -> u8 {
    let now = Instant::now();
    let count = match self.last_press.get() {
      Some((at, press_x, press_y, count))
        if press_x == x && press_y == y && now.duration_since(at) <= MULTI_CLICK && count < 3 =>
      {
        count + 1
      }
      _ => 1,
    };

    self.last_press.set(Some((now, x, y, count)));

    count
  }

  /// The run's source text, when `node` is a selectable, non-empty text run.
  fn run_text(&self, node: NodeId) -> Option<String> {
    let nodes = self.nodes.borrow();

    nodes
      .get(&node)
      .and_then(|record| record.kind.selectable_text())
  }

  /// The character offset within `node`'s source at the run-local cell, clamped into
  /// the nearest wrapped line and its ends, against the last layout.
  fn run_offset(&self, node: NodeId, local_x: i64, local_y: i64) -> Option<usize> {
    let text = self.run_text(node)?;
    let (width, _) = self.node_size(node);
    let lines = wrap_spans(&text, usize::try_from(width).unwrap_or(1));
    let row = usize::try_from(local_y.max(0))
      .unwrap_or(0)
      .min(lines.len().saturating_sub(1));
    let (start, line) = &lines[row];
    let length = line.chars().count();
    let column = usize::try_from(local_x.max(0)).unwrap_or(0).min(length);

    Some(start + column)
  }

  /// The source position at the cell (`x`, `y`), when it lies on a selectable run.
  pub(crate) fn text_position_at(&self, x: u16, y: u16) -> Option<SelectionPoint> {
    let path = self.hit_test(x, y);
    let (node, (origin_x, origin_y)) = *path.last()?;
    let offset = self.run_offset(node, i64::from(x) - origin_x, i64::from(y) - origin_y)?;

    Some(SelectionPoint { node, offset })
  }

  /// The link destination at the cell (`x`, `y`), when the cell carries a linked
  /// span's character—exact hits only, never clamped into a neighboring line.
  pub(crate) fn link_at(&self, x: u16, y: u16) -> Option<String> {
    let path = self.hit_test(x, y);
    let (node, (origin_x, origin_y)) = *path.last()?;
    let (width, _) = self.node_size(node);
    let nodes = self.nodes.borrow();
    let NodeKind::Paragraph { spans, .. } = &nodes.get(&node)?.kind
    else {
      return None;
    };
    let text: String = spans.iter().map(|span| span.text.as_str()).collect();
    let lines = wrap_spans(&text, usize::try_from(width).unwrap_or(1));
    let row = usize::try_from(i64::from(y) - origin_y).ok()?;
    let column = usize::try_from(i64::from(x) - origin_x).ok()?;
    let (start, line) = lines.get(row)?;

    if column >= line.chars().count() {
      return None;
    }

    let offset = start + column;
    let mut cursor = 0;

    spans.iter().find_map(|span| {
      let end = cursor + span.text.chars().count();
      let hit = (cursor..end).contains(&offset);

      cursor = end;

      if hit { span.link.clone() } else { None }
    })
  }

  /// The source position nearest the cell (`x`, `y`) over every selectable run—by
  /// vertical distance first, then horizontal, earlier runs winning ties. Runs outside
  /// an open modal are ignored while one is open.
  fn nearest_text_position(&self, x: u16, y: u16) -> Option<SelectionPoint> {
    let modal = match self.topmost_open_overlay() {
      Some((top, true)) => Some(top),
      _ => None,
    };
    let (pointer_x, pointer_y) = (i64::from(x), i64::from(y));
    let mut best: Option<(i64, i64)> = None;
    let mut nearest: Option<(NodeId, (i64, i64))> = None;

    for node in self.document_runs() {
      if let Some(modal) = modal
        && !self.ancestors(node).contains(&modal)
      {
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

  /// Every selectable run in document order, the subtrees of closed overlays excluded.
  fn document_runs(&self) -> Vec<NodeId> {
    let nodes = self.nodes.borrow();
    let mut order = Vec::new();
    let mut stack = vec![ROOT_ID];

    while let Some(current) = stack.pop() {
      let Some(record) = nodes.get(&current)
      else {
        continue;
      };

      if let NodeKind::Overlay { open: false, .. } = record.kind {
        continue;
      }

      if record.kind.selectable_text().is_some() {
        order.push(current);
      }

      // Reverse, so the stack pops children in tree order.
      stack.extend(record.children.iter().rev());
    }

    order
  }

  /// The selection's ends in document order, with every selectable run it spans.
  fn selection_span(&self) -> Option<(SelectionPoint, SelectionPoint, Vec<NodeId>)> {
    let selection = self.selection.get()?;
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

  /// The selected character range within `node`'s source, when the selection touches
  /// it; a range running past the node's end is `usize::MAX`, clamped by the caller.
  pub(crate) fn selection_range(&self, node: NodeId) -> Option<(usize, usize)> {
    let (start, end, runs) = self.selection_span()?;

    if !runs.contains(&node) {
      return None;
    }

    let from = if node == start.node { start.offset } else { 0 };
    let to = if node == end.node {
      end.offset
    }
    else {
      usize::MAX
    };

    (from < to).then_some((from, to))
  }

  /// The selected text as it appears in the source—each run's slice, runs joined by
  /// newlines—or `None` when nothing is selected.
  pub(crate) fn selected_text(&self) -> Option<String> {
    let (start, end, runs) = self.selection_span()?;
    let mut parts = Vec::new();

    for node in runs {
      let Some(text) = self.run_text(node)
      else {
        continue;
      };

      let count = text.chars().count();
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
        parts.push(text.chars().skip(from).take(to - from).collect::<String>());
      }
    }

    (!parts.is_empty()).then(|| parts.join("\n"))
  }

  /// Anchors a fresh selection at `point` and starts its drag.
  pub(crate) fn anchor_selection(&self, point: SelectionPoint) {
    self.selection.set(Some(Selection {
      anchor: point,
      focus: point,
    }));
    self.drag.set(Some(crate::state::Drag::Selection));
    self.dirty.set(true);
  }

  /// Follows a selection drag to (`x`, `y`): the exact position under the pointer, or
  /// the nearest one.
  pub(crate) fn extend_selection(&self, x: u16, y: u16) {
    let Some(point) = self
      .text_position_at(x, y)
      .or_else(|| self.nearest_text_position(x, y))
    else {
      return;
    };

    let Some(mut selection) = self.selection.get()
    else {
      return;
    };

    if selection.focus != point {
      selection.focus = point;
      self.selection.set(Some(selection));
      self.dirty.set(true);
    }
  }

  /// Selects the characters around `point` sharing its class: a word, a whitespace gap,
  /// or a punctuation run.
  pub(crate) fn select_word(&self, point: SelectionPoint) {
    let Some(text) = self.run_text(point.node)
    else {
      return;
    };

    let characters: Vec<char> = text.chars().collect();
    let at = point.offset.min(characters.len() - 1);
    let class = character_class(characters[at]);
    let mut from = at;
    let mut to = at + 1;

    while from > 0 && character_class(characters[from - 1]) == class {
      from -= 1;
    }

    while to < characters.len() && character_class(characters[to]) == class {
      to += 1;
    }

    self.selection.set(Some(Selection {
      anchor: SelectionPoint {
        node: point.node,
        offset: from,
      },
      focus: SelectionPoint {
        node: point.node,
        offset: to,
      },
    }));
    self.dirty.set(true);
  }

  /// Selects the whole run at `point`.
  pub(crate) fn select_run(&self, point: SelectionPoint) {
    let Some(text) = self.run_text(point.node)
    else {
      return;
    };

    self.selection.set(Some(Selection {
      anchor: SelectionPoint {
        node: point.node,
        offset: 0,
      },
      focus: SelectionPoint {
        node: point.node,
        offset: text.chars().count(),
      },
    }));
    self.dirty.set(true);
  }

  /// Clears the selection and closes the copy menu.
  pub(crate) fn clear_selection(&self) {
    let selection = self.selection.take().is_some();
    let menu = self.menu.take().is_some();

    if selection || menu {
      self.dirty.set(true);
    }
  }

  /// Drops the selection when one of its ends lives in `node`, on the node's text
  /// change or removal.
  pub(crate) fn invalidate_selection(&self, node: NodeId) {
    if let Some(selection) = self.selection.get()
      && (selection.anchor.node == node || selection.focus.node == node)
    {
      self.clear_selection();
    }
  }

  /// Opens the copy menu with its top-left at the pressed cell, when a selection
  /// exists.
  pub(crate) fn open_menu(&self, x: u16, y: u16) {
    if self.selected_text().is_some() {
      self.menu.set(Some((x, y)));
      self.dirty.set(true);
    }
  }

  /// Closes the copy menu.
  pub(crate) fn close_menu(&self) {
    if self.menu.take().is_some() {
      self.dirty.set(true);
    }
  }

  /// The open menu's rectangle—origin and size, in cells—shifted to fit the terminal;
  /// a disabled menu grows a row for its hint.
  pub(crate) fn menu_bounds(&self) -> Option<(i64, i64, i64, i64)> {
    let (x, y) = self.menu.get()?;
    let (columns, rows) = self.last_size.get();
    let (label, height) = if self.clipboard.get() {
      (MENU_ITEM.chars().count(), 3)
    }
    else {
      (MENU_ITEM.chars().count().max(MENU_HINT.chars().count()), 4)
    };
    let width = i64::try_from(label).unwrap_or(0) + 4;
    let x = i64::from(x).min(i64::from(columns) - width).max(0);
    let y = i64::from(y).min(i64::from(rows) - height).max(0);

    Some((x, y, width, height))
  }

  /// Whether the cell (`x`, `y`) lies on the open menu's `Copy` item.
  pub(crate) fn copy_hit(&self, x: u16, y: u16) -> bool {
    let Some((menu_x, menu_y, width, _)) = self.menu_bounds()
    else {
      return false;
    };

    let (x, y) = (i64::from(x), i64::from(y));

    y == menu_y + 1 && x > menu_x && x < menu_x + width - 1
  }

  /// Stages the selected text for the terminal loop's clipboard write.
  pub(crate) fn copy_selection(&self) {
    if let Some(text) = self.selected_text() {
      *self.pending_copy.borrow_mut() = Some(text);
    }
  }

  /// A right press: opens the copy menu at the cell while a selection exists, and
  /// closes it otherwise.
  pub(crate) fn right_press(&self, x: u16, y: u16) {
    if self.selection.get().is_some() {
      self.open_menu(x, y);
    }
    else {
      self.close_menu();
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
