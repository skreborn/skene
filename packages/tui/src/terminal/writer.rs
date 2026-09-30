//! What the loop writes to the terminal: the modes it sets and restores, its queries,
//! and each frame, its rows and its pictures.

use std::{
  collections::HashMap,
  io::{self, Write},
};

use skene_core::{MouseCursor, TextDecorationStyle};

use crate::{
  detect::{self, ColorDepth},
  escape::{self, Color, CursorStyle, DecMode, MoveTo, QueryMode, Sgr, Underline},
  paint,
  text::{CellScale, Glyph},
  with_records,
};

/// A DEC private mode set or reset, as one control.
pub(super) const fn dec_mode(code: u16, on: bool) -> DecMode {
  DecMode { code, on }
}

/// Writes the controls that take the terminal over: the title saved, the alternate
/// screen, mouse reporting when `mouse`, bracketed paste, scheme notifications,
/// grapheme clustering, in-band resize notifications, autowrap off, and the cursor
/// hidden.
pub(super) fn write_setup(out: &mut (impl Write + ?Sized), mouse: bool) -> io::Result<()> {
  write!(
    out,
    "{}{}",
    escape::PUSH_TITLE,
    dec_mode(escape::ALTERNATE_SCREEN, true)
  )?;

  if mouse {
    write_mouse(out, true)?;
  }

  write!(
    out,
    "{}{}{}{}{}{}",
    dec_mode(escape::BRACKETED_PASTE, true),
    dec_mode(escape::THEME, true),
    dec_mode(escape::GRAPHEME_CLUSTERING, true),
    dec_mode(escape::IN_BAND_RESIZE, true),
    dec_mode(escape::AUTOWRAP, false),
    dec_mode(escape::SHOW_CURSOR, false),
  )
}

/// Writes the modes taking the mouse from the terminal when `on`, and handing it back
/// when not.
pub(super) fn write_mouse(out: &mut (impl Write + ?Sized), on: bool) -> io::Result<()> {
  write!(
    out,
    "{}{}{}{}",
    dec_mode(escape::MOUSE_TRACKING, on),
    dec_mode(escape::BUTTON_EVENT_MOUSE, on),
    dec_mode(escape::ANY_EVENT_MOUSE, on),
    dec_mode(escape::SGR_MOUSE, on),
  )
}

/// Writes the queries whose replies upgrade the launch assumptions: the scheme (a 2031
/// report, the OSC 11 background), the terminal's own colors (the OSC 10 foreground
/// and the OSC 4 base colors), the kitty keyboard flags, synchronized output,
/// grapheme clustering, in-band resize and the answering clipboard (DECRPM reports
/// for modes 2026, 2027, 2048 and 5522), styled underlines (a DECRQSS report after a curly
/// underline), the color depth (a DECRQSS report after a true-color and after a palette
/// background), and text sizing (a cursor position report after a space at scale two from the home
/// cell, which lands on the third column where the terminal sized it), and graphics
/// (the kitty protocol's query, answered `OK` where it is spoken); then the primary
/// device attributes request, whose reply closes the queries' reply window.
pub(super) fn write_queries(out: &mut (impl Write + ?Sized)) -> io::Result<()> {
  write!(
    out,
    "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
    escape::QUERY_THEME,
    QueryMode(escape::SYNCHRONIZED_OUTPUT),
    QueryMode(escape::GRAPHEME_CLUSTERING),
    QueryMode(escape::IN_BAND_RESIZE),
    QueryMode(escape::ANSWERING_CLIPBOARD),
    escape::QUERY_BACKGROUND,
    escape::QUERY_FOREGROUND,
    escape::QUERY_PALETTE,
    escape::QUERY_KEYBOARD_FLAGS,
    Sgr::Background(detect::TRUE_COLOR_PROBE),
    escape::REQUEST_GRAPHIC_RENDITION,
    Sgr::Background(Color::Palette(detect::PALETTE_PROBE)),
    escape::REQUEST_GRAPHIC_RENDITION,
    Sgr::Underline(Underline::Curly),
    escape::REQUEST_GRAPHIC_RENDITION,
    Sgr::Reset,
    move_to(0, 0),
    SIZING_PROBE,
    escape::REQUEST_CURSOR_POSITION,
    escape::GraphicsQuery(GRAPHICS_PROBE),
    escape::REQUEST_PRIMARY_ATTRIBUTES,
  )
}

/// Writes the queries for the terminal's own colors—the OSC 11 background, the OSC 10
/// foreground and the OSC 4 base colors—then the primary device attributes request,
/// whose reply closes their reply window.
pub(super) fn write_color_queries(out: &mut (impl Write + ?Sized)) -> io::Result<()> {
  write!(
    out,
    "{}{}{}{}",
    escape::QUERY_BACKGROUND,
    escape::QUERY_FOREGROUND,
    escape::QUERY_PALETTE,
    escape::REQUEST_PRIMARY_ATTRIBUTES,
  )
}

/// The image id the graphics probe asks under; a painted box's ids are its record's.
pub(super) const GRAPHICS_PROBE: u32 = 0x7fff_ffff;

/// A space at scale two: the text sizing probe, two cells wide where the terminal
/// sizes text and nothing elsewhere.
const SIZING_PROBE: &str = "\x1b]66;s=2; \x1b\\";

/// The column the probe's cursor position report names where the terminal sized it.
pub(super) const SIZED_COLUMN: u16 = 2;

/// Writes the controls that hand the terminal back: the graphic rendition reset, the
/// legacy key encoding, mouse reporting, bracketed paste, scheme and resize
/// notifications and grapheme clustering off, autowrap on, the cursor shown in its
/// default shape, the main screen, and the title restored. Idempotent but for the
/// title, which pops what setup saved.
fn write_restore(out: &mut (impl Write + ?Sized)) -> io::Result<()> {
  write!(
    out,
    "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
    Sgr::Reset,
    escape::PointerShape("default"),
    escape::KeyboardFlags(0),
    dec_mode(escape::THEME, false),
    dec_mode(escape::IN_BAND_RESIZE, false),
    dec_mode(escape::GRAPHEME_CLUSTERING, false),
    dec_mode(escape::BRACKETED_PASTE, false),
    dec_mode(escape::SGR_PIXELS_MOUSE, false),
    dec_mode(escape::SGR_MOUSE, false),
    dec_mode(escape::ANY_EVENT_MOUSE, false),
    dec_mode(escape::BUTTON_EVENT_MOUSE, false),
    dec_mode(escape::MOUSE_TRACKING, false),
    dec_mode(escape::AUTOWRAP, true),
    CursorStyle::Default,
    dec_mode(escape::SHOW_CURSOR, true),
    dec_mode(escape::ALTERNATE_SCREEN, false),
    escape::POP_TITLE,
  )
}

/// Writes the controls that free every picture and hand the terminal back.
pub(super) fn write_hand_back(out: &mut (impl Write + ?Sized)) -> io::Result<()> {
  write!(out, "{}", escape::FREE_ALL)?;
  write_restore(out)
}

/// Emits the terminal title via OSC 2; terminals without support ignore it.
pub(super) fn emit_title(out: &mut impl Write, title: &str) {
  let _ = write!(out, "{}", escape::Title(title));
  let _ = out.flush();
}

/// The OSC 22 name of a pointer shape; the terminal's own shape where none is set.
pub(super) const fn pointer_shape(cursor: Option<MouseCursor>) -> &'static str {
  match cursor {
    Some(MouseCursor::Click) => "pointer",
    Some(MouseCursor::Text) => "text",
    Some(MouseCursor::Grab) => "grab",
    Some(MouseCursor::Grabbing) => "grabbing",
    Some(MouseCursor::Default) | None => "default",
  }
}

/// Writes the pointer shape the current hover resolves to when it differs from
/// `shape`, the one last written, which follows.
pub(super) fn sync_pointer_shape(out: &mut impl Write, shape: &mut &'static str) {
  let current = pointer_shape(with_records(|records| records.hover_cursor()));

  if current != *shape {
    *shape = current;

    let _ = write!(out, "{}", escape::PointerShape(current));
    let _ = out.flush();
  }
}

/// The control moving the cursor to the cell (`column`, `row`).
const fn move_to(column: u16, row: u16) -> MoveTo {
  MoveTo { column, row }
}

/// What the terminal renders of a cell's attributes: the color depth painted RGB
/// quantizes to, the base colors it shows, and whether an underline keeps its style
/// and color.
#[derive(Clone, Copy)]
pub(super) struct Rendition {
  pub(super) depth: ColorDepth,
  pub(super) palette: detect::Palette,
  pub(super) styled_underlines: bool,
}

impl Rendition {
  /// The underline control for `decoration`: the style and color where the terminal
  /// keeps them, a plain single underline elsewhere.
  const fn underline(self, decoration: paint::CellDecoration) -> Underline {
    match decoration.underline {
      None => Underline::None,
      Some(_) if !self.styled_underlines => Underline::Single,
      Some(TextDecorationStyle::Solid) => Underline::Single,
      Some(TextDecorationStyle::Double) => Underline::Double,
      Some(TextDecorationStyle::Dotted) => Underline::Dotted,
      Some(TextDecorationStyle::Dashed) => Underline::Dashed,
      Some(TextDecorationStyle::Wavy) => Underline::Curly,
    }
  }

  /// Writes the attributes taking a cell's decoration from `current` to `next`.
  fn write_decoration(
    self,
    out: &mut impl Write,
    current: paint::CellDecoration,
    next: paint::CellDecoration,
  ) {
    let underline = self.underline(next);

    if underline != self.underline(current) {
      let _ = write!(out, "{underline}", underline = Sgr::Underline(underline));
    }

    if self.styled_underlines && next.underline_color != current.underline_color {
      let color = next.underline_color.map_or(Color::Reset, |rgb| {
        self.depth.terminal_color(rgb, &self.palette)
      });

      let _ = write!(out, "{}", Sgr::UnderlineColor(color));
    }

    if next.overline != current.overline {
      let _ = write!(out, "{}", Sgr::Overline(next.overline));
    }

    if next.line_through != current.line_through {
      let _ = write!(out, "{}", Sgr::StrikeThrough(next.line_through));
    }
  }
}

/// What the terminal holds of the frame's pictures: the generation each image was
/// transmitted at, and the cells each is placed over.
#[derive(Default)]
pub(super) struct Placed {
  pub(super) transmitted: HashMap<u32, u64>,
  pub(super) placements: HashMap<u32, paint::PlacedCells>,
}

/// Writes a painted frame to the terminal under `rendition`; rows unchanged since the
/// previous frame are skipped, the pictures are transmitted where their image changed
/// and placed where their cells did (every one again without a previous frame), an
/// image no picture shows any more is freed, and the whole frame sits between the
/// synchronized-output brackets (mode 2026) when `synchronized`.
pub(super) fn draw(
  out: &mut impl Write,
  buffer: &paint::Buffer,
  rendition: Rendition,
  previous: Option<&paint::Buffer>,
  synchronized: bool,
  placed: &mut Placed,
) {
  if synchronized {
    let _ = write!(out, "{}", dec_mode(escape::SYNCHRONIZED_OUTPUT, true));
  }

  // The cursor hides while the cells are written, so it never shows mid-frame.
  let _ = write!(
    out,
    "{}{}{}",
    dec_mode(escape::SHOW_CURSOR, false),
    move_to(0, 0),
    Sgr::Reset
  );

  for row in 0..buffer.height {
    if !paint::row_changed(previous, buffer, row) {
      continue;
    }

    draw_row(out, buffer, row, rendition);
  }

  draw_pictures(out, buffer, placed, previous.is_none());

  // The focused entry's caret is the terminal's own bar cursor, blinking as the
  // terminal does.
  if let Some((column, row)) = buffer.caret {
    let _ = write!(
      out,
      "{}{}{}",
      move_to(column, row),
      CursorStyle::BlinkingBar,
      dec_mode(escape::SHOW_CURSOR, true)
    );
  }

  if synchronized {
    let _ = write!(out, "{}", dec_mode(escape::SYNCHRONIZED_OUTPUT, false));
  }

  let _ = out.flush();
}

/// Writes the frame's pictures: an image transmitted where its generation moved on,
/// placed over its cells where those moved or the frame is written `whole`, and freed
/// where no picture shows it any more.
fn draw_pictures(out: &mut impl Write, buffer: &paint::Buffer, placed: &mut Placed, whole: bool) {
  let mut shown = Vec::new();

  for picture in &buffer.pictures {
    shown.push(picture.id);

    if placed.transmitted.get(&picture.id) != Some(&picture.image.generation) {
      let _ = escape::write_transmit(
        out,
        picture.id,
        picture.image.width,
        picture.image.height,
        &picture.image.pixels,
      );

      placed
        .transmitted
        .insert(picture.id, picture.image.generation);
      placed.placements.remove(&picture.id);
    }

    let cells = picture.cells();

    if !whole && placed.placements.get(&picture.id) == Some(&cells) {
      continue;
    }

    if placed.placements.contains_key(&picture.id) {
      let _ = write!(out, "{}", escape::Unplace(picture.id));
    }

    let _ = write!(
      out,
      "{}{}",
      move_to(picture.column, picture.row),
      escape::Place {
        id: picture.id,
        columns: picture.columns,
        rows: picture.rows,
        crop: picture.crop,
        layer: picture.layer,
      }
    );
    placed.placements.insert(picture.id, cells);
  }

  let gone: Vec<u32> = placed
    .transmitted
    .keys()
    .filter(|id| !shown.contains(id))
    .copied()
    .collect();

  for id in gone {
    let _ = write!(out, "{}", escape::Free(id));

    placed.transmitted.remove(&id);
    placed.placements.remove(&id);
  }
}

/// Writes the row `row` of a painted frame under `rendition`: the cursor at its start,
/// the attributes reset, each glyph after the attributes that change for it, a sized
/// glyph as a sized run, the cells a glyph's advance covers left to the terminal, and
/// a block's lower rows stepped over before the next glyph.
fn draw_row(out: &mut impl Write, buffer: &paint::Buffer, row: usize, rendition: Rendition) {
  let depth = rendition.depth;

  let _ = write!(
    out,
    "{}{}",
    move_to(0, u16::try_from(row).unwrap_or(u16::MAX)),
    Sgr::Reset
  );

  let mut current = paint::TerminalCell::EMPTY;
  // The column the terminal's cursor is at: a glyph advances it by its block's
  // columns, and a block's lower rows are stepped over before the next glyph.
  let mut cursor = 0;

  for (column, cell) in buffer.cells[row * buffer.width..(row + 1) * buffer.width]
    .iter()
    .enumerate()
  {
    if cell.glyph == Glyph::Continuation {
      continue;
    }

    if cursor != column {
      let _ = write!(
        out,
        "{}",
        move_to(
          u16::try_from(column).unwrap_or(u16::MAX),
          u16::try_from(row).unwrap_or(u16::MAX)
        )
      );
    }

    cursor = column + cell.glyph.cells() * cell.scale.rows();

    if cell.foreground != current.foreground {
      let color = cell.foreground.map_or(Color::Reset, |rgb| {
        depth.terminal_color(rgb, &rendition.palette)
      });

      let _ = write!(out, "{}", Sgr::Foreground(color));
    }

    if cell.background != current.background {
      let color = cell.background.map_or(Color::Reset, |rgb| {
        depth.terminal_color(rgb, &rendition.palette)
      });

      let _ = write!(out, "{}", Sgr::Background(color));
    }

    if cell.bold != current.bold {
      let _ = write!(out, "{}", Sgr::Bold(cell.bold));
    }

    if cell.decoration != current.decoration {
      rendition.write_decoration(out, current.decoration, cell.decoration);
    }

    if cell.reverse != current.reverse {
      let _ = write!(out, "{}", Sgr::Reverse(cell.reverse));
    }

    // OSC 8 anchors: close the running link on any change, open the next.
    if cell.link != current.link {
      if current.link.is_some() {
        let _ = write!(out, "{}", escape::Hyperlink(""));
      }

      if let Some(url) = cell
        .link
        .and_then(|index| buffer.links.get(usize::from(index)))
      {
        let _ = write!(out, "{}", escape::Hyperlink(url));
      }
    }

    let _ = match cell.scale {
      CellScale::ONE => write!(out, "{}", cell.glyph),
      CellScale::Whole(scale) => write!(out, "\x1b]66;s={scale};{}\x1b\\", cell.glyph),
      CellScale::Fraction(numerator, denominator) => write!(
        out,
        "\x1b]66;n={numerator}:d={denominator}:v=1;{}\x1b\\",
        cell.glyph
      ),
    };

    current = *cell;
  }

  if current.link.is_some() {
    let _ = write!(out, "{}", escape::Hyperlink(""));
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Sixteen colors and plain underlines.
  const PLAIN: Rendition = Rendition {
    depth: ColorDepth::Ansi16,
    palette: detect::Palette::DEFAULT,
    styled_underlines: false,
  };

  /// A styled, colored underline is written as its style and color where the terminal
  /// kept the probe, and as a plain underline elsewhere; an overline and a line through
  /// are written as their own attributes either way.
  #[test]
  fn decorations_are_written_as_the_terminal_keeps_them() {
    let mut buffer = paint::Buffer::new(3, 1);

    buffer.cells[0].decoration = paint::CellDecoration {
      underline: Some(TextDecorationStyle::Wavy),
      underline_color: Some((255, 0, 0)),
      ..paint::CellDecoration::NONE
    };
    buffer.cells[1].decoration = paint::CellDecoration {
      overline: true,
      ..paint::CellDecoration::NONE
    };
    buffer.cells[2].decoration = paint::CellDecoration {
      line_through: true,
      ..paint::CellDecoration::NONE
    };

    let mut styled = Vec::new();
    let mut plain = Vec::new();

    draw(
      &mut styled,
      &buffer,
      Rendition {
        depth: ColorDepth::TrueColor,
        palette: detect::Palette::DEFAULT,
        styled_underlines: true,
      },
      None,
      false,
      &mut Placed::default(),
    );
    draw(
      &mut plain,
      &buffer,
      PLAIN,
      None,
      false,
      &mut Placed::default(),
    );

    let styled = String::from_utf8(styled).unwrap();
    let plain = String::from_utf8(plain).unwrap();

    assert!(styled.contains("\x1b[4:3m"), "{styled:?}");
    assert!(styled.contains("\x1b[58:2::255:0:0m"), "{styled:?}");
    assert!(
      styled.contains("\x1b[53m") && styled.contains("\x1b[55m"),
      "{styled:?}"
    );
    assert!(styled.contains("\x1b[9m"), "{styled:?}");
    assert!(
      plain.contains("\x1b[4m") && !plain.contains("4:3"),
      "{plain:?}"
    );
    assert!(!plain.contains("58:"), "{plain:?}");
    assert!(
      plain.contains("\x1b[53m") && plain.contains("\x1b[9m"),
      "{plain:?}"
    );
  }

  /// In-band resize notifications are asked for, queried and handed back with the
  /// other modes.
  #[test]
  fn in_band_resize_is_set_queried_and_reset() {
    let mut setup = Vec::new();
    let mut queries = Vec::new();
    let mut restore = Vec::new();

    write_setup(&mut setup, false).unwrap();
    write_queries(&mut queries).unwrap();
    write_restore(&mut restore).unwrap();

    assert!(String::from_utf8(setup).unwrap().contains("\x1b[?2048h"));
    let queries = String::from_utf8(queries).unwrap();

    assert!(queries.contains("\x1b[?2048$p"));
    assert!(queries.contains("\x1b[?5522$p"));
    assert!(String::from_utf8(restore).unwrap().contains("\x1b[?2048l"));
  }

  /// The mouse modes are set together and reset together.
  #[test]
  fn the_mouse_modes_are_written_as_one() {
    let mut on = Vec::new();
    let mut off = Vec::new();

    write_mouse(&mut on, true).unwrap();
    write_mouse(&mut off, false).unwrap();

    assert_eq!(
      String::from_utf8(on).unwrap(),
      "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h"
    );
    assert_eq!(
      String::from_utf8(off).unwrap(),
      "\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l"
    );
  }

  /// Every pointer shape the GUI's `cursor` takes has an OSC 22 name, and the
  /// restoring controls hand the shape back to the terminal.
  #[test]
  fn pointer_shapes_are_named_and_restored() {
    assert_eq!(pointer_shape(Some(MouseCursor::Click)), "pointer");
    assert_eq!(pointer_shape(Some(MouseCursor::Text)), "text");
    assert_eq!(pointer_shape(Some(MouseCursor::Grab)), "grab");
    assert_eq!(pointer_shape(Some(MouseCursor::Grabbing)), "grabbing");
    assert_eq!(pointer_shape(Some(MouseCursor::Default)), "default");
    assert_eq!(pointer_shape(None), "default");

    let mut restore = Vec::new();

    write_restore(&mut restore).unwrap();

    assert!(
      String::from_utf8(restore)
        .unwrap()
        .contains("\x1b]22;default\x1b\\")
    );
  }

  /// A scaled cell is written as a sized run, the cells its block covers on its row
  /// are left to the terminal's advance, and a block's lower rows are stepped over
  /// before the next glyph; a fraction names its parts and sits at the cell's foot.
  #[test]
  fn sized_cells_are_written_as_sized_runs_and_their_blocks_stepped_over() {
    let mut buffer = paint::Buffer::new(4, 2);

    buffer.put_sized(0, 0, "H", 1, CellScale::Whole(2), |_| {});
    buffer.put_sized(2, 0, "i", 1, CellScale::Fraction(1, 2), |_| {});
    buffer.put_cluster(2, 1, "x", 1, |_| {});

    let mut out = Vec::new();

    draw(
      &mut out,
      &buffer,
      PLAIN,
      None,
      false,
      &mut Placed::default(),
    );

    let out = String::from_utf8(out).unwrap();

    assert!(
      out.contains("\x1b]66;s=2;H\x1b\\\x1b]66;n=1:d=2:v=1;i\x1b\\ "),
      "{out:?}"
    );
    assert!(
      out.contains("\x1b[2;1H\x1b[m\x1b[2;3Hx "),
      "the second row steps past the block: {out:?}"
    );
  }

  /// At sixteen colors a painted RGB is written as the nearest base color the
  /// terminal reported, not the nearest default.
  #[test]
  fn sixteen_colors_pick_from_the_reported_palette() {
    let mut buffer = paint::Buffer::new(1, 1);

    buffer.cells[0].foreground = Some((40, 44, 52));

    let mut palette = detect::Palette::DEFAULT;

    palette.set(4, (40, 44, 52));

    let mut reported = Vec::new();
    let mut plain = Vec::new();

    draw(
      &mut reported,
      &buffer,
      Rendition { palette, ..PLAIN },
      None,
      false,
      &mut Placed::default(),
    );
    draw(
      &mut plain,
      &buffer,
      PLAIN,
      None,
      false,
      &mut Placed::default(),
    );

    assert!(String::from_utf8(reported).unwrap().contains("\x1b[34m"));
    assert!(String::from_utf8(plain).unwrap().contains("\x1b[30m"));
  }

  /// A picture is transmitted once per image generation and placed over its cells,
  /// placed again without transmission where its cells move, and freed once it is
  /// gone from the frame.
  #[test]
  fn pictures_are_transmitted_once_placed_on_change_and_freed_when_gone() {
    let image = std::rc::Rc::new(paint::Image {
      generation: 1,
      width: 2,
      height: 1,
      pixels: vec![255, 0, 0, 255, 0, 0, 255, 255],
    });
    let placement = |column| paint::Placement {
      id: 3,
      column,
      row: 1,
      columns: 2,
      rows: 1,
      crop: (0, 0, 2, 1),
      layer: 0,
      image: std::rc::Rc::clone(&image),
    };
    let mut buffer = paint::Buffer::new(6, 2);

    buffer.pictures.push(placement(0));

    let mut placed = Placed::default();
    let mut first = Vec::new();

    draw(&mut first, &buffer, PLAIN, None, false, &mut placed);

    let first = String::from_utf8(first).unwrap();

    assert!(
      first.contains("\x1b_Ga=t,f=32,s=2,v=1,i=3,q=2,m=0;/wAA/wAA//8=\x1b\\"),
      "{first:?}"
    );
    assert!(
      first.contains("\x1b[2;1H\x1b_Ga=p,i=3,p=1,c=2,r=1,x=0,y=0,w=2,h=1,z=0,C=1,q=2\x1b\\"),
      "{first:?}"
    );

    let mut same = Vec::new();

    draw(&mut same, &buffer, PLAIN, Some(&buffer), false, &mut placed);

    assert!(!String::from_utf8(same).unwrap().contains("\x1b_G"));

    let mut moved_frame = paint::Buffer::new(6, 2);

    moved_frame.pictures.push(placement(2));

    let mut moved = Vec::new();

    draw(
      &mut moved,
      &moved_frame,
      PLAIN,
      Some(&buffer),
      false,
      &mut placed,
    );

    let moved = String::from_utf8(moved).unwrap();

    assert!(!moved.contains("a=t,"), "{moved:?}");
    assert!(
      moved.contains("\x1b_Ga=d,d=i,i=3,q=2\x1b\\\x1b[2;3H\x1b_Ga=p,i=3"),
      "{moved:?}"
    );

    let mut gone = Vec::new();

    draw(
      &mut gone,
      &paint::Buffer::new(6, 2),
      PLAIN,
      Some(&moved_frame),
      false,
      &mut placed,
    );

    assert!(
      String::from_utf8(gone)
        .unwrap()
        .contains("\x1b_Ga=d,d=I,i=3,q=2\x1b\\")
    );
    assert!(placed.transmitted.is_empty());
  }

  /// Launch probes text sizing from the home cell with a space at scale two and asks
  /// where the cursor landed, and fences its queries with the primary device
  /// attributes request.
  #[test]
  fn text_sizing_is_probed_from_the_home_cell() {
    let mut out = Vec::new();

    write_queries(&mut out).unwrap();

    let out = String::from_utf8(out).unwrap();

    assert!(
      out.ends_with(
        "\x1b[1;1H\x1b]66;s=2; \x1b\\\x1b[6n\x1b_Gi=2147483647,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\
         \x1b[c"
      ),
      "{out:?}"
    );
  }

  /// A theme change asks for the terminal's own colors afresh, fenced by the primary
  /// device attributes request.
  #[test]
  fn the_colors_are_asked_for_in_a_window_of_their_own() {
    let mut out = Vec::new();

    write_color_queries(&mut out).unwrap();

    let out = String::from_utf8(out).unwrap();

    assert!(out.starts_with("\x1b]11;?"), "{out:?}");
    assert!(out.contains("\x1b]10;?"), "{out:?}");
    assert!(out.ends_with("\x1b[c"), "{out:?}");
  }

  /// A frame is bracketed by mode 2026 when the terminal confirmed it can toggle the
  /// mode, and written plain otherwise; either way the cursor hides while it is
  /// written.
  #[test]
  fn a_frame_is_bracketed_by_synchronized_output_when_confirmed() {
    let buffer = paint::Buffer::new(2, 1);
    let mut synchronized = Vec::new();
    let mut plain = Vec::new();

    draw(
      &mut synchronized,
      &buffer,
      PLAIN,
      None,
      true,
      &mut Placed::default(),
    );
    draw(
      &mut plain,
      &buffer,
      PLAIN,
      None,
      false,
      &mut Placed::default(),
    );

    let synchronized = String::from_utf8(synchronized).unwrap();
    let plain = String::from_utf8(plain).unwrap();

    assert!(synchronized.starts_with("\x1b[?2026h"), "{synchronized:?}");
    assert!(synchronized.ends_with("\x1b[?2026l"), "{synchronized:?}");
    assert_eq!(
      &synchronized["\x1b[?2026h".len()..synchronized.len() - "\x1b[?2026l".len()],
      plain
    );
    assert!(!plain.contains("2026"), "{plain:?}");
    assert!(plain.starts_with("\x1b[?25l"), "{plain:?}");
  }
}
