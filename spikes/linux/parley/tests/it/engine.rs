//! The engine against the trait's contract.

use skene_foundation::{
  Color,
  text::{
    Intrinsic, Laid as _, LeafMeasure, MeasureText as _, PaintStyle, Paragraph as _,
    ParagraphStyle, Space, TextLeaf,
  },
};

use crate::{laid, paragraph};

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

#[test]
fn a_repaint_changes_colors_and_no_geometry() {
  let mut paragraph = paragraph("alpha beta", ParagraphStyle::default());
  let laid = paragraph.layout(None);
  let before = laid.run_extents();

  assert!(paragraph.repaint(6..10, &PaintStyle::new(Color::rgb(0xff_0000))));

  let after = laid.run_extents();

  assert_eq!(
    laid.paint_at(7).color,
    Color::rgb(0xff_0000),
    "the existing layout repaints"
  );
  assert_eq!(laid.paint_at(0).color, Color::BLACK);
  assert!(after.len() > before.len(), "the stretch splits");
  assert_eq!(
    after.iter().map(|extent| extent.width).sum::<f32>(),
    before.iter().map(|extent| extent.width).sum::<f32>()
  );
}
