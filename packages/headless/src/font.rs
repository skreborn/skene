//! The pinned font headless measures text in: Inter's regular face (4.0), subset to
//! Latin-1 and the combining acute, the face the conformance suite's pages draw in.

use std::sync::OnceLock;

use rustc_hash::FxHashMap;
use skrifa::{
  GlyphId,
  MetadataProvider,
  prelude::{FontRef, LocationRef, Size},
};

/// The pinned face's advances, read once from its tables.
pub(crate) struct Pinned {
  units_per_em: f32,
  advances: FxHashMap<u32, f32>,
  missing: f32,
}

impl Pinned {
  /// The face, as OpenType.
  pub(crate) const DATA: &'static [u8] = include_bytes!("../fonts/inter-regular.otf");

  /// The face's advances.
  pub(crate) fn get() -> &'static Self {
    static PINNED: OnceLock<Pinned> = OnceLock::new();

    PINNED.get_or_init(Self::read)
  }

  /// Reads the advances from the face's tables.
  fn read() -> Self {
    let font = FontRef::new(Self::DATA).expect("the pinned face parses");
    let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
    let advances = font
      .charmap()
      .mappings()
      .map(|(character, glyph)| (character, metrics.advance_width(glyph).unwrap_or(0.)))
      .collect();

    Self {
      units_per_em: f32::from(
        font
          .metrics(Size::unscaled(), LocationRef::default())
          .units_per_em,
      ),
      advances,
      missing: metrics.advance_width(GlyphId::NOTDEF).unwrap_or(0.),
    }
  }

  /// How far `text` advances a line at `size`: each character as far as its glyph
  /// does, and a character the face lacks as far as its missing glyph does.
  pub(crate) fn advance(&self, text: &str, size: f32) -> f32 {
    let units: f32 = text
      .chars()
      .map(|character| {
        self
          .advances
          .get(&u32::from(character))
          .copied()
          .unwrap_or(self.missing)
      })
      .sum();

    units * size / self.units_per_em
  }
}
