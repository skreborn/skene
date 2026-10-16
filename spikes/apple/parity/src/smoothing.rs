//! Rasterization alone, under each drawing condition: font smoothing on and off, opaque and
//! transparent backgrounds, black on white and white on black, at two scales. TextKit's own
//! glyphs at its own positions (see [`crate::isolate`]) drawn by TextKit (A), by
//! `CTFontDrawGlyphs` (C's rasterizer) and by vello (B's; inverted for white text, since
//! vello's coverage has no colour dependence), so layout cannot account for a difference.

use serde_json::json;
use text_parity::corpus::Corpus;

use crate::{
  cg::{Ground, InkDiff, Smoothing, ink_weight, inverted},
  isolate::{self, Bytes},
  probes::Report,
  textkit::{self, Setup},
};

/// The study, at `scales`.
pub fn study(report: &mut Report, scales: &[f32]) {
  report.line("\n[rasterization alone, TextKit's glyphs at TextKit's positions, by condition]");
  report.line(
    "  scale ground: A ink on/off | A on vs off | A vs CG on | A vs CG off | A vs vello (A on) | A vs vello (A off) | vello ink / A ink   [means over cases]",
  );

  let corpus = Corpus::builtin();
  let ids = [
    "latin-paragraph/13/320",
    "latin-paragraph/17/200",
    "sizes-latin/34/max",
    "han-ja/17/max",
    "thai/17/160",
    "arabic/17/120",
    "emoji-zwj/17/max",
  ];
  let grounds = [
    Ground::White,
    Ground::Black,
    Ground::ClearOverWhite,
    Ground::ClearOverBlack,
  ];
  let mut bytes = Bytes::default();
  let mut rows = Vec::new();

  for scale in scales {
    for ground in grounds {
      let white_ground = matches!(ground, Ground::White | Ground::ClearOverWhite);
      let mut ratio = 0.;
      let mut vello_ratio = 0.;
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
        let placed = isolate::placed(&tk.laid, variant.case, tk.record.truncated);
        let a_on = textkit::image(&tk.laid, width, height, *scale, ground, Smoothing(true));
        let a_off = textkit::image(&tk.laid, width, height, *scale, ground, Smoothing(false));
        let cg_on = isolate::draw_cg(&placed, width, height, *scale, ground, Smoothing(true));
        let cg_off = isolate::draw_cg(&placed, width, height, *scale, ground, Smoothing(false));
        let vello = isolate::draw_vello(&placed, &mut bytes, width, height, *scale);
        let vello = if ground.white_text() {
          inverted(&vello)
        }
        else {
          vello
        };
        let weight_on = ink_weight(&a_on, white_ground);
        let weight_off = ink_weight(&a_off, white_ground);
        let weight_vello = ink_weight(&vello, white_ground);
        let diffs = [
          InkDiff::of(&a_on, &a_off, white_ground),
          InkDiff::of(&a_on, &cg_on, white_ground),
          InkDiff::of(&a_off, &cg_off, white_ground),
          InkDiff::of(&a_on, &vello, white_ground),
          InkDiff::of(&a_off, &vello, white_ground),
        ];

        ratio += if weight_off > 0. {
          weight_on / weight_off
        }
        else {
          1.
        };
        vello_ratio += if weight_on > 0. {
          weight_vello / weight_on
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
          "variant": id, "scale": scale, "ground": ground.name(), "weight_on": weight_on,
          "weight_off": weight_off, "weight_vello": weight_vello, "a_on_off": diffs[0],
          "a_cg_on": diffs[1], "a_cg_off": diffs[2], "a_vello_on": diffs[3], "a_vello_off": diffs[4],
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
          let _ = cg_on.write(&dir.join(format!("{stem}_cg-on.png")));
          let _ = vello.write(&dir.join(format!("{stem}_vello.png")));
        }
      }

      let n = count.max(1) as f64;
      let cell = |at: usize| format!("mean {:.2} max {}", sums[at].0 / n, sums[at].1);

      report.line(format!(
        "  {scale}x {}: {:.3} | {} | {} | {} | {} | {} | {:.3}",
        ground.name(),
        ratio / n,
        cell(0),
        cell(1),
        cell(2),
        cell(3),
        cell(4),
        vello_ratio / n
      ));
    }
  }

  report.put("smoothing", json!(rows));
}
