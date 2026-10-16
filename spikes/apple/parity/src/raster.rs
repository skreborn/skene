//! Option C: Parley's glyph runs drawn by Core Text, `CTFontDrawGlyphs` into a Core Graphics
//! bitmap, the rasterizer TextKit itself draws through.
//!
//! A run's font is opened from its file (`CTFontManagerCreateFontDescriptorsFromURL`, the face
//! picked by PostScript name) or taken from the Core Text font it was registered from, and set
//! to exactly Parley's instance: every axis's user value recovered from the run's normalized
//! coordinates, so Core Text's automatic optical size never draws another instance than Parley
//! shaped. A synthetic bold is a fill-and-stroke of Skia's width, a synthetic slant a shear per
//! glyph about its origin.

use std::{collections::HashMap, ptr::NonNull};

use objc2_core_foundation::{
  CFArray,
  CFNumber,
  CFRetained,
  CFString,
  CFURL,
  CGAffineTransform,
  CGPoint,
};
use objc2_core_graphics::{CGContext, CGGlyph, CGTextDrawingMode};
use objc2_core_text::{
  CTFont,
  CTFontDescriptor,
  CTFontManagerCreateFontDescriptorsFromURL,
  kCTFontNameAttribute,
};
use skrifa::{FontRef, MetadataProvider as _, Tag};
use text_parity::{record::MARGIN, render::embolden_per_side};

use crate::{cg::Canvas, coretext};

/// Where a run's Core Text font comes from.
#[derive(Clone)]
pub enum Source {
  /// A font file and the face in it.
  File {
    /// The file.
    path: String,
    /// The face's PostScript name, which picks it among the file's faces.
    postscript: String,
    /// The face's index, where the name finds none.
    index: u32,
  },
  /// The Core Text font the run's font was registered from.
  Font(CFRetained<CTFont>),
}

/// One run to draw.
pub struct DrawRun<'a> {
  /// The font's source.
  pub source: Source,
  /// A key naming the source (its file and face, or its registration).
  pub key: String,
  /// The font's bytes, for its axes.
  pub data: &'a [u8],
  /// The face index in `data`.
  pub index: u32,
  /// The size in points.
  pub size: f32,
  /// Parley's normalized coordinates (F2Dot14 bits), in the font's axis order.
  pub coords: Vec<i16>,
  /// A synthetic bold.
  pub embolden: bool,
  /// A synthetic slant, in degrees.
  pub skew: Option<f32>,
  /// The glyphs: id, origin x and baseline y in the layout.
  pub glyphs: Vec<(u32, f32, f32)>,
}

/// Every axis's user value for `coords`, found by bisection through the font's own
/// normalization (`avar` included).
#[must_use]
pub fn user_values(data: &[u8], index: u32, coords: &[i16]) -> Vec<(Tag, f32)> {
  let Ok(font) = FontRef::from_index(data, index)
  else {
    return Vec::new();
  };
  let axes = font.axes();
  let mut out = Vec::new();

  for (at, axis) in axes.iter().enumerate() {
    let target = coords.get(at).copied().unwrap_or(0);

    if target == 0 {
      out.push((axis.tag(), axis.default_value()));

      continue;
    }

    let normalized = |value: f32| -> i16 {
      axes
        .location([(axis.tag(), value)])
        .coords()
        .get(at)
        .map_or(0, |coord| coord.to_bits())
    };
    let (mut low, mut high) = (axis.min_value(), axis.max_value());

    for _ in 0..40 {
      let middle = (low + high) / 2.;

      if normalized(middle) < target {
        low = middle;
      }
      else {
        high = middle;
      }
    }

    out.push((axis.tag(), (low + high) / 2.));
  }

  out
}

/// Opens fonts and draws runs, caching each font by source, size and instance.
#[derive(Default)]
pub struct Raster {
  descriptors: HashMap<String, Option<CFRetained<CTFontDescriptor>>>,
  fonts: HashMap<String, CFRetained<CTFont>>,
  /// What went wrong, once per key.
  pub problems: Vec<String>,
  /// Fonts whose instance Core Text reports differently from the one asked.
  pub instance_mismatches: Vec<String>,
}

impl Raster {
  fn descriptor(&mut self, run: &DrawRun<'_>) -> Option<CFRetained<CTFontDescriptor>> {
    if let Some(found) = self.descriptors.get(&run.key) {
      return found.clone();
    }

    let found = match &run.source {
      Source::Font(font) => Some(unsafe { font.font_descriptor() }),
      Source::File {
        path,
        postscript,
        index,
      } => CFURL::from_file_path(path).and_then(|url| {
        let array = unsafe { CTFontManagerCreateFontDescriptorsFromURL(&url) }?;
        let array: &CFArray<CTFontDescriptor> = unsafe { array.cast_unchecked() };
        let all: Vec<CFRetained<CTFontDescriptor>> = array.iter().collect();
        let named = all.iter().find(|descriptor| {
          unsafe { descriptor.attribute(kCTFontNameAttribute) }
            .and_then(|name| name.downcast::<CFString>().ok())
            .is_some_and(|name| name.to_string() == *postscript)
        });

        named.cloned().or_else(|| all.get(*index as usize).cloned())
      }),
    };

    if found.is_none() {
      self
        .problems
        .push(format!("{}: no Core Text descriptor", run.key));
    }

    self.descriptors.insert(run.key.clone(), found.clone());

    found
  }

  /// The Core Text font of `run`, at its size and instance.
  pub fn font(&mut self, run: &DrawRun<'_>) -> Option<CFRetained<CTFont>> {
    let key = format!("{}@{}@{:?}", run.key, run.size, run.coords);

    if let Some(font) = self.fonts.get(&key) {
      return Some(font.clone());
    }

    let mut descriptor = self.descriptor(run)?;
    let values = user_values(run.data, run.index, &run.coords);

    for (tag, value) in &values {
      let identifier = CFNumber::new_i64(i64::from(u32::from_be_bytes(tag.to_be_bytes())));

      descriptor = unsafe { descriptor.copy_with_variation(&identifier, f64::from(*value)) };
    }

    let font =
      unsafe { CTFont::with_font_descriptor(&descriptor, f64::from(run.size), std::ptr::null()) };

    if !values.is_empty() {
      let got = coretext::variation(&font);
      let differs = values.iter().any(|(tag, value)| {
        let name = tag.to_string();

        got
          .iter()
          .find(|(got, _)| *got == name)
          .is_some_and(|(_, got)| (got - f64::from(*value)).abs() > 0.05)
      });

      if differs && self.instance_mismatches.len() < 20 {
        self.instance_mismatches.push(format!(
          "{} at {}: asked {:?}, Core Text has {:?}",
          run.key,
          run.size,
          values
            .iter()
            .map(|(tag, value)| format!("{tag}={value:.2}"))
            .collect::<Vec<_>>(),
          got
        ));
      }
    }

    self.fonts.insert(key, font.clone());

    Some(font)
  }

  /// Draws `runs` into `canvas` (y up, points), the layout's top left at the margin.
  pub fn draw(&mut self, canvas: &Canvas, runs: &[DrawRun<'_>]) {
    let context: &CGContext = &canvas.context;
    let height = canvas.height_points();
    let shade = if canvas.ground.white_text() { 1. } else { 0. };

    CGContext::save_g_state(Some(context));
    canvas.points_up();
    canvas.identity_text();
    CGContext::set_rgb_fill_color(Some(context), shade, shade, shade, 1.);
    CGContext::set_rgb_stroke_color(Some(context), shade, shade, shade, 1.);

    for run in runs {
      if run.glyphs.is_empty() {
        continue;
      }

      let Some(font) = self.font(run)
      else {
        continue;
      };

      if run.embolden {
        CGContext::set_text_drawing_mode(Some(context), CGTextDrawingMode::FillStroke);
        CGContext::set_line_width(Some(context), f64::from(2. * embolden_per_side(run.size)));
      }
      else {
        CGContext::set_text_drawing_mode(Some(context), CGTextDrawingMode::Fill);
      }

      let mut ids: Vec<CGGlyph> = run.glyphs.iter().map(|glyph| glyph.0 as CGGlyph).collect();
      let mut positions: Vec<CGPoint> = run
        .glyphs
        .iter()
        .map(|glyph| {
          CGPoint::new(
            f64::from(MARGIN + glyph.1),
            height - f64::from(MARGIN + glyph.2),
          )
        })
        .collect();

      match run.skew {
        Some(degrees) => {
          let slant = f64::from(degrees.to_radians().tan());

          for (id, at) in ids.iter_mut().zip(positions.iter()) {
            let mut origin = CGPoint::new(0., 0.);

            CGContext::save_g_state(Some(context));
            CGContext::translate_ctm(Some(context), at.x, at.y);
            CGContext::concat_ctm(
              Some(context),
              CGAffineTransform {
                a: 1.,
                b: 0.,
                c: slant,
                d: 1.,
                tx: 0.,
                ty: 0.,
              },
            );
            unsafe { font.draw_glyphs(NonNull::from(id), NonNull::from(&mut origin), 1, context) };
            CGContext::restore_g_state(Some(context));
          }
        }
        None => unsafe {
          font.draw_glyphs(
            NonNull::new_unchecked(ids.as_mut_ptr()),
            NonNull::new_unchecked(positions.as_mut_ptr()),
            ids.len(),
            context,
          );
        },
      }
    }

    CGContext::restore_g_state(Some(context));
  }
}
