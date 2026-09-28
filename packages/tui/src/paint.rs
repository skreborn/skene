//! Painting the laid-out tree into a cell buffer.
//!
//! The tree paints back-to-front into a [`Buffer`], overlays in a second pass over
//! it; the buffer serializes to plain text for tests or to the terminal for the
//! interactive loop.

use std::rc::Rc;

use skene_core::{Color, ControlKind, TextDecoration, TextDecorationStyle};

use crate::{
  BorderGlyphs,
  cells::Cells,
  layout::{
    INLINE_BOX,
    has_border,
    inline_box_cells,
    is_inline_box,
    is_paragraph,
    paragraph_runs,
    paragraph_text,
  },
  store::{Contrast, Handle, Kind, PlainRun, Record, Store},
  text::{self, CellScale, Glyph, Scales},
};

/// One terminal cell's paint state.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct TerminalCell {
  pub(crate) glyph: Glyph,
  pub(crate) foreground: Option<(u8, u8, u8)>,
  pub(crate) background: Option<(u8, u8, u8)>,
  pub(crate) bold: bool,
  pub(crate) decoration: CellDecoration,
  /// Whether the cell paints in reverse video.
  pub(crate) reverse: bool,
  /// The cell's hyperlink, an index into the frame's [`Buffer::links`].
  pub(crate) link: Option<u16>,
  /// The scale the cell's glyph draws at; its block's later cells are continuations.
  pub(crate) scale: CellScale,
}

impl TerminalCell {
  pub(crate) const EMPTY: Self = Self {
    glyph: Glyph::SPACE,
    foreground: None,
    background: None,
    bold: false,
    decoration: CellDecoration::NONE,
    reverse: false,
    link: None,
    scale: CellScale::ONE,
  };
}

/// The decoration lines one painted cell carries, for a headless test asserting paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[must_use]
pub struct CellDecoration {
  /// The underline's style, `None` without one.
  pub underline: Option<TextDecorationStyle>,
  /// The underline's color, the text's where `None`.
  pub underline_color: Option<(u8, u8, u8)>,
  /// Whether a line runs above the text.
  pub overline: bool,
  /// Whether a line runs through the text.
  pub line_through: bool,
}

impl CellDecoration {
  /// No line.
  pub const NONE: Self = Self {
    underline: None,
    underline_color: None,
    overline: false,
    line_through: false,
  };

  /// The lines a run's resolved style draws: its decoration, or an underline where it
  /// sets none and is a link.
  fn of(style: Option<&PlainRun>, linked: bool) -> Self {
    let line = style
      .and_then(|style| style.decoration)
      .unwrap_or(if linked {
        TextDecoration::Underline
      }
      else {
        TextDecoration::None
      });
    let underline = (line == TextDecoration::Underline).then(|| {
      style
        .and_then(|style| style.decoration_style)
        .unwrap_or_default()
    });

    Self {
      underline,
      underline_color: underline
        .and_then(|_| style.and_then(|style| style.decoration_color))
        .and_then(terminal_rgb),
      overline: line == TextDecoration::Overline,
      line_through: line == TextDecoration::LineThrough,
    }
  }
}

/// A picture rendered from a painted box: its pixels as unpremultiplied RGBA8, rows
/// top to bottom, under the generation the terminal transmits it as.
pub(crate) struct Image {
  pub(crate) generation: u64,
  pub(crate) width: u16,
  pub(crate) height: u16,
  pub(crate) pixels: Vec<u8>,
}

/// Where a frame places a picture: the cells it covers on the screen, the part of
/// the image those cells show, in pixels, and the layer it draws on.
#[derive(Clone)]
pub(crate) struct Placement {
  /// The image's id at the terminal.
  pub(crate) id: u32,
  pub(crate) column: u16,
  pub(crate) row: u16,
  pub(crate) columns: u16,
  pub(crate) rows: u16,
  /// The source rectangle: left, top, width, height.
  pub(crate) crop: (u16, u16, u16, u16),
  /// The layer: zero over the text for a painted box, beneath every colored cell
  /// for a backdrop.
  pub(crate) layer: i32,
  pub(crate) image: Rc<Image>,
}

/// A placement's cells, crop and layer: what the terminal is told, apart from the
/// image.
pub(crate) type PlacedCells = (u16, u16, u16, u16, (u16, u16, u16, u16), i32);

impl Placement {
  /// The cells, the crop and the layer: what the terminal is told, apart from the
  /// image.
  pub(crate) fn cells(&self) -> PlacedCells {
    (
      self.column,
      self.row,
      self.columns,
      self.rows,
      self.crop,
      self.layer,
    )
  }
}

/// The lowest layer a backdrop draws on: beneath every colored cell, the box's own
/// cells left uncolored so it shows through, its depth in the tree added so a nested
/// box's backdrop draws over its parent's.
const BACKDROP_Z: i32 = -1_073_741_824 - 1024;

/// The id a box's backdrop image takes at the terminal: apart from its picture's.
const BACKDROP_ID: u32 = 1 << 30;

/// A painted box's picture as one frame places it, for a headless test asserting what
/// a terminal with graphics shows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Picture {
  /// The first column the picture covers.
  pub column: u16,
  /// The first row the picture covers.
  pub row: u16,
  /// How many columns it covers.
  pub columns: u16,
  /// How many rows it covers.
  pub rows: u16,
  /// The part of the image the cells show: left, top, width, height, in pixels.
  pub crop: (u16, u16, u16, u16),
  /// The layer: zero for a painted box's picture over the text, negative for a box's
  /// backdrop beneath every colored cell.
  pub layer: i32,
  /// The image's width in pixels.
  pub width: u16,
  /// The image's height in pixels.
  pub height: u16,
  /// The image's pixels as unpremultiplied RGBA8, rows top to bottom.
  pub pixels: Vec<u8>,
}

impl From<&Placement> for Picture {
  fn from(placement: &Placement) -> Self {
    Self {
      column: placement.column,
      row: placement.row,
      columns: placement.columns,
      rows: placement.rows,
      crop: placement.crop,
      layer: placement.layer,
      width: placement.image.width,
      height: placement.image.height,
      pixels: placement.image.pixels.clone(),
    }
  }
}

/// A painted frame: `width` x `height` cells in row-major order.
#[derive(Clone)]
pub(crate) struct Buffer {
  pub(crate) width: usize,
  pub(crate) height: usize,
  pub(crate) cells: Vec<TerminalCell>,
  /// The focused entry's caret cell, shown as the terminal's own bar cursor.
  pub(crate) caret: Option<(u16, u16)>,
  /// The frame's hyperlink destinations, indexed by [`TerminalCell::link`], emitted as
  /// OSC 8 anchors.
  pub(crate) links: Vec<String>,
  /// The pictures the frame places, over the cells they cover.
  pub(crate) pictures: Vec<Placement>,
  /// The active clip rectangle (start inclusive, end exclusive), while a clipping box
  /// paints its children.
  clip: Option<(isize, isize, isize, isize)>,
  /// The rows this paint rewrites; every row when absent.
  rows: Option<Vec<bool>>,
}

impl Buffer {
  pub(crate) fn new(width: usize, height: usize) -> Self {
    Self {
      width,
      height,
      cells: vec![TerminalCell::EMPTY; width * height],
      caret: None,
      links: Vec::new(),
      pictures: Vec::new(),
      clip: None,
      rows: None,
    }
  }

  /// This frame readied for a paint that rewrites `rows` alone: those rows cleared, and
  /// the caret and the pictures dropped for the paint to record afresh; the links stay,
  /// so the rows kept still name theirs.
  pub(crate) fn repainting(mut self, rows: &[bool]) -> Self {
    for (row, repaint) in rows.iter().enumerate() {
      if *repaint {
        self.cells[row * self.width..(row + 1) * self.width].fill(TerminalCell::EMPTY);
      }
    }

    self.caret = None;
    self.pictures.clear();
    self.rows = Some(rows.to_vec());

    self
  }

  /// The part of the rectangle at (`x`, `y`) of `width` by `height` cells that lies
  /// on the frame and inside the active clip, as its cell and size; `None` when none
  /// of it does.
  fn visible(
    &self,
    x: isize,
    y: isize,
    width: isize,
    height: isize,
  ) -> Option<(u16, u16, u16, u16)> {
    let frame = (
      0,
      0,
      isize::try_from(self.width).unwrap_or(isize::MAX),
      isize::try_from(self.height).unwrap_or(isize::MAX),
    );
    let bounds = self.clip.map_or(frame, |clip| intersect(clip, frame));
    let (left, top, right, bottom) = intersect(bounds, (x, y, x + width, y + height));

    if right <= left || bottom <= top {
      return None;
    }

    Some((
      u16::try_from(left).ok()?,
      u16::try_from(top).ok()?,
      u16::try_from(right - left).ok()?,
      u16::try_from(bottom - top).ok()?,
    ))
  }

  /// The index of `destination` among the frame's hyperlinks, added where it is new.
  pub(crate) fn link(&mut self, destination: &str) -> u16 {
    let index = self
      .links
      .iter()
      .position(|link| link == destination)
      .unwrap_or_else(|| {
        self.links.push(destination.to_owned());

        self.links.len() - 1
      });

    u16::try_from(index).unwrap_or(u16::MAX)
  }

  /// The destination of `cell`'s hyperlink in this frame.
  fn destination(&self, cell: &TerminalCell) -> Option<&str> {
    cell
      .link
      .and_then(|index| self.links.get(usize::from(index)))
      .map(String::as_str)
  }

  /// Blanks what no whole block backs: a continuation no glyph's block covers, and a
  /// glyph whose block another paint broke into, so the terminal is never left a half
  /// of a wide glyph to draw or keep.
  pub(crate) fn mend_blocks(&mut self) {
    let mut covered = vec![false; self.cells.len()];
    let mut broken = Vec::new();

    for index in 0..self.cells.len() {
      let cell = self.cells[index];

      if cell.glyph == Glyph::Continuation {
        continue;
      }

      let (x, y) = (index % self.width, index / self.width);
      let rows = cell.scale.rows();
      let columns = cell.glyph.cells() * rows;
      let mut whole = x + columns <= self.width && y + rows <= self.height;

      for row in y..(y + rows).min(self.height) {
        for column in x..(x + columns).min(self.width) {
          let at = row * self.width + column;

          if at != index {
            covered[at] = true;
            whole &= self.cells[at].glyph == Glyph::Continuation;
          }
        }
      }

      if !whole {
        broken.push(index);
      }
    }

    for index in broken {
      let cell = &mut self.cells[index];

      cell.glyph = Glyph::SPACE;
      cell.scale = CellScale::ONE;
    }

    for (index, cell) in self.cells.iter_mut().enumerate() {
      if cell.glyph == Glyph::Continuation && !covered[index] {
        cell.glyph = Glyph::SPACE;
      }
    }
  }

  /// Whether the cell (`x`, `y`) lies on the frame and inside the active clip.
  fn in_clip(&self, x: isize, y: isize) -> bool {
    if x < 0 || y < 0 {
      return false;
    }

    if let Some((clip_x, clip_y, clip_end_x, clip_end_y)) = self.clip
      && (x < clip_x || y < clip_y || x >= clip_end_x || y >= clip_end_y)
    {
      return false;
    }

    let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y))
    else {
      return false;
    };

    x < self.width && y < self.height
  }

  pub(crate) fn cell_mut(&mut self, x: isize, y: isize) -> Option<&mut TerminalCell> {
    if !self.in_clip(x, y) {
      return None;
    }

    let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);

    if self.rows.as_ref().is_some_and(|rows| !rows[y]) {
      return None;
    }

    self.cells.get_mut(y * self.width + x)
  }

  /// Paints `cluster` at (`x`, `y`) over `width` cells—its glyph in the first, a
  /// continuation in each later one, `style` applied to every cell—or a blank where
  /// a later cell would fall outside the frame or the clip; nothing for no width.
  pub(crate) fn put_cluster(
    &mut self,
    x: isize,
    y: isize,
    cluster: &str,
    width: usize,
    style: impl Fn(&mut TerminalCell),
  ) {
    self.put_sized(x, y, cluster, width, CellScale::ONE, style);
  }

  /// Paints `cluster` at (`x`, `y`) at `scale`: a block of the scale's rows by `width`
  /// times them, its glyph and scale in the top-left cell and a continuation in every
  /// other, `style` applied to each—or blanks where the block's first or last cell
  /// would fall outside the frame or the clip; nothing for no width.
  pub(crate) fn put_sized(
    &mut self,
    x: isize,
    y: isize,
    cluster: &str,
    width: usize,
    scale: CellScale,
    style: impl Fn(&mut TerminalCell),
  ) {
    if width == 0 {
      return;
    }

    let rows = scale.rows();
    let columns = width * rows;
    let last = (
      x + isize::try_from(columns - 1).expect("fits"),
      y + isize::try_from(rows - 1).expect("fits"),
    );
    let fits = self.in_clip(x, y) && self.in_clip(last.0, last.1);

    for row in 0..rows {
      for column in 0..columns {
        let Some(cell) = self.cell_mut(
          x + isize::try_from(column).expect("fits"),
          y + isize::try_from(row).expect("fits"),
        )
        else {
          continue;
        };

        (cell.glyph, cell.scale) = match (fits, row, column) {
          (false, ..) => (Glyph::SPACE, CellScale::ONE),
          (true, 0, 0) => (Glyph::of(cluster), scale),
          (true, ..) => (Glyph::Continuation, CellScale::ONE),
        };
        style(cell);
      }
    }
  }

  /// The frame as plain text, one row per line, a wide cluster on its first cell
  /// alone, a scaled cluster followed by the blanks its block adds to its row and
  /// its lower rows blank, trailing spaces trimmed.
  pub(crate) fn to_text(&self) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();

    for row in 0..self.height {
      let mut line = String::new();
      let mut covered = 0;

      for (column, cell) in self.cells[row * self.width..(row + 1) * self.width]
        .iter()
        .enumerate()
      {
        if cell.glyph == Glyph::Continuation {
          if column >= covered {
            line.push(' ');
          }

          continue;
        }

        let cells = cell.glyph.cells();

        covered = column + cells * cell.scale.rows();

        let _ = write!(line, "{}", cell.glyph);

        line.extend(std::iter::repeat_n(' ', cells * (cell.scale.rows() - 1)));
      }

      out.push_str(line.trim_end());
      out.push('\n');
    }

    out
  }
}

/// Wraps text to `width` cells: a line ends where the Unicode line breaking algorithm
/// allows, the furthest that fits, its trailing spaces hanging past it; a word longer
/// than a full line breaks mid-word, a cluster too wide for a line takes one alone,
/// and a newline always breaks. Yields at least one line, each paired with its start
/// offset into the source, in clusters, newlines included.
pub(crate) fn wrap_spans(text: &str, width: usize) -> Vec<(usize, String)> {
  wrap_sized(text, width, &Scales::default())
    .into_iter()
    .map(|line| (line.start, line.text))
    .collect()
}

/// One wrapped line of sized text: where it starts in the source, in clusters, its
/// text, and the rows it takes, those of its tallest cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SizedLine {
  pub(crate) start: usize,
  pub(crate) text: String,
  pub(crate) rows: usize,
}

impl SizedLine {
  /// The cells the line spans, each cluster at its scale.
  pub(crate) fn cells(&self, scales: &Scales) -> usize {
    text::clusters(&self.text)
      .enumerate()
      .map(|(index, (_, cluster))| scales.cells(self.start + index, cluster))
      .sum()
  }

  /// The cluster a hit at the cell `column` lands on: the one whose block holds it,
  /// the count past the line.
  pub(crate) fn cluster_at(&self, column: usize, scales: &Scales) -> Option<usize> {
    let mut cell = 0;

    for (index, (_, cluster)) in text::clusters(&self.text).enumerate() {
      cell += scales.cells(self.start + index, cluster);

      if column < cell {
        return Some(index);
      }
    }

    None
  }

  /// The cluster a caret goes to for the cell `column`: the one whose first cell it
  /// is, the next for a later cell of a block, and the count past the line.
  pub(crate) fn index_at_column(&self, column: usize, scales: &Scales) -> usize {
    let mut cell = 0;

    for (index, (_, cluster)) in text::clusters(&self.text).enumerate() {
      let next = cell + scales.cells(self.start + index, cluster);

      if column < next {
        return if column == cell { index } else { index + 1 };
      }

      cell = next;
    }

    text::count(&self.text)
  }
}

/// The wrapped lines of sized text, and the row each starts on.
pub(crate) fn sized_rows(lines: &[SizedLine]) -> Vec<usize> {
  let mut row = 0;

  lines
    .iter()
    .map(|line| {
      let start = row;

      row += line.rows;

      start
    })
    .collect()
}

/// The line whose rows hold the row `row` of the wrapped `lines`, if any.
pub(crate) fn line_holding(lines: &[SizedLine], row: usize) -> Option<usize> {
  sized_rows(lines)
    .iter()
    .zip(lines)
    .position(|(start, line)| (*start..start + line.rows).contains(&row))
}

/// The line holding the row `row` of the wrapped `lines`, clamped into them.
pub(crate) fn line_at_row(lines: &[SizedLine], row: usize) -> usize {
  line_holding(lines, row).unwrap_or(if row == 0 {
    0
  }
  else {
    lines.len().saturating_sub(1)
  })
}

/// [`wrap_spans`] with each cluster as wide as its scale makes it, and each line as
/// tall as its tallest cluster.
pub(crate) fn wrap_sized(text: &str, width: usize, scales: &Scales) -> Vec<SizedLine> {
  let width = width.max(1);
  let mut lines = Vec::new();
  let mut base = 0;

  for paragraph in text.split('\n') {
    let clusters: Vec<(&str, usize)> = text::clusters(paragraph)
      .enumerate()
      .map(|(index, (_, cluster))| (cluster, scales.cells(base + index, cluster)))
      .collect();
    let line = |from: usize, to: usize| SizedLine {
      start: base + from,
      text: clusters[from..to]
        .iter()
        .map(|(cluster, _)| *cluster)
        .collect(),
      rows: (from..to)
        .map(|index| scales.at(base + index).rows())
        .max()
        .unwrap_or_else(|| scales.at(base + from).rows()),
    };
    if clusters.is_empty() {
      lines.push(line(0, 0));
      base += 1;

      continue;
    }

    let breaks = line_breaks(paragraph);
    let mut start = 0;

    while start < clusters.len() {
      // The furthest break whose line, its trailing spaces aside, fits.
      let mut cells = 0;
      let mut trailing = 0;
      let mut furthest = None;
      let mut index = start;

      loop {
        if index > start && breaks[index] && cells - trailing <= width {
          furthest = Some(index);
        }

        if index == clusters.len() || cells - trailing > width {
          break;
        }

        let (cluster, cluster_cells) = clusters[index];

        cells += cluster_cells;
        trailing = if cluster == " " {
          trailing + cluster_cells
        }
        else {
          0
        };
        index += 1;
      }

      // Without one, a word wider than the line breaks where the line fills, and a
      // cluster too wide for a line takes one alone.
      let end = furthest.unwrap_or_else(|| {
        let mut cells = 0;
        let mut fit = start;

        while fit < clusters.len() && cells + clusters[fit].1 <= width {
          cells += clusters[fit].1;
          fit += 1;
        }

        fit.max(start + 1)
      });
      // The spaces a line ends in hang past it: gone at a wrap, and kept only while
      // they fit at the paragraph's end.
      let mut shown = end;
      let mut cells: usize = clusters[start..end].iter().map(|(_, cells)| cells).sum();

      while shown > start && clusters[shown - 1].0 == " " && (end < clusters.len() || cells > width)
      {
        cells -= clusters[shown - 1].1;
        shown -= 1;
      }

      lines.push(line(start, shown));
      start = end;
    }

    base += clusters.len() + 1;
  }

  lines
}

/// Wraps text to `width` columns; the lines of [`wrap_spans`], offsets dropped.
#[cfg(test)]
pub(crate) fn wrap_text(text: &str, width: usize) -> Vec<String> {
  wrap_spans(text, width)
    .into_iter()
    .map(|(_, line)| line)
    .collect()
}

/// The widest stretch between two places a line may end, its trailing spaces aside, in
/// cells at its scales: the narrowest a wrap can go.
pub(crate) fn longest_word(text: &str, scales: &Scales) -> usize {
  let mut longest = 0;
  let mut base = 0;

  for paragraph in text.split('\n') {
    let breaks = line_breaks(paragraph);
    let mut word = 0;
    let mut trailing = 0;

    for (index, (_, cluster)) in text::clusters(paragraph).enumerate() {
      if breaks[index] {
        word = 0;
        trailing = 0;
      }

      let cells = scales.cells(base + index, cluster);

      word += cells;
      trailing = if cluster == " " { trailing + cells } else { 0 };
      longest = longest.max(word - trailing);
    }

    base += text::count(paragraph) + 1;
  }

  longest
}

/// Where a line may end in `paragraph`, by cluster, as the Unicode line breaking
/// algorithm finds it: before the cluster at an index holding `true`, and at the end.
fn line_breaks(paragraph: &str) -> Vec<bool> {
  let starts: Vec<usize> = text::clusters(paragraph).map(|(byte, _)| byte).collect();
  let mut breaks = vec![false; starts.len() + 1];

  for (byte, _) in unicode_linebreak::linebreaks(paragraph) {
    if byte == paragraph.len() {
      breaks[starts.len()] = true;
    }
    else if let Ok(index) = starts.binary_search(&byte) {
      breaks[index] = true;
    }
  }

  breaks
}

/// Paints the frame into `buffer`: the tree, then every open overlay, and mends the
/// wide glyphs a later paint broke into; a picture an open overlay covers in part is
/// left out, since the terminal draws it over the overlay's cells.
pub(crate) fn paint(store: &Store, buffer: &mut Buffer) {
  paint_node(store, buffer, store.root, 0, 0);

  let overlays = store.open_overlays();

  buffer.pictures.retain(|picture| {
    picture.layer < 0
      || !overlays.iter().any(|overlay| {
        let (origin_x, origin_y) = store.overlay_origin(*overlay);
        let (width, height) = store.node_size(*overlay);

        i64::from(picture.column) < origin_x + width
          && i64::from(picture.column + picture.columns) > origin_x
          && i64::from(picture.row) < origin_y + height
          && i64::from(picture.row + picture.rows) > origin_y
      })
  });

  for overlay in overlays {
    paint_overlay(store, buffer, overlay);
  }

  buffer.mend_blocks();
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
  let contrast = store.contrast();

  if let Some(color) = backdrop {
    let [red, green, blue, alpha] = color.to_rgba();

    if alpha > 0. {
      for row in 0..buffer.height {
        if buffer.rows.as_ref().is_some_and(|rows| !rows[row]) {
          continue;
        }

        for cell in &mut buffer.cells[row * buffer.width..(row + 1) * buffer.width] {
          tint(cell, red, green, blue, alpha, contrast);
        }
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

/// One paragraph span's paint: its ink, weight, decoration, link, and the emphasis it
/// paints in while it is the link whose focus shows.
struct SpanPaint {
  foreground: Option<(u8, u8, u8)>,
  bold: bool,
  decoration: CellDecoration,
  link: Option<u16>,
  /// The emphasis of the link whose focus shows; `None` while the span is not it.
  focused: Option<Emphasis>,
  selection: Option<SelectionColors>,
}

/// An emphasis over a run's cells: the terminal's reverse video, or an ink on a
/// surface.
#[derive(Clone, Copy)]
enum Emphasis {
  Reverse,
  Colors(SelectionColors),
}

impl Emphasis {
  /// The ink and surface, `None` for reverse video.
  const fn colors(self) -> Option<SelectionColors> {
    match self {
      Self::Reverse => None,
      Self::Colors(colors) => Some(colors),
    }
  }
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
    /// Wrapped lines over the concatenated text, each cluster at its span's scale.
    lines: Vec<SizedLine>,
    /// Each concatenated cluster's span index.
    attribution: Vec<u16>,
    spans: Vec<SpanPaint>,
    scales: Scales,
    links: Vec<String>,
    selection: Option<(usize, usize)>,
  },
  Box {
    background: Option<(u8, u8, u8)>,
    border: Option<BorderPaint>,
    shadow: Option<Color>,
  },
  Text {
    lines: Vec<SizedLine>,
    foreground: Option<(u8, u8, u8)>,
    bold: bool,
    scale: CellScale,
    decoration: CellDecoration,
    link: Option<String>,
    /// The emphasis of the link whose focus shows; `None` while the run is not it.
    focused: Option<Emphasis>,
    selection: Option<SelectionPaint>,
  },
  Field {
    background: Option<(u8, u8, u8)>,
    border: Option<BorderPaint>,
    shadow: Option<Color>,
    /// The visible rows.
    rows: Vec<String>,
    /// The value's ink, the terminal's own where `None`.
    foreground: Option<(u8, u8, u8)>,
    bold: bool,
    /// Whether the rows are the placeholder, drawn at half the ink's strength.
    placeholder: bool,
    /// The caret's row and column within the rows.
    caret: Option<(usize, usize)>,
    inset: (isize, isize),
    /// The selection over the visible rows, while the entry holds focus.
    selection: Option<SelectionPaint>,
  },
  None,
}

/// The border paint of a resolved box style.
fn border_paint(store: &Store, handle: Handle) -> Option<BorderPaint> {
  let cache = store.record(handle)?.kind.box_record()?.cache.as_ref()?;

  has_border(&cache.style).then(|| BorderPaint {
    glyphs: cache.style.ext.border_glyphs.unwrap_or_default(),
    color: cache.border_ink.and_then(terminal_rgb),
  })
}

/// The selection colors of a resolved run: its own, or reverse video when both are
/// unset or transparent.
fn selection_colors(style: &PlainRun) -> Option<SelectionColors> {
  emphasis_colors(style.selection_color, style.selection_background)
}

/// The emphasis a link run paints in while its focus shows: the typography's colors,
/// or reverse video when both are unset or transparent.
fn link_focus_emphasis(store: &Store, run: Handle) -> Emphasis {
  let (ink, surface) = crate::resolve::Env::of(store, run).resolve_link_focus();

  emphasis_colors(ink, surface).map_or(Emphasis::Reverse, Emphasis::Colors)
}

/// An emphasis in `ink` on `surface`, each filled in for the other when one is set;
/// reverse video when both are unset or transparent.
fn emphasis_colors(ink: Option<Color>, surface: Option<Color>) -> Option<SelectionColors> {
  let ink = ink.and_then(terminal_rgb);
  let surface = surface.and_then(terminal_rgb);

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
      let scales = paragraph_scales(store, handle);
      let lines = wrap_sized(&text, width, &scales);
      let runs = paragraph_runs(store, handle);
      let mut destinations = Vec::new();
      let mut ends = Vec::new();
      let spans: Vec<SpanPaint> = runs
        .iter()
        .map(|run| {
          let record = store.record(*run).expect("a live run");
          let Kind::Run { props, cache, .. } = &record.kind
          else {
            unreachable!("paragraph runs are runs");
          };
          let style = cache.as_ref().map(|cache| &cache.style);
          let link = props.link.as_ref().map(|url| {
            destinations.push(url.clone());
            u16::try_from(destinations.len() - 1).unwrap_or(0)
          });

          ends.push(ends.last().unwrap_or(&0) + record.text.len());

          SpanPaint {
            foreground: style.and_then(|style| style.color).and_then(terminal_rgb),
            bold: style.and_then(|style| style.ext.bold).unwrap_or(false),
            decoration: CellDecoration::of(style, link.is_some()),
            link,
            focused: (link.is_some() && store.focus_shows(*run))
              .then(|| link_focus_emphasis(store, *run)),
            selection: style.and_then(selection_colors),
          }
        })
        .collect();

      // Each cluster of the paragraph belongs to the run its first byte came from.
      let attribution = text::clusters(&text)
        .map(|(byte, _)| {
          let index = ends.iter().position(|end| byte < *end).unwrap_or(0);

          u16::try_from(index).unwrap_or(0)
        })
        .collect();

      NodePaint::Paragraph {
        lines,
        attribution,
        spans,
        scales,
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
      let area = store.field_area(handle);
      let (rows, caret) = field.visible_rows(area);
      let run = crate::resolve::Env::of(store, handle).resolve_entry_run();
      let selection = field
        .selection()
        .filter(|_| focused)
        .map(|range| SelectionPaint {
          range,
          starts: rows.iter().map(|(start, _)| *start).collect(),
          colors: selection_colors(&run),
        });

      NodePaint::Field {
        background,
        border,
        shadow,
        rows: rows.into_iter().map(|(_, row)| row).collect(),
        foreground: run.color.and_then(terminal_rgb),
        bold: run.ext.bold.unwrap_or(false),
        placeholder: field.value.is_empty(),
        caret: caret.filter(|_| focused && area.columns > 0),
        inset,
        selection,
      }
    }
    Kind::Run { .. } if is_inline_box(store, handle) => NodePaint::None,
    Kind::Run { props, cache, .. } => {
      let style = cache.as_ref().map(|cache| &cache.style);
      let scale = run_scale(store, handle);
      let lines = wrap_sized(&record.text, width, &Scales::uniform(scale));
      let selection = props
        .selectable
        .then(|| {
          store.selection_range(handle).map(|range| SelectionPaint {
            range,
            starts: lines.iter().map(|line| line.start).collect(),
            colors: style.and_then(selection_colors),
          })
        })
        .flatten();

      NodePaint::Text {
        lines,
        foreground: style.and_then(|style| style.color).and_then(terminal_rgb),
        bold: style.and_then(|style| style.ext.bold).unwrap_or(false),
        scale,
        decoration: CellDecoration::of(style, props.link.is_some()),
        link: props.link.clone(),
        focused: (props.link.is_some() && focused).then(|| link_focus_emphasis(store, handle)),
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
  let edges = border;
  let focused = store.focus_shows(handle);
  let contrast = store.contrast();
  let paragraph = is_paragraph(store, handle);

  match node_paint(store, handle, width, focused) {
    NodePaint::Paragraph {
      lines,
      attribution,
      spans,
      scales,
      links,
      selection,
    } => {
      // The paragraph's links join the frame's.
      let links: Vec<u16> = links.iter().map(|link| buffer.link(link)).collect();

      for (line, row) in lines.iter().zip(sized_rows(&lines)) {
        let mut column = 0;

        for (index, (_, cluster)) in text::clusters(&line.text).enumerate() {
          let cell_x = x + isize::try_from(column).expect("fits");
          let width = text::width(cluster);
          let offset = line.start + index;
          let scale = scales.at(offset);
          // A smaller cluster sits on the line's bottom rows, as on a baseline.
          let row_y = y + isize::try_from(row + line.rows - scale.rows()).expect("fits");

          column += width * scale.rows();

          let Some(paint) = attribution
            .get(offset)
            .and_then(|index| spans.get(usize::from(*index)))
          else {
            continue;
          };

          if cluster.starts_with(INLINE_BOX) {
            continue;
          }

          let selected = selection.is_some_and(|(from, to)| offset >= from && offset < to);

          buffer.put_sized(cell_x, row_y, cluster, width, scale, |cell| {
            if paint.foreground.is_some() {
              cell.foreground = paint.foreground;
            }

            cell.bold = paint.bold;
            cell.decoration = paint.decoration;
            cell.link = paint
              .link
              .and_then(|index| links.get(usize::from(index)).copied());

            if let Some(emphasis) = paint.focused {
              select_cell(cell, emphasis.colors());
            }

            if selected {
              select_cell(cell, paint.selection);
            }
          });
        }
      }

      // An inline box paints its own layout at the cell its placeholder flowed into.
      for (run, (cell_x, cell_y)) in inline_box_cells(store, handle) {
        let (location_x, location_y) = store.node_location(run);

        paint_node(
          store,
          buffer,
          run,
          x + isize::try_from(cell_x - location_x).unwrap_or(0),
          y + isize::try_from(cell_y - location_y).unwrap_or(0),
        );
      }
    }
    NodePaint::Box {
      background,
      border,
      shadow,
    } => {
      // A backdrop carries the decoration as pixels beneath the cells, which stay
      // uncolored and unbordered so it shows through.
      if place_backdrop(store, buffer, handle, (x, y), (width, height)) {
        // Nothing in cells.
      }
      else {
        if let Some(shadow) = shadow {
          draw_shadow(buffer, x, y, width, height, shadow, contrast);
        }

        // A childless, borderless box paints from its parent's origin by eighths:
        // whole cells in its color, and a cell an edge crosses as a block glyph
        // carrying its color beside what lies beneath.
        match store.smooth_span(handle).filter(|_| border.is_none()) {
          Some((start, extent)) => {
            draw_smooth(buffer, offset_x, y, height, start, extent, background);
          }
          None => fill(buffer, x, y, width, height, background),
        }

        if let Some(border) = border
          && border.color.is_some()
        {
          draw_border(buffer, x, y, width, height, border.glyphs, border.color);
        }
      }

      if let Some(painter) = &record.painter {
        let size = skene_core::Size::new(
          u32::try_from(isize::try_from(width).unwrap_or(0) - edges.0 - edges.2).unwrap_or(0),
          u32::try_from(isize::try_from(height).unwrap_or(0) - edges.1 - edges.3).unwrap_or(0),
        );
        let mut cells = Cells::new(buffer, (x + edges.0, y + edges.1), size);
        let rect = cells.rect();

        painter(&mut cells, rect);
      }

      if let Some(image) = record
        .picture
        .as_ref()
        .and_then(|picture| picture.image.clone())
        && store.graphics
      {
        place_image(
          store,
          buffer,
          handle.index() + 1,
          image,
          0,
          (x + edges.0, y + edges.1),
          (
            isize::try_from(width).unwrap_or(0) - edges.0 - edges.2,
            isize::try_from(height).unwrap_or(0) - edges.1 - edges.3,
          ),
        );
      }
    }
    NodePaint::Text {
      lines,
      foreground,
      bold,
      scale,
      decoration,
      link,
      focused,
      selection,
    } => {
      let link = link.map(|url| buffer.link(&url));
      let scales = Scales::uniform(scale);

      for (line, row) in lines.iter().zip(sized_rows(&lines)) {
        let row_y = y + isize::try_from(row).expect("fits");

        draw_sized(buffer, x, row_y, &line.text, scale, |cell| {
          if foreground.is_some() {
            cell.foreground = foreground;
          }

          cell.bold = bold;

          if link.is_some() || decoration != CellDecoration::NONE {
            cell.decoration = decoration;
            cell.link = link;

            if let Some(emphasis) = focused {
              select_cell(cell, emphasis.colors());
            }
          }
        });
      }

      if let Some(selection) = selection {
        draw_selection(buffer, x, y, &lines, &scales, &selection);
      }
    }
    NodePaint::Field {
      background,
      border,
      shadow,
      rows,
      foreground,
      bold,
      placeholder,
      caret,
      inset,
      selection,
    } => {
      if !place_backdrop(store, buffer, handle, (x, y), (width, height)) {
        if let Some(shadow) = shadow {
          draw_shadow(buffer, x, y, width, height, shadow, contrast);
        }

        fill(buffer, x, y, width, height, background);

        if let Some(border) = border
          && border.color.is_some()
        {
          draw_border(buffer, x, y, width, height, border.glyphs, border.color);
        }
      }

      for (row, text) in rows.iter().enumerate() {
        draw_sized(
          buffer,
          x + inset.0,
          y + inset.1 + isize::try_from(row).expect("fits"),
          text,
          CellScale::ONE,
          |cell| {
            cell.foreground = if placeholder {
              Some(halfway(
                foreground.unwrap_or(contrast.ink),
                cell.background.unwrap_or(contrast.base),
              ))
            }
            else {
              foreground
            };
            cell.bold = bold;
          },
        );
      }

      if let Some(selection) = selection {
        let lines: Vec<SizedLine> = rows
          .iter()
          .zip(&selection.starts)
          .map(|(text, start)| SizedLine {
            start: *start,
            text: text.clone(),
            rows: 1,
          })
          .collect();

        draw_selection(
          buffer,
          x + inset.0,
          y + inset.1,
          &lines,
          &Scales::default(),
          &selection,
        );
      }

      // The caret: the terminal's own bar cursor at the cell, recorded only while the
      // cell is visible under the current clip.
      if let Some((row, column)) = caret {
        let (caret_x, caret_y) = (
          x + inset.0 + isize::try_from(column).expect("fits"),
          y + inset.1 + isize::try_from(row).expect("fits"),
        );

        if buffer.in_clip(caret_x, caret_y) {
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

/// Places `image` as `id` on `layer` over the visible part of the rectangle at
/// (`x`, `y`) of `width` by `height` cells, cropping the image to the cells in view.
fn place_image(
  store: &Store,
  buffer: &mut Buffer,
  id: u32,
  image: Rc<Image>,
  layer: i32,
  (x, y): (isize, isize),
  (width, height): (isize, isize),
) {
  let Some((column, row, columns, rows)) = buffer.visible(x, y, width, height)
  else {
    return;
  };
  let (cell_width, cell_height) = store.cell_pixels;
  let left = u16::try_from(isize::try_from(column).unwrap_or(0) - x).unwrap_or(0);
  let top = u16::try_from(isize::try_from(row).unwrap_or(0) - y).unwrap_or(0);

  buffer.pictures.push(Placement {
    id,
    column,
    row,
    columns,
    rows,
    crop: (
      left.saturating_mul(cell_width),
      top.saturating_mul(cell_height),
      columns.saturating_mul(cell_width).min(image.width),
      rows.saturating_mul(cell_height).min(image.height),
    ),
    layer,
    image,
  });
}

/// Places the backdrop of the box `handle`, where it has one and the terminal shows
/// graphics, over the box at (`x`, `y`) of `width` by `height` cells and the shadow
/// band beyond it, beneath every colored cell at the box's depth; `true` when placed,
/// so the cells carry none of the decoration.
fn place_backdrop(
  store: &Store,
  buffer: &mut Buffer,
  handle: Handle,
  (x, y): (isize, isize),
  (width, height): (usize, usize),
) -> bool {
  if !store.graphics {
    return false;
  }

  let Some(backdrop) = store
    .record(handle)
    .and_then(|record| record.backdrop.as_ref())
  else {
    return false;
  };
  let ancestors = store.ancestors(handle);
  let depth = ancestors.len()
    + 512
      * usize::from(
        ancestors
          .iter()
          .any(|ancestor| store.record(*ancestor).is_some_and(Record::is_overlay)),
      );
  let layer = BACKDROP_Z + i32::try_from(depth.min(1023)).unwrap_or(1023);
  let band = isize::from(backdrop.key.shadow.is_some());

  place_image(
    store,
    buffer,
    BACKDROP_ID + handle.index(),
    Rc::clone(&backdrop.image),
    layer,
    (x, y),
    (
      isize::try_from(width).unwrap_or(0) + band,
      isize::try_from(height).unwrap_or(0) + band,
    ),
  );

  true
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
  let contrast = store.contrast();

  if limit_y > 0 {
    let track = height - border.1 - border.3;
    let (top, thumb) = thumb_span_eighths(track, offset_y, limit_y);

    for row in 0..track {
      if let Some(cell) = buffer.cell_mut(x + width - 1, y + border.1 + row) {
        let (ink, track_tint) = track_colors(cell, contrast);
        // A leading partial paints the complement glyph with the colors swapped, in
        // place of the upper-block run Unicode lacks.
        let (character, foreground, background) = match eighth_coverage(row, top, thumb) {
          Coverage::Full => ('█', ink, track_tint),
          Coverage::Leading(eighths) => (lower_block(8 - eighths), track_tint, ink),
          Coverage::Trailing(eighths) => (lower_block(eighths), ink, track_tint),
          Coverage::None => (' ', ink, track_tint),
        };

        cell.glyph = Glyph::Char(character);
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
        let (ink, track_tint) = track_colors(cell, contrast);
        // A trailing partial paints the complement glyph with the colors swapped, in
        // place of the right-block run Unicode lacks.
        let (character, foreground, background) = match eighth_coverage(column, start, thumb) {
          Coverage::Full => ('█', ink, track_tint),
          Coverage::Leading(eighths) => (left_block(eighths), ink, track_tint),
          Coverage::Trailing(eighths) => (left_block(8 - eighths), track_tint, ink),
          Coverage::None => (' ', ink, track_tint),
        };

        cell.glyph = Glyph::Char(character);
        cell.foreground = Some(foreground);
        cell.background = Some(background);
        cell.bold = false;
        cell.reverse = false;
      }
    }
  }
}

/// One scrollbar cell's ink and track: the terminal's foreground under `contrast`,
/// and a quarter of that ink over the cell beneath.
fn track_colors(cell: &TerminalCell, contrast: Contrast) -> ((u8, u8, u8), (u8, u8, u8)) {
  let Contrast { ink, base } = contrast;
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

/// Fills a box spanning `extent` eighth cells from `start`, an eighth offset from the
/// cell at `x`, down `height` rows: whole cells take `color` as their background, and
/// a cell the box covers in part takes a block glyph with the box's color on its side
/// and the color already there on the other.
fn draw_smooth(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  height: usize,
  start: isize,
  extent: isize,
  color: Option<(u8, u8, u8)>,
) {
  if color.is_none() || extent <= 0 {
    return;
  }

  let first = start.div_euclid(8);
  let last = (start + extent - 1).div_euclid(8);

  for row in 0..height {
    let row_y = y + isize::try_from(row).expect("fits");

    for column in first..=last {
      let Some(cell) = buffer.cell_mut(x + column, row_y)
      else {
        continue;
      };

      match eighth_coverage(column, start, extent) {
        Coverage::Full => {
          *cell = TerminalCell {
            background: color,
            ..TerminalCell::EMPTY
          };
        }
        Coverage::Leading(eighths) => {
          *cell = TerminalCell {
            glyph: Glyph::Char(left_block(eighths)),
            foreground: color,
            background: cell.background,
            ..TerminalCell::EMPTY
          };
        }
        Coverage::Trailing(eighths) => {
          *cell = TerminalCell {
            glyph: Glyph::Char(left_block(8 - eighths)),
            foreground: left_color(cell),
            background: color,
            ..TerminalCell::EMPTY
          };
        }
        Coverage::None => {}
      }
    }
  }
}

/// The color a cell shows at its left edge: a left-block glyph's ink, else its
/// background.
fn left_color(cell: &TerminalCell) -> Option<(u8, u8, u8)> {
  if matches!(
    cell.glyph,
    Glyph::Char('▏' | '▎' | '▍' | '▌' | '▋' | '▊' | '▉')
  ) {
    cell.foreground
  }
  else {
    cell.background
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
      cell.glyph = Glyph::Char(character);

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

/// Draws one line of text at `scale`, `style` applied to every cell of each block.
fn draw_sized(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  line: &str,
  scale: CellScale,
  style: impl Fn(&mut TerminalCell),
) {
  let mut column = 0;

  for (_, cluster) in text::clusters(line) {
    let width = text::width(cluster);

    buffer.put_sized(
      x + isize::try_from(column).expect("fits"),
      y,
      cluster,
      width,
      scale,
      &style,
    );
    column += width * scale.rows();
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

/// Recolors a run's selected cells, every cell of a selected cluster's block.
fn draw_selection(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  lines: &[SizedLine],
  scales: &Scales,
  selection: &SelectionPaint,
) {
  let (from, to) = selection.range;

  for (line, row) in lines.iter().zip(sized_rows(lines)) {
    let mut column = 0;

    for (index, (_, cluster)) in text::clusters(&line.text).enumerate() {
      let offset = line.start + index;
      let scale = scales.at(offset);
      let width = text::width(cluster) * scale.rows();
      let top = row + line.rows - scale.rows();

      if offset >= from && offset < to {
        for block_row in top..top + scale.rows() {
          for cell in column..column + width {
            if let Some(cell) = buffer.cell_mut(
              x + isize::try_from(cell).expect("fits"),
              y + isize::try_from(block_row).expect("fits"),
            ) {
              select_cell(cell, selection.colors);
            }
          }
        }
      }

      column += width;
    }
  }
}

/// The scale the run `handle` draws at: its resolved size where the terminal sizes
/// text, one cell elsewhere and for an inline box.
pub(crate) fn run_scale(store: &Store, handle: Handle) -> CellScale {
  if is_inline_box(store, handle) {
    return CellScale::ONE;
  }

  match store.record(handle).map(|record| &record.kind) {
    Some(Kind::Run { cache, .. }) => {
      text::scale_of(cache.as_ref().and_then(|cache| cache.style.ext.size))
    }
    _ => CellScale::ONE,
  }
}

/// The scale of each run's stretch of the paragraph `handle`'s text.
pub(crate) fn paragraph_scales(store: &Store, handle: Handle) -> Scales {
  Scales::of(paragraph_runs(store, handle).into_iter().map(|run| {
    (
      store
        .record(run)
        .map_or(0, |record| text::count(&record.text)),
      run_scale(store, run),
    )
  }))
}

/// Casts a box's shadow: the one-cell band below and to the right, the shadow color
/// mixed into both the foreground and the background of each covered cell by its
/// alpha. An unset color beneath mixes against the terminal's own under `contrast`.
fn draw_shadow(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  color: Color,
  contrast: Contrast,
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
      tint(cell, red, green, blue, alpha, contrast);
    }
  };

  for row in 1..=height {
    cast(x + width, y + row);
  }

  for column in 1..width {
    cast(x + column, y + height);
  }
}

/// The color halfway between `over` and `beneath`.
fn halfway(over: (u8, u8, u8), beneath: (u8, u8, u8)) -> (u8, u8, u8) {
  let channel = |over: u8, beneath: u8| -> u8 {
    u8::try_from((u16::from(over) + u16::from(beneath)).div_ceil(2)).unwrap_or(u8::MAX)
  };

  (
    channel(over.0, beneath.0),
    channel(over.1, beneath.1),
    channel(over.2, beneath.2),
  )
}

/// Mixes a color into both of a cell's colors by `alpha`, an unset color mixing
/// against the terminal's own under `contrast`.
fn tint(cell: &mut TerminalCell, red: f32, green: f32, blue: f32, alpha: f32, contrast: Contrast) {
  let alpha = alpha.clamp(0., 1.);
  let Contrast {
    ink: foreground_base,
    base: background_base,
  } = contrast;
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
/// differs in that row, a hyperlink compared by its destination.
pub(crate) fn row_changed(previous: Option<&Buffer>, next: &Buffer, row: usize) -> bool {
  match previous {
    Some(previous) if previous.width == next.width && previous.height == next.height => {
      let span = row * next.width..(row + 1) * next.width;

      previous.cells[span.clone()]
        .iter()
        .zip(&next.cells[span])
        .any(|(before, after)| {
          TerminalCell {
            link: None,
            ..*before
          } != TerminalCell {
            link: None,
            ..*after
          } || previous.destination(before) != next.destination(after)
        })
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

    previous.cells[1].glyph = Glyph::Char('a');
    next.cells[1].glyph = Glyph::Char('a');
    next.cells[4].glyph = Glyph::Char('b');

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

  /// A wide glyph whose first cell falls left of the clip leaves blanks, never a lone
  /// continuation; a narrow glyph painted over either half of a wide one blanks the
  /// other half.
  #[test]
  fn no_half_of_a_wide_glyph_survives() {
    text::reset();

    let glyphs =
      |buffer: &Buffer| -> Vec<Glyph> { buffer.cells.iter().map(|cell| cell.glyph).collect() };
    let mut clipped = Buffer::new(4, 1);

    clipped.clip = Some((1, 0, 4, 1));
    clipped.put_sized(0, 0, "日", 2, CellScale::ONE, |_| {});
    clipped.mend_blocks();

    assert_eq!(glyphs(&clipped)[1], Glyph::SPACE);

    let mut over_lead = Buffer::new(4, 1);

    over_lead.put_sized(0, 0, "日", 2, CellScale::ONE, |_| {});
    over_lead.put_sized(0, 0, "a", 1, CellScale::ONE, |_| {});
    over_lead.mend_blocks();

    assert_eq!(glyphs(&over_lead)[..2], [Glyph::Char('a'), Glyph::SPACE]);

    let mut over_tail = Buffer::new(4, 1);

    over_tail.put_sized(0, 0, "日", 2, CellScale::ONE, |_| {});
    over_tail.put_sized(1, 0, "b", 1, CellScale::ONE, |_| {});
    over_tail.mend_blocks();

    assert_eq!(glyphs(&over_tail)[..2], [Glyph::SPACE, Glyph::Char('b')]);
  }

  /// Two frames compare their hyperlinks by destination, whatever index each frame's
  /// table gave it, and a repaint keeps the links of the rows it leaves.
  #[test]
  fn links_compare_by_destination() {
    let frame = |links: &[&str], link: u16| {
      let mut buffer = Buffer::new(2, 1);

      for destination in links {
        let _ = buffer.link(destination);
      }

      buffer.cells[0].link = Some(link);

      buffer
    };

    assert!(!row_changed(
      Some(&frame(&["a", "b"], 1)),
      &frame(&["b"], 0),
      0
    ));
    assert!(row_changed(Some(&frame(&["a"], 0)), &frame(&["b"], 0), 0));

    let kept = frame(&["a"], 0).repainting(&[false]);

    assert_eq!(kept.destination(&kept.cells[0]), Some("a"));
  }

  /// Wide clusters wrap by their cells: two of them fill a four-cell line, and one
  /// too wide for its line takes a line alone.
  #[test]
  fn wrapping_counts_cells_not_characters() {
    text::reset();

    assert_eq!(wrap_text("日本語", 4), ["日本", "語"]);
    assert_eq!(wrap_text("日本", 1), ["日", "本"]);
    assert_eq!(wrap_text("ae\u{301}b", 2), ["ae\u{301}", "b"]);
    assert_eq!(longest_word("日本 ab", &Scales::default()), 2);
  }

  /// A line ends where the Unicode line breaking algorithm allows: after a hyphen and
  /// between ideographs as well as at spaces, never before closing punctuation.
  #[test]
  fn lines_break_where_unicode_allows() {
    text::reset();

    assert_eq!(wrap_text("foo-bar", 5), ["foo-", "bar"]);
    assert_eq!(wrap_text("日本語テキスト", 6), ["日本語", "テキス", "ト"]);
    assert_eq!(wrap_text("one (two)", 6), ["one", "(two)"]);
    assert_eq!(longest_word("foo-bar baz", &Scales::default()), 4);
  }

  /// A scaled cluster wraps at its scaled width, a line is as tall as its tallest
  /// cluster, a hit lands on the cluster whose block holds it, and the longest word
  /// counts the scale.
  #[test]
  fn sized_text_wraps_and_hits_by_block() {
    text::reset();

    let scales = Scales::of([(3, CellScale::Whole(2)), (10, CellScale::ONE)]);
    let lines = wrap_sized("abc de", 8, &scales);

    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!((lines[0].text.as_str(), lines[0].rows), ("abc", 2));
    assert_eq!((lines[1].text.as_str(), lines[1].rows), ("de", 1));
    assert_eq!(lines[0].cells(&scales), 6);
    assert_eq!(lines[0].cluster_at(3, &scales), Some(1));
    assert_eq!(lines[0].cluster_at(6, &scales), None);
    assert_eq!(lines[0].index_at_column(1, &scales), 1);
    assert_eq!(lines[0].index_at_column(2, &scales), 1);
    assert_eq!(lines[0].index_at_column(9, &scales), 3);
    assert_eq!(sized_rows(&lines), [0, 2]);
    assert_eq!(line_at_row(&lines, 1), 0);
    assert_eq!(line_at_row(&lines, 2), 1);
    assert_eq!(line_at_row(&lines, 7), 1);
    assert_eq!(line_holding(&lines, 1), Some(0));
    assert_eq!(line_holding(&lines, 3), None);
    assert_eq!(longest_word("abc de", &scales), 6);
    assert_eq!(
      wrap_sized("ab cd", 8, &Scales::uniform(CellScale::Whole(2)))[0].rows,
      2
    );
    assert_eq!(
      wrap_sized("ab cd", 8, &Scales::uniform(CellScale::Whole(3))).len(),
      2,
      "at scale three two letters fill six of eight cells"
    );
  }
}
