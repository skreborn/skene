//! Generates the typeface the terminal measures its text in: every code point a glyph as
//! many cells wide as the terminal draws it on its own, zero, one or two, a cell an em
//! and its ascent the whole cell.

use std::{env, fs, path::PathBuf};

use unicode_width::UnicodeWidthChar;

/// The family the typeface is known by.
const FAMILY: &str = "Skene Cells";

/// The units of one em, one cell.
const UNITS: u16 = 1000;

/// The dotted circle, left unmapped so a shaper inserts none before a lone mark.
const DOTTED_CIRCLE: u32 = 0x25cc;

fn main() {
  let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("cells.otf");

  fs::write(out, typeface()).expect("the measuring typeface writes");
  println!("cargo::rerun-if-changed=build.rs");
}

/// The glyph `code` maps to: the zero, one or two cells wide one, a control one cell.
fn glyph(code: u32) -> u32 {
  match char::from_u32(code).map(UnicodeWidthChar::width) {
    Some(Some(0)) => 1,
    Some(Some(2)) => 3,
    _ => 2,
  }
}

/// The typeface, as OpenType: its tables in the order of their tags.
fn typeface() -> Vec<u8> {
  let tables: [(&[u8; 4], Vec<u8>); 8] = [
    (b"OS/2", os2()),
    (b"cmap", cmap()),
    (b"head", head()),
    (b"hhea", hhea()),
    (b"hmtx", hmtx()),
    (b"maxp", maxp()),
    (b"name", name()),
    (b"post", post()),
  ];
  let mut font = Vec::new();
  let count = u16::try_from(tables.len()).expect("a handful of tables");

  font.extend(0x0001_0000_u32.to_be_bytes());
  font.extend(count.to_be_bytes());
  font.extend(128_u16.to_be_bytes());
  font.extend(3_u16.to_be_bytes());
  font.extend(0_u16.to_be_bytes());

  let mut offset = 12 + 16 * tables.len();

  for (tag, data) in &tables {
    font.extend(*tag);
    font.extend(0_u32.to_be_bytes());
    font.extend(u32::try_from(offset).expect("a small font").to_be_bytes());
    font.extend(
      u32::try_from(data.len())
        .expect("a small font")
        .to_be_bytes(),
    );
    offset += data.len().next_multiple_of(4);
  }

  for (_, data) in tables {
    let padded = data.len().next_multiple_of(4);

    font.extend(&data);
    font.resize(font.len() + padded - data.len(), 0);
  }

  font
}

/// The character map: every code point but a surrogate and the dotted circle, in runs of
/// one glyph (format 13).
fn cmap() -> Vec<u8> {
  let mut groups: Vec<(u32, u32, u32)> = Vec::new();

  for code in
    (0..=0x0010_ffff_u32).filter(|code| !(0xd800..=0xdfff).contains(code) && *code != DOTTED_CIRCLE)
  {
    let glyph = glyph(code);

    match groups.last_mut() {
      Some((_, end, last)) if *last == glyph && *end + 1 == code => *end = code,
      _ => groups.push((code, code, glyph)),
    }
  }

  let count = u32::try_from(groups.len()).expect("a few thousand groups");
  let mut table = Vec::new();

  table.extend(0_u16.to_be_bytes());
  table.extend(1_u16.to_be_bytes());
  table.extend(3_u16.to_be_bytes());
  table.extend(10_u16.to_be_bytes());
  table.extend(12_u32.to_be_bytes());
  table.extend(13_u16.to_be_bytes());
  table.extend(0_u16.to_be_bytes());
  table.extend((16 + 12 * count).to_be_bytes());
  table.extend(0_u32.to_be_bytes());
  table.extend(count.to_be_bytes());

  for (start, end, glyph) in groups {
    table.extend(start.to_be_bytes());
    table.extend(end.to_be_bytes());
    table.extend(glyph.to_be_bytes());
  }

  table
}

/// The font header.
fn head() -> Vec<u8> {
  let mut table = Vec::new();

  table.extend(0x0001_0000_u32.to_be_bytes());
  table.extend(0x0001_0000_u32.to_be_bytes());
  table.extend(0_u32.to_be_bytes());
  table.extend(0x5f0f_3cf5_u32.to_be_bytes());
  table.extend(0x000b_u16.to_be_bytes());
  table.extend(UNITS.to_be_bytes());
  table.extend(0_i64.to_be_bytes());
  table.extend(0_i64.to_be_bytes());
  table.extend(0_i16.to_be_bytes());
  table.extend(0_i16.to_be_bytes());
  table.extend((2 * UNITS).to_be_bytes());
  table.extend(UNITS.to_be_bytes());
  table.extend(0_u16.to_be_bytes());
  table.extend(8_u16.to_be_bytes());
  table.extend(2_i16.to_be_bytes());
  table.extend(0_i16.to_be_bytes());
  table.extend(0_i16.to_be_bytes());

  table
}

/// The horizontal header: an ascent of a cell, no descent, and four glyphs' metrics.
fn hhea() -> Vec<u8> {
  let mut table = Vec::new();

  table.extend(0x0001_0000_u32.to_be_bytes());
  table.extend(UNITS.to_be_bytes());
  table.extend(0_i16.to_be_bytes());
  table.extend(0_i16.to_be_bytes());
  table.extend((2 * UNITS).to_be_bytes());
  table.extend(0_i16.to_be_bytes());
  table.extend(0_i16.to_be_bytes());
  table.extend((2 * UNITS).to_be_bytes());
  table.extend(1_i16.to_be_bytes());
  table.extend(0_i16.to_be_bytes());
  table.extend(0_i16.to_be_bytes());

  for _ in 0..4 {
    table.extend(0_i16.to_be_bytes());
  }

  table.extend(0_i16.to_be_bytes());
  table.extend(4_u16.to_be_bytes());

  table
}

/// The glyphs' advances: the missing glyph and the narrow one a cell, the zero-width one
/// nothing, the wide one two cells.
fn hmtx() -> Vec<u8> {
  [UNITS, 0, UNITS, 2 * UNITS]
    .into_iter()
    .flat_map(|advance| [advance.to_be_bytes(), 0_i16.to_be_bytes()])
    .flatten()
    .collect()
}

/// The glyph count, with no outlines.
fn maxp() -> Vec<u8> {
  let mut table = Vec::new();

  table.extend(0x0000_5000_u32.to_be_bytes());
  table.extend(4_u16.to_be_bytes());

  table
}

/// The family's name.
fn name() -> Vec<u8> {
  let family: Vec<u8> = FAMILY.encode_utf16().flat_map(u16::to_be_bytes).collect();
  let mut table = Vec::new();

  table.extend(0_u16.to_be_bytes());
  table.extend(1_u16.to_be_bytes());
  table.extend(18_u16.to_be_bytes());
  table.extend(3_u16.to_be_bytes());
  table.extend(1_u16.to_be_bytes());
  table.extend(0x0409_u16.to_be_bytes());
  table.extend(1_u16.to_be_bytes());
  table.extend(
    u16::try_from(family.len())
      .expect("a short name")
      .to_be_bytes(),
  );
  table.extend(0_u16.to_be_bytes());
  table.extend(family);

  table
}

/// The typeface's metrics for OS/2 and Windows (version 4): an ascent of a cell, no
/// descent, the typographic metrics in force.
fn os2() -> Vec<u8> {
  let mut table = Vec::new();

  table.extend(4_u16.to_be_bytes());
  table.extend(UNITS.to_be_bytes());
  table.extend(400_u16.to_be_bytes());
  table.extend(5_u16.to_be_bytes());
  table.extend(0_u16.to_be_bytes());

  for _ in 0..8 {
    table.extend(0_i16.to_be_bytes());
  }

  table.extend(50_i16.to_be_bytes());
  table.extend(300_i16.to_be_bytes());
  table.extend(0_i16.to_be_bytes());
  table.extend([0; 10]);

  for _ in 0..4 {
    table.extend(0_u32.to_be_bytes());
  }

  table.extend(*b"    ");
  table.extend(0x00c0_u16.to_be_bytes());
  table.extend(0_u16.to_be_bytes());
  table.extend(0xffff_u16.to_be_bytes());
  table.extend(UNITS.to_be_bytes());
  table.extend(0_i16.to_be_bytes());
  table.extend(0_i16.to_be_bytes());
  table.extend(UNITS.to_be_bytes());
  table.extend(0_u16.to_be_bytes());
  table.extend(0_u32.to_be_bytes());
  table.extend(0_u32.to_be_bytes());
  table.extend(500_i16.to_be_bytes());
  table.extend(700_i16.to_be_bytes());
  table.extend(0_u16.to_be_bytes());
  table.extend(32_u16.to_be_bytes());
  table.extend(0_u16.to_be_bytes());

  table
}

/// The PostScript table (version 3, no glyph names).
fn post() -> Vec<u8> {
  let mut table = Vec::new();

  table.extend(0x0003_0000_u32.to_be_bytes());
  table.extend(0_u32.to_be_bytes());
  table.extend((-100_i16).to_be_bytes());
  table.extend(50_i16.to_be_bytes());
  table.extend(1_u32.to_be_bytes());

  for _ in 0..4 {
    table.extend(0_u32.to_be_bytes());
  }

  table
}
