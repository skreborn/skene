//! The Linux text verification: the platform-text cases laid out by Skene's Parley engine over
//! fontconfig and drawn by FreeType, measured against what Pango does with the same text.
//!
//! ```text
//! verify shots OUT      every case: metrics, lines and fonts, PNGs (FreeType masks, vello outlines)
//! verify emoji PNG      which emoji clusters draw in color, in our image and in Pango's PNG
//! verify settings OUT [AA,HINTING,STYLE]
//!                       the rendering settings fontconfig gives Inter (over the desktop's
//!                       XSETTINGS-style values where given), and a sample's coverage
//! verify ink            measured size against drawn ink, per case
//! verify bench          1000 labels: build, measure, lay out under taffy, draw
//! ```

use std::{fmt::Write as _, rc::Rc, time::Instant};

use skene_foundation::{
  Color,
  layout::{Axis, ContainerLayout, LayoutHost, Measurement, Rect, ResolvedLayout},
  seam::node::LayoutKind,
  text::{
    AttributedText, Laid as _, MeasureText, Paragraph as _, ParagraphStyle, RunStyle,
    TextEngine as _, TextLeaf,
  },
  value::{GridTrack, Tracks},
};
use skene_parley::{
  ParleyEngine, ParleyLaid, ParleyParagraph,
  render::{MaskPainter, paint_outlines},
};
use vello_cpu::{Pixmap, RenderContext, Resources, kurbo, peniko};

/// The size the cases are set at, in pixels.
const SIZE: f32 = 13.;

/// The platform-text cases: a name, the text, and the width it wraps at (none where zero).
const CASES: &[(&str, &str, f32)] = &[
  ("latin", "The quick brown fox jumps over the lazy dog", 0.),
  ("emoji", "Emoji 👍🏽 👨‍👩‍👧 🇰🇷 😀 ❤️ ☺ ☺️", 0.),
  (
    "cjk",
    "日本語の文章を折り返します。中文文本换行。한국어 문장입니다.",
    110.,
  ),
  ("thai", "ภาษาไทยเป็นภาษาที่ไม่มีการเว้นวรรคระหว่างคำ", 90.),
  ("khmer", "ភាសាខ្មែរគឺជាភាសាផ្លូវការរបស់ប្រទេសកម្ពុជា", 90.),
  (
    "hyphen",
    "Donaudampfschifffahrtsgesellschaft internationalization",
    110.,
  ),
  (
    "softhyphen",
    "Incom\u{AD}pre\u{AD}hen\u{AD}si\u{AD}bil\u{AD}i\u{AD}ties every\u{AD}where",
    90.,
  ),
  ("bidi", "Hello שלום עולם world مرحبا بالعالم 123", 0.),
  ("arabic", "مرحبا بالعالم هذا نص عربي طويل للالتفاف", 110.),
  (
    "ellipsis",
    "A long label that must fit in two lines and end with an ellipsis when it overflows",
    120.,
  ),
];

fn main() {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  let engine = ParleyEngine::system();

  match arguments.first().map(String::as_str) {
    Some("shots") => shots(&engine, &arguments[1]),
    Some("emoji") => emoji(&engine, &arguments[1]),
    Some("settings") => settings(&engine, &arguments[1], arguments.get(2).map(String::as_str)),
    Some("follow") => follow(&engine),
    Some("ink") => ink(&engine),
    Some("bench") => bench(&engine),
    _ => panic!("verify shots|emoji|settings|follow|ink|bench"),
  }
}

fn style_for(name: &str) -> ParagraphStyle {
  if name == "ellipsis" {
    ParagraphStyle::default().with_max_lines(2)
  } else {
    ParagraphStyle::default()
  }
}

fn lay_out(engine: &ParleyEngine, name: &str, text: &str, width: f32) -> ParleyLaid {
  let paragraph = engine.paragraph(
    &AttributedText::plain(text, RunStyle::new(SIZE)),
    &style_for(name),
  );

  paragraph.layout((width > 0.).then_some(width))
}

/// A white canvas with `laid` drawn at (4, 4) by FreeType's masks.
fn draw_masks(
  engine: &ParleyEngine,
  painter: &mut MaskPainter,
  laid: &ParleyLaid,
  width: u16,
  height: u16,
) -> Pixmap {
  let mut pixmap = canvas(width, height);
  let mut fallback = engine.fallback();

  painter.paint(&mut pixmap, laid, (4., 4.), fallback.as_deref_mut());
  pixmap
}

/// A white canvas with `laid` drawn at (4, 4) as vello outlines.
fn draw_outlines(laid: &ParleyLaid, width: u16, height: u16) -> Pixmap {
  let mut context = RenderContext::new(width, height);
  let mut resources = Resources::new();
  let mut pixmap = Pixmap::new(width, height);

  context.set_paint(peniko::Color::WHITE);
  context.fill_rect(&kurbo::Rect::new(
    0.,
    0.,
    f64::from(width),
    f64::from(height),
  ));
  paint_outlines(&mut context, &mut resources, laid, (4., 4.));
  context.flush();
  context.render(&mut pixmap, &mut resources);
  pixmap
}

fn canvas(width: u16, height: u16) -> Pixmap {
  let mut pixmap = Pixmap::new(width, height);

  pixmap.data_as_u8_slice_mut().fill(255);
  pixmap
}

fn size_of(laid: &ParleyLaid) -> (u16, u16) {
  let size = laid.size();

  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "image sizes"
  )]
  (
    (size.width.ceil() as u16 + 8).max(16),
    (size.height.ceil() as u16 + 8).max(16),
  )
}

fn save(pixmap: Pixmap, path: &str) {
  std::fs::write(path, pixmap.into_png().expect("encodes")).expect("writes");
}

/// `text` as a JSON string.
fn quoted(text: &str) -> String {
  let mut quoted = String::from("\"");

  for character in text.chars() {
    match character {
      '"' => quoted.push_str("\\\""),
      '\\' => quoted.push_str("\\\\"),
      character if character.is_control() => {
        let _ = write!(quoted, "\\u{:04x}", u32::from(character));
      }
      character => quoted.push(character),
    }
  }

  quoted.push('"');
  quoted
}

fn shots(engine: &ParleyEngine, out: &str) {
  std::fs::create_dir_all(out).expect("creates");

  let mut painter = MaskPainter::new().expect("FreeType loads");

  for (name, text, width) in CASES {
    let measured = lay_out(engine, name, text, *width);
    let laid = lay_out(engine, name, text, measured.size().width);
    let paragraph = engine.paragraph(
      &AttributedText::plain(*text, RunStyle::new(SIZE)),
      &style_for(name),
    );
    let intrinsic = paragraph.intrinsic();
    let size = laid.size();
    let mut report = String::new();
    let _ = writeln!(report, "== {name} width {width}");
    let _ = writeln!(
      report,
      "  skene size {:.1}x{:.1} baseline {:.2} lines {} truncated {} minmax {:.1}/{:.1}",
      size.width,
      size.height,
      laid.first_baseline(),
      laid.line_count(),
      laid.truncated(),
      intrinsic.min,
      intrinsic.max
    );
    let lines: Vec<String> = laid
      .lines()
      .iter()
      .map(|line| format!("[{}]", &text[line.range.clone()]))
      .collect();
    let _ = writeln!(report, "    lines {}", lines.join(" "));
    let runs: Vec<String> = laid
      .run_families()
      .into_iter()
      .map(|(range, family)| format!("{}<{family}>", &text[range]))
      .collect();
    let _ = writeln!(report, "    runs {}", runs.join(" "));
    let marks = laid.glyph_runs().len() - laid.run_families().len();
    let _ = writeln!(report, "    marks {marks}");

    print!("{report}");

    let json = format!(
      "{{\"width\": {:.2}, \"height\": {:.2}, \"lines\": [{}], \"runs\": [{}]}}\n",
      size.width,
      size.height,
      laid
        .lines()
        .iter()
        .map(|line| quoted(&text[line.range.clone()]))
        .collect::<Vec<_>>()
        .join(", "),
      laid
        .run_families()
        .iter()
        .map(|(range, family)| format!("[{}, {}, {}]", range.start, range.end, quoted(family)))
        .collect::<Vec<_>>()
        .join(", ")
    );

    std::fs::write(format!("{out}/{name}-skene.json"), json).expect("writes");

    let (w, h) = size_of(&laid);

    save(
      draw_masks(engine, &mut painter, &laid, w, h),
      &format!("{out}/{name}-skene.png"),
    );
    save(
      draw_outlines(&laid, w, h),
      &format!("{out}/{name}-skene-outlines.png"),
    );
  }
}

/// Whether a pixel is colored rather than gray.
fn chroma(pixel: [u8; 4]) -> bool {
  let (max, min) = (
    pixel[..3].iter().max().copied().unwrap_or(0),
    pixel[..3].iter().min().copied().unwrap_or(0),
  );

  max - min > 60
}

fn emoji(engine: &ParleyEngine, path: &str) {
  let (_, text, _) = CASES[1];
  let laid = lay_out(engine, "emoji", text, 0.);
  let (w, h) = size_of(&laid);
  let mut painter = MaskPainter::new().expect("FreeType loads");
  let pixmap = draw_masks(engine, &mut painter, &laid, w, h);
  let outlines = draw_outlines(&laid, w, h);
  let colored = |pixmap: &Pixmap, range: std::ops::Range<usize>| {
    let caret = |at: usize| {
      laid
        .caret(skene_foundation::text::Position::new(at))
        .primary
    };
    let (start, end) = (caret(range.start), caret(range.end));

    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "pixels"
    )]
    let (x0, x1, y0, y1) = (
      start.x.min(end.x) as u16 + 4,
      start.x.max(end.x).ceil() as u16 + 4,
      start.y as u16 + 4,
      (start.y + start.height).ceil() as u16 + 4,
    );

    (y0..y1.min(h)).any(|y| {
      (x0..x1.min(w)).any(|x| {
        let pixel = pixmap.sample(x, y);

        chroma([pixel.r, pixel.g, pixel.b, pixel.a])
      })
    })
  };
  let mut start = text.find(' ').unwrap_or(0) + 1;
  let mut masks = 0;
  let mut vello = 0;

  for cluster in text[start..].split(' ') {
    let range = start..start + cluster.len();
    let (in_masks, in_outlines) = (
      colored(&pixmap, range.clone()),
      colored(&outlines, range.clone()),
    );
    let bitmap = laid.glyph_runs().iter().any(|run| {
      let settings = painter.settings_for(&run.font, run.size, engine.fallback().as_deref_mut());

      run
        .glyphs
        .iter()
        .filter(|glyph| range.contains(&glyph.at))
        .any(|glyph| {
          painter
            .rasterizer_mut()
            .mask(&run.font, &run.coords, glyph.id, run.size, 0, &settings)
            .is_some_and(|mask| matches!(mask.pixels, skene_parley::linux::MaskPixels::Color(_)))
        })
    });
    let family = laid
      .run_families()
      .into_iter()
      .find(|(range, _)| text[range.clone()].contains(cluster))
      .map(|(_, family)| family)
      .unwrap_or_default();

    masks += usize::from(bitmap);
    vello += usize::from(in_outlines);
    println!(
      "  {cluster} font={family} color-glyph={bitmap} chromatic-masks={in_masks} chromatic-outlines={in_outlines}"
    );
    start += cluster.len() + 1;
  }

  println!("emoji drawn from a color glyph: {masks}/7; chromatic under vello outlines: {vello}/7");
  save(pixmap, path);
}

fn settings(engine: &ParleyEngine, out: &str, desktop: Option<&str>) {
  if let Some(desktop) = desktop {
    let values: Vec<i32> = desktop
      .split(',')
      .map(|value| value.parse().expect("a number"))
      .collect();

    engine
      .fallback()
      .expect("fontconfig loads")
      .fontconfig()
      .set_desktop(skene_parley::linux::DesktopSettings {
        antialias: Some(values[0] != 0),
        hinting: Some(values[1] != 0),
        hint_style: u8::try_from(values[2]).ok(),
      });
  }

  let (_, text, _) = CASES[0];
  let laid = lay_out(engine, "latin", text, 0.);
  let (w, h) = size_of(&laid);
  let mut painter = MaskPainter::new().expect("FreeType loads");
  let pixmap = draw_masks(engine, &mut painter, &laid, w, h);
  let font = laid.glyph_runs()[0].font.clone();
  let settings = painter.settings_for(
    &font,
    laid.glyph_runs()[0].size,
    engine.fallback().as_deref_mut(),
  );
  let pixels = pixmap.data_as_u8_slice();
  let partial = pixels
    .as_chunks::<4>()
    .0
    .iter()
    .filter(|pixel| pixel[0] > 0 && pixel[0] < 255)
    .count();
  let checksum = pixels.iter().fold(0_u64, |hash, byte| {
    hash
      .wrapping_mul(1_099_511_628_211)
      .wrapping_add(u64::from(*byte))
  });

  println!(
    "settings antialias={} hinting={} hintstyle={} autohint={} partial-pixels={partial} checksum={checksum:016x}",
    settings.antialias, settings.hinting, settings.hint_style, settings.autohint
  );
  save(pixmap, &format!("{out}.png"));
}

/// How many of `pixmap`'s pixels are neither white nor black: none where glyphs are drawn
/// without antialiasing.
fn partial(pixmap: &Pixmap) -> usize {
  pixmap
    .data_as_u8_slice()
    .as_chunks::<4>()
    .0
    .iter()
    .filter(|pixel| pixel[0] > 0 && pixel[0] < 255)
    .count()
}

/// The rendering settings following the desktop and the request: one painter drawing the
/// same text before and after the desktop's settings change, and a regular and a bold text
/// at 10 and 20 pixels asked of fontconfig with their size and weight.
fn follow(engine: &ParleyEngine) {
  let text = "The quick brown fox";
  let lay = |style: RunStyle| {
    engine
      .paragraph(
        &AttributedText::plain(text, style),
        &ParagraphStyle::default(),
      )
      .layout(None)
  };
  let mut painter = MaskPainter::new().expect("FreeType loads");
  let draw = |painter: &mut MaskPainter, laid: &ParleyLaid| {
    let (w, h) = size_of(laid);
    let pixmap = draw_masks(engine, painter, laid, w, h);
    let run = &laid.glyph_runs()[0];
    let settings = painter.settings_for(&run.font, run.size, engine.fallback().as_deref_mut());

    (settings.antialias, partial(&pixmap))
  };
  let set = |antialias: bool| {
    engine
      .fallback()
      .expect("fontconfig loads")
      .fontconfig()
      .set_desktop(skene_parley::linux::DesktopSettings {
        antialias: Some(antialias),
        hinting: Some(true),
        hint_style: Some(1),
      });
  };
  let regular = lay(RunStyle::new(SIZE));

  set(true);

  let before = draw(&mut painter, &regular);

  set(false);

  let after = draw(&mut painter, &regular);

  println!(
    "desktop antialias on then off, one painter: antialias {} partial-pixels {} then antialias {} partial-pixels {}",
    before.0, before.1, after.0, after.1
  );

  set(true);

  for (name, style) in [
    ("regular 10px", RunStyle::new(10.)),
    ("regular 20px", RunStyle::new(20.)),
    (
      "bold 20px",
      RunStyle::new(20.).with_weight(skene_foundation::FontWeight::BOLD),
    ),
  ] {
    let (antialias, partial) = draw(&mut painter, &lay(style));

    println!("{name}: antialias {antialias} partial-pixels {partial}");
  }
}

fn ink(engine: &ParleyEngine) {
  let mut painter = MaskPainter::new().expect("FreeType loads");
  let mut worst: f32 = 0.;

  for (name, text, width) in CASES {
    // Measured within the case's width, then drawn as a realization draws it: laid out at the
    // width the layout engine gives the box, the width the text used.
    let measured = lay_out(engine, name, text, *width);
    let laid = lay_out(engine, name, text, measured.size().width);
    let size = laid.size();
    let (w, h) = size_of(&laid);
    let pixmap = draw_masks(engine, &mut painter, &laid, w, h);

    assert_eq!(
      laid.line_count(),
      measured.line_count(),
      "the used width keeps the lines"
    );
    let (mut left, mut right, mut top, mut bottom) = (u16::MAX, 0, u16::MAX, 0);

    for y in 0..h {
      for x in 0..w {
        let pixel = pixmap.sample(x, y);

        if pixel.r < 250 || pixel.g < 250 || pixel.b < 250 {
          left = left.min(x);
          right = right.max(x + 1);
          top = top.min(y);
          bottom = bottom.max(y + 1);
        }
      }
    }

    let ink = (
      f32::from(left) - 4.,
      f32::from(right) - 4.,
      f32::from(top) - 4.,
      f32::from(bottom) - 4.,
    );
    let off = (ink.1 - size.width).abs();

    worst = worst.max(off);
    println!(
      "  {name}: measured {:.1}x{:.1}, ink x {:.0}..{:.0} y {:.0}..{:.0}, right edge off by {off:.1}",
      size.width, size.height, ink.0, ink.1, ink.2, ink.3
    );
  }

  println!("ink worst right-edge difference {worst:.1}px");
}

fn labels() -> Vec<String> {
  (0..1000)
    .map(|i| match i % 10 {
      0 => format!("設定項目 {i} の説明テキスト"),
      5 => format!("Item {i} 👍 with emoji"),
      _ => format!("Settings item {i}: a short description of what it does"),
    })
    .collect()
}

/// Text leaves under the layout engine.
#[derive(Clone)]
struct Leaf(Rc<TextLeaf<ParleyParagraph>>);

struct Host;

impl LayoutHost for Host {
  type Child = Leaf;

  fn measure(_: &Leaf, _: Axis, _: Option<f32>) -> Measurement {
    unreachable!("text leaves measure through the engine")
  }

  fn constant_height(_: &Leaf) -> bool {
    false
  }

  fn is_visible(_: &Leaf) -> bool {
    true
  }

  fn is_container(_: &Leaf) -> bool {
    false
  }

  fn text(child: &Leaf) -> Option<Rc<dyn MeasureText>> {
    Some(child.0.clone())
  }

  fn allocate(_: &Leaf, _: Rect, _: Rect, _: Option<f32>) {}
}

fn bench(engine: &ParleyEngine) {
  let labels = labels();
  let style = ParagraphStyle::default();
  let attributed: Vec<AttributedText> = labels
    .iter()
    .map(|label| {
      AttributedText::plain(
        label.as_str(),
        RunStyle::new(SIZE).with_paint(skene_foundation::text::PaintStyle::new(Color::BLACK)),
      )
    })
    .collect();

  for round in 0..3 {
    let start = Instant::now();
    let paragraphs: Vec<ParleyParagraph> = attributed
      .iter()
      .map(|text| engine.paragraph(text, &style))
      .collect();
    let widest: f32 = paragraphs
      .iter()
      .map(|paragraph| paragraph.intrinsic().max)
      .sum();
    let built = start.elapsed();
    let start = Instant::now();
    let height: f32 = paragraphs
      .iter()
      .map(|paragraph| paragraph.layout(Some(150.)).size().height)
      .sum();
    let at_150 = start.elapsed();
    let start = Instant::now();
    let leaves: Vec<Leaf> = paragraphs
      .into_iter()
      .map(|paragraph| Leaf(Rc::new(TextLeaf::new(paragraph))))
      .collect();
    let mut grid = ContainerLayout::<Host>::new(
      LayoutKind::Grid,
      ResolvedLayout {
        columns: Some(Tracks(vec![GridTrack::Flex(1.); 4].into())),
        ..ResolvedLayout::default()
      },
    );

    grid.set_children(
      leaves
        .iter()
        .map(|leaf| (leaf.clone(), ResolvedLayout::default()))
        .collect(),
    );

    let measured = grid.measure(Axis::Vertical, Some(700.));

    grid.allocate((700., measured.natural), None);

    let laid_out = start.elapsed();
    let layouts: u32 = leaves.iter().map(|leaf| leaf.0.layouts()).sum();

    println!(
      "round {round}: build+max-content {:.2} ms; layout at 150 {:.2} ms; taffy grid of 1000 text leaves at 700px {:.2} ms ({layouts} engine layouts, height {:.0}); sums {widest:.0}/{height:.0}",
      built.as_secs_f64() * 1e3,
      at_150.as_secs_f64() * 1e3,
      laid_out.as_secs_f64() * 1e3,
      measured.natural
    );
  }

  let laid: Vec<ParleyLaid> = attributed
    .iter()
    .map(|text| engine.paragraph(text, &style).layout(None))
    .collect();
  let mut painter = MaskPainter::new().expect("FreeType loads");
  let mut times = Vec::new();

  for _ in 0..4 {
    let start = Instant::now();
    let mut pixmap = canvas(1600, 2000);
    let mut fallback = engine.fallback();

    for (at, laid) in laid.iter().enumerate() {
      #[expect(clippy::cast_precision_loss, reason = "a grid of labels")]
      let origin = ((at % 4) as f32 * 400., (at / 4) as f32 * 8.);

      painter.paint(&mut pixmap, laid, origin, fallback.as_deref_mut());
    }

    times.push(format!("{:.1}", start.elapsed().as_secs_f64() * 1e3));
  }

  println!(
    "draw 1000 labels FreeType masks into 1600x2000: {:?} ms ({} masks cached)",
    times,
    painter.rasterizer().cached()
  );
}
