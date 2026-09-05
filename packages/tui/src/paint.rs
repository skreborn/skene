//! Painting the laid-out tree into a cell buffer.
//!
//! The tree paints back-to-front into a [`Buffer`], overlays in a second pass over
//! it; the buffer serializes to plain text for tests or to the terminal for the
//! interactive loop.

use skene_core::{Color, next::ControlKind};

use crate::{
  BorderGlyphs,
  layout::{is_paragraph, paragraph_runs, paragraph_text},
  store::{Handle, Kind, PlainRun, Record, Store},
};

/// One terminal cell's paint state.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct TerminalCell {
  pub(crate) character: char,
  pub(crate) foreground: Option<(u8, u8, u8)>,
  pub(crate) background: Option<(u8, u8, u8)>,
  pub(crate) bold: bool,
  pub(crate) underline: bool,
  /// Whether the cell paints in reverse video.
  pub(crate) reverse: bool,
  /// The cell's hyperlink, an index into the frame's [`Buffer::links`].
  pub(crate) link: Option<u16>,
}

impl TerminalCell {
  pub(crate) const EMPTY: Self = Self {
    character: ' ',
    foreground: None,
    background: None,
    bold: false,
    underline: false,
    reverse: false,
    link: None,
  };
}

/// A painted frame: `width` x `height` cells in row-major order.
pub(crate) struct Buffer {
  pub(crate) width: usize,
  pub(crate) height: usize,
  pub(crate) cells: Vec<TerminalCell>,
  /// The focused entry's caret cell, shown as the terminal's own bar cursor.
  pub(crate) caret: Option<(u16, u16)>,
  /// The frame's hyperlink destinations, indexed by [`TerminalCell::link`], emitted as
  /// OSC 8 anchors.
  pub(crate) links: Vec<String>,
  /// The active clip rectangle (start inclusive, end exclusive), while a clipping box
  /// paints its children.
  clip: Option<(isize, isize, isize, isize)>,
}

impl Buffer {
  pub(crate) fn new(width: usize, height: usize) -> Self {
    Self {
      width,
      height,
      cells: vec![TerminalCell::EMPTY; width * height],
      caret: None,
      links: Vec::new(),
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
pub(crate) fn wrap_text(text: &str, width: usize) -> Vec<String> {
  wrap_spans(text, width)
    .into_iter()
    .map(|(_, line)| line)
    .collect()
}

/// The first visible column of an entry's value: zero until the caret passes the inner
/// width, then sliding to keep the caret on the last cell.
pub(crate) const fn field_window(caret: usize, inner: usize) -> usize {
  caret.saturating_sub(inner.saturating_sub(1))
}

/// The longest unbreakable word's width in cells: the narrowest a wrap can go.
pub(crate) fn longest_word(text: &str) -> usize {
  text
    .split([' ', '\n'])
    .map(|word| word.chars().count())
    .max()
    .unwrap_or(0)
}

/// Paints the whole frame: the tree, then every open overlay, then the copy menu.
pub(crate) fn paint(store: &Store, width: u16, height: u16) -> Buffer {
  let mut buffer = Buffer::new(usize::from(width), usize::from(height));

  paint_node(store, &mut buffer, store.root, 0, 0);

  for overlay in store.open_overlays() {
    paint_overlay(store, &mut buffer, overlay);
  }

  draw_menu(store, &mut buffer, store.dark());

  buffer
}

/// Paints one open overlay over the frame: its backdrop scrim, then its subtree at its
/// computed origin.
fn paint_overlay(store: &Store, buffer: &mut Buffer, overlay: Handle) {
  let Some(Kind::Overlay {
    backdrop, modal, ..
  }) = store.record(overlay).map(|record| &record.kind)
  else {
    return;
  };
  let dark = store.dark();

  if let Some(color) = backdrop {
    let [red, green, blue, alpha] = color.to_rgba();

    if alpha > 0. {
      for cell in &mut buffer.cells {
        tint(cell, red, green, blue, alpha, dark);
      }
    }
  }

  let (origin_x, origin_y) = store.overlay_origin(overlay);

  // A modal makes everything beneath inert, taking the caret with it; a popup hides
  // the caret only when it covers the cell. Either way the overlay's own content may
  // set a fresh one while it paints.
  if let Some((caret_x, caret_y)) = buffer.caret {
    let (overlay_width, overlay_height) = store.node_size(overlay);
    let covered = i64::from(caret_x) >= origin_x
      && i64::from(caret_x) < origin_x + overlay_width
      && i64::from(caret_y) >= origin_y
      && i64::from(caret_y) < origin_y + overlay_height;

    if *modal || covered {
      buffer.caret = None;
    }
  }

  let (location_x, location_y) = store.node_location(overlay);

  paint_node(
    store,
    buffer,
    overlay,
    isize::try_from(origin_x - location_x).unwrap_or(0),
    isize::try_from(origin_y - location_y).unwrap_or(0),
  );
}

/// A selection's ink and highlight.
type SelectionColors = ((u8, u8, u8), (u8, u8, u8));

/// A border's paint: its character set and its color—`None` for a fully transparent
/// border, which keeps its cells but paints no glyphs.
struct BorderPaint {
  glyphs: BorderGlyphs,
  color: Option<(u8, u8, u8)>,
}

/// One paragraph span's paint: its ink, weight, underline, and link.
struct SpanPaint {
  foreground: Option<(u8, u8, u8)>,
  bold: bool,
  underline: bool,
  link: Option<u16>,
  selection: Option<SelectionColors>,
}

/// A run's selection highlight: the selected source range, each painted line's start
/// offset in the source, and the ink and highlight colors, `None` for reverse video.
struct SelectionPaint {
  range: (usize, usize),
  starts: Vec<usize>,
  colors: Option<SelectionColors>,
}

/// What one node contributes to the frame.
enum NodePaint {
  Paragraph {
    /// Wrapped lines over the concatenated text, each with its start offset.
    lines: Vec<(usize, String)>,
    /// Each concatenated character's span index.
    attribution: Vec<u16>,
    spans: Vec<SpanPaint>,
    links: Vec<String>,
    selection: Option<(usize, usize)>,
  },
  Box {
    background: Option<(u8, u8, u8)>,
    border: Option<BorderPaint>,
    shadow: Option<Color>,
  },
  Text {
    lines: Vec<String>,
    foreground: Option<(u8, u8, u8)>,
    bold: bool,
    underline: bool,
    link: Option<String>,
    selection: Option<SelectionPaint>,
  },
  Field {
    background: Option<(u8, u8, u8)>,
    border: Option<BorderPaint>,
    shadow: Option<Color>,
    /// The visible rows, and the caret's row and column within them.
    rows: Vec<String>,
    dim: bool,
    caret: Option<(usize, usize)>,
    inset: (isize, isize),
  },
  None,
}

/// The border paint of a resolved box style.
fn border_paint(store: &Store, handle: Handle) -> Option<BorderPaint> {
  let cache = store.record(handle)?.kind.box_record()?.cache.as_ref()?;

  cache.style.border.as_ref().map(|_| BorderPaint {
    glyphs: cache.style.ext.border_glyphs.unwrap_or_default(),
    color: cache.border_ink.and_then(terminal_rgb),
  })
}

/// The selection colors of a resolved run: its own, or reverse video when both are
/// unset or transparent.
fn selection_colors(style: &PlainRun) -> Option<SelectionColors> {
  let ink = style.selection_color.and_then(terminal_rgb);
  let surface = style.selection_background.and_then(terminal_rgb);

  match (ink, surface) {
    (None, None) => None,
    (ink, surface) => Some((
      ink.unwrap_or((255, 255, 255)),
      surface.unwrap_or((52, 88, 134)),
    )),
  }
}

/// What `handle` paints, from its resolved style.
// A match dispatch over the record kinds; length is the kind count, not complexity.
#[expect(clippy::too_many_lines)]
fn node_paint(store: &Store, handle: Handle, width: usize, focused: bool) -> NodePaint {
  let Some(record) = store.record(handle)
  else {
    return NodePaint::None;
  };

  match &record.kind {
    Kind::Anchor | Kind::Scope => NodePaint::None,
    Kind::Box(inner) if is_paragraph(store, handle) => {
      let _ = inner;
      let text = paragraph_text(store, handle);
      let lines = wrap_spans(&text, width);
      let runs = paragraph_runs(store, handle);
      let mut destinations = Vec::new();
      let mut attribution = Vec::new();
      let spans: Vec<SpanPaint> = runs
        .iter()
        .enumerate()
        .map(|(index, run)| {
          let record = store.record(*run).expect("a live run");
          let Kind::Run { props, cache } = &record.kind
          else {
            unreachable!("paragraph runs are runs");
          };
          let style = cache.as_ref().map(|cache| &cache.style);
          let link = props.link.as_ref().map(|url| {
            destinations.push(url.clone());
            u16::try_from(destinations.len() - 1).unwrap_or(0)
          });

          attribution.extend(std::iter::repeat_n(
            u16::try_from(index).unwrap_or(0),
            record.text.chars().count(),
          ));

          SpanPaint {
            foreground: style.and_then(|style| style.color).and_then(terminal_rgb),
            bold: style.and_then(|style| style.ext.bold).unwrap_or(false),
            underline: link.is_some(),
            link,
            selection: style.and_then(selection_colors),
          }
        })
        .collect();

      NodePaint::Paragraph {
        lines,
        attribution,
        spans,
        links: destinations,
        selection: store.selection_range(handle),
      }
    }
    Kind::Box(inner) | Kind::Overlay { inner, .. } => {
      let style = inner.style();

      NodePaint::Box {
        background: style.and_then(|style| style.color).and_then(terminal_rgb),
        border: border_paint(store, handle),
        shadow: style.and_then(|style| style.ext.shadow),
      }
    }
    Kind::Control { inner, field, .. } => {
      let style = inner.style();
      let background = style.and_then(|style| style.color).and_then(terminal_rgb);
      let shadow = style.and_then(|style| style.ext.shadow);
      let border = border_paint(store, handle);

      let Some(field) = field
      else {
        return NodePaint::Box {
          background,
          border,
          shadow,
        };
      };
      let padding = style.and_then(|style| style.padding).unwrap_or_default();
      let border_cells = usize::from(border.is_some());
      let inset = (
        isize::try_from(border_cells).unwrap_or(0) + isize::try_from(padding.left).unwrap_or(0),
        isize::try_from(border_cells).unwrap_or(0) + isize::try_from(padding.top).unwrap_or(0),
      );
      let inner_width = width
        .saturating_sub(2 * border_cells)
        .saturating_sub(usize::try_from(padding.left + padding.right).unwrap_or(0));
      let (text, dim) = if field.value.is_empty() {
        (field.placeholder.clone().unwrap_or_default(), true)
      }
      else {
        (field.value.clone(), false)
      };
      let (rows, caret) = crate::field::visible_rows(&text, field.caret, inner_width, field.lines);

      NodePaint::Field {
        background,
        border,
        shadow,
        rows,
        dim,
        caret: (focused && inner_width > 0).then_some(caret),
        inset,
      }
    }
    Kind::Run { props, cache } => {
      let style = cache.as_ref().map(|cache| &cache.style);
      let spans = wrap_spans(&record.text, width);
      let selection = props
        .selectable
        .then(|| {
          store.selection_range(handle).map(|range| SelectionPaint {
            range,
            starts: spans.iter().map(|(start, _)| *start).collect(),
            colors: style.and_then(selection_colors),
          })
        })
        .flatten();

      NodePaint::Text {
        lines: spans.into_iter().map(|(_, line)| line).collect(),
        foreground: style.and_then(|style| style.color).and_then(terminal_rgb),
        bold: style.and_then(|style| style.ext.bold).unwrap_or(false),
        underline: props.link.is_some(),
        link: props.link.clone(),
        selection,
      }
    }
  }
}

/// Paints `handle` at its layout position offset by (`offset_x`, `offset_y`), then its
/// children.
// A match dispatch over the paints; length is the kind count, not complexity.
#[expect(clippy::too_many_lines)]
fn paint_node(
  store: &Store,
  buffer: &mut Buffer,
  handle: Handle,
  offset_x: isize,
  offset_y: isize,
) {
  let Some(record) = store.record(handle)
  else {
    return;
  };

  if !record.kind.is_laid_out() {
    for child in record.children.clone() {
      paint_node(store, buffer, child, offset_x, offset_y);
    }

    return;
  }

  let (location_x, location_y) = store.node_location(handle);
  let (x, y) = (
    offset_x + isize::try_from(location_x).unwrap_or(0),
    offset_y + isize::try_from(location_y).unwrap_or(0),
  );
  let (width, height) = store.node_size(handle);
  let (width, height) = (
    usize::try_from(width).unwrap_or(0),
    usize::try_from(height).unwrap_or(0),
  );
  let border = store.node_border(handle);
  let border = (
    isize::try_from(border.0).unwrap_or(0),
    isize::try_from(border.1).unwrap_or(0),
    isize::try_from(border.2).unwrap_or(0),
    isize::try_from(border.3).unwrap_or(0),
  );
  let focused = store.focused == Some(handle);
  let dark = store.dark();
  let paragraph = is_paragraph(store, handle);

  match node_paint(store, handle, width, focused) {
    NodePaint::Paragraph {
      lines,
      attribution,
      spans,
      links,
      selection,
    } => {
      // The paragraph's links join the frame registry; span indices shift by the base.
      let base = u16::try_from(buffer.links.len()).unwrap_or(0);

      buffer.links.extend(links);

      for (row, (start, line)) in lines.iter().enumerate() {
        let row_y = y + isize::try_from(row).expect("fits");

        for (column, character) in line.chars().enumerate() {
          let Some(cell) = buffer.cell_mut(x + isize::try_from(column).expect("fits"), row_y)
          else {
            continue;
          };
          let offset = start + column;
          let Some(paint) = attribution
            .get(offset)
            .and_then(|index| spans.get(usize::from(*index)))
          else {
            continue;
          };

          cell.character = character;

          if paint.foreground.is_some() {
            cell.foreground = paint.foreground;
          }

          cell.bold = paint.bold;
          cell.underline = paint.underline;
          cell.link = paint.link.map(|index| base + index);

          if selection.is_some_and(|(from, to)| offset >= from && offset < to) {
            select_cell(cell, paint.selection);
          }
        }
      }
    }
    NodePaint::Box {
      background,
      border,
      shadow,
    } => {
      if let Some(shadow) = shadow {
        draw_shadow(buffer, x, y, width, height, shadow, dark);
      }

      fill(buffer, x, y, width, height, background);

      if let Some(border) = border
        && border.color.is_some()
      {
        draw_border(buffer, x, y, width, height, border.glyphs, border.color);
      }
    }
    NodePaint::Text {
      lines,
      foreground,
      bold,
      underline,
      link,
      selection,
    } => {
      let link = link.map(|url| {
        buffer.links.push(url);

        u16::try_from(buffer.links.len() - 1).unwrap_or(0)
      });

      for (row, line) in lines.iter().enumerate() {
        let row_y = y + isize::try_from(row).expect("fits");

        draw_text(buffer, x, row_y, line, foreground, bold, false);

        if underline {
          for column in 0..line.chars().count() {
            if let Some(cell) = buffer.cell_mut(x + isize::try_from(column).expect("fits"), row_y) {
              cell.underline = true;
              cell.link = link;
            }
          }
        }
      }

      if let Some(selection) = selection {
        draw_selection(buffer, x, y, &lines, &selection);
      }
    }
    NodePaint::Field {
      background,
      border,
      shadow,
      rows,
      dim,
      caret,
      inset,
    } => {
      if let Some(shadow) = shadow {
        draw_shadow(buffer, x, y, width, height, shadow, dark);
      }

      fill(buffer, x, y, width, height, background);

      if let Some(border) = border
        && border.color.is_some()
      {
        draw_border(buffer, x, y, width, height, border.glyphs, border.color);
      }

      for (row, text) in rows.iter().enumerate() {
        draw_text(
          buffer,
          x + inset.0,
          y + inset.1 + isize::try_from(row).expect("fits"),
          text,
          None,
          false,
          dim,
        );
      }

      // The caret: the terminal's own bar cursor at the cell, recorded only while the
      // cell is visible under the current clip.
      if let Some((row, column)) = caret {
        let (caret_x, caret_y) = (
          x + inset.0 + isize::try_from(column).expect("fits"),
          y + inset.1 + isize::try_from(row).expect("fits"),
        );

        if buffer.cell_mut(caret_x, caret_y).is_some() {
          buffer.caret = u16::try_from(caret_x).ok().zip(u16::try_from(caret_y).ok());
        }
      }
    }
    NodePaint::None => {}
  }

  if paragraph {
    return;
  }

  let saved_clip = buffer.clip;

  if store.clips(handle) {
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

  let (scroll_x, scroll_y) = store.scroll_offset(handle);
  let (child_x, child_y) = (
    x - isize::try_from(scroll_x).unwrap_or(0),
    y - isize::try_from(scroll_y).unwrap_or(0),
  );

  for child in store.children(handle).to_vec() {
    if store.record(child).is_some_and(Record::is_overlay) {
      continue;
    }

    paint_node(store, buffer, child, child_x, child_y);
  }

  buffer.clip = saved_clip;

  if store.is_scrollable(handle) {
    draw_scrollbars(store, buffer, handle, x, y, width, height, border);
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
  store: &Store,
  buffer: &mut Buffer,
  handle: Handle,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  border: (isize, isize, isize, isize),
) {
  let (offset_x, offset_y) = store.scroll_offset(handle);
  let (limit_x, limit_y) = store.scroll_limits(handle);
  let (width, height) = (
    isize::try_from(width).unwrap_or(0),
    isize::try_from(height).unwrap_or(0),
  );
  let dark = store.dark();

  if limit_y > 0 {
    let track = height - border.1 - border.3;
    let (top, thumb) = thumb_span_eighths(track, offset_y, limit_y);

    for row in 0..track {
      if let Some(cell) = buffer.cell_mut(x + width - 1, y + border.1 + row) {
        let (ink, track_tint) = track_colors(cell, dark);
        // A leading partial paints the complement glyph with the colors swapped, in
        // place of the upper-block run Unicode lacks.
        let (character, foreground, background) = match eighth_coverage(row, top, thumb) {
          Coverage::Full => ('█', ink, track_tint),
          Coverage::Leading(eighths) => (lower_block(8 - eighths), track_tint, ink),
          Coverage::Trailing(eighths) => (lower_block(eighths), ink, track_tint),
          Coverage::None => (' ', ink, track_tint),
        };

        cell.character = character;
        cell.foreground = Some(foreground);
        cell.background = Some(background);
        cell.bold = false;
        cell.reverse = false;
      }
    }
  }

  if limit_x > 0 {
    let track = width - border.0 - border.2;
    let (start, thumb) = thumb_span_eighths(track, offset_x, limit_x);

    for column in 0..track {
      if let Some(cell) = buffer.cell_mut(x + border.0 + column, y + height - 1) {
        let (ink, track_tint) = track_colors(cell, dark);
        // A trailing partial paints the complement glyph with the colors swapped, in
        // place of the right-block run Unicode lacks.
        let (character, foreground, background) = match eighth_coverage(column, start, thumb) {
          Coverage::Full => ('█', ink, track_tint),
          Coverage::Leading(eighths) => (left_block(eighths), ink, track_tint),
          Coverage::Trailing(eighths) => (left_block(8 - eighths), track_tint, ink),
          Coverage::None => (' ', ink, track_tint),
        };

        cell.character = character;
        cell.foreground = Some(foreground);
        cell.background = Some(background);
        cell.bold = false;
        cell.reverse = false;
      }
    }
  }
}

/// One scrollbar cell's ink and track: the terminal default foreground for the
/// detected background, and a quarter of that ink over the cell beneath.
fn track_colors(cell: &TerminalCell, dark: bool) -> ((u8, u8, u8), (u8, u8, u8)) {
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

  (
    ink,
    (
      quarter(ink.0, beneath.0),
      quarter(ink.1, beneath.1),
      quarter(ink.2, beneath.2),
    ),
  )
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
/// within the scrollable content, in whole cells.
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

/// Fills a rectangle: a background paints opaquely, clearing whatever was beneath.
fn fill(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  background: Option<(u8, u8, u8)>,
) {
  if background.is_none() {
    return;
  }

  for row in 0..height {
    for column in 0..width {
      if let Some(cell) = buffer.cell_mut(
        x + isize::try_from(column).expect("fits"),
        y + isize::try_from(row).expect("fits"),
      ) {
        *cell = TerminalCell {
          background,
          ..TerminalCell::EMPTY
        };
      }
    }
  }
}

/// Draws a one-cell box border with the given character set.
pub(crate) fn draw_border(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  glyphs: BorderGlyphs,
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
fn draw_text(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  text: &str,
  foreground: Option<(u8, u8, u8)>,
  bold: bool,
  dim: bool,
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
  }
}

/// Paints one cell as selected: in the selection colors, or in reverse video.
fn select_cell(cell: &mut TerminalCell, colors: Option<SelectionColors>) {
  match colors {
    Some((ink, surface)) => {
      cell.foreground = Some(ink);
      cell.background = Some(surface);
    }
    None => cell.reverse = true,
  }
}

/// Recolors a run's selected cells.
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
        select_cell(cell, selection.colors);
      }
    }
  }
}

/// Paints the copy menu over everything at its anchored cell, the `Copy` item
/// highlighted while the pointer is on it.
fn draw_menu(store: &Store, buffer: &mut Buffer, dark: bool) {
  let Some((menu_x, menu_y, menu_width, menu_height)) = store.menu_bounds()
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

  // The menu covering the caret cell hides the terminal cursor with it.
  if let Some((caret_x, caret_y)) = buffer.caret
    && isize::try_from(caret_x)
      .is_ok_and(|caret_x| caret_x >= x && caret_x < x + isize::try_from(width).unwrap_or(0))
    && isize::try_from(caret_y)
      .is_ok_and(|caret_y| caret_y >= y && caret_y < y + isize::try_from(height).unwrap_or(0))
  {
    buffer.caret = None;
  }

  fill(buffer, x, y, width, height, Some(backdrop));
  draw_border(
    buffer,
    x,
    y,
    width,
    height,
    BorderGlyphs::default(),
    Some((128, 128, 128)),
  );

  let enabled = store.clipboard;
  let (pointer_x, pointer_y) = store.last_pointer;
  let hovered = enabled && store.copy_hit(pointer_x, pointer_y);

  if hovered {
    for column in 1..width.saturating_sub(1) {
      if let Some(cell) = buffer.cell_mut(x + isize::try_from(column).expect("fits"), y + 1) {
        cell.reverse = true;
      }
    }
  }

  draw_text(
    buffer,
    x + 2,
    y + 1,
    crate::selection::MENU_ITEM,
    None,
    false,
    !enabled,
  );

  if !enabled {
    draw_text(
      buffer,
      x + 2,
      y + 2,
      crate::selection::MENU_HINT,
      None,
      false,
      true,
    );
  }
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
  let alpha = alpha.clamp(0., 1.);

  if alpha <= 0. || width == 0 || height == 0 {
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
  let alpha = alpha.clamp(0., 1.);
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
      let mixed = (over.clamp(0., 1.) * 255.)
        .mul_add(alpha, f32::from(beneath) * (1. - alpha))
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

/// A [`Color`]'s terminal RGB, or `None` when fully transparent.
pub(crate) fn terminal_rgb(color: Color) -> Option<(u8, u8, u8)> {
  let [red, green, blue, alpha] = color.to_rgba();

  if alpha <= 0. {
    return None;
  }

  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  Some((
    (red.clamp(0., 1.) * 255.).round() as u8,
    (green.clamp(0., 1.) * 255.).round() as u8,
    (blue.clamp(0., 1.) * 255.).round() as u8,
  ))
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

/// The kind of the control `handle`, if it is one.
pub(crate) fn control_kind(store: &Store, handle: Handle) -> Option<ControlKind> {
  match store.record(handle)?.kind {
    Kind::Control { kind, .. } => Some(kind),
    _ => None,
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
}
