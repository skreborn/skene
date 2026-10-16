//! Spike: DirectWrite lays text out under taffy, and its glyph runs enter vello two ways; then
//! the text-parity corpus three ways (A DirectWrite, B Parley and vello, C Parley drawn by
//! DirectWrite) and the probes the text-engine revisit asks for.
//!
//! Each part prints `key: value` lines into `results.txt` in the output directory (`SPIKE_OUT`,
//! `out` by default), beside the rendered PNGs. `SPIKE_PARTS` names the parts, comma-separated
//! (`spike`, `parity`, `probes`, `labels`, `textscale`, `userfont`, `fontique`,
//! `dwritestart`); the first four by default.

#[cfg(windows)]
mod engine;
#[cfg(windows)]
mod faces;
mod image;
#[cfg(windows)]
mod labels;
#[cfg(windows)]
mod native;
#[cfg(windows)]
mod paint;
#[cfg(windows)]
mod parity;
#[cfg(windows)]
mod probes;
#[cfg(windows)]
mod resolver;

/// The lines a run prints and keeps.
#[derive(Default)]
pub struct Report {
  pub lines: Vec<String>,
}

impl Report {
  pub fn line(&mut self, s: impl Into<String>) {
    let s = s.into();
    println!("{s}");
    self.lines.push(s);
  }
}

#[cfg(not(windows))]
fn main() {
  eprintln!("Windows only: cargo check --target x86_64-pc-windows-msvc");
}

#[cfg(windows)]
fn main() {
  let out = std::path::PathBuf::from(std::env::var("SPIKE_OUT").unwrap_or_else(|_| "out".into()));
  std::fs::create_dir_all(&out).expect("output directory");
  let parts = std::env::var("SPIKE_PARTS").unwrap_or_else(|_| "spike,parity,probes,labels".into());
  let file = std::env::var("SPIKE_RESULTS").unwrap_or_else(|_| "results.txt".into());
  let mut report = Report::default();

  for part in parts.split(',').map(str::trim).filter(|p| !p.is_empty()) {
    report.line(format!("=== part {part}"));
    let started = std::time::Instant::now();
    let result =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
        match part {
          "spike" => spike::run(&out, &mut report).map_err(|e| e.to_string()),
          "parity" => parity::run(&out, &spike::factory()?, &mut report),
          "probes" => probes::run(&out, &spike::factory()?, &mut report),
          "labels" => labels::run(&out, &spike::factory()?, &mut report),
          "textscale" => probes::text_scale(&spike::factory()?, &mut report),
          "userfont" => probes::user_font(&spike::factory()?, &mut report),
          "fontique" => probes::fontique(&mut report),
          "dwritestart" => probes::dwrite_startup(&mut report),
          other => Err(format!("unknown part {other}")),
        }
      }));

    match result {
      Ok(Ok(())) => {}
      Ok(Err(e)) => report.line(format!("error: {part}: {e}")),
      Err(panic) => report.line(format!(
        "panic: {part}: {}",
        panic
          .downcast_ref::<String>()
          .cloned()
          .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
          .unwrap_or_default()
      )),
    }

    report.line(format!(
      "=== part {part} took {:.0} ms",
      started.elapsed().as_secs_f64() * 1e3
    ));
    std::fs::write(out.join(&file), report.lines.join("\n") + "\n").expect("results");
  }
}

#[cfg(windows)]
mod spike {
  use std::{path::Path, rc::Rc, time::Instant};

  use taffy::prelude::*;
  use windows::Win32::{
    Graphics::DirectWrite::*,
    System::Com::{COINIT_MULTITHREADED, CoInitializeEx},
  };
  use windows_core::{Interface, Result};

  use crate::{
    Report,
    engine::{EllipsisStrategy, Engine, Laid, Paragraph, ParagraphStyle, line_metrics},
    image::{Img, Region},
    paint::{AtlasPainter, ColourPath, Floor, Item, Reference, face_path, new_context, to_img},
  };

  /// The shared DirectWrite factory, COM initialized on this thread.
  pub fn factory() -> std::result::Result<IDWriteFactory4, String> {
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }.map_err(|e| e.to_string())
  }

  macro_rules! say {
    ($r:expr, $($t:tt)*) => { $r.line(format!($($t)*)) };
  }

  const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

  fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
  }

  fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
  }

  /// Absolute boxes of a tree's nodes.
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

  fn find(boxes: &[(NodeId, [f32; 4])], node: NodeId) -> [f32; 4] {
    boxes.iter().find(|(n, _)| *n == node).unwrap().1
  }

  fn compute(tree: &mut TaffyTree<usize>, root: NodeId, paragraphs: &[Paragraph]) {
    tree
      .compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, ctx, _| {
        ctx.map_or(taffy::LayoutOutput::HIDDEN, |i| {
          paragraphs[*i].measure(input)
        })
      })
      .unwrap();
  }

  pub fn run(out: &Path, r: &mut Report) -> Result<()> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
    let engine = Engine::new()?;
    say!(
      r,
      "env.dwrite_factory7: {}",
      engine.factory.cast::<IDWriteFactory7>().is_ok()
    );
    say!(
      r,
      "env.dwrite_factory8: {}",
      engine.factory.cast::<IDWriteFactory8>().is_ok()
    );
    say!(
      r,
      "env.threads_available: {}",
      std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
    );
    say!(
      r,
      "env.gpu: none used; every path below renders on the CPU (vello_cpu, Direct2D software WIC target)"
    );

    leaf_checks(&engine, r)?;
    scene(&engine, out, r)?;
    perf(&engine, out, r)?;
    Ok(())
  }

  // -------------------------------------------------------------------------------------------
  // The leaf: the clamp fix and the immutable laid values.

  fn leaf_checks(engine: &Rc<Engine>, r: &mut Report) -> Result<()> {
    let word = engine.paragraph(
      "Supercalifragilisticexpialidocious",
      ParagraphStyle::default(),
    )?;
    let sentence = engine.paragraph(
      "The quick brown fox jumps over the lazy dog again and again",
      ParagraphStyle::default(),
    )?;
    say!(r, "leaf.word.intrinsic: {:?}", word.intrinsic());
    say!(
      r,
      "leaf.sentence.intrinsic: min={:.2} max={:.2}",
      sentence.min,
      sentence.max
    );

    let paragraphs = vec![word, sentence];
    let mid = (paragraphs[1].min + paragraphs[1].max) / 2.0;

    for (name, index, width, start) in [
      ("word_in_40", 0usize, 40.0f32, false),
      ("word_in_1000", 0, 1000.0, true),
      ("sentence_in_mid_start", 1, mid, true),
      ("sentence_in_mid_stretch", 1, mid, false),
    ] {
      let mut tree: TaffyTree<usize> = TaffyTree::new();
      tree.disable_rounding();
      let leaf = tree.new_leaf_with_context(Style::default(), index).unwrap();
      let root = tree
        .new_with_children(
          Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: if start {
              AlignItems::START
            }
            else {
              AlignItems::NORMAL
            },
            size: Size {
              width: length(width),
              height: auto(),
            },
            ..Default::default()
          },
          &[leaf],
        )
        .unwrap();
      compute(&mut tree, root, &paragraphs);
      let l = tree.layout(leaf).unwrap();
      let laid = paragraphs[index].layout(Some(l.size.width))?;
      say!(
        r,
        "leaf.{name}: container={width:.2} leaf={:.2}x{:.2} lines={} fits={}",
        l.size.width,
        l.size.height,
        laid.line_count,
        l.size.width <= width + 0.01
      );
    }

    let p = &paragraphs[1];
    let narrow = p.layout(Some(120.0))?;
    let before = (narrow.line_count, narrow.height);
    let wide = p.layout(Some(600.0))?;
    let reread = line_metrics(&narrow.pieces()[0].layout)?.len();
    say!(
      r,
      "leaf.immutable: laid@120 lines={} h={:.2}; after laid@600 (lines={}) laid@120 re-read lines={} h={:.2}; unchanged={}",
      before.0,
      before.1,
      wide.line_count,
      reread,
      narrow.height,
      reread == before.0 && narrow.height == before.1
    );

    let caret = narrow.caret(4)?;
    let hit = narrow.hit_test(caret.0 + 0.5, caret.1 + caret.2 / 2.0)?;
    let rects = narrow.range_rects(0, p.utf16.len() as u32)?;
    say!(
      r,
      "leaf.geometry: caret(4)=({:.2},{:.2},h{:.2}) hit_test(caret)={:?} range_rects(all)={} (one per line: {})",
      caret.0,
      caret.1,
      caret.2,
      hit,
      rects.len(),
      rects.len() == narrow.line_count
    );
    Ok(())
  }

  // -------------------------------------------------------------------------------------------
  // The scene: overlay, clip, opacity group, emoji and two ellipsis strategies, drawn three ways.

  struct Scene {
    items: Vec<Item>,
    w: u32,
    h: u32,
    overlay: [f32; 4],
    clip: [f32; 4],
    clip_leaf: [f32; 4],
    group_rect: [f32; 4],
    emoji: [f32; 4],
    panels: Vec<(&'static str, [f32; 4])>,
    ellipsis: Vec<(&'static str, Rc<Laid>, [f32; 4])>,
    emoji_laid: Rc<Laid>,
  }

  const LOREM: &str = "DirectWrite lays this paragraph out at three hundred pixels and caps it at two lines, so the second line should end in an ellipsis and nothing of the third line or later should be drawn anywhere.";

  fn build_scene(engine: &Rc<Engine>) -> Result<Scene> {
    let style = |size: f32, weight: i32| ParagraphStyle {
      size,
      weight,
      ..Default::default()
    };
    let capped = |strategy| ParagraphStyle {
      size: 18.0,
      max_lines: Some(2),
      ellipsis: strategy,
      ..Default::default()
    };
    let paragraphs = vec![
      engine.paragraph("Overlay above: Hamburgefonstiv", style(28.0, 400))?,
      engine.paragraph("Clipped: Hamburgefonstiv 0123", style(36.0, 400))?,
      engine.paragraph("Group opacity: Hamburgefonstiv", style(32.0, 700))?,
      engine.paragraph("Emoji: \u{1F600}\u{1F389}\u{1F44D}\u{1F3FD}\u{2764}\u{FE0F}\u{1F1EB}\u{1F1EE}\u{1F9D1}\u{200D}\u{1F4BB} ok", style(32.0, 400))?,
      engine.paragraph(LOREM, capped(EllipsisStrategy::Native))?,
      engine.paragraph(LOREM, capped(EllipsisStrategy::Split))?,
      engine.paragraph("Supercalifragilistic", style(16.0, 400))?,
    ];

    let mut tree: TaffyTree<usize> = TaffyTree::new();
    tree.disable_rounding();
    let mut children = Vec::new();
    let mut leaves = Vec::new();

    for (i, fixed) in [None, None, None, None, Some(300.0), Some(300.0), Some(60.0)]
      .into_iter()
      .enumerate()
    {
      let leaf = tree.new_leaf_with_context(Style::default(), i).unwrap();
      leaves.push(leaf);

      match fixed {
        None => children.push(leaf),
        Some(w) => children.push(
          tree
            .new_with_children(
              Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size {
                  width: length(w),
                  height: auto(),
                },
                ..Default::default()
              },
              &[leaf],
            )
            .unwrap(),
        ),
      }
    }

    let root = tree
      .new_with_children(
        Style {
          display: Display::Flex,
          flex_direction: FlexDirection::Column,
          align_items: AlignItems::START,
          size: Size {
            width: length(800.0),
            height: auto(),
          },
          padding: Rect {
            left: length(20.0),
            right: length(20.0),
            top: length(20.0),
            bottom: length(20.0),
          },
          gap: Size {
            width: length(0.0),
            height: length(28.0),
          },
          ..Default::default()
        },
        &children,
      )
      .unwrap();
    compute(&mut tree, root, &paragraphs);
    let mut boxes = Vec::new();
    absolute(&tree, root, (0.0, 0.0), &mut boxes);
    let rootbox = find(&boxes, root);
    let b: Vec<[f32; 4]> = leaves.iter().map(|&l| find(&boxes, l)).collect();
    let laid: Vec<Rc<Laid>> = paragraphs
      .iter()
      .zip(&b)
      .map(|(p, b)| p.layout(Some(b[2])))
      .collect::<Result<_>>()?;
    let snap = |b: [f32; 4]| [b[0].round(), b[1].round(), b[2], b[3]];
    let text = |i: usize| Item::Text {
      laid: laid[i].clone(),
      x: b[i][0].round(),
      y: b[i][1].round(),
      color: BLACK,
    };
    let mut items = Vec::new();

    // Overlay: a translucent red rectangle drawn after the right half of the text.
    let overlay = [
      (b[0][0] + b[0][2] / 2.0).round(),
      (b[0][1] - 4.0).round(),
      (b[0][2] / 2.0 + 4.0).round(),
      (b[0][3] + 8.0).round(),
    ];
    items.push(text(0));
    items.push(Item::Fill {
      rect: overlay,
      color: [1.0, 0.0, 0.0, 0.5],
    });

    // Clip: a whole-pixel rectangle cutting through the glyphs on every side.
    let c = snap(b[1]);
    let clip = [
      c[0] + 40.0,
      c[1] + (c[3] * 0.3).round(),
      (c[2] - 100.0).round(),
      (c[3] * 0.4).round(),
    ];
    items.push(Item::PushClip(clip));
    items.push(text(1));
    items.push(Item::Pop);

    // Opacity group: a blue rectangle and black text above it, composited together at 0.5.
    let g = snap(b[2]);
    let group_rect = [g[0], g[1], (g[2] * 0.6).round(), g[3].round()];
    items.push(Item::PushOpacity(0.5));
    items.push(Item::Fill {
      rect: group_rect,
      color: [0.0, 0.0, 1.0, 1.0],
    });
    items.push(text(2));
    items.push(Item::Pop);

    items.push(text(3));

    for i in [4, 5, 6] {
      let parent = tree.parent(leaves[i]).unwrap();
      items.push(Item::Fill {
        rect: snap(find(&boxes, parent)),
        color: [0.92, 0.92, 0.92, 1.0],
      });
      items.push(text(i));
    }

    Ok(Scene {
      w: (rootbox[2].ceil()) as u32,
      h: (rootbox[3].ceil()) as u32,
      overlay,
      clip,
      clip_leaf: snap(b[1]),
      group_rect,
      emoji: snap(b[3]),
      panels: vec![
        ("overlay", snap(b[0])),
        ("clip", snap(b[1])),
        ("opacity", snap(b[2])),
        ("emoji", snap(b[3])),
        ("ellipsis_native", snap(b[4])),
        ("ellipsis_split", snap(b[5])),
        ("break_word", snap(b[6])),
      ],
      ellipsis: vec![
        ("native", laid[4].clone(), snap(b[4])),
        ("split", laid[5].clone(), snap(b[5])),
      ],
      emoji_laid: laid[3].clone(),
      items,
    })
  }

  fn scene(engine: &Rc<Engine>, out: &Path, r: &mut Report) -> Result<()> {
    let s = build_scene(engine)?;
    say!(r, "scene.size: {}x{}", s.w, s.h);

    let mut floor = Floor::new();
    let mut atlas = AtlasPainter::new(&engine.factory)?;
    let reference = Reference::new()?;
    let mut ctx = new_context(s.w as u16, s.h as u16, 0);

    let floor_img = to_img(&floor.render(&mut ctx, &s.items)?);
    let atlas_img = to_img(&atlas.render(&mut ctx, &s.items)?);
    let mut raw = AtlasPainter::new(&engine.factory)?;
    raw.contrast = 0.0;
    let raw_img = to_img(&raw.render(&mut ctx, &s.items)?);
    let mut layered = AtlasPainter::new(&engine.factory)?;
    layered.colour_path = ColourPath::Layers;
    let layers_img = to_img(&layered.render(&mut ctx, &s.items)?);
    let ref_img = reference.render(&s.items, s.w, s.h)?;
    let bare: Vec<Item> = s
      .items
      .iter()
      .filter(|i| !matches!(i, Item::Text { .. }))
      .cloned()
      .collect();
    let bg = to_img(&Floor::new().render(&mut ctx, &bare)?);

    floor_img.save(&out.join("scene-floor.png"));
    atlas_img.save(&out.join("scene-atlas.png"));
    ref_img.save(&out.join("scene-reference.png"));
    floor_img
      .diff(&ref_img)
      .save(&out.join("scene-diff-floor-vs-reference.png"));
    atlas_img
      .diff(&ref_img)
      .save(&out.join("scene-diff-atlas-vs-reference.png"));
    Img::hstack(&[&floor_img, &atlas_img, &ref_img]).save(&out.join("scene-side-by-side.png"));
    atlas.save_atlas(&out.join("atlas.png"));
    layers_img.save(&out.join("scene-atlas-colr-layers.png"));
    say!(
      r,
      "atlas.colour_path: {:?} (Direct2D unavailable because: {:?}); colour cells drawn by Direct2D={}",
      atlas.colour_path,
      atlas.colour_unavailable,
      atlas.colour_cells
    );

    say!(
      r,
      "atlas.grayscale_enhanced_contrast: {:.3}",
      atlas.contrast
    );
    say!(
      r,
      "atlas.cells_rasterized: {} errors={} first_error={:?}",
      atlas.rasterized,
      atlas.raster_errors,
      atlas.first_error
    );
    say!(
      r,
      "floor.glyph_errors: {} unresolved_faces={} simulated_runs={}",
      floor.glyph_errors,
      floor.fonts.unresolved,
      floor.fonts.simulated_runs
    );

    let (w, h) = (s.w as usize, s.h as usize);
    let region = |b: [f32; 4]| Region::from_f32(b, w, h);
    let mut za = Vec::new();

    for (name, img) in [
      ("floor", &floor_img),
      ("atlas", &atlas_img),
      ("atlas_colr_layers", &layers_img),
      ("reference", &ref_img),
    ] {
      // Z-order: under the overlay nothing is darker in red than the overlay over black.
      let ov = Region::from_f32(
        [
          s.overlay[0] + 1.0,
          s.overlay[1] + 1.0,
          s.overlay[2] - 2.0,
          s.overlay[3] - 2.0,
        ],
        w,
        h,
      );
      let min_r = ov.iter().map(|(x, y)| img.at(x, y)[0]).min().unwrap_or(0);
      let under = ov.iter().filter(|&(x, y)| img.at(x, y)[1] < 90).count();
      let z_ok = min_r >= 110 && under >= 30;

      // Clip: nothing drawn outside the clip rectangle around the clipped text.
      let around = region(s.clip_leaf).inflate(8, w, h);
      let inside = region(s.clip);
      let outside_ink = around
        .iter()
        .filter(|&(x, y)| !inside.contains(x, y) && img.at(x, y)[..3].iter().any(|&c| c < 250))
        .count();
      let inside_ink = inside
        .iter()
        .filter(|&(x, y)| img.at(x, y)[0] < 200)
        .count();
      let clip_ok = outside_ink == 0 && inside_ink >= 50;

      // Opacity group: composited as one layer, text over the rectangle is grey, not navy.
      let gr = Region::from_f32(
        [
          s.group_rect[0] + 1.0,
          s.group_rect[1] + 1.0,
          s.group_rect[2] - 2.0,
          s.group_rect[3] - 2.0,
        ],
        w,
        h,
      );
      let g_min_r = gr.iter().map(|(x, y)| img.at(x, y)[0]).min().unwrap_or(0);
      let g_min_b = gr.iter().map(|(x, y)| img.at(x, y)[2]).min().unwrap_or(255);
      let group_ok = g_min_r >= 110 && g_min_b <= 150;

      say!(
        r,
        "a.{name}: zorder={} (min red under overlay={min_r}, text pixels under overlay={under}); clip={} (ink outside clip={outside_ink}, inside={inside_ink}); group={} (min red in group={g_min_r}, min blue={g_min_b})",
        if z_ok { "PASS" } else { "FAIL" },
        if clip_ok { "PASS" } else { "FAIL" },
        if group_ok { "PASS" } else { "FAIL" },
      );

      if name != "reference" && name != "atlas_colr_layers" {
        za.push(z_ok && clip_ok && group_ok);
      }

      let chroma = img.chroma_count(region(s.emoji));
      say!(
        r,
        "d.{name}: colour pixels in the emoji line={chroma} ({})",
        if chroma >= 200 { "PASS" } else { "FAIL" }
      );
    }

    for (name, img) in [
      ("floor", &floor_img),
      ("atlas", &atlas_img),
      ("atlas_without_contrast", &raw_img),
      ("atlas_colr_layers", &layers_img),
    ] {
      for (panel, b) in &s.panels {
        let reg = region(*b).inflate(4, w, h);
        let ink_ref = ref_img.darkness(&bg, reg);
        say!(
          r,
          "compare.{name}.{panel}: ink_iou={:.3} mean_abs_diff={:.2} darkness_ratio={:.3}",
          img.ink_iou(&ref_img, &bg, reg),
          img.mean_abs_diff(&ref_img, reg),
          if ink_ref > 0.0 {
            img.darkness(&bg, reg) / ink_ref
          }
          else {
            0.0
          }
        );
      }
    }

    say!(
      r,
      "a.verdict: {}",
      if za.iter().all(|&ok| ok) {
        "PASS"
      }
      else {
        "FAIL"
      }
    );

    // (c) the ellipsis on the last visible line.
    for (name, laid, b) in &s.ellipsis {
      let runs = laid.runs()?;
      let signs: Vec<_> = runs.runs.iter().filter(|r| r.inline).collect();
      let baselines: Vec<f32> = {
        let mut v: Vec<f32> = runs.runs.iter().map(|r| r.origin.1).collect();
        v.dedup();
        v
      };
      let last = laid.line_baselines.last().copied().unwrap_or(0.0);
      let below = runs.runs.iter().filter(|r| r.origin.1 > last + 1.0).count();
      let sign_y = signs.first().map(|r| r.origin.1);
      let sign_right = signs.iter().map(|r| r.right()).fold(0.0f32, f32::max);
      let on_last = sign_y.is_some_and(|y| (y - last).abs() <= 1.0);
      let reg = Region::from_f32(
        [b[0] - 2.0, b[1] + laid.height + 1.0, b[2] + 4.0, 24.0],
        w,
        h,
      );
      let ink_below: Vec<usize> = [&floor_img, &atlas_img, &ref_img]
        .iter()
        .map(|i| i.ink_count(&bg, reg))
        .collect();
      let mut detail = String::new();

      for piece in laid.pieces() {
        let lines = line_metrics(&piece.layout)?;
        detail += &format!(
          " [piece y={:.1}: {} lines, trimmed={:?}, max_height={:.1}]",
          piece.y,
          lines.len(),
          lines
            .iter()
            .map(|l| l.isTrimmed.as_bool())
            .collect::<Vec<_>>(),
          unsafe { piece.layout.GetMaxHeight() }
        );
      }

      let ok = laid.line_count == 2
        && on_last
        && below == 0
        && sign_right <= laid.box_width + 0.5
        && ink_below.iter().all(|&n| n == 0);
      say!(
        r,
        "c.{name}: {} truncated={} visible_lines={} line_baselines={:?} run_baselines={:?} ellipsis_runs={} ellipsis_baseline={:?} ellipsis_right={:.2} box_width={:.2} runs_below_last_line={below} ink_below_box(floor,atlas,ref)={:?}{detail}",
        if ok { "PASS" } else { "FAIL" },
        laid.truncated,
        laid.line_count,
        laid.line_baselines,
        baselines,
        signs.len(),
        sign_y,
        sign_right,
        laid.box_width,
        ink_below,
      );
    }

    // (d) which face the emoji came from.
    for run in &laid_runs(&s.emoji_laid)? {
      let colour = run
        .face
        .cast::<IDWriteFontFace2>()
        .map(|f| unsafe { f.IsColorFont() }.as_bool())
        .unwrap_or(false);
      say!(
        r,
        "d.run: glyphs={} em={:.1} colour_font={colour} file={}",
        run.glyphs.len(),
        run.em,
        face_path(&run.face).unwrap_or_else(|e| format!("? {e}"))
      );
    }

    say!(
      r,
      "d.atlas_colr_layers: layers drawn={} unsupported_layers={} unsupported_formats={:?} seen_formats={:?}",
      layered.color_layers,
      layered.unsupported_layers,
      layered.layer_formats,
      layered.seen_formats
    );
    Ok(())
  }

  fn laid_runs(laid: &Laid) -> Result<Vec<crate::engine::GlyphRunRec>> {
    let c = laid.glyph_runs(0.0, 0.0)?;
    Ok(c.runs)
  }

  // -------------------------------------------------------------------------------------------
  // (b) 1000 labels.

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

  fn perf(engine: &Rc<Engine>, out: &Path, r: &mut Report) -> Result<()> {
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

    let label = ParagraphStyle {
      size: 13.0,
      ..Default::default()
    };
    let make = |texts: &[String]| -> Result<(Vec<Paragraph>, f64, f64)> {
      let t = Instant::now();
      let first = engine.paragraph(&texts[0], label.clone())?;
      let t_first = ms(t.elapsed());
      let mut out = vec![first];

      for text in &texts[1..] {
        out.push(engine.paragraph(text, label.clone())?);
      }

      Ok((out, t_first, ms(t.elapsed())))
    };
    let (paragraphs, t_first, t_para) = make(&texts)?;
    let (_, t_first_again, t_para_again) = make(&texts)?;

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
    let root_style = |w: f32| Style {
      display: Display::Flex,
      flex_direction: FlexDirection::Row,
      flex_wrap: FlexWrap::Wrap,
      align_items: AlignItems::START,
      size: Size {
        width: length(w),
        height: auto(),
      },
      padding: Rect {
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
    };
    let root = tree.new_with_children(root_style(1200.0), &leaves).unwrap();

    let counters = |e: &Engine| {
      (
        e.counters.measure_calls.get(),
        e.counters.laid_created.get(),
        e.counters.laid_cache_hits.get(),
      )
    };
    let c0 = counters(engine);
    let t = Instant::now();
    compute(&mut tree, root, &paragraphs);
    let t_cold = ms(t.elapsed());
    let c1 = counters(engine);

    for &leaf in &leaves {
      tree.mark_dirty(leaf).unwrap();
    }

    let t = Instant::now();
    compute(&mut tree, root, &paragraphs);
    let t_warm = ms(t.elapsed());
    let c2 = counters(engine);

    tree.set_style(root, root_style(1100.0)).unwrap();
    let t = Instant::now();
    compute(&mut tree, root, &paragraphs);
    let t_resize = ms(t.elapsed());
    let c3 = counters(engine);

    tree.set_style(root, root_style(1200.0)).unwrap();
    compute(&mut tree, root, &paragraphs);

    say!(
      r,
      "b.paragraphs: 1000 created (format cached per style, one probe layout each for the intrinsic widths) in {t_para:.2} ms, the first alone {t_first:.2} ms; the same 1000 again {t_para_again:.2} ms (first {t_first_again:.2} ms)"
    );
    say!(
      r,
      "b.layout.cold: {t_cold:.2} ms; measure calls={} laid created={} cache hits={}",
      c1.0 - c0.0,
      c1.1 - c0.1,
      c1.2 - c0.2
    );
    say!(
      r,
      "b.layout.warm_same_width (all leaves dirtied): {t_warm:.2} ms; measure calls={} laid created={} cache hits={}",
      c2.0 - c1.0,
      c2.1 - c1.1,
      c2.2 - c1.2
    );
    say!(
      r,
      "b.layout.container_resize_1200_to_1100: {t_resize:.2} ms; measure calls={} laid created={} cache hits={}",
      c3.0 - c2.0,
      c3.1 - c2.1,
      c3.2 - c2.2
    );

    // The cost of immutability: a fresh layout per width against re-wrapping a shared one.
    let widths = [90.0f32, 120.0, 150.0];
    let t = Instant::now();
    let mut sink = 0.0;

    for p in &paragraphs {
      for w in widths {
        sink += p.rewrap_height(w)?;
      }
    }

    let t_rewrap = ms(t.elapsed());
    let t = Instant::now();

    for (i, p) in paragraphs.iter().enumerate() {
      for w in widths {
        // Off-grid widths, so none is a cache hit.
        sink += p.layout(Some(w + 0.001 * (i % 7) as f32 + 0.37))?.height;
      }
    }

    let t_fresh = ms(t.elapsed());
    say!(
      r,
      "b.immutability_cost: 3000 heights-for-width by SetMaxWidth on a shared layout {t_rewrap:.2} ms ({:.2} us each) vs a fresh owned IDWriteTextLayout {t_fresh:.2} ms ({:.2} us each) [checksum {sink:.0}]",
      t_rewrap * 1000.0 / 3000.0,
      t_fresh * 1000.0 / 3000.0
    );

    // Draw: the final laid values at their used widths.
    let mut boxes = Vec::new();
    absolute(&tree, root, (0.0, 0.0), &mut boxes);
    let rootbox = find(&boxes, root);
    let (w, h) = (rootbox[2].ceil() as u32, rootbox[3].ceil() as u32);
    let mut items = Vec::new();

    for (i, &leaf) in leaves.iter().enumerate() {
      let b = find(&boxes, leaf);
      let laid = paragraphs[i].layout(Some(b[2]))?;
      items.push(Item::Text {
        laid,
        x: b[0].round(),
        y: b[1].round(),
        color: BLACK,
      });
    }

    let t = Instant::now();
    let mut glyphs = 0usize;
    let mut runs = 0usize;

    for item in &items {
      if let Item::Text { laid, .. } = item {
        let c = laid.runs()?;
        runs += c.runs.len();
        glyphs += c.runs.iter().map(|r| r.glyphs.len()).sum::<usize>();
      }
    }

    let t_collect = ms(t.elapsed());
    say!(r, "b.canvas: {w}x{h}, {runs} glyph runs, {glyphs} glyphs");
    say!(
      r,
      "b.collect_glyph_runs (IDWriteTextLayout::Draw into the collecting renderer, once per laid value): {t_collect:.2} ms"
    );

    let threads = std::thread::available_parallelism()
      .map(|n| n.get())
      .unwrap_or(1)
      .saturating_sub(1)
      .clamp(1, 3) as u16;

    for (label, n) in [("st", 0u16), ("mt", threads)] {
      let mut ctx = new_context(w as u16, h as u16, n);
      let mut floor = Floor::new();
      let t = Instant::now();
      let first = floor.render(&mut ctx, &items)?;
      let t_floor_cold = ms(t.elapsed());
      let mut warm = Vec::new();

      for _ in 0..5 {
        let t = Instant::now();
        let _ = floor.render(&mut ctx, &items)?;
        warm.push(ms(t.elapsed()));
      }

      say!(
        r,
        "b.draw.floor.{label} (threads={n}): cold {t_floor_cold:.2} ms, warm median {:.2} ms (min {:.2})",
        median(warm.clone()),
        warm.iter().cloned().fold(f64::MAX, f64::min)
      );

      if n == 0 {
        to_img(&first).save(&out.join("labels-floor.png"));
      }

      let mut atlas = AtlasPainter::new(&engine.factory)?;
      let t = Instant::now();
      let first = atlas.render(&mut ctx, &items)?;
      let t_atlas_cold = ms(t.elapsed());
      let mut warm = Vec::new();

      for _ in 0..5 {
        let t = Instant::now();
        let _ = atlas.render(&mut ctx, &items)?;
        warm.push(ms(t.elapsed()));
      }

      say!(
        r,
        "b.draw.atlas.{label} (threads={n}): cold {t_atlas_cold:.2} ms ({} cells rasterized by DirectWrite), warm median {:.2} ms (min {:.2})",
        atlas.rasterized,
        median(warm.clone()),
        warm.iter().cloned().fold(f64::MAX, f64::min)
      );

      if n == 0 {
        to_img(&first).save(&out.join("labels-atlas.png"));
      }
    }

    let reference = Reference::new()?;
    let t = Instant::now();
    let img = reference.render(&items, w, h)?;
    let t_ref_cold = ms(t.elapsed());
    let mut warm = Vec::new();

    for _ in 0..3 {
      let t = Instant::now();
      let _ = reference.render(&items, w, h)?;
      warm.push(ms(t.elapsed()));
    }

    img.save(&out.join("labels-reference.png"));
    say!(
      r,
      "b.draw.reference_d2d_wic_software: cold {t_ref_cold:.2} ms, warm median {:.2} ms (includes bitmap creation and readback)",
      median(warm)
    );
    Ok(())
  }
}
