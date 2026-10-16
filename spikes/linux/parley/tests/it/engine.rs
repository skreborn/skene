//! The engine against the trait's contract: a layout's geometry never changes, and its paint
//! is its paragraph's.

use std::rc::Rc;

use skene_foundation::{
  Color,
  text::{
    AttributedText, Intrinsic, Laid, LeafMeasure, MaskState, Masked, MeasureText as _, PaintStyle,
    Paragraph as _, ParagraphStyle, Position, RunStyle, Space, TextEngine as _, TextLeaf,
  },
};
use skene_parley::{ParleyEngine, shaping::PARALLEL};

use crate::{SIZE, laid, paragraph};

#[test]
fn a_batch_shapes_as_its_paragraphs_shape_one_at_a_time() {
  let engine = ParleyEngine::system();
  let texts: Vec<(AttributedText, ParagraphStyle)> = (0..PARALLEL * 2)
    .map(|index| {
      (
        AttributedText::plain(
          format!("Item number {index}, a row of the list"),
          RunStyle::new(SIZE),
        ),
        ParagraphStyle::default(),
      )
    })
    .collect();
  let batch = engine.paragraphs(&texts);

  assert_eq!(batch.len(), texts.len());

  for ((text, style), shaped) in texts.iter().zip(&batch) {
    let alone = engine.paragraph(text, style);

    assert_eq!(shaped.text(), text.value());
    assert_eq!(shaped.intrinsic(), alone.intrinsic());

    for width in [None, Some(60.)] {
      let (together, apart) = (shaped.layout(width), alone.layout(width));

      assert_eq!(
        together.size(),
        apart.size(),
        "{} at {width:?}",
        text.value()
      );
      assert_eq!(together.line_count(), apart.line_count());
    }
  }
}

#[test]
fn a_layout_is_immutable_and_independent_of_later_ones() {
  let paragraph = paragraph("alpha beta gamma delta", ParagraphStyle::default());
  let wide = paragraph.layout(None);
  let narrow = paragraph.layout(Some(60.));
  let again = paragraph.layout(None);

  assert_eq!(wide.line_count(), 1);
  assert!(narrow.line_count() > 1);
  assert_eq!(
    wide.size(),
    again.size(),
    "a later layout leaves the first unchanged"
  );
  assert_eq!(wide.line_count(), 1);
}

#[test]
fn the_leaf_reports_used_widths_and_breaks_words_under_min_content() {
  let leaf = TextLeaf::new(paragraph("alpha beta", ParagraphStyle::default()));
  let Intrinsic { min, max } = leaf.intrinsic();
  let measure = |space| {
    let LeafMeasure { width, height, .. } = leaf.measure((None, None), (space, Space::MaxContent));

    (width, height)
  };

  assert!(min < max);
  assert_eq!(measure(Space::MinContent).0, min);
  assert_eq!(measure(Space::MaxContent).0, max);

  let (width, height) = measure(Space::Definite(max - 1.));

  assert!(width <= max - 1. && width >= min - 0.5, "{width}");
  assert!(height > measure(Space::MaxContent).1, "wrapped");

  let (width, _) = measure(Space::Definite(min / 2.));

  assert!(
    width <= min / 2. + 0.5,
    "a word breaks where the line fills: {width} of {min}"
  );
}

#[test]
fn baselines_come_out_of_the_layout() {
  let laid = laid("one\ntwo", None);

  assert_eq!(laid.line_count(), 2);
  assert!(laid.first_baseline() > 0.);
  assert!(laid.last_baseline() > laid.first_baseline());
  assert!(laid.lines()[0].hard_break);
}

#[test]
fn an_empty_text_lays_out_one_line_and_a_trailing_newline_opens_one() {
  assert_eq!(laid("", None).line_count(), 1);
  assert!(laid("", None).size().height > 0.);
  assert_eq!(laid("one\n", None).line_count(), 2);
}

#[test]
fn blanks_and_newlines_are_kept() {
  let one = laid("a b", None).size().width;
  let three = laid("a   b", None).size().width;

  assert!(three > one + 4., "blanks are not collapsed: {one} {three}");
}

#[test]
fn thai_breaks_between_words() {
  // The platform-text case at 90 pixels, set at its size there: Pango's lines (libthai's
  // dictionary) and these (ICU4X's) agree.
  let pango = ["ภาษาไทยเป็น", "ภาษาที่ไม่มีการ", "เว้นวรรค", "ระหว่างคำ"];
  let text = pango.concat();
  let laid = skene_foundation::text::TextEngine::paragraph(
    &skene_parley::ParleyEngine::system(),
    &skene_foundation::text::AttributedText::plain(
      text.as_str(),
      skene_foundation::text::RunStyle::new(13.),
    ),
    &ParagraphStyle::default(),
  )
  .layout(Some(90.));
  let lines: Vec<&str> = laid
    .lines()
    .iter()
    .map(|line| &text[line.range.clone()])
    .collect();

  assert_eq!(lines, pango);
}

#[test]
fn a_break_at_a_soft_hyphen_draws_a_hyphen() {
  let text = "Incom\u{AD}pre\u{AD}hen\u{AD}si\u{AD}bil\u{AD}i\u{AD}ties every\u{AD}where";
  let laid = laid(text, Some(110.));
  let lines = laid.lines();
  let marks = laid.glyph_runs().len()
    - laid
      .layout()
      .lines()
      .map(|line| line.runs().count())
      .sum::<usize>();
  let soft = lines
    .iter()
    .filter(|line| text[line.range.clone()].ends_with('\u{AD}'))
    .count();

  assert!(soft >= 1);
  assert_eq!(marks, soft, "one hyphen a soft break");
  assert!(
    lines.iter().all(|line| line.width <= 110.),
    "the hyphen fits its line"
  );
}

#[test]
fn most_lines_cut_the_text_with_an_ellipsis() {
  let text = "A long label that must fit in two lines and end with an ellipsis when it overflows";
  let laid = paragraph(text, ParagraphStyle::default().with_max_lines(2)).layout(Some(120.));

  assert_eq!(laid.line_count(), 2);
  assert!(laid.truncated());
  assert!(laid.lines().iter().all(|line| line.width <= 120.));

  let whole = paragraph(text, ParagraphStyle::default().with_max_lines(9)).layout(Some(120.));

  assert!(!whole.truncated());
}

/// Everything about `laid` that is geometry, its paint left out: its size, baselines and
/// lines, the carets at every character boundary, the rectangles of each character and of
/// the whole text, and its stretches' places, joined per line where paint split them.
fn geometry(laid: &impl Laid, text: &str) -> String {
  let mut out = format!(
    "{:?} {} {} {} {}\n{:?}\n",
    laid.size(),
    laid.first_baseline(),
    laid.last_baseline(),
    laid.line_count(),
    laid.truncated(),
    laid.lines()
  );
  let bounds: Vec<usize> = text
    .char_indices()
    .map(|(index, _)| index)
    .chain([text.len()])
    .collect();

  for pair in bounds.windows(2) {
    out += &format!(
      "{} {:?} {:?}\n",
      pair[0],
      laid.caret(Position::new(pair[0])),
      laid.range_rects(pair[0]..pair[1])
    );
  }

  out += &format!(
    "{:?} {:?}\n",
    laid.caret(Position::new(text.len())),
    laid.range_rects(0..text.len())
  );

  let mut lines: Vec<(usize, f32, f32, f32)> = Vec::new();

  for extent in laid.run_extents() {
    match lines.last_mut() {
      Some(line) if line.0 == extent.line => line.2 += extent.width,
      _ => lines.push((extent.line, extent.x, extent.width, extent.baseline)),
    }
  }

  out + &format!("{lines:?}")
}

/// The color each stretch of `laid` is painted with, by the bytes it covers.
fn colors(laid: &impl Laid) -> Vec<(std::ops::Range<usize>, Color)> {
  laid
    .run_extents()
    .into_iter()
    .map(|extent| (extent.range, extent.paint.color))
    .collect()
}

/// Whether every stretch of `laid` inside `range` is painted `inside` and every other one
/// `outside`, with at least one of each.
fn painted(laid: &impl Laid, range: std::ops::Range<usize>, inside: Color, outside: Color) -> bool {
  let colors = colors(laid);
  let within =
    |stretch: &std::ops::Range<usize>| stretch.start >= range.start && stretch.end <= range.end;

  colors.iter().any(|(stretch, _)| within(stretch))
    && colors.iter().any(|(stretch, _)| !within(stretch))
    && colors
      .iter()
      .all(|(stretch, color)| *color == if within(stretch) { inside } else { outside })
}

#[test]
fn a_repaint_shows_in_every_layout_handed_out_and_moves_nothing() {
  let text = "alpha beta gamma";
  let red = Color::rgb(0xff_0000);
  let mut paragraph = paragraph(text, ParagraphStyle::default());
  let (wide, narrow) = (paragraph.layout(None), paragraph.layout(Some(60.)));
  let before = (geometry(&wide, text), geometry(&narrow, text));

  assert!(narrow.line_count() > 1, "two layouts, at two widths");
  assert!(paragraph.repaint(6..10, &PaintStyle::new(red)));

  for (laid, shape) in [(&wide, &before.0), (&narrow, &before.1)] {
    assert_eq!(&geometry(laid, text), shape, "a repaint moves nothing");
    assert!(
      painted(laid, 6..10, red, Color::BLACK),
      "a layout handed out before the repaint shows it: {:?}",
      colors(laid)
    );
    assert_eq!(laid.paint_at(7).color, red);
    assert_eq!(laid.paint_at(0).color, Color::BLACK);
  }

  let after = paragraph.layout(Some(60.));

  assert_eq!(
    geometry(&after, text),
    before.1,
    "nor does a layout made after"
  );
  assert!(painted(&after, 6..10, red, Color::BLACK));
}

#[test]
fn a_masked_layout_keeps_the_contract_in_the_real_texts_bytes() {
  let text = "pa\u{301}ss wörd";
  let state = Rc::new(MaskState::new('•'));

  state.set_masked(true);

  let engine = Masked::new(ParleyEngine::system(), Rc::clone(&state));
  let mut paragraph = engine.paragraph(
    &AttributedText::plain(text, RunStyle::new(SIZE)),
    &ParagraphStyle::default(),
  );
  let laid = paragraph.layout(None);
  let before = geometry(&laid, text);
  let red = Color::rgb(0xff_0000);
  // The second cluster to the fourth: `a\u{301}`, `s`, `s`.
  let range = 1..6;

  assert!(paragraph.repaint(range.clone(), &PaintStyle::new(red)));
  assert_eq!(geometry(&laid, text), before, "a repaint moves no mark");
  assert!(
    painted(&laid, range, red, Color::BLACK),
    "the marks of the clusters repainted show it, in the real text's bytes: {:?}",
    colors(&laid)
  );
}
