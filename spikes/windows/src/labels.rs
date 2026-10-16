//! The first round's 1000 labels (`spike` part, `b.*` lines) with Parley in DirectWrite's place:
//! the same texts, the same taffy tree, the same three-entry cache of immutable layouts per
//! paragraph, then drawn by vello's outlines (B) and through DirectWrite's atlas (C).

use std::{cell::RefCell, path::Path, rc::Rc, time::Instant};

use taffy::{Baselines, LayoutInput, LayoutOutput, Rect as TRect, prelude::*};
use text_parity::{
  bench::plain_case,
  corpus::{Case, Variant},
  fonts::Fonts,
  layout::{Laid, Options as LayoutOptions, ParleyEngine, Shaped},
  render,
};
use windows::Win32::Graphics::DirectWrite::IDWriteFactory4;

use crate::{
  Report,
  faces::Faces,
  paint::{Item, new_context, to_img},
  parity::{Msg, Painters, Res},
};

const WORDS: [&str; 24] = [
  "Settings",
  "Account",
  "Network",
  "Display",
  "Privacy",
  "Updates",
  "Battery",
  "Storage",
  "Sound",
  "Bluetooth",
  "devices",
  "options",
  "advanced",
  "notifications",
  "focus",
  "assist",
  "language",
  "region",
  "日本語",
  "中文",
  "한국어",
  "שלום",
  "café",
  "naïve",
];

fn ms(started: Instant) -> f64 {
  started.elapsed().as_secs_f64() * 1e3
}

#[derive(Default)]
struct Counters {
  measure: std::cell::Cell<u64>,
  created: std::cell::Cell<u64>,
  hits: std::cell::Cell<u64>,
}

/// A Parley paragraph as a taffy leaf: shaped once, broken per width into an immutable layout.
struct Paragraph {
  shaped: Shaped,
  min: f32,
  max: f32,
  cache: RefCell<Vec<(Option<u32>, Rc<Laid>)>>,
}

impl Paragraph {
  fn layout(&self, width: Option<f32>, c: &Counters) -> Rc<Laid> {
    let key = width.map(f32::to_bits);

    if let Some((_, laid)) = self.cache.borrow().iter().find(|(k, _)| *k == key) {
      c.hits.set(c.hits.get() + 1);
      return laid.clone();
    }

    c.created.set(c.created.get() + 1);
    let laid = Rc::new(self.shaped.break_at(width));
    let mut cache = self.cache.borrow_mut();

    if cache.len() == 3 {
      cache.remove(0);
    }

    cache.push((key, laid.clone()));
    laid
  }

  fn measure(&self, input: LayoutInput, c: &Counters) -> LayoutOutput {
    c.measure.set(c.measure.get() + 1);
    let width = match (input.known_dimensions.width, input.available_space.width) {
      (Some(w), _) => Some(w),
      (None, AvailableSpace::MinContent) => Some(self.min),
      (None, AvailableSpace::MaxContent) => None,
      (None, AvailableSpace::Definite(w)) if w >= self.max => None,
      (None, AvailableSpace::Definite(w)) => Some(w),
    };
    let laid = self.layout(width, c);
    let used = match (input.known_dimensions.width, input.available_space.width) {
      (Some(w), _) => w,
      (None, AvailableSpace::MinContent) => self.min,
      (None, AvailableSpace::MaxContent) => self.max,
      (None, AvailableSpace::Definite(w)) => laid.layout.width().min(w),
    };
    let first = laid.layout.get(0).map_or(0.0, |l| l.metrics().baseline);
    let last = laid
      .layout
      .get(laid.layout.len().saturating_sub(1))
      .map_or(0.0, |l| l.metrics().baseline);

    LayoutOutput::from_sizes_and_baselines(
      Size {
        width: used,
        height: input
          .known_dimensions
          .height
          .unwrap_or(laid.layout.height()),
      },
      TRect::zero(),
      Baselines {
        first: Some(first),
        last: Some(last),
      },
    )
  }
}

fn root_style(w: f32) -> Style {
  Style {
    display: Display::Flex,
    flex_direction: FlexDirection::Row,
    flex_wrap: FlexWrap::Wrap,
    align_items: AlignItems::START,
    size: Size {
      width: length(w),
      height: auto(),
    },
    padding: TRect {
      left: length(10.0),
      right: length(10.0),
      top: length(10.0),
      bottom: length(10.0),
    },
    gap: Size {
      width: length(6.0),
      height: length(4.0),
    },
    ..Default::default()
  }
}

fn absolute(
  tree: &TaffyTree<usize>,
  node: NodeId,
  at: (f32, f32),
  out: &mut Vec<(NodeId, [f32; 4])>,
) {
  let l = tree.layout(node).unwrap();
  let x = at.0 + l.location.x;
  let y = at.1 + l.location.y;
  out.push((node, [x, y, l.size.width, l.size.height]));

  for child in tree.children(node).unwrap() {
    absolute(tree, child, (x, y), out);
  }
}

/// The 1000 labels with Parley.
pub fn run(out: &Path, factory: &IDWriteFactory4, r: &mut Report) -> Res<()> {
  let texts: Vec<String> = (0..1000)
    .map(|i| {
      let n = 1 + (i * 7) % 5;
      let mut s = format!("{i}.");

      for k in 0..n {
        s.push(' ');
        s.push_str(WORDS[(i * 13 + k * 5) % WORDS.len()]);
      }

      s
    })
    .collect();
  let cases: Vec<Case> = texts
    .iter()
    .enumerate()
    .map(|(i, t)| plain_case(&format!("label-{i}"), t))
    .collect();
  let started = Instant::now();
  let fonts = Fonts::system();
  let startup = ms(started);
  let mut engine = ParleyEngine::new(fonts, LayoutOptions::default());
  let make = |engine: &mut ParleyEngine| -> (Vec<Paragraph>, f64, f64) {
    let started = Instant::now();
    let mut out = Vec::new();
    let mut first = 0.0;

    for (at, case) in cases.iter().enumerate() {
      let shaped = engine.shape(&Variant {
        case,
        size: 13.0,
        width: None,
      });
      let intrinsic = shaped.intrinsic();
      out.push(Paragraph {
        shaped,
        min: intrinsic.min,
        max: intrinsic.max,
        cache: RefCell::new(Vec::new()),
      });

      if at == 0 {
        first = ms(started);
      }
    }

    (out, first, ms(started))
  };
  let (paragraphs, t_first, t_para) = make(&mut engine);
  let (_, t_first_again, t_para_again) = make(&mut engine);
  let counters = Counters::default();
  let mut tree: TaffyTree<usize> = TaffyTree::new();
  tree.disable_rounding();
  let leaves: Vec<NodeId> = (0..paragraphs.len())
    .map(|i| {
      tree
        .new_leaf_with_context(
          Style {
            max_size: Size {
              width: length(150.0),
              height: auto(),
            },
            ..Default::default()
          },
          i,
        )
        .unwrap()
    })
    .collect();
  let root = tree.new_with_children(root_style(1200.0), &leaves).unwrap();
  let compute = |tree: &mut TaffyTree<usize>| {
    tree
      .compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, ctx, _| {
        ctx.map_or(LayoutOutput::HIDDEN, |i| {
          paragraphs[*i].measure(input, &counters)
        })
      })
      .unwrap();
  };
  let snapshot = |c: &Counters| (c.measure.get(), c.created.get(), c.hits.get());
  let c0 = snapshot(&counters);
  let started = Instant::now();
  compute(&mut tree);
  let t_cold = ms(started);
  let c1 = snapshot(&counters);

  for &leaf in &leaves {
    tree.mark_dirty(leaf).unwrap();
  }

  let started = Instant::now();
  compute(&mut tree);
  let t_warm = ms(started);
  let c2 = snapshot(&counters);
  tree.set_style(root, root_style(1100.0)).unwrap();
  let started = Instant::now();
  compute(&mut tree);
  let t_resize = ms(started);
  let c3 = snapshot(&counters);
  tree.set_style(root, root_style(1200.0)).unwrap();
  compute(&mut tree);

  r.line(format!(
    "labels.parley.fonts: Fonts::system() {startup:.2} ms"
  ));
  r.line(format!(
    "labels.parley.paragraphs: 1000 shaped with intrinsic widths in {t_para:.2} ms (first {t_first:.2} ms); again {t_para_again:.2} ms (first {t_first_again:.2} ms)"
  ));
  r.line(format!(
    "labels.parley.layout.cold: {t_cold:.2} ms; measure calls={} laid created={} cache hits={}",
    c1.0 - c0.0,
    c1.1 - c0.1,
    c1.2 - c0.2
  ));
  r.line(format!(
    "labels.parley.layout.warm_same_width (all leaves dirtied): {t_warm:.2} ms; measure calls={} laid created={} cache hits={}",
    c2.0 - c1.0,
    c2.1 - c1.1,
    c2.2 - c1.2
  ));
  r.line(format!(
    "labels.parley.layout.container_resize_1200_to_1100: {t_resize:.2} ms; measure calls={} laid created={} cache hits={}",
    c3.0 - c2.0,
    c3.1 - c2.1,
    c3.2 - c2.2
  ));

  let widths = [90.0f32, 120.0, 150.0];
  let started = Instant::now();
  let mut sink = 0.0;

  for (i, p) in paragraphs.iter().enumerate() {
    for w in widths {
      sink += p
        .shaped
        .break_at(Some(w + 0.001 * (i % 7) as f32 + 0.37))
        .layout
        .height();
    }
  }

  let t_fresh = ms(started);
  r.line(format!(
    "labels.parley.immutability: 3000 fresh layouts per width {t_fresh:.2} ms ({:.2} us each) [checksum {sink:.0}]",
    t_fresh * 1000.0 / 3000.0
  ));

  // Draw at the used widths.
  let mut boxes = Vec::new();
  absolute(&tree, root, (0.0, 0.0), &mut boxes);
  let find = |n: NodeId| boxes.iter().find(|(m, _)| *m == n).unwrap().1;
  let rootbox = find(root);
  let (w, h) = (rootbox[2].ceil() as u32, rootbox[3].ceil() as u32);
  let laid: Vec<(Rc<Laid>, (f32, f32))> = leaves
    .iter()
    .enumerate()
    .map(|(i, &leaf)| {
      let b = find(leaf);
      (
        paragraphs[i].layout(Some(b[2]), &counters),
        (b[0].round(), b[1].round()),
      )
    })
    .collect();
  let placed: Vec<(&Laid, (f32, f32))> = laid.iter().map(|(l, at)| (&**l, *at)).collect();
  let started = Instant::now();
  let (pixmap, glyphs) = render::render_many(&placed, w as u16, h as u16);
  let t_b_cold = ms(started);
  let mut warm = Vec::new();

  for _ in 0..3 {
    let started = Instant::now();
    let _ = render::render_many(&placed, w as u16, h as u16);
    warm.push(ms(started));
  }

  warm.sort_by(f64::total_cmp);
  render::write_png(pixmap, &out.join("labels-parley-vello.png"))?;
  r.line(format!(
    "labels.parley.draw.b (vello_cpu outlines, harness render_many, includes glyph-run extraction): cold {t_b_cold:.2} ms, warm median {:.2} ms; {glyphs} glyphs on {w}x{h}",
    warm[1]
  ));

  // C: Parley's runs through DirectWrite's atlas.
  let started = Instant::now();
  let mut faces = Faces::new(factory).msg("faces")?;
  let items: Vec<Item> = laid
    .iter()
    .map(|(l, (x, y))| {
      let records = l.glyph_run_records(&mut engine.fonts);
      Item::Glyphs {
        runs: Rc::new(faces.runs(&records)),
        x: *x,
        y: *y,
        color: [0.0, 0.0, 0.0, 1.0],
      }
    })
    .collect();
  let t_convert = ms(started);
  let mut painters = Painters::new(factory)?;
  let mut ctx = new_context(w as u16, h as u16, 0);
  let started = Instant::now();
  let first = painters.atlas.render(&mut ctx, &items).msg("atlas")?;
  let t_c_cold = ms(started);
  let mut warm = Vec::new();

  for _ in 0..5 {
    let started = Instant::now();
    let _ = painters.atlas.render(&mut ctx, &items).msg("atlas")?;
    warm.push(ms(started));
  }

  warm.sort_by(f64::total_cmp);
  to_img(&first).save(&out.join("labels-parley-atlas.png"));
  r.line(format!(
    "labels.parley.draw.c (DirectWrite atlas over Parley's runs): runs to faces {t_convert:.2} ms; cold {t_c_cold:.2} ms ({} cells), warm median {:.2} ms; faces failed {:?}",
    painters.atlas.rasterized,
    warm[2],
    faces.failures.iter().take(3).collect::<Vec<_>>()
  ));

  // B with warm caches: vello's outlines through the first round's floor painter, which keeps its
  // fonts and glyph caches across frames, over the same runs.
  let started = Instant::now();
  let _ = painters.floor.render(&mut ctx, &items).msg("floor")?;
  let t_f_cold = ms(started);
  let mut warm = Vec::new();

  for _ in 0..5 {
    let started = Instant::now();
    let _ = painters.floor.render(&mut ctx, &items).msg("floor")?;
    warm.push(ms(started));
  }

  warm.sort_by(f64::total_cmp);
  r.line(format!(
    "labels.parley.draw.b_floor (vello_cpu outlines over Parley's runs, caches kept): cold {t_f_cold:.2} ms, warm median {:.2} ms",
    warm[2]
  ));

  Ok(())
}
