//! Genmoji without Apple Intelligence: a custom `CTAdaptiveImageProviding` object (an image it
//! draws itself) on U+FFFC as `kCTAdaptiveImageProviderAttributeName`, measured by
//! `CTFontGetTypographicBoundsForAdaptiveImageProvider`, by Core Text's own line, and by
//! TextKit 2; then laid out by Parley as an inline box of those bounds and drawn by
//! `CTFontDrawImageFromAdaptiveImageProviderAtPoint` (C) or composited from the provider's
//! image (B).
#![allow(unused_unsafe)]

use std::{borrow::Cow, cell::RefCell};

use objc2::{
  AnyThread,
  DefinedClass,
  define_class,
  msg_send,
  rc::Retained,
  runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject},
};
use objc2_core_foundation::{CFRetained, CGFloat, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
  CGBitmapContextCreate,
  CGBitmapContextCreateImage,
  CGColorSpace,
  CGContext,
  CGImage,
  CGImageAlphaInfo,
};
use objc2_core_text::{CTAdaptiveImageProviding, kCTAdaptiveImageProviderAttributeName};
use objc2_foundation::{NSMutableAttributedString, NSRange, NSString};
use parley::{
  FontFamily,
  FontFamilyName,
  InlineBox,
  InlineBoxKind,
  LayoutContext,
  PositionedLayoutItem,
  StyleProperty,
  VerticalAlign,
};
use serde_json::json;
use text_parity::{
  bench::plain_case,
  image::Rgba,
  layout::Laid,
  record::MARGIN,
  render::{self, RenderOptions},
};

use crate::{
  cg::{Canvas, Ground, InkDiff, Smoothing},
  platform::{NSFontAttributeName, NSForegroundColorAttributeName, as_ct, gray, system_font},
  probes::Report,
  raster::Raster,
  sets::tuned_runs,
  textkit,
  tuned::TunedEngine,
};

/// The provider's state: the images it handed out (kept alive) and the sizes it was asked for.
pub struct Ivars {
  images: RefCell<Vec<CFRetained<CGImage>>>,
  /// Each request: proposed size and scale.
  pub asked: RefCell<Vec<(f64, f64, f64)>>,
}

/// Draws a disc in a square of `pixels`: the stand-in for a Genmoji's image.
#[must_use]
pub fn disc(pixels: usize) -> Option<CFRetained<CGImage>> {
  let space = CGColorSpace::new_device_rgb()?;
  let context = unsafe {
    CGBitmapContextCreate(
      std::ptr::null_mut(),
      pixels,
      pixels,
      8,
      pixels * 4,
      Some(&space),
      CGImageAlphaInfo::PremultipliedLast.0,
    )
  }?;
  let side = pixels as f64;

  CGContext::set_rgb_fill_color(Some(&context), 0.85, 0.2, 0.3, 1.);
  CGContext::fill_ellipse_in_rect(
    Some(&context),
    CGRect::new(
      CGPoint::new(side * 0.05, side * 0.05),
      CGSize::new(side * 0.9, side * 0.9),
    ),
  );
  CGContext::set_rgb_fill_color(Some(&context), 1., 0.9, 0.2, 1.);
  CGContext::fill_ellipse_in_rect(
    Some(&context),
    CGRect::new(
      CGPoint::new(side * 0.3, side * 0.3),
      CGSize::new(side * 0.4, side * 0.4),
    ),
  );

  CGBitmapContextCreateImage(Some(&context))
}

define_class!(
  // SAFETY: NSObject has no subclassing requirements, and the class has no Drop.
  #[unsafe(super(NSObject))]
  #[name = "SkeneParityAdaptiveImage"]
  #[ivars = Ivars]
  /// A provider of a Genmoji-like image.
  pub struct AdaptiveImage;

  unsafe impl NSObjectProtocol for AdaptiveImage {}

  unsafe impl CTAdaptiveImageProviding for AdaptiveImage {
    #[unsafe(method(imageForProposedSize:scaleFactor:imageOffset:imageSize:))]
    unsafe fn image_for(
      &self,
      proposed: CGSize,
      scale: CGFloat,
      offset: *mut CGPoint,
      size: *mut CGSize,
    ) -> *const CGImage {
      self
        .ivars()
        .asked
        .borrow_mut()
        .push((proposed.width, proposed.height, scale));

      let side = proposed.width.max(proposed.height).max(1.);
      let Some(image) = disc((side * scale).round().max(1.) as usize)
      else {
        return std::ptr::null();
      };

      unsafe {
        if !offset.is_null() {
          *offset = CGPoint::new(0., 0.);
        }

        if !size.is_null() {
          *size = CGSize::new(side, side);
        }
      }

      let pointer: *const CGImage = &*image;

      self.ivars().images.borrow_mut().push(image);

      pointer
    }
  }
);

impl AdaptiveImage {
  /// A new provider.
  #[must_use]
  pub fn new() -> Retained<Self> {
    let this = Self::alloc().set_ivars(Ivars {
      images: RefCell::new(Vec::new()),
      asked: RefCell::new(Vec::new()),
    });

    unsafe { msg_send![super(this), init] }
  }
}

/// The probe: bounds by Core Text, TextKit and Parley, and the drawings compared.
pub fn probe(report: &mut Report, tuned: &mut TunedEngine, raster: &mut Raster) {
  report.line("\n[Genmoji: a custom CTAdaptiveImageProviding on U+FFFC]");

  let provider = AdaptiveImage::new();
  let protocol: &ProtocolObject<dyn CTAdaptiveImageProviding> =
    ProtocolObject::from_ref(&*provider);
  let size = 17.;
  let font = system_font(size, 400, false);
  let ct = as_ct(&font);
  let bounds = unsafe { ct.typographic_bounds_for_adaptive_image_provider(Some(protocol)) };

  report.line(format!(
    "  CTFontGetTypographicBoundsForAdaptiveImageProvider at {size}pt: origin ({:.2}, {:.2}) size {:.2} x {:.2}",
    bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height
  ));

  let text = "A\u{FFFC}B";
  let string = NSString::from_str(text);
  let attributed =
    NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &string);
  let all = NSRange::new(0, string.length());
  let color = gray(false);

  unsafe {
    let object: &AnyObject = &font;
    attributed.addAttribute_value_range(NSFontAttributeName, object, all);
    let object: &AnyObject = &color;
    attributed.addAttribute_value_range(NSForegroundColorAttributeName, object, all);
    let key: &NSString =
      &*std::ptr::from_ref(kCTAdaptiveImageProviderAttributeName).cast::<NSString>();
    let object: &AnyObject = &provider;
    attributed.addAttribute_value_range(key, object, NSRange::new(1, 1));
  }

  let cf: &objc2_core_foundation::CFAttributedString =
    unsafe { &*std::ptr::from_ref(&*attributed).cast() };
  let line = crate::coretext::line_of(cf);
  let runs = crate::coretext::runs(&line);
  let placeholder: f64 = runs
    .iter()
    .flat_map(|run| run.glyphs.iter())
    .filter(|glyph| glyph.index == 1)
    .map(|glyph| glyph.advance)
    .sum();
  let (line_width, ascent, descent, leading) = crate::coretext::line_metrics(&line);

  report.line(format!(
    "  CTLine: width {line_width:.2}, U+FFFC advance {placeholder:.2}, ascent {ascent:.2} descent {descent:.2} leading {leading:.2}"
  ));

  let laid = textkit::lay_out(&attributed, textkit::UNBOUNDED, 0, false);
  let segment = laid.range_width(1, 2);
  let (tk_width, tk_height) = textkit::usage(&laid);
  let tk_line = laid.lines.first().map_or((0., 0.), |line| {
    (line.bounds.size.width, line.bounds.size.height)
  });

  report.line(format!(
    "  TextKit 2: usage {tk_width:.2} x {tk_height:.2}, line {:.2} x {:.2}, U+FFFC segment width {segment:.2}",
    tk_line.0, tk_line.1
  ));

  let case = plain_case("genmoji", "AB");
  let families = tuned.families(&case, size as f32);
  let mut layouts: LayoutContext<()> = LayoutContext::new();
  let names: Vec<FontFamilyName<'static>> = families
    .iter()
    .map(|family| FontFamilyName::Named(Cow::Owned(family.clone())))
    .collect();
  let mut builder = layouts.ranged_builder(&mut tuned.fonts.context, "AB", 1., false);

  builder.push_default(StyleProperty::FontFamily(FontFamily::List(Cow::Owned(
    names,
  ))));
  builder.push_default(StyleProperty::FontSize(size as f32));
  builder.push_inline_box(InlineBox {
    id: 1,
    kind: InlineBoxKind::InFlow,
    index: 1,
    width: bounds.size.width as f32,
    height: bounds.size.height as f32,
    baseline: Some((bounds.size.height + bounds.origin.y) as f32),
    vertical_align: VerticalAlign::BASELINE,
  });

  let mut layout = builder.build("AB");

  layout.break_all_lines(None);

  let mut placed: Option<f32> = None;
  let mut baseline = 0.;

  for line in layout.lines() {
    baseline = line.metrics().baseline;

    for item in line.items() {
      if let PositionedLayoutItem::InlineBox(inline) = item {
        placed = Some(inline.x);
      }
    }
  }

  let parley_width = layout.width();
  let parley_height = layout.height();

  report.line(format!(
    "  Parley inline box: layout {parley_width:.2} x {parley_height:.2}, box at x {:.2} (families {})",
    placed.unwrap_or(f32::NAN),
    families.join(", ")
  ));

  let laid_parley = Laid {
    layout,
    text: "AB".to_owned(),
    marks: Vec::new(),
    truncated: false,
    notes: Vec::new(),
  };
  let width = tk_width.max(f64::from(parley_width)) as f32;
  let height = tk_height.max(f64::from(parley_height)) as f32;
  let mut rows = Vec::new();

  for scale in [2f32, 3.] {
    let a = textkit::image(&laid, width, height, scale, Ground::White, Smoothing(true));
    let canvas = Canvas::for_box(width, height, scale, Ground::White, Smoothing(true));

    raster.draw(&canvas, &tuned_runs(&laid_parley, tuned));

    if let Some(inline_x) = placed {
      let top = canvas.height_points();
      let context = &*canvas.context;

      CGContext::save_g_state(Some(context));
      canvas.points_up();
      unsafe {
        ct.draw_image_from_adaptive_image_provider_at_point(
          protocol,
          CGPoint::new(
            f64::from(MARGIN + inline_x),
            top - f64::from(MARGIN + baseline),
          ),
          context,
        );
      }
      CGContext::restore_g_state(Some(context));
    }

    let c = canvas.to_rgba();
    let mut b = vello(&laid_parley, width, height, scale);

    if let Some(inline_x) = placed {
      composite(
        &mut b,
        bounds,
        (MARGIN + inline_x) * scale,
        (MARGIN + baseline) * scale,
        scale,
      );
    }

    let a_c = InkDiff::of(&a, &c, true);
    let a_b = InkDiff::of(&a, &b, true);

    report.line(format!("  @{scale}x: TextKit vs C (CTFontDrawImageFromAdaptiveImageProviderAtPoint) {a_c}; TextKit vs B (image composited) {a_b}"));
    rows.push(json!({"scale": scale, "a_vs_c": a_c, "a_vs_b": a_b}));

    if let Ok(dir) = std::env::var("PARITY_OUT") {
      let dir = std::path::Path::new(&dir).join("genmoji");
      let _ = std::fs::create_dir_all(&dir);
      let _ = a.write(&dir.join(format!("a@{scale}x.png")));
      let _ = b.write(&dir.join(format!("b@{scale}x.png")));
      let _ = c.write(&dir.join(format!("c@{scale}x.png")));
    }
  }

  let asked = provider.ivars().asked.borrow().clone();

  report.line(format!(
    "  provider asked for (proposed w, h, scale): {asked:?}"
  ));
  report.put(
    "genmoji",
    json!({
      "ct_bounds": [bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height],
      "ctline": {"width": line_width, "placeholder_advance": placeholder, "ascent": ascent, "descent": descent},
      "textkit": {"usage": [tk_width, tk_height], "line": [tk_line.0, tk_line.1], "segment": segment},
      "parley": {"width": parley_width, "height": parley_height, "box_x": placed},
      "pixels": rows,
      "asked": asked,
    }),
  );
}

fn vello(laid: &Laid, width: f32, height: f32, scale: f32) -> Rgba {
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

/// Blends the provider's image for `bounds` into `image`, its origin on the baseline at
/// `(x, baseline)` device pixels.
fn composite(image: &mut Rgba, bounds: CGRect, x: f32, baseline: f32, scale: f32) {
  let side = bounds.size.width.max(bounds.size.height).max(1.);
  let size = CGSize::new(side, side);
  let Some(source) = disc((side * f64::from(scale)).round().max(1.) as usize)
  else {
    return;
  };
  let wide = (size.width * f64::from(scale)).round().max(1.) as usize;
  let high = (size.height * f64::from(scale)).round().max(1.) as usize;
  let canvas = Canvas::new(wide, high, 1., Ground::ClearOverWhite, Smoothing(false));

  CGContext::draw_image(
    Some(&canvas.context),
    CGRect::new(CGPoint::new(0., 0.), CGSize::new(wide as f64, high as f64)),
    Some(&source),
  );

  let data = objc2_core_graphics::CGBitmapContextGetData(Some(&canvas.context)).cast::<u8>();
  let pixels = unsafe { std::slice::from_raw_parts(data, wide * high * 4) };
  let left = x.round() as i64;
  let top = (baseline - ((bounds.size.height + bounds.origin.y) as f32 * scale)).round() as i64;

  for y in 0..high {
    for x in 0..wide {
      let (tx, ty) = (left + x as i64, top + y as i64);

      if tx < 0 || ty < 0 || tx >= i64::from(image.width) || ty >= i64::from(image.height) {
        continue;
      }

      let source = &pixels[(y * wide + x) * 4..(y * wide + x) * 4 + 4];
      let alpha = u32::from(source[3]);
      let at = (ty as usize * image.width as usize + tx as usize) * 4;

      for channel in 0..3 {
        let under = u32::from(image.data[at + channel]);

        image.data[at + channel] =
          (u32::from(source[channel]) + (under * (255 - alpha) + 127) / 255).min(255) as u8;
      }
    }
  }
}
