//! One frame: the restyle and layout passes, the size reports and scroll-into-view
//! they settle, then the paint.

use std::rc::Rc;

use skene_foundation::{
  Canvas,
  Color,
  NodeId,
  Size,
  kurbo::{Rect, RoundedRect, Stroke},
};
use skene_layout::runtime::Runtime;

use crate::{
  Terminal,
  layout::{border_edges, has_border},
  paint::{self, Buffer, Image, ShadowPaint, terminal_rgb},
  records::{Backdrop, BackdropKey, CellRect, PlainBox, Records},
  with_records,
};

/// How many times a frame re-lays out for a size report or a container measurement
/// before painting what it has.
const SETTLE_ROUNDS: usize = 4;

/// Lays the tree out at `width` x `height` cells and paints it, once the pending turn
/// has ended and the caret drag's autoscroll has stepped.
///
/// The layout settles as [`settle`] says. The paint rewrites the rows the changed nodes
/// covered and cover, over the previous frame; a whole-frame change rewrites every row.
pub(crate) fn render(width: u16, height: u16) -> Buffer {
  crate::realize::end_turn();
  resize(width, height);
  crate::field::autoscroll();
  crate::field::report_selections();
  crate::realize::end_turn();
  settle(width, height);
  paint_frame(width, height)
}

/// Lays the tree out at `width` x `height` cells until it settles, painting nothing.
///
/// The size reports, container measurements and scroll-into-view resolve after each
/// layout; the handlers they run may rebuild content, so the layout is computed again,
/// once their turn has ended, until it settles (bounded, so a pathological feedback loop
/// still ends).
pub(crate) fn settle(width: u16, height: u16) {
  for _ in 0..SETTLE_ROUNDS {
    let laid_out = with_records(|records| {
      let restyled = records.runtime.write_styles();

      records.runtime.write_children();

      if !(restyled || records.runtime.layout_dirty) {
        return false;
      }

      records.runtime.layout_dirty = false;
      records
        .runtime
        .compute(Size::new(f32::from(width), f32::from(height)));
      records.fit_entries();

      true
    });
    let containers = laid_out && measure_containers();
    let reported = laid_out && report_sizes();
    let revealed = reveal();

    crate::realize::end_turn();

    let settled =
      !(containers || reported || revealed || with_records(|records| records.runtime.layout_dirty));

    if settled {
      break;
    }
  }
}

/// Paints the laid-out tree into a frame of `width` x `height` cells.
fn paint_frame(width: u16, height: u16) -> Buffer {
  with_records(|records| {
    let (width, height) = (usize::from(width), usize::from(height));
    let previous = records
      .last_frame
      .take()
      .filter(|frame| frame.width == width && frame.height == height);
    let full = std::mem::take(&mut records.runtime.paint_dirty) || previous.is_none();
    let moved = std::mem::take(&mut records.runtime.covers_moved) || full;
    let (damaged, rects) = records.take_damage();

    if let Some(previous) = previous.as_ref().filter(|_| !moved)
      && damaged.is_empty()
      && rects.is_empty()
    {
      records.repainted = Vec::new();
      records.last_frame = Some(previous.clone());

      return previous.clone();
    }

    render_pictures(records);
    render_backdrops(records);

    let rows = damaged_rows(records, height, damaged, rects, moved);
    let mut buffer = match previous {
      Some(previous) if !full => previous.repainting(&rows),
      _ => Buffer::new(width, height),
    };

    records.repainted = rows
      .iter()
      .enumerate()
      .filter(|(_, repaint)| full || **repaint)
      .map(|(row, _)| row)
      .collect();

    paint::paint(records, &mut buffer);

    if moved {
      record_painted(records);
    }

    records.last_frame = Some(buffer.clone());

    buffer
  })
}

/// The rows the next paint rewrites: those the `damaged` nodes covered at the last
/// paint and cover now, the `rects` no node covers any more, and, where the covers
/// `moved`, those of every node the layout or a scroll moved or resized.
fn damaged_rows(
  records: &Records,
  height: usize,
  damaged: Vec<NodeId>,
  mut rects: Vec<CellRect>,
  moved: bool,
) -> Vec<bool> {
  let frames = records.runtime.frames();
  let cover = |node: NodeId| frames.cover(node).map(cell_rect);

  for node in damaged {
    let target = paint_owner(records, node);

    if let Some(cell) = records.cell(target) {
      rects.extend(cell.painted);
      rects.extend(cover(target));
    }
  }

  if moved {
    for (node, record) in records.runtime.records() {
      let (painted, covers) = (record.platform.painted, cover(node));

      if painted != covers {
        rects.extend(painted);
        rects.extend(covers);
      }
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

/// The node whose paint covers `node`: the paragraph an inline run flows in, or an
/// inline box's content, through any groups; else the node itself.
fn paint_owner(records: &Records, node: NodeId) -> NodeId {
  let mut owner = node;

  while let Some(parent) = records.runtime.layout_parent(owner) {
    if !(records.runtime.is_paragraph(parent) || records.runtime.is_inline_run(parent)) {
      break;
    }

    owner = parent;
  }

  owner
}

/// The screen cells `rect`, a cover the terminal's layout places at whole cells, spans.
#[expect(
  clippy::cast_possible_truncation,
  reason = "the terminal's layout places a box at whole cells, far inside i64"
)]
const fn cell_rect(rect: skene_foundation::Rect<f32>) -> CellRect {
  (
    rect.origin.x as i64,
    rect.origin.y as i64,
    rect.size.width as i64,
    rect.size.height as i64,
  )
}

/// Renders every painted box's picture whose image is missing or of another size than
/// the box's content in pixels, while the terminal shows graphics: the painter runs
/// on a canvas of that size, and the pixels take the next generation.
fn render_pictures(records: &mut Records) {
  if !records.runtime.window().painting() {
    return;
  }

  let (cell_width, cell_height) = records.cell_pixels;
  let pictured: Vec<NodeId> = records
    .runtime
    .records()
    .filter(|(_, record)| record.platform.picture.is_some())
    .map(|(node, _)| node)
    .collect();

  for node in pictured {
    let Some(picture) = records.cell(node).and_then(|cell| cell.picture.as_ref())
    else {
      continue;
    };
    let size = records.client_size(node);
    let width = u16::try_from(size.width.saturating_mul(i32::from(cell_width))).unwrap_or(u16::MAX);
    let height =
      u16::try_from(size.height.saturating_mul(i32::from(cell_height))).unwrap_or(u16::MAX);

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

    records.picture_generation += 1;

    let generation = records.picture_generation;

    if let Some(picture) = records
      .cell_mut(node)
      .and_then(|cell| cell.picture.as_mut())
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
fn render_backdrops(records: &mut Records) {
  if !records.runtime.window().painting() {
    return;
  }

  let cell_pixels = records.cell_pixels;
  let ink = records.contrast().ink;
  let changed: Vec<(NodeId, Option<BackdropKey>)> = records
    .runtime
    .records()
    .filter_map(|(node, record)| {
      let cache = record.kind.box_record()?.cache.as_ref()?;
      let key = BackdropKey::of(&cache.style, records.node_size(node), cell_pixels, ink);
      let held = record
        .platform
        .backdrop
        .as_ref()
        .map(|backdrop| &backdrop.key);

      (held != key.as_ref()).then_some((node, key))
    })
    .collect();

  for (node, key) in changed {
    let backdrop = key.map(|key| {
      let pixels = render_backdrop(&key);

      records.picture_generation += 1;

      Backdrop {
        image: Rc::new(Image {
          generation: records.picture_generation,
          width: key.columns.saturating_mul(cell_pixels.0),
          height: key.rows.saturating_mul(cell_pixels.1),
          pixels,
        }),
        key,
      }
    });

    if let Some(cell) = records.cell_mut(node) {
      cell.backdrop = backdrop;
    }
  }
}

impl BackdropKey {
  /// The backdrop a box of `style` at `size` cells asks for, with a border of no color
  /// in `ink`; `None` where the box is empty or its decoration is one cells draw.
  fn of(
    style: &PlainBox,
    (width, height): (i64, i64),
    cell_pixels: (u16, u16),
    ink: (u8, u8, u8),
  ) -> Option<Self> {
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
        None => Some(ink),
      })
      .flatten();

    if width <= 0 || height <= 0 || (radius <= 0. && border.is_none() && shadow.is_none()) {
      return None;
    }

    let band = if shadow.is_some() {
      ShadowPaint::band(style)
    }
    else {
      0
    };
    let (shadow_offset, shadow_softening) = ShadowPaint::extent(style);

    Some(Self {
      columns: u16::try_from(width)
        .unwrap_or(u16::MAX)
        .saturating_add(band),
      rows: u16::try_from(height)
        .unwrap_or(u16::MAX)
        .saturating_add(band),
      band,
      cell_pixels,
      background: style.background.and_then(terminal_rgb),
      border,
      border_edges: border_edges(style),
      radius,
      shadow,
      shadow_offset,
      shadow_softening,
    })
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

    if edges.start && edges.top && edges.end && edges.bottom {
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
        (edges.end, width - line, 0., width, height),
        (edges.bottom, 0., height - line, width, height),
        (edges.start, 0., 0., line, height),
      ] {
        if drawn {
          canvas.fill(&Rect::new(x0, y0, x1, y1), ink);
        }
      }
    }
  }

  canvas.pixels()
}

/// Records the rectangle every node covers as the one it covered at this paint.
fn record_painted(records: &mut Records) {
  let covers: Vec<(NodeId, CellRect)> = records
    .runtime
    .frames()
    .entries()
    .iter()
    .filter_map(|frame| Some((frame.node, cell_rect(frame.cover?))))
    .collect();

  for (_, record) in records.runtime.records_mut() {
    record.platform.painted = None;
  }

  for (node, cover) in covers {
    if let Some(cell) = records.cell_mut(node) {
      cell.painted = Some(cover);
    }
  }
}

/// Reports the terminal size, moving the viewport signal and every resolved style when
/// it changed.
pub(crate) fn resize(width: u16, height: u16) {
  let viewport = Size::new(i32::from(width), i32::from(height));

  Runtime::<Terminal>::report(|facts| facts.reported.viewport = viewport).write();
}

/// Measures every named container and publishes the sizes; `true` when a size changed,
/// in which case every style is stale and the container sizes signal moves.
fn measure_containers() -> bool {
  let measured = with_records(|records| {
    records
      .runtime
      .records()
      .filter_map(|(node, record)| Some((record.container()?, records.client_size(node))))
      .collect::<Vec<_>>()
  });

  Runtime::<Terminal>::publish_container_sizes(measured)
}

/// One observer with the size to report to it.
type Report = (Rc<dyn Fn(Size<i32>)>, Size<i32>);

/// Reports every observed node's size where it changed since the last report; `true`
/// when any observer ran.
fn report_sizes() -> bool {
  let reports: Vec<Report> = with_records(|records| {
    let changed: Vec<(NodeId, Size<i32>)> = records
      .runtime
      .records()
      .filter(|(_, record)| !record.observers.is_empty())
      .filter_map(|(node, record)| {
        let size = records.client_size(node);

        (record.reported != Some(size)).then_some((node, size))
      })
      .collect();
    let mut reports = Vec::new();

    for (node, size) in changed {
      if let Some(record) = records.runtime.record_mut(node) {
        record.reported = Some(size);

        reports.extend(
          record
            .observers
            .iter()
            .map(|observer| (Rc::clone(observer), size)),
        );
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
  let adjusted = with_records(|records| {
    let node = records.runtime.take_reveal()?;

    Some(records.runtime.reveal_offsets(node))
  })
  .unwrap_or_default();
  let changed = !adjusted.is_empty();

  for (node, offset) in adjusted {
    Runtime::<Terminal>::scroll_to(node, offset);
  }

  changed
}
