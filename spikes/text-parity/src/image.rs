//! Images as the comparison reads them: any PNG, composited over white, and the pixel metrics
//! of two of them.

use std::{fs::File, io::BufReader, path::Path};

use png::{ColorType, Transformations};
use serde::{Deserialize, Serialize};

/// An 8-bit RGBA image, not premultiplied.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Rgba {
  /// Its width in pixels.
  pub width: u32,
  /// Its height in pixels.
  pub height: u32,
  /// Its pixels, row by row, four bytes each.
  pub data: Vec<u8>,
}

impl Rgba {
  /// A white image.
  #[must_use]
  pub fn white(width: u32, height: u32) -> Self {
    Self {
      width,
      height,
      data: vec![255; width as usize * height as usize * 4],
    }
  }

  /// Reads a PNG of any colour type and depth.
  ///
  /// # Errors
  ///
  /// Where the file cannot be read or decoded.
  pub fn read(path: &Path) -> Result<Self, String> {
    let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));

    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);

    let mut reader = decoder
      .read_info()
      .map_err(|error| format!("{}: {error}", path.display()))?;
    let mut buffer = vec![0; reader.output_buffer_size().unwrap_or_default()];
    let info = reader
      .next_frame(&mut buffer)
      .map_err(|error| format!("{}: {error}", path.display()))?;
    let pixels = info.width as usize * info.height as usize;
    let mut data = Vec::with_capacity(pixels * 4);

    for pixel in buffer[..info.buffer_size()].chunks(info.color_type.samples()) {
      match info.color_type {
        ColorType::Rgba => data.extend_from_slice(pixel),
        ColorType::Rgb => data.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]),
        ColorType::GrayscaleAlpha => {
          data.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]])
        }
        ColorType::Grayscale => data.extend_from_slice(&[pixel[0], pixel[0], pixel[0], 255]),
        ColorType::Indexed => return Err(format!("{}: palette left unexpanded", path.display())),
      }
    }

    Ok(Self {
      width: info.width,
      height: info.height,
      data,
    })
  }

  /// Writes the image as an RGBA PNG.
  ///
  /// # Errors
  ///
  /// Where encoding or writing fails.
  pub fn write(&self, path: &Path) -> Result<(), String> {
    let file = File::create(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), self.width, self.height);

    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;

    writer
      .write_image_data(&self.data)
      .map_err(|error| error.to_string())
  }

  /// The pixel at `x`, `y` composited over white; white outside the image.
  #[must_use]
  pub fn over_white(&self, x: u32, y: u32) -> [u8; 3] {
    if x >= self.width || y >= self.height {
      return [255; 3];
    }

    let at = (y as usize * self.width as usize + x as usize) * 4;
    let alpha = u32::from(self.data[at + 3]);
    let channel = |value: u8| {
      #[expect(clippy::cast_possible_truncation, reason = "at most 255")]
      let mixed = ((u32::from(value) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;

      mixed
    };

    [
      channel(self.data[at]),
      channel(self.data[at + 1]),
      channel(self.data[at + 2]),
    ]
  }

  /// The luminance at `x`, `y` (Rec. 601), over white.
  #[must_use]
  pub fn luminance(&self, x: u32, y: u32) -> f64 {
    let [red, green, blue] = self.over_white(x, y);

    0.299 * f64::from(red) + 0.587 * f64::from(green) + 0.114 * f64::from(blue)
  }
}

/// The rectangle ink covers: `[left, top, right, bottom]`, right and bottom exclusive.
pub type InkBox = [u32; 4];

/// How two images of one variant differ.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct PixelMetrics {
  /// The compared canvas: both images padded with white to the larger of each extent.
  pub width: u32,
  /// See `width`.
  pub height: u32,
  /// Whether the two images had the same size.
  pub same_size: bool,
  /// The mean, over the canvas, of each pixel's largest channel difference, out of 255.
  pub mean_abs_diff: f64,
  /// The share of pixels whose largest channel difference exceeds `threshold`.
  pub share_over: f64,
  /// The threshold `share_over` counts beyond.
  pub threshold: u8,
  /// The largest channel difference anywhere.
  pub max_diff: u8,
  /// The ink box of the first image (a pixel is ink where a channel is below `255 - threshold`).
  pub ink_a: Option<InkBox>,
  /// The ink box of the second image.
  pub ink_b: Option<InkBox>,
  /// Second minus first, edge by edge, in pixels.
  pub ink_delta: Option<[i64; 4]>,
  /// Ink pixels in both over ink pixels in either.
  pub ink_iou: f64,
  /// Mean SSIM of luminance over 8 by 8 windows at a stride of 4.
  pub ssim: f64,
}

fn is_ink(pixel: [u8; 3], threshold: u8) -> bool {
  pixel.iter().any(|channel| *channel < 255 - threshold)
}

fn ink_box(image: &Rgba, width: u32, height: u32, threshold: u8) -> Option<InkBox> {
  let mut found: Option<InkBox> = None;

  for y in 0..height {
    for x in 0..width {
      if is_ink(image.over_white(x, y), threshold) {
        found = Some(match found {
          None => [x, y, x + 1, y + 1],
          Some([left, top, right, bottom]) => {
            [left.min(x), top.min(y), right.max(x + 1), bottom.max(y + 1)]
          }
        });
      }
    }
  }

  found
}

/// Mean SSIM of the luminance of `a` and `b` over `width` by `height`.
fn ssim(a: &Rgba, b: &Rgba, width: u32, height: u32) -> f64 {
  const WINDOW: u32 = 8;
  const STRIDE: u32 = 4;
  const C1: f64 = (0.01 * 255.) * (0.01 * 255.);
  const C2: f64 = (0.03 * 255.) * (0.03 * 255.);

  let window_wide = WINDOW.min(width);
  let window_high = WINDOW.min(height);
  let mut total = 0.;
  let mut windows = 0_u32;
  let mut y = 0;

  loop {
    let mut x = 0;

    loop {
      let mut sum_a = 0.;
      let mut sum_b = 0.;
      let mut sum_aa = 0.;
      let mut sum_bb = 0.;
      let mut sum_ab = 0.;

      for dy in 0..window_high {
        for dx in 0..window_wide {
          let la = a.luminance(x + dx, y + dy);
          let lb = b.luminance(x + dx, y + dy);

          sum_a += la;
          sum_b += lb;
          sum_aa += la * la;
          sum_bb += lb * lb;
          sum_ab += la * lb;
        }
      }

      let n = f64::from(window_wide * window_high);
      let mean_a = sum_a / n;
      let mean_b = sum_b / n;
      let var_a = (sum_aa / n - mean_a * mean_a).max(0.);
      let var_b = (sum_bb / n - mean_b * mean_b).max(0.);
      let covariance = sum_ab / n - mean_a * mean_b;

      total += ((2. * mean_a * mean_b + C1) * (2. * covariance + C2))
        / ((mean_a * mean_a + mean_b * mean_b + C1) * (var_a + var_b + C2));
      windows += 1;

      if x + window_wide >= width {
        break;
      }

      x = (x + STRIDE).min(width - window_wide);
    }

    if y + window_high >= height {
      break;
    }

    y = (y + STRIDE).min(height - window_high);
  }

  total / f64::from(windows.max(1))
}

/// How `a` and `b` differ, both padded with white to the larger of each extent.
#[must_use]
pub fn compare(a: &Rgba, b: &Rgba, threshold: u8) -> PixelMetrics {
  let width = a.width.max(b.width);
  let height = a.height.max(b.height);
  let mut sum = 0_u64;
  let mut over = 0_u64;
  let mut max_diff = 0;
  let mut both = 0_u64;
  let mut either = 0_u64;

  for y in 0..height {
    for x in 0..width {
      let pa = a.over_white(x, y);
      let pb = b.over_white(x, y);
      let diff = (0..3)
        .map(|channel| pa[channel].abs_diff(pb[channel]))
        .max()
        .unwrap_or(0);

      sum += u64::from(diff);
      max_diff = max_diff.max(diff);

      if diff > threshold {
        over += 1;
      }

      let (ink_a, ink_b) = (is_ink(pa, threshold), is_ink(pb, threshold));

      both += u64::from(ink_a && ink_b);
      either += u64::from(ink_a || ink_b);
    }
  }

  let pixels = (u64::from(width) * u64::from(height)).max(1);
  let ink_a = ink_box(a, width, height, threshold);
  let ink_b = ink_box(b, width, height, threshold);
  #[expect(clippy::cast_precision_loss, reason = "pixel counts")]
  let share = |count: u64, of: u64| count as f64 / of.max(1) as f64;

  PixelMetrics {
    width,
    height,
    same_size: (a.width, a.height) == (b.width, b.height),
    mean_abs_diff: share(sum, pixels),
    share_over: share(over, pixels),
    threshold,
    max_diff,
    ink_delta: ink_a.zip(ink_b).map(|(a, b)| {
      [
        i64::from(b[0]) - i64::from(a[0]),
        i64::from(b[1]) - i64::from(a[1]),
        i64::from(b[2]) - i64::from(a[2]),
        i64::from(b[3]) - i64::from(a[3]),
      ]
    }),
    ink_a,
    ink_b,
    ink_iou: if either == 0 { 1. } else { share(both, either) },
    ssim: ssim(a, b, width, height),
  }
}

/// A difference image: where both have ink it is dark, ink only in `a` red, only in `b` blue.
#[must_use]
pub fn difference(a: &Rgba, b: &Rgba) -> Rgba {
  let width = a.width.max(b.width);
  let height = a.height.max(b.height);
  let mut out = Rgba::white(width, height);

  for y in 0..height {
    for x in 0..width {
      #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "0 to 255"
      )]
      let (la, lb) = (
        a.luminance(x, y).round() as u8,
        b.luminance(x, y).round() as u8,
      );
      let at = (y as usize * width as usize + x as usize) * 4;

      out.data[at..at + 4].copy_from_slice(&[lb, la.min(lb), la, 255]);
    }
  }

  out
}

#[cfg(test)]
mod tests {
  use super::*;

  fn with_dot(width: u32, height: u32, x: u32, y: u32) -> Rgba {
    let mut image = Rgba::white(width, height);
    let at = (y as usize * width as usize + x as usize) * 4;

    image.data[at..at + 4].copy_from_slice(&[0, 0, 0, 255]);

    image
  }

  #[test]
  fn identical_images_agree() {
    let image = with_dot(20, 10, 3, 4);
    let metrics = compare(&image, &image, 64);

    assert_eq!(metrics.mean_abs_diff, 0.);
    assert_eq!(metrics.share_over, 0.);
    assert_eq!(metrics.ink_iou, 1.);
    assert!((metrics.ssim - 1.).abs() < 1e-9, "ssim {}", metrics.ssim);
    assert_eq!(metrics.ink_delta, Some([0, 0, 0, 0]));
  }

  #[test]
  fn a_moved_dot_differs() {
    let a = with_dot(20, 10, 3, 4);
    let b = with_dot(21, 10, 5, 4);
    let metrics = compare(&a, &b, 64);

    assert!(!metrics.same_size);
    assert_eq!(metrics.width, 21);
    assert_eq!(metrics.ink_delta, Some([2, 0, 2, 0]));
    assert_eq!(metrics.ink_iou, 0.);
    assert!(metrics.ssim < 1.);
    assert_eq!(metrics.max_diff, 255);
  }

  #[test]
  fn transparency_is_white() {
    let mut clear = Rgba::white(2, 2);

    clear
      .data
      .iter_mut()
      .skip(3)
      .step_by(4)
      .for_each(|alpha| *alpha = 0);
    clear.data[0] = 0;

    assert_eq!(clear.over_white(0, 0), [255; 3]);
  }
}
