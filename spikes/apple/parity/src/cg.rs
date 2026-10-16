//! Core Graphics bitmaps: a context by the harness's image convention, read back as the
//! harness's [`Rgba`].

use objc2_core_foundation::{CFRetained, CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
  CGBitmapContextCreate,
  CGBitmapContextGetData,
  CGColorSpace,
  CGContext,
  CGImageAlphaInfo,
};
use text_parity::{image::Rgba, record::image_size};

/// What a bitmap starts as before text is drawn into it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ground {
  /// Opaque white (the convention: black text on white).
  White,
  /// Opaque black (white text on black).
  Black,
  /// Transparent, composited over white when read.
  ClearOverWhite,
  /// Transparent, composited over black when read.
  ClearOverBlack,
}

impl Ground {
  /// Whether text on this ground is white.
  #[must_use]
  pub const fn white_text(self) -> bool {
    matches!(self, Self::Black | Self::ClearOverBlack)
  }

  /// A short name for reports.
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::White => "opaque black-on-white",
      Self::Black => "opaque white-on-black",
      Self::ClearOverWhite => "transparent black (over white)",
      Self::ClearOverBlack => "transparent white (over black)",
    }
  }
}

/// How glyphs are smoothed: Core Graphics' font smoothing (stem dilation on macOS) allowed and
/// asked for, or turned off.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Smoothing(pub bool);

/// A bitmap context of `width` by `height` device pixels at `scale`.
pub struct Canvas {
  /// The context.
  pub context: CFRetained<CGContext>,
  /// Pixels across.
  pub width: usize,
  /// Pixels down.
  pub height: usize,
  /// Device pixels per point.
  pub scale: f64,
  /// What it started as.
  pub ground: Ground,
}

impl Canvas {
  /// A canvas by the image convention for a layout box of `width` by `height` points.
  #[must_use]
  pub fn for_box(
    width: f32,
    height: f32,
    scale: f32,
    ground: Ground,
    smoothing: Smoothing,
  ) -> Self {
    let (wide, high) = image_size(width, height, scale);

    Self::new(
      wide as usize,
      high as usize,
      f64::from(scale),
      ground,
      smoothing,
    )
  }

  /// A canvas of `width` by `height` pixels.
  ///
  /// # Panics
  ///
  /// Where Core Graphics refuses the context.
  #[must_use]
  pub fn new(
    width: usize,
    height: usize,
    scale: f64,
    ground: Ground,
    smoothing: Smoothing,
  ) -> Self {
    let space = CGColorSpace::new_device_rgb().expect("a device RGB colour space");
    let context = unsafe {
      CGBitmapContextCreate(
        std::ptr::null_mut(),
        width.max(1),
        height.max(1),
        8,
        width.max(1) * 4,
        Some(&space),
        CGImageAlphaInfo::PremultipliedLast.0,
      )
    }
    .expect("a bitmap context");
    let full = CGRect::new(
      CGPoint::new(0., 0.),
      CGSize::new(width as f64, height as f64),
    );

    match ground {
      Ground::White => {
        CGContext::set_rgb_fill_color(Some(&context), 1., 1., 1., 1.);
        CGContext::fill_rect(Some(&context), full);
      }
      Ground::Black => {
        CGContext::set_rgb_fill_color(Some(&context), 0., 0., 0., 1.);
        CGContext::fill_rect(Some(&context), full);
      }
      Ground::ClearOverWhite | Ground::ClearOverBlack => {
        CGContext::clear_rect(Some(&context), full);
      }
    }

    CGContext::set_allows_font_smoothing(Some(&context), smoothing.0);
    CGContext::set_should_smooth_fonts(Some(&context), smoothing.0);

    Self {
      context,
      width,
      height,
      scale,
      ground,
    }
  }

  /// Sets the transform to points with y down and the origin at the layout's top left (inside
  /// the margin): what TextKit 2 draws in.
  pub fn flip_to_layout(&self) {
    let context = Some(&*self.context);

    CGContext::translate_ctm(context, 0., self.height as f64);
    CGContext::scale_ctm(context, self.scale, -self.scale);
    CGContext::translate_ctm(
      context,
      f64::from(text_parity::record::MARGIN),
      f64::from(text_parity::record::MARGIN),
    );
  }

  /// Sets the transform to points with y up (Core Graphics' own), unflipped.
  pub fn points_up(&self) {
    CGContext::scale_ctm(Some(&*self.context), self.scale, self.scale);
  }

  /// The canvas's height in points.
  #[must_use]
  pub fn height_points(&self) -> f64 {
    self.height as f64 / self.scale
  }

  /// Resets the text matrix.
  pub fn identity_text(&self) {
    CGContext::set_text_matrix(
      Some(&*self.context),
      CGAffineTransform {
        a: 1.,
        b: 0.,
        c: 0.,
        d: 1.,
        tx: 0.,
        ty: 0.,
      },
    );
  }

  /// The pixels, un-premultiplied and composited over the ground's colour where transparent.
  #[must_use]
  pub fn to_rgba(&self) -> Rgba {
    let data = CGBitmapContextGetData(Some(&self.context)).cast::<u8>();
    let count = self.width.max(1) * self.height.max(1) * 4;
    let bytes = unsafe { std::slice::from_raw_parts(data, count) };
    let under: u32 = match self.ground {
      Ground::White | Ground::ClearOverWhite => 255,
      Ground::Black | Ground::ClearOverBlack => 0,
    };
    let mut out = Vec::with_capacity(count);

    for pixel in bytes.chunks_exact(4) {
      let alpha = u32::from(pixel[3]);

      for channel in 0..3 {
        // Premultiplied: colour plus the ground through what alpha leaves.
        let value = u32::from(pixel[channel]) + (under * (255 - alpha) + 127) / 255;

        out.push(value.min(255) as u8);
      }

      out.push(255);
    }

    Rgba {
      width: self.width.max(1) as u32,
      height: self.height.max(1) as u32,
      data: out,
    }
  }
}

/// An image with every channel inverted: black text on white drawn by an engine that has no
/// colour of its own read as white on black.
#[must_use]
pub fn inverted(image: &Rgba) -> Rgba {
  let mut out = image.clone();

  for pixel in out.data.chunks_exact_mut(4) {
    for channel in 0..3 {
      pixel[channel] = 255 - pixel[channel];
    }
  }

  out
}

/// Pixel metrics over the pixels either image inks (the prior Apple spike's measure): the
/// largest channel difference, its mean over inked pixels, and how many differ by more than
/// 64.
#[derive(Clone, Copy, PartialEq, Debug, Default, serde::Serialize)]
pub struct InkDiff {
  /// The largest channel difference.
  pub max: u8,
  /// The mean of each inked pixel's largest channel difference.
  pub mean: f64,
  /// Pixels differing by more than 64.
  pub over64: usize,
  /// Pixels either image inks.
  pub inked: usize,
}

impl InkDiff {
  /// Compares `a` and `b`, both padded with `ground` to the larger extent; a pixel is inked
  /// where it is more than 5 away from the ground.
  #[must_use]
  pub fn of(a: &Rgba, b: &Rgba, ground_white: bool) -> Self {
    let width = a.width.max(b.width);
    let height = a.height.max(b.height);
    let ground = if ground_white { 255u8 } else { 0 };
    let at = |image: &Rgba, x: u32, y: u32| -> [u8; 3] {
      if x >= image.width || y >= image.height {
        return [ground; 3];
      }

      let index = (y as usize * image.width as usize + x as usize) * 4;

      [
        image.data[index],
        image.data[index + 1],
        image.data[index + 2],
      ]
    };
    let mut diff = Self::default();
    let mut total = 0u64;

    for y in 0..height {
      for x in 0..width {
        let (pa, pb) = (at(a, x, y), at(b, x, y));
        let blank = |p: [u8; 3]| p.iter().all(|&c| c.abs_diff(ground) <= 5);

        if blank(pa) && blank(pb) {
          continue;
        }

        diff.inked += 1;

        let d = (0..3).map(|i| pa[i].abs_diff(pb[i])).max().unwrap_or(0);

        diff.max = diff.max.max(d);
        total += u64::from(d);

        if d > 64 {
          diff.over64 += 1;
        }
      }
    }

    diff.mean = if diff.inked == 0 {
      0.
    }
    else {
      total as f64 / diff.inked as f64
    };

    diff
  }
}

impl std::fmt::Display for InkDiff {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "max {} mean {:.2} >64 {} ({:.2}% of {} inked)",
      self.max,
      self.mean,
      self.over64,
      if self.inked == 0 {
        0.
      }
      else {
        100. * self.over64 as f64 / self.inked as f64
      },
      self.inked
    )
  }
}

/// The mean ink darkness (coverage) of an image of dark text on white, or light on black: how
/// heavy its strokes are, 0 to 255 per pixel summed over the image.
#[must_use]
pub fn ink_weight(image: &Rgba, ground_white: bool) -> f64 {
  let mut total = 0u64;

  for pixel in image.data.chunks_exact(4) {
    let luminance =
      (u32::from(pixel[0]) * 299 + u32::from(pixel[1]) * 587 + u32::from(pixel[2]) * 114) / 1000;

    total += u64::from(if ground_white {
      255 - luminance
    }
    else {
      luminance
    });
  }

  total as f64 / 255.
}
