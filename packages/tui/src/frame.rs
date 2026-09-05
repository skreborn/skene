//! One frame: the restyle and layout passes, the size reports and scroll-into-view
//! they settle, then the paint.

use std::rc::Rc;

use skene_core::{Size, next::ContainerName};
use skene_reactive::Set;

use crate::{
  interaction::scroll_to,
  layout::{self, is_inline_box, is_paragraph},
  paint::{self, Buffer},
  store::{CellRect, Handle, Record, Store},
  with_store,
};

/// How many times a frame re-lays out for a size report or a container measurement
/// before painting what it has.
const SETTLE_ROUNDS: usize = 4;

/// Lays the tree out at `width` x `height` cells and paints it.
///
/// The size reports, container measurements, inline box measurements and
/// scroll-into-view resolve between layout and paint; the handlers they run may rebuild
/// content, so the layout is computed again until it settles (bounded, so a
/// pathological feedback loop still paints). The paint rewrites the rows the changed
/// nodes covered and cover, over the previous frame; a whole-frame change rewrites
/// every row.
pub(crate) fn render(width: u16, height: u16) -> Buffer {
  resize(width, height);

  for _ in 0..SETTLE_ROUNDS {
    let laid_out = with_store(|store| {
      let restyled = layout::restyle(store);

      layout::rebuild_children(store);

      if !(restyled || store.layout_dirty) {
        return false;
      }

      store.layout_dirty = false;
      layout::compute(store, width, height);
      layout::measure_inline_boxes(store);

      true
    });
    let containers = laid_out && measure_containers();
    let reported = laid_out && report_sizes();
    let revealed = reveal();

    let settled = !(containers || reported || revealed || with_store(|store| store.layout_dirty));

    if settled {
      break;
    }
  }

  with_store(|store| {
    let (width, height) = (usize::from(width), usize::from(height));
    let previous = store
      .last_frame
      .take()
      .filter(|frame| frame.width == width && frame.height == height);
    let full = std::mem::take(&mut store.paint_dirty) || previous.is_none();
    let rows = damaged_rows(store, height);
    let mut buffer = match previous {
      Some(previous) if !full => previous.repainting(&rows),
      _ => Buffer::new(width, height),
    };

    store.repainted = rows
      .iter()
      .enumerate()
      .filter(|(_, repaint)| full || **repaint)
      .map(|(row, _)| row)
      .collect();

    paint::paint(store, &mut buffer);
    record_painted(store);

    store.last_frame = Some(buffer.clone());

    buffer
  })
}

/// The rows the next paint rewrites: those the damaged nodes covered at the last paint
/// and cover now, those of the rectangles no node covers any more, and those of every
/// node the layout moved or resized.
fn damaged_rows(store: &mut Store, height: usize) -> Vec<bool> {
  let (damaged, mut rects) = store.take_damage();

  for handle in damaged {
    let target = paint_owner(store, handle);

    rects.extend(store.record(target).and_then(|record| record.painted));
    rects.extend(current_rect(store, target));
  }

  for handle in store.handles() {
    let Some(record) = store.record(handle)
    else {
      continue;
    };

    if !record.kind.is_laid_out() {
      continue;
    }

    let current = current_rect(store, handle);

    if record.painted != current {
      rects.extend(record.painted);
      rects.extend(current);
    }
  }

  let mut rows = vec![false; height];

  for (_, y, _, extent) in rects {
    let from = usize::try_from(y.max(0)).unwrap_or(0).min(height);
    let to = usize::try_from((y + extent).max(0))
      .unwrap_or(0)
      .min(height);

    rows[from..to].fill(true);
  }

  rows
}

/// The node whose paint covers `handle`: the paragraph an inline run or an inline box
/// flows in, else the node itself.
fn paint_owner(store: &Store, handle: Handle) -> Handle {
  let mut owner = handle;

  while let Some(parent) = store.layout_parent(owner) {
    if !(is_paragraph(store, parent) || is_inline_box(store, parent)) {
      break;
    }

    owner = parent;
  }

  owner
}

/// The screen rectangle `handle` paints now, grown by the cell its shadow casts into;
/// `None` for a node without extent.
fn current_rect(store: &Store, handle: Handle) -> Option<CellRect> {
  let (width, height) = store.node_size(handle);

  if width <= 0 || height <= 0 {
    return None;
  }

  let (x, y) = store.screen_origin(handle);
  let shadow = i64::from(
    store
      .record(handle)
      .and_then(|record| record.kind.box_style())
      .is_some_and(|style| style.ext.shadow.is_some()),
  );

  Some((x, y, width + shadow, height + shadow))
}

/// Records the rectangle every laid-out node covers at this paint.
fn record_painted(store: &mut Store) {
  for handle in store.handles() {
    let rect = current_rect(store, handle);

    if let Some(record) = store.record_mut(handle) {
      record.painted = rect;
    }
  }
}

/// Records the terminal size, moving the viewport signal and every resolved style when
/// it changed.
pub(crate) fn resize(width: u16, height: u16) {
  let write = with_store(|store| {
    if store.last_size == (width, height) {
      return None;
    }

    store.last_size = (width, height);
    store.invalidate_styles();

    Some((store.media().set_viewport, store.viewport()))
  });

  if let Some((write, viewport)) = write {
    write.set(viewport);
  }
}

/// Measures every named container; `true` when a size changed, in which case every
/// style is stale and the container sizes signal moves.
fn measure_containers() -> bool {
  let (changed, publish) = with_store(|store| {
    let mut changed: Vec<(ContainerName, Size<u32>)> = Vec::new();

    for handle in store.handles() {
      let Some(name) = store.record(handle).and_then(Record::container)
      else {
        continue;
      };
      let size = store.client_size(handle);

      if store.container_sizes.get(&name) != Some(&size) {
        changed.push((name, size));
      }
    }

    if changed.is_empty() {
      return (false, None);
    }

    for (name, size) in &changed {
      store.container_sizes.insert(*name, *size);
    }

    store.invalidate_styles();

    (true, Some((store.media().signals.container_sizes, changed)))
  });

  if let Some((sizes, changed)) = publish {
    for (name, size) in changed {
      sizes.set(name, size);
    }
  }

  changed
}

/// One observer with the size to report to it.
type Report = (Rc<dyn Fn(Size<u32>)>, Size<u32>);

/// Reports every observed node's size where it changed since the last report; `true`
/// when any observer ran.
fn report_sizes() -> bool {
  let reports: Vec<Report> = with_store(|store| {
    let mut reports = Vec::new();

    for handle in store.handles() {
      let Some(record) = store.record(handle)
      else {
        continue;
      };

      if record.observers.is_empty() {
        continue;
      }

      let size = store.client_size(handle);

      if record.reported == Some(size) {
        continue;
      }

      reports.extend(
        record
          .observers
          .iter()
          .map(|observer| (Rc::clone(observer), size)),
      );

      if let Some(record) = store.record_mut(handle) {
        record.reported = Some(size);
      }
    }

    reports
  });
  let ran = !reports.is_empty();

  for (report, size) in reports {
    report(size);
  }

  ran
}

/// Scrolls ancestors so the pending reveal target is visible; `true` when any offset
/// changed.
fn reveal() -> bool {
  let adjusted = with_store(|store| {
    let node = store.reveal.take()?;

    Some(adjustments(store, node))
  })
  .unwrap_or_default();
  let changed = !adjusted.is_empty();

  for (handle, offsets) in adjusted {
    scroll_to(handle, offsets);
  }

  changed
}

/// The scroll offsets each scrollable ancestor of `handle` takes to show it.
fn adjustments(store: &Store, handle: Handle) -> Vec<(Handle, (u32, u32))> {
  let mut adjusted = Vec::new();
  let (mut rel_x, mut rel_y) = store.node_location(handle);
  let (width, height) = store.node_size(handle);
  let mut current = store.layout_parent(handle);

  while let Some(ancestor) = current {
    let (mut offset_x, mut offset_y) = store.scroll_offset(ancestor);

    if store.is_scrollable(ancestor) {
      let (limit_x, limit_y) = store.scroll_limits(ancestor);
      let (ancestor_width, ancestor_height) = store.node_size(ancestor);
      let border = store.node_border(ancestor);
      let next = (
        fit_axis(
          i64::from(offset_x),
          rel_x,
          width,
          ancestor_width,
          border.0,
          i64::from(limit_x),
        ),
        fit_axis(
          i64::from(offset_y),
          rel_y,
          height,
          ancestor_height,
          border.1,
          i64::from(limit_y),
        ),
      );
      let next = (
        u32::try_from(next.0).unwrap_or(0),
        u32::try_from(next.1).unwrap_or(0),
      );

      if next != (offset_x, offset_y) {
        adjusted.push((ancestor, next));
      }

      (offset_x, offset_y) = next;
    }

    let (location_x, location_y) = store.node_location(ancestor);

    rel_x += location_x - i64::from(offset_x);
    rel_y += location_y - i64::from(offset_y);
    current = store.layout_parent(ancestor);
  }

  adjusted
}

/// The nearest offset showing `[rel, rel + extent)` within a viewport of `size` behind
/// a border, clamped to `[0, limit]`; an oversized target aligns to its start.
const fn fit_axis(offset: i64, rel: i64, extent: i64, size: i64, border: i64, limit: i64) -> i64 {
  let min_offset = rel + extent - size + border;
  let max_offset = rel - border;
  let next = if min_offset > max_offset || offset > max_offset {
    max_offset
  }
  else if offset < min_offset {
    min_offset
  }
  else {
    offset
  };

  if next < 0 {
    0
  }
  else if next > limit {
    limit
  }
  else {
    next
  }
}
