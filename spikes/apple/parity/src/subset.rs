//! A corpus of our own (cases of the built-in corpus at other sizes, in other fonts) run through
//! A, stock B, tuned B and tuned C, and compared: the emoji sizes, each Dynamic Type category,
//! Bold Text.

use std::path::Path;

use serde_json::{Value, json};
use text_parity::{
  compare::{self, Options as CompareOptions},
  corpus::{Case, Corpus, Variant},
  layout::ParleyEngine,
  record::{RecordSet, VariantTimings, scale_key},
  render::{self, RenderOptions},
};

use crate::{
  cg::{Canvas, Ground, Smoothing},
  probes::Report,
  raster::Raster,
  sets::{Context, image_path, tuned_runs},
  textkit::{self, Setup},
  tuned::TunedEngine,
};

/// Cases of the built-in corpus renamed with `suffix`, each at `size` and `width`.
#[must_use]
pub fn cases(ids: &[(&str, Option<f32>)], suffix: &str, sizes: &[f32]) -> Vec<Case> {
  let corpus = Corpus::builtin();

  ids
    .iter()
    .filter_map(|(id, width)| {
      let mut case = corpus.case(id)?.clone();

      case.id = format!("{id}-{suffix}");
      case.sizes = sizes.to_vec();
      case.widths = vec![*width];

      Some(case)
    })
    .collect()
}

/// The four sets of `corpus` under `dir`.
pub struct Sets {
  /// TextKit 2.
  pub a: RecordSet,
  /// Stock Parley and vello.
  pub b: RecordSet,
  /// Tuned Parley and vello.
  pub bt: RecordSet,
  /// Tuned Parley drawn by Core Text.
  pub ct: RecordSet,
}

/// Runs `corpus` through every engine, A set up by `setup`.
#[expect(clippy::too_many_arguments, reason = "a spike's driver")]
pub fn run(
  context: &Context,
  corpus: &Corpus,
  dir: &Path,
  setup: &dyn Fn(&Variant<'_>) -> Setup,
  weight: &dyn Fn(&Case) -> u16,
  line_height: Option<f32>,
  (tuned_b, tuned_c): (&mut TunedEngine, &mut TunedEngine),
  raster: &mut Raster,
  stock: &mut ParleyEngine,
) -> Sets {
  let saved = (tuned_b.tuning.line_height, tuned_c.tuning.line_height);

  tuned_b.tuning.line_height = line_height;
  tuned_c.tuning.line_height = line_height;

  let names = ["a", "b", "bt", "ct"];

  for name in names {
    let _ = std::fs::create_dir_all(dir.join(name).join("png"));
  }

  let mut sets = Sets {
    a: context.set("textkit2", &context.os_version, "coregraphics", &[]),
    b: context.set(
      "parley",
      "0.12.0",
      "vello_cpu 0.3.0",
      &[("fonts", "fontique system")],
    ),
    bt: context.set("parley-tuned", "0.12.0", "vello_cpu 0.3.0", &[]),
    ct: context.set("parley-tuned", "0.12.0", "coretext CTFontDrawGlyphs", &[]),
  };

  let mut hvgl = crate::outline::HvglFonts::default();

  for variant in corpus.variants() {
    let tk = textkit::variant(&variant, &setup(&variant), 0);
    let mut a = tk.record;
    let (width, height) = a.image_box(variant.width);

    for scale in &context.scales {
      let file = image_path(&variant, *scale);
      let image = textkit::image(
        &tk.laid,
        width,
        height,
        *scale,
        Ground::White,
        Smoothing(true),
      );
      let _ = image.write(&dir.join("a").join(&file));

      a.images.insert(scale_key(*scale), file);
    }

    sets.a.records.push(a);

    let laid = stock.lay_out(&variant);
    let intrinsic = stock.shape(&variant).intrinsic();
    let mut b = laid.record(&variant, &mut stock.fonts, Some(intrinsic), false);
    let (width, height) = b.image_box(variant.width);

    for scale in &context.scales {
      let file = image_path(&variant, *scale);
      let _ = render::write_png(
        render::render(&laid, width, height, *scale, RenderOptions::default()),
        &dir.join("b").join(&file),
      );

      b.images.insert(scale_key(*scale), file);
    }

    sets.b.records.push(b);

    let mut heavier = variant.case.clone();

    heavier.weight = weight(variant.case);

    let tuned_variant = Variant {
      case: &heavier,
      size: variant.size,
      width: variant.width,
    };

    for (engine, vello) in [(&mut *tuned_b, true), (&mut *tuned_c, false)] {
      let laid = engine.lay_out(&tuned_variant);
      let mut record = engine.record(&tuned_variant, &laid, None);

      record.timings = Some(VariantTimings::default());

      let (width, height) = record.image_box(variant.width);
      let runs = if vello {
        Vec::new()
      }
      else {
        tuned_runs(&laid.laid, engine)
      };
      let name = if vello { "bt" } else { "ct" };
      let outlined = if vello {
        crate::sets::hvgl_glyphs(&laid.laid, engine, raster, &mut hvgl)
      }
      else {
        Vec::new()
      };

      for scale in &context.scales {
        let file = image_path(&variant, *scale);

        if vello {
          let pixmap =
            crate::sets::tuned_vello(&laid.laid, engine, &outlined, (width, height), *scale);

          let _ = render::write_png(pixmap, &dir.join(name).join(&file));
        }
        else {
          let canvas = Canvas::for_box(width, height, *scale, Ground::White, Smoothing(true));

          raster.draw(&canvas, &runs);

          let _ = canvas.to_rgba().write(&dir.join(name).join(&file));
        }

        record.images.insert(scale_key(*scale), file);
      }

      record.glyph_runs.clear();

      if vello {
        sets.bt.records.push(record);
      }
      else {
        sets.ct.records.push(record);
      }
    }
  }

  tuned_b.tuning.line_height = saved.0;
  tuned_c.tuning.line_height = saved.1;

  for (name, set) in names.iter().zip([&sets.a, &sets.b, &sets.bt, &sets.ct]) {
    let _ = set.write(&dir.join(name).join("records.json"));
  }

  let _ = std::fs::write(
    dir.join("corpus.json"),
    serde_json::to_string_pretty(corpus).unwrap_or_default(),
  );

  sets
}

/// The comparisons of A with each other set, as one summary line and JSON.
pub fn summarize(
  report: &mut Report,
  label: &str,
  corpus: &Corpus,
  dir: &Path,
  sets: &Sets,
) -> Value {
  let options = CompareOptions::default();
  let mut out = serde_json::Map::new();
  let mut cells = Vec::new();

  for (name, set) in [("b", &sets.b), ("bt", &sets.bt), ("ct", &sets.ct)] {
    let comparison = compare::compare(
      corpus,
      (&sets.a, &dir.join("a")),
      (set, &dir.join(name)),
      &options,
    );
    let totals = &comparison.totals;

    cells.push(format!(
      "A-{name}: breaks {}/{} heights±{} {}/{} family {}/{} SSIM {:.3} MAD {:.2}",
      totals.breaks_identical,
      totals.variants,
      options.tolerance,
      totals.heights_within,
      totals.variants,
      totals.same_family,
      totals.characters,
      totals.mean_ssim,
      totals.mean_abs_diff
    ));
    let _ = std::fs::write(
      dir.join(format!("a-{name}.md")),
      comparison.markdown(corpus),
    );
    out.insert(
      name.to_owned(),
      serde_json::to_value(&comparison.totals).unwrap_or_default(),
    );
    out.insert(
      format!("{name}_variants"),
      json!(comparison.variants.iter().map(|variant| json!({
        "variant": variant.variant, "breaks": variant.breaks_identical, "lines": variant.lines,
        "height_delta": variant.height_delta, "first_baseline_delta": variant.first_baseline_delta,
        "width_delta": variant.max_width_delta, "fonts": [variant.fonts.same_family, variant.fonts.characters],
        "pixels": variant.pixels,
      })).collect::<Vec<_>>()),
    );
  }

  report.line(format!("  {label}: {}", cells.join(" | ")));

  Value::Object(out)
}
