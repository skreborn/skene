//! The changes Skene carries in Parley (`vendor/parley`, `vendor/parley_engine`), each held
//! against the case of `spikes/text-parity`'s corpus that found it, set at the corpus's 17
//! pixels in the system's fonts as fontconfig resolves them.

use std::ops::Range;

use icu_properties::{
  CodePointMapData,
  props::{GeneralCategory, IndicSyllabicCategory, JoiningType},
};
use skene_foundation::text::{
  AttributedText, Laid as _, Paragraph as _, ParagraphStyle, RunStyle, TextEngine as _, TextRun,
};
use skene_parley::{ParleyEngine, ParleyLaid, TabSize, Typeface};
use skrifa::{
  FontRef, MetadataProvider as _,
  instance::{LocationRef, Size},
  raw::TableProvider as _,
};
use unicode_segmentation::UnicodeSegmentation as _;

/// The corpus's size.
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
const COMBINING: &str = "Stacked: á̂̃ ǭ ẋ̣ q̣̇ Z͓͑͒a͔͕l͖g͗o é ñ";

/// `text` laid out by `engine` at the corpus's size within `width`.
fn lay(engine: &ParleyEngine, text: &str, width: Option<f32>) -> ParleyLaid {
  engine
    .paragraph(
      &AttributedText::plain(text, RunStyle::new(CORPUS)),
      &ParagraphStyle::default(),
    )
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
fn glyph_in(run: &skene_parley::GlyphRun, ch: char) -> Option<(u32, f32)> {
  let font = FontRef::from_index(run.font.data.data(), run.font.index).ok()?;
  let glyph = font.charmap().map(ch)?;
  let advance = font
    .glyph_metrics(Size::new(run.size), LocationRef::default())
    .advance_width(glyph)?;

  Some((glyph.to_u32(), advance))
}

#[test]
fn a_break_after_a_soft_hyphen_draws_a_hyphen_at_its_width() {
  // Pango's lines, which a hyphen counted at its width in the line keeps.
  let pango: [(f32, &[&str]); 2] = [
    (
      90.,
      &[
        "Incom\u{AD}pre\u{AD}",
        "hen\u{AD}si\u{AD}bil\u{AD}i\u{AD}",
        "ties every\u{AD}",
        "where, ",
        "extra\u{AD}or\u{AD}di\u{AD}",
        "nary",
      ],
    ),
    (
      140.,
      &[
        "Incom\u{AD}pre\u{AD}hen\u{AD}si\u{AD}",
        "bil\u{AD}i\u{AD}ties every\u{AD}",
        "where, extra\u{AD}or\u{AD}di\u{AD}",
        "nary",
      ],
    ),
  ];
  let engine = ParleyEngine::system();

  for (width, expected) in pango {
    let laid = lay(&engine, SOFT_HYPHENS, Some(width));
    let runs = laid.glyph_runs();
    let mut hyphens = 0;

    assert_eq!(lines(&laid, SOFT_HYPHENS), expected, "at {width}");

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
        } else {
          assert_ne!(
            glyph.id, hyphen,
            "a soft hyphen inside a line draws nothing"
          );
        }
      }
    }

    assert!(
      hyphens >= 3,
      "the lines break after soft hyphens ({hyphens})"
    );
  }
}

/// Whether no line of `laid` starts inside a cluster of `text`: every line starts at a
/// grapheme boundary, not on a combining mark nor after an invisible stacker.
fn breaks_between_clusters(laid: &ParleyLaid, text: &str) -> Result<(), String> {
  let general = const { CodePointMapData::<GeneralCategory>::new() };
  let syllabic = const { CodePointMapData::<IndicSyllabicCategory>::new() };
  let graphemes: Vec<usize> = text.grapheme_indices(true).map(|(at, _)| at).collect();

  for line in laid.lines().iter().skip(1) {
    let at = line.range.start;
    let next = text[at..].chars().next().unwrap_or(' ');
    let previous = text[..at].chars().next_back().unwrap_or(' ');

    if !graphemes.contains(&at)
      || matches!(
        general.get(next),
        GeneralCategory::NonspacingMark
          | GeneralCategory::SpacingMark
          | GeneralCategory::EnclosingMark
      )
      || syllabic.get(previous) == IndicSyllabicCategory::InvisibleStacker
    {
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
  let engine = ParleyEngine::system();

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
      style: RunStyle::new(CORPUS + 6.),
    })
    .collect();
  let attributed = AttributedText::new(text, RunStyle::new(CORPUS), runs);
  let paragraph = ParleyEngine::system().paragraph(&attributed, &ParagraphStyle::default());

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
  let engine = ParleyEngine::system();

  for (text, width, right_to_left) in [
    (BIDI_LTR, 120., false),
    (BIDI_RTL, 100., true),
    (BIDI_NUMBERS, 150., false),
  ] {
    let laid = lay(&engine, text, Some(width));
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
        } else {
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
      "{text}: lines with trailing blanks are checked"
    );
  }
}

#[test]
fn a_tab_advances_to_a_stop_eight_spaces_apart() {
  let laid = lay(&ParleyEngine::system(), TABS, None);
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
  let engine = ParleyEngine::system();

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

/// Stacked marks: the corpus case's (`COMBINING`, two marks above a base and one below in
/// `Z͓͑͒`), and two or three marks on one side of a base, each anchored to the base alone in
/// some fonts.
const STACKS: &[&str] = &[
  "Z\u{353}\u{351}\u{352}",
  "a\u{301}\u{302}\u{303}",
  "q\u{323}\u{307}",
  "x\u{307}\u{323}",
  "e\u{302}\u{301}",
  "a\u{308}\u{301}",
  "u\u{308}\u{304}",
];

/// The families on this system that set Latin with combining marks.
const FAMILIES: &[&str] = &[
  "Inter",
  "DejaVu Sans",
  "DejaVu Serif",
  "DejaVu Sans Mono",
  "Liberation Sans",
  "Liberation Serif",
  "Liberation Mono",
  "FreeSans",
  "FreeSerif",
  "Cantarell",
  "Carlito",
  "Caladea",
  "Unifont",
];

/// The ink boxes (`left`, `right`, `top`, `bottom`) of the glyphs `laid` draws in `family`,
/// grapheme by grapheme of `text`.
fn inks_by_grapheme(laid: &ParleyLaid, text: &str, family: &str) -> Vec<Vec<[f32; 4]>> {
  let mut graphemes: Vec<(usize, Vec<[f32; 4]>)> = Vec::new();

  for run in laid.glyph_runs() {
    let font = FontRef::from_index(run.font.data.data(), run.font.index).expect("a font");
    let name = font
      .localized_strings(skrifa::string::StringId::FAMILY_NAME)
      .english_or_first()
      .map(|name| name.to_string());

    if name.as_deref() != Some(family) {
      continue;
    }

    let metrics = font.glyph_metrics(Size::new(run.size), LocationRef::default());

    for glyph in &run.glyphs {
      let Some(bounds) = metrics.bounds(skrifa::GlyphId::new(glyph.id)) else {
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
fn stacked_marks_clear_one_another() {
  // No two marks of a grapheme collide as drawn, in any of these families.
  let engine = ParleyEngine::system();
  let mut pairs = 0;

  for family in FAMILIES {
    for text in STACKS.iter().copied().chain([COMBINING]) {
      let style = RunStyle::new(CORPUS)
        .with_family(skene_foundation::value::FontFamily::Named((*family).into()));
      let laid = engine
        .paragraph(
          &AttributedText::plain(text, style),
          &ParagraphStyle::default(),
        )
        .layout(None);

      for inks in inks_by_grapheme(&laid, text, family) {
        // The base, then its marks: no two marks collide.
        for (index, a) in inks.iter().enumerate().skip(1) {
          for b in inks.iter().skip(index + 1) {
            pairs += 1;
            assert!(
              !collide(*a, *b),
              "{family}: the marks of {text:?} collide: {a:?} {b:?}"
            );
          }
        }
      }
    }
  }

  assert!(pairs > 50, "the cases stack marks ({pairs} pairs)");
}

#[test]
fn marks_the_font_keeps_apart_stay_where_it_puts_them() {
  // Vietnamese in Inter, whose `mkmk` stacks its tone marks: every glyph where HarfRust puts it
  // shaping the text whole.
  let text = "Tiếng Việt có nhiều dấu thanh: Nguyễn Thị Hương, phở, người, được";
  let laid = lay(&ParleyEngine::system(), text, None);
  let runs = laid.glyph_runs();
  let run = &runs[0];

  assert_eq!(runs.len(), 1, "one font sets the text");

  let font = harfrust::FontRef::from_index(run.font.data.data(), run.font.index).expect("a font");
  let data = harfrust::ShaperData::new(&font);
  let shaper = data.shaper(&font).build();
  let mut buffer = harfrust::UnicodeBuffer::new();

  buffer.push_str(text);
  buffer.guess_segment_properties();

  let shaped = shaper.shape(
    buffer,
    harfrust::ShapeOptions::new().point_size(Some(run.size)),
  );
  let scale = run.size / f32::from(font.head().expect("a head").units_per_em());
  let baseline = laid.first_baseline();

  assert_eq!(shaped.glyph_infos().len(), run.glyphs.len());

  for ((info, position), glyph) in shaped
    .glyph_infos()
    .iter()
    .zip(shaped.glyph_positions())
    .zip(&run.glyphs)
  {
    assert_eq!(info.glyph_id, glyph.id);

    #[expect(clippy::cast_precision_loss, reason = "font units")]
    let y = baseline - position.y_offset as f32 * scale;

    assert!(
      (glyph.y - y).abs() < 0.01,
      "glyph {} moved from {y} to {}",
      glyph.id,
      glyph.y
    );
  }
}

#[test]
fn thai_lao_khmer_and_burmese_break_between_dictionary_words() {
  let engine = ParleyEngine::system();
  let words = icu_segmenter::WordSegmenter::new_dictionary(
    icu_segmenter::options::WordBreakInvariantOptions::default(),
  );

  for text in [THAI, LAO, KHMER, BURMESE] {
    let boundaries: Vec<usize> = words.segment_str(text).collect();
    let paragraph = engine.paragraph(
      &AttributedText::plain(text, RunStyle::new(CORPUS)),
      &ParagraphStyle::default(),
    );
    let widths = paragraph.intrinsic();
    let laid = paragraph.layout(Some(160.));

    assert!(
      widths.min < 0.4 * widths.max,
      "{text}: words break where no blank is: {widths:?}"
    );
    assert!(laid.line_count() > 2);

    for line in laid.lines().iter().skip(1) {
      assert!(
        boundaries.contains(&line.range.start),
        "{text}: a line starts between dictionary words: {:?}",
        lines(&laid, text)
      );
    }
  }
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
    COMBINING,
    "مرحبا بالعالم هذا نص عربي طويل للالتفاف مرحبا بالعالم",
    "Emoji 👍🏽 👨‍👩‍👧 🇰🇷 😀 ❤️ ☺ ☺️ office affine ffi -> != == www",
    "Item number 1, a row of the list. Item number 2, a row of the list.",
    "Iİıi fi fl Tİ İstanbul",
  ];
  let cold = ParleyEngine::system();
  let cached = ParleyEngine::system();

  cold.set_shape_cache(0);

  for round in 0..2 {
    for text in texts {
      // Plain, and in runs of other sizes, weights, slants and a locale, so words meet the
      // cache under every key.
      let mut styled = vec![];

      for (index, (at, word)) in text.split_word_bound_indices().enumerate() {
        let style = match index % 4 {
          0 => RunStyle::new(CORPUS + 3.),
          1 => RunStyle::new(CORPUS).with_weight(skene_foundation::FontWeight::BOLD),
          2 => RunStyle {
            slant: skene_foundation::value::FontStyle::Italic,
            ..RunStyle::new(CORPUS)
          },
          _ => RunStyle::new(CORPUS),
        };

        styled.push(TextRun {
          range: at..at + word.len(),
          style,
        });
      }

      for attributed in [
        AttributedText::plain(text, RunStyle::new(CORPUS)),
        AttributedText::new(text, RunStyle::new(CORPUS), styled),
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

/// DejaVu Sans, which joins Arabic, as an engine's only typeface.
fn dejavu() -> ParleyEngine {
  let data = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
    .expect("DejaVu Sans is installed");
  let engine = ParleyEngine::with_typefaces(&[Typeface {
    family: "DejaVu Sans",
    data: Box::leak(data.into_boxed_slice()),
  }]);

  engine.set_shape_cache(0);
  engine
}

/// The glyphs of `text`, set in `engine`, in order of the bytes they draw.
fn glyph_ids(engine: &ParleyEngine, text: &str) -> Vec<(usize, u32)> {
  let style = RunStyle::new(CORPUS).with_family(skene_foundation::value::FontFamily::Named(
    "DejaVu Sans".into(),
  ));
  let laid = engine
    .paragraph(
      &AttributedText::plain(text, style),
      &ParagraphStyle::default(),
    )
    .layout(None);
  let mut glyphs: Vec<(usize, u32)> = laid
    .glyph_runs()
    .iter()
    .flat_map(|run| &run.glyphs)
    .map(|glyph| (glyph.at, glyph.id))
    .collect();

  glyphs.sort_by_key(|(at, _)| *at);
  glyphs
}

/// The glyphs the Arabic letters beh (U+0628) of `text` are drawn as, set in `engine`: those of
/// `forms`, its four, at the bytes the letters are.
fn behs(engine: &ParleyEngine, text: &str, forms: &[u32]) -> Vec<u32> {
  let letters: Vec<usize> = text.match_indices('\u{628}').map(|(at, _)| at).collect();

  glyph_ids(engine, text)
    .into_iter()
    .filter(|(at, id)| letters.contains(at) && forms.contains(id))
    .map(|(_, id)| id)
    .collect()
}

#[test]
fn the_shape_caches_context_reads_joining_as_harfrust_does() {
  // The cache keeps the text around a word by what HarfRust reads of it, its joining type:
  // every character ICU calls non-joining must join nothing in HarfRust, and every one ICU
  // calls transparent must let its neighbours join across it.
  let joining = const { CodePointMapData::<JoiningType>::new() };
  let general = const { CodePointMapData::<GeneralCategory>::new() };
  let engine = dejavu();
  // Its initial, medial and final forms, and its isolated one.
  let mut forms: Vec<u32> = glyph_ids(&engine, "\u{628}\u{628}\u{628}")
    .into_iter()
    .chain(glyph_ids(&engine, "\u{628}"))
    .map(|(_, id)| id)
    .collect();

  forms.sort_unstable();
  forms.dedup();
  assert_eq!(forms.len(), 4, "the font joins beh: {forms:?}");

  let forms = forms.as_slice();
  let alone_after = behs(&engine, "\u{628} ", forms);
  let alone_before = behs(&engine, " \u{628}", forms);
  let joined = behs(&engine, "\u{628}\u{628}", forms);
  let mut checked = 0;

  for ch in (0..0x3_0000).filter_map(char::from_u32) {
    let category = general.get(ch);

    if matches!(
      category,
      GeneralCategory::Unassigned | GeneralCategory::PrivateUse | GeneralCategory::Surrogate
    ) || ch.is_control()
    {
      continue;
    }

    match joining.get(ch) {
      JoiningType::NonJoining => {
        checked += 1;
        assert_eq!(
          behs(&engine, &format!("\u{628}{ch}"), forms),
          alone_after,
          "{ch:?} after"
        );
        assert_eq!(
          behs(&engine, &format!("{ch}\u{628}"), forms),
          alone_before,
          "{ch:?} before"
        );
      }
      JoiningType::Transparent => {
        checked += 1;
        assert_eq!(
          behs(&engine, &format!("\u{628}{ch}\u{628}"), forms),
          joined,
          "{ch:?} between"
        );
      }
      _ => {}
    }
  }

  assert!(checked > 100_000, "{checked} characters checked");
}

#[test]
fn a_word_a_style_splits_is_cached_by_what_joins_it() {
  // A word is cached alone, then met again joined to a letter before it and after it that a
  // size sets apart, so it takes its medial and final forms and not the ones it had alone.
  let cold = ParleyEngine::system();
  let cached = ParleyEngine::system();

  cold.set_shape_cache(0);

  for (text, larger) in [
    ("\u{639}\u{627}\u{644}\u{645}", 0..0),
    ("\u{628}\u{627}\u{644}\u{639}\u{627}\u{644}\u{645}", 0..6),
    ("\u{639}\u{627}\u{644}\u{645}\u{628}", 8..10),
  ] {
    let attributed = AttributedText::new(
      text,
      RunStyle::new(CORPUS),
      vec![TextRun {
        range: larger,
        style: RunStyle::new(CORPUS + 3.),
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

/// Text across spaces each of `FAMILIES` does something across: Inter's contextual alternates
/// (an `x` between numbers, arrows, a colon between numbers, raised punctuation), the `kern`
/// pairs Liberation's and FreeSans's fonts begin with a space, Carlito's Greek capitals with
/// tonos; and words between, in each of which a run splits.
const ACROSS_A_SPACE: &[&str] = &[
  "Order 3 x 4 = 12 and 1 - 2 : 3 or 10 :30 then (A) [B] {C} @ home",
  "a -> b <- c => d <= e != f >= g -- h \u{2014} i \u{2192} j",
  "A Taste Of Yesterday Very Well, Ανω Ταυ Υψος Λαος Δελτα",
  "1 2 12 21 1 1 2 2 11 22",
  "Ά Έ Ή Ί Ό Ύ Ώ Ἀθῆναι",
  "office affine, the quick brown fox; the quick brown fox",
];

#[test]
fn the_shape_cache_keeps_what_a_font_does_across_a_space() {
  let cold = ParleyEngine::system();
  let cached = ParleyEngine::system();

  cold.set_shape_cache(0);

  for family in FAMILIES {
    let style = RunStyle::new(CORPUS)
      .with_family(skene_foundation::value::FontFamily::Named((*family).into()));

    for round in 0..2 {
      for text in ACROSS_A_SPACE {
        let attributed = AttributedText::plain(*text, style.clone());
        let paragraph = ParagraphStyle::default();
        let (a, b) = (
          cold.paragraph(&attributed, &paragraph).layout(None),
          cached.paragraph(&attributed, &paragraph).layout(None),
        );

        assert_eq!(glyphs(&a), glyphs(&b), "{family}, round {round}: {text}");
      }
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
  // A run holding an `x` or punctuation, which Inter's contextual alternates read across a
  // space, is shaped whole; one without splits into words.
  let engine = ParleyEngine::system();
  let style =
    RunStyle::new(CORPUS).with_family(skene_foundation::value::FontFamily::Named("Inter".into()));
  let shape = |text: &str| {
    let before = engine.shape_cache_stats();
    let _ = engine.paragraph(
      &AttributedText::plain(text, style.clone()),
      &ParagraphStyle::default(),
    );
    let after = engine.shape_cache_stats();

    (after.0 - before.0, after.1 - before.1)
  };

  assert_eq!(shape("hello world"), (0, 2), "two words shaped");
  assert_eq!(shape("hello there"), (1, 1), "a word found, a word shaped");
  assert_eq!(shape("hello 3 x 4"), (0, 1), "a run with an x is one word");
  assert_eq!(shape("hello 3 x 4"), (1, 0), "found whole");
}

const SOFT_HYPHENS_HEBREW: &str =
  "מי\u{AD}לה ארו\u{AD}כה מאו\u{AD}ד בע\u{AD}ב\u{AD}רית ובש\u{AD}פות אח\u{AD}רות";

/// `text` laid out by `engine` at the corpus's size in a paragraph running right to left.
fn lay_rtl(engine: &ParleyEngine, text: &str) -> skene_parley::ParleyParagraph {
  engine.paragraph(
    &AttributedText::plain(text, RunStyle::new(CORPUS)),
    &ParagraphStyle {
      direction: skene_foundation::text::BaseDirection::RightToLeft,
      ..ParagraphStyle::default()
    },
  )
}

#[test]
fn a_soft_hyphen_breaks_the_least_width_in_right_to_left_text_too() {
  let engine = ParleyEngine::system();

  for (text, rtl) in [(SOFT_HYPHENS, false), (SOFT_HYPHENS_HEBREW, true)] {
    let paragraph = if rtl {
      lay_rtl(&engine, text)
    } else {
      engine.paragraph(
        &AttributedText::plain(text, RunStyle::new(CORPUS)),
        &ParagraphStyle::default(),
      )
    };
    let least = paragraph.intrinsic().min;
    let widest_word = text
      .split(' ')
      .map(|word| paragraph_width(&engine, &word.replace('\u{AD}', ""), rtl))
      .fold(0., f32::max);

    eprintln!("{text:?}: least {least}, widest word {widest_word}");
    assert!(
      least < widest_word - 1.,
      "the least width breaks after soft hyphens: {least} against the widest word's {widest_word}"
    );
  }
}

/// The width `text` lays out at on one line.
fn paragraph_width(engine: &ParleyEngine, text: &str, rtl: bool) -> f32 {
  let paragraph = if rtl {
    lay_rtl(engine, text)
  } else {
    engine.paragraph(
      &AttributedText::plain(text, RunStyle::new(CORPUS)),
      &ParagraphStyle::default(),
    )
  };

  paragraph.intrinsic().max
}

/// The advance of glyph `id` of `run`'s font at the run's size.
fn advance_of(run: &skene_parley::GlyphRun, id: u32) -> f32 {
  FontRef::from_index(run.font.data.data(), run.font.index)
    .ok()
    .and_then(|font| {
      font
        .glyph_metrics(Size::new(run.size), LocationRef::default())
        .advance_width(skrifa::GlyphId::new(id))
    })
    .unwrap_or(0.)
}

#[test]
fn a_soft_hyphen_in_right_to_left_text_draws_its_hyphen_at_the_lines_end() {
  let engine = ParleyEngine::system();
  let long = "אב גד הו זח אל\u{AD}טר\u{AD}נ\u{AD}טי\u{AD}בי\u{AD}ות טל ים";
  let mixed =
    "Some text then מי\u{AD}לה ארו\u{AD}כה and then extra\u{AD}or\u{AD}di\u{AD}nary words";
  let mut checked = 0;

  for text in [SOFT_HYPHENS_HEBREW, long, mixed] {
    let paragraph = lay_rtl(&engine, text);

    for width in (20..=240).step_by(4) {
      #[expect(clippy::cast_precision_loss, reason = "small widths")]
      let width = width as f32;
      let laid = paragraph.layout(Some(width));
      let runs = laid.glyph_runs();
      // Each glyph's left edge, advance and byte, with whether it is a hyphen.
      let glyphs: Vec<(f32, f32, usize, bool)> = runs
        .iter()
        .flat_map(|run| {
          let hyphens = [glyph_in(run, '\u{2010}'), glyph_in(run, '-')];

          run.glyphs.iter().map(move |glyph| {
            (
              glyph.x,
              advance_of(run, glyph.id),
              glyph.at,
              hyphens
                .iter()
                .flatten()
                .any(|(hyphen, _)| *hyphen == glyph.id),
            )
          })
        })
        .collect();

      for line in laid.lines() {
        let shown = &text[line.range.clone()];

        if !shown.ends_with('\u{AD}') {
          continue;
        }

        checked += 1;

        let at = line.range.end - '\u{AD}'.len_utf8();
        let (x, advance, _, hyphen) = *glyphs
          .iter()
          .find(|(_, _, byte, _)| *byte == at)
          .expect("the soft hyphen has a glyph");

        assert!(hyphen, "at {width}, {shown:?} draws a hyphen");

        // The hyphen stands apart from the line's other glyphs, left of them all where a Hebrew
        // word ends the line.
        let hebrew = shown
          .trim_end_matches('\u{AD}')
          .chars()
          .next_back()
          .is_some_and(|ch| ('\u{590}'..='\u{5FF}').contains(&ch));

        for (other_x, other_advance, byte, _) in &glyphs {
          if !line.range.contains(byte) || *byte == at || text[*byte..].starts_with(' ') {
            continue;
          }

          assert!(
            x + advance <= other_x + 0.5 || other_x + other_advance <= x + 0.5,
            "at {width}, {shown:?}: the hyphen at {x}+{advance} overlaps the glyph of byte \
             {byte} at {other_x}+{other_advance}"
          );
          assert!(
            !hebrew || x <= *other_x,
            "at {width}, {shown:?}: the hyphen at {x} is right of the glyph of byte {byte} at \
             {other_x}"
          );
        }
      }
    }
  }

  assert!(checked > 40, "lines break after soft hyphens: {checked}");
}
