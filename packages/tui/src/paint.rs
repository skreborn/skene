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
}

impl Buffer {
  fn new(width: usize, height: usize) -> Self {
    Self {
      width,
      height,
      cells: vec![TerminalCell::EMPTY; width * height],
    }
  }

  fn cell_mut(&mut self, x: isize, y: isize) -> Option<&mut TerminalCell> {
    if x < 0 || y < 0 {
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
pub(crate) fn render(state: &State, width: u16, height: u16) -> Buffer {
  let environment = Environment {
    columns: width,
    rows: height,
  };

  refresh_styles(state, environment);

  {
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

  let mut buffer = Buffer::new(usize::from(width), usize::from(height));

  paint_node(state, environment, &mut buffer, ROOT_ID, 0, 0);

  buffer
}

/// Refreshes every node's taffy style from its effective style in `environment`.
fn refresh_styles(state: &State, environment: Environment) {
  let nodes = state.nodes.borrow();
  let focused = state.focused.get();
  let mut taffy = state.taffy.borrow_mut();

  for (id, record) in nodes.iter() {
    let node_focused = focused == Some(*id);
    // The single mounted child fills the viewport, as the web reset's `body > *` does.
    let fills_root = record.parent == Some(ROOT_ID);

    match &record.kind {
      NodeKind::Box {
        layout,
        style,
        variants,
      } => {
        let effective = effective_box_style(style, variants, environment, node_focused);
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
        let effective = effective_box_style(style, variants, environment, node_focused);
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
  let (kind_paint, children, x, y, width, height) = {
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

    let focused = state.focused.get() == Some(node);
    let paint = match &record.kind {
      NodeKind::Box {
        style, variants, ..
      } => {
        let effective = effective_box_style(style, variants, environment, focused);

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
        let effective = effective_run_style(style, variants, environment, focused);

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
        let effective = effective_box_style(style, variants, environment, focused);
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
      NodeKind::Group => NodePaint::None,
    };

    (paint, record.children.clone(), x, y, width, height)
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

  for child in children {
    paint_node(state, environment, buffer, child, x, y);
  }
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
