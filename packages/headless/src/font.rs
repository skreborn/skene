//! The pinned font headless measures and draws text in: Inter's regular face (4.0),
//! subset to Latin-1 and the combining acute, the face the conformance suite's pages
//! draw in.

use std::sync::OnceLock;

use rustc_hash::FxHashMap;
use skrifa::{
  GlyphId,
  MetadataProvider,
  prelude::{FontRef, LocationRef, Size},
};

/// The pinned face's metrics, read once from its tables, in font units.
pub(crate) struct Pinned {
  units_per_em: f32,
  advances: FxHashMap<u32, f32>,
  missing: f32,
  average: f32,
  widest: f32,
  ascent: f32,
  descent: f32,
  underline: Line,
  strikeout: Line,
}

/// Where a decoration line sits: its top's height above the baseline, and its
/// thickness.
#[derive(Clone, Copy)]
pub(crate) struct Line {
  /// The top's height above the baseline; negative below it.
  pub offset: f32,
  /// The thickness.
  pub thickness: f32,
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
    let face = font.metrics(Size::unscaled(), LocationRef::default());
    let units_per_em = f32::from(face.units_per_em);
    let line = |decoration: Option<skrifa::metrics::Decoration>, offset: f32| {
      decoration.map_or(
        Line {
          offset: offset * units_per_em,
          thickness: units_per_em / 16.,
        },
        |decoration| Line {
          offset: decoration.offset,
          thickness: decoration.thickness,
        },
      )
    };

    Self {
      units_per_em,
      advances,
      missing: metrics.advance_width(GlyphId::NOTDEF).unwrap_or(0.),
      average: face.average_width.unwrap_or(units_per_em / 2.),
      widest: face.max_width.unwrap_or(units_per_em),
      ascent: face.ascent,
      descent: face.descent,
      underline: line(face.underline, -0.1),
      strikeout: line(face.strikeout, 0.3),
    }
  }

  /// The face, to read glyphs from.
  pub(crate) fn font() -> FontRef<'static> {
    FontRef::new(Self::DATA).expect("the pinned face parses")
  }

  /// How far `character` advances a line at `size`.
  pub(crate) fn advance_of(&self, character: char, size: f32) -> f32 {
    self
      .advances
      .get(&u32::from(character))
      .copied()
      .unwrap_or(self.missing)
      * size
      / self.units_per_em
  }

  /// How wide a text entry lays out at `size`, its value `lines` tall, as Chromium sizes
  /// an empty `<input>` and `<textarea>`: twenty average characters, and a single line
  /// the face's widest advance past the first of them.
  pub(crate) fn entry_width(&self, size: f32, lines: u16) -> f32 {
    let scale = size / self.units_per_em;
    let average = self.average * scale;
    let characters = (average * 20.).ceil();

    if lines > 1 {
      characters
    }
    else {
      characters + (self.widest * scale).round() - average
    }
  }

  /// How far the baseline of a line `height` tall at `size` sits below its top: half
  /// the line's room beyond the face's ascent and descent above, as CSS leads a line.
  pub(crate) fn baseline(&self, height: f32, size: f32) -> f32 {
    let scale = size / self.units_per_em;
    let content = (self.ascent - self.descent) * scale;

    (height - content) / 2. + self.ascent * scale
  }

  /// The face's underline at `size`, in pixels.
  pub(crate) fn underline(&self, size: f32) -> Line {
    self.scaled(self.underline, size)
  }

  /// The face's strikeout at `size`, in pixels.
  pub(crate) fn strikeout(&self, size: f32) -> Line {
    self.scaled(self.strikeout, size)
  }

  /// How far above the baseline the face's ascent reaches at `size`, in pixels.
  pub(crate) fn ascent(&self, size: f32) -> f32 {
    self.ascent * size / self.units_per_em
  }

  /// `line` at `size`, in pixels.
  fn scaled(&self, line: Line, size: f32) -> Line {
    let scale = size / self.units_per_em;

    Line {
      offset: line.offset * scale,
      thickness: line.thickness * scale,
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
