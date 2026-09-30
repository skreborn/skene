//! The terminal's interaction: hit testing, hover and focus, presses and clicks,
//! wheel scrolling and scrollbar drags, each routed through core's dispatch.
//!
//! No borrow of the store is held while a handler runs: every entry point reads what
//! it needs, releases the store, dispatches, then writes the outcome back.

use skene_core::{
  Axes,
  Axis,
  ClickEvent,
  ClipboardAction,
  KeyEvent,
  Modifiers,
  MouseCursor,
  NodeId,
  Overflow,
  Placement,
  PointerButton,
  PointerButtons,
  PointerEvent,
  PointerInfo,
  PointerType,
  Prevented,
  Scene,
  ScrollEvent,
  Visibility,
  WheelUnit,
  dispatch_click,
  dispatch_context_menu,
  dispatch_double_click,
  dispatch_key_down,
  dispatch_key_up,
  dispatch_pointer_cancel,
  dispatch_pointer_down,
  dispatch_pointer_move,
  dispatch_pointer_up,
  dispatch_wheel,
  drop_drag,
  end_drag,
  move_drag,
  notify_dismiss,
  notify_focus_moved,
  notify_pointer_moved,
  notify_scroll,
  start_drag,
};

use crate::{
  Tui,
  layout::{
    enclosing_paragraph,
    inline_box_cells,
    is_inline_box,
    is_inline_run,
    is_paragraph,
    paragraph_cluster_at,
    paragraph_run_at,
  },
  paint::control_entry,
  store::{Drag, Handle, Kind, Press, Record, ScrollLatch, Store},
  with_store,
};

/// The rows or columns one wheel report scrolls: a browser's three lines per notch.
pub(crate) const WHEEL_STEP: i64 = 3;

/// The focus move an overlay's opening asks for.
pub(crate) enum OverlayFocus {
  /// Focus stays where it is.
  Held,
  /// Focus moves to the node, or leaves every node.
  Moved(Option<Handle>),
}

impl Store {
  /// Every open overlay reachable from the root, in the order they opened; later
  /// entries stack on top.
  pub(crate) fn open_overlays(&self) -> Vec<Handle> {
    Scene::<Tui>::with(|scene| scene.open_overlays().to_vec())
      .into_iter()
      .filter(|overlay| self.is_reachable(*overlay))
      .collect()
  }

  /// Whether `handle` sits inside an open overlay.
  pub(crate) fn inside_open_overlay(&self, handle: Option<Handle>) -> bool {
    handle.is_some_and(|handle| {
      let ancestors = self.ancestors(handle);

      self
        .open_overlays()
        .iter()
        .any(|overlay| ancestors.contains(overlay))
    })
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

  /// The topmost open modal overlay.
  pub(crate) fn topmost_modal(&self) -> Option<Handle> {
    self
      .open_overlays()
      .into_iter()
      .rev()
      .find(|overlay| self.record(*overlay).is_some_and(Record::is_modal_overlay))
  }

  /// Whether `handle` is reachable while the topmost modal is open: inside it, or inside
  /// an overlay opened above it; everything is while no modal is open.
  pub(crate) fn reachable_under_modal(&self, handle: Handle) -> bool {
    let overlays = self.open_overlays();
    let Some(modal) = overlays
      .iter()
      .rposition(|overlay| self.record(*overlay).is_some_and(Record::is_modal_overlay))
    else {
      return true;
    };
    let ancestors = self.ancestors(handle);

    overlays[modal..]
      .iter()
      .any(|overlay| *overlay == handle || ancestors.contains(overlay))
  }

  /// The tab stops in traversal order, as the scene orders them: positioned ones
  /// first by position, then the rest in document order; closed overlays and hidden
  /// boxes excluded, and an open modal containing the whole order with the overlays
  /// opened after it.
  pub(crate) fn tab_order(&self) -> Vec<Handle> {
    self.tab_stops()
  }

  /// The tab stops' nodes in traversal order, as [`tab_order`](Self::tab_order) orders
  /// them.
  fn tab_stops(&self) -> Vec<NodeId> {
    let Some(root) = crate::platform::root_node()
    else {
      return Vec::new();
    };
    let overlays: Vec<NodeId> = self
      .open_overlays()
      .into_iter()
      .filter_map(|overlay| self.record(overlay)?.node)
      .collect();

    Scene::<Tui>::with(|scene| {
      scene.tab_order(root, &overlays, |node| {
        scene.contains(node) && self.is_hidden(node)
      })
    })
  }

  /// The deepest node under the cell (`x`, `y`) and its ancestors, the root first: the
  /// path hover and a press hold along.
  pub(crate) fn pointer_path(&self, x: u16, y: u16) -> Vec<Handle> {
    self
      .hit_test(x, y)
      .last()
      .map(|(deepest, _)| self.ancestors(*deepest).into_iter().rev().collect())
      .unwrap_or_default()
  }

  /// The path from the root to the deepest node containing the cell (`x`, `y`), each
  /// with its absolute origin, against the last computed layout.
  pub(crate) fn hit_test(&self, x: u16, y: u16) -> Vec<(Handle, (i64, i64))> {
    let modal = self.topmost_modal();

    // Overlays hit first, topmost first; below the topmost open modal nothing is
    // hittable.
    for overlay in self.open_overlays().iter().rev() {
      let mut path = Vec::new();

      if self.hit_overlay(*overlay, x, y, &mut path) {
        return path;
      }

      if modal == Some(*overlay) {
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
    if !record.kind.is_laid_out() {
      for child in self.children_reversed(handle) {
        if self.hit_walk(child, offset_x, offset_y, x, y, path) {
          return true;
        }
      }

      return false;
    }

    if self.is_hidden(handle) {
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
      self.hit_paragraph(handle, (origin_x, origin_y), x, y, path);

      return true;
    }

    // A scrolled box shifts its children by its offsets.
    let (scroll_x, scroll_y) = self.scroll_offset(handle);
    let (child_x, child_y) = (
      origin_x - i64::from(scroll_x),
      origin_y - i64::from(scroll_y),
    );

    // Later children paint on top, so they win the hit; overlays hit separately.
    for child in self.children_reversed(handle) {
      if self.record(child).is_some_and(Record::is_overlay) {
        continue;
      }

      if self.hit_walk(child, child_x, child_y, x, y, path) {
        break;
      }
    }

    true
  }

  /// Hits the runs of the paragraph `handle`, at `origin`, under the point: the runs
  /// holding the cluster drawn there join the path from the outermost group to the leaf,
  /// each at the paragraph's origin, and an inline box hits through its own layout.
  fn hit_paragraph(
    &self,
    handle: Handle,
    origin: (i64, i64),
    x: i64,
    y: i64,
    path: &mut Vec<(Handle, (i64, i64))>,
  ) {
    let (Ok(column), Ok(row)) = (usize::try_from(x - origin.0), usize::try_from(y - origin.1))
    else {
      return;
    };
    let Some(leaf) = paragraph_cluster_at(self, handle, column, row)
      .and_then(|offset| paragraph_run_at(self, handle, offset))
    else {
      return;
    };
    let mut chain: Vec<Handle> = self
      .ancestors(leaf)
      .into_iter()
      .skip(1)
      .take_while(|ancestor| *ancestor != handle)
      .filter(|ancestor| {
        matches!(
          self.record(*ancestor).map(|record| &record.kind),
          Some(Kind::Run { .. })
        )
      })
      .collect();

    chain.reverse();
    path.extend(chain.into_iter().map(|run| (run, origin)));

    if !is_inline_box(self, leaf) {
      path.push((leaf, origin));

      return;
    }

    if let Some((_, (cell_x, cell_y))) = inline_box_cells(self, handle)
      .into_iter()
      .find(|(run, _)| *run == leaf)
    {
      let (location_x, location_y) = self.node_location(leaf);

      self.hit_walk(
        leaf,
        origin.0 + cell_x - location_x,
        origin.1 + cell_y - location_y,
        x,
        y,
        path,
      );
    }
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

  /// A node's position within its layout parent: its laid-out location, or for a run
  /// flowing in a paragraph, through any groups, the cell an inline box's placeholder
  /// flowed into and the paragraph's origin for any other.
  fn inline_location(&self, handle: Handle) -> (i64, i64) {
    let paragraph = is_inline_run(self, handle)
      .then(|| enclosing_paragraph(self, handle))
      .flatten();

    match paragraph {
      Some(paragraph) => inline_box_cells(self, paragraph)
        .into_iter()
        .find(|(run, _)| *run == handle)
        .map_or((0, 0), |(_, cell)| cell),
      None => self.node_location(handle),
    }
  }

  /// A node's on-screen origin: its layout locations summed up the parent chain, each
  /// ancestor's scroll offsets subtracted, content inside an overlay positioned against
  /// the overlay's origin.
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
      placement,
      anchor,
      position,
      ..
    }) = self.record(handle).map(|record| &record.kind)
    else {
      return (0, 0);
    };
    let (placement, anchor, position) = (*placement, *anchor, *position);
    let (width, height) = self.node_size(handle);

    // A pinned overlay sits at its cell, shifted back into the screen.
    if let Some((x, y)) = position {
      return (x.min(columns - width).max(0), y.min(rows - height).max(0));
    }

    if let Some(anchor) = anchor.filter(|_| matches!(placement, Placement::Top | Placement::Bottom))
    {
      let (anchor_x, anchor_y) = self.screen_origin(anchor);
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
    if !self.mouse_taken() {
      return None;
    }

    if matches!(
      self.drag,
      Some(Drag::Scrollbar { .. } | Drag::Item { started: true, .. })
    ) || self.drags_control()
    {
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

      if self.drags_as_control(*handle) {
        return Some(MouseCursor::Grab);
      }

      match control_entry(self, *handle) {
        Some(true) => return Some(MouseCursor::Text),
        Some(false) => return Some(MouseCursor::Click),
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

  /// Whether a held left press landed on a control that follows the pointer until the
  /// release.
  fn drags_control(&self) -> bool {
    self.buttons.contains(PointerButton::Primary)
      && self
        .press
        .as_ref()
        .and_then(|press| press.target)
        .is_some_and(|target| self.drags_as_control(target))
  }

  /// A node's overflow per axis as CSS computes it, `Visible` when unset or the node has
  /// no box style.
  fn overflow(&self, handle: Handle) -> Axes<Overflow> {
    self
      .record(handle)
      .and_then(|record| record.kind.box_style())
      .and_then(|style| style.overflow)
      .map_or(Axes::all(Overflow::Visible), Axes::computed)
  }

  /// Whether the node's box is hidden: it and its content draw nothing and take neither
  /// the pointer nor focus.
  pub(crate) fn is_hidden(&self, handle: Handle) -> bool {
    self
      .record(handle)
      .and_then(|record| record.kind.box_style())
      .and_then(|style| style.visibility)
      == Some(Visibility::Hidden)
  }

  /// Whether the node's content scrolls: a box overflowing by its style on either axis,
  /// or an editor windowing its wrapped rows.
  pub(crate) fn is_scrollable(&self, handle: Handle) -> bool {
    self.is_editor(handle) || self.overflow(handle).scrolls()
  }

  /// Whether the node clips its children when painting.
  pub(crate) fn clips(&self, handle: Handle) -> bool {
    self.overflow(handle) != Axes::all(Overflow::Visible)
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

  /// A node's maximum scroll offsets, from the last computed layout and zero on an axis
  /// that does not scroll; an editor's is how far its window can scroll down its rows.
  pub(crate) fn scroll_limits(&self, handle: Handle) -> (u32, u32) {
    if let Some(limit) = self.editor_window_limit(handle) {
      return (0, limit);
    }

    let Some(taffy) = self.record(handle).and_then(|record| record.taffy)
    else {
      return (0, 0);
    };
    let overflow = self.overflow(handle);

    match self.taffy.layout(taffy) {
      #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a scroll extent is rounded and clamped non-negative, a cell count within u32"
      )]
      Ok(layout) => (
        if overflow.horizontal.scrolls() {
          layout.scroll_width().round().max(0.) as u32
        }
        else {
          0
        },
        if overflow.vertical.scrolls() {
          layout.scroll_height().round().max(0.) as u32
        }
        else {
          0
        },
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
    let (origin_x, origin_y) = self.screen_origin(handle);
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

    for (handle, _) in self.hit_test(x, y).into_iter().rev() {
      if !self.is_scrollable(handle) {
        continue;
      }

      let (limit_x, limit_y) = self.scroll_limits(handle);
      let (origin_x, origin_y) = self.screen_origin(handle);
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
    clippy::cast_sign_loss,
    reason = "a track's cells stay far below the integers f32 holds exactly, and the offset is rounded from a clamped, non-negative position"
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
  /// asks for: a modal opening takes focus to its first tab stop, or from every node
  /// when it has none. The scene returns focus as an overlay closes.
  pub(crate) fn set_overlay_open(
    &mut self,
    handle: Handle,
    open: bool,
    anchor: Option<Handle>,
  ) -> OverlayFocus {
    let Some(Kind::Overlay {
      open: held,
      anchor: bound,
      modal,
      ..
    }) = self.record_mut(handle).map(|record| &mut record.kind)
    else {
      return OverlayFocus::Held;
    };
    let was_open = std::mem::replace(held, open);
    let modal = *modal;

    *bound = anchor;
    self.layout_dirty = true;
    self.paint_dirty = true;

    if !open || was_open || !modal {
      return OverlayFocus::Held;
    }

    OverlayFocus::Moved(
      self
        .tab_order()
        .into_iter()
        .find(|stop| self.ancestors(*stop).contains(&handle)),
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
/// its caret at the end, and the entry focus left collapses its selection—unless focus
/// left it for an open popup, which keeps it for the popup to act on.
pub(crate) fn set_focus(next: Option<Handle>) {
  focus_on(next, false);
}

/// Moves focus to `next` as a script or a closing popup does: like [`set_focus`], but
/// the entry it reaches keeps its selection and caret as they were.
pub(crate) fn set_focus_restored(next: Option<Handle>) {
  focus_on(next, true);
}

fn focus_on(next: Option<Handle>, restored: bool) {
  let moved = store(|store| {
    let previous = store.focused();

    if previous == next {
      return None;
    }

    if let Some(previous) = previous {
      if !store.inside_open_overlay(next) {
        store.collapse_entry_selection(previous);
      }

      store.damage(previous);
    }

    if let Some(next) = next {
      if !restored
        && let Some(field) = store
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

    Some((
      next.and_then(|next| store.record(next).and_then(|record| record.node)),
      store.popups_left_by(next),
    ))
  });

  if let Some((gained, dismissed)) = moved {
    notify_focus_moved::<Tui>(gained);

    for popup in dismissed {
      notify_dismiss::<Tui>(popup);
    }
  }

  crate::field::report_selections();
}

/// Moves focus by `step` through the tab stops, wrapping at both ends; from a focused
/// node that is no tab stop (one disabled while it held focus), to the nearest stop
/// after it, or before it for a negative `step`, in document order.
pub(crate) fn move_focus(step: isize) {
  let next = store(|store| {
    let root = crate::platform::root_node()?;
    let order = store.tab_stops();
    let from = store
      .focused()
      .and_then(|focused| store.record(focused)?.node);

    Scene::<Tui>::with(|scene| scene.tab_from(root, &order, from, step))
  });

  if next.is_some() {
    set_focus(next);
  }
}

/// Updates the hover path to the deepest node under (`x`, `y`) and its ancestors, as a
/// browser's `:hover` holds on the ancestors of what the pointer is over, an overlay's
/// among them, notifying the nodes the pointer left and entered.
pub(crate) fn set_hover(x: u16, y: u16) {
  let deepest = store(|store| {
    let next = store.pointer_path(x, y);

    if std::mem::replace(&mut store.last_pointer, (x, y)) != (x, y) {
      // A real pointer move releases the wheel's latched target.
      store.scroll_latch = None;
    }

    let deepest = next.last().and_then(|handle| store.record(*handle)?.node);
    store.set_hovered(next);

    deepest
  });

  notify_pointer_moved::<Tui>(deepest);
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
  let ((within_x, within_y), buttons) = store(|store| (store.pointer_within, store.buttons));
  let (x, y) = (f32::from(x) + within_x, f32::from(y) + within_y);

  #[expect(
    clippy::cast_precision_loss,
    reason = "a cell offset stays far below the integers f32 holds exactly"
  )]
  PointerEvent {
    local_x: x - origin.0 as f32,
    local_y: y - origin.1 as f32,
    button,
    buttons,
    modifiers,
    pressure: if buttons.is_empty() { 0. } else { 0.5 },
    ..PointerEvent::mouse(x, y)
  }
}

/// The deepest node under (`x`, `y`) with its origin, or the node declaring drag axes
/// the held press captured.
fn pointer_target(store: &Store, x: u16, y: u16) -> Option<(Handle, (i64, i64))> {
  if let Some(press) = &store.press
    && store.buttons.contains(PointerButton::Primary)
    && let Some(captured) = press.captured
  {
    return Some((captured, store.screen_origin(captured)));
  }

  store.hit_test(x, y).last().copied()
}

/// Dispatches a pointer move at (`x`, `y`), the terminal reporting `dragging` held: the
/// hover path follows it, the move reaches the node declaring drag axes the press
/// captured, or else the node under the pointer, and a drag in progress follows.
pub(crate) fn pointer_move(x: u16, y: u16, dragging: Option<PointerButton>, modifiers: Modifiers) {
  if !store(|store| store.mouse_taken()) {
    return;
  }

  set_hover(x, y);

  let target = store(|store| pointer_target(store, x, y));

  if let Some((handle, origin)) = target
    && let Some(node) = store(|store| store.record(handle).and_then(|record| record.node))
  {
    let _ = dispatch_pointer_move::<Tui>(node, &pointer_event(x, y, origin, None, modifiers));
  }

  if dragging == Some(PointerButton::Primary) {
    drag_to(x, y);
  }
}

/// Dispatches a press at (`x`, `y`): the hover path follows it, the press reaches the
/// deepest node, and—unless a handler prevented the platform—the left press focuses,
/// records the path for its release, places an entry's caret or anchors a selection
/// and arms a link, and the right press places an entry's caret; a right press then
/// asks the deepest node for its context menu. An entry's selection the press moved is
/// reported.
pub(crate) fn pointer_press(x: u16, y: u16, button: PointerButton, modifiers: Modifiers) {
  press(x, y, button, modifiers);
  crate::field::report_selections();
}

/// The press of [`pointer_press`], before its report.
#[expect(
  clippy::too_many_lines,
  reason = "one press has these stages; length is the stage count, not complexity"
)]
fn press(x: u16, y: u16, button: PointerButton, modifiers: Modifiers) {
  store(|store| store.set_focus_visible(false));

  if !store(|store| store.mouse_taken()) {
    return;
  }

  set_hover(x, y);

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
    let path = store.hit_test(x, y);
    let deepest = path
      .last()
      .and_then(|(handle, _)| store.record(*handle)?.node);
    let overlays: Vec<NodeId> = store
      .open_overlays()
      .into_iter()
      .filter_map(|overlay| store.record(overlay)?.node)
      .collect();
    let dismissed = Scene::<Tui>::with(|scene| scene.dismissed_by_press(&overlays, deepest));
    let inside_overlay = store
      .topmost_open_overlay()
      .is_some_and(|(top, _)| store.hit_overlay(top, x, y, &mut Vec::new()));

    // A press outside the topmost modal, and the overlays above it, reaches nothing.
    let blocked = store.topmost_modal().is_some() && path.is_empty();

    // A left press starts over, unless it lands in an open overlay, whose items act on
    // the selection beneath; a right press keeps the selection for the context menu.
    if button == PointerButton::Primary && !inside_overlay {
      store.clear_selection();
    }

    (path, dismissed, blocked)
  });

  for overlay in dismissed {
    notify_dismiss::<Tui>(overlay);
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
  // A press on a link's cell focuses the linked run, as a click on an anchor does,
  // whether the run is hit itself or flows in a paragraph.
  let link = store(|store| store.link_at(x, y));
  let focus_target = store(|store| {
    link
      .as_ref()
      .map(|(run, _)| *run)
      .or_else(|| {
        path
          .iter()
          .rev()
          .find_map(|(handle, _)| store.focus_holder(*handle))
      })
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
  let entry_held_focus = entry.is_some_and(|entry| store(|store| store.focused() == Some(entry)));

  if button == PointerButton::Primary || entry.is_some() {
    set_focus(focus_target);
  }

  let node = store(|store| store.record(deepest).and_then(|record| record.node));
  let prevented = node.map_or(Prevented::default(), |node| {
    dispatch_pointer_down::<Tui>(node, &pointer_event(x, y, origin, Some(button), modifiers))
  });

  store(|store| {
    if button == PointerButton::Primary {
      let path = store.ancestors(deepest);
      let captured = path.iter().copied().find(|handle| store.drags(*handle));
      let count = store.register_press(x, y);

      store.press = Some(Press {
        path: path.into_iter().rev().collect(),
        pressed: deepest,
        captured,
        target: focus_target,
        count,
      });
      store.restyle_pending = true;

      Scene::<Tui>::with(|scene| scene.set_pressed(node));
    }
  });

  if !prevented.platform {
    press_default(
      x,
      y,
      button,
      modifiers,
      &path,
      entry,
      entry_held_focus,
      link,
    );
  }

  // A primary press under a node offering drag data arms a drag of it, which starts
  // once the pointer leaves the cell.
  if button == PointerButton::Primary
    && !prevented.platform
    && let Some(node) = node
    && Scene::<Tui>::with(|scene| scene.drag_source(node)).is_some()
  {
    store(|store| {
      store.drag = Some(Drag::Item {
        node,
        from: (x, y),
        started: false,
      });
    });
  }

  // The right press asks for the context menu once its default has placed the caret.
  if button == PointerButton::Secondary
    && let Some(node) = node
  {
    let _ =
      dispatch_context_menu::<Tui>(node, &pointer_event(x, y, origin, Some(button), modifiers));
  }
}

/// The terminal's own response to a press the handlers let stand: the caret placed,
/// a selection anchored or extended, a link armed.
#[expect(
  clippy::too_many_arguments,
  reason = "each argument is a separate fact of the press the default acts on"
)]
fn press_default(
  x: u16,
  y: u16,
  button: PointerButton,
  modifiers: Modifiers,
  path: &[(Handle, (i64, i64))],
  entry: Option<Handle>,
  entry_held_focus: bool,
  link: Option<(Handle, String)>,
) {
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
        let count = store.press.as_ref().map_or(1, |press| press.count);

        if let Some((entry, local_x, local_y)) = entry_local {
          match count {
            2 => store.select_entry_word(entry),
            3 => store.select_entry_line(entry),
            _ => store.place_caret(entry, local_x, local_y, modifiers.shift),
          }
        }
        // A press on a selectable run starts a selection: a drag extends it, a double
        // click takes the word, a triple click the whole run.
        else if let Some(point) = store.text_position_at(x, y) {
          match count {
            2 => store.select_word(point),
            3 => store.select_run(point),
            _ => store.anchor_selection(point),
          }
        }

        // A press on a link arms it; the release on the same cell opens it.
        store.pressed_link = link.map(|(_, url)| (x, y, url));
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
    }),
    _ => {}
  }
}

/// Dispatches a release at (`x`, `y`): the release reaches the node declaring drag axes
/// the press captured, or else the node under the pointer, then the click completes on
/// the deepest node under both the press and the release—releasing over a different
/// target clicks nothing, as on the web.
pub(crate) fn pointer_release(x: u16, y: u16, button: PointerButton, modifiers: Modifiers) {
  if !store(|store| store.mouse_taken()) {
    return;
  }

  // A release ends an item drag: one that started drops where it is, its press long
  // cancelled, and one that never did is an ordinary release.
  let dragged = store(|store| match store.drag {
    Some(Drag::Item { started, .. }) if button == PointerButton::Primary => {
      store.drag = None;

      started
    }
    _ => false,
  });

  if dragged {
    let _ = drop_drag::<Tui>();

    store(|store| store.restyle_pending = true);

    return;
  }

  let target = store(|store| {
    store
      .press
      .as_ref()
      .and_then(|press| press.captured)
      .map(|captured| (captured, store.screen_origin(captured)))
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

  let (clicked, pressed_link, count) = store(|store| {
    store.drag = None;

    let Some(press) = store.press.take()
    else {
      return (None, None, 0);
    };

    store.restyle_pending = true;
    Scene::<Tui>::with(|scene| scene.set_pressed(None));

    let released = store.pointer_path(x, y);
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
      press.count,
    )
  });
  let event = ClickEvent { modifiers, count };
  let prevented = clicked.map_or(Prevented::default(), |node| {
    let prevented = dispatch_click::<Tui>(node, &event);

    if count == 2 {
      let _ = dispatch_double_click::<Tui>(node, &event);
    }

    prevented
  });

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

/// Follows an item drag of what `node` offers to the cell (`x`, `y`): the drag starts
/// once the pointer leaves the press's cell `from`, the press cancelled at the node it
/// reached, and then moves over the node under the pointer.
fn drag_item(node: NodeId, from: (u16, u16), started: bool, x: u16, y: u16) {
  if !started {
    if (x, y) == from {
      return;
    }

    if start_drag::<Tui>(node).is_none() {
      store(|store| store.drag = None);

      return;
    }

    cancel_press();
    store(|store| {
      store.drag = Some(Drag::Item {
        node,
        from,
        started: true,
      });
    });
  }

  let over = store(|store| {
    let (handle, _) = store.hit_test(x, y).last().copied()?;

    store.record(handle)?.node
  });
  let _ = move_drag::<Tui>(over);

  store(|store| store.restyle_pending = true);
}

/// Follows the drag in progress to the pointer at (`x`, `y`), if one is under way: a
/// scrollbar's thumb follows the pointer's position within the cell.
pub(crate) fn drag_to(x: u16, y: u16) {
  let item = store(|store| match store.drag {
    Some(Drag::Item {
      node,
      from,
      started,
    }) => Some((node, from, started)),
    _ => None,
  });

  if let Some((node, from, started)) = item {
    drag_item(node, from, started, x, y);

    return;
  }

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
    Some(Drag::Item { .. }) | None => None,
  });

  if let Some((handle, offsets)) = scrolled {
    scroll_to(handle, offsets);
  }

  crate::field::report_selections();
}

/// A count of cells as a position along a track.
#[expect(
  clippy::cast_precision_loss,
  reason = "a track's cells stay far below the integers f32 holds exactly"
)]
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
    #[expect(
      clippy::cast_precision_loss,
      reason = "a scroll offset in cells stays far below the integers f32 holds exactly"
    )]
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

/// Turns the wheel over the cell (`x`, `y`) by rows and columns: the turn reaches the
/// node under the pointer first, then, unless a handler prevented the platform, scrolls
/// there—a gesture in progress keeping its latched box, consuming the wheel at that
/// box's limits, and a fresh gesture targeting the innermost box under the cell that
/// can still move in the given direction.
pub(crate) fn wheel(x: u16, y: u16, delta_x: i64, delta_y: i64, modifiers: Modifiers) {
  if !store(|store| store.mouse_taken()) {
    return;
  }

  let under = store(|store| {
    let (handle, origin) = store.hit_test(x, y).last().copied()?;

    Some((store.record(handle)?.node?, origin))
  });

  if let Some((node, origin)) = under {
    #[expect(
      clippy::cast_precision_loss,
      reason = "a wheel's turn in rows stays far below the integers f32 holds exactly"
    )]
    let delta = Axes::new(delta_x as f32, delta_y as f32);
    let event = pointer_event(x, y, origin, None, modifiers).wheel(delta, WheelUnit::Lines);

    if dispatch_wheel::<Tui>(node, &event).platform {
      return;
    }
  }

  let now = std::time::Instant::now();
  let (latched, candidates) = store(|store| {
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
/// by `pages` pages, a page the rows it shows less one, as a browser keeps a line of
/// context.
pub(crate) fn page_scroll(pages: i64) {
  let target = store(|store| {
    let target = store
      .focused()
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

          stack.extend(store.children_reversed(current));
        }

        None
      })?;
    let rows = if store.is_editor(target) {
      i64::try_from(store.field_area(target).rows).unwrap_or(i64::MAX)
    }
    else {
      let (_, height) = store.node_size(target);
      let (_, top, _, bottom) = store.node_border(target);

      height - top - bottom
    };

    Some((target, (rows - 1).max(1)))
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
    // A modifier pressed alone shows focus no more than it does in a browser.
    if !event.is_modifier() {
      store.set_focus_visible(true);
    }

    let handle = store.focused().unwrap_or(store.root);

    store.record(handle).and_then(|record| record.node)
  });
  let prevented = target.map_or(Prevented::default(), |node| {
    dispatch_key_down::<Tui>(node, event)
  });

  // The handlers' writes have taken effect by the time the default runs.
  skene_reactive::Runtime::flush_sync();

  if !prevented.platform {
    key_default(event);
  }

  crate::field::report_selections();
}

/// Dispatches a key release to the focused node, or the root; the terminal has no
/// response of its own to one.
pub(crate) fn key_up(event: &KeyEvent) {
  let target = store(|store| {
    let handle = store.focused().unwrap_or(store.root);

    store.record(handle).and_then(|record| record.node)
  });

  if let Some(node) = target {
    let _ = dispatch_key_up::<Tui>(node, event);
  }
}

/// The terminal's own response to a key.
fn key_default(event: &KeyEvent) {
  // The mouse key hands the mouse over and takes it back; any other key, a modifier
  // alone aside, takes it back and goes on to act.
  let (mouse_key, released) = store(|store| {
    (
      store.mouse
        && store
          .mouse_key
          .is_some_and(|shortcut| event.matches(&shortcut)),
      store.mouse_released,
    )
  });

  if mouse_key {
    if released {
      store(|store| store.mouse_released = false);
    }
    else {
      release_mouse();
    }

    return;
  }

  if released && !event.is_modifier() {
    store(|store| store.mouse_released = false);
  }

  let offers = |action| store(|store| store.offers(action));
  let ctrl = event.modifiers.ctrl && !event.modifiers.alt;

  match event.shortcut_key().as_ref() {
    "Tab" => {
      move_focus(if event.modifiers.shift { -1 } else { 1 });

      return;
    }
    "Enter" if store(|store| store.focused_link()).is_some() => {
      follow_focused_link(event.modifiers);

      return;
    }
    "Escape" => {
      // The most transient layer goes first: a drag, an open overlay, then the
      // selection.
      if store(|store| {
        matches!(store.drag, Some(Drag::Item { started: true, .. })) && store.drag.take().is_some()
      }) {
        end_drag::<Tui>();
        store(|store| store.restyle_pending = true);

        return;
      }

      let dismissed = store(|store| {
        if let Some((top, _)) = store.topmost_open_overlay() {
          return store.record(top).and_then(|record| record.node);
        }

        if store.has_selection() {
          store.clear_selection();

          if let Some(entry) = store.focused_entry() {
            store.collapse_entry_selection(entry);
          }
        }

        None
      });

      if let Some(dismissed) = dismissed {
        notify_dismiss::<Tui>(dismissed);
      }

      return;
    }
    "PageDown" => page_scroll(1),
    "PageUp" => page_scroll(-1),
    // The menu key, and `Shift-F10`, ask the focused node for its context menu at its
    // bottom-start corner, as a browser does.
    "ContextMenu" => {
      context_menu_from_keyboard(event.modifiers);

      return;
    }
    "F10" if event.modifiers.shift => {
      context_menu_from_keyboard(event.modifiers);

      return;
    }
    // The clipboard keys the launch offers: `Ctrl+C` copies whichever selection
    // exists, `Ctrl+X` cuts the focused entry's, `Ctrl+V` pastes into the focused
    // entry; one not offered is an ordinary key.
    "c" if ctrl && offers(ClipboardAction::Copy) => {
      store(Store::copy_selection);

      return;
    }
    "x" if ctrl && offers(ClipboardAction::Cut) => {
      crate::field::cut_focused();

      return;
    }
    "v" if ctrl && offers(ClipboardAction::Paste) => {
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

/// Asks the focused node, or the root, for its context menu at the node's bottom-start
/// corner, the request naming no pointer.
fn context_menu_from_keyboard(modifiers: Modifiers) {
  let Some((node, origin, x, y)) = store(|store| {
    let handle = store.focused().unwrap_or(store.root);
    let node = store.record(handle).and_then(|record| record.node)?;
    let (origin_x, origin_y) = store.screen_origin(handle);
    let (_, height) = store.node_size(handle);

    Some((
      node,
      (origin_x, origin_y),
      u16::try_from(origin_x.max(0)).unwrap_or(u16::MAX),
      u16::try_from((origin_y + height).max(0)).unwrap_or(u16::MAX),
    ))
  })
  else {
    return;
  };

  let event = PointerEvent {
    pointer: PointerInfo {
      pointer_id: None,
      persistent_device_id: None,
      pointer_type: PointerType::Unknown,
    },
    ..pointer_event(x, y, origin, None, modifiers)
  };
  let _ = dispatch_context_menu::<Tui>(node, &event);
}

/// Hands the mouse to the terminal until a key takes it back: a press in progress is
/// cancelled at the node it reached, the hover path empties, its nodes told the pointer
/// left, and every mouse event is ignored meanwhile.
fn release_mouse() {
  store(|store| {
    store.mouse_released = true;
    store.scroll_latch = None;
    store.drag = None;
    store.pressed_link = None;
    store.buttons.clear();

    store.set_hovered(Vec::new());
  });

  end_drag::<Tui>();
  cancel_press();
  notify_pointer_moved::<Tui>(None);
}

/// Cancels the press in progress at the node it reached, which holds `Pressed` no
/// longer, as a platform taking the pointer for itself does.
fn cancel_press() {
  let cancelled = store(|store| {
    let press = store.press.take()?;

    store.restyle_pending = true;

    let reached = press.captured.unwrap_or(press.pressed);

    Some((reached, store.screen_origin(reached), store.last_pointer))
  });
  let Some((reached, origin, (x, y))) = cancelled
  else {
    return;
  };

  Scene::<Tui>::with(|scene| scene.set_pressed(None));

  if let Some(node) = store(|store| store.record(reached).and_then(|record| record.node)) {
    let event = PointerEvent {
      buttons: PointerButtons::new(),
      pressure: 0.,
      ..pointer_event(x, y, origin, None, Modifiers::NONE)
    };
    let _ = dispatch_pointer_cancel::<Tui>(node, &event);
  }
}

/// Clicks the focused link with `modifiers` held and, unless a handler prevented the
/// platform, stages its destination for the terminal loop's system handler: `Enter` on
/// a link is its click, as on an anchor.
fn follow_focused_link(modifiers: Modifiers) {
  let Some((node, url)) = store(|store| store.focused_link())
  else {
    return;
  };
  let prevented = dispatch_click::<Tui>(
    node,
    &ClickEvent {
      modifiers,
      count: 0,
    },
  );

  if prevented.platform {
    return;
  }

  store(|store| store.pending_open = Some(url));
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
