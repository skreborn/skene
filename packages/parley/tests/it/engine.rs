//! The engine against the trait's contract (a layout's geometry never changes, and its paint
//! is its paragraph's) and the guaranteed core (blanks and newlines kept, a word too wide for
//! its line broken where the line fills, an empty text one line).

use std::{fmt::Write as _, ops::Range, rc::Rc};

use skene_foundation::{
  Color,
  text::{
    AttributedText,
    Intrinsic,
    Laid,
    LeafMeasure,
    MaskState,
    Masked,
    MeasureText as _,
    PaintStyle,
    Paragraph as _,
    ParagraphStyle,
    Position,
    Space,
    TextEngine as _,
    TextLeaf,
    TextStyle,
  },
};
use skene_parley::PARALLEL;

use crate::{SIZE, inter, laid, paragraph};

#[test]
fn a_batch_shapes_as_its_paragraphs_shape_one_at_a_time() {
  let engine = inter();
  let texts: Vec<(AttributedText, ParagraphStyle)> = (0..PARALLEL * 2)
    .map(|index| {
      (
        AttributedText::plain(
          format!("Item number {index}, a row of the list"),
          TextStyle::new(SIZE),
        ),
        ParagraphStyle::default(),
      )
    })
    .collect();
  let batch = engine.paragraphs(texts.iter().cloned());

  assert_eq!(batch.len(), texts.len());

  for ((text, style), made) in texts.iter().zip(&batch) {
    let shaped = &made.paragraph;
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
  let empty = laid("", None);
  let letter = laid("a", None);

  assert_eq!(empty.line_count(), 1);
  assert!(empty.size().height > 0.);
  assert!(
    (empty.size().height - letter.size().height).abs() < 0.01,
    "an empty text is a line as tall as a letter's"
  );
  assert_eq!(laid("one\n", None).line_count(), 2);
  assert!(laid("one\n", None).size().height > letter.size().height);
}

#[test]
fn blanks_and_newlines_are_kept() {
  let one = laid("a b", None).size().width;
  let three = laid("a   b", None).size().width;

  assert!(three > one + 4., "blanks are not collapsed: {one} {three}");

  let leading = laid("  a", None).size().width;

  assert!(
    leading > laid("a", None).size().width + 4.,
    "leading blanks are kept"
  );
  assert_eq!(laid("a\nb", None).line_count(), 2);
  assert_eq!(
    laid("a\n\nb", None).line_count(),
    3,
    "an empty line is kept"
  );
}

#[test]
fn a_word_too_wide_for_its_line_breaks_where_the_line_fills() {
  let text = "incomprehensibilities";
  let whole = laid(text, None).size().width;
  let laid = laid(text, Some(whole / 3.));
  let lines = laid.lines();

  assert!(lines.len() >= 3, "{lines:?}");

  for line in &lines[..lines.len() - 1] {
    assert!(line.width <= whole / 3. + 0.01, "{line:?}");
    assert!(
      line.width > whole / 3. - SIZE,
      "the line fills before it breaks: {line:?}"
    );
  }
}

#[test]
fn thai_breaks_between_words() {
  // The dictionary's words, each a place a line may start: Thai sets no blank between them.
  let words = [
    "ภาษา",
    "ไทย",
    "เป็น",
    "ภาษา",
    "ที่",
    "ไม่",
    "มี",
    "การ",
    "เว้น",
    "วรรค",
    "ระหว่าง",
    "คำ",
  ];
  let text = words.concat();
  let starts: Vec<usize> = words
    .iter()
    .scan(0, |at, word| {
      let start = *at;

      *at += word.len();

      Some(start)
    })
    .collect();
  let paragraph = inter().paragraph(
    &AttributedText::plain(text.as_str(), TextStyle::new(13.)),
    &ParagraphStyle::default(),
  );
  let widths = paragraph.intrinsic();

  assert!(
    widths.min < 0.4 * widths.max,
    "words break where no blank is: {widths:?}"
  );

  let mut wrapped = 0;

  for width in (60..=200).step_by(10) {
    #[expect(clippy::cast_precision_loss, reason = "small widths")]
    let laid = paragraph.layout(Some(width as f32));
    let lines = laid.lines();

    if lines.len() > 1 {
      wrapped += 1;
    }

    for line in lines.iter().skip(1) {
      assert!(
        starts.contains(&line.range.start),
        "at {width}, a line starts inside a word: {:?}",
        lines
          .iter()
          .map(|line| &text[line.range.clone()])
          .collect::<Vec<_>>()
      );
    }
  }

  assert!(wrapped > 5, "the text wraps at most widths: {wrapped}");
}

#[test]
fn most_lines_cut_the_text_with_an_ellipsis() {
  let text = "A long label that must fit in two lines and end with an ellipsis when it overflows";
  let laid = paragraph(text, ParagraphStyle::default().with_max_lines(2)).layout(Some(120.));

  assert_eq!(laid.line_count(), 2);
  assert!(laid.truncated());
  assert!(laid.lines().iter().all(|line| line.width <= 120.));
  assert!(
    laid.glyph_runs().len() > 1,
    "the ellipsis is drawn beside the text"
  );

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
    let _ = writeln!(
      out,
      "{} {:?} {:?}",
      pair[0],
      laid.caret(Position::new(pair[0])),
      laid.range_rects(pair[0]..pair[1])
    );
  }

  let _ = writeln!(
    out,
    "{:?} {:?}",
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
fn colors(laid: &impl Laid) -> Vec<(Range<usize>, Color)> {
  laid
    .run_extents()
    .into_iter()
    .map(|extent| (extent.range, extent.paint.color))
    .collect()
}

/// Whether every stretch of `laid` inside `range` is painted `inside` and every other one
/// `outside`, with at least one of each.
fn painted(laid: &impl Laid, range: Range<usize>, inside: Color, outside: Color) -> bool {
  let colors = colors(laid);
  let within = |stretch: &Range<usize>| stretch.start >= range.start && stretch.end <= range.end;

  colors.iter().any(|(stretch, _)| within(stretch))
    && colors.iter().any(|(stretch, _)| !within(stretch))
    && colors
      .iter()
      .all(|(stretch, color)| *color == if within(stretch) { inside } else { outside })
}

#[test]
fn a_repaint_shows_in_every_layout_handed_out_and_moves_nothing() {
  let text = "alpha beta gamma";
  let red = Color::srgb(1., 0., 0.);
  let ink = PaintStyle::default().color;
  let mut paragraph = paragraph(text, ParagraphStyle::default());
  let (wide, narrow) = (paragraph.layout(None), paragraph.layout(Some(60.)));
  let before = (geometry(&wide, text), geometry(&narrow, text));

  assert!(narrow.line_count() > 1, "two layouts, at two widths");
  assert!(paragraph.repaint(6..10, &PaintStyle::new(red)));

  for (laid, shape) in [(&wide, &before.0), (&narrow, &before.1)] {
    assert_eq!(&geometry(laid, text), shape, "a repaint moves nothing");
    assert!(
      painted(laid, 6..10, red, ink),
      "a layout handed out before the repaint shows it: {:?}",
      colors(laid)
    );
    assert_eq!(laid.paint_at(7).color, red);
    assert_eq!(laid.paint_at(0).color, ink);
  }

  let after = paragraph.layout(Some(60.));

  assert_eq!(
    geometry(&after, text),
    before.1,
    "nor does a layout made after"
  );
  assert!(painted(&after, 6..10, red, ink));
}

#[test]
fn a_masked_layout_keeps_the_contract_in_the_real_texts_bytes() {
  let text = "pa\u{301}ss wörd";
  let state = Rc::new(MaskState::new('•'));

  state.set_masked(true);

  let engine = Masked::new(inter(), Rc::clone(&state));
  let mut paragraph = engine.paragraph(
    &AttributedText::plain(text, TextStyle::new(SIZE)),
    &ParagraphStyle::default(),
  );
  let laid = paragraph.layout(None);
  let before = geometry(&laid, text);
  let red = Color::srgb(1., 0., 0.);
  // The second cluster to the fourth: `a\u{301}`, `s`, `s`.
  let range = 1..6;

  assert!(paragraph.repaint(range.clone(), &PaintStyle::new(red)));
  assert_eq!(geometry(&laid, text), before, "a repaint moves no mark");
  assert!(
    painted(&laid, range, red, PaintStyle::default().color),
    "the marks of the clusters repainted show it, in the real text's bytes: {:?}",
    colors(&laid)
  );
}

#[test]
fn an_inline_box_flows_among_the_characters_and_raises_its_line() {
  use skene_foundation::text::InlineBox;

  let text = "ab cd";
  let plain = laid(text, None);
  let attributed =
    AttributedText::plain(text, TextStyle::new(SIZE)).with_boxes(vec![InlineBox::new(3, 30., 40.)]);
  let laid = inter()
    .paragraph(&attributed, &ParagraphStyle::default())
    .layout(None);
  let frames = laid.inline_boxes();
  let before = laid.range_rects(0..3);
  let after = laid
    .glyph_runs()
    .iter()
    .flat_map(|run| run.glyphs.clone())
    .find(|glyph| glyph.at == 3)
    .expect("a glyph draws the c");

  assert_eq!(frames.len(), 1);

  let frame = frames[0];

  assert_eq!((frame.size.width, frame.size.height), (30., 40.));
  assert!(
    (laid.size().width - plain.size().width - 30.).abs() < 0.01,
    "the box takes its width in the line"
  );
  assert!(frame.origin.x >= before[0].origin.x + before[0].size.width - 0.01);
  assert!(
    after.x >= frame.origin.x + 30. - 0.01,
    "the c follows the box"
  );
  assert!(laid.size().height >= 40., "the box raises its line");
  assert!(
    (frame.origin.y + 40. - laid.first_baseline()).abs() < 0.01,
    "the box stands on the baseline"
  );

  let cut = inter()
    .paragraph(
      &AttributedText::plain("ab\ncd", TextStyle::new(SIZE))
        .with_boxes(vec![InlineBox::new(4, 10., 10.)]),
      &ParagraphStyle::default().with_max_lines(1),
    )
    .layout(None);

  assert_eq!(
    cut.inline_boxes(),
    vec![skene_foundation::Rect::ZERO],
    "a box on a line cut has no frame"
  );
}

#[test]
fn a_paragraph_starting_its_lines_at_the_left_says_so() {
  use skene_foundation::text::{BaseDirection, TextAlignment};

  let anchored = |text: &str, style: ParagraphStyle| paragraph(text, style).is_left_anchored();
  let style = ParagraphStyle::default();

  assert!(anchored("abc", style));
  assert!(
    !anchored("\u{5E9}\u{5DC}\u{5D5}\u{5DD}", style),
    "right to left"
  );
  assert!(anchored(
    "\u{5E9}\u{5DC}\u{5D5}\u{5DD}",
    style.with_alignment(TextAlignment::End)
  ));
  assert!(!anchored(
    "abc",
    style.with_alignment(TextAlignment::Center)
  ));
  assert!(anchored(
    "abc",
    style
      .with_direction(BaseDirection::RightToLeft)
      .with_alignment(TextAlignment::Left)
  ));

  // Where it says so, a layout wider than the text is the unbounded one.
  let wide = paragraph("abc\nde", style.with_alignment(TextAlignment::Center));

  assert!(
    wide.layout(Some(400.)).lines()[1].left > wide.layout(None).lines()[1].left,
    "a centered line moves with the width"
  );
}

#[test]
fn a_band_of_a_long_text_holds_only_the_lines_that_may_show_in_it() {
  let text = (0..3000).fold(String::new(), |mut text, line| {
    let _ = writeln!(text, "Line number {line}");

    text
  });
  let laid = inter()
    .paragraph(
      &AttributedText::plain(text.as_str(), TextStyle::new(SIZE)),
      &ParagraphStyle::default(),
    )
    .layout(Some(400.));
  let lines = laid.lines();
  let height = lines[1].top - lines[0].top;
  let band = (lines[1000].top, lines[1000].top + height * 10.);
  // The bytes of the lines that reach into the band, a line's height around it.
  let near = |at: usize| {
    lines.iter().any(|line| {
      line.range.contains(&at)
        && line.top + line.height + height >= band.0
        && line.top - height <= band.1
    })
  };
  let all: usize = laid.glyph_runs().iter().map(|run| run.glyphs.len()).sum();
  let shown: Vec<usize> = laid
    .glyph_runs_in(band)
    .iter()
    .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.at))
    .collect();

  assert!(
    shown.iter().all(|at| near(*at)),
    "only the band's lines are drawn"
  );
  assert!(
    shown.len() * 100 < all,
    "a band of ten lines draws a few hundredths of the text: {} of {all}",
    shown.len()
  );

  for (index, line) in lines.iter().enumerate().skip(1000).take(10) {
    assert!(
      shown.iter().any(|at| line.range.contains(at)),
      "line {index} in the band is drawn"
    );
  }

  let extents = laid.run_extents_in(band);

  assert_ne!(extents.len(), 0);
  assert!(extents.iter().all(|extent| near(extent.range.start)));
  assert_eq!(
    laid.run_extents_in((f32::NEG_INFINITY, f32::INFINITY)),
    laid.run_extents(),
    "an unbounded band is the whole text"
  );
}
