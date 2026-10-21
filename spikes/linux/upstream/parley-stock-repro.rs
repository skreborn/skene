//! Reproductions of the gaps Skene's patched Parley closes, run against crates.io parley 0.12.0.

use parley::{
  FontContext, FontFamily, Layout, LayoutContext, OverflowWrap, PositionedLayoutItem,
  StyleProperty,
};

fn lay(
  fonts: &mut FontContext,
  layouts: &mut LayoutContext<()>,
  text: &str,
  family: &str,
  size: f32,
  width: Option<f32>,
  ranged: &[(std::ops::Range<usize>, f32)],
) -> Layout<()> {
  let mut builder = layouts.ranged_builder(fonts, text, 1., true);

  builder.push_default(StyleProperty::FontFamily(FontFamily::named(family)));
  builder.push_default(StyleProperty::FontSize(size));
  builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::Anywhere));

  for (range, size) in ranged {
    builder.push(StyleProperty::FontSize(*size), range.clone());
  }

  let mut layout = builder.build(text);

  layout.break_all_lines(width);
  layout
}

fn lines<'a>(layout: &Layout<()>, text: &'a str) -> Vec<&'a str> {
  layout.lines().map(|line| &text[line.text_range()]).collect()
}

/// Each line's clusters in visual order: (text, x, advance).
fn clusters<'a>(layout: &Layout<()>, text: &'a str) -> Vec<Vec<(&'a str, f32, f32)>> {
  layout
    .lines()
    .map(|line| {
      let mut out = Vec::new();

      for item in line.items() {
        if let PositionedLayoutItem::GlyphRun(run) = item {
          let mut x = run.offset();

          for cluster in run.run().visual_clusters() {
            out.push((&text[cluster.text_range()], x, cluster.advance()));
            x += cluster.advance();
          }
        }
      }

      out
    })
    .collect()
}

fn main() {
  let mut fonts = FontContext::new();
  let mut layouts = LayoutContext::new();

  println!("== 1. a break inside a cluster");
  let burmese = "မြန်မာဘာသာစကားသည် မြန်မာနိုင်ငံ၏ ရုံးသုံးဘာသာစကား ဖြစ်သည်။";
  let layout = lay(&mut fonts, &mut layouts, burmese, "Unifont", 17., Some(10.), &[]);
  println!("Burmese in Unifont at 17 px within 10 px, overflow-wrap anywhere: {:?}", lines(&layout, burmese));
  let khmer = "ភាសាខ្មែរគឺជាភាសាផ្លូវការ";
  let layout = lay(&mut fonts, &mut layouts, khmer, "Unifont", 17., Some(10.), &[]);
  println!("Khmer in Unifont at 17 px within 10 px: {:?}", lines(&layout, khmer));
  let acutes = "e\u{301}e\u{301}e\u{301}";
  let larger: Vec<_> = acutes.char_indices().filter(|(_, ch)| *ch == '\u{301}').map(|(at, _)| (at..at + 2, 23.)).collect();
  let layout = lay(&mut fonts, &mut layouts, acutes, "Inter", 17., Some(5.), &larger);
  println!("e + U+0301 thrice in Inter, each acute larger, within 5 px: {:?}", lines(&layout, acutes));

  println!("== 2. UAX #9 L1");
  let bidi = "Hello שלום עולם world";
  let layout = lay(&mut fonts, &mut layouts, bidi, "DejaVu Sans", 17., Some(120.), &[]);
  println!("lines {:?}", lines(&layout, bidi));
  for (index, line) in clusters(&layout, bidi).iter().enumerate() {
    println!("line {index} width {:.1}: {:?}", layout.lines().nth(index).unwrap().metrics().advance, line.iter().map(|(t, x, a)| format!("{t:?}@{x:.1}+{a:.1}")).collect::<Vec<_>>());
  }

  println!("== 3. soft hyphen");
  let soft = "Incom\u{AD}pre\u{AD}hen\u{AD}si\u{AD}bil\u{AD}i\u{AD}ties every\u{AD}where";
  let layout = lay(&mut fonts, &mut layouts, soft, "Inter", 17., Some(90.), &[]);
  println!("lines {:?}", lines(&layout, soft));
  for (index, line) in clusters(&layout, soft).iter().enumerate() {
    let last = line.iter().rev().find(|(t, _, _)| !t.trim().is_empty() || *t == "\u{AD}");
    println!("line {index}: last cluster {last:?}");
  }

  println!("== 4. tabs");
  let tabs = "col1\tcol2\tcol3\n1\t22\t333";
  let layout = lay(&mut fonts, &mut layouts, tabs, "Inter", 17., None, &[]);
  for (index, line) in clusters(&layout, tabs).iter().enumerate() {
    println!("line {index}: {:?}", line.iter().map(|(t, x, a)| format!("{t:?}@{x:.1}+{a:.1}")).collect::<Vec<_>>());
  }

  println!("== 5. stacked marks");
  for text in ["Z\u{353}\u{351}\u{352}", "a\u{301}\u{302}\u{303}"] {
    let layout = lay(&mut fonts, &mut layouts, text, "Inter", 17., None, &[]);
    for line in layout.lines() {
      for item in line.items() {
        if let PositionedLayoutItem::GlyphRun(run) = item {
          println!("{text:?}: {:?}", run.positioned_glyphs().map(|g| (g.id, (g.x * 10.).round() / 10., (g.y * 10.).round() / 10.)).collect::<Vec<_>>());
        }
      }
    }
  }
}
