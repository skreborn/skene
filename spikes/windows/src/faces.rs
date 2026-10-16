//! Option C's faces: Parley's glyph runs handed to DirectWrite, each font opened from the file
//! fontique loaded it from (`IDWriteFactory6::CreateFontResource`), its variation axes and
//! synthesis as DirectWrite's axis values and simulations.

use std::collections::HashMap;

use text_parity::record::GlyphRunRecord;
use windows::Win32::Graphics::DirectWrite::*;
use windows_core::{Interface, PCWSTR, Result};

use crate::engine::{Collected, GlyphRunRec, wide};

type FaceKey = (String, u32, i32, Vec<(String, u32)>);

/// DirectWrite faces made from font files, cached.
pub struct Faces {
  factory: IDWriteFactory6,
  cache: HashMap<FaceKey, Option<IDWriteFontFace>>,
  /// Fonts that could not be opened: why, once each.
  pub failures: Vec<String>,
  /// Glyphs left undrawn for want of a face.
  pub skipped_glyphs: usize,
}

/// A variation axis tag as DirectWrite packs it.
pub fn axis_tag(tag: &str) -> DWRITE_FONT_AXIS_TAG {
  let mut bytes = [b' '; 4];

  for (at, b) in tag.bytes().take(4).enumerate() {
    bytes[at] = b;
  }

  DWRITE_FONT_AXIS_TAG(u32::from_le_bytes(bytes))
}

impl Faces {
  pub fn new(factory: &IDWriteFactory4) -> Result<Self> {
    Ok(Self {
      factory: factory.cast()?,
      cache: HashMap::new(),
      failures: Vec::new(),
      skipped_glyphs: 0,
    })
  }

  /// The face of `path`'s face `index`, with `simulations` and the axis values `axes`.
  pub fn face(
    &mut self,
    path: &str,
    index: u32,
    simulations: DWRITE_FONT_SIMULATIONS,
    axes: &[(String, f32)],
  ) -> Option<IDWriteFontFace> {
    let key = (
      path.to_owned(),
      index,
      simulations.0,
      axes.iter().map(|(t, v)| (t.clone(), v.to_bits())).collect(),
    );

    if let Some(face) = self.cache.get(&key) {
      return face.clone();
    }

    let made = (|| -> Result<IDWriteFontFace> {
      let name = wide(path);
      let file = unsafe {
        IDWriteFactory::CreateFontFileReference(&self.factory, PCWSTR(name.as_ptr()), None)?
      };
      let resource = unsafe { self.factory.CreateFontResource(&file, index)? };
      let values: Vec<DWRITE_FONT_AXIS_VALUE> = axes
        .iter()
        .map(|(tag, value)| DWRITE_FONT_AXIS_VALUE {
          axisTag: axis_tag(tag),
          value: *value,
        })
        .collect();
      unsafe { resource.CreateFontFace(simulations, &values)? }.cast()
    })();
    let face = match made {
      Ok(face) => Some(face),
      Err(e) => {
        self.failures.push(format!("{path}#{index}: {e}"));
        None
      }
    };

    self.cache.insert(key, face.clone());
    face
  }

  /// Parley's glyph runs as DirectWrite glyph runs, in DIPs: each glyph placed by an offset from
  /// its run's baseline, so DirectWrite draws it where Parley put it.
  pub fn runs(&mut self, records: &[GlyphRunRecord]) -> Collected {
    let mut out = Collected::default();

    for run in records {
      let Some(path) = &run.font.path
      else {
        self.skipped_glyphs += run.glyphs.len();
        self
          .failures
          .push(format!("{}: no file", run.font.postscript));
        continue;
      };
      let mut simulations = DWRITE_FONT_SIMULATIONS_NONE.0;

      if run.embolden {
        simulations |= DWRITE_FONT_SIMULATIONS_BOLD.0;
      }

      if run.skew.is_some() {
        simulations |= DWRITE_FONT_SIMULATIONS_OBLIQUE.0;
      }

      let axes: Vec<(String, f32)> = run
        .variations
        .iter()
        .map(|v| (v.tag.clone(), v.value))
        .collect();
      let Some(face) = self.face(
        path,
        run.font.index,
        DWRITE_FONT_SIMULATIONS(simulations),
        &axes,
      )
      else {
        self.skipped_glyphs += run.glyphs.len();
        continue;
      };

      if run.glyphs.is_empty() {
        continue;
      }

      // The baseline most glyphs sit on; a mark's vertical offset is its own.
      let mut ys: Vec<(u32, usize)> = Vec::new();

      for g in &run.glyphs {
        let bits = (g.y * 1000.0).round().to_bits();

        match ys.iter_mut().find(|(y, _)| *y == bits) {
          Some((_, n)) => *n += 1,
          None => ys.push((bits, 1)),
        }
      }

      let baseline = ys
        .iter()
        .max_by_key(|(_, n)| *n)
        .map_or(0.0, |(y, _)| f32::from_bits(*y) / 1000.0);

      out.runs.push(GlyphRunRec {
        face,
        em: run.size,
        glyphs: run.glyphs.iter().map(|g| g.id as u16).collect(),
        advances: vec![0.0; run.glyphs.len()],
        offsets: run
          .glyphs
          .iter()
          .map(|g| DWRITE_GLYPH_OFFSET {
            advanceOffset: g.x,
            ascenderOffset: baseline - g.y,
          })
          .collect(),
        origin: (0.0, baseline),
        bidi: 0,
        sideways: false,
        inline: false,
        text_position: 0,
        cluster_map: Vec::new(),
      });
    }

    out
  }
}
