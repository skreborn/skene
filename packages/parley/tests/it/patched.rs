//! The changes Skene carries in Parley (`vendor/parley`, `vendor/parley_engine`), each held
//! against a case the pinned face can set: a soft hyphen's break, breaks between clusters,
//! UAX #9's rule L1, tab stops, the per-word shape cache, and stacked acutes.

use std::ops::Range;

use skene_foundation::text::{
  AttributedText,
  BaseDirection,
  FontSlant,
  FontWeight,
  Laid as _,
  Paragraph as _,
  ParagraphStyle,
  TextEngine as _,
  TextRun,
  TextStyle,
};
use skene_parley::{GlyphRun, ParleyEngine, ParleyLaid, TabSize};
use skrifa::{
  FontRef,
  GlyphId,
  MetadataProvider as _,
  instance::{LocationRef, Size},
};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::inter;

/// The size the cases are set at.
const CORPUS: f32 = 17.;

const SOFT_HYPHENS: &str = "Incom\u{AD}pre\u{AD}hen\u{AD}si\u{AD}bil\u{AD}i\u{AD}ties every\u{AD}where, extra\u{AD}or\u{AD}di\u{AD}nary";
const THAI: &str = "ภาษาไทยเป็นภาษาที่ไม่มีการเว้นวรรคระหว่างคำ การตัดคำจึงต้องใช้พจนานุกรม";
const KHMER: &str = "ភាសាខ្មែរគឺជាភាសាផ្លូវការរបស់ប្រទេសកម្ពុជា ហើយត្រូវបានប្រើប្រាស់ដោយប្រជាជនរាប់លាននាក់";
const LAO: &str = "ພາສາລາວເປັນພາສາທາງການຂອງສາທາລະນະລັດປະຊາທິປະໄຕປະຊາຊົນລາວ";
const BURMESE: &str = "မြန်မာဘာသာစကားသည် မြန်မာနိုင်ငံ၏ ရုံးသုံးဘာသာစကား ဖြစ်သည်။";
const DEVANAGARI: &str = "हिन्दी भारत की राजभाषा है। क्षत्रिय, ज्ञान, श्री, द्वार, कृष्ण";
const BIDI_NUMBERS: &str = "Order 12345 שולם ב-15/10/2026 בסך ₪1,234.50 (כולל מע״מ) total";
const BIDI_LTR: &str = "Hello שלום עולם world مرحبا بالعالم 123";
const BIDI_RTL: &str = "مرحبا Hello 123 world عالم!";
const TABS: &str = "col1\tcol2\tcol3\n1\t22\t333";

/// `text` laid out by `engine` at the cases' size within `width`.
fn lay(engine: &ParleyEngine, text: &str, width: Option<f32>) -> ParleyLaid {
  lay_in(engine, text, ParagraphStyle::default(), width)
}

/// `text` laid out by `engine` at the cases' size in `style` within `width`.
fn lay_in(
  engine: &ParleyEngine,
  text: &str,
  style: ParagraphStyle,
  width: Option<f32>,
) -> ParleyLaid {
  engine
    .paragraph(&AttributedText::plain(text, TextStyle::new(CORPUS)), &style)
    .layout(width)
}

/// Each line's text.
fn lines<'a>(laid: &ParleyLaid, text: &'a str) -> Vec<&'a str> {
  laid
    .lines()
    .iter()
    .map(|line| &text[line.range.clone()])
    .collect()
}

/// Each line's clusters in visual order: the bytes each covers, its left edge and advance.
fn visual(laid: &ParleyLaid) -> Vec<Vec<(Range<usize>, f32, f32)>> {
  laid
    .layout()
    .lines()
    .map(|line| {
      let mut x = line.metrics().offset;
      let mut clusters = Vec::new();

      for run in line.runs() {
        for cluster in run.visual_clusters() {
          clusters.push((cluster.text_range(), x, cluster.advance()));
          x += cluster.advance();
        }
      }

      clusters
    })
    .collect()
}

/// The glyph `ch` maps to in the font of `run`, with its advance at the run's size.
fn glyph_in(run: &GlyphRun, ch: char) -> Option<(u32, f32)> {
  let font = FontRef::from_index(run.font.data.data(), run.font.index).ok()?;
  let glyph = font.charmap().map(ch)?;
  let advance = font
    .glyph_metrics(Size::new(run.size), LocationRef::default())
    .advance_width(glyph)?;

  Some((glyph.to_u32(), advance))
}

#[test]
fn a_break_after_a_soft_hyphen_draws_a_hyphen_at_its_width() {
  let engine = inter();

  for width in [90., 140.] {
    let laid = lay(&engine, SOFT_HYPHENS, Some(width));
    let runs = laid.glyph_runs();
    let mut hyphens = 0;

    for line in laid.lines() {
      assert!(line.width <= width, "a line fits its hyphen: {line:?}");

      // Every soft hyphen's glyph on the line: the hyphen where the line breaks after it, a
      // blank anywhere else.
      for (offset, _) in SOFT_HYPHENS[line.range.clone()].match_indices('\u{AD}') {
        let at = line.range.start + offset;
        let (run, glyph) = runs
          .iter()
          .find_map(|run| {
            run
              .glyphs
              .iter()
              .find(|glyph| glyph.at == at)
              .map(|glyph| (run, *glyph))
          })
          .expect("a soft hyphen has a glyph");
        let (hyphen, advance) = glyph_in(run, '\u{2010}')
          .or_else(|| glyph_in(run, '-'))
          .expect("the font draws a hyphen");

        if at + '\u{AD}'.len_utf8() == line.range.end {
          hyphens += 1;
          assert_eq!(glyph.id, hyphen, "the break draws a hyphen");
          assert!(
            (glyph.x + advance - (line.left + line.width)).abs() < 0.01,
            "the line's width ends with its hyphen: {} + {advance} against {}",
            glyph.x,
            line.left + line.width
          );
        }
        else {
          assert_ne!(
            glyph.id, hyphen,
            "a soft hyphen inside a line draws nothing"
          );
        }
      }
    }

    assert!(
      hyphens >= 3,
      "the lines break after soft hyphens ({hyphens}): {:?}",
      lines(&laid, SOFT_HYPHENS)
    );
  }
}

#[test]
fn a_soft_hyphen_breaks_the_least_width() {
  let engine = inter();
  let paragraph = engine.paragraph(
    &AttributedText::plain(SOFT_HYPHENS, TextStyle::new(CORPUS)),
    &ParagraphStyle::default(),
  );
  let least = paragraph.intrinsic().min;
  let widest_word = SOFT_HYPHENS
    .split(' ')
    .map(|word| {
      engine
        .paragraph(
          &AttributedText::plain(word.replace('\u{AD}', ""), TextStyle::new(CORPUS)),
          &ParagraphStyle::default(),
        )
        .intrinsic()
        .max
    })
    .fold(0., f32::max);

  assert!(
    least < widest_word - 1.,
    "the least width breaks after soft hyphens: {least} against the widest word's {widest_word}"
  );
}

/// The invisible stackers a line never starts after: Myanmar's virama, Khmer's coeng and Tai
/// Tham's sakot, each joining the consonant after it into its cluster.
const STACKERS: [char; 3] = ['\u{1039}', '\u{17D2}', '\u{1A60}'];

/// Whether no line of `laid` starts inside a cluster of `text`: every line starts at a
/// grapheme boundary, never after an invisible stacker.
fn breaks_between_clusters(laid: &ParleyLaid, text: &str) -> Result<(), String> {
  let graphemes: Vec<usize> = text.grapheme_indices(true).map(|(at, _)| at).collect();

  for line in laid.lines().iter().skip(1) {
    let at = line.range.start;
    let next = text[at..].chars().next().unwrap_or(' ');
    let previous = text[..at].chars().next_back().unwrap_or(' ');

    if !graphemes.contains(&at) || STACKERS.contains(&previous) {
      return Err(format!(
        "{:?} breaks between {previous:?} and {next:?}",
        lines(laid, text)
      ));
    }
  }

  Ok(())
}

#[test]
fn no_break_falls_inside_a_cluster() {
  let engine = inter();

  for text in [BURMESE, KHMER, LAO, THAI, DEVANAGARI] {
    for width in (10..=200).step_by(5) {
      #[expect(clippy::cast_precision_loss, reason = "small widths")]
      let laid = lay(&engine, text, Some(width as f32));

      if let Err(error) = breaks_between_clusters(&laid, text) {
        panic!("at {width}: {error}");
      }
    }
  }
}

#[test]
fn no_break_falls_where_a_style_splits_a_grapheme() {
  // Each acute set larger than its base: the change of size splits each grapheme in two
  // shaping runs, which must not be two places to break.
  let text = "e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}";
  let runs = text
    .char_indices()
    .filter(|(_, ch)| *ch == '\u{301}')
    .map(|(at, ch)| TextRun {
      range: at..at + ch.len_utf8(),
      style: TextStyle::new(CORPUS + 6.),
    })
    .collect();
  let attributed = AttributedText::new(text, TextStyle::new(CORPUS), runs);
  let paragraph = inter().paragraph(&attributed, &ParagraphStyle::default());

  for width in [1., 5., 10., 15., 20.] {
    let laid = paragraph.layout(Some(width));

    assert!(laid.line_count() > 1, "the text wraps at {width}");
    breaks_between_clusters(&laid, text).unwrap();
  }
}

#[test]
fn a_wrapped_bidi_lines_trailing_blanks_hang_at_its_end() {
  // UAX #9 rule L1: whitespace ending a line takes the paragraph's level, so it is drawn at the
  // line's end edge (right in a left-to-right paragraph, left in a right-to-left one), where it
  // hangs out of the line's width.
  let engine = inter();

  for (text, width, right_to_left) in [
    (BIDI_LTR, 120., false),
    (BIDI_RTL, 100., true),
    (BIDI_NUMBERS, 150., false),
  ] {
    let style = if right_to_left {
      ParagraphStyle::default().with_direction(BaseDirection::RightToLeft)
    }
    else {
      ParagraphStyle::default()
    };
    let laid = lay_in(&engine, text, style, Some(width));
    let clusters = visual(&laid);
    let mut checked = 0;

    for (line, clusters) in laid.lines().iter().zip(&clusters) {
      let trailing = text[line.range.clone()].trim_end().len() + line.range.start;
      let (blank, ink): (Vec<_>, Vec<_>) = clusters
        .iter()
        .partition(|(range, _, _)| range.start >= trailing);

      if blank.is_empty() || ink.is_empty() {
        continue;
      }

      checked += 1;

      let ink_left = ink.iter().map(|(_, x, _)| *x).fold(f32::MAX, f32::min);
      let ink_right = ink
        .iter()
        .map(|(_, x, advance)| x + advance)
        .fold(f32::MIN, f32::max);

      for (range, x, advance) in &blank {
        if right_to_left {
          assert!(
            x + advance <= ink_left + 0.01,
            "{:?}: the blank at {range:?} sits left of the line's text",
            &text[line.range.clone()]
          );
        }
        else {
          assert!(
            *x >= ink_right - 0.01,
            "{:?}: the blank at {range:?} sits right of the line's text",
            &text[line.range.clone()]
          );
        }
      }

      assert!(
        (line.width - (ink_right - ink_left)).abs() < 0.01,
        "{:?}: the blanks hang out of the line's width",
        &text[line.range.clone()]
      );
    }

    assert!(
      checked >= 2,
      "{text}: lines with trailing blanks are checked: {:?}",
      lines(&laid, text)
    );
  }
}

#[test]
fn a_tab_advances_to_a_stop_eight_spaces_apart() {
  let laid = lay(&inter(), TABS, None);
  let clusters: Vec<_> = visual(&laid).into_iter().flatten().collect();
  let x = |piece: &str| {
    let at = TABS.find(piece).expect("a piece of the text");

    clusters
      .iter()
      .find(|(range, _, _)| range.start == at)
      .map(|(_, x, _)| *x)
      .expect("a cluster starts there")
  };
  let run = &laid.glyph_runs()[0];
  let (space_glyph, space) = glyph_in(run, ' ').expect("the font has a space");
  let stop = 8. * space;

  for (first, second) in [("col2", "22"), ("col3", "333")] {
    assert!(
      (x(first) - x(second)).abs() < 0.01,
      "{first} and {second} align"
    );
    assert!(
      (x(first) / stop - (x(first) / stop).round()).abs() < 0.001,
      "{first} starts at a stop: {} against {stop}",
      x(first)
    );
  }

  assert!(x("col2") > x("col1") + space / 2.);
  assert!(
    laid
      .glyph_runs()
      .iter()
      .flat_map(|run| &run.glyphs)
      .filter(|glyph| TABS.as_bytes()[glyph.at] == b'\t')
      .all(|glyph| glyph.id == space_glyph),
    "a tab draws a blank"
  );
}

#[test]
fn an_engine_sets_its_tab_stops() {
  let engine = inter();

  engine.set_tab_size(TabSize::Length(50.));

  let laid = lay(&engine, "a\tb\tc", None);
  let clusters: Vec<_> = visual(&laid).into_iter().flatten().collect();

  assert!((clusters[2].1 - 50.).abs() < 0.01, "b at the first stop");
  assert!((clusters[4].1 - 100.).abs() < 0.01, "c at the second");
}

/// Whether two marks' ink boxes (`left`, `right`, `top`, `bottom`) overlap as a collision does.
fn collide(a: [f32; 4], b: [f32; 4]) -> bool {
  let across = a[1].min(b[1]) - a[0].max(b[0]);
  let down = a[3].min(b[3]) - a[2].max(b[2]);

  across > 0.25 * (a[1] - a[0]).min(b[1] - b[0]) && down > 0.1 * (a[3] - a[2]).min(b[3] - b[2])
}

/// Acutes stacked over a base, the one mark the pinned face holds: over bases Latin-1 has no
/// accented letter for, which shaping would compose with the first acute.
const STACKS: &[&str] = &[
  "q\u{301}\u{301}",
  "x\u{301}\u{301}\u{301}",
  "Z\u{301}\u{301}",
  "m\u{301}\u{301}\u{301}\u{301}",
  "Stacked: a\u{301}\u{301} w\u{301}\u{301} e\u{301}",
];

/// The ink boxes (`left`, `right`, `top`, `bottom`) of the glyphs `laid` draws, grapheme by
/// grapheme of `text`.
fn inks_by_grapheme(laid: &ParleyLaid, text: &str) -> Vec<Vec<[f32; 4]>> {
  let mut graphemes: Vec<(usize, Vec<[f32; 4]>)> = Vec::new();

  for run in laid.glyph_runs() {
    let font = FontRef::from_index(run.font.data.data(), run.font.index).expect("a font");
    let metrics = font.glyph_metrics(Size::new(run.size), LocationRef::default());

    for glyph in &run.glyphs {
      let Some(bounds) = metrics.bounds(GlyphId::new(glyph.id))
      else {
        continue;
      };
      let ink = [
        glyph.x + bounds.x_min,
        glyph.x + bounds.x_max,
        glyph.y - bounds.y_max,
        glyph.y - bounds.y_min,
      ];
      let grapheme = text
        .grapheme_indices(true)
        .map(|(at, _)| at)
        .take_while(|at| *at <= glyph.at)
        .last()
        .unwrap_or(0);

      match graphemes.last_mut() {
        Some((at, inks)) if *at == grapheme => inks.push(ink),
        _ => graphemes.push((grapheme, vec![ink])),
      }
    }
  }

  graphemes.into_iter().map(|(_, inks)| inks).collect()
}

#[test]
fn stacked_acutes_clear_one_another() {
  // No two marks of a grapheme collide as drawn.
  let engine = inter();
  let mut pairs = 0;

  for text in STACKS {
    let laid = lay(&engine, text, None);

    for inks in inks_by_grapheme(&laid, text) {
      // The base, then its marks: no two marks collide.
      for (index, a) in inks.iter().enumerate().skip(1) {
        for b in inks.iter().skip(index + 1) {
          pairs += 1;
          assert!(
            !collide(*a, *b),
            "the marks of {text:?} collide: {a:?} {b:?}"
          );
        }
      }
    }
  }

  assert!(pairs >= 8, "the cases stack marks ({pairs} pairs)");
}

/// Every glyph of `laid`: its font's data length, its id and its position.
fn glyphs(laid: &ParleyLaid) -> Vec<(usize, u32, f32, f32, usize)> {
  laid
    .glyph_runs()
    .iter()
    .flat_map(|run| {
      run.glyphs.iter().map(|glyph| {
        (
          run.font.data.data().len(),
          glyph.id,
          glyph.x,
          glyph.y,
          glyph.at,
        )
      })
    })
    .collect()
}

#[test]
fn the_shape_cache_changes_no_layout() {
  let texts = [
    SOFT_HYPHENS,
    THAI,
    KHMER,
    LAO,
    BURMESE,
    DEVANAGARI,
    BIDI_NUMBERS,
    BIDI_LTR,
    BIDI_RTL,
    TABS,
    "Stacked: a\u{301}\u{301} é ñ Ünïcödé",
    "مرحبا بالعالم هذا نص عربي طويل للالتفاف مرحبا بالعالم",
    "Emoji 👍🏽 👨‍👩‍👧 🇰🇷 😀 ❤️ ☺ ☺️ office affine ffi -> != == www",
    "Item number 1, a row of the list. Item number 2, a row of the list.",
    "Iİıi fi fl Tİ İstanbul",
  ];
  let cold = inter();
  let cached = inter();

  cold.set_shape_cache(0);

  for round in 0..2 {
    for text in texts {
      // Plain, and in runs of other sizes, weights and slants, so words meet the cache under
      // every key.
      let mut styled = vec![];

      for (index, (at, word)) in text.split_word_bound_indices().enumerate() {
        let style = match index % 4 {
          0 => TextStyle::new(CORPUS + 3.),
          1 => TextStyle::new(CORPUS).with_weight(FontWeight::BOLD),
          2 => TextStyle::new(CORPUS).with_slant(FontSlant::Italic),
          _ => TextStyle::new(CORPUS),
        };

        styled.push(TextRun {
          range: at..at + word.len(),
          style,
        });
      }

      for attributed in [
        AttributedText::plain(text, TextStyle::new(CORPUS)),
        AttributedText::new(text, TextStyle::new(CORPUS), styled),
      ] {
        for width in [None, Some(120.)] {
          let style = ParagraphStyle::default();
          let (a, b) = (
            cold.paragraph(&attributed, &style).layout(width),
            cached.paragraph(&attributed, &style).layout(width),
          );

          assert_eq!(glyphs(&a), glyphs(&b), "round {round}: {text} at {width:?}");
          assert_eq!(
            a.lines().iter().map(|line| line.width).collect::<Vec<_>>(),
            b.lines().iter().map(|line| line.width).collect::<Vec<_>>()
          );
        }
      }
    }
  }

  let (hits, misses) = cached.shape_cache_stats();

  assert!(
    hits > misses,
    "words repeat from the cache: {hits} found, {misses} shaped"
  );
  assert_eq!(
    cold.shape_cache_stats(),
    (0, 0),
    "a cache of no words keeps none"
  );
}

#[test]
fn a_word_a_style_splits_is_cached_by_what_joins_it() {
  // A word is cached alone, then met again joined to a letter before it and after it that a
  // size sets apart, so it is shaped again in its context and not taken from the cache.
  let cold = inter();
  let cached = inter();

  cold.set_shape_cache(0);

  for (text, larger) in [
    ("\u{639}\u{627}\u{644}\u{645}", 0..0),
    ("\u{628}\u{627}\u{644}\u{639}\u{627}\u{644}\u{645}", 0..6),
    ("\u{639}\u{627}\u{644}\u{645}\u{628}", 8..10),
  ] {
    let attributed = AttributedText::new(
      text,
      TextStyle::new(CORPUS),
      vec![TextRun {
        range: larger,
        style: TextStyle::new(CORPUS + 3.),
      }],
    );
    let style = ParagraphStyle::default();

    assert_eq!(
      glyphs(&cold.paragraph(&attributed, &style).layout(None)),
      glyphs(&cached.paragraph(&attributed, &style).layout(None)),
      "{text}"
    );
  }

  assert_eq!(
    cached.shape_cache_stats().0,
    0,
    "the word joined is no word found"
  );
}

/// Text across spaces a full face's contextual alternates read across (an `x` between
/// numbers, arrows, a colon between numbers, raised punctuation), and words between, in each of
/// which a run splits.
const ACROSS_A_SPACE: &[&str] = &[
  "Order 3 x 4 = 12 and 1 - 2 : 3 or 10 :30 then (A) [B] {C} @ home",
  "a -> b <- c => d <= e != f >= g -- h \u{2014} i \u{2192} j",
  "A Taste Of Yesterday Very Well",
  "1 2 12 21 1 1 2 2 11 22",
  "office affine, the quick brown fox; the quick brown fox",
];

#[test]
fn the_shape_cache_keeps_what_a_font_does_across_a_space() {
  let cold = inter();
  let cached = inter();

  cold.set_shape_cache(0);

  for round in 0..2 {
    for text in ACROSS_A_SPACE {
      let attributed = AttributedText::plain(*text, TextStyle::new(CORPUS));
      let paragraph = ParagraphStyle::default();
      let (a, b) = (
        cold.paragraph(&attributed, &paragraph).layout(None),
        cached.paragraph(&attributed, &paragraph).layout(None),
      );

      assert_eq!(glyphs(&a), glyphs(&b), "round {round}: {text}");
    }
  }

  let (hits, misses) = cached.shape_cache_stats();

  assert!(
    hits > misses,
    "words repeat from the cache: {hits} found, {misses} shaped"
  );
}

#[test]
fn a_run_splits_where_its_font_does_nothing_across_its_spaces() {
  let engine = inter();
  let shape = |text: &str| {
    let before = engine.shape_cache_stats();
    let _ = engine.paragraph(
      &AttributedText::plain(text, TextStyle::new(CORPUS)),
      &ParagraphStyle::default(),
    );
    let after = engine.shape_cache_stats();

    (after.0 - before.0, after.1 - before.1)
  };

  assert_eq!(shape("hello world"), (0, 2), "two words shaped");
  assert_eq!(shape("hello there"), (1, 1), "a word found, a word shaped");
}
