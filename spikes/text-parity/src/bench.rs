//! Timings: the font collection's start-up, each variant's cold and warm layout, and 1000
//! labels laid out, laid out again at a new width, and drawn.

use std::time::Instant;

use crate::{
  corpus::{Alignment, Case, Corpus, Direction, Hyphenation, LineBreak, Variant, WordBreak},
  layout::{Laid, ParleyEngine},
  record::{LabelTimings, VariantTimings},
  render,
};

/// Milliseconds since `started`.
#[must_use]
pub fn since(started: Instant) -> f64 {
  started.elapsed().as_secs_f64() * 1e3
}

/// The median of `values`.
#[must_use]
pub fn median(mut values: Vec<f64>) -> f64 {
  values.sort_by(f64::total_cmp);

  values.get(values.len() / 2).copied().unwrap_or_default()
}

/// A plain case: `text` in the system font, start-aligned, in English.
#[must_use]
pub fn plain_case(id: &str, text: &str) -> Case {
  Case {
    id: id.to_owned(),
    note: String::new(),
    tags: Vec::new(),
    text: text.to_owned(),
    locale: "en-US".to_owned(),
    family: "system".to_owned(),
    sizes: vec![17.],
    widths: vec![None],
    weight: 400,
    italic: false,
    alignment: Alignment::Start,
    direction: Direction::Auto,
    max_lines: None,
    ellipsis: false,
    hyphenation: Hyphenation::None,
    line_break: LineBreak::Normal,
    word_break: WordBreak::Normal,
  }
}

/// The first layout after the collection is made: `Hello` at 17.
pub fn first_layout(engine: &mut ParleyEngine) -> f64 {
  let case = plain_case("hello", "Hello");
  let variant = Variant {
    case: &case,
    size: 17.,
    width: None,
  };
  let started = Instant::now();
  let _ = engine.lay_out(&variant);

  since(started)
}

/// `variant` laid out once cold and `repeats` times warm.
pub fn time_variant(
  engine: &mut ParleyEngine,
  variant: &Variant<'_>,
  repeats: usize,
) -> (Laid, VariantTimings) {
  let started = Instant::now();
  let laid = engine.lay_out(variant);
  let cold_ms = since(started);
  let warm = (0..repeats)
    .map(|_| {
      let started = Instant::now();
      let _ = engine.lay_out(variant);

      since(started)
    })
    .collect();

  (
    laid,
    VariantTimings {
      cold_ms,
      warm_ms: median(warm),
    },
  )
}

/// `count` labels from the corpus's texts (its cases in turn, the empty one left out), at
/// `size`, wrapped at `width`: built, laid out, laid out again at `width - 10`, and drawn into
/// one image eight columns wide.
pub fn labels(
  engine: &mut ParleyEngine,
  corpus: &Corpus,
  count: usize,
  size: f32,
  width: f32,
) -> LabelTimings {
  let cases: Vec<&Case> = corpus
    .cases
    .iter()
    .filter(|case| !case.text.is_empty())
    .collect();
  let variants: Vec<Variant<'_>> = (0..count)
    .map(|at| Variant {
      case: cases[at % cases.len()],
      size,
      width: Some(width),
    })
    .collect();
  let started = Instant::now();
  let shaped: Vec<_> = variants
    .iter()
    .map(|variant| engine.shape(variant))
    .collect();
  let build_ms = since(started);
  let started = Instant::now();
  let laid: Vec<Laid> = shaped
    .iter()
    .map(|shaped| shaped.break_at(Some(width)))
    .collect();
  let layout_ms = since(started);
  let started = Instant::now();
  let again: Vec<Laid> = shaped
    .iter()
    .map(|shaped| shaped.break_at(Some(width - 10.)))
    .collect();
  let relayout_ms = since(started);

  drop(again);

  const COLUMNS: usize = 8;
  let column = width + 10.;
  let mut tops = [0_f32; COLUMNS];
  let mut placed = Vec::with_capacity(laid.len());

  for (at, laid) in laid.iter().enumerate() {
    let slot = at % COLUMNS;

    #[expect(clippy::cast_precision_loss, reason = "eight columns")]
    placed.push((laid, (slot as f32 * column, tops[slot])));
    tops[slot] += laid.layout.height() + 4.;
  }

  let lines = laid.iter().map(|laid| laid.layout.len()).sum();
  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "an image's extent"
  )]
  let (wide, high) = (
    (column * COLUMNS as f32).ceil().min(65535.) as u16,
    tops.iter().copied().fold(1., f32::max).ceil().min(65535.) as u16,
  );
  let started = Instant::now();
  let (_, glyphs) = render::render_many(&placed, wide, high);
  let draw_ms = since(started);

  LabelTimings {
    count,
    size,
    width,
    build_ms,
    layout_ms,
    relayout_ms: Some(relayout_ms),
    draw_ms: Some(draw_ms),
    glyphs: Some(glyphs),
    lines: Some(lines),
  }
}
