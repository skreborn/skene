//! Opaque RGBA images: PNG output, region statistics and comparisons.

use std::path::Path;

#[derive(Clone)]
pub struct Img {
  pub w: usize,
  pub h: usize,
  pub px: Vec<[u8; 4]>,
}

/// An integer pixel rectangle: x0, y0 inclusive, x1, y1 exclusive.
#[derive(Clone, Copy, Debug)]
pub struct Region {
  pub x0: usize,
  pub y0: usize,
  pub x1: usize,
  pub y1: usize,
}

impl Region {
  pub fn from_f32(r: [f32; 4], w: usize, h: usize) -> Self {
    Self {
      x0: (r[0].max(0.0) as usize).min(w),
      y0: (r[1].max(0.0) as usize).min(h),
      x1: ((r[0] + r[2]).ceil().max(0.0) as usize).min(w),
      y1: ((r[1] + r[3]).ceil().max(0.0) as usize).min(h),
    }
  }

  pub fn inflate(self, by: usize, w: usize, h: usize) -> Self {
    Self {
      x0: self.x0.saturating_sub(by),
      y0: self.y0.saturating_sub(by),
      x1: (self.x1 + by).min(w),
      y1: (self.y1 + by).min(h),
    }
  }

  pub const fn contains(&self, x: usize, y: usize) -> bool {
    x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
  }

  pub fn iter(self) -> impl Iterator<Item = (usize, usize)> {
    (self.y0..self.y1).flat_map(move |y| (self.x0..self.x1).map(move |x| (x, y)))
  }
}

impl Img {
  pub fn from_rgba(w: usize, h: usize, data: &[u8]) -> Self {
    Self {
      w,
      h,
      px: data
        .chunks_exact(4)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .collect(),
    }
  }

  pub fn from_bgra(w: usize, h: usize, data: &[u8]) -> Self {
    Self {
      w,
      h,
      px: data
        .chunks_exact(4)
        .map(|p| [p[2], p[1], p[0], p[3]])
        .collect(),
    }
  }

  pub fn at(&self, x: usize, y: usize) -> [u8; 4] {
    self.px[y * self.w + x]
  }

  pub fn save(&self, path: &Path) {
    let file = std::fs::File::create(path).expect("create png");
    let mut encoder =
      png::Encoder::new(std::io::BufWriter::new(file), self.w as u32, self.h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    let bytes: Vec<u8> = self.px.iter().flatten().copied().collect();
    writer.write_image_data(&bytes).expect("png data");
  }

  /// The absolute difference, amplified four times, opaque.
  pub fn diff(&self, other: &Self) -> Self {
    let px = self
      .px
      .iter()
      .zip(&other.px)
      .map(|(a, b)| {
        let d = |i: usize| (a[i].abs_diff(b[i]) as u32 * 4).min(255) as u8;
        [255 - d(0), 255 - d(1), 255 - d(2), 255]
      })
      .collect();
    Self {
      w: self.w,
      h: self.h,
      px,
    }
  }

  /// Ink: where this image differs from the text-less background by more than 48 in sum.
  pub fn ink(&self, background: &Self, region: Region) -> Vec<bool> {
    region
      .iter()
      .map(|(x, y)| {
        let a = self.at(x, y);
        let b = background.at(x, y);
        (0..3).map(|i| a[i].abs_diff(b[i]) as u32).sum::<u32>() > 48
      })
      .collect()
  }

  pub fn ink_count(&self, background: &Self, region: Region) -> usize {
    self.ink(background, region).iter().filter(|&&i| i).count()
  }

  /// Intersection over union of two images' ink in a region.
  pub fn ink_iou(&self, other: &Self, background: &Self, region: Region) -> f64 {
    let a = self.ink(background, region);
    let b = other.ink(background, region);
    let inter = a.iter().zip(&b).filter(|(x, y)| **x && **y).count();
    let union = a.iter().zip(&b).filter(|(x, y)| **x || **y).count();

    if union == 0 {
      1.0
    }
    else {
      inter as f64 / union as f64
    }
  }

  /// Mean absolute difference per channel (0..255) in a region.
  pub fn mean_abs_diff(&self, other: &Self, region: Region) -> f64 {
    let mut sum = 0u64;
    let mut n = 0u64;

    for (x, y) in region.iter() {
      let a = self.at(x, y);
      let b = other.at(x, y);
      sum += (0..3).map(|i| a[i].abs_diff(b[i]) as u64).sum::<u64>();
      n += 3;
    }

    if n == 0 { 0.0 } else { sum as f64 / n as f64 }
  }

  /// Total darkness (sum of 255 - luma) in a region, for comparing ink weight.
  pub fn darkness(&self, background: &Self, region: Region) -> f64 {
    region
      .iter()
      .map(|(x, y)| {
        let luma = |p: [u8; 4]| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
        (luma(background.at(x, y)) - luma(self.at(x, y))).max(0.0)
      })
      .sum()
  }

  /// Pixels whose channels spread by more than 60: colour, not grey.
  pub fn chroma_count(&self, region: Region) -> usize {
    region
      .iter()
      .filter(|&(x, y)| {
        let p = self.at(x, y);
        let max = p[0].max(p[1]).max(p[2]);
        let min = p[0].min(p[1]).min(p[2]);
        max - min > 60
      })
      .count()
  }

  pub fn crop(&self, r: Region) -> Self {
    let px = r.iter().map(|(x, y)| self.at(x, y)).collect();
    Self {
      w: r.x1 - r.x0,
      h: r.y1 - r.y0,
      px,
    }
  }

  /// Side by side with a 4-pixel grey gutter.
  pub fn hstack(images: &[&Self]) -> Self {
    let h = images.iter().map(|i| i.h).max().unwrap_or(0);
    let w = images.iter().map(|i| i.w).sum::<usize>() + 4 * images.len().saturating_sub(1);
    let mut px = vec![[128, 128, 128, 255]; w * h];
    let mut x0 = 0;

    for image in images {
      for y in 0..image.h {
        for x in 0..image.w {
          px[y * w + x0 + x] = image.at(x, y);
        }
      }

      x0 += image.w + 4;
    }

    Self { w, h, px }
  }
}
