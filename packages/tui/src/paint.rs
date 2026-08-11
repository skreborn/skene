//! Layout and painting into a cell buffer.
//!
//! Each render refreshes every node's taffy style from its effective style (base plus
//! the variants holding in the current environment), computes the layout, and paints the
//! tree back-to-front into a [`Buffer`]—which serializes to plain text for tests or to
//! the terminal for the interactive loop.

use skene_core::{Color, NodeId};
use taffy::prelude::{AvailableSpace, length};

use crate::{
  state::{NodeKind, NodeRecord, ROOT_ID, State},
  style::{
    Environment,
    Interaction,
    apply_box_sizing,
    box_taffy_style,
    effective_box_style,
    effective_control_style,
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

  draw_menu(state, &mut buffer, environment.dark);

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
    .compute_layout_with_measure(
      root_taffy,
      taffy::geometry::Size {
        width: AvailableSpace::Definite(f32::from(width)),
        height: AvailableSpace::Definite(f32::from(height)),
      },
      |known, available, _, text, _| measure_run(known, available, text),
    )
    .expect("compute the layout");
}

/// Measures a text run's leaf: its text wrapped at the width constraint, in cells.
///
/// A leaf carrying no text measures to its known dimensions or zero.
fn measure_run(
  known: taffy::geometry::Size<Option<f32>>,
  available: taffy::geometry::Size<AvailableSpace>,
  text: Option<&mut String>,
) -> taffy::geometry::Size<f32> {
  let Some(text) = text
  else {
    return taffy::geometry::Size {
      width: known.width.unwrap_or(0.0),
      height: known.height.unwrap_or(0.0),
    };
  };

  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  let limit = match (known.width, available.width) {
    (Some(width), _) | (None, AvailableSpace::Definite(width)) => width.max(0.0) as usize,
    (None, AvailableSpace::MinContent) => longest_word(text),
    (None, AvailableSpace::MaxContent) => usize::MAX,
  };
  let lines = wrap_text(text, limit);

  #[expect(clippy::cast_precision_loss)]
  taffy::geometry::Size {
    width: known.width.unwrap_or_else(|| {
      lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0) as f32
    }),
    height: known.height.unwrap_or(lines.len() as f32),
  }
}

/// Wraps text to `width` columns: lines break at spaces, a word longer than a full
/// line breaks mid-word, and a newline always breaks. Yields at least one line, each
/// paired with its start offset into the source, in characters, newlines included.
pub(crate) fn wrap_spans(text: &str, width: usize) -> Vec<(usize, String)> {
  let width = width.max(1);
  let mut lines = Vec::new();
  let mut base = 0;

  for paragraph in text.split('\n') {
    let characters: Vec<char> = paragraph.chars().collect();
    let mut start = 0;

    loop {
      if characters.len() - start <= width {
        lines.push((base + start, characters[start..].iter().collect()));

        break;
      }

      let window = &characters[start..start + width];
      let break_at = if characters[start + width] == ' ' {
        width
      }
      else {
        match window.iter().rposition(|&character| character == ' ') {
          Some(position) if position > 0 => position,
          _ => width,
        }
      };

      lines.push((
        base + start,
        characters[start..start + break_at].iter().collect(),
      ));
      start += break_at;

      while characters.get(start) == Some(&' ') {
        start += 1;
      }

      if start == characters.len() {
        break;
      }
    }

    base += characters.len() + 1;
  }

  lines
}

/// Wraps text to `width` columns; the lines of [`wrap_spans`], offsets dropped.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
  wrap_spans(text, width)
    .into_iter()
    .map(|(_, line)| line)
    .collect()
}

/// The first visible column of an input's value: zero until the caret passes the inner
/// width, then sliding to keep the caret on the last cell.
pub(crate) const fn field_window(caret: usize, inner: usize) -> usize {
  caret.saturating_sub(inner.saturating_sub(1))
}

/// The longest unbreakable word's width in cells: the narrowest a wrap can go.
fn longest_word(text: &str) -> usize {
  text
    .split([' ', '\n'])
    .map(|word| word.chars().count())
    .max()
    .unwrap_or(0)
}

/// Paints one open overlay over the frame: its backdrop scrim, then its subtree at its
/// computed origin.
fn paint_overlay(state: &State, environment: Environment, buffer: &mut Buffer, overlay: NodeId) {
  let backdrop = {
    let nodes = state.nodes.borrow();

    match nodes.get(&overlay).map(|record| &record.kind) {
      Some(NodeKind::Overlay { backdrop, .. }) => *backdrop,
      _ => return,
    }
  };

  if let Some(color) = backdrop {
    let [red, green, blue, alpha] = color.to_rgba();

    if alpha > 0.0 {
      for cell in &mut buffer.cells {
        tint(cell, red, green, blue, alpha, environment.dark);
      }
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

/// Applies a node's wrapper annotations—and its stack membership—to its taffy style,
/// for a kind that carries no box style of its own.
fn apply_annotations(out: &mut taffy::style::Style, record: &NodeRecord) {
  if let Some(annotations) = &record.annotations {
    apply_box_sizing(out, annotations, record.in_stack);
  }
  else if record.in_stack {
    apply_box_sizing(out, &crate::props::BoxStyle::default(), true);
  }
}

/// Refreshes every node's taffy style from its effective style in `environment`.
// A match dispatch over the node kinds; length is the kind count, not complexity.
#[expect(clippy::too_many_lines)]
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

          taffy
            .set_node_context(record.taffy, None)
            .expect("clear a run's text");
        }
        else {
          // The leaf stays auto-sized; the measure function wraps the text.
          taffy
            .set_node_context(record.taffy, Some(text.clone()))
            .expect("carry a run's text");
        }

        apply_annotations(&mut out, record);
        taffy.set_style(record.taffy, out).expect("style a run");
      }
      NodeKind::TextField {
        style,
        variants,
        value,
        placeholder,
        ..
      } => {
        let effective = effective_box_style(style, variants, environment, interaction);
        let mut out = taffy::style::Style::default();

        apply_box_sizing(&mut out, &effective, record.in_stack);

        if effective.width.is_none() {
          // One cell past the value keeps the caret inside the box at the end.
          #[expect(clippy::cast_precision_loss)]
          let content = (value.chars().count() + 1)
            .max(placeholder.as_ref().map_or(0, |text| text.chars().count()))
            .max(8) as f32;

          out.size.width = length(content);
        }

        if effective.height.is_none() {
          out.size.height = length(if effective.border.is_some() { 3.0 } else { 1.0 });
        }

        taffy.set_style(record.taffy, out).expect("style an input");
      }
      NodeKind::Checkbox { marks, label, .. } | NodeKind::Radio { marks, label, .. } => {
        let mut out = taffy::style::Style::default();

        apply_annotations(&mut out, record);

        #[expect(clippy::cast_precision_loss)]
        let width = (marks.0.chars().count().max(marks.1.chars().count())
          + label.as_ref().map_or(0, |label| label.chars().count() + 1)) as f32;

        out.size = taffy::geometry::Size {
          width: length(width),
          height: length(1.0),
        };

        taffy.set_style(record.taffy, out).expect("style a control");
      }
      NodeKind::Picker { options, .. } => {
        let mut out = taffy::style::Style::default();

        apply_annotations(&mut out, record);

        #[expect(clippy::cast_precision_loss)]
        let longest = options
          .iter()
          .map(|(_, label)| label.chars().count())
          .max()
          .unwrap_or(0) as f32;

        out.size = taffy::geometry::Size {
          width: length(longest + 2.0),
          height: length(1.0),
        };

        taffy.set_style(record.taffy, out).expect("style a control");
      }
      NodeKind::Slider { .. } => {
        // The track fills its container's width; wrap in a sized `Container` to bound
        // it.
        let mut out = taffy::style::Style::default();

        apply_annotations(&mut out, record);

        out.size = taffy::geometry::Size {
          width: taffy::prelude::percent(1.0),
          height: length(1.0),
        };

        taffy.set_style(record.taffy, out).expect("style a control");
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
        else if record.in_stack || record.annotations.is_some() {
          let mut out = taffy::style::Style::default();

          apply_annotations(&mut out, record);
          taffy.set_style(record.taffy, out).expect("style a group");
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
            glyphs: effective.border_glyphs.unwrap_or_default(),
            color: terminal_rgb(border),
          }),
          shadow: effective.shadow,
          reverse: focused,
        }
      }
      NodeKind::Run {
        style,
        variants,
        text,
        selectable,
      } => {
        let effective = effective_run_style(style, variants, environment, interaction);
        let spans = wrap_spans(text, width);
        let selection = selectable
          .then(|| {
            state.selection_range(node).map(|range| SelectionPaint {
              range,
              starts: spans.iter().map(|(start, _)| *start).collect(),
              colors: selection_colors(&effective, environment.dark),
            })
          })
          .flatten();

        NodePaint::Text {
          lines: spans.into_iter().map(|(_, line)| line).collect(),
          foreground: effective.color.and_then(terminal_rgb),
          bold: effective.bold.unwrap_or(false),
          dim: false,
          reverse: false,
          selection,
        }
      }
      NodeKind::TextField {
        style,
        variants,
        value,
        placeholder,
        caret,
      } => {
        let effective = effective_box_style(style, variants, environment, interaction);
        let (text, dim) = if value.is_empty() {
          (placeholder.clone().unwrap_or_default(), true)
        }
        else {
          (value.clone(), false)
        };

        let inset = usize::from(effective.border.is_some());
        let inner = width.saturating_sub(2 * inset);
        let window = field_window(*caret, inner);
        let text: String = text.chars().skip(window).take(inner).collect();

        NodePaint::TextField {
          background: effective.color.and_then(terminal_rgb),
          border: effective.border.map(|border| BorderPaint {
            glyphs: effective.border_glyphs.unwrap_or_default(),
            color: terminal_rgb(border),
          }),
          shadow: effective.shadow,
          text,
          dim,
          caret: (focused && inner > 0).then(|| caret - window),
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
            glyphs: effective.border_glyphs.unwrap_or_default(),
            color: terminal_rgb(border),
          }),
          shadow: effective.shadow,
          reverse: false,
        }
      }
      NodeKind::Checkbox {
        style,
        variants,
        marks,
        label,
        checked,
      }
      | NodeKind::Radio {
        style,
        variants,
        marks,
        label,
        checked,
        ..
      } => {
        let effective = effective_control_style(style, variants, environment, interaction);
        let mark = if *checked { &marks.0 } else { &marks.1 };
        let text = match label {
          Some(label) => format!("{mark} {label}"),
          None => mark.clone(),
        };

        NodePaint::Text {
          lines: vec![text],
          foreground: effective.color.and_then(terminal_rgb),
          bold: effective.bold.unwrap_or(false),
          dim: false,
          reverse: focused,
          selection: None,
        }
      }
      NodeKind::Picker {
        style,
        variants,
        options,
        indicator,
        value,
      } => {
        let effective = effective_control_style(style, variants, environment, interaction);
        let label = options
          .iter()
          .find(|(option, _)| option == value)
          .map_or_else(|| value.clone(), |(_, label)| label.clone());

        NodePaint::Text {
          lines: vec![format!("{label} {indicator}")],
          foreground: effective.color.and_then(terminal_rgb),
          bold: effective.bold.unwrap_or(false),
          dim: false,
          reverse: focused,
          selection: None,
        }
      }
      NodeKind::Slider {
        style,
        variants,
        glyphs,
        min,
        max,
        value,
        ..
      } => {
        let effective = effective_control_style(style, variants, environment, interaction);

        NodePaint::Text {
          lines: vec![slider_track(width, *min, *max, *value, *glyphs)],
          foreground: effective.color.and_then(terminal_rgb),
          bold: effective.bold.unwrap_or(false),
          dim: false,
          reverse: focused,
          selection: None,
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
      shadow,
      reverse,
    } => {
      if let Some(shadow) = shadow {
        draw_shadow(buffer, x, y, width, height, shadow, environment.dark);
      }

      fill(buffer, x, y, width, height, background, reverse);

      if let Some(border) = border {
        draw_border(buffer, x, y, width, height, border.glyphs, border.color);
      }
    }
    NodePaint::Text {
      lines,
      foreground,
      bold,
      dim,
      reverse,
      selection,
    } => {
      for (row, line) in lines.iter().enumerate() {
        draw_text(
          buffer,
          x,
          y + isize::try_from(row).expect("fits"),
          line,
          foreground,
          bold,
          dim,
          reverse,
        );
      }

      if let Some(selection) = selection {
        draw_selection(buffer, x, y, &lines, &selection);
      }
    }
    NodePaint::TextField {
      background,
      border,
      shadow,
      text,
      dim,
      caret,
      reverse,
    } => {
      if let Some(shadow) = shadow {
        draw_shadow(buffer, x, y, width, height, shadow, environment.dark);
      }

      fill(buffer, x, y, width, height, background, reverse);

      let inset = if let Some(border) = border {
        draw_border(buffer, x, y, width, height, border.glyphs, border.color);

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

      // The block caret: the one cell flipped back out of the focused reverse video.
      if let Some(column) = caret
        && let Some(cell) = buffer.cell_mut(
          x + inset + isize::try_from(column).expect("fits"),
          y + inset,
        )
      {
        cell.reverse = false;
      }
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

  let dark = state.dark.get();

  if limit_y > 0 {
    let track = height - border.1 - border.3;
    let (top, thumb) = thumb_span_eighths(track, offset_y, limit_y);

    for row in 0..track {
      if let Some(cell) = buffer.cell_mut(x + width - 1, y + border.1 + row) {
        let (character, reverse) = match eighth_coverage(row, top, thumb) {
          Coverage::Full => ('█', false),
          Coverage::Leading(eighths) => (lower_block(8 - eighths), true),
          Coverage::Trailing(eighths) => (lower_block(eighths), false),
          Coverage::None => (' ', false),
        };

        paint_track_cell(cell, dark);

        cell.character = character;
        cell.reverse = reverse;
      }
    }
  }

  if limit_x > 0 {
    let track = width - border.0 - border.2;
    let (start, thumb) = thumb_span_eighths(track, offset_x, limit_x);

    for column in 0..track {
      if let Some(cell) = buffer.cell_mut(x + border.0 + column, y + height - 1) {
        let (character, reverse) = match eighth_coverage(column, start, thumb) {
          Coverage::Full => ('█', false),
          Coverage::Leading(eighths) => (left_block(eighths), false),
          Coverage::Trailing(eighths) => (left_block(8 - eighths), true),
          Coverage::None => (' ', false),
        };

        paint_track_cell(cell, dark);

        cell.character = character;
        cell.reverse = reverse;
      }
    }
  }
}

/// Colors one scrollbar cell: the thumb inks in the terminal default foreground for the
/// detected background, the track is a quarter of that ink over the cell beneath—so a
/// partially covered cell's remainder matches the track exactly.
fn paint_track_cell(cell: &mut TerminalCell, dark: bool) {
  let (ink, base) = if dark {
    ((255, 255, 255), (0, 0, 0))
  }
  else {
    ((0, 0, 0), (255, 255, 255))
  };
  let beneath = cell.background.unwrap_or(base);
  let quarter = |over: u8, under: u8| -> u8 {
    u8::try_from((u16::from(over) + 3 * u16::from(under) + 2) / 4).unwrap_or(u8::MAX)
  };

  cell.foreground = Some(ink);
  cell.background = Some((
    quarter(ink.0, beneath.0),
    quarter(ink.1, beneath.1),
    quarter(ink.2, beneath.2),
  ));
  cell.bold = false;
}

/// How much of a track cell the thumb covers, in eighth-cell resolution.
enum Coverage {
  Full,
  /// The leading eighths only: the thumb ends mid-cell.
  Leading(u8),
  /// The trailing eighths only: the thumb starts mid-cell.
  Trailing(u8),
  None,
}

/// The thumb's coverage of the track cell at `position`, against an eighth-cell span.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
const fn eighth_coverage(position: isize, start_eighth: isize, thumb_eighths: isize) -> Coverage {
  let cell_start = position * 8;
  let cell_end = cell_start + 8;
  let end_eighth = start_eighth + thumb_eighths;
  let from = if start_eighth > cell_start {
    start_eighth
  }
  else {
    cell_start
  };
  let to = if end_eighth < cell_end {
    end_eighth
  }
  else {
    cell_end
  };

  if to - from <= 0 {
    return Coverage::None;
  }

  if to - from >= 8 {
    return Coverage::Full;
  }

  if start_eighth > cell_start {
    Coverage::Trailing((to - from) as u8)
  }
  else {
    Coverage::Leading((to - from) as u8)
  }
}

/// The block character filling a cell's lowest `eighths`/8, `eighths` in `1..=7`.
const fn lower_block(eighths: u8) -> char {
  match eighths {
    1 => '▁',
    2 => '▂',
    3 => '▃',
    4 => '▄',
    5 => '▅',
    6 => '▆',
    _ => '▇',
  }
}

/// The block character filling a cell's leftmost `eighths`/8, `eighths` in `1..=7`.
const fn left_block(eighths: u8) -> char {
  match eighths {
    1 => '▏',
    2 => '▎',
    3 => '▍',
    4 => '▌',
    5 => '▋',
    6 => '▊',
    _ => '▉',
  }
}

/// A scrollbar thumb's start and length along a track, proportional to the viewport
/// within the scrollable content, in whole cells. The grab arithmetic works at this
/// resolution; the paint refines to eighths.
pub(crate) fn thumb_span(track: isize, offset: u32, limit: u32) -> (isize, isize) {
  if track <= 0 {
    return (0, 0);
  }

  let limit = isize::try_from(limit).unwrap_or(0);
  let content = track + limit;
  let thumb = (track * track / content).max(1).min(track);
  let start = (track - thumb) * isize::try_from(offset).unwrap_or(0) / limit.max(1);

  (start, thumb)
}

/// [`thumb_span`] at eighth-cell resolution, for the smooth paint.
fn thumb_span_eighths(track: isize, offset: u32, limit: u32) -> (isize, isize) {
  if track <= 0 {
    return (0, 0);
  }

  let eighths = track * 8;
  let limit = isize::try_from(limit).unwrap_or(0);
  let content = track + limit;
  let thumb = (eighths * track / content).max(8).min(eighths);
  let start = (eighths - thumb) * isize::try_from(offset).unwrap_or(0) / limit.max(1);

  (start, thumb)
}

/// A border's paint: present, with its character set and an optional explicit color.
struct BorderPaint {
  glyphs: crate::BorderGlyphs,
  color: Option<(u8, u8, u8)>,
}

/// What one node contributes to the frame.
/// A run's selection highlight: the selected source range, each painted line's start
/// offset in the source, and the ink and highlight colors.
struct SelectionPaint {
  range: (usize, usize),
  starts: Vec<usize>,
  colors: ((u8, u8, u8), (u8, u8, u8)),
}

enum NodePaint {
  Box {
    background: Option<(u8, u8, u8)>,
    border: Option<BorderPaint>,
    shadow: Option<Color>,
    reverse: bool,
  },
  Text {
    lines: Vec<String>,
    foreground: Option<(u8, u8, u8)>,
    bold: bool,
    dim: bool,
    reverse: bool,
    selection: Option<SelectionPaint>,
  },
  TextField {
    background: Option<(u8, u8, u8)>,
    border: Option<BorderPaint>,
    shadow: Option<Color>,
    text: String,
    dim: bool,
    caret: Option<usize>,
    reverse: bool,
  },
  None,
}

/// Fills a rectangle: a background paints opaquely, clearing whatever was beneath;
/// the focus attribute applies either way.
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
          cell.character = ' ';
          cell.foreground = None;
          cell.bold = false;
          cell.background = background;
        }

        cell.reverse = reverse;
      }
    }
  }
}

/// Draws a one-cell box border with the given character set.
fn draw_border(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  glyphs: crate::BorderGlyphs,
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

    put(cx, y, glyphs.horizontal);
    put(cx, bottom, glyphs.horizontal);
  }

  for row in 1..height - 1 {
    let cy = y + isize::try_from(row).expect("fits");

    put(x, cy, glyphs.vertical);
    put(right, cy, glyphs.vertical);
  }

  put(x, y, glyphs.top_left);
  put(right, y, glyphs.top_right);
  put(x, bottom, glyphs.bottom_left);
  put(right, bottom, glyphs.bottom_right);
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

/// The selection ink and highlight for a dark terminal, when no theme sets them.
const DARK_SELECTION: ((u8, u8, u8), (u8, u8, u8)) = ((255, 255, 255), (52, 88, 134));

/// The selection ink and highlight for a light terminal, when no theme sets them.
const LIGHT_SELECTION: ((u8, u8, u8), (u8, u8, u8)) = ((0, 0, 0), (174, 208, 250));

/// A run's selection ink and highlight: its effective style's, or the defaults for the
/// detected background.
fn selection_colors(style: &crate::props::RunStyle, dark: bool) -> ((u8, u8, u8), (u8, u8, u8)) {
  let default = if dark {
    DARK_SELECTION
  }
  else {
    LIGHT_SELECTION
  };

  (
    style
      .selection_color
      .and_then(terminal_rgb)
      .unwrap_or(default.0),
    style
      .selection_background
      .and_then(terminal_rgb)
      .unwrap_or(default.1),
  )
}

/// Recolors a run's selected cells: the selection ink over its highlight.
fn draw_selection(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  lines: &[String],
  selection: &SelectionPaint,
) {
  let (from, to) = selection.range;

  for (row, line) in lines.iter().enumerate() {
    let start = selection.starts[row];

    for (column, _) in line.chars().enumerate() {
      let offset = start + column;

      if offset < from || offset >= to {
        continue;
      }

      if let Some(cell) = buffer.cell_mut(
        x + isize::try_from(column).expect("fits"),
        y + isize::try_from(row).expect("fits"),
      ) {
        cell.foreground = Some(selection.colors.0);
        cell.background = Some(selection.colors.1);
        cell.reverse = false;
      }
    }
  }
}

/// Paints the copy menu over everything at its anchored cell, the `Copy` item
/// highlighted while the pointer is on it.
fn draw_menu(state: &State, buffer: &mut Buffer, dark: bool) {
  let Some((menu_x, menu_y, menu_width, menu_height)) = state.menu_bounds()
  else {
    return;
  };

  let (x, y) = (
    isize::try_from(menu_x).unwrap_or(0),
    isize::try_from(menu_y).unwrap_or(0),
  );
  let (width, height) = (
    usize::try_from(menu_width).unwrap_or(0),
    usize::try_from(menu_height).unwrap_or(0),
  );
  let backdrop = if dark { (0, 0, 0) } else { (255, 255, 255) };

  fill(buffer, x, y, width, height, Some(backdrop), false);
  draw_border(
    buffer,
    x,
    y,
    width,
    height,
    crate::BorderGlyphs::default(),
    Some((128, 128, 128)),
  );

  let (pointer_x, pointer_y) = state.last_pointer.get();
  let hovered = state.copy_hit(pointer_x, pointer_y);
  let (ink, highlight) = if dark {
    DARK_SELECTION
  }
  else {
    LIGHT_SELECTION
  };

  if hovered {
    for column in 1..width.saturating_sub(1) {
      if let Some(cell) = buffer.cell_mut(x + isize::try_from(column).expect("fits"), y + 1) {
        cell.background = Some(highlight);
      }
    }
  }

  draw_text(
    buffer,
    x + 2,
    y + 1,
    crate::selection::MENU_ITEM,
    hovered.then_some(ink),
    false,
    false,
    false,
  );
}

/// A slider's track over `width` cells, the thumb at the value's cell.
fn slider_track(width: usize, min: f32, max: f32, value: f32, glyphs: (char, char)) -> String {
  if width == 0 {
    return String::new();
  }

  let span = max - min;
  let fraction = if span > 0.0 {
    ((value - min) / span).clamp(0.0, 1.0)
  }
  else {
    0.0
  };

  #[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
  )]
  let thumb = (fraction * (width - 1) as f32).round() as usize;

  (0..width)
    .map(|cell| if cell == thumb { glyphs.1 } else { glyphs.0 })
    .collect()
}

/// Casts a box's shadow: the one-cell band below and to the right, the shadow color
/// mixed into both the foreground and the background of each covered cell by its
/// alpha. An unset color beneath mixes against the terminal's default for the detected
/// background.
fn draw_shadow(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  color: Color,
  dark: bool,
) {
  let [red, green, blue, alpha] = color.to_rgba();
  let alpha = alpha.clamp(0.0, 1.0);

  if alpha <= 0.0 || width == 0 || height == 0 {
    return;
  }

  let (width, height) = (
    isize::try_from(width).unwrap_or(0),
    isize::try_from(height).unwrap_or(0),
  );
  let mut cast = |cell_x: isize, cell_y: isize| {
    if let Some(cell) = buffer.cell_mut(cell_x, cell_y) {
      tint(cell, red, green, blue, alpha, dark);
    }
  };

  for row in 1..=height {
    cast(x + width, y + row);
  }

  for column in 1..width {
    cast(x + column, y + height);
  }
}

/// Mixes a color into both of a cell's colors by `alpha`, an unset color mixing
/// against the terminal default for the detected `dark` background.
fn tint(cell: &mut TerminalCell, red: f32, green: f32, blue: f32, alpha: f32, dark: bool) {
  let alpha = alpha.clamp(0.0, 1.0);
  let (foreground_base, background_base) = if dark {
    ((255, 255, 255), (0, 0, 0))
  }
  else {
    ((0, 0, 0), (255, 255, 255))
  };
  let mix = |existing: Option<(u8, u8, u8)>, base: (u8, u8, u8)| -> (u8, u8, u8) {
    let (base_red, base_green, base_blue) = existing.unwrap_or(base);
    let channel = |over: f32, beneath: u8| -> u8 {
      #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
      let mixed = (over.clamp(0.0, 1.0) * 255.0)
        .mul_add(alpha, f32::from(beneath) * (1.0 - alpha))
        .round() as u8;

      mixed
    };

    (
      channel(red, base_red),
      channel(green, base_green),
      channel(blue, base_blue),
    )
  };

  cell.foreground = Some(mix(cell.foreground, foreground_base));
  cell.background = Some(mix(cell.background, background_base));
}

/// Whether `row` must be rewritten: the previous frame is absent, differently sized, or
/// differs in that row.
pub(crate) fn row_changed(previous: Option<&Buffer>, next: &Buffer, row: usize) -> bool {
  match previous {
    Some(previous) if previous.width == next.width && previous.height == next.height => {
      previous.cells[row * previous.width..(row + 1) * previous.width]
        != next.cells[row * next.width..(row + 1) * next.width]
    }
    _ => true,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rows_diff_against_the_previous_frame() {
    let mut previous = Buffer::new(3, 2);
    let mut next = Buffer::new(3, 2);

    previous.cells[1].character = 'a';
    next.cells[1].character = 'a';
    next.cells[4].character = 'b';

    assert!(!row_changed(Some(&previous), &next, 0));
    assert!(row_changed(Some(&previous), &next, 1));
    assert!(row_changed(None, &next, 0));

    let resized = Buffer::new(2, 2);

    assert!(row_changed(Some(&resized), &next, 0));
  }

  #[test]
  fn wrapping_breaks_at_spaces_words_and_newlines() {
    assert_eq!(wrap_text("alpha beta gamma", 8), ["alpha", "beta", "gamma"]);
    assert_eq!(wrap_text("alpha beta gamma", 11), ["alpha beta", "gamma"]);
    assert_eq!(wrap_text("abcdefgh", 3), ["abc", "def", "gh"]);
    assert_eq!(wrap_text("ab\ncd", 10), ["ab", "cd"]);
    assert_eq!(wrap_text("", 5), [""]);
    assert_eq!(wrap_text("ab  ", 2), ["ab"]);
  }

  #[test]
  fn slider_tracks_place_the_thumb() {
    let glyphs = ('-', '*');

    assert_eq!(slider_track(5, 0.0, 4.0, 0.0, glyphs), "*----");
    assert_eq!(slider_track(5, 0.0, 4.0, 2.0, glyphs), "--*--");
    assert_eq!(slider_track(5, 0.0, 4.0, 4.0, glyphs), "----*");
  }
}
