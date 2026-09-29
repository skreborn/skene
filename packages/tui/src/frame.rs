//! One frame: the restyle and layout passes, the size reports and scroll-into-view
//! they settle, then the paint.

use std::rc::Rc;

use skene_common::{
  Canvas,
  kurbo::{Rect, RoundedRect, Stroke},
};
use skene_core::{Color, ContainerName, Inset, Size};
use skene_reactive::Set;

use crate::{
  interaction::scroll_to,
  layout::{self, border_edges, has_border, is_inline_run, is_paragraph},
  paint::{self, Buffer, Image, ShadowPaint, terminal_rgb},
  store::{Backdrop, BackdropKey, CellRect, Handle, Record, Store},
  with_store,
};

/// How many times a frame re-lays out for a size report or a container measurement
/// before painting what it has.
const SETTLE_ROUNDS: usize = 4;

/// Lays the tree out at `width` x `height` cells and paints it, once the pending turn
/// has ended.
///
/// The size reports, container measurements, inline box measurements and
/// scroll-into-view resolve between layout and paint; the handlers they run may rebuild
/// content, so the layout is computed again, once their turn has ended, until it
/// settles (bounded, so a pathological feedback loop still paints). The paint rewrites the rows the
/// changed nodes covered and cover, over the previous frame; a whole-frame change rewrites
/// every row.
pub(crate) fn render(width: u16, height: u16) -> Buffer {
  crate::platform::end_turn();
  resize(width, height);
  with_store(Store::autoscroll_caret_drag);
  crate::field::report_selections();
  crate::platform::end_turn();

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

    crate::platform::end_turn();

    let settled = !(containers || reported || revealed || with_store(|store| store.layout_dirty));

    if settled {
      break;
    }
  }

  with_store(|store| {
    render_pictures(store);
    render_backdrops(store);

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

/// The node whose paint covers `handle`: the paragraph an inline run flows in, or an
/// inline box's content, through any groups; else the node itself.
fn paint_owner(store: &Store, handle: Handle) -> Handle {
  let mut owner = handle;

  while let Some(parent) = store.layout_parent(owner) {
    if !(is_paragraph(store, parent) || is_inline_run(store, parent)) {
      break;
    }

    owner = parent;
  }

  owner
}

/// The screen rectangle `handle` paints now, grown by the cells its shadow reaches;
/// `None` for a node without extent.
fn current_rect(store: &Store, handle: Handle) -> Option<CellRect> {
  let (width, height) = store.node_size(handle);

  if width <= 0 || height <= 0 {
    return None;
  }

  let (x, y) = store.screen_origin(handle);
  let band = store
    .record(handle)
    .and_then(|record| record.kind.box_style())
    .map_or(0, |style| i64::from(ShadowPaint::band(style)));

  Some((x, y, width + band, height + band))
}

/// Renders every painted box's picture whose image is missing or of another size than
/// the box's content in pixels, while the terminal shows graphics: the painter runs
/// on a canvas of that size, and the pixels take the next generation.
fn render_pictures(store: &mut Store) {
  if !store.graphics {
    return;
  }

  let (cell_width, cell_height) = store.cell_pixels;

  for handle in store.handles() {
    let Some(record) = store.record(handle)
    else {
      continue;
    };
    let Some(picture) = &record.picture
    else {
      continue;
    };
    let size = store.client_size(handle);
    let width = u16::try_from(size.width.saturating_mul(u32::from(cell_width))).unwrap_or(u16::MAX);
    let height =
      u16::try_from(size.height.saturating_mul(u32::from(cell_height))).unwrap_or(u16::MAX);

    if width == 0 || height == 0 {
      continue;
    }

    if picture
      .image
      .as_ref()
      .is_some_and(|image| (image.width, image.height) == (width, height))
    {
      continue;
    }

    let painter = Rc::clone(&picture.painter);
    let mut canvas = Canvas::new(width, height);

    painter(&mut canvas, Size::new(f32::from(width), f32::from(height)));

    let pixels = canvas.pixels();

    store.picture_generation += 1;

    let generation = store.picture_generation;

    if let Some(picture) = store
      .record_mut(handle)
      .and_then(|record| record.picture.as_mut())
    {
      picture.image = Some(Rc::new(Image {
        generation,
        width,
        height,
        pixels,
      }));
    }
  }
}

/// Renders the backdrop of every box whose decoration asks for more than cells draw—a
/// corner radius, a border, a shadow—while the terminal shows graphics, and drops the
/// backdrop of a box that no longer does: the image is the box's cells and the
/// shadow's band in pixels, rendered afresh when the size, the cell's pixels or any
/// part of the decoration changed.
fn render_backdrops(store: &mut Store) {
  if !store.graphics {
    return;
  }

  let cell_pixels = store.cell_pixels;

  for handle in store.handles() {
    let Some(record) = store.record(handle)
    else {
      continue;
    };
    let Some(cache) = record
      .kind
      .box_record()
      .and_then(|inner| inner.cache.as_ref())
    else {
      continue;
    };
    let style = &cache.style;
    let radius = style
      .ext
      .corner_radius
      .map_or(0., |radius| radius.0)
      .max(0.);
    let shadow = style
      .ext
      .shadow
      .map(Color::to_rgba)
      .filter(|rgba| rgba[3] > 0.);
    let border = has_border(style)
      .then(|| match style.border_color {
        Some(color) => terminal_rgb(color),
        None => Some(store.contrast().ink),
      })
      .flatten();
    let (width, height) = store.node_size(handle);
    let band = if shadow.is_some() {
      ShadowPaint::band(style)
    }
    else {
      0
    };
    let (shadow_offset, shadow_softening) = ShadowPaint::extent(style);
    let key = BackdropKey {
      columns: u16::try_from(width)
        .unwrap_or(u16::MAX)
        .saturating_add(band),
      rows: u16::try_from(height)
        .unwrap_or(u16::MAX)
        .saturating_add(band),
      band,
      cell_pixels,
      background: style.color.and_then(terminal_rgb),
      border,
      border_edges: border_edges(style),
      radius,
      shadow,
      shadow_offset,
      shadow_softening,
    };
    let wanted = width > 0 && height > 0 && (radius > 0. || border.is_some() || shadow.is_some());

    if !wanted {
      if let Some(record) = store.record_mut(handle) {
        record.backdrop = None;
      }

      continue;
    }

    if record
      .backdrop
      .as_ref()
      .is_some_and(|backdrop| backdrop.key == key)
    {
      continue;
    }

    let pixels = render_backdrop(&key);

    store.picture_generation += 1;

    let generation = store.picture_generation;

    if let Some(record) = store.record_mut(handle) {
      record.backdrop = Some(Backdrop {
        image: Rc::new(Image {
          generation,
          width: key.columns.saturating_mul(cell_pixels.0),
          height: key.rows.saturating_mul(cell_pixels.1),
          pixels,
        }),
        key,
      });
    }
  }
}

/// The pixels of a backdrop: the shadow as three layers at its offset down and right,
/// each growing by half its softening, the background as a rounded rectangle at the
/// radius in rows, and the border as a line a quarter cell wide inside the edge.
fn render_backdrop(key: &BackdropKey) -> Vec<u8> {
  let (cell_width, cell_height) = (f64::from(key.cell_pixels.0), f64::from(key.cell_pixels.1));
  let band = f64::from(key.band);
  let width = (f64::from(key.columns) - band) * cell_width;
  let height = (f64::from(key.rows) - band) * cell_height;
  let radius = f64::from(key.radius) * cell_height;
  let mut canvas = Canvas::new(
    key.columns.saturating_mul(key.cell_pixels.0),
    key.rows.saturating_mul(key.cell_pixels.1),
  );

  if let Some([red, green, blue, alpha]) = key.shadow {
    let offset = f64::from(key.shadow_offset);
    let softening = f64::from(key.shadow_softening);

    for (layer, spread) in [0.5_f32, 0.3, 0.2].into_iter().enumerate() {
      #[expect(clippy::cast_precision_loss, reason = "a shadow has three layers")]
      let grow = layer as f64 * cell_width * softening / 2.;

      canvas.fill(
        &RoundedRect::new(
          cell_width * offset - grow,
          cell_height * offset - grow,
          width + cell_width * offset + grow,
          height + cell_height * offset + grow,
          radius + grow,
        ),
        Color::srgb(red, green, blue).with_alpha(alpha * spread),
      );
    }
  }

  if let Some((red, green, blue)) = key.background {
    canvas.fill(
      &RoundedRect::new(0., 0., width, height, radius),
      Color::srgb(
        f32::from(red) / 255.,
        f32::from(green) / 255.,
        f32::from(blue) / 255.,
      ),
    );
  }

  if let Some((red, green, blue)) = key.border {
    let line = (cell_width / 4.).max(2.);
    let ink = Color::srgb(
      f32::from(red) / 255.,
      f32::from(green) / 255.,
      f32::from(blue) / 255.,
    );
    let edges = key.border_edges;

    if edges == Inset::all(true) {
      canvas.stroke(
        &RoundedRect::new(
          line / 2.,
          line / 2.,
          width - line / 2.,
          height - line / 2.,
          (radius - line / 2.).max(0.),
        ),
        &Stroke::new(line),
        ink,
      );
    }
    else {
      for (drawn, x0, y0, x1, y1) in [
        (edges.top, 0., 0., width, line),
        (edges.right, width - line, 0., width, height),
        (edges.bottom, 0., height - line, width, height),
        (edges.left, 0., 0., line, height),
      ] {
        if drawn {
          canvas.fill(&Rect::new(x0, y0, x1, y1), ink);
        }
      }
    }
  }

  canvas.pixels()
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

    Some(adjustments(store, paint_owner(store, node)))
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
