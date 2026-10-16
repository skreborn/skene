//! Drawing a laid paragraph into Skene's renderer: vello's outlines on every platform (the
//! floor), and on Linux FreeType's masks under the user's settings (the upgrade), each with
//! the shared decoration painter.

use skene_foundation::{
  Color,
  text::{DecorationLine, Laid as _},
  value::TextDecorationStyle,
};
use vello_cpu::{
  Glyph, RenderContext, Resources,
  kurbo::{Affine, BezPath, Rect, Stroke},
  peniko::Color as PenikoColor,
};

use crate::ParleyLaid;

/// A Skene color as vello's.
fn color(color: Color) -> PenikoColor {
  PenikoColor::from_rgba8(color.red, color.green, color.blue, color.alpha)
}

/// Draws `laid`'s glyphs as vello outlines, its top left at `origin`: each glyph in its
/// paint's color, a color glyph (COLR, bitmap) in its own.
pub fn paint_outlines(
  context: &mut RenderContext,
  resources: &mut Resources,
  laid: &ParleyLaid,
  origin: (f64, f64),
) {
  context.set_transform(Affine::translate(origin));

  for run in laid.glyph_runs() {
    let mut start = 0;

    while start < run.glyphs.len() {
      let paint = laid.paint_at(run.glyphs[start].at);
      let end = run.glyphs[start..]
        .iter()
        .position(|glyph| laid.paint_at(glyph.at) != paint)
        .map_or(run.glyphs.len(), |offset| start + offset);

      context.set_paint(color(paint.color));
      context
        .glyph_run(resources, &run.font)
        .font_size(run.size)
        .normalized_coords(&run.coords)
        .hint(false)
        .fill_glyphs(run.glyphs[start..end].iter().map(|glyph| Glyph {
          id: glyph.id,
          x: glyph.x,
          y: glyph.y,
        }))
        .ok();
      start = end;
    }
  }

  paint_decorations(context, laid);
}

/// Draws `laid`'s decorations (underline, line-through, overline, in their styles), through
/// the shared painter's geometry, under the current transform.
pub fn paint_decorations(context: &mut RenderContext, laid: &ParleyLaid) {
  for extent in laid.run_extents() {
    for line in extent.decoration_lines() {
      context.set_paint(color(line.color));
      paint_line(context, &line);
    }
  }
}

fn paint_line(context: &mut RenderContext, line: &DecorationLine) {
  let (x, y, width, thickness) = (
    f64::from(line.x),
    f64::from(line.y),
    f64::from(line.width),
    f64::from(line.thickness),
  );

  match line.style {
    TextDecorationStyle::Solid | TextDecorationStyle::Double => {
      context.fill_rect(&Rect::new(x, y, x + width, y + thickness));
    }
    TextDecorationStyle::Dotted | TextDecorationStyle::Dashed => {
      let (on, off) = if line.style == TextDecorationStyle::Dotted {
        (thickness, thickness)
      } else {
        (thickness * 3., thickness * 2.)
      };
      let mut at = x;

      while at < x + width {
        context.fill_rect(&Rect::new(at, y, (at + on).min(x + width), y + thickness));
        at += on + off;
      }
    }
    TextDecorationStyle::Wavy => {
      let mut path = BezPath::new();
      let step = thickness * 2.;
      let mut at = x;
      let mut up = true;

      path.move_to((at, y + thickness));

      while at < x + width {
        let next = (at + step).min(x + width);

        path.line_to((next, if up { y } else { y + thickness * 2. }));
        at = next;
        up = !up;
      }

      context.set_stroke(Stroke::new(thickness));
      context.stroke_path(&path);
    }
  }
}

#[cfg(target_os = "linux")]
pub use self::masks::{MaskOptions, MaskPainter};

#[cfg(target_os = "linux")]
mod masks {
  use std::collections::HashMap;

  use skrifa::{FontRef, MetadataProvider};
  use vello_cpu::Pixmap;

  use crate::{
    ParleyLaid,
    linux::{Fallback, MaskPixels, Rasterizer, RenderSettings, Request},
  };

  /// How [`MaskPainter::paint_with`] composites: at a scale, cut to a clip, faded.
  #[derive(Clone, Copy, PartialEq, Debug)]
  pub struct MaskOptions {
    /// The pixmap's pixels per layout pixel: glyphs are rasterized at their size times it.
    pub scale: f32,
    /// The pixmap's columns and rows drawn into, left, top, right and bottom (exclusive);
    /// the whole pixmap where `None`.
    pub clip: Option<(i64, i64, i64, i64)>,
    /// How opaque the glyphs are drawn, 0 to 1.
    pub opacity: f32,
  }

  impl Default for MaskOptions {
    fn default() -> Self {
      Self {
        scale: 1.,
        clip: None,
        opacity: 1.,
      }
    }
  }

  /// FreeType's masks composited into a pixmap: every glyph rasterized under the rendering
  /// settings fontconfig gives its font, colored by its paint, a color glyph in its own.
  pub struct MaskPainter {
    rasterizer: Rasterizer,
    settings: HashMap<(u64, u32, u32), RenderSettings>,
    generation: u64,
  }

  impl MaskPainter {
    /// The painter, `None` where FreeType fails to load.
    #[must_use]
    pub fn new() -> Option<Self> {
      Some(Self {
        rasterizer: Rasterizer::new()?,
        settings: HashMap::new(),
        generation: 0,
      })
    }

    /// The rasterizer and its cache.
    #[must_use]
    pub const fn rasterizer(&self) -> &Rasterizer {
      &self.rasterizer
    }

    /// The rasterizer, to draw a mask apart from a paint.
    pub const fn rasterizer_mut(&mut self) -> &mut Rasterizer {
      &mut self.rasterizer
    }

    /// The rendering settings of `font` at `size` pixels, from `fallback`'s fontconfig: asked
    /// with the font's own weight and slant and the size, kept per font and size until the
    /// desktop's settings change.
    pub fn settings_for(
      &mut self,
      font: &parley::FontData,
      size: f32,
      mut fallback: Option<&mut Fallback>,
    ) -> RenderSettings {
      if let Some(fallback) = fallback.as_deref_mut() {
        let generation = fallback.fontconfig().generation();

        if generation != self.generation {
          self.settings.clear();
          self.generation = generation;
        }
      }

      #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a size in quarter pixels"
      )]
      let key = (font.data.id(), font.index, (size * 4.).round() as u32);

      if let Some(settings) = self.settings.get(&key) {
        return *settings;
      }

      let face = FontRef::from_index(font.data.data(), font.index).ok();
      let family = face
        .as_ref()
        .and_then(|face| {
          face
            .localized_strings(skrifa::string::StringId::FAMILY_NAME)
            .english_or_first()
            .map(|name| name.to_string())
        })
        .unwrap_or_default();
      let attributes = face.as_ref().map(MetadataProvider::attributes);
      #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an OpenType weight"
      )]
      let weight = attributes.map_or(400, |attributes| {
        attributes.weight.value().round().clamp(1., 1000.) as u16
      });
      let italic = attributes
        .is_some_and(|attributes| !matches!(attributes.style, skrifa::attribute::Style::Normal));
      let settings = fallback.map_or(RenderSettings::DEFAULT, |fallback| {
        fallback.fontconfig().render_settings(
          &Request {
            family: family.clone(),
            weight,
            italic,
          },
          &family,
          size,
        )
      });

      self.settings.insert(key, settings);

      settings
    }

    /// Composites `laid`'s glyphs into `pixmap` (premultiplied RGBA), its top left at
    /// `origin`, under each font's settings from `fallback`'s fontconfig.
    pub fn paint(
      &mut self,
      pixmap: &mut Pixmap,
      laid: &ParleyLaid,
      origin: (f32, f32),
      fallback: Option<&mut Fallback>,
    ) {
      self.paint_with(pixmap, laid, origin, MaskOptions::default(), fallback);
    }

    /// Composites `laid`'s glyphs into `pixmap` (premultiplied RGBA) as
    /// [`MaskPainter::paint`] does, its top left at `origin` in device pixels, at the
    /// options' scale (the layout's pixels to the pixmap's), cut to their clip and faded by
    /// their opacity.
    pub fn paint_with(
      &mut self,
      pixmap: &mut Pixmap,
      laid: &ParleyLaid,
      origin: (f32, f32),
      options: MaskOptions,
      mut fallback: Option<&mut Fallback>,
    ) {
      let (width, height) = (i64::from(pixmap.width()), i64::from(pixmap.height()));
      let (left_edge, top_edge, right_edge, bottom_edge) =
        options.clip.unwrap_or((0, 0, width, height));
      let (left_edge, top_edge) = (left_edge.max(0), top_edge.max(0));
      let (right_edge, bottom_edge) = (right_edge.min(width), bottom_edge.min(height));
      let fade = options.opacity.clamp(0., 1.);
      let data = pixmap.data_as_u8_slice_mut();

      for run in laid.glyph_runs() {
        let settings =
          self.settings_for(&run.font, run.size * options.scale, fallback.as_deref_mut());

        for glyph in &run.glyphs {
          let x = origin.0 + glyph.x * options.scale;
          let y = origin.1 + glyph.y * options.scale;
          let phase = Rasterizer::phase(x, &settings);
          let Some(mask) = self.rasterizer.mask(
            &run.font,
            &run.coords,
            glyph.id,
            run.size * options.scale,
            phase,
            &settings,
          ) else {
            continue;
          };
          let paint = laid.paint_at(glyph.at).color;
          #[expect(clippy::cast_possible_truncation, reason = "pixel positions")]
          let (left, top) = (
            x.floor() as i64 + i64::from(mask.left),
            y.round() as i64 - i64::from(mask.top),
          );

          for row in 0..i64::from(mask.height) {
            let py = top + row;

            if py < top_edge || py >= bottom_edge {
              continue;
            }

            for column in 0..i64::from(mask.width) {
              let px = left + column;

              if px < left_edge || px >= right_edge {
                continue;
              }

              let source = (row * i64::from(mask.width) + column) as usize;
              let target = ((py * width + px) * 4) as usize;
              #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "0 to 255"
              )]
              let faded = (f32::from(paint.alpha) * fade).round() as u32;
              let pixel = match &mask.pixels {
                MaskPixels::Alpha(alpha) => {
                  let coverage = u32::from(alpha[source]) * faded / 255;

                  [
                    u32::from(paint.red) * coverage / 255,
                    u32::from(paint.green) * coverage / 255,
                    u32::from(paint.blue) * coverage / 255,
                    coverage,
                  ]
                }
                MaskPixels::Color(rgba) => {
                  let at = source * 4;
                  #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "0 to 255"
                  )]
                  let fade = |value: u8| (f32::from(value) * fade).round() as u32;

                  [
                    fade(rgba[at]),
                    fade(rgba[at + 1]),
                    fade(rgba[at + 2]),
                    fade(rgba[at + 3]),
                  ]
                }
              };
              let inverse = 255 - pixel[3];

              for channel in 0..4 {
                let under = u32::from(data[target + channel]);

                data[target + channel] =
                  u8::try_from((pixel[channel] + under * inverse / 255).min(255)).unwrap_or(255);
              }
            }
          }
        }
      }
    }
  }
}
