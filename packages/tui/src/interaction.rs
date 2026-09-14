//! The terminal's interaction: hit testing, hover and focus, presses and clicks,
//! wheel scrolling and scrollbar drags, each routed through core's dispatch.
//!
//! No borrow of the store is held while a handler runs: every entry point reads what
//! it needs, releases the store, dispatches, then writes the outcome back.

use skene_core::{
  Axis,
  ControlKind,
  KeyEvent,
  Modifiers,
  MouseCursor,
  NodeId,
  Overflow,
  Placement,
  PointerButton,
  PointerEvent,
  Prevented,
  ScrollEvent,
  dispatch_click,
  dispatch_key_down,
  dispatch_pointer_down,
  dispatch_pointer_move,
  dispatch_pointer_up,
  notify_blur,
  notify_dismiss,
  notify_focus,
  notify_pointer_enter,
  notify_pointer_leave,
  notify_scroll,
};

use crate::{
  Tui,
  layout::{inline_box_cells, is_paragraph},
  paint::control_kind,
  selection::MenuItem,
  store::{Drag, Handle, Kind, Press, Record, ScrollLatch, Store},
  with_store,
};

/// The rows or columns one wheel report scrolls: a browser's three lines per notch.
pub(crate) const WHEEL_STEP: i64 = 3;

/// The focus move an overlay's opening or closing asks for.
pub(crate) enum OverlayFocus {
  /// Focus stays where it is.
  Held,
  /// Focus moves to the node, or leaves every node.
  Moved(Option<Handle>),
}

impl Store {
  /// Every open overlay, in build order, reachable from the root; later entries stack
  /// on top.
  pub(crate) fn open_overlays(&self) -> Vec<Handle> {
    self
      .overlays
      .iter()
      .copied()
      .filter(|overlay| {
        self.record(*overlay).is_some_and(Record::is_open_overlay) && self.is_reachable(*overlay)
      })
      .collect()
  }

  /// The topmost open overlay and whether it is modal.
  pub(crate) fn topmost_open_overlay(&self) -> Option<(Handle, bool)> {
    self.open_overlays().into_iter().rev().find_map(|overlay| {
      match self.record(overlay).map(|record| &record.kind) {
        Some(Kind::Overlay { modal, .. }) => Some((overlay, *modal)),
        _ => None,
      }
    })
  }

  /// The tab stops in traversal order: positioned ones first by position, then the
  /// rest in document order; closed overlays excluded, and an open modal containing
  /// the whole order.
  pub(crate) fn tab_order(&self) -> Vec<Handle> {
    let mut positioned = Vec::new();
    let mut ordered = Vec::new();
    let mut stack = vec![self.root];

    while let Some(current) = stack.pop() {
      let Some(record) = self.record(current)
      else {
        continue;
      };

      if record.is_overlay() && !record.is_open_overlay() {
        continue;
      }

      match record.focus_order() {
        Some(skene_core::FocusOrder::Position(position)) => positioned.push((position, current)),
        Some(skene_core::FocusOrder::Auto) => ordered.push(current),
        Some(skene_core::FocusOrder::Skip) | None => {}
      }

      stack.extend(record.children.iter().rev());
    }

    positioned.sort_by_key(|(position, _)| *position);

    let mut order: Vec<Handle> = positioned.into_iter().map(|(_, handle)| handle).collect();

    order.extend(ordered);

    if let Some((top, true)) = self.topmost_open_overlay() {
      order.retain(|node| self.ancestors(*node).contains(&top));
    }

    order
  }

  /// The path from the root to the deepest node containing the cell (`x`, `y`), each
  /// with its absolute origin, against the last computed layout.
  pub(crate) fn hit_test(&self, x: u16, y: u16) -> Vec<(Handle, (i64, i64))> {
    // Overlays hit first, topmost first; below an open modal nothing is hittable.
    for overlay in self.open_overlays().iter().rev() {
      let mut path = Vec::new();

      if self.hit_overlay(*overlay, x, y, &mut path) {
        return path;
      }

      if matches!(self.topmost_open_overlay(), Some((top, true)) if top == *overlay) {
        return Vec::new();
      }
    }

    let mut path = Vec::new();

    self.hit_walk(self.root, 0, 0, i64::from(x), i64::from(y), &mut path);

    path
  }

  /// Descends into the deepest child containing the point, preferring later (topmost)
  /// children, accumulating the path.
  fn hit_walk(
    &self,
    handle: Handle,
    offset_x: i64,
    offset_y: i64,
    x: i64,
    y: i64,
    path: &mut Vec<(Handle, (i64, i64))>,
  ) -> bool {
    let Some(record) = self.record(handle)
    else {
      return false;
    };
    let children = record.children.clone();

    if !record.kind.is_laid_out() {
      for child in children.iter().rev() {
        if self.hit_walk(*child, offset_x, offset_y, x, y, path) {
          return true;
        }
      }

      return false;
    }

    let (location_x, location_y) = self.node_location(handle);
    let (origin_x, origin_y) = (offset_x + location_x, offset_y + location_y);
    let (width, height) = self.node_size(handle);
    let contains = x >= origin_x && x < origin_x + width && y >= origin_y && y < origin_y + height;

    if handle != self.root && !contains {
      return false;
    }

    path.push((handle, (origin_x, origin_y)));

    if is_paragraph(self, handle) {
      // An inline box hits through the cells its placeholder flowed into.
      for (run, (cell_x, cell_y)) in inline_box_cells(self, handle) {
        let (location_x, location_y) = self.node_location(run);

        if self.hit_walk(
          run,
          origin_x + cell_x - location_x,
          origin_y + cell_y - location_y,
          x,
          y,
          path,
        ) {
          break;
        }
      }

      return true;
    }

    // A scrolled box shifts its children by its offsets.
    let (scroll_x, scroll_y) = self.scroll_offset(handle);
    let (child_x, child_y) = (
      origin_x - i64::from(scroll_x),
      origin_y - i64::from(scroll_y),
    );

    // Later children paint on top, so they win the hit; overlays hit separately.
    for child in children.iter().rev() {
      if self.record(*child).is_some_and(Record::is_overlay) {
        continue;
      }

      if self.hit_walk(*child, child_x, child_y, x, y, path) {
        break;
      }
    }

    true
  }

  /// Hits `overlay`'s subtree at its computed origin, accumulating the path.
  pub(crate) fn hit_overlay(
    &self,
    overlay: Handle,
    x: u16,
    y: u16,
    path: &mut Vec<(Handle, (i64, i64))>,
  ) -> bool {
    let (origin_x, origin_y) = self.overlay_origin(overlay);
    let (location_x, location_y) = self.node_location(overlay);

    self.hit_walk(
      overlay,
      origin_x - location_x,
      origin_y - location_y,
      i64::from(x),
      i64::from(y),
      path,
    )
  }

  /// A node's absolute origin: its layout locations summed up the parent chain, each
  /// ancestor's scroll offsets subtracted.
  pub(crate) fn absolute_origin(&self, handle: Handle) -> (i64, i64) {
    let mut current = Some(handle);
    let (mut x, mut y) = (0, 0);

    while let Some(visited) = current {
      let Some(record) = self.record(visited)
      else {
        break;
      };

      if record.kind.is_laid_out() {
        let (location_x, location_y) = self.inline_location(visited);

        x += location_x;
        y += location_y;

        if let Some(parent) = self.layout_parent(visited) {
          let (scroll_x, scroll_y) = self.scroll_offset(parent);

          x -= i64::from(scroll_x);
          y -= i64::from(scroll_y);
        }
      }

      current = record.parent;
    }

    (x, y)
  }

  /// A node's position within its layout parent: its laid-out location, or the cell
  /// its placeholder flowed into for an inline box inside a paragraph.
  fn inline_location(&self, handle: Handle) -> (i64, i64) {
    let inline = self
      .layout_parent(handle)
      .filter(|parent| is_paragraph(self, *parent));

    match inline {
      Some(paragraph) => inline_box_cells(self, paragraph)
        .into_iter()
        .find(|(run, _)| *run == handle)
        .map_or((0, 0), |(_, cell)| cell),
      None => self.node_location(handle),
    }
  }

  /// A node's on-screen origin: like [`absolute_origin`](Self::absolute_origin), but
  /// content inside an overlay positions against the overlay's computed origin.
  pub(crate) fn screen_origin(&self, handle: Handle) -> (i64, i64) {
    let (mut x, mut y) = (0, 0);
    let mut current = Some(handle);

    while let Some(visited) = current {
      let Some(record) = self.record(visited)
      else {
        break;
      };

      if record.is_overlay() {
        let (overlay_x, overlay_y) = self.overlay_origin(visited);

        return (x + overlay_x, y + overlay_y);
      }

      if record.kind.is_laid_out() {
        let (location_x, location_y) = self.inline_location(visited);

        x += location_x;
        y += location_y;

        if let Some(parent) = self.layout_parent(visited) {
          let (scroll_x, scroll_y) = self.scroll_offset(parent);

          x -= i64::from(scroll_x);
          y -= i64::from(scroll_y);
        }
      }

      current = record.parent;
    }

    (x, y)
  }

  /// An open overlay's absolute origin: anchored `Top`/`Bottom` position against the
  /// anchor, everything else against the terminal.
  pub(crate) fn overlay_origin(&self, handle: Handle) -> (i64, i64) {
    let (columns, rows) = self.last_size;
    let (columns, rows) = (i64::from(columns), i64::from(rows));
    let Some(Kind::Overlay {
      placement, anchor, ..
    }) = self.record(handle).map(|record| &record.kind)
    else {
      return (0, 0);
    };
    let (placement, anchor) = (*placement, *anchor);
    let (width, height) = self.node_size(handle);

    if let Some(anchor) = anchor.filter(|_| matches!(placement, Placement::Top | Placement::Bottom))
    {
      let (anchor_x, anchor_y) = self.absolute_origin(anchor);
      let (_, anchor_height) = self.node_size(anchor);
      let x = anchor_x.min(columns - width).max(0);
      let below = anchor_y + anchor_height;
      let above = anchor_y - height;

      // The preferred side flips when it runs out of room, as a browser popup does.
      return match placement {
        Placement::Top => {
          if above < 0 && below + height <= rows {
            (x, below)
          }
          else {
            (x, above.max(0))
          }
        }
        _ => {
          if below + height > rows && above >= 0 {
            (x, above)
          }
          else {
            (x, below.min(rows - height).max(0))
          }
        }
      };
    }

    match placement {
      Placement::Center => ((columns - width) / 2, (rows - height) / 2),
      Placement::Top => ((columns - width) / 2, 0),
      Placement::TopStart => (0, 0),
      Placement::TopEnd => (columns - width, 0),
      Placement::Bottom => ((columns - width) / 2, rows - height),
      Placement::BottomStart => (0, rows - height),
      Placement::BottomEnd => (columns - width, rows - height),
    }
  }

  /// The pointer shape for the current hover path: the deepest hovered node with a
  /// cursor set, else the default its kind implies.
  pub(crate) fn hover_cursor(&self) -> Option<MouseCursor> {
    if matches!(self.drag, Some(Drag::Scrollbar { .. })) || self.drags_range() {
      return Some(MouseCursor::Grabbing);
    }

    if matches!(self.drag, Some(Drag::Caret(_))) {
      return Some(MouseCursor::Text);
    }

    let (pointer_x, pointer_y) = self.last_pointer;

    if self.link_at(pointer_x, pointer_y).is_some() {
      return Some(MouseCursor::Click);
    }

    self.hovered().iter().rev().find_map(|handle| {
      let record = self.record(*handle)?;

      if let Some(style) = record.kind.box_style()
        && let Some(cursor) = style.cursor
      {
        return Some(cursor);
      }

      match control_kind(self, *handle) {
        Some(ControlKind::TextField | ControlKind::TextEditor) => return Some(MouseCursor::Text),
        Some(ControlKind::Range) => return Some(MouseCursor::Grab),
        Some(_) => return Some(MouseCursor::Click),
        None => {}
      }

      if record.is_focusable() {
        return Some(MouseCursor::Click);
      }

      self
        .selectable_text(*handle)
        .is_some()
        .then_some(MouseCursor::Text)
    })
  }

  /// Whether a held left press landed on a slider, which follows the pointer until the
  /// release.
  fn drags_range(&self) -> bool {
    self.left_held
      && self
        .press
        .as_ref()
        .and_then(|press| press.target)
        .is_some_and(|target| control_kind(self, target) == Some(ControlKind::Range))
  }

  /// A node's resolved overflow, `Visible` when unset or the node has no box style.
  fn overflow(&self, handle: Handle) -> Overflow {
    self
      .record(handle)
      .and_then(|record| record.kind.box_style())
      .and_then(|style| style.overflow)
      .unwrap_or(Overflow::Visible)
  }

  /// Whether the node's content scrolls: a box overflowing by its style, or an editor
  /// windowing its wrapped rows.
  pub(crate) fn is_scrollable(&self, handle: Handle) -> bool {
    self.is_editor(handle) || matches!(self.overflow(handle), Overflow::Scroll | Overflow::Auto)
  }

  /// Whether the node clips its children when painting.
  pub(crate) fn clips(&self, handle: Handle) -> bool {
    !matches!(self.overflow(handle), Overflow::Visible)
  }

  /// A node's scroll offsets, clamped to what the last layout allows; an editor's is
  /// its window down its rows.
  pub(crate) fn scroll_offset(&self, handle: Handle) -> (u32, u32) {
    if let Some(window) = self.editor_window(handle) {
      return (0, window);
    }

    let Some((x, y)) = self.record(handle).map(|record| record.scroll)
    else {
      return (0, 0);
    };

    if (x, y) == (0, 0) {
      return (0, 0);
    }

    let (limit_x, limit_y) = self.scroll_limits(handle);

    (x.min(limit_x), y.min(limit_y))
  }

  /// A node's maximum scroll offsets, from the last computed layout; an editor's is
  /// how far its window can scroll down its rows.
  pub(crate) fn scroll_limits(&self, handle: Handle) -> (u32, u32) {
    if let Some(limit) = self.editor_window_limit(handle) {
      return (0, limit);
    }

    let Some(taffy) = self.record(handle).and_then(|record| record.taffy)
    else {
      return (0, 0);
    };

    match self.taffy.layout(taffy) {
      #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
      Ok(layout) => (
        layout.scroll_width().round().max(0.) as u32,
        layout.scroll_height().round().max(0.) as u32,
      ),
      Err(_) => (0, 0),
    }
  }

  /// The offsets `handle` would scroll to by a delta, clamped; `None` when it does not
  /// scroll or cannot move further.
  fn scrolled_by(&self, handle: Handle, delta_x: i64, delta_y: i64) -> Option<(u32, u32)> {
    if !self.is_scrollable(handle) {
      return None;
    }

    let (limit_x, limit_y) = self.scroll_limits(handle);
    let (x, y) = self.scroll_offset(handle);
    let next = (
      u32::try_from((i64::from(x) + delta_x).clamp(0, i64::from(limit_x))).expect("clamped"),
      u32::try_from((i64::from(y) + delta_y).clamp(0, i64::from(limit_y))).expect("clamped"),
    );

    (next != (x, y)).then_some(next)
  }

  /// Stores a node's offsets and repaints; the scroll notification is the caller's. An
  /// editor's window moves instead, notifying nothing.
  fn write_scroll(&mut self, handle: Handle, offsets: (u32, u32)) -> Option<NodeId> {
    self.damage(handle);

    if self.is_editor(handle) {
      self.set_editor_window(handle, offsets.1);

      return None;
    }

    let record = self.record_mut(handle)?;

    record.scroll = offsets;

    record.node
  }

  /// A scrollable's track along `axis`: its absolute start, its length in cells, and
  /// the scroll limit.
  fn scroll_geometry(&self, handle: Handle, axis: Axis) -> (i64, i64, u32) {
    let (limit_x, limit_y) = self.scroll_limits(handle);
    let (origin_x, origin_y) = self.absolute_origin(handle);
    let (width, height) = self.node_size(handle);
    let border = self.node_border(handle);

    match axis {
      Axis::Vertical => (origin_y + border.1, height - border.1 - border.3, limit_y),
      Axis::Horizontal => (origin_x + border.0, width - border.0 - border.2, limit_x),
    }
  }

  /// The scrollbar under (`x`, `y`), if any: the owning scrollable, its axis, and the
  /// pointer's position within the track, exact where the terminal reports pixels.
  fn scrollbar_hit(&self, x: u16, y: u16) -> Option<(Handle, Axis, f32)> {
    let (px, py) = (i64::from(x), i64::from(y));
    let (within_x, within_y) = self.pointer_within;

    for handle in self.handles() {
      if !self.is_scrollable(handle) {
        continue;
      }

      let (limit_x, limit_y) = self.scroll_limits(handle);
      let (origin_x, origin_y) = self.absolute_origin(handle);
      let (width, height) = self.node_size(handle);

      if limit_y > 0 && px == origin_x + width - 1 {
        let (track_start, track, _) = self.scroll_geometry(handle, Axis::Vertical);

        if py >= track_start && py < track_start + track {
          return Some((handle, Axis::Vertical, cells(py - track_start) + within_y));
        }
      }

      if limit_x > 0 && py == origin_y + height - 1 {
        let (track_start, track, _) = self.scroll_geometry(handle, Axis::Horizontal);

        if px >= track_start && px < track_start + track {
          return Some((handle, Axis::Horizontal, cells(px - track_start) + within_x));
        }
      }
    }

    None
  }

  /// The offsets that land the thumb's start nearest `target_start`, in track cells,
  /// clamped to the track.
  #[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
  )]
  fn thumb_offsets(
    &self,
    handle: Handle,
    axis: Axis,
    track: i64,
    limit: u32,
    target_start: f32,
  ) -> (u32, u32) {
    let (_, thumb) = crate::paint::thumb_span(isize::try_from(track).unwrap_or(0), 0, limit);
    let room = cells(track - thumb as i64);
    let target_start = target_start.clamp(0., room.max(0.));
    let offset = (target_start * limit as f32 / room.max(1.)).round() as u32;
    let (x, y) = self.scroll_offset(handle);

    match axis {
      Axis::Horizontal => (offset, y),
      Axis::Vertical => (x, offset),
    }
  }

  /// The open anchored popups focus leaves when it moves to `next`: neither the popup
  /// nor the anchor it opened from holds the new focus, so each asks to close.
  fn popups_left_by(&self, next: Option<Handle>) -> Vec<NodeId> {
    let ancestors = next.map(|next| self.ancestors(next)).unwrap_or_default();

    self
      .open_overlays()
      .into_iter()
      .filter_map(|overlay| {
        let record = self.record(overlay)?;
        let Kind::Overlay {
          modal: false,
          anchor: Some(anchor),
          ..
        } = &record.kind
        else {
          return None;
        };

        if ancestors.contains(&overlay) || ancestors.contains(anchor) {
          return None;
        }

        record.node
      })
      .collect()
  }

  /// Opens or closes an overlay against `anchor`, returning the focus move the change
  /// asks for. A modal opening takes focus to its first tab stop, or from every node
  /// when it has none; an overlay closing—a modal, or a popup holding the focus—returns
  /// it to the node focused when it opened, else to its anchor's focus holder.
  pub(crate) fn set_overlay_open(
    &mut self,
    handle: Handle,
    open: bool,
    anchor: Option<Handle>,
  ) -> OverlayFocus {
    let focused = self.focused;
    let Some(Kind::Overlay {
      open: held,
      anchor: bound,
      modal,
      return_focus,
      ..
    }) = self.record_mut(handle).map(|record| &mut record.kind)
    else {
      return OverlayFocus::Held;
    };
    let was_open = std::mem::replace(held, open);
    let modal = *modal;
    let previous = match (was_open, open) {
      (false, true) => std::mem::replace(return_focus, focused),
      (true, false) => return_focus.take(),
      _ => None,
    };

    *bound = anchor;
    self.layout_dirty = true;
    self.paint_dirty = true;

    if open == was_open {
      return OverlayFocus::Held;
    }

    if open {
      if !modal {
        return OverlayFocus::Held;
      }

      return OverlayFocus::Moved(
        self
          .tab_order()
          .into_iter()
          .find(|stop| self.ancestors(*stop).contains(&handle)),
      );
    }

    let inside = focused.is_some_and(|focused| self.ancestors(focused).contains(&handle));

    if !modal && !inside {
      return OverlayFocus::Held;
    }

    OverlayFocus::Moved(
      previous
        .filter(|previous| {
          self.is_reachable(*previous) && self.record(*previous).is_some_and(Record::is_focusable)
        })
        .or_else(|| anchor.and_then(|anchor| self.focus_holder(anchor))),
    )
  }

  /// Whether the record is a text entry.
  pub(crate) fn is_field(&self, handle: Handle) -> bool {
    self
      .record(handle)
      .is_some_and(|record| record.kind.field().is_some())
  }
}

/// The store, released; runs `operation` on it.
fn store<R>(operation: impl FnOnce(&mut Store) -> R) -> R {
  with_store(operation)
}

/// Moves focus to `next`, notifying the node focus left and the one it reached; the
/// newly focused node is scrolled into view at the next render, a newly focused
/// single-line entry selects its whole value with the caret at its end, an editor puts
/// its caret at the end, and the entry focus left collapses its selection.
pub(crate) fn set_focus(next: Option<Handle>) {
  let (previous, gained, dismissed) = store(|store| {
    let previous = std::mem::replace(&mut store.focused, next);

    if previous == next {
      return (None, None, Vec::new());
    }

    if let Some(previous) = previous {
      store.collapse_entry_selection(previous);
      store.damage(previous);
    }

    if let Some(next) = next {
      if let Some(field) = store
        .record_mut(next)
        .and_then(|record| record.kind.field_mut())
      {
        if field.lines <= 1 {
          field.select_all();
        }
        else {
          field.move_caret(field.count(), false);
        }
      }

      store.settle_entry(next);
      store.damage(next);
      store.reveal = Some(next);
    }

    (
      previous.and_then(|previous| store.record(previous).and_then(|record| record.node)),
      next.and_then(|next| store.record(next).and_then(|record| record.node)),
      store.popups_left_by(next),
    )
  });

  if let Some(previous) = previous {
    notify_blur::<Tui>(previous);
  }

  if let Some(gained) = gained {
    notify_focus::<Tui>(gained);
  }

  for popup in dismissed {
    notify_dismiss::<Tui>(popup);
  }
}

/// Moves focus by `step` through the tab stops, wrapping at both ends.
pub(crate) fn move_focus(step: isize) {
  let next = store(|store| {
    let order = store.tab_order();

    if order.is_empty() {
      return None;
    }

    Some(
      match store
        .focused
        .and_then(|focused| order.iter().position(|node| *node == focused))
      {
        Some(index) => {
          let count = isize::try_from(order.len()).expect("a small focus order");
          let index = isize::try_from(index).expect("a small focus order");

          order[usize::try_from((index + step).rem_euclid(count)).expect("in range")]
        }
        None => {
          if step >= 0 {
            order[0]
          }
          else {
            order[order.len() - 1]
          }
        }
      },
    )
  });

  if next.is_some() {
    set_focus(next);
  }
}

/// Updates the hover path to the nodes under (`x`, `y`), notifying the nodes the
/// pointer left and entered.
pub(crate) fn set_hover(x: u16, y: u16) {
  let (left, entered) = store(|store| {
    let next: Vec<Handle> = store.hit_test(x, y).iter().map(|(node, _)| *node).collect();

    if std::mem::replace(&mut store.last_pointer, (x, y)) != (x, y) {
      // A real pointer move releases the wheel's latched target.
      store.scroll_latch = None;

      if store.menu.is_some() {
        store.paint_dirty = true;
      }
    }

    if store.hovered() == next {
      return (Vec::new(), Vec::new());
    }

    // The nodes whose hover state changed repaint through the restyle that follows,
    // where their resolved style differs.
    let previous = store.set_hovered(next);
    let node_of =
      |store: &Store, handle: Handle| store.record(handle).and_then(|record| record.node);
    let left: Vec<NodeId> = previous
      .iter()
      .rev()
      .filter(|node| !store.hovered().contains(node))
      .filter_map(|handle| node_of(store, *handle))
      .collect();
    let entered: Vec<NodeId> = store
      .hovered()
      .iter()
      .filter(|node| !previous.contains(node))
      .filter_map(|handle| node_of(store, *handle))
      .collect();

    (left, entered)
  });

  for node in left {
    notify_pointer_leave::<Tui>(node);
  }

  for node in entered {
    notify_pointer_enter::<Tui>(node);
  }
}

/// A pointer event at the cell (`x`, `y`) for a node whose screen origin is `origin`:
/// the positions carry where the pointer sits within the cell, its center unless the
/// terminal reported pixels.
fn pointer_event(
  x: u16,
  y: u16,
  origin: (i64, i64),
  button: Option<PointerButton>,
  modifiers: Modifiers,
) -> PointerEvent {
  let (within_x, within_y) = store(|store| store.pointer_within);

  #[expect(clippy::cast_precision_loss)]
  PointerEvent {
    x: f32::from(x) + within_x,
    y: f32::from(y) + within_y,
    local_x: (i64::from(x) - origin.0) as f32 + within_x,
    local_y: (i64::from(y) - origin.1) as f32 + within_y,
    button,
    modifiers,
  }
}

/// The deepest node under (`x`, `y`) with its origin, or the node the pressed button
/// captured.
fn pointer_target(store: &Store, x: u16, y: u16) -> Option<(Handle, (i64, i64))> {
  if let Some(press) = &store.press
    && store.left_held
  {
    return Some((press.captured, store.screen_origin(press.captured)));
  }

  store.hit_test(x, y).last().copied()
}

/// Dispatches a pointer move at (`x`, `y`): the hover path follows it, the move
/// reaches the node under the pointer or the one the press captured, and a drag in
/// progress follows.
pub(crate) fn pointer_move(x: u16, y: u16, button: Option<PointerButton>, modifiers: Modifiers) {
  if !store(|store| store.mouse) {
    return;
  }

  set_hover(x, y);

  let target = store(|store| pointer_target(store, x, y));

  if let Some((handle, origin)) = target
    && let Some(node) = store(|store| store.record(handle).and_then(|record| record.node))
  {
    let _ = dispatch_pointer_move::<Tui>(node, &pointer_event(x, y, origin, button, modifiers));
  }

  if button == Some(PointerButton::Primary) {
    drag_to(x, y);
  }
}

/// Dispatches a press at (`x`, `y`): the hover path follows it, the press reaches the
/// deepest node, and—unless a handler prevented the platform—the left press focuses,
/// records the path for its release, places an entry's caret or anchors a selection
/// and arms a link, or the right press opens the context menu.
// One press has these stages; length is the stage count, not complexity.
#[expect(clippy::too_many_lines)]
pub(crate) fn pointer_press(x: u16, y: u16, button: PointerButton, modifiers: Modifiers) {
  if !store(|store| store.mouse) {
    return;
  }

  set_hover(x, y);

  // A press on an enabled item of the open menu waits for its release there; a press
  // on a disabled item holds the menu open, and anywhere else closes it. Either way
  // the press is consumed.
  let consumed = store(|store| {
    if store.menu.is_none() {
      return false;
    }

    match store.menu_item_at(x, y) {
      Some((item, true)) if button == PointerButton::Primary => store.pressed_item = Some(item),
      Some(_) => {}
      None => store.close_menu(),
    }

    true
  });

  if consumed {
    return;
  }

  // A press on a scrollbar grabs it and consumes the press.
  let grabbed = store(|store| {
    if button != PointerButton::Primary {
      return None;
    }

    let (handle, axis, position) = store.scrollbar_hit(x, y)?;
    let (_, track, limit) = store.scroll_geometry(handle, axis);
    let offset = match axis {
      Axis::Horizontal => store.scroll_offset(handle).0,
      Axis::Vertical => store.scroll_offset(handle).1,
    };
    let (start, thumb) =
      crate::paint::thumb_span(isize::try_from(track).unwrap_or(0), offset, limit);
    let (start, thumb) = (cells(start as i64), cells(thumb as i64));
    let grab = if position >= start && position < start + thumb {
      position - start
    }
    else {
      thumb / 2.
    };
    let offsets = store.thumb_offsets(handle, axis, track, limit, position - grab);

    store.drag = Some(Drag::Scrollbar {
      node: handle,
      axis,
      grab,
    });

    Some((handle, offsets))
  });

  if let Some((handle, offsets)) = grabbed {
    scroll_to(handle, offsets);

    return;
  }

  let (path, dismissed, modal_blocked) = store(|store| {
    // A left press starts over; a right press keeps the selection for the menu.
    if button == PointerButton::Primary {
      store.clear_selection();
    }

    let path = store.hit_test(x, y);
    let mut dismissed = None;
    let mut blocked = false;

    // A press on the anchor a popup opened from leaves it to the anchor's click.
    if let Some((top, modal)) = store.topmost_open_overlay() {
      let mut overlay_path = Vec::new();
      let on_anchor = !modal
        && store
          .record(top)
          .and_then(Record::overlay_anchor)
          .is_some_and(|anchor| path.iter().any(|(handle, _)| *handle == anchor));

      if !store.hit_overlay(top, x, y, &mut overlay_path) && !on_anchor {
        dismissed = store.record(top).and_then(|record| record.node);
        blocked = modal;
      }
    }

    (path, dismissed, blocked)
  });

  if let Some(dismissed) = dismissed {
    notify_dismiss::<Tui>(dismissed);
  }

  if modal_blocked {
    return;
  }

  let Some((deepest, origin)) = path.last().copied()
  else {
    if button == PointerButton::Primary {
      set_focus(None);
    }

    return;
  };
  let focus_target = store(|store| {
    path
      .iter()
      .rev()
      .find_map(|(handle, _)| store.focus_holder(*handle))
      .or_else(|| {
        // A press inside an overlay with no focusable of its own keeps focus on the
        // overlay's nearest focusable ancestor, so a popup's rows never blur the
        // control that opened it.
        let (first, _) = path.first()?;

        if !store.record(*first).is_some_and(Record::is_overlay) {
          return None;
        }

        store
          .ancestors(*first)
          .into_iter()
          .find(|node| store.record(*node).is_some_and(Record::is_focusable))
      })
  });

  // A right press on an entry focuses it too, so its edit menu is the focused entry's.
  let entry = focus_target.filter(|target| store(|store| store.is_field(*target)));
  let entry_held_focus = entry.is_some_and(|entry| store(|store| store.focused == Some(entry)));

  if button == PointerButton::Primary || entry.is_some() {
    set_focus(focus_target);
  }

  let node = store(|store| store.record(deepest).and_then(|record| record.node));
  let prevented = node.map_or(Prevented::default(), |node| {
    dispatch_pointer_down::<Tui>(node, &pointer_event(x, y, origin, Some(button), modifiers))
  });

  store(|store| {
    if button == PointerButton::Primary {
      store.press = Some(Press {
        path: path.iter().map(|(handle, _)| *handle).collect(),
        captured: deepest,
        target: focus_target,
      });

      if let Some(target) = focus_target {
        store.damage(target);
      }
    }
  });

  if prevented.platform {
    return;
  }

  let entry_local = entry.map(|entry| {
    let origin = path
      .iter()
      .find(|(handle, _)| *handle == entry)
      .map_or((0, 0), |(_, origin)| *origin);

    (entry, i64::from(x) - origin.0, i64::from(y) - origin.1)
  });

  match button {
    PointerButton::Primary => {
      store(|store| {
        // A press on an entry places its caret—`Shift` extending the selection to
        // it—and starts the drag that extends it: a double click takes the word, a
        // triple click the line.
        if let Some((entry, local_x, local_y)) = entry_local {
          match store.register_press(x, y) {
            2 => store.select_entry_word(entry),
            3 => store.select_entry_line(entry),
            _ => store.place_caret(entry, local_x, local_y, modifiers.shift),
          }
        }
        // A press on a selectable run starts a selection: a drag extends it, a double
        // click takes the word, a triple click the whole run.
        else if let Some(point) = store.text_position_at(x, y) {
          match store.register_press(x, y) {
            2 => store.select_word(point),
            3 => store.select_run(point),
            _ => store.anchor_selection(point),
          }
        }
        else {
          store.last_press = None;
        }

        // A press on a link arms it; the release on the same cell opens it.
        store.pressed_link = store.link_at(x, y).map(|url| (x, y, url));
      });
    }
    PointerButton::Secondary => store(|store| {
      // A right press outside the selection of the entry holding focus places its
      // caret there first; one focusing the entry places it regardless.
      if let Some((entry, local_x, local_y)) = entry_local
        && !(entry_held_focus && store.entry_selection_contains(entry, local_x, local_y))
      {
        store.place_caret(entry, local_x, local_y, false);
        store.drag = None;
      }

      store.right_press(x, y, entry);
    }),
    PointerButton::Auxiliary => {}
  }
}

/// Dispatches a release at (`x`, `y`): the release reaches the captured node, then the
/// click completes on the deepest node under both the press and the release—releasing
/// over a different target clicks nothing, as on the web.
pub(crate) fn pointer_release(x: u16, y: u16, button: PointerButton, modifiers: Modifiers) {
  if !store(|store| store.mouse) {
    return;
  }

  // A release on the item its press landed on runs the item; the menu closes either
  // way, and the release is consumed.
  let (menu_pressed, item) = store(|store| {
    let Some(pressed) = store.pressed_item.take()
    else {
      return (false, None);
    };
    let item = store
      .menu_item_at(x, y)
      .filter(|(item, _)| *item == pressed)
      .map(|(item, _)| item);

    match item {
      Some(MenuItem::Copy) => store.copy_selection(),
      Some(MenuItem::Paste) => store.request_paste(),
      Some(MenuItem::Cut) | None => {}
    }

    store.close_menu();

    (true, item)
  });

  if item == Some(MenuItem::Cut) {
    crate::field::cut_focused();
  }

  if menu_pressed {
    return;
  }

  let target = store(|store| {
    store
      .press
      .as_ref()
      .map(|press| (press.captured, store.screen_origin(press.captured)))
      .or_else(|| pointer_target(store, x, y))
  });

  if let Some((handle, origin)) = target
    && let Some(node) = store(|store| store.record(handle).and_then(|record| record.node))
  {
    let _ = dispatch_pointer_up::<Tui>(node, &pointer_event(x, y, origin, Some(button), modifiers));
  }

  if button != PointerButton::Primary {
    return;
  }

  let (clicked, pressed_link) = store(|store| {
    store.drag = None;

    let Some(press) = store.press.take()
    else {
      return (None, None);
    };

    if let Some(target) = press.target {
      store.damage(target);
    }

    let released: Vec<Handle> = store.hit_test(x, y).iter().map(|(node, _)| *node).collect();
    let common = press
      .path
      .iter()
      .zip(&released)
      .take_while(|(press, release)| press == release)
      .map(|(node, _)| *node)
      .last();
    let pressed_link = store.pressed_link.take();

    (
      common.and_then(|handle| store.record(handle).and_then(|record| record.node)),
      pressed_link,
    )
  });

  let prevented = clicked.map_or(Prevented::default(), dispatch_click::<Tui>);

  if prevented.platform {
    return;
  }

  // A click on a link—press and release on one cell, so a selection drag never
  // opens—stages it for the terminal loop's system handler.
  if let Some((press_x, press_y, url)) = pressed_link
    && (press_x, press_y) == (x, y)
  {
    store(|store| store.pending_open = Some(url));
  }
}

/// Follows the drag in progress to the pointer at (`x`, `y`), if one is under way: a
/// scrollbar's thumb follows the pointer's position within the cell.
pub(crate) fn drag_to(x: u16, y: u16) {
  let scrolled = store(|store| match store.drag {
    Some(Drag::Scrollbar { node, axis, grab }) => {
      let (track_start, track, limit) = store.scroll_geometry(node, axis);
      let (within_x, within_y) = store.pointer_within;
      let along = match axis {
        Axis::Horizontal => cells(i64::from(x) - track_start) + within_x,
        Axis::Vertical => cells(i64::from(y) - track_start) + within_y,
      };

      Some((
        node,
        store.thumb_offsets(node, axis, track, limit, along - grab),
      ))
    }
    Some(Drag::Selection) => {
      store.extend_selection(x, y);

      None
    }
    Some(Drag::Caret(handle)) => {
      store.drag_caret(handle, x, y);

      None
    }
    None => None,
  });

  if let Some((handle, offsets)) = scrolled {
    scroll_to(handle, offsets);
  }
}

/// A count of cells as a position along a track.
#[expect(clippy::cast_precision_loss)]
const fn cells(count: i64) -> f32 {
  count as f32
}

/// Writes `offsets` into `handle` and notifies its scroll handlers.
pub(crate) fn scroll_to(handle: Handle, offsets: (u32, u32)) {
  let node = store(|store| {
    if store.scroll_offset(handle) == offsets {
      return None;
    }

    store.write_scroll(handle, offsets)
  });

  if let Some(node) = node {
    #[expect(clippy::cast_precision_loss)]
    notify_scroll::<Tui>(
      node,
      &ScrollEvent {
        x: offsets.0 as f32,
        y: offsets.1 as f32,
      },
    );
  }
}

/// Scrolls `handle` by a delta, clamped to its limits; `true` when it moved. A
/// single-line field scrolls its window along its value by the horizontal delta.
pub(crate) fn scroll_by(handle: Handle, delta_x: i64, delta_y: i64) -> bool {
  if let Some(moved) = store(|store| store.scroll_field_by(handle, delta_x)) {
    return moved;
  }

  let Some(offsets) = store(|store| store.scrolled_by(handle, delta_x, delta_y))
  else {
    return false;
  };

  scroll_to(handle, offsets);

  true
}

/// Scrolls under the cell (`x`, `y`): a gesture in progress keeps its latched box,
/// consuming the wheel at that box's limits, and a fresh gesture targets the innermost
/// box under the cell that can still move in the given direction.
pub(crate) fn wheel(x: u16, y: u16, delta_x: i64, delta_y: i64) {
  if !store(|store| store.mouse) {
    return;
  }

  let now = std::time::Instant::now();
  let (latched, candidates) = store(|store| {
    store.close_menu();

    let latched = match store.scroll_latch.as_mut() {
      Some(entry)
        if entry.cell == (x, y)
          && now.duration_since(entry.last) <= crate::store::SCROLL_GESTURE =>
      {
        entry.last = now;

        Some(entry.target)
      }
      _ => None,
    };
    let candidates: Vec<Handle> = store
      .hit_test(x, y)
      .iter()
      .rev()
      .map(|(node, _)| *node)
      .collect();

    (latched, candidates)
  });

  if let Some(target) = latched {
    let _ = scroll_by(target, delta_x, delta_y);

    return;
  }

  for handle in candidates {
    if scroll_by(handle, delta_x, delta_y) {
      store(|store| {
        store.scroll_latch = Some(ScrollLatch {
          cell: (x, y),
          target: handle,
          last: now,
        });
      });

      return;
    }
  }
}

/// Scrolls the nearest scrollable box around the focus (or the first in tree order)
/// by `pages` viewport heights.
pub(crate) fn page_scroll(pages: i64) {
  let target = store(|store| {
    let target = store
      .focused
      .and_then(|focused| {
        store
          .ancestors(focused)
          .into_iter()
          .find(|node| store.is_scrollable(*node))
      })
      .or_else(|| {
        let mut stack = vec![store.root];

        while let Some(current) = stack.pop() {
          if store.is_scrollable(current) {
            return Some(current);
          }

          stack.extend(store.children(current).iter().rev());
        }

        None
      })?;
    let (_, height) = store.node_size(target);

    Some((target, height.max(1)))
  });

  if let Some((handle, height)) = target {
    let _ = scroll_by(handle, 0, pages * height);
  }
}

/// Dispatches a key at the focused node—or the root when nothing holds focus—then runs
/// the terminal's own response unless a handler prevented it: focus movement,
/// dismissal, page scrolling, the clipboard keys and entry editing.
pub(crate) fn key(event: &KeyEvent) {
  let target = store(|store| {
    let handle = store.focused.unwrap_or(store.root);

    store.record(handle).and_then(|record| record.node)
  });
  let prevented = target.map_or(Prevented::default(), |node| {
    dispatch_key_down::<Tui>(node, event)
  });

  // The handlers' writes have taken effect by the time the default runs.
  skene_reactive::Runtime::flush_sync();

  if prevented.platform {
    return;
  }

  key_default(event);
}

/// The terminal's own response to a key.
fn key_default(event: &KeyEvent) {
  let clipboard = store(|store| store.clipboard);
  let ctrl = event.modifiers.ctrl && !event.modifiers.alt;

  match event.key.as_str() {
    "Tab" => {
      move_focus(if event.modifiers.shift { -1 } else { 1 });

      return;
    }
    "Escape" => {
      // The most transient layer goes first: the menu, then the selection, then an
      // open overlay.
      let dismissed = store(|store| {
        if store.menu.is_some() {
          store.close_menu();

          return None;
        }

        if store.has_selection() {
          store.clear_selection();

          if let Some(entry) = store.focused_entry() {
            store.collapse_entry_selection(entry);
          }

          return None;
        }

        let (top, _) = store.topmost_open_overlay()?;

        store.record(top).and_then(|record| record.node)
      });

      if let Some(dismissed) = dismissed {
        notify_dismiss::<Tui>(dismissed);
      }

      return;
    }
    "PageDown" => page_scroll(1),
    "PageUp" => page_scroll(-1),
    // The clipboard keys the launch offers: `Ctrl+C` copies whichever selection
    // exists, `Ctrl+X` cuts the focused entry's, `Ctrl+V` pastes into the focused
    // entry; one not offered is an ordinary key.
    "c" | "C" if ctrl && clipboard.copy => {
      store(Store::copy_selection);

      return;
    }
    "x" | "X" if ctrl && clipboard.copy => {
      crate::field::cut_focused();

      return;
    }
    "v" | "V" if ctrl && clipboard.paste => {
      store(|store| {
        if store.focused_entry().is_some() {
          store.request_paste();
        }
      });

      return;
    }
    _ => {}
  }

  crate::field::edit_focused(event);
}

/// Inserts `text`, pasted by the terminal, into the focused entry.
pub(crate) fn paste(text: &str) {
  crate::field::paste_focused(text);
}

/// Inserts `text`, the terminal's answer to the last clipboard read, into the entry
/// that asked for it.
pub(crate) fn paste_reply(text: &str) {
  crate::field::paste_requested(text);
}
