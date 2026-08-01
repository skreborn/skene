//! Layout and painting into a cell buffer.
//!
//! Each render refreshes every node's taffy style from its effective style (base plus
//! the variants holding in the current environment), computes the layout, and paints the
//! tree back-to-front into a [`Buffer`]—which serializes to plain text for tests or to
//! the terminal for the interactive loop.

use skene_core::NodeId;
use taffy::prelude::{AvailableSpace, length};

use crate::{
  state::{NodeKind, ROOT_ID, State},
  style::{
    Environment,
    Interaction,
    apply_box_sizing,
    box_taffy_style,
    effective_box_style,
    effective_run_style,
    terminal_rgb,
  },
};

/// One terminal cell's paint state.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct TerminalCell {
  pub(crate) character: char,
  pub(crate) foreground: Option<(u8, u8, u8)>,
  pub(crate) background: Option<(u8, u8, u8)>,
  pub(crate) bold: bool,
  pub(crate) reverse: bool,
}

impl TerminalCell {
  const EMPTY: Self = Self {
    character: ' ',
    foreground: None,
    background: None,
    bold: false,
    reverse: false,
  };
}

/// A painted frame: `width` x `height` cells in row-major order.
pub(crate) struct Buffer {
  pub(crate) width: usize,
  pub(crate) height: usize,
  pub(crate) cells: Vec<TerminalCell>,
  /// The active clip rectangle (start inclusive, end exclusive), while a clipping box
  /// paints its children.
  clip: Option<(isize, isize, isize, isize)>,
}

impl Buffer {
  fn new(width: usize, height: usize) -> Self {
    Self {
      width,
      height,
      cells: vec![TerminalCell::EMPTY; width * height],
      clip: None,
    }
  }

  fn cell_mut(&mut self, x: isize, y: isize) -> Option<&mut TerminalCell> {
    if x < 0 || y < 0 {
      return None;
    }

    if let Some((clip_x, clip_y, clip_end_x, clip_end_y)) = self.clip
      && (x < clip_x || y < clip_y || x >= clip_end_x || y >= clip_end_y)
    {
      return None;
    }

    let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);

    if x >= self.width || y >= self.height {
      return None;
    }

    self.cells.get_mut(y * self.width + x)
  }

  /// The frame as plain text, one row per line, trailing spaces trimmed.
  pub(crate) fn to_text(&self) -> String {
    let mut out = String::new();

    for row in 0..self.height {
      let line: String = self.cells[row * self.width..(row + 1) * self.width]
        .iter()
        .map(|cell| cell.character)
        .collect();

      out.push_str(line.trim_end());
      out.push('\n');
    }

    out
  }
}

/// Lays the tree out at `width` x `height` and paints it.
///
/// A pending scroll-into-view and the viewport watchers resolve between layout and
/// paint; the handlers they fire may rebuild content, so the layout is computed again
/// until it settles (bounded, so a pathological feedback loop still paints).
pub(crate) fn render(state: &State, width: u16, height: u16) -> Buffer {
  let environment = Environment {
    columns: width,
    rows: height,
    dark: state.dark.get(),
  };

  state.last_size.set((width, height));
  compute_layout(state, environment, width, height);

  for _ in 0..3 {
    let revealed = state.resolve_reveal();
    let viewports = state.notify_viewports();

    if !(revealed || viewports) {
      break;
    }

    compute_layout(state, environment, width, height);
  }

  let mut buffer = Buffer::new(usize::from(width), usize::from(height));

  paint_node(state, environment, &mut buffer, ROOT_ID, 0, 0);

  for overlay in state.open_overlays() {
    paint_overlay(state, environment, &mut buffer, overlay);
  }

  buffer
}

/// Refreshes every taffy style and computes the layout at `width` x `height`.
fn compute_layout(state: &State, environment: Environment, width: u16, height: u16) {
  refresh_styles(state, environment);

  let root_taffy = state.nodes.borrow()[&ROOT_ID].taffy;
  let mut taffy = state.taffy.borrow_mut();

  taffy
    .set_style(
      root_taffy,
      taffy::style::Style {
        display: taffy::style::Display::Flex,
        flex_direction: taffy::style::FlexDirection::Column,
        size: taffy::geometry::Size {
          width: length(f32::from(width)),
          height: length(f32::from(height)),
        },
        ..taffy::style::Style::default()
      },
    )
    .expect("style the root");

  taffy
    .compute_layout(
      root_taffy,
      taffy::geometry::Size {
        width: AvailableSpace::Definite(f32::from(width)),
        height: AvailableSpace::Definite(f32::from(height)),
      },
    )
    .expect("compute the layout");
}

/// Paints one open overlay over the frame: its backdrop scrim, then its subtree at its
/// computed origin.
fn paint_overlay(state: &State, environment: Environment, buffer: &mut Buffer, overlay: NodeId) {
  let backdrop = {
    let nodes = state.nodes.borrow();

    match nodes.get(&overlay).map(|record| &record.kind) {
      Some(NodeKind::Overlay { backdrop, .. }) => backdrop.and_then(terminal_rgb),
      _ => return,
    }
  };

  if let Some(color) = backdrop {
    for cell in &mut buffer.cells {
      *cell = TerminalCell {
        background: Some(color),
        ..TerminalCell::EMPTY
      };
    }
  }

  let (origin_x, origin_y) = state.overlay_origin(overlay);
  let (location_x, location_y) = {
    let nodes = state.nodes.borrow();
    let Some(record) = nodes.get(&overlay)
    else {
      return;
    };

    match state.taffy.borrow().layout(record.taffy) {
      #[expect(clippy::cast_possible_truncation)]
      Ok(layout) => (
        layout.location.x.round() as isize,
        layout.location.y.round() as isize,
      ),
      Err(_) => (0, 0),
    }
  };

  paint_node(
    state,
    environment,
    buffer,
    overlay,
    isize::try_from(origin_x).unwrap_or(0) - location_x,
    isize::try_from(origin_y).unwrap_or(0) - location_y,
  );
}

/// Refreshes every node's taffy style from its effective style in `environment`.
fn refresh_styles(state: &State, environment: Environment) {
  let nodes = state.nodes.borrow();
  let focused = state.focused.get();
  let mut taffy = state.taffy.borrow_mut();

  let hovered = state.hovered.borrow();

  for (id, record) in nodes.iter() {
    let interaction = Interaction {
      focused: focused == Some(*id),
      hovered: hovered.contains(id),
    };

    // The single mounted child fills the viewport, as the web reset's `body > *` does.
    let fills_root = record.parent == Some(ROOT_ID);

    match &record.kind {
      NodeKind::Box {
        layout,
        style,
        variants,
      } => {
        let effective = effective_box_style(style, variants, environment, interaction);
        let mut out = box_taffy_style(*layout, &effective, record.in_stack);

        if fills_root {
          out.flex_grow = 1.0;
        }

        taffy.set_style(record.taffy, out).expect("style a box");
      }
      NodeKind::Run { text, .. } => {
        let mut out = taffy::style::Style::default();

        if text.is_empty() && !record.children.is_empty() {
          // A group or embedded-view run: lay its children out in a row.
          out.display = taffy::style::Display::Flex;
        }
        else {
          #[expect(clippy::cast_precision_loss)]
          let width = text.chars().count() as f32;

          out.size = taffy::geometry::Size {
            width: length(width),
            height: length(1.0),
          };
        }

        if record.in_stack {
          apply_box_sizing(&mut out, &crate::props::BoxStyle::default(), true);
        }

        taffy.set_style(record.taffy, out).expect("style a run");
      }
      NodeKind::Input {
        style,
        variants,
        value,
        placeholder,
      } => {
        let effective = effective_box_style(style, variants, environment, interaction);
        let mut out = taffy::style::Style::default();

        apply_box_sizing(&mut out, &effective, record.in_stack);

        if effective.width.is_none() {
          #[expect(clippy::cast_precision_loss)]
          let content = value
            .chars()
            .count()
            .max(placeholder.as_ref().map_or(0, |text| text.chars().count()))
            .max(8) as f32;

          out.size.width = length(content);
        }

        if effective.height.is_none() {
          out.size.height = length(if effective.border.is_some() { 3.0 } else { 1.0 });
        }

        taffy.set_style(record.taffy, out).expect("style an input");
      }
      NodeKind::Overlay {
        open,
        style,
        variants,
        ..
      } => {
        let effective = effective_box_style(style, variants, environment, interaction);
        let mut out = taffy::style::Style {
          position: taffy::style::Position::Absolute,
          ..taffy::style::Style::default()
        };

        apply_box_sizing(&mut out, &effective, false);

        if !*open {
          out.display = taffy::style::Display::None;
        }

        taffy
          .set_style(record.taffy, out)
          .expect("style an overlay");
      }
      NodeKind::Group => {
        if fills_root {
          taffy
            .set_style(
              record.taffy,
              taffy::style::Style {
                display: taffy::style::Display::Block,
                flex_grow: 1.0,
                ..taffy::style::Style::default()
              },
            )
            .expect("style a group");
        }
      }
    }
  }
}

/// Paints `node` at its layout position offset by (`offset_x`, `offset_y`), then its
/// children.
// A match dispatch over the node kinds; length is the kind count, not complexity.
#[expect(clippy::too_many_lines)]
fn paint_node(
  state: &State,
  environment: Environment,
  buffer: &mut Buffer,
  node: NodeId,
  offset_x: isize,
  offset_y: isize,
) {
  let (kind_paint, children, x, y, width, height, border) = {
    let nodes = state.nodes.borrow();
    let Some(record) = nodes.get(&node)
    else {
      return;
    };

    let layout = *state
      .taffy
      .borrow()
      .layout(record.taffy)
      .expect("a computed layout");

    #[expect(clippy::cast_possible_truncation)]
    let (x, y) = (
      offset_x + layout.location.x.round() as isize,
      offset_y + layout.location.y.round() as isize,
    );

    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (width, height) = (
      layout.size.width.round().max(0.0) as usize,
      layout.size.height.round().max(0.0) as usize,
    );

    #[expect(clippy::cast_possible_truncation)]
    let border = (
      layout.border.left.round() as isize,
      layout.border.top.round() as isize,
      layout.border.right.round() as isize,
      layout.border.bottom.round() as isize,
    );

    let interaction = Interaction {
      focused: state.focused.get() == Some(node),
      hovered: state.hovered.borrow().contains(&node),
    };

    let focused = interaction.focused;
    let paint = match &record.kind {
      NodeKind::Box {
        style, variants, ..
      } => {
        let effective = effective_box_style(style, variants, environment, interaction);

        NodePaint::Box {
          background: effective.color.and_then(terminal_rgb),
          border: effective.border.map(|border| BorderPaint {
            color: terminal_rgb(border),
          }),
          reverse: focused,
        }
      }
      NodeKind::Run {
        style,
        variants,
        text,
        ..
      } => {
        let effective = effective_run_style(style, variants, environment, interaction);

        NodePaint::Text {
          text: text.clone(),
          foreground: effective.color.and_then(terminal_rgb),
          bold: effective.bold.unwrap_or(false),
          dim: false,
          reverse: false,
        }
      }
      NodeKind::Input {
        style,
        variants,
        value,
        placeholder,
      } => {
        let effective = effective_box_style(style, variants, environment, interaction);
        let (text, dim) = if value.is_empty() {
          (placeholder.clone().unwrap_or_default(), true)
        }
        else {
          (value.clone(), false)
        };

        NodePaint::Input {
          background: effective.color.and_then(terminal_rgb),
          border: effective.border.map(|border| BorderPaint {
            color: terminal_rgb(border),
          }),
          text,
          dim,
          reverse: focused,
        }
      }
      NodeKind::Overlay {
        style, variants, ..
      } => {
        let effective = effective_box_style(style, variants, environment, interaction);

        NodePaint::Box {
          background: effective.color.and_then(terminal_rgb),
          border: effective.border.map(|border| BorderPaint {
            color: terminal_rgb(border),
          }),
          reverse: false,
        }
      }
      NodeKind::Group => NodePaint::None,
    };

    (paint, record.children.clone(), x, y, width, height, border)
  };

  match kind_paint {
    NodePaint::Box {
      background,
      border,
      reverse,
    } => {
      fill(buffer, x, y, width, height, background, reverse);

      if let Some(border) = border {
        draw_border(buffer, x, y, width, height, border.color);
      }
    }
    NodePaint::Text {
      text,
      foreground,
      bold,
      dim,
      reverse,
    } => draw_text(buffer, x, y, &text, foreground, bold, dim, reverse),
    NodePaint::Input {
      background,
      border,
      text,
      dim,
      reverse,
    } => {
      fill(buffer, x, y, width, height, background, reverse);

      let inset = if let Some(border) = border {
        draw_border(buffer, x, y, width, height, border.color);

        1
      }
      else {
        0
      };

      draw_text(
        buffer,
        x + inset,
        y + inset,
        &text,
        None,
        false,
        dim,
        reverse,
      );
    }
    NodePaint::None => {}
  }

  let saved_clip = buffer.clip;

  if state.clips(node) {
    let rect = (
      x + border.0,
      y + border.1,
      x + isize::try_from(width).unwrap_or(isize::MAX) - border.2,
      y + isize::try_from(height).unwrap_or(isize::MAX) - border.3,
    );

    buffer.clip = Some(match saved_clip {
      Some(outer) => intersect(outer, rect),
      None => rect,
    });
  }

  let (scroll_x, scroll_y) = state.scroll_offset(node);
  let (child_x, child_y) = (
    x - isize::try_from(scroll_x).unwrap_or(0),
    y - isize::try_from(scroll_y).unwrap_or(0),
  );

  for child in children {
    if state.is_overlay(child) {
      continue;
    }

    paint_node(state, environment, buffer, child, child_x, child_y);
  }

  buffer.clip = saved_clip;

  if state.is_scrollable(node) {
    draw_scrollbars(state, buffer, node, x, y, width, height, border);
  }
}

/// The intersection of two clip rectangles.
const fn intersect(
  first: (isize, isize, isize, isize),
  second: (isize, isize, isize, isize),
) -> (isize, isize, isize, isize) {
  (
    if first.0 > second.0 {
      first.0
    }
    else {
      second.0
    },
    if first.1 > second.1 {
      first.1
    }
    else {
      second.1
    },
    if first.2 < second.2 {
      first.2
    }
    else {
      second.2
    },
    if first.3 < second.3 {
      first.3
    }
    else {
      second.3
    },
  )
}

/// Overlays scrollbar tracks and thumbs on a scrollable box's right and bottom edges,
/// each drawn only while its axis overflows.
#[expect(clippy::too_many_arguments)]
fn draw_scrollbars(
  state: &State,
  buffer: &mut Buffer,
  node: NodeId,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  border: (isize, isize, isize, isize),
) {
  let (offset_x, offset_y) = state.scroll_offset(node);
  let (limit_x, limit_y) = state.scroll_limits(node);
  let (width, height) = (
    isize::try_from(width).unwrap_or(0),
    isize::try_from(height).unwrap_or(0),
  );

  if limit_y > 0 {
    let track = height - border.1 - border.3;
    let (top, thumb) = thumb_span(track, offset_y, limit_y);

    for row in 0..track {
      if let Some(cell) = buffer.cell_mut(x + width - 1, y + border.1 + row) {
        cell.character = if row >= top && row < top + thumb {
          '█'
        }
        else {
          '░'
        };
      }
    }
  }

  if limit_x > 0 {
    let track = width - border.0 - border.2;
    let (start, thumb) = thumb_span(track, offset_x, limit_x);

    for column in 0..track {
      if let Some(cell) = buffer.cell_mut(x + border.0 + column, y + height - 1) {
        cell.character = if column >= start && column < start + thumb {
          '█'
        }
        else {
          '░'
        };
      }
    }
  }
}

/// A scrollbar thumb's start and length along a track, proportional to the viewport
/// within the scrollable content.
fn thumb_span(track: isize, offset: u32, limit: u32) -> (isize, isize) {
  if track <= 0 {
    return (0, 0);
  }

  let limit = isize::try_from(limit).unwrap_or(0);
  let content = track + limit;
  let thumb = (track * track / content).max(1).min(track);
  let start = (track - thumb) * isize::try_from(offset).unwrap_or(0) / limit.max(1);

  (start, thumb)
}

/// A border's paint: present, with an optional explicit color.
struct BorderPaint {
  color: Option<(u8, u8, u8)>,
}

/// What one node contributes to the frame.
enum NodePaint {
  Box {
    background: Option<(u8, u8, u8)>,
    border: Option<BorderPaint>,
    reverse: bool,
  },
  Text {
    text: String,
    foreground: Option<(u8, u8, u8)>,
    bold: bool,
    dim: bool,
    reverse: bool,
  },
  Input {
    background: Option<(u8, u8, u8)>,
    border: Option<BorderPaint>,
    text: String,
    dim: bool,
    reverse: bool,
  },
  None,
}

/// Fills a rectangle's background and focus attribute.
fn fill(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  background: Option<(u8, u8, u8)>,
  reverse: bool,
) {
  if background.is_none() && !reverse {
    return;
  }

  for row in 0..height {
    for column in 0..width {
      if let Some(cell) = buffer.cell_mut(
        x + isize::try_from(column).expect("fits"),
        y + isize::try_from(row).expect("fits"),
      ) {
        if background.is_some() {
          cell.background = background;
        }

        cell.reverse = reverse;
      }
    }
  }
}

/// Draws a single-line box border with box-drawing characters.
fn draw_border(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  color: Option<(u8, u8, u8)>,
) {
  if width < 2 || height < 2 {
    return;
  }

  let right = x + isize::try_from(width - 1).expect("fits");
  let bottom = y + isize::try_from(height - 1).expect("fits");
  let mut put = |cx: isize, cy: isize, character: char| {
    if let Some(cell) = buffer.cell_mut(cx, cy) {
      cell.character = character;

      if color.is_some() {
        cell.foreground = color;
      }
    }
  };

  for column in 1..width - 1 {
    let cx = x + isize::try_from(column).expect("fits");

    put(cx, y, '─');
    put(cx, bottom, '─');
  }

  for row in 1..height - 1 {
    let cy = y + isize::try_from(row).expect("fits");

    put(x, cy, '│');
    put(right, cy, '│');
  }

  put(x, y, '┌');
  put(right, y, '┐');
  put(x, bottom, '└');
  put(right, bottom, '┘');
}

/// Draws one line of text.
#[expect(clippy::too_many_arguments)]
fn draw_text(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  text: &str,
  foreground: Option<(u8, u8, u8)>,
  bold: bool,
  dim: bool,
  reverse: bool,
) {
  for (index, character) in text.chars().enumerate() {
    let Some(cell) = buffer.cell_mut(x + isize::try_from(index).expect("fits"), y)
    else {
      continue;
    };

    cell.character = character;

    if foreground.is_some() {
      cell.foreground = foreground;
    }
    else if dim {
      cell.foreground = Some((128, 128, 128));
    }

    cell.bold = bold;

    if reverse {
      cell.reverse = true;
    }
  }
}
