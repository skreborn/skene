//! The rasterizer alone: TextKit's own layout (its lines, fonts, glyphs and positions, read
//! through a `CTLine` of each line fragment) drawn twice more, by Core Text's
//! `CTFontDrawGlyphs` and by vello. Against TextKit's own drawing, the first shows whether C's
//! rasterizer is TextKit's, the second what B's rasterizer alone changes, layout held equal.
#![allow(unused_unsafe)]

use std::{collections::HashMap, ptr::NonNull, sync::Arc};

use objc2_core_foundation::{CFRetained, CGPoint};
use objc2_core_graphics::{CGContext, CGGlyph};
use objc2_core_text::CTFont;
use parley::{
  FontData,
  fontique::{Blob, Synthesis},
};
use skrifa::{FontRef, MetadataProvider as _, Tag};
use text_parity::{
  image::Rgba,
  layout::{GlyphRun, PositionedGlyph},
  record::MARGIN,
  render::{self, RenderOptions},
};

use crate::{
  cg::{Canvas, Ground, Smoothing},
  coretext,
  textkit::TkLaid,
};

/// One run of TextKit's layout: its Core Text font and its glyphs at layout positions.
pub struct PlacedRun {
  /// The font as TextKit used it.
  pub font: CFRetained<CTFont>,
  /// Glyph id, origin x, baseline y.
  pub glyphs: Vec<(u16, f64, f64)>,
}

/// TextKit's glyphs, line by line, at the positions its lines give them.
#[must_use]
pub fn placed(laid: &TkLaid, case: &text_parity::corpus::Case, truncated: bool) -> Vec<PlacedRun> {
  let mut out = Vec::new();

  for (index, line) in laid.lines.iter().enumerate() {
    let ct = crate::textkit::drawn_line(laid, index, case, truncated);
    let x = crate::textkit::origin_x(line);
    let baseline = line.frame.y + line.bounds.origin.y + line.glyph_origin.y;

    for run in coretext::runs(&ct) {
      out.push(PlacedRun {
        font: run.font.clone(),
        glyphs: run
          .glyphs
          .iter()
          .map(|glyph| (glyph.id, x + glyph.x, baseline))
          .collect(),
      });
    }
  }

  out
}

/// `runs` drawn by `CTFontDrawGlyphs` in their own fonts.
#[must_use]
pub fn draw_cg(
  runs: &[PlacedRun],
  width: f32,
  height: f32,
  scale: f32,
  ground: Ground,
  smoothing: Smoothing,
) -> Rgba {
  let canvas = Canvas::for_box(width, height, scale, ground, smoothing);
  let context: &CGContext = &canvas.context;
  let top = canvas.height_points();
  let shade = if ground.white_text() { 1. } else { 0. };

  canvas.points_up();
  canvas.identity_text();
  CGContext::set_rgb_fill_color(Some(context), shade, shade, shade, 1.);

  for run in runs {
    if run.glyphs.is_empty() {
      continue;
    }

    let mut ids: Vec<CGGlyph> = run.glyphs.iter().map(|glyph| glyph.0).collect();
    let mut positions: Vec<CGPoint> = run
      .glyphs
      .iter()
      .map(|glyph| {
        CGPoint::new(
          f64::from(MARGIN) + glyph.1,
          top - (f64::from(MARGIN) + glyph.2),
        )
      })
      .collect();

    unsafe {
      run.font.draw_glyphs(
        NonNull::new_unchecked(ids.as_mut_ptr()),
        NonNull::new_unchecked(positions.as_mut_ptr()),
        ids.len(),
        context,
      );
    }
  }

  canvas.to_rgba()
}

/// Core Text fonts as Parley's font data: the file where it can be read (the face found by
/// PostScript name), else an sfnt from the font's tables.
#[derive(Default)]
pub struct Bytes {
  fonts: HashMap<String, Option<FontData>>,
}

impl Bytes {
  /// The font data of `font`.
  pub fn of(&mut self, font: &CTFont) -> Option<FontData> {
    let info = coretext::info(font);
    let key = format!(
      "{}#{}",
      info.path.clone().unwrap_or_default(),
      info.postscript
    );

    if let Some(found) = self.fonts.get(&key) {
      return found.clone();
    }

    let from_file = info.path.as_ref().and_then(|path| {
      let data = crate::sets::read_file(path)?;
      let count = match skrifa::raw::FileRef::new(&data) {
        Ok(skrifa::raw::FileRef::Collection(collection)) => collection.len(),
        _ => 1,
      };
      let index = (0..count)
        .find(|index| {
          text_parity::fonts::names_of(&data, *index)
            .is_some_and(|(_, postscript)| postscript == info.postscript)
        })
        .or((count == 1).then_some(0))?;

      Some(FontData::new(Blob::new(data), index))
    });
    let found = from_file
      .or_else(|| coretext::sfnt(font).map(|bytes| FontData::new(Blob::new(Arc::new(bytes)), 0)));

    self.fonts.insert(key, found.clone());

    found
  }
}

/// `runs` drawn by vello, each font at Core Text's instance.
#[must_use]
pub fn draw_vello(
  runs: &[PlacedRun],
  bytes: &mut Bytes,
  width: f32,
  height: f32,
  scale: f32,
) -> Rgba {
  let mut glyph_runs = Vec::new();

  for run in runs {
    let Some(data) = bytes.of(&run.font)
    else {
      continue;
    };
    let variation = coretext::variation(&run.font);
    let coords: Vec<i16> = FontRef::from_index(data.data.data(), data.index)
      .map(|font| {
        let settings: Vec<(Tag, f32)> = variation
          .iter()
          .filter_map(|(tag, value)| {
            Some((Tag::new(tag.as_bytes().try_into().ok()?), *value as f32))
          })
          .collect();

        font
          .axes()
          .location(settings)
          .coords()
          .iter()
          .map(|coord| coord.to_bits())
          .collect()
      })
      .unwrap_or_default();

    glyph_runs.push(GlyphRun {
      line: 0,
      font: data,
      size: unsafe { run.font.size() } as f32,
      coords,
      synthesis: Synthesis::default(),
      baseline: run.glyphs.first().map_or(0., |glyph| glyph.2 as f32),
      glyphs: run
        .glyphs
        .iter()
        .map(|glyph| PositionedGlyph {
          id: u32::from(glyph.0),
          x: glyph.1 as f32,
          y: glyph.2 as f32,
          cluster: 0,
        })
        .collect(),
    });
  }

  let pixmap = render::render_runs(&glyph_runs, width, height, scale, RenderOptions::default());
  let (wide, high) = (u32::from(pixmap.width()), u32::from(pixmap.height()));
  let data = pixmap.data_as_u8_slice().to_vec();
  let mut out = Rgba {
    width: wide,
    height: high,
    data,
  };

  // vello's pixmap is premultiplied; over its white ground every pixel is opaque already.
  for pixel in out.data.chunks_exact_mut(4) {
    pixel[3] = 255;
  }

  out
}
