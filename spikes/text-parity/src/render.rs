//! A layout drawn with vello_cpu: black glyphs on white, colour glyphs in their own colours, at
//! a scale, into the image convention every engine follows (see [`crate::record::image_size`]).

use glifo::FontEmbolden;
use skrifa::{FontRef, MetadataProvider as _, instance::LocationRef};
use vello_cpu::{
  Glyph,
  Pixmap,
  RenderContext,
  Resources,
  kurbo::{Affine, Diagonal2, Rect},
  peniko::Color,
};

use crate::{
  layout::{GlyphRun, Laid},
  record::{MARGIN, image_size},
};

/// How glyphs are drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RenderOptions {
  /// Vertical hinting from the font (vello's only kind); off draws outlines as they are.
  pub hint: bool,
}

/// The per-side outline growth of a synthetic bold at `size`, in pixels: Skia's (and so
/// Chromium's) fake-bold ratio, 1/24 of the size at 9 and below to 1/32 at 36 and above, half on
/// each side.
#[must_use]
pub fn embolden_per_side(size: f32) -> f32 {
  let t = ((size - 9.) / (36. - 9.)).clamp(0., 1.);
  let ratio = (1. / 24.) + t * ((1. / 32.) - (1. / 24.));

  size * ratio / 2.
}

/// Draws `runs` under `transform` with the current paint.
///
/// A synthetic slant shears the scene about the run's baseline rather than going through
/// glifo's `glyph_transform`, which vello_cpu 0.3 draws unslanted under a scaled transform.
fn fill_runs(
  context: &mut RenderContext,
  resources: &mut Resources,
  runs: &[GlyphRun],
  options: RenderOptions,
  (transform, scale): (Affine, f32),
) {
  for run in runs {
    let shear = run.synthesis.skew().map_or(Affine::IDENTITY, |degrees| {
      let baseline = f64::from(run.baseline);

      Affine::translate((0., baseline))
        * Affine::new([1., 0., -f64::from(degrees.to_radians().tan()), 1., 0., 0.])
        * Affine::translate((0., -baseline))
    });

    context.set_transform(transform * shear);

    let mut builder = context
      .glyph_run(resources, &run.font)
      .font_size(run.size)
      .normalized_coords(&run.coords)
      .hint(options.hint);

    if run.synthesis.embolden() {
      let pixels = embolden_per_side(run.size);
      let amount = if options.hint {
        pixels * scale
      }
      else {
        let upem = FontRef::from_index(run.font.data.data(), run.font.index)
          .map(|font| {
            f32::from(
              font
                .metrics(skrifa::instance::Size::unscaled(), LocationRef::default())
                .units_per_em,
            )
          })
          .unwrap_or(1000.);

        pixels * upem / run.size
      };

      builder = builder.font_embolden(FontEmbolden::new(Diagonal2::new(
        f64::from(amount),
        f64::from(amount),
      )));
    }

    let _ = builder.fill_glyphs(run.glyphs.iter().map(|glyph| Glyph {
      id: glyph.id,
      x: glyph.x,
      y: glyph.y,
    }));
  }
}

/// `laid` drawn in a box of `width` by `height` logical pixels at `scale`, with the margin.
///
/// # Panics
///
/// Where the image would be wider or taller than 65535 pixels.
#[must_use]
pub fn render(laid: &Laid, width: f32, height: f32, scale: f32, options: RenderOptions) -> Pixmap {
  render_runs(&laid.glyph_runs(), width, height, scale, options)
}

/// `runs` drawn in a box of `width` by `height` logical pixels at `scale`, with the margin.
///
/// # Panics
///
/// Where the image would be wider or taller than 65535 pixels.
#[must_use]
pub fn render_runs(
  runs: &[GlyphRun],
  width: f32,
  height: f32,
  scale: f32,
  options: RenderOptions,
) -> Pixmap {
  let (pixels_wide, pixels_high) = image_size(width, height, scale);
  let wide = u16::try_from(pixels_wide).expect("an image under 65536 pixels wide");
  let high = u16::try_from(pixels_high).expect("an image under 65536 pixels high");
  let mut context = RenderContext::new(wide, high);
  let mut resources = Resources::new();

  context.set_paint(Color::WHITE);
  context.fill_rect(&Rect::new(0., 0., f64::from(wide), f64::from(high)));
  context.set_paint(Color::BLACK);
  fill_runs(
    &mut context,
    &mut resources,
    runs,
    options,
    (
      Affine::scale(f64::from(scale)) * Affine::translate((f64::from(MARGIN), f64::from(MARGIN))),
      scale,
    ),
  );
  context.flush();

  let mut pixmap = Pixmap::new(wide, high);

  context.render(&mut pixmap, &mut resources);

  pixmap
}

/// Many layouts drawn into one image at scale 1, each at its offset: the 1000-label draw.
///
/// # Panics
///
/// Where the image would be wider or taller than 65535 pixels.
#[must_use]
pub fn render_many(layouts: &[(&Laid, (f32, f32))], width: u16, height: u16) -> (Pixmap, usize) {
  let mut context = RenderContext::new(width, height);
  let mut resources = Resources::new();
  let mut glyphs = 0;

  context.set_paint(Color::WHITE);
  context.fill_rect(&Rect::new(0., 0., f64::from(width), f64::from(height)));
  context.set_paint(Color::BLACK);

  for (laid, (x, y)) in layouts {
    let runs = laid.glyph_runs();

    glyphs += runs.iter().map(|run| run.glyphs.len()).sum::<usize>();
    fill_runs(
      &mut context,
      &mut resources,
      &runs,
      RenderOptions::default(),
      (Affine::translate((f64::from(*x), f64::from(*y))), 1.),
    );
  }

  context.flush();

  let mut pixmap = Pixmap::new(width, height);

  context.render(&mut pixmap, &mut resources);

  (pixmap, glyphs)
}

/// Writes `pixmap` as a PNG.
///
/// # Errors
///
/// Where encoding or writing fails.
pub fn write_png(pixmap: Pixmap, path: &std::path::Path) -> Result<(), String> {
  let png = pixmap.into_png().map_err(|error| error.to_string())?;

  std::fs::write(path, png).map_err(|error| format!("{}: {error}", path.display()))
}
