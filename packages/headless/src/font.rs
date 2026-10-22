//! The pinned font headless shapes and draws text in: Inter's regular face (4.1),
//! subset to Latin-1 and the combining acute, the face the conformance suite's pages
//! draw in.

use std::sync::OnceLock;

use skene_parley::{ParleyEngine, Typeface};
use skrifa::{
  MetadataProvider,
  prelude::{FontRef, LocationRef, Size},
};

/// The pinned face, as the layout shapes text in it.
pub(crate) const INTER: Typeface = Typeface::new("Inter", Pinned::DATA);

thread_local! {
  /// The engine text is laid out through, in the pinned face alone.
  static ENGINE: ParleyEngine = {
    let engine = ParleyEngine::with_typefaces(&[INTER]);

    engine.set_quantize(false);

    engine
  };
}

/// The engine text is laid out through: Parley over the pinned face alone, its tab stops
/// eight blanks apart as a browser's `tab-size` sets them, its line metrics fractional as a
/// browser keeps them.
pub(crate) fn engine() -> ParleyEngine {
  ENGINE.with(Clone::clone)
}

/// The pinned face's metrics a text entry is sized and a text trimmed by, read once from
/// its tables, in font units.
pub(crate) struct Pinned {
  units_per_em: f32,
  average: f32,
  span: f32,
  cap: f32,
}

impl Pinned {
  /// The face, as OpenType.
  pub(crate) const DATA: &'static [u8] = include_bytes!("../fonts/inter-regular.otf");

  /// The face's metrics.
  pub(crate) fn get() -> &'static Self {
    static PINNED: OnceLock<Pinned> = OnceLock::new();

    PINNED.get_or_init(Self::read)
  }

  /// Reads the metrics from the face's tables.
  fn read() -> Self {
    let face = Self::font().metrics(Size::unscaled(), LocationRef::default());
    let units_per_em = f32::from(face.units_per_em);

    Self {
      units_per_em,
      cap: face.cap_height.unwrap_or(face.ascent * 0.7),
      average: face.average_width.unwrap_or(units_per_em / 2.),
      span: face
        .bounds
        .map_or(units_per_em, |bounds| bounds.x_max - bounds.x_min),
    }
  }

  /// How far above the baseline the face's capital letters reach at a font size of one:
  /// its cap height, or seven tenths of its ascent where it states none.
  pub(crate) fn cap_height(&self) -> f32 {
    self.cap / self.units_per_em
  }

  /// The face, to read glyphs from.
  pub(crate) fn font() -> FontRef<'static> {
    FontRef::new(Self::DATA).expect("the pinned face parses")
  }

  /// How wide a text entry lays out at `size`, its value `lines` tall, as Chromium sizes
  /// an empty `<input>` and `<textarea>` beside overlay scrollbars: twenty average
  /// characters, and a single line the width of the face's bounds in place of one of
  /// them, rounded up to a pixel.
  pub(crate) fn entry_width(&self, size: f32, lines: u16) -> f32 {
    let scale = size / self.units_per_em;
    let average = self.average * scale;

    if lines > 1 {
      (average * 20.).ceil()
    }
    else {
      (average * 19. + (self.span * scale).round()).ceil()
    }
  }
}
