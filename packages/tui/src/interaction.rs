//! The terminal's interaction: hit testing, hover and focus, presses and clicks,
//! wheel scrolling and scrollbar drags, each routed through core's dispatch.
//!
//! No borrow of the store is held while a handler runs: every entry point reads what
//! it needs, releases the store, dispatches, then writes the outcome back.

use skene_core::{
  Axis,
  KeyEvent,
  Modifiers,
  MouseCursor,
  Overflow,
  Placement,
  PointerButton,
  PointerEvent,
  Prevented,
  ScrollEvent,
  next::{
    ControlKind,
    NodeId,
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
  },
};

use crate::{
  Tui,
  paint::control_kind,
  store::{Drag, Handle, Kind, Press, Record, ScrollLatch, Store},
  with_store,
};

/// The rows or columns one wheel report scrolls: a browser's three lines per notch.
pub(crate) const WHEEL_STEP: i64 = 3;

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

    if crate::layout::is_paragraph(self, handle) {
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
        let (location_x, location_y) = self.node_location(visited);

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
        let (location_x, location_y) = self.node_location(visited);

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
    if matches!(self.drag, Some(Drag::Scrollbar { .. })) {
      return Some(MouseCursor::Grabbing);
    }

    let (pointer_x, pointer_y) = self.last_pointer;

    if self.link_at(pointer_x, pointer_y).is_some() {
      return Some(MouseCursor::Click);
    }

    self.hovered.iter().rev().find_map(|handle| {
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

  /// A node's resolved overflow, `Visible` when unset or the node has no box style.
  fn overflow(&self, handle: Handle) -> Overflow {
    self
      .record(handle)
      .and_then(|record| record.kind.box_style())
      .and_then(|style| style.overflow)
      .unwrap_or(Overflow::Visible)
  }

  /// Whether the node's content scrolls.
  pub(crate) fn is_scrollable(&self, handle: Handle) -> bool {
    matches!(self.overflow(handle), Overflow::Scroll | Overflow::Auto)
  }

  /// Whether the node clips its children when painting.
  pub(crate) fn clips(&self, handle: Handle) -> bool {
    !matches!(self.overflow(handle), Overflow::Visible)
  }

  /// A node's scroll offsets, clamped to what the last layout allows.
  pub(crate) fn scroll_offset(&self, handle: Handle) -> (u32, u32) {
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

  /// A node's maximum scroll offsets, from the last computed layout.
  pub(crate) fn scroll_limits(&self, handle: Handle) -> (u32, u32) {
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

  /// Stores a node's offsets and repaints; the scroll notification is the caller's.
  fn write_scroll(&mut self, handle: Handle, offsets: (u32, u32)) -> Option<NodeId> {
    self.paint_dirty = true;

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
  /// pointer's position within the track.
  fn scrollbar_hit(&self, x: u16, y: u16) -> Option<(Handle, Axis, i64)> {
    let (px, py) = (i64::from(x), i64::from(y));

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
          return Some((handle, Axis::Vertical, py - track_start));
        }
      }

      if limit_x > 0 && py == origin_y + height - 1 {
        let (track_start, track, _) = self.scroll_geometry(handle, Axis::Horizontal);

        if px >= track_start && px < track_start + track {
          return Some((handle, Axis::Horizontal, px - track_start));
        }
      }
    }

    None
  }

  /// The offsets that land the thumb's start on `target_start` within its track,
  /// clamped.
  fn thumb_offsets(
    &self,
    handle: Handle,
    axis: Axis,
    track: i64,
    limit: u32,
    target_start: i64,
  ) -> (u32, u32) {
    let (_, thumb) = crate::paint::thumb_span(isize::try_from(track).unwrap_or(0), 0, limit);
    let thumb = thumb as i64;
    let room = (track - thumb).max(1);
    let target_start = target_start.clamp(0, track - thumb);
    let offset = u32::try_from(target_start * i64::from(limit) / room).unwrap_or(0);
    let (x, y) = self.scroll_offset(handle);

    match axis {
      Axis::Horizontal => (offset, y),
      Axis::Vertical => (x, offset),
    }
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
/// newly focused node is scrolled into view at the next render, and a newly focused
/// entry's caret moves to the value's end.
pub(crate) fn set_focus(next: Option<Handle>) {
  let (previous, gained) = store(|store| {
    let previous = std::mem::replace(&mut store.focused, next);

    if previous == next {
      return (None, None);
    }

    store.paint_dirty = true;

    if let Some(next) = next {
      if let Some(field) = store
        .record_mut(next)
        .and_then(|record| record.kind.field_mut())
      {
        field.caret = field.value.chars().count();
      }

      store.reveal = Some(next);
    }

    (
      previous.and_then(|previous| store.record(previous).and_then(|record| record.node)),
      next.and_then(|next| store.record(next).and_then(|record| record.node)),
    )
  });

  if let Some(previous) = previous {
    notify_blur::<Tui>(previous);
  }

  if let Some(gained) = gained {
    notify_focus::<Tui>(gained);
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
    let previous = std::mem::take(&mut store.hovered);

    if std::mem::replace(&mut store.last_pointer, (x, y)) != (x, y) {
      // A real pointer move releases the wheel's latched target.
      store.scroll_latch = None;

      if store.menu.is_some() {
        store.paint_dirty = true;
      }
    }

    if previous == next {
      store.hovered = next;

      return (Vec::new(), Vec::new());
    }

    let node_of =
      |store: &Store, handle: Handle| store.record(handle).and_then(|record| record.node);
    let left: Vec<NodeId> = previous
      .iter()
      .rev()
      .filter(|node| !next.contains(node))
      .filter_map(|handle| node_of(store, *handle))
      .collect();
    let entered: Vec<NodeId> = next
      .iter()
      .filter(|node| !previous.contains(node))
      .filter_map(|handle| node_of(store, *handle))
      .collect();

    store.hovered = next;
    store.paint_dirty = true;

    (left, entered)
  });

  for node in left {
    notify_pointer_leave::<Tui>(node);
  }

  for node in entered {
    notify_pointer_enter::<Tui>(node);
  }
}

/// The portable pointer event at (`x`, `y`), local to the node at `origin`.
fn pointer_event(
  x: u16,
  y: u16,
  origin: (i64, i64),
  button: Option<PointerButton>,
  modifiers: Modifiers,
) -> PointerEvent {
  #[expect(clippy::cast_precision_loss)]
  PointerEvent {
    x: f32::from(x),
    y: f32::from(y),
    local_x: (i64::from(x) - origin.0) as f32,
    local_y: (i64::from(y) - origin.1) as f32,
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
/// records the path for its release, anchors a selection and arms a link, or the right
/// press opens the copy menu.
// One press has these stages; length is the stage count, not complexity.
#[expect(clippy::too_many_lines)]
pub(crate) fn pointer_press(x: u16, y: u16, button: PointerButton, modifiers: Modifiers) {
  set_hover(x, y);

  // A press on the open copy menu waits for its release on `Copy`; anywhere else it
  // just closes the menu. Either way the press is consumed.
  let consumed = store(|store| {
    if store.menu.is_none() {
      return false;
    }

    if button == PointerButton::Primary && store.copy_hit(x, y) {
      store.pressed_copy = store.clipboard;
    }
    else {
      store.close_menu();
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
    let (start, thumb) = (start as i64, thumb as i64);
    let grab = if position >= start && position < start + thumb {
      position - start
    }
    else {
      thumb / 2
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
    store.clear_selection();

    let path = store.hit_test(x, y);
    let mut dismissed = None;
    let mut blocked = false;

    if let Some((top, modal)) = store.topmost_open_overlay() {
      let mut overlay_path = Vec::new();

      if !store.hit_overlay(top, x, y, &mut overlay_path) {
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

  if button == PointerButton::Primary {
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
    }
  });

  if prevented.platform {
    return;
  }

  match button {
    PointerButton::Primary => {
      store(|store| {
        // A press on an entry places its caret.
        if let Some(target) = focus_target
          && store.is_field(target)
        {
          let origin = path
            .iter()
            .find(|(handle, _)| *handle == target)
            .map_or((0, 0), |(_, origin)| *origin);

          store.place_caret(target, i64::from(x) - origin.0, i64::from(y) - origin.1);
        }

        // A press on a selectable run starts a selection: a drag extends it, a double
        // click takes the word, a triple click the whole run.
        if let Some(point) = store.text_position_at(x, y) {
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
    PointerButton::Secondary => store(|store| store.right_press(x, y)),
    PointerButton::Auxiliary => {}
  }
}

/// Dispatches a release at (`x`, `y`): the release reaches the captured node, then the
/// click completes on the deepest node under both the press and the release—releasing
/// over a different target clicks nothing, as on the web.
pub(crate) fn pointer_release(x: u16, y: u16, button: PointerButton, modifiers: Modifiers) {
  let copied = store(|store| {
    if !std::mem::take(&mut store.pressed_copy) {
      return false;
    }

    if store.copy_hit(x, y) {
      store.copy_selection();
    }

    store.close_menu();

    true
  });

  if copied {
    return;
  }

  let target = store(|store| pointer_target(store, x, y));

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

/// Follows the drag in progress to the pointer at (`x`, `y`), if one is under way.
pub(crate) fn drag_to(x: u16, y: u16) {
  let scrolled = store(|store| match store.drag {
    Some(Drag::Scrollbar { node, axis, grab }) => {
      let (track_start, track, limit) = store.scroll_geometry(node, axis);
      let along = match axis {
        Axis::Horizontal => i64::from(x),
        Axis::Vertical => i64::from(y),
      };

      Some((
        node,
        store.thumb_offsets(node, axis, track, limit, along - track_start - grab),
      ))
    }
    Some(Drag::Selection) => {
      store.extend_selection(x, y);

      None
    }
    None => None,
  });

  if let Some((handle, offsets)) = scrolled {
    scroll_to(handle, offsets);
  }
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

/// Scrolls `handle` by a delta, clamped to its limits; `true` when it moved.
pub(crate) fn scroll_by(handle: Handle, delta_x: i64, delta_y: i64) -> bool {
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
/// dismissal, page scrolling and entry editing.
pub(crate) fn key(event: &KeyEvent) {
  let target = store(|store| {
    let handle = store.focused.unwrap_or(store.root);

    store.record(handle).and_then(|record| record.node)
  });
  let prevented = target.map_or(Prevented::default(), |node| {
    dispatch_key_down::<Tui>(node, event)
  });

  if prevented.platform {
    return;
  }

  key_default(event);
}

/// The terminal's own response to a key.
fn key_default(event: &KeyEvent) {
  match event.key.as_str() {
    "Tab" => {
      move_focus(if event.modifiers.shift { -1 } else { 1 });

      return;
    }
    "Escape" => {
      // The most transient layer goes first: the copy menu, then the selection, then
      // an open overlay.
      let dismissed = store(|store| {
        if store.menu.is_some() {
          store.close_menu();

          return None;
        }

        if store.selection.is_some() {
          store.clear_selection();

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
    _ => {}
  }

  crate::field::edit_focused(event);
}
