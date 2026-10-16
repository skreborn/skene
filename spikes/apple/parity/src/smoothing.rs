//! Rasterization under each drawing condition: font smoothing on and off, opaque and
//! transparent backgrounds, black on white and white on black, at two scales. TextKit (A),
//! tuned Parley drawn by Core Text (C) and by vello (B, inverted for white text, since vello's
//! coverage has no colour dependence).

use serde_json::json;
use text_parity::{
  corpus::Corpus,
  image::Rgba,
  render::{self, RenderOptions},
};

use crate::{
  cg::{Canvas, Ground, InkDiff, Smoothing, ink_weight, inverted},
  probes::Report,
  raster::Raster,
  sets::tuned_runs,
  textkit::{self, Setup},
  tuned::TunedEngine,
};

fn vello(laid: &text_parity::layout::Laid, width: f32, height: f32, scale: f32) -> Rgba {
  let pixmap = render::render(laid, width, height, scale, RenderOptions::default());
  let mut out = Rgba {
    width: u32::from(pixmap.width()),
    height: u32::from(pixmap.height()),
    data: pixmap.data_as_u8_slice().to_vec(),
  };

  for pixel in out.data.chunks_exact_mut(4) {
    pixel[3] = 255;
  }

  out
}

/// The study, at `scales`.
pub fn study(report: &mut Report, tuned: &mut TunedEngine, raster: &mut Raster, scales: &[f32]) {
  report.line("\n[rasterization by condition: smoothing, background, colour]");
  report.line("  scale ground: A weight on/off | A on vs off | A vs C (on) | A vs C (off) | A vs B (on) | A vs B (off)   [means over cases]");

  let corpus = Corpus::builtin();
  let ids = [
    "latin-paragraph/13/320",
    "latin-paragraph/17/200",
    "han-zh-hans/17/120",
    "thai/17/160",
    "arabic/17/120",
    "emoji-zwj/17/120",
  ];
  let grounds = [
    Ground::White,
    Ground::Black,
    Ground::ClearOverWhite,
    Ground::ClearOverBlack,
  ];
  let mut rows = Vec::new();

  for scale in scales {
    for ground in grounds {
      let white_ground = matches!(ground, Ground::White | Ground::ClearOverWhite);
      let mut ratio = 0.;
      let mut sums = [(0f64, 0u8); 5];
      let mut count = 0usize;

      for id in ids {
        let Some(variant) = corpus.variant(id)
        else {
          continue;
        };
        let setup = Setup {
          white: ground.white_text(),
          ..Setup::default()
        };
        let tk = textkit::variant(&variant, &setup, 0);
        let (width, height) = tk.record.image_box(variant.width);
        let laid = tuned.lay_out(&variant);
        let runs = tuned_runs(&laid.laid, tuned);
        let a_on = textkit::image(&tk.laid, width, height, *scale, ground, Smoothing(true));
        let a_off = textkit::image(&tk.laid, width, height, *scale, ground, Smoothing(false));
        let c = |smooth: bool, raster: &mut Raster| {
          let canvas = Canvas::for_box(width, height, *scale, ground, Smoothing(smooth));

          raster.draw(&canvas, &runs);
          canvas.to_rgba()
        };
        let c_on = c(true, raster);
        let c_off = c(false, raster);
        let b = vello(&laid.laid, width, height, *scale);
        let b = if ground.white_text() { inverted(&b) } else { b };
        let weight_on = ink_weight(&a_on, white_ground);
        let weight_off = ink_weight(&a_off, white_ground);
        let diffs = [
          InkDiff::of(&a_on, &a_off, white_ground),
          InkDiff::of(&a_on, &c_on, white_ground),
          InkDiff::of(&a_off, &c_off, white_ground),
          InkDiff::of(&a_on, &b, white_ground),
          InkDiff::of(&a_off, &b, white_ground),
        ];

        ratio += if weight_off > 0. {
          weight_on / weight_off
        }
        else {
          1.
        };

        for (at, diff) in diffs.iter().enumerate() {
          sums[at].0 += diff.mean;
          sums[at].1 = sums[at].1.max(diff.max);
        }

        count += 1;
        rows.push(json!({
          "variant": id, "scale": scale, "ground": ground.name(), "weight_on": weight_on, "weight_off": weight_off,
          "a_on_off": diffs[0], "a_c_on": diffs[1], "a_c_off": diffs[2], "a_b_on": diffs[3], "a_b_off": diffs[4],
          "b_weight": ink_weight(&b, white_ground), "c_weight_on": ink_weight(&c_on, white_ground),
        }));

        if let Ok(dir) = std::env::var("PARITY_OUT") {
          let dir = std::path::Path::new(&dir).join("smoothing");
          let stem = format!(
            "{}_{}_{scale}x",
            variant.file_stem(),
            ground.name().replace([' ', '(', ')'], "")
          );
          let _ = std::fs::create_dir_all(&dir);
          let _ = a_on.write(&dir.join(format!("{stem}_a-on.png")));
          let _ = a_off.write(&dir.join(format!("{stem}_a-off.png")));
          let _ = c_on.write(&dir.join(format!("{stem}_c-on.png")));
          let _ = b.write(&dir.join(format!("{stem}_b.png")));
        }
      }

      let n = count.max(1) as f64;
      let cell = |at: usize| format!("mean {:.2} max {}", sums[at].0 / n, sums[at].1);

      report.line(format!(
        "  {scale}x {}: {:.3} | {} | {} | {} | {} | {}",
        ground.name(),
        ratio / n,
        cell(0),
        cell(1),
        cell(2),
        cell(3),
        cell(4)
      ));
    }
  }

  report.put("smoothing", json!(rows));
}
