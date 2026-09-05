//! One frame: the restyle and layout passes, the size reports and scroll-into-view
//! they settle, then the paint.

use std::rc::Rc;

use skene_core::{Size, next::ContainerName};
use skene_reactive::Set;

use crate::{
  interaction::scroll_to,
  layout,
  paint::{self, Buffer},
  store::{Handle, Record, Store},
  with_store,
};

/// How many times a frame re-lays out for a size report or a container measurement
/// before painting what it has.
const SETTLE_ROUNDS: usize = 4;

/// Lays the tree out at `width` x `height` cells and paints it.
///
/// The size reports, container measurements and scroll-into-view resolve between
/// layout and paint; the handlers they run may rebuild content, so the layout is
/// computed again until it settles (bounded, so a pathological feedback loop still
/// paints).
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

      true
    });

    if !laid_out {
      break;
    }

    let containers = measure_containers();
    let reported = report_sizes();
    let revealed = reveal();

    if !(containers || reported || revealed) {
      break;
    }
  }

  with_store(|store| {
    store.paint_dirty = false;

    paint::paint(store, width, height)
  })
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
