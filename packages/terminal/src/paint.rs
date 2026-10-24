//! Painting the laid-out tree into a cell buffer.
//!
//! The tree paints back-to-front into a [`Buffer`], overlays in a second pass over
//! it; the buffer serializes to plain text for tests or to the terminal for the
//! interactive loop.

use std::rc::Rc;

use skene_foundation::{
  Axis,
  Color,
  Inset,
  Length,
  NativeElement,
  Scene,
  TextDecoration,
  TextDecorationStyle,
  text::TextIndex,
};

use crate::{
  BorderGlyphs,
  ScrollbarGlyphs,
  Terminal,
  cells::Cells,
  engine::{Drawn, drawn},
  layout::{
    Source,
    border_edges,
    flow_of,
    has_border,
    inline_boxes,
    is_inline_box,
    is_paragraph,
    padding_cells,
  },
  records::{Contrast, Handle, Kind, PlainBox, PlainRun, Record, Records, Visible},
  text::{self, CellScale, Glyph},
};

/// One terminal cell's paint state.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct TerminalCell {
  /// The glyph the cell shows.
  pub(crate) glyph: Glyph,
  /// The glyph's ink; `None` for the terminal's own.
  pub(crate) foreground: Option<(u8, u8, u8)>,
  /// The cell's surface; `None` for the terminal's own.
  pub(crate) background: Option<(u8, u8, u8)>,
  /// Whether the glyph draws bold.
  pub(crate) bold: bool,
  /// The decoration lines the cell carries.
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

/// The decoration lines one painted cell carries, for an in-process test asserting paint.
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
  /// The leftmost screen column covered.
  pub(crate) column: u16,
  /// The top screen row covered.
  pub(crate) row: u16,
  /// How many columns are covered.
  pub(crate) columns: u16,
  /// How many rows are covered.
  pub(crate) rows: u16,
  /// The source rectangle: left, top, width, height.
  pub(crate) crop: (u16, u16, u16, u16),
  /// The layer: zero over the text for a painted box, beneath every colored cell
  /// for a backdrop.
  pub(crate) layer: i32,
  /// The image placed.
  pub(crate) image: Rc<Image>,
}

/// A placement's cells, crop and layer: what the terminal is told, apart from the
/// image.
pub(crate) type PlacedCells = (u16, u16, u16, u16, (u16, u16, u16, u16), i32);

impl Placement {
  /// The cells, the crop and the layer: what the terminal is told, apart from the
  /// image.
  pub(crate) const fn cells(&self) -> PlacedCells {
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

/// A painted box's picture as one frame places it, for an in-process test asserting what
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

/// A cell's look apart from its glyph, as a styled frame names it.
#[derive(Clone, Copy, PartialEq)]
struct CellStyle {
  /// The glyph's ink; `None` for the terminal's own.
  foreground: Option<(u8, u8, u8)>,
  /// The cell's surface; `None` for the terminal's own.
  background: Option<(u8, u8, u8)>,
  /// Whether the glyph draws bold.
  bold: bool,
  /// Whether the cell paints in reverse video.
  reverse: bool,
  /// The decoration lines the cell carries.
  decoration: CellDecoration,
  /// Whether the cell is part of a hyperlink.
  link: bool,
  /// The scale the cell's glyph draws at.
  scale: CellScale,
}

impl CellStyle {
  /// The look of `cell`.
  const fn of(cell: &TerminalCell) -> Self {
    Self {
      foreground: cell.foreground,
      background: cell.background,
      bold: cell.bold,
      reverse: cell.reverse,
      decoration: cell.decoration,
      link: cell.link.is_some(),
      scale: cell.scale,
    }
  }

  /// Whether the cell paints as the terminal's own, with nothing on it.
  fn is_plain(&self) -> bool {
    *self == Self::of(&TerminalCell::EMPTY)
  }

  /// The character naming the `index`th style a frame met.
  fn key(index: usize) -> char {
    const KEYS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    KEYS.get(index).map_or_else(
      || char::from_u32(0x100 + u32::try_from(index).unwrap_or(0)).unwrap_or('?'),
      |key| char::from(*key),
    )
  }
}

impl std::fmt::Display for CellStyle {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let hex = |(red, green, blue): (u8, u8, u8)| format!("#{red:02x}{green:02x}{blue:02x}");
    let mut parts = Vec::new();

    if let Some(color) = self.foreground {
      parts.push(format!("ink {}", hex(color)));
    }

    if let Some(color) = self.background {
      parts.push(format!("surface {}", hex(color)));
    }

    if self.bold {
      parts.push("bold".to_owned());
    }

    if self.reverse {
      parts.push("reverse".to_owned());
    }

    if let Some(style) = self.decoration.underline {
      parts.push(match self.decoration.underline_color {
        Some(color) => format!("underline {style:?} {}", hex(color)),
        None => format!("underline {style:?}"),
      });
    }

    if self.decoration.overline {
      parts.push("overline".to_owned());
    }

    if self.decoration.line_through {
      parts.push("line through".to_owned());
    }

    if self.link {
      parts.push("link".to_owned());
    }

    if self.scale != CellScale::ONE {
      parts.push(format!("{:?}", self.scale));
    }

    formatter.write_str(&parts.join(", "))
  }
}

/// A painted frame: `width` x `height` cells in row-major order.
#[derive(Clone)]
pub(crate) struct Buffer {
  /// The frame's width, in cells.
  pub(crate) width: usize,
  /// The frame's height, in cells.
  pub(crate) height: usize,
  /// The cells, in row-major order.
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

  /// Whether a repaint rewrites any of the `height` rows from `y`: every row of a whole
  /// frame, the damaged ones of a partial one.
  fn repaints(&self, y: isize, height: usize) -> bool {
    let Some(rows) = &self.rows
    else {
      return true;
    };
    let top = usize::try_from(y.max(0)).unwrap_or(0).min(rows.len());
    let bottom = usize::try_from(y.saturating_add_unsigned(height).max(0))
      .unwrap_or(0)
      .min(rows.len());

    rows[top..bottom].contains(&true)
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

  /// Paints `cluster` at (`x`, `y`) at `scale` across the `cells` it advances, as
  /// [`put_sized`](Self::put_sized) does.
  pub(crate) fn put_advancing(
    &mut self,
    x: isize,
    y: isize,
    cluster: &str,
    cells: usize,
    scale: CellScale,
    style: impl Fn(&mut TerminalCell),
  ) {
    self.put_sized(x, y, cluster, cells / scale.rows(), scale, style);
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

  /// The frame as text with its styles, for a file a test compares.
  ///
  /// The text as [`to_text`](Self::to_text) draws it; after a blank line, each cell's
  /// style as a character, `.` for a cell painting as the terminal's own; after another,
  /// the style each character stands for.
  pub(crate) fn to_styled(&self) -> String {
    use std::fmt::Write as _;

    let mut styles: Vec<CellStyle> = Vec::new();
    let mut out = self.to_text();

    out.push('\n');

    for row in self.cells.chunks(self.width.max(1)) {
      let mut line = String::new();

      for cell in row {
        let style = CellStyle::of(cell);

        if style.is_plain() {
          line.push('.');

          continue;
        }

        let index = styles
          .iter()
          .position(|known| *known == style)
          .unwrap_or_else(|| {
            styles.push(style);

            styles.len() - 1
          });

        line.push(CellStyle::key(index));
      }

      out.push_str(line.trim_end_matches('.'));
      out.push('\n');
    }

    out.push('\n');

    for (index, style) in styles.iter().enumerate() {
      let _ = writeln!(out, "{} {style}", CellStyle::key(index));
    }

    out
  }
}

/// Paints the frame into `buffer`: the tree, then every open overlay, and mends the
/// wide glyphs a later paint broke into; a picture an open overlay covers in part is
/// left out.
pub(crate) fn paint(records: &Records, buffer: &mut Buffer) {
  paint_node(records, buffer, records.root, 0, 0);

  let overlays = records.open_overlays();

  buffer.pictures.retain(|picture| {
    picture.layer < 0
      || !overlays.iter().any(|overlay| {
        let (origin_x, origin_y) = records.overlay_origin(*overlay);
        let (width, height) = records.node_size(*overlay);

        i64::from(picture.column) < origin_x + width
          && i64::from(picture.column + picture.columns) > origin_x
          && i64::from(picture.row) < origin_y + height
          && i64::from(picture.row + picture.rows) > origin_y
      })
  });

  for overlay in overlays {
    paint_overlay(records, buffer, overlay);
  }

  buffer.mend_blocks();
}

/// Paints one open overlay over the frame: its backdrop scrim, then its subtree at its
/// computed origin.
fn paint_overlay(records: &Records, buffer: &mut Buffer, overlay: Handle) {
  let Some(Kind::Overlay { backdrop, .. }) = records.record(overlay).map(|record| &record.kind)
  else {
    return;
  };
  let modal = Scene::<Terminal>::read(|scene| scene.is_modal(overlay));
  let contrast = records.contrast();

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

  let (origin_x, origin_y) = records.overlay_origin(overlay);

  // A modal makes everything beneath inert, taking the caret with it; a popup hides
  // the caret only when it covers the cell. Either way the overlay's own content may
  // set a fresh one while it paints.
  if let Some((caret_x, caret_y)) = buffer.caret {
    let (overlay_width, overlay_height) = records.node_size(overlay);
    let covered = i64::from(caret_x) >= origin_x
      && i64::from(caret_x) < origin_x + overlay_width
      && i64::from(caret_y) >= origin_y
      && i64::from(caret_y) < origin_y + overlay_height;

    if modal || covered {
      buffer.caret = None;
    }
  }

  let (location_x, location_y) = records.node_location(overlay);

  paint_node(
    records,
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
  edges: Inset<bool>,
  shown: bool,
}

/// One paragraph span's paint: its ink, weight, decoration, link, and the emphasis it
/// paints in while it is the link whose focus shows.
struct SpanPaint {
  /// The span's ink; `None` for the terminal's own.
  foreground: Option<(u8, u8, u8)>,
  /// Whether the span draws bold.
  bold: bool,
  /// The decoration lines the span's cells carry.
  decoration: CellDecoration,
  /// The span's hyperlink, an index into the frame's [`Buffer::links`].
  link: Option<u16>,
  /// The emphasis of the link whose focus shows; `None` while the span is not it.
  focused: Option<Emphasis>,
  /// The colors the span's selected cells paint in; reverse video when `None`.
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

/// One cluster a text draws: where it draws, the span it belongs to, its scale, and
/// whether it is selected.
struct ClusterPaint {
  drawn: Drawn,
  span: u16,
  scale: CellScale,
  selected: bool,
}

/// What one node contributes to the frame.
enum NodePaint {
  Paragraph {
    /// Each cluster the paragraph's layout draws, in visual order.
    clusters: Vec<ClusterPaint>,
    spans: Vec<SpanPaint>,
    links: Vec<String>,
    /// Each inline box, with the cell its frame's top left is in.
    boxes: Vec<(Handle, (i64, i64))>,
  },
  Box {
    background: Option<(u8, u8, u8)>,
    border: Option<BorderPaint>,
    shadow: Option<ShadowPaint>,
  },
  Text {
    /// Each cluster the run's layout draws, in visual order.
    clusters: Vec<ClusterPaint>,
    foreground: Option<(u8, u8, u8)>,
    bold: bool,
    decoration: CellDecoration,
    link: Option<String>,
    /// The emphasis of the link whose focus shows; `None` while the run is not it.
    focused: Option<Emphasis>,
    /// The colors the run's selected cells paint in; reverse video when `None`.
    selection: Option<SelectionColors>,
  },
  Field {
    background: Option<(u8, u8, u8)>,
    border: Option<BorderPaint>,
    shadow: Option<ShadowPaint>,
    /// What the entry shows in its area, the caret only while it holds focus.
    visible: Visible,
    /// The value's ink, the terminal's own where `None`.
    foreground: Option<(u8, u8, u8)>,
    bold: bool,
    inset: (isize, isize),
    /// The colors the selected cells paint in; reverse video when `None`.
    selection: Option<SelectionColors>,
  },
  None,
}

/// Each cluster the text node `handle`, laid out `width` cells wide, draws, with its
/// span (the index of its run among `source`'s), its run's scale and whether `selection`,
/// a range of `source`'s clusters, holds it.
fn cluster_paints(
  records: &Records,
  handle: Handle,
  width: usize,
  source: &Source,
  selection: Option<(usize, usize)>,
) -> Vec<ClusterPaint> {
  let Some(flow) = flow_of(records, handle)
  else {
    return Vec::new();
  };
  let laid = flow.laid(width);
  let text = flow.flow().text();
  let runs: Vec<Handle> = source.runs().map(|(run, _)| run).collect();
  let scales: Vec<CellScale> = runs.iter().map(|run| run_scale(records, *run)).collect();

  drawn(&laid, text, &TextIndex::new(text))
    .into_iter()
    .filter_map(|drawn| {
      let (run, cluster) = source.at_flow(drawn.at)?;
      let span = runs.iter().position(|known| *known == run)?;

      Some(ClusterPaint {
        drawn,
        span: u16::try_from(span).unwrap_or(0),
        scale: scales[span],
        selected: selection.is_some_and(|(from, to)| cluster >= from && cluster < to),
      })
    })
    .collect()
}

/// The border paint of a resolved box style: in its color, the terminal's own ink where
/// it sets none, and unshown where its color is transparent.
fn border_paint(records: &Records, handle: Handle) -> Option<BorderPaint> {
  let style = &records
    .record(handle)?
    .kind
    .box_record()?
    .cache
    .as_ref()?
    .style;

  has_border(style).then(|| BorderPaint {
    glyphs: style.ext.border_glyphs.unwrap_or_default(),
    color: style.border_color.and_then(terminal_rgb),
    edges: border_edges(style),
    shown: style
      .border_color
      .is_none_or(|color| terminal_rgb(color).is_some()),
  })
}

/// The selection colors of a resolved run: its own, or reverse video when both are
/// unset or transparent.
fn selection_colors(style: &PlainRun) -> Option<SelectionColors> {
  emphasis_colors(style.selection_color, style.selection_background)
}

/// The emphasis a link run paints in while its focus shows: the typography's colors,
/// or reverse video when both are unset or transparent.
fn link_focus_emphasis(records: &Records, run: Handle) -> Emphasis {
  let (ink, surface) = crate::resolve::Env::of(records, run).resolve_link_focus();

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
#[expect(
  clippy::too_many_lines,
  reason = "a match dispatch over the record kinds; length is the kind count, not complexity"
)]
fn node_paint(records: &Records, handle: Handle, width: usize, focused: bool) -> NodePaint {
  let Some(record) = records.record(handle)
  else {
    return NodePaint::None;
  };

  match &record.kind {
    Kind::Box(_) if is_paragraph(records, handle) => {
      let source = Source::of(records, handle);
      let mut destinations = Vec::new();
      let spans: Vec<SpanPaint> = source
        .runs()
        .map(|(run, _)| {
          let Some(Kind::Run { facts, cache, .. }) = records.record(run).map(|record| &record.kind)
          else {
            unreachable!("paragraph runs are runs");
          };
          let style = cache.as_ref().map(|cache| &cache.style);
          let link = facts.link.as_ref().map(|url| {
            destinations.push(url.clone());
            u16::try_from(destinations.len() - 1).unwrap_or(0)
          });

          SpanPaint {
            foreground: style.and_then(|style| style.color).and_then(terminal_rgb),
            bold: style.and_then(|style| style.ext.bold).unwrap_or(false),
            decoration: CellDecoration::of(style, link.is_some()),
            link,
            focused: (link.is_some() && records.focus_shows(run))
              .then(|| link_focus_emphasis(records, run)),
            selection: style.and_then(selection_colors),
          }
        })
        .collect();

      NodePaint::Paragraph {
        clusters: cluster_paints(
          records,
          handle,
          width,
          &source,
          records.selection_range(handle),
        ),
        spans,
        links: destinations,
        boxes: inline_boxes(records, handle),
      }
    }
    Kind::Box(inner) | Kind::Overlay { inner, .. } => {
      let style = inner.style();

      NodePaint::Box {
        background: style
          .and_then(|style| style.background)
          .and_then(terminal_rgb),
        border: border_paint(records, handle),
        shadow: style.and_then(ShadowPaint::of),
      }
    }
    Kind::Control { inner, field, .. } => {
      let style = inner.style();
      let background = style
        .and_then(|style| style.background)
        .and_then(terminal_rgb);
      let shadow = style.and_then(ShadowPaint::of);
      let border = border_paint(records, handle);

      let Some(field) = field
      else {
        return NodePaint::Box {
          background,
          border,
          shadow,
        };
      };
      let padding = style.map(padding_cells).unwrap_or_default();
      let border_cells = usize::from(border.is_some());
      let inset = (
        isize::try_from(border_cells).unwrap_or(0) + isize::try_from(padding.start).unwrap_or(0),
        isize::try_from(border_cells).unwrap_or(0) + isize::try_from(padding.top).unwrap_or(0),
      );
      let area = records.field_area(handle);
      let mut visible = field.visible(area, focused);
      let run = crate::resolve::Env::of(records, handle).resolve_entry_run();

      if !(focused && area.columns > 0) {
        visible.caret = None;
      }

      NodePaint::Field {
        background,
        border,
        shadow,
        visible,
        foreground: run.color.and_then(terminal_rgb),
        bold: run.ext.bold.unwrap_or(false),
        inset,
        selection: selection_colors(&run),
      }
    }
    Kind::Run { .. } if is_inline_box(records, handle) => NodePaint::None,
    Kind::Run { facts, cache, .. } => {
      let style = cache.as_ref().map(|cache| &cache.style);
      let selection = facts
        .selectable
        .then(|| records.selection_range(handle))
        .flatten();

      NodePaint::Text {
        clusters: cluster_paints(
          records,
          handle,
          width,
          &Source::of(records, handle),
          selection,
        ),
        foreground: style.and_then(|style| style.color).and_then(terminal_rgb),
        bold: style.and_then(|style| style.ext.bold).unwrap_or(false),
        decoration: CellDecoration::of(style, facts.link.is_some()),
        link: facts.link.clone(),
        focused: (facts.link.is_some() && focused).then(|| link_focus_emphasis(records, handle)),
        selection: style.and_then(selection_colors),
      }
    }
  }
}

/// Paints `handle` at its layout position offset by (`offset_x`, `offset_y`), then its
/// children.
#[expect(
  clippy::too_many_lines,
  reason = "a match dispatch over the paints; length is the kind count, not complexity"
)]
fn paint_node(
  records: &Records,
  buffer: &mut Buffer,
  handle: Handle,
  offset_x: isize,
  offset_y: isize,
) {
  let Some(record) = records.record(handle)
  else {
    for child in records.children(handle) {
      if records.record(child).is_some_and(Record::is_overlay) {
        continue;
      }

      paint_node(records, buffer, child, offset_x, offset_y);
    }

    return;
  };

  if record.is_invisible() {
    return;
  }

  let frame = records.frame_of(record);
  let (x, y) = (
    offset_x + isize::try_from(frame.x).unwrap_or(0),
    offset_y + isize::try_from(frame.y).unwrap_or(0),
  );
  let (width, height) = (
    usize::try_from(frame.width).unwrap_or(0),
    usize::try_from(frame.height).unwrap_or(0),
  );
  let run = matches!(record.kind, Kind::Run { .. });
  let reach = record.kind.box_style().map_or(0, |style| {
    isize::try_from(ShadowPaint::band(style)).unwrap_or(0)
  });
  let shown = buffer
    .visible(
      x - reach,
      y - reach,
      isize::try_from(width).unwrap_or(isize::MAX) + 2 * reach,
      isize::try_from(height).unwrap_or(isize::MAX) + 2 * reach,
    )
    .is_some();
  let repainted = !run || buffer.repaints(y, height);

  if (run && !(shown && repainted)) || (!shown && records.clips(handle)) {
    return;
  }

  let border = if run {
    (0, 0, 0, 0)
  }
  else {
    let border = records.node_border(handle);

    (
      isize::try_from(border.0).unwrap_or(0),
      isize::try_from(border.1).unwrap_or(0),
      isize::try_from(border.2).unwrap_or(0),
      isize::try_from(border.3).unwrap_or(0),
    )
  };
  let edges = border;
  let paragraph = !run && is_paragraph(records, handle);

  if shown && repainted {
    let focused = records.focus_shows(handle);
    let contrast = records.contrast();

    match node_paint(records, handle, width, focused) {
      NodePaint::Paragraph {
        clusters,
        spans,
        links,
        boxes,
      } => {
        // The paragraph's links join the frame's.
        let links: Vec<u16> = links.iter().map(|link| buffer.link(link)).collect();
        let text = flow_of(records, handle).map_or("", |flow| flow.flow().text());

        for cluster in &clusters {
          let Some(paint) = spans.get(usize::from(cluster.span))
          else {
            continue;
          };
          let Drawn {
            at,
            end,
            column,
            top,
            advance,
          } = cluster.drawn;

          buffer.put_advancing(
            x + isize::try_from(column).expect("fits"),
            y + isize::try_from(top).expect("fits"),
            &text[at..end],
            advance,
            cluster.scale,
            |cell| {
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

              if cluster.selected {
                select_cell(cell, paint.selection);
              }
            },
          );
        }

        // An inline box paints its own layout at the cell its frame lies at.
        for (run, (cell_x, cell_y)) in boxes {
          let (location_x, location_y) = records.node_location(run);

          paint_node(
            records,
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
        if place_backdrop(records, buffer, handle, (x, y), (width, height)) {
          // Nothing in cells.
        }
        else {
          if let Some(shadow) = shadow {
            draw_shadow(buffer, x, y, width, height, shadow, contrast);
          }

          // A childless, borderless box paints from its parent's origin by eighths:
          // whole cells in its color, and a cell an edge crosses as a block glyph
          // carrying its color beside what lies beneath.
          match records.smooth_span(handle).filter(|_| border.is_none()) {
            Some((start, extent)) => {
              draw_smooth(buffer, offset_x, y, height, start, extent, background);
            }
            None => fill(buffer, x, y, width, height, background),
          }

          if let Some(border) = border
            && border.shown
          {
            draw_border(buffer, x, y, width, height, &border);
          }
        }

        if let Some(painter) = &record.painter {
          let size = skene_foundation::Size::new(
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
          && records.graphics
        {
          place_image(
            records,
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
        clusters,
        foreground,
        bold,
        decoration,
        link,
        focused,
        selection,
      } => {
        let link = link.map(|url| buffer.link(&url));
        let text = flow_of(records, handle).map_or("", |flow| flow.flow().text());

        for cluster in &clusters {
          let Drawn {
            at,
            end,
            column,
            top,
            advance,
          } = cluster.drawn;

          buffer.put_advancing(
            x + isize::try_from(column).expect("fits"),
            y + isize::try_from(top).expect("fits"),
            &text[at..end],
            advance,
            cluster.scale,
            |cell| {
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

              if cluster.selected {
                select_cell(cell, selection);
              }
            },
          );
        }
      }
      NodePaint::Field {
        background,
        border,
        shadow,
        visible,
        foreground,
        bold,
        inset,
        selection,
      } => {
        if !place_backdrop(records, buffer, handle, (x, y), (width, height)) {
          if let Some(shadow) = shadow {
            draw_shadow(buffer, x, y, width, height, shadow, contrast);
          }

          fill(buffer, x, y, width, height, background);

          if let Some(border) = border
            && border.shown
          {
            draw_border(buffer, x, y, width, height, &border);
          }
        }

        for cluster in &visible.clusters {
          buffer.put_cluster(
            x + inset.0 + isize::try_from(cluster.column).expect("fits"),
            y + inset.1 + isize::try_from(cluster.row).expect("fits"),
            &cluster.text,
            cluster.advance,
            |cell| {
              cell.foreground = if visible.placeholder {
                Some(halfway(
                  foreground.unwrap_or(contrast.ink),
                  cell.background.unwrap_or(contrast.base),
                ))
              }
              else {
                foreground
              };
              cell.bold = bold;

              if cluster.selected {
                select_cell(cell, selection);
              }
            },
          );
        }

        // The caret: the terminal's own bar cursor at the cell, recorded only while the
        // cell is visible under the current clip.
        if let Some((row, column)) = visible.caret {
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
  }

  if paragraph {
    return;
  }

  let saved_clip = buffer.clip;

  if records.clips(handle) {
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

  let (scroll_x, scroll_y) = records.scroll_offset(handle);
  let (child_x, child_y) = (
    x - isize::try_from(scroll_x).unwrap_or(0),
    y - isize::try_from(scroll_y).unwrap_or(0),
  );

  for child in records.children(handle) {
    if records.record(child).is_some_and(Record::is_overlay) {
      continue;
    }

    paint_node(records, buffer, child, child_x, child_y);
  }

  buffer.clip = saved_clip;

  if records.is_scrollable(handle) {
    draw_scrollbars(records, buffer, handle, x, y, width, height, border);
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
  records: &Records,
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
  let (cell_width, cell_height) = records.cell_pixels;
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
  records: &Records,
  buffer: &mut Buffer,
  handle: Handle,
  (x, y): (isize, isize),
  (width, height): (usize, usize),
) -> bool {
  if !records.graphics {
    return false;
  }

  let Some(backdrop) = records
    .record(handle)
    .and_then(|record| record.backdrop.as_ref())
  else {
    return false;
  };
  let ancestors = records.ancestors(handle);
  let depth = ancestors.len()
    + 512
      * usize::from(
        ancestors
          .iter()
          .any(|ancestor| records.record(*ancestor).is_some_and(Record::is_overlay)),
      );
  let layer = BACKDROP_Z + i32::try_from(depth.min(1023)).unwrap_or(1023);
  let band = isize::try_from(backdrop.key.band).unwrap_or(0);

  place_image(
    records,
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
#[expect(
  clippy::too_many_arguments,
  reason = "each argument is a separate fact of the box the scrollbars are drawn on"
)]
fn draw_scrollbars(
  records: &Records,
  buffer: &mut Buffer,
  handle: Handle,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  border: (isize, isize, isize, isize),
) {
  let (offset_x, offset_y) = records.scroll_offset(handle);
  let (limit_x, limit_y) = records.scroll_limits(handle);
  let (width, height) = (
    isize::try_from(width).unwrap_or(0),
    isize::try_from(height).unwrap_or(0),
  );
  let contrast = records.contrast();
  let style = records
    .record(handle)
    .and_then(|record| record.kind.box_style());
  let bar = BarStyle {
    glyphs: style
      .and_then(|style| style.ext.scrollbar_glyphs)
      .unwrap_or_default(),
    thumb: style
      .and_then(|style| style.ext.scrollbar_thumb)
      .and_then(terminal_rgb),
    track: style
      .and_then(|style| style.ext.scrollbar_track)
      .and_then(terminal_rgb),
  };

  if limit_y > 0 {
    let track = height - border.1 - border.3;
    let (top, thumb) = thumb_span_eighths(track, offset_y, limit_y);

    for row in 0..track {
      if let Some(cell) = buffer.cell_mut(x + width - 1, y + border.1 + row) {
        bar.paint(
          cell,
          contrast,
          eighth_coverage(row, top, thumb),
          Axis::Vertical,
        );
      }
    }
  }

  if limit_x > 0 {
    let track = width - border.0 - border.2;
    let (start, thumb) = thumb_span_eighths(track, offset_x, limit_x);

    for column in 0..track {
      if let Some(cell) = buffer.cell_mut(x + border.0 + column, y + height - 1) {
        bar.paint(
          cell,
          contrast,
          eighth_coverage(column, start, thumb),
          Axis::Horizontal,
        );
      }
    }
  }
}

/// A box's scrollbars as its style draws them: their glyphs, and their thumb's and
/// track's colors where set.
#[derive(Clone, Copy)]
struct BarStyle {
  glyphs: ScrollbarGlyphs,
  thumb: Option<(u8, u8, u8)>,
  track: Option<(u8, u8, u8)>,
}

impl BarStyle {
  /// Paints one cell of the scrollbar along `axis` the thumb covers by `coverage`.
  ///
  /// A full-block thumb moves by eighths over a track drawn as the cell's background,
  /// its partial cells the block elements; any other thumb moves by whole cells, a cell
  /// at least half covered taking it, and both it and the track draw as lines over
  /// the cell's own background.
  fn paint(self, cell: &mut TerminalCell, contrast: Contrast, coverage: Coverage, axis: Axis) {
    let (ink, tint) = self.colors(cell, contrast);
    let (thumb, track) = match axis {
      Axis::Vertical => (self.glyphs.vertical_thumb, self.glyphs.vertical_track),
      Axis::Horizontal => (self.glyphs.horizontal_thumb, self.glyphs.horizontal_track),
    };
    let (character, foreground, background) = if thumb == '█' {
      // A partial whose block Unicode lacks paints the complement glyph with the
      // colors swapped: the upper blocks down a vertical bar, the right ones along a
      // horizontal one.
      let (character, foreground, background) = match (coverage, axis) {
        (Coverage::Full, _) => ('█', ink, tint),
        (Coverage::Leading(eighths), Axis::Vertical) => (lower_block(8 - eighths), tint, ink),
        (Coverage::Trailing(eighths), Axis::Vertical) => (lower_block(eighths), ink, tint),
        (Coverage::Leading(eighths), Axis::Horizontal) => (left_block(eighths), ink, tint),
        (Coverage::Trailing(eighths), Axis::Horizontal) => (left_block(8 - eighths), tint, ink),
        (Coverage::None, _) => (track, ink, tint),
      };

      (character, foreground, Some(background))
    }
    else {
      let covered = match coverage {
        Coverage::Full => true,
        Coverage::Leading(eighths) | Coverage::Trailing(eighths) => eighths >= 4,
        Coverage::None => false,
      };

      if covered {
        (thumb, ink, cell.background)
      }
      else {
        (track, tint, cell.background)
      }
    };

    cell.glyph = Glyph::Char(character);
    cell.foreground = Some(foreground);
    cell.background = background;
    cell.bold = false;
    cell.reverse = false;
  }

  /// One scrollbar cell's thumb and track colors: the style's where set, else the
  /// terminal's foreground under `contrast` and a quarter of the thumb's color over the
  /// cell beneath.
  fn colors(self, cell: &TerminalCell, contrast: Contrast) -> ((u8, u8, u8), (u8, u8, u8)) {
    let Contrast { ink, base } = contrast;
    let ink = self.thumb.unwrap_or(ink);

    (
      ink,
      self
        .track
        .unwrap_or_else(|| quarter_over(ink, cell.background.unwrap_or(base))),
    )
  }
}

/// A quarter of `ink` over `beneath`.
fn quarter_over(ink: (u8, u8, u8), beneath: (u8, u8, u8)) -> (u8, u8, u8) {
  let quarter = |over: u8, under: u8| -> u8 {
    u8::try_from((u16::from(over) + 3 * u16::from(under) + 2) / 4).unwrap_or(u8::MAX)
  };

  (
    quarter(ink.0, beneath.0),
    quarter(ink.1, beneath.1),
    quarter(ink.2, beneath.2),
  )
}

/// How much of a track cell the thumb covers, in eighth-cell resolution.
enum Coverage {
  /// The whole cell.
  Full,
  /// The leading eighths only: the thumb ends mid-cell.
  Leading(u8),
  /// The trailing eighths only: the thumb starts mid-cell.
  Trailing(u8),
  /// None of the cell.
  None,
}

/// The thumb's coverage of the track cell at `position`, against an eighth-cell span.
#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "a partial coverage is positive and below eight eighths when narrowed"
)]
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
const fn left_color(cell: &TerminalCell) -> Option<(u8, u8, u8)> {
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

/// Draws a one-cell border on the edges `border` names, in its glyph set: a corner where
/// two drawn edges meet, and an edge's line running through the corner where the other
/// is undrawn.
fn draw_border(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  border: &BorderPaint,
) {
  if width == 0 || height == 0 {
    return;
  }

  let BorderPaint {
    glyphs,
    color,
    edges,
    ..
  } = *border;
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

  for column in 0..width {
    let cx = x + isize::try_from(column).expect("fits");

    if edges.top {
      put(cx, y, glyphs.horizontal);
    }

    if edges.bottom {
      put(cx, bottom, glyphs.horizontal);
    }
  }

  for row in 0..height {
    let cy = y + isize::try_from(row).expect("fits");

    if edges.start {
      put(x, cy, glyphs.vertical);
    }

    if edges.end {
      put(right, cy, glyphs.vertical);
    }
  }

  if edges.top && edges.start {
    put(x, y, glyphs.top_left);
  }

  if edges.top && edges.end {
    put(right, y, glyphs.top_right);
  }

  if edges.bottom && edges.start {
    put(x, bottom, glyphs.bottom_left);
  }

  if edges.bottom && edges.end {
    put(right, bottom, glyphs.bottom_right);
  }
}

/// Paints one cell as selected: in the selection colors, or in reverse video.
const fn select_cell(cell: &mut TerminalCell, colors: Option<SelectionColors>) {
  match colors {
    Some((ink, surface)) => {
      cell.foreground = Some(ink);
      cell.background = Some(surface);
    }
    None => cell.reverse = true,
  }
}

/// The scale the run `handle` draws at: its resolved size where the terminal sizes
/// text, one cell elsewhere and for an inline box.
pub(crate) fn run_scale(records: &Records, handle: Handle) -> CellScale {
  if is_inline_box(records, handle) {
    return CellScale::ONE;
  }

  match records.record(handle).map(|record| &record.kind) {
    Some(Kind::Run { cache, .. }) => {
      text::scale_of(cache.as_ref().and_then(|cache| cache.style.ext.size))
    }
    _ => CellScale::ONE,
  }
}

/// How far a shadow falls down and right where its box sets no offset, in cells.
pub(crate) const SHADOW_OFFSET: f32 = 0.5;

/// How far a shadow's edge softens past its offset where its box sets no softening, in
/// cells.
pub(crate) const SHADOW_SOFTENING: f32 = 0.5;

/// A box's shadow as cells draw it: its color and the cells it reaches down and right.
#[derive(Clone, Copy)]
pub(crate) struct ShadowPaint {
  color: Color,
  reach: isize,
}

impl ShadowPaint {
  /// The shadow `style` casts in cells, its offset rounded up; `None` for no shadow or
  /// one reaching no cell.
  pub(crate) fn of(style: &PlainBox) -> Option<Self> {
    let color = style.ext.shadow?;
    let (offset, _) = Self::extent(style);
    #[expect(
      clippy::cast_possible_truncation,
      reason = "an offset in cells is far below the integers isize holds"
    )]
    let reach = offset.ceil() as isize;

    (reach > 0).then_some(Self { color, reach })
  }

  /// The shadow's offset and softening in `style`, in cells, each at least zero.
  pub(crate) fn extent(style: &PlainBox) -> (f32, f32) {
    let length =
      |length: Option<Length>, unset: f32| length.map_or(unset, |length| length.0).max(0.);

    (
      length(style.ext.shadow_offset, SHADOW_OFFSET),
      length(style.ext.shadow_softening, SHADOW_SOFTENING),
    )
  }

  /// The cells the shadow `style` casts reaches down and right with its softening,
  /// where the terminal shows graphics; `0` for no shadow.
  pub(crate) fn band(style: &PlainBox) -> u16 {
    if style.ext.shadow.is_none() {
      return 0;
    }

    let (offset, softening) = Self::extent(style);
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "a reach in cells is at least zero and far below the integers u16 holds"
    )]
    let band = (offset + softening).ceil() as u16;

    band
  }
}

/// Casts a box's shadow: the box's rectangle moved down and right by the shadow's reach,
/// where it falls outside the box, the shadow color mixed into both the foreground and
/// the background of each covered cell by its alpha. An unset color beneath mixes
/// against the terminal's own under `contrast`.
fn draw_shadow(
  buffer: &mut Buffer,
  x: isize,
  y: isize,
  width: usize,
  height: usize,
  shadow: ShadowPaint,
  contrast: Contrast,
) {
  let [red, green, blue, alpha] = shadow.color.to_rgba();
  let alpha = alpha.clamp(0., 1.);

  if alpha <= 0. || width == 0 || height == 0 {
    return;
  }

  let (width, height) = (
    isize::try_from(width).unwrap_or(0),
    isize::try_from(height).unwrap_or(0),
  );
  let reach = shadow.reach;

  for row in reach..height + reach {
    for column in reach..width + reach {
      if row < height && column < width {
        continue;
      }

      if let Some(cell) = buffer.cell_mut(x + column, y + row) {
        tint(cell, red, green, blue, alpha, contrast);
      }
    }
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
      #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the channel is clamped to 0..=1 before it is scaled to 0..=255"
      )]
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

  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "each channel is clamped to 0..=1 before it is scaled to 0..=255"
  )]
  Some((
    (red.clamp(0., 1.) * 255.).round() as u8,
    (green.clamp(0., 1.) * 255.).round() as u8,
    (blue.clamp(0., 1.) * 255.).round() as u8,
  ))
}

/// The columns of `row` that differ from `previous`: from the first cell differing to
/// the last, widened to whole glyphs, and the whole row where nothing was painted before,
/// the frame's size moved or a scaled block stands in the row; `None` where none does.
pub(crate) fn changed_columns(
  previous: Option<&Buffer>,
  next: &Buffer,
  row: usize,
) -> Option<std::ops::Range<usize>> {
  let width = next.width;
  let Some(previous) =
    previous.filter(|previous| previous.width == next.width && previous.height == next.height)
  else {
    return Some(0..width);
  };
  let span = row * width..(row + 1) * width;
  let (before, after) = (&previous.cells[span.clone()], &next.cells[span]);
  let differs = |column: usize| {
    let (before, after) = (&before[column], &after[column]);

    TerminalCell {
      link: None,
      ..*before
    } != TerminalCell {
      link: None,
      ..*after
    } || previous.destination(before) != next.destination(after)
  };
  let first = (0..width).find(|&column| differs(column))?;
  let last = (first..width).rev().find(|&column| differs(column))?;

  if before
    .iter()
    .chain(after)
    .any(|cell| cell.scale != CellScale::ONE)
  {
    return Some(0..width);
  }

  let continues = |column: usize| {
    before[column].glyph == Glyph::Continuation || after[column].glyph == Glyph::Continuation
  };
  let mut start = first;
  let mut end = last + 1;

  while start > 0 && continues(start) {
    start -= 1;
  }

  while end < width && continues(end) {
    end += 1;
  }

  Some(start..end)
}

/// Whether the control `handle` is a text entry; `None` where it is no control.
pub(crate) fn control_entry(records: &Records, handle: Handle) -> Option<bool> {
  match records.record(handle)?.kind {
    Kind::Control { native, .. } => Some(native.is_some_and(NativeElement::is_text_entry)),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use skene_foundation::text::{AttributedText, Laid as _, Paragraph as _, TextEngine, TextRun};

  use super::*;
  use crate::{
    engine::{CellEngine, CellLaid, engine, paragraph_style, wrap_spans},
    layout::cell_point,
  };

  /// Wraps text to `width` cells: each line's shown clusters.
  fn wrap_text(text: &str, width: usize) -> Vec<String> {
    wrap_spans(text, width)
      .into_iter()
      .map(|(_, line)| line)
      .collect()
  }

  /// The paragraph of `text`, its stretches `scales`: each a byte length and the scale of
  /// its clusters.
  fn paragraph(text: &str, scales: &[(usize, CellScale)]) -> <CellEngine as TextEngine>::Paragraph {
    let mut at = 0;
    let runs = scales
      .iter()
      .map(|(length, scale)| {
        let range = at..(at + length).min(text.len());

        at = range.end;

        TextRun {
          range,
          style: scale.style(),
        }
      })
      .collect();

    engine().paragraph(
      &AttributedText::new(text, CellScale::ONE.style(), runs),
      &paragraph_style(),
    )
  }

  /// `text` laid out unbounded where `width` is `None`, its stretches `scales`.
  fn sized(text: &str, scales: &[(usize, CellScale)], width: Option<f32>) -> CellLaid {
    paragraph(text, scales).layout(width)
  }

  /// The widest stretch between two places a line may end, its trailing blanks aside, in
  /// cells at its scales: the narrowest a wrap can go.
  fn longest_word(text: &str, scales: &[(usize, CellScale)]) -> f32 {
    paragraph(text, scales).intrinsic().min
  }

  #[test]
  fn rows_diff_against_the_previous_frame() {
    let mut previous = Buffer::new(3, 2);
    let mut next = Buffer::new(3, 2);

    previous.cells[1].glyph = Glyph::Char('a');
    next.cells[1].glyph = Glyph::Char('a');
    next.cells[4].glyph = Glyph::Char('b');

    assert_eq!(changed_columns(Some(&previous), &next, 0), None);
    assert_eq!(changed_columns(Some(&previous), &next, 1), Some(1..2));
    assert_eq!(changed_columns(None, &next, 0), Some(0..3));

    let resized = Buffer::new(2, 2);

    assert_eq!(changed_columns(Some(&resized), &next, 0), Some(0..3));
  }

  /// A change inside a wide glyph widens to the whole glyph, and a scaled block's row
  /// rewrites whole.
  #[test]
  fn a_changed_span_covers_whole_glyphs() {
    text::reset();

    let mut previous = Buffer::new(4, 1);

    previous.put_sized(0, 0, "日", 2, CellScale::ONE, |_| {});

    let mut next = previous.clone();

    next.cells[1].background = Some((255, 0, 0));

    assert_eq!(changed_columns(Some(&previous), &next, 0), Some(0..2));

    let mut scaled = previous.clone();

    scaled.cells[3].scale = CellScale::Whole(2);

    assert_eq!(changed_columns(Some(&previous), &scaled, 0), Some(0..4));
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

    assert_eq!(
      changed_columns(Some(&frame(&["a", "b"], 1)), &frame(&["b"], 0), 0),
      None
    );
    assert!(changed_columns(Some(&frame(&["a"], 0)), &frame(&["b"], 0), 0).is_some());

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
    assert_eq!(longest_word("日本 ab", &[(9, CellScale::ONE)]), 2.);
  }

  /// A line ends where the Unicode line breaking algorithm allows: after a hyphen and
  /// between ideographs as well as at spaces, never before closing punctuation.
  #[test]
  fn lines_break_where_unicode_allows() {
    text::reset();

    assert_eq!(wrap_text("foo-bar", 5), ["foo-", "bar"]);
    assert_eq!(wrap_text("日本語テキスト", 6), ["日本語", "テキス", "ト"]);
    assert_eq!(wrap_text("one (two)", 6), ["one", "(two)"]);
    assert_eq!(longest_word("foo-bar baz", &[(11, CellScale::ONE)]), 4.);
  }

  /// A scaled cluster wraps at its scaled width, a line is as tall as its tallest
  /// cluster, a hit lands on the cluster whose block holds it, and the longest word
  /// counts the scale.
  #[test]
  fn sized_text_wraps_and_hits_by_block() {
    text::reset();

    let scales = [(3, CellScale::Whole(2)), (10, CellScale::ONE)];
    let laid = sized("abc de", &scales, Some(8.));
    let lines = laid.lines();
    let at = |column: usize, row: usize| {
      let point = cell_point(column, row);
      let hit = laid.hit_test(point.x, point.y);

      (hit.position.index, hit.inside)
    };

    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(
      (lines[0].range.clone(), lines[0].height, lines[0].width),
      (0..4, 2., 6.)
    );
    assert_eq!((lines[1].range.clone(), lines[1].height), (4..6, 1.));
    assert_eq!(
      at(3, 0),
      (2, true),
      "a hit on a block's later cell is on the text, a caret there after it"
    );
    assert!(!at(6, 0).1, "nothing is drawn past the line");
    assert_eq!(
      at(1, 0).0,
      1,
      "a later cell of a block places a caret after it"
    );
    assert_eq!(
      at(2, 0).0,
      1,
      "a block's first cell places a caret before it"
    );
    assert_eq!((lines[0].top, lines[1].top), (0., 2.));
    assert_eq!(at(0, 1).0, 0, "a block's lower row is its line's");
    assert_eq!(at(0, 2).0, 4);
    assert_eq!(at(0, 7).0, 4, "a hit below the text lands on its last line");
    assert_eq!(longest_word("abc de", &scales), 6.);
    assert_eq!(
      sized("ab cd", &[(5, CellScale::Whole(2))], Some(8.)).lines()[0].height,
      2.
    );
    assert_eq!(
      sized("ab cd", &[(5, CellScale::Whole(3))], Some(8.)).line_count(),
      2,
      "at scale three two letters fill six of eight cells"
    );
  }
}
