//! Glyphs whose outlines only Core Text reads (an `hvgl` font's: PingFang UI on iOS 26 and
//! macOS 15) filled by vello from `CTFontCreatePathForGlyph`, over an image vello drew of the
//! rest: B's drawing where the font's own tables hold no outline vello can read.

use std::{collections::HashMap, ffi::c_void, ptr::NonNull};

use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{CGGlyph, CGPath, CGPathElement, CGPathElementType};
use objc2_core_text::CTFont;
use text_parity::record::{MARGIN, image_size};
use vello_cpu::{
  Pixmap,
  RenderContext,
  Resources,
  kurbo::{Affine, BezPath, Point},
  peniko::Color,
};

use crate::coretext;

/// One glyph to fill: its font at its size and instance, its id, its origin x and baseline y in
/// the layout.
pub struct Outlined {
  /// The font.
  pub font: CFRetained<CTFont>,
  /// The glyph.
  pub id: u16,
  /// The origin's x.
  pub x: f64,
  /// The baseline's y, down from the layout's top.
  pub y: f64,
}

unsafe extern "C-unwind" fn element(info: *mut c_void, element: NonNull<CGPathElement>) {
  let path = unsafe { &mut *info.cast::<BezPath>() };
  let element = unsafe { element.as_ref() };
  let points = element.points.as_ptr();
  let at = |index: usize| {
    let point = unsafe { *points.add(index) };

    Point::new(point.x, point.y)
  };

  match element.r#type {
    CGPathElementType::MoveToPoint => path.move_to(at(0)),
    CGPathElementType::AddLineToPoint => path.line_to(at(0)),
    CGPathElementType::AddQuadCurveToPoint => path.quad_to(at(0), at(1)),
    CGPathElementType::AddCurveToPoint => path.curve_to(at(0), at(1), at(2)),
    CGPathElementType::CloseSubpath => path.close_path(),
    _ => {}
  }
}

/// `path` as a kurbo path, in its own coordinates (y up).
#[must_use]
pub fn bez(path: &CGPath) -> BezPath {
  let mut out = BezPath::new();

  unsafe { CGPath::apply(Some(path), (&raw mut out).cast(), Some(element)) };

  out
}

/// Which fonts have `hvgl` outlines, by PostScript name.
#[derive(Default)]
pub struct HvglFonts(HashMap<String, bool>);

impl HvglFonts {
  /// Whether `font` has `hvgl` outlines.
  pub fn has(&mut self, font: &CTFont) -> bool {
    let name = unsafe { font.post_script_name() }.to_string();

    *self
      .0
      .entry(name)
      .or_insert_with(|| coretext::tables(font).contains(&coretext::tag_of("hvgl")))
  }
}

/// Fills `glyphs` black over `pixmap`, an image of a `width` by `height` layout at `scale` with
/// the margin, each from Core Text's outline; returns how many had an outline.
pub fn fill(
  pixmap: &mut Pixmap,
  glyphs: &[Outlined],
  width: f32,
  height: f32,
  scale: f32,
) -> usize {
  if glyphs.is_empty() {
    return 0;
  }

  let (wide, high) = image_size(width, height, scale);
  let (wide, high) = (
    wide.min(u32::from(u16::MAX)) as u16,
    high.min(u32::from(u16::MAX)) as u16,
  );
  let mut context = RenderContext::new(wide, high);
  let mut resources = Resources::new();
  let mut filled = 0;

  context.set_paint(Color::BLACK);

  for glyph in glyphs {
    let Some(path) = (unsafe {
      glyph
        .font
        .path_for_glyph(glyph.id as CGGlyph, std::ptr::null())
    })
    else {
      continue;
    };

    context.set_transform(
      Affine::scale(f64::from(scale))
        * Affine::translate((f64::from(MARGIN) + glyph.x, f64::from(MARGIN) + glyph.y))
        * Affine::FLIP_Y,
    );
    context.fill_path(&bez(&path));
    filled += 1;
  }

  context.flush();

  let mut layer = Pixmap::new(wide, high);

  context.render(&mut layer, &mut resources);

  let coverage = layer.data_as_u8_slice();
  let base = pixmap.data_as_u8_slice_mut();

  for (pixel, over) in base.chunks_exact_mut(4).zip(coverage.chunks_exact(4)) {
    let alpha = u32::from(over[3]);

    for channel in &mut pixel[..3] {
      *channel = ((u32::from(*channel) * (255 - alpha) + 127) / 255) as u8;
    }
  }

  filled
}
