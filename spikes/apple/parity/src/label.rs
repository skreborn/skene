//! The platform's own label beside TextKit 2: `NSTextField` (macOS, rendered with
//! `cacheDisplayInRect:`) or `UILabel` (iOS, its layer rendered), given the variant's
//! attributed text and drawn by the image convention, so B and C can be held against the
//! control an app's other text is drawn by.
#![allow(unused_unsafe)]

use objc2::MainThreadMarker;
#[allow(unused_imports)]
use text_parity::{corpus::Variant, image::Rgba, record::MARGIN};

#[allow(unused_imports)]
use crate::{
  cg::{Canvas, Ground, Smoothing},
  textkit::{self, Setup},
};

/// The label's image for `variant` in a box of `width` by `height` at `scale`, its text placed
/// where TextKit's node draws it.
#[cfg(target_os = "ios")]
#[must_use]
pub fn image(
  mtm: MainThreadMarker,
  variant: &Variant<'_>,
  width: f32,
  height: f32,
  scale: f32,
) -> Rgba {
  use objc2_core_foundation::{CGPoint, CGRect, CGSize};
  use objc2_ui_kit::UILabel;

  let attributed = textkit::attributed(variant, &Setup::default());
  let label = UILabel::new(mtm);

  label.setAttributedText(Some(&attributed));
  label.setNumberOfLines(variant.case.max_lines.map_or(0, |lines| lines as isize));

  let fitted = label.sizeThatFits(CGSize::new(f64::from(width), 1.0e6));

  label.setFrame(CGRect::new(
    CGPoint::new(0., 0.),
    CGSize::new(f64::from(width), fitted.height),
  ));
  label.layoutIfNeeded();

  let canvas = Canvas::for_box(width, height, scale, Ground::White, Smoothing(true));

  canvas.flip_to_layout();
  crate::platform::with_context(&canvas.context, || {
    label.layer().renderInContext(&canvas.context)
  });

  canvas.to_rgba()
}

/// The label's image for `variant` in a box of `width` by `height` at `scale`, its text placed
/// where TextKit's node draws it (the field's 2-point inset taken off).
#[cfg(target_os = "macos")]
#[must_use]
pub fn image(
  mtm: MainThreadMarker,
  variant: &Variant<'_>,
  width: f32,
  height: f32,
  scale: f32,
) -> Rgba {
  use objc2::AnyThread;
  use objc2_app_kit::{NSBitmapImageRep, NSDeviceRGBColorSpace, NSTextField};
  use objc2_foundation::{NSPoint, NSRect, NSSize};

  let attributed = textkit::attributed(variant, &Setup::default());
  let field = NSTextField::labelWithAttributedString(&attributed, mtm);
  let lines = variant.case.max_lines.map_or(0, |lines| lines as isize);

  field.setMaximumNumberOfLines(lines);
  field.setPreferredMaxLayoutWidth(f64::from(width));

  if let Some(cell) = field.cell() {
    cell.setWraps(true);
  }

  let inset = 2.;
  let size = NSSize::new(
    f64::from(width) + 2. * inset,
    f64::from(height).max(field.fittingSize().height),
  );

  field.setFrame(NSRect::new(NSPoint::new(0., 0.), size));

  let pixels_wide = (size.width * f64::from(scale)).round() as isize;
  let pixels_high = (size.height * f64::from(scale)).round() as isize;
  let rep = unsafe {
    NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
      NSBitmapImageRep::alloc(),
      std::ptr::null_mut(),
      pixels_wide.max(1),
      pixels_high.max(1),
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
  field.cacheDisplayInRect_toBitmapImageRep(field.bounds(), &rep);

  let data = rep.bitmapData();
  let row = rep.bytesPerRow() as usize;
  let (out_wide, out_high) = text_parity::record::image_size(width, height, scale);
  let mut out = Rgba::white(out_wide, out_high);
  let left = ((MARGIN - inset as f32) * scale).round() as i64;
  let top = (MARGIN * scale).round() as i64;

  for y in 0..pixels_high.max(1) as usize {
    for x in 0..pixels_wide.max(1) as usize {
      let (tx, ty) = (x as i64 + left, y as i64 + top);

      if tx < 0 || ty < 0 || tx >= i64::from(out_wide) || ty >= i64::from(out_high) {
        continue;
      }

      let at = unsafe { data.add(y * row + x * 4) };
      let alpha = u32::from(unsafe { *at.add(3) });
      let index = (ty as usize * out_wide as usize + tx as usize) * 4;

      for channel in 0..3 {
        let value = u32::from(unsafe { *at.add(channel) }) + (255 * (255 - alpha) + 127) / 255;

        out.data[index + channel] = value.min(255) as u8;
      }
    }
  }

  out
}
