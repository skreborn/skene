//! The macOS half of the Apple text spike: run with `cargo test --release --test spike`.

use std::{fmt::Write as _, rc::Rc, time::Instant};

use apple_text_spike::{Available, Image, NodeView, Paragraph, TextNode, calls};
use objc2::{AnyThread, MainThreadMarker, rc::Retained};
use objc2_app_kit::{
  NSAppearance, NSAppearanceNameAqua, NSApplication, NSBitmapImageRep,
  NSColor, NSDeviceRGBColorSpace, NSFont, NSTextField, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use taffy::prelude::*;

const STRINGS: [(&str, &str); 7] = [
  ("latin", "The quick brown fox jumps over the lazy dog, then naps."),
  ("zh-Hans", "敏捷的棕色狐狸跳过了懒狗，然后睡了一觉。"),
  ("ja", "素早い茶色の狐がのろまな犬を飛び越えて、昼寝をした。"),
  ("ko", "빠른 갈색 여우가 게으른 개를 뛰어넘고 낮잠을 잤다."),
  ("thai", "สุนัขจิ้งจอกสีน้ำตาลกระโดดข้ามสุนัขขี้เกียจแล้วนอนหลับ"),
  ("emoji-zwj", "Family 👨‍👩‍👧‍👦 and 🏳️‍🌈 flags 👩🏽‍💻 at work"),
  ("rtl-mixed", "مرحبا بالعالم hello world مرة أخرى 123"),
];

const SCALE: f64 = 2.0;

fn render(view: &NSView, size: NSSize) -> Image {
  let width = (size.width * SCALE).round() as isize;
  let height = (size.height * SCALE).round() as isize;

  let rep = unsafe {
    NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
      NSBitmapImageRep::alloc(),
      std::ptr::null_mut(),
      width,
      height,
      8,
      4,
      true,
      false,
      NSDeviceRGBColorSpace,
      0,
      32,
    )
  }
  .expect("a bitmap");

  rep.setSize(size);
  view.cacheDisplayInRect_toBitmapImageRep(view.bounds(), &rep);
  let data = rep.bitmapData();
  let row = rep.bytesPerRow() as usize;
  let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);

  for y in 0..height as usize {
    for x in 0..width as usize {
      let at = unsafe { data.add(y * row + x * 4) };
      let alpha = unsafe { *at.add(3) };

      for channel in 0..3 {
        let value = unsafe { *at.add(channel) };
        pixels.push(value.saturating_add(255 - alpha));
      }
    }
  }

  Image { width: width as usize, height: height as usize, scale: SCALE, pixels }
}

fn main() {
  let mtm = MainThreadMarker::new().expect("the main thread");
  let app = NSApplication::sharedApplication(mtm);
  let aqua = NSAppearance::appearanceNamed(unsafe { NSAppearanceNameAqua });
  app.setAppearance(aqua.as_deref());
  let font = NSFont::systemFontOfSize(13.0);
  let mut out = String::new();
  writeln!(out, "font: {} {}pt", NSFont::systemFontOfSize(13.0).fontName(), font.pointSize()).unwrap();

  writeln!(out, "\n[pixel diff and extents, single line at max-content, {SCALE}x]").unwrap();
  writeln!(out, "(a) ink inside the measured box; (b) node vs NSTextField: raw at the same origin, and best within +-4 px; (c) min-content wrap").unwrap();

  for (name, text) in STRINGS {
    let node = TextNode::new(Paragraph::new(text, &font, 0));
    let (min, max) = node.paragraph.intrinsic();
    let measured = node.measure(Available::MaxContent);
    let laid = node.laid(measured.width);

    let field = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    field.setFont(Some(&font));
    field.setTextColor(Some(&NSColor::blackColor()));
    let fitting = field.fittingSize();

    let canvas = NSSize::new(
      measured.width.max(fitting.width).ceil() + 4.0,
      measured.height.max(fitting.height).ceil() + 4.0,
    );

    let frame = NSRect::new(NSPoint::new(0.0, 0.0), canvas);
    let view = NodeView::new(mtm, frame, laid.clone());
    field.setFrame(frame);
    let ours = render(&view, canvas);
    let theirs = render(&field, canvas);

    let ink = ours.ink();
    let extent = match ink {
      Some((x0, y0, x1, y1)) => {
        let px = |v: usize| v as f64 / SCALE;
        let over = [
          -px(x0),
          -px(y0),
          px(x1) - measured.width,
          px(y1) - measured.height,
        ];
        let worst = over.iter().cloned().fold(f64::MIN, f64::max);
        format!(
          "ink=({:.1},{:.1})-({:.1},{:.1}) overflow l/t/r/b=({:.2},{:.2},{:.2},{:.2}) {}",
          px(x0), px(y0), px(x1), px(y1), over[0], over[1], over[2], over[3],
          if worst <= 1.0 / SCALE { "PASS" } else { "FAIL" }
        )
      }
      None => "no ink FAIL".to_owned(),
    };

    let (raw, best) = ours.diff(&theirs, 4);

    let laid_min = node.paragraph.layout(min);
    let breaks = node.paragraph.break_opportunities();
    let mid_word = laid_min.line_starts.iter().filter(|start| !breaks.contains(start)).count();
    let widest = laid_min.line_widths.iter().cloned().fold(0.0, f64::max);
    let min_ok = mid_word == 0 && laid_min.size.0 <= min + 0.01;

    writeln!(out, "{name}: measured={:.2}x{:.2} baseline={:.2}/{:.2} lines={} NSTextField.fittingSize={:.2}x{:.2}", measured.width, measured.height, measured.first_baseline, measured.last_baseline, laid.line_count, fitting.width, fitting.height).unwrap();
    writeln!(out, "  (a) {extent}").unwrap();
    writeln!(out, "  (b) raw {raw}").unwrap();
    writeln!(out, "  (b) best {best}").unwrap();
    writeln!(out, "  (c) min={min:.2} max={max:.2} at-min: lines={} used={:.2} widest-line={widest:.2} mid-segment-breaks={mid_word} {}", laid_min.line_count, laid_min.size.0, if min_ok { "PASS" } else { "FAIL" }).unwrap();
  }

  writeln!(out, "\n[taffy: 1000 leaves in a column, align-items start]").unwrap();
  let texts: Vec<String> = (0..1000).map(|i| format!("{} #{i}", STRINGS[i % STRINGS.len()].1)).collect();

  let start = Instant::now();
  let before = calls();
  let nodes: Vec<TextNode> = texts.iter().map(|text| TextNode::new(Paragraph::new(text, &font, 0))).collect();
  let build_calls = calls() - before;
  let build_ms = start.elapsed().as_secs_f64() * 1e3;

  let mut tree: TaffyTree<usize> = TaffyTree::new();
  let leaves: Vec<NodeId> = (0..nodes.len()).map(|i| tree.new_leaf_with_context(Style::default(), i).unwrap()).collect();

  let column = |width: f32| Style {
    display: Display::Flex,
    flex_direction: FlexDirection::Column,
    align_items: AlignItems::START,
    size: Size { width: length(width), height: auto() },
    ..Style::default()
  };

  let root = tree.new_with_children(column(320.0), &leaves).unwrap();

  let pass = |tree: &mut TaffyTree<usize>, label: &str, out: &mut String| {
    for node in &nodes {
      node.measures.set(0);
      node.layouts.set(0);
    }

    let before = calls();
    let start = Instant::now();

    tree
      .compute_layout_with_measure(root, Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent }, |inputs, _, context, style| {
        let Some(&mut index) = context else {
          return taffy::LayoutOutput::HIDDEN;
        };

        let node = &nodes[index];
        let mut baselines = taffy::Baselines::NONE;

        let mut output = taffy::compute_leaf_layout(inputs, style, |_, _| 0.0, |known, available| {
          if let (Some(width), Some(height)) = (known.width, known.height) {
            return Size { width, height };
          }

          let request = match (known.width, available.width) {
            (Some(width), _) => Available::Known(width as f64),
            (None, AvailableSpace::MinContent) => Available::MinContent,
            (None, AvailableSpace::MaxContent) => Available::MaxContent,
            (None, AvailableSpace::Definite(width)) => Available::Definite(width as f64),
          };

          let measured = node.measure(request);
          baselines = taffy::Baselines { first: Some(measured.first_baseline as f32), last: Some(measured.last_baseline as f32) };
          Size { width: measured.width as f32, height: known.height.unwrap_or(measured.height as f32) }
        });

        output.baselines = baselines;
        output
      })
      .unwrap();

    let ms = start.elapsed().as_secs_f64() * 1e3;
    let spent = calls() - before;
    let measures: usize = nodes.iter().map(|node| node.measures.get()).sum();
    let layouts: usize = nodes.iter().map(|node| node.layouts.get()).sum();
    let max_measures = nodes.iter().map(|node| node.measures.get()).max().unwrap_or(0);

    writeln!(
      out,
      "{label}: {ms:.1} ms, objc calls/leaf={:.1}, measure calls/leaf={:.2} (max {max_measures}), TextKit layouts/leaf={:.2}",
      spent as f64 / nodes.len() as f64,
      measures as f64 / nodes.len() as f64,
      layouts as f64 / nodes.len() as f64,
    )
    .unwrap();
  };

  writeln!(out, "build 1000 paragraphs: {build_ms:.1} ms, objc calls/leaf={:.1}", build_calls as f64 / 1000.0).unwrap();
  pass(&mut tree, "cold layout at 320", &mut out);
  pass(&mut tree, "unchanged relayout (taffy cache)", &mut out);
  tree.set_style(root, column(300.0)).unwrap();
  pass(&mut tree, "width change 320->300", &mut out);

  for &leaf in &leaves {
    tree.mark_dirty(leaf).unwrap();
  }

  pass(&mut tree, "every leaf dirty, same width (text cache warm)", &mut out);

  let start = Instant::now();
  let before = calls();
  let fresh: Vec<Paragraph> = texts.iter().map(|text| Paragraph::new(text, &font, 0)).collect();

  for paragraph in &fresh {
    paragraph.intrinsic();
  }

  writeln!(
    out,
    "intrinsic min/max for 1000 fresh paragraphs (incl. build): {:.1} ms, objc calls/leaf={:.1}",
    start.elapsed().as_secs_f64() * 1e3,
    (calls() - before) as f64 / 1000.0
  )
  .unwrap();

  let start = Instant::now();
  let fields: Vec<Retained<NSTextField>> = texts
    .iter()
    .map(|text| {
      let field = NSTextField::wrappingLabelWithString(&NSString::from_str(text), mtm);
      field.setFont(Some(&font));
      field
    })
    .collect();

  let build = start.elapsed().as_secs_f64() * 1e3;
  let measure = |width: f64| {
    let start = Instant::now();

    for field in &fields {
      let cell = field.cell().unwrap();
      std::hint::black_box(cell.cellSizeForBounds(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, 1.0e7))));
    }

    start.elapsed().as_secs_f64() * 1e3
  };

  let at320 = measure(320.0);
  let at300 = measure(300.0);
  let again = measure(300.0);
  writeln!(out, "NSTextField baseline: build 1000 {build:.1} ms, cellSizeForBounds at 320 {at320:.1} ms, at 300 {at300:.1} ms, at 300 again {again:.1} ms").unwrap();

  let laid = Rc::new(Paragraph::new(STRINGS[0].1, &font, 0)).layout(200.0);
  let before = calls();
  let _ = laid.range_rects(4, 9);
  writeln!(out, "range_rects for one word: {} objc calls", calls() - before).unwrap();

  println!("=== RESULTS (macos) ===\n{out}=== END ===");

  if let Ok(path) = std::env::var("SPIKE_OUT") {
    std::fs::write(path, &out).unwrap();
  }
}
