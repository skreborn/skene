//! In-app text selection: source-anchored ranges over selectable runs, and the context
//! menu.
//!
//! A selection's ends live in source space—a run and a character offset into its
//! source text—so it survives reflow and copies as the text appears in the source, soft
//! wraps excluded. A press anchors it, a drag extends it across runs in document order,
//! a double click takes the word, a triple click the run. A text entry's selection lives
//! in its [`Field`](crate::store::Field) instead, over its value. Right-clicking while a
//! selection exists opens the copy menu at the pressed cell, and right-clicking a text
//! entry its edit menu; `Copy` stages the text for the terminal loop's OSC 52 write,
//! `Cut` removes it as well, and `Paste` stages an OSC 52 read whose reply lands in
//! the entry that asked. The launch's [`Clipboard`] says which items exist; within it
//! every item is offered on every terminal, since OSC 52 has no capability query, and a
//! terminal lacking it ignores the write and answers no read.

use std::time::{Duration, Instant};

use crate::{
  Clipboard,
  field::word_bounds,
  layout::{is_paragraph, paragraph_runs, paragraph_text},
  paint::wrap_spans,
  store::{Drag, Handle, Kind, Store},
};

/// One end of a selection: a run and a character offset into its source text.
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

/// The context menu: its top-left cell and the items it lists.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Menu {
  pub(crate) x: u16,
  pub(crate) y: u16,
  pub(crate) kind: MenuKind,
}

/// Which menu is open: the copy menu over the document's selection, or the edit menu
/// over a text entry.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuKind {
  Copy,
  Edit,
}

/// One item of the context menu.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuItem {
  Cut,
  Copy,
  Paste,
}

impl Clipboard {
  /// Whether the configuration offers `item`: `Cut` and `Copy` with `copy`, `Paste`
  /// with `paste`.
  pub(crate) const fn offers(self, item: MenuItem) -> bool {
    match item {
      MenuItem::Cut | MenuItem::Copy => self.copy,
      MenuItem::Paste => self.paste,
    }
  }
}

impl MenuItem {
  /// The item's label.
  pub(crate) const fn label(self) -> &'static str {
    match self {
      Self::Cut => "Cut",
      Self::Copy => "Copy",
      Self::Paste => "Paste",
    }
  }
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
  /// outside every control and the launch offers `Copy`: a control's label is chrome,
  /// whatever it opts into, and without `Copy` nothing in the document is selectable.
  pub(crate) fn selectable_text(&self, handle: Handle) -> Option<String> {
    let record = self.record(handle)?;

    if !self.clipboard.copy || self.enclosing_control(handle).is_some() {
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

  /// The character offset within the node's source at the node-local cell, clamped
  /// into the nearest wrapped line and its ends, against the last layout.
  fn run_offset(&self, handle: Handle, local_x: i64, local_y: i64) -> Option<usize> {
    let text = self.selectable_text(handle)?;
    let (width, _) = self.node_size(handle);
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

  /// The link destination at the cell (`x`, `y`), when the cell carries a linked run's
  /// character—exact hits only, never clamped into a neighboring line.
  pub(crate) fn link_at(&self, x: u16, y: u16) -> Option<String> {
    let path = self.hit_test(x, y);
    let (node, (origin_x, origin_y)) = *path.last()?;
    let (width, _) = self.node_size(node);
    let record = self.record(node)?;
    let row = usize::try_from(i64::from(y) - origin_y).ok()?;
    let column = usize::try_from(i64::from(x) - origin_x).ok()?;

    if let Kind::Run { props, .. } = &record.kind {
      let lines = wrap_spans(&record.text, usize::try_from(width).unwrap_or(1));
      let (_, line) = lines.get(row)?;

      return (column < line.chars().count())
        .then(|| props.link.clone())
        .flatten();
    }

    if !is_paragraph(self, node) {
      return None;
    }

    let text = paragraph_text(self, node);
    let lines = wrap_spans(&text, usize::try_from(width).unwrap_or(1));
    let (start, line) = lines.get(row)?;

    if column >= line.chars().count() {
      return None;
    }

    let offset = start + column;
    let mut cursor = 0;

    paragraph_runs(self, node).into_iter().find_map(|run| {
      let record = self.record(run)?;
      let end = cursor + record.text.chars().count();
      let hit = (cursor..end).contains(&offset);

      cursor = end;

      match &record.kind {
        Kind::Run { props, .. } if hit => props.link.clone(),
        _ => None,
      }
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
    let mut nearest: Option<(Handle, (i64, i64))> = None;

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

  /// Whether anything is selected: a range over the document's runs, or one inside the
  /// focused entry.
  pub(crate) fn has_selection(&self) -> bool {
    self.selection.is_some()
      || self
        .focused_entry()
        .and_then(|entry| self.record(entry)?.kind.field()?.selection())
        .is_some()
  }

  /// The selected text: the focused entry's, or the document's as it appears in the
  /// source—each run's slice, runs joined by newlines—or `None` when nothing is
  /// selected.
  pub(crate) fn selected_text(&self) -> Option<String> {
    if let Some(entry) = self.focused_entry() {
      return self.record(entry)?.kind.field()?.selected_text();
    }

    let (start, end, runs) = self.selection_span()?;
    let mut parts = Vec::new();

    for node in runs {
      let Some(text) = self.selectable_text(node)
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

  /// Selects the characters around `point` sharing its class: a word, a whitespace gap,
  /// or a punctuation run.
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
        offset: text.chars().count(),
      },
    });
    self.paint_dirty = true;
  }

  /// Clears the document's selection and closes the menu.
  pub(crate) fn clear_selection(&mut self) {
    let selection = self.selection.take().is_some();
    let menu = self.menu.take().is_some();

    if selection || menu {
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

  /// Opens the menu with its top-left at the pressed cell.
  fn open_menu(&mut self, x: u16, y: u16, kind: MenuKind) {
    self.menu = Some(Menu { x, y, kind });
    self.paint_dirty = true;
  }

  /// Closes the menu.
  pub(crate) fn close_menu(&mut self) {
    if self.menu.take().is_some() {
      self.paint_dirty = true;
    }
  }

  /// The open menu's items in order—those the launch offers—each with whether it is
  /// enabled: `Cut` and `Copy` need a selection, `Paste` nothing.
  pub(crate) fn menu_items(&self) -> Vec<(MenuItem, bool)> {
    let Some(menu) = self.menu
    else {
      return Vec::new();
    };
    let copies = self.has_selection();
    let items = match menu.kind {
      MenuKind::Copy => vec![(MenuItem::Copy, copies)],
      MenuKind::Edit => vec![
        (MenuItem::Cut, copies),
        (MenuItem::Copy, copies),
        (MenuItem::Paste, true),
      ],
    };

    items
      .into_iter()
      .filter(|(item, _)| self.clipboard.offers(*item))
      .collect()
  }

  /// The open menu's rectangle—origin and size, in cells—shifted to fit the terminal:
  /// a row per item and the border around them.
  pub(crate) fn menu_bounds(&self) -> Option<(i64, i64, i64, i64)> {
    let menu = self.menu?;
    let (columns, rows) = self.last_size;
    let items = self.menu_items();
    let label = items
      .iter()
      .map(|(item, _)| item.label().chars().count())
      .max()
      .unwrap_or(0);
    let width = i64::try_from(label).unwrap_or(0) + 4;
    let height = i64::try_from(items.len()).unwrap_or(0) + 2;
    let x = i64::from(menu.x).min(i64::from(columns) - width).max(0);
    let y = i64::from(menu.y).min(i64::from(rows) - height).max(0);

    Some((x, y, width, height))
  }

  /// The open menu's item at the cell (`x`, `y`), enabled or not.
  pub(crate) fn menu_item_at(&self, x: u16, y: u16) -> Option<(MenuItem, bool)> {
    let (menu_x, menu_y, width, _) = self.menu_bounds()?;
    let (x, y) = (i64::from(x), i64::from(y));

    if x <= menu_x || x >= menu_x + width - 1 {
      return None;
    }

    let row = usize::try_from(y - menu_y - 1).ok()?;

    self.menu_items().get(row).copied()
  }

  /// Stages the selected text for the terminal loop's clipboard write; nothing is
  /// staged when the launch offers no `Copy`.
  pub(crate) fn copy_selection(&mut self) {
    if self.clipboard.copy
      && let Some(text) = self.selected_text()
    {
      self.pending_copy = Some(text);
    }
  }

  /// Stages a clipboard read for the terminal loop, its reply bound for the focused
  /// entry; nothing is staged without one, or when the launch offers no `Paste`.
  pub(crate) fn request_paste(&mut self) {
    if self.clipboard.paste
      && let Some(entry) = self.focused_entry()
    {
      self.paste_target = Some(entry);
      self.pending_paste = true;
    }
  }

  /// A right press: opens the edit menu at the cell over the focused text entry when
  /// the launch offers any of its items, the copy menu while the document holds a
  /// selection, and closes the menu otherwise.
  pub(crate) fn right_press(&mut self, x: u16, y: u16, entry: Option<Handle>) {
    if entry.is_some_and(|entry| self.focused == Some(entry)) && self.clipboard != Clipboard::NONE {
      self.open_menu(x, y, MenuKind::Edit);
    }
    else if self.selection.is_some() {
      self.open_menu(x, y, MenuKind::Copy);
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
