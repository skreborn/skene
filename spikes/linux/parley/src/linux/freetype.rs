//! FreeType, the system's rasterizer: glyph masks drawn under the rendering settings
//! fontconfig gives each font, kept in a cache Skene's renderer composites from.
//!
//! The bindings are declared here and linked against the system's `libfreetype`; no build
//! script runs. Grayscale only: a subpixel (LCD) mode needs a component-alpha pass the
//! renderer lacks.

use std::{
  collections::HashMap,
  ffi::{c_char, c_int, c_long, c_short, c_uint, c_ushort, c_void},
  ptr,
  rc::Rc,
};

use parley::FontData;

use crate::linux::fontconfig::RenderSettings;

type FtLibrary = *mut c_void;
type FtPos = c_long;

#[repr(C)]
struct FtGeneric {
  data: *mut c_void,
  finalizer: *mut c_void,
}

#[repr(C)]
struct FtBBox {
  x_min: FtPos,
  y_min: FtPos,
  x_max: FtPos,
  y_max: FtPos,
}

#[repr(C)]
struct FtBitmapSize {
  height: c_short,
  width: c_short,
  size: FtPos,
  x_ppem: FtPos,
  y_ppem: FtPos,
}

#[repr(C)]
struct FtVector {
  x: FtPos,
  y: FtPos,
}

#[repr(C)]
struct FtMatrix {
  xx: c_long,
  xy: c_long,
  yx: c_long,
  yy: c_long,
}

#[repr(C)]
struct FtBitmap {
  rows: c_uint,
  width: c_uint,
  pitch: c_int,
  buffer: *mut u8,
  num_grays: c_ushort,
  pixel_mode: u8,
  palette_mode: u8,
  palette: *mut c_void,
}

#[repr(C)]
struct FtGlyphMetrics {
  width: FtPos,
  height: FtPos,
  hori_bearing_x: FtPos,
  hori_bearing_y: FtPos,
  hori_advance: FtPos,
  vert_bearing_x: FtPos,
  vert_bearing_y: FtPos,
  vert_advance: FtPos,
}

#[repr(C)]
struct FtGlyphSlotRec {
  library: FtLibrary,
  face: *mut FtFaceRec,
  next: *mut FtGlyphSlotRec,
  glyph_index: c_uint,
  generic: FtGeneric,
  metrics: FtGlyphMetrics,
  linear_hori_advance: c_long,
  linear_vert_advance: c_long,
  advance: FtVector,
  format: c_uint,
  bitmap: FtBitmap,
  bitmap_left: c_int,
  bitmap_top: c_int,
}

#[repr(C)]
struct FtFaceRec {
  num_faces: c_long,
  face_index: c_long,
  face_flags: c_long,
  style_flags: c_long,
  num_glyphs: c_long,
  family_name: *mut c_char,
  style_name: *mut c_char,
  num_fixed_sizes: c_int,
  available_sizes: *mut FtBitmapSize,
  num_charmaps: c_int,
  charmaps: *mut c_void,
  generic: FtGeneric,
  bbox: FtBBox,
  units_per_em: c_ushort,
  ascender: c_short,
  descender: c_short,
  height: c_short,
  max_advance_width: c_short,
  max_advance_height: c_short,
  underline_position: c_short,
  underline_thickness: c_short,
  glyph: *mut FtGlyphSlotRec,
}

const FACE_FLAG_SCALABLE: c_long = 1;
const FACE_FLAG_FIXED_SIZES: c_long = 1 << 1;
const FACE_FLAG_MULTIPLE_MASTERS: c_long = 1 << 8;
const LOAD_NO_HINTING: i32 = 1 << 1;
const LOAD_FORCE_AUTOHINT: i32 = 1 << 5;
const LOAD_NO_BITMAP: i32 = 1 << 3;
const LOAD_COLOR: i32 = 1 << 20;
const RENDER_MODE_NORMAL: c_uint = 0;
const RENDER_MODE_LIGHT: c_uint = 1;
const RENDER_MODE_MONO: c_uint = 2;
const PIXEL_MODE_MONO: u8 = 1;
const PIXEL_MODE_GRAY: u8 = 2;
const PIXEL_MODE_BGRA: u8 = 7;

const fn load_target(mode: c_uint) -> i32 {
  ((mode & 15) << 16) as i32
}

#[link(name = "freetype")]
unsafe extern "C" {
  fn FT_Init_FreeType(library: *mut FtLibrary) -> c_int;
  fn FT_Done_FreeType(library: FtLibrary) -> c_int;
  fn FT_New_Memory_Face(
    library: FtLibrary,
    base: *const u8,
    size: c_long,
    index: c_long,
    face: *mut *mut FtFaceRec,
  ) -> c_int;
  fn FT_Done_Face(face: *mut FtFaceRec) -> c_int;
  fn FT_Set_Char_Size(
    face: *mut FtFaceRec,
    width: c_long,
    height: c_long,
    horizontal: c_uint,
    vertical: c_uint,
  ) -> c_int;
  fn FT_Select_Size(face: *mut FtFaceRec, strike: c_int) -> c_int;
  fn FT_Set_Transform(face: *mut FtFaceRec, matrix: *mut FtMatrix, delta: *mut FtVector);
  fn FT_Load_Glyph(face: *mut FtFaceRec, glyph: c_uint, flags: i32) -> c_int;
  fn FT_Render_Glyph(slot: *mut FtGlyphSlotRec, mode: c_uint) -> c_int;
  fn FT_Set_Var_Blend_Coordinates(
    face: *mut FtFaceRec,
    count: c_uint,
    coords: *mut c_long,
  ) -> c_int;
}

/// A glyph's pixels.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MaskPixels {
  /// Coverage, one byte a pixel: the run's color shows through it.
  Alpha(Vec<u8>),
  /// Premultiplied RGBA, four bytes a pixel: a color glyph's own.
  Color(Vec<u8>),
}

/// A rasterized glyph: its pixels and where they sit against the pen.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GlyphMask {
  /// The width in pixels.
  pub width: u32,
  /// The height in pixels.
  pub height: u32,
  /// The left edge's offset from the pen.
  pub left: i32,
  /// The top edge's offset above the baseline.
  pub top: i32,
  /// The pixels.
  pub pixels: MaskPixels,
}

/// A face FreeType holds, with the font data it reads kept alive.
struct Face {
  face: *mut FtFaceRec,
  _data: FontData,
}

impl Drop for Face {
  fn drop(&mut self) {
    unsafe { FT_Done_Face(self.face) };
  }
}

/// What a mask is cached by.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Key {
  font: (u64, u32),
  coords: Vec<i16>,
  glyph: u32,
  size: u32,
  phase: u8,
  settings: RenderSettings,
}

/// The positions a glyph is drawn at within a pixel: quarters, so a glyph keeps its subpixel
/// place where hinting leaves the horizontal axis alone.
pub const PHASES: u8 = 4;

/// FreeType and the masks it drew.
pub struct Rasterizer {
  library: FtLibrary,
  faces: HashMap<(u64, u32), Face>,
  masks: HashMap<Key, Option<Rc<GlyphMask>>>,
  hits: u64,
  misses: u64,
}

impl Rasterizer {
  /// FreeType, initialized; `None` where it fails to.
  #[must_use]
  pub fn new() -> Option<Self> {
    let mut library = ptr::null_mut();

    (unsafe { FT_Init_FreeType(&raw mut library) } == 0).then(|| Self {
      library,
      faces: HashMap::new(),
      masks: HashMap::new(),
      hits: 0,
      misses: 0,
    })
  }

  /// How many masks are cached.
  #[must_use]
  pub fn cached(&self) -> usize {
    self.masks.len()
  }

  /// How many asks for a mask the cache answered, and how many FreeType drew: the cache's
  /// hit rate.
  #[must_use]
  pub const fn lookups(&self) -> (u64, u64) {
    (self.hits, self.misses)
  }

  /// The subpixel phase a glyph at `x` is drawn in under `settings`: none where hinting
  /// fits outlines to the pixel grid horizontally (medium, full) or edges are not antialiased.
  #[must_use]
  pub fn phase(x: f32, settings: &RenderSettings) -> u8 {
    if !settings.antialias || (settings.hinting && settings.hint_style >= 2) {
      return 0;
    }

    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "a fraction of a pixel in quarters"
    )]
    let phase = ((x - x.floor()) * f32::from(PHASES)).round() as u8 % PHASES;

    phase
  }

  fn face(&mut self, font: &FontData) -> Option<*mut FtFaceRec> {
    let key = (font.data.id(), font.index);

    if let Some(face) = self.faces.get(&key) {
      return Some(face.face);
    }

    let data = font.data.data();
    let mut face = ptr::null_mut();
    let loaded = unsafe {
      FT_New_Memory_Face(
        self.library,
        data.as_ptr(),
        c_long::try_from(data.len()).ok()?,
        c_long::from(font.index),
        &raw mut face,
      )
    };

    if loaded != 0 || face.is_null() {
      return None;
    }

    self.faces.insert(
      key,
      Face {
        face,
        _data: font.clone(),
      },
    );

    Some(face)
  }

  /// The mask of `glyph` of `font` (at the variation `coords`) at `size` pixels a em, drawn
  /// `phase` quarters of a pixel right of the pen, under `settings`; `None` for a glyph with
  /// no pixels.
  pub fn mask(
    &mut self,
    font: &FontData,
    coords: &[i16],
    glyph: u32,
    size: f32,
    phase: u8,
    settings: &RenderSettings,
  ) -> Option<Rc<GlyphMask>> {
    let key = Key {
      font: (font.data.id(), font.index),
      coords: coords.to_vec(),
      glyph,
      size: size.to_bits(),
      phase,
      settings: *settings,
    };

    if let Some(mask) = self.masks.get(&key) {
      self.hits += 1;

      return mask.clone();
    }

    self.misses += 1;

    let mask = self
      .face(font)
      .and_then(|face| unsafe { Self::draw(face, coords, glyph, size, phase, settings) });
    let mask = mask.map(Rc::new);

    self.masks.insert(key, mask.clone());

    mask
  }

  /// Draws one glyph with FreeType.
  ///
  /// # Safety
  ///
  /// `face` is a live face of this rasterizer's library.
  unsafe fn draw(
    face: *mut FtFaceRec,
    coords: &[i16],
    glyph: u32,
    size: f32,
    phase: u8,
    settings: &RenderSettings,
  ) -> Option<GlyphMask> {
    let flags = unsafe { (*face).face_flags };
    let scalable = flags & FACE_FLAG_SCALABLE != 0;
    let mut scale = 1.;

    if scalable {
      #[expect(clippy::cast_possible_truncation, reason = "a font size in 26.6")]
      let size_26_6 = (size * 64.).round() as c_long;

      unsafe { FT_Set_Char_Size(face, 0, size_26_6, 72, 72) };
    } else if flags & FACE_FLAG_FIXED_SIZES != 0 {
      let count = usize::try_from(unsafe { (*face).num_fixed_sizes }).unwrap_or(0);
      let strikes = unsafe { std::slice::from_raw_parts((*face).available_sizes, count) };
      #[expect(clippy::cast_precision_loss, reason = "a strike's size in 26.6")]
      let ppem = |strike: &FtBitmapSize| strike.y_ppem as f32 / 64.;
      let best = strikes.iter().enumerate().min_by(|(_, a), (_, b)| {
        let rank = |strike: &FtBitmapSize| {
          let ppem = ppem(strike);

          if ppem >= size {
            ppem - size
          } else {
            1000. + size - ppem
          }
        };

        rank(a).total_cmp(&rank(b))
      })?;

      unsafe { FT_Select_Size(face, c_int::try_from(best.0).ok()?) };
      scale = size / ppem(best.1);
    } else {
      return None;
    }

    if flags & FACE_FLAG_MULTIPLE_MASTERS != 0 && !coords.is_empty() {
      let mut fixed: Vec<c_long> = coords
        .iter()
        .map(|coord| c_long::from(*coord) << 2)
        .collect();

      unsafe {
        FT_Set_Var_Blend_Coordinates(
          face,
          c_uint::try_from(fixed.len()).ok()?,
          fixed.as_mut_ptr(),
        );
      };
    }

    let mut delta = FtVector {
      x: FtPos::from(phase) * 64 / FtPos::from(PHASES),
      y: 0,
    };

    unsafe { FT_Set_Transform(face, ptr::null_mut(), &raw mut delta) };

    let (target, render) = match (settings.antialias, settings.hinting, settings.hint_style) {
      (false, ..) => (load_target(RENDER_MODE_MONO), RENDER_MODE_MONO),
      (true, true, 1) => (load_target(RENDER_MODE_LIGHT), RENDER_MODE_NORMAL),
      _ => (load_target(RENDER_MODE_NORMAL), RENDER_MODE_NORMAL),
    };
    let mut load = LOAD_COLOR | target;

    if !settings.hinting || settings.hint_style == 0 {
      load |= LOAD_NO_HINTING;
    }

    if settings.autohint {
      load |= LOAD_FORCE_AUTOHINT;
    }

    if !settings.embedded_bitmaps && scalable {
      load |= LOAD_NO_BITMAP;
    }

    if unsafe { FT_Load_Glyph(face, glyph, load) } != 0 {
      return None;
    }

    let slot = unsafe { (*face).glyph };

    if unsafe { FT_Render_Glyph(slot, render) } != 0 {
      return None;
    }

    let bitmap = unsafe { &(*slot).bitmap };
    let (width, height) = (bitmap.width, bitmap.rows);

    if width == 0 || height == 0 || bitmap.buffer.is_null() {
      return None;
    }

    let pitch = isize::try_from(bitmap.pitch).ok()?;
    let row = |y: u32| unsafe {
      let start = if pitch >= 0 {
        bitmap.buffer.offset(pitch * y as isize)
      } else {
        bitmap.buffer.offset(-pitch * (height - 1 - y) as isize)
      };

      std::slice::from_raw_parts(start, pitch.unsigned_abs())
    };
    let (left, top) = unsafe { ((*slot).bitmap_left, (*slot).bitmap_top) };
    let mask = match bitmap.pixel_mode {
      PIXEL_MODE_GRAY => {
        let mut pixels = Vec::with_capacity((width * height) as usize);

        for y in 0..height {
          pixels.extend_from_slice(&row(y)[..width as usize]);
        }

        GlyphMask {
          width,
          height,
          left,
          top,
          pixels: MaskPixels::Alpha(pixels),
        }
      }
      PIXEL_MODE_MONO => {
        let mut pixels = Vec::with_capacity((width * height) as usize);

        for y in 0..height {
          let bits = row(y);

          for x in 0..width as usize {
            let on = bits[x / 8] & (0x80 >> (x % 8)) != 0;

            pixels.push(if on { 255 } else { 0 });
          }
        }

        GlyphMask {
          width,
          height,
          left,
          top,
          pixels: MaskPixels::Alpha(pixels),
        }
      }
      PIXEL_MODE_BGRA => {
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height {
          for pixel in row(y)[..width as usize * 4].as_chunks::<4>().0 {
            pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
          }
        }

        let mask = GlyphMask {
          width,
          height,
          left,
          top,
          pixels: MaskPixels::Color(pixels),
        };

        if (scale - 1.).abs() > f32::EPSILON {
          mask.scaled(scale)
        } else {
          mask
        }
      }
      _ => return None,
    };

    Some(mask)
  }
}

impl Drop for Rasterizer {
  fn drop(&mut self) {
    self.masks.clear();
    self.faces.clear();
    unsafe { FT_Done_FreeType(self.library) };
  }
}

impl GlyphMask {
  /// The mask scaled by `scale` (a bitmap strike drawn at another size), each pixel the
  /// average of the source pixels it covers.
  #[must_use]
  pub fn scaled(&self, scale: f32) -> Self {
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      clippy::cast_precision_loss,
      reason = "pixel sizes of a glyph"
    )]
    let size = |value: u32| ((value as f32 * scale).round() as u32).max(1);
    let (width, height) = (size(self.width), size(self.height));
    let channels = match self.pixels {
      MaskPixels::Alpha(_) => 1,
      MaskPixels::Color(_) => 4,
    };
    let source = match &self.pixels {
      MaskPixels::Alpha(pixels) | MaskPixels::Color(pixels) => pixels,
    };
    let mut pixels = vec![0_u8; (width * height) as usize * channels];

    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      clippy::cast_precision_loss,
      reason = "pixel coordinates of a glyph"
    )]
    for y in 0..height {
      let (y0, y1) = (
        (y as f32 / scale).floor() as u32,
        (((y + 1) as f32 / scale).ceil() as u32).min(self.height),
      );

      for x in 0..width {
        let (x0, x1) = (
          (x as f32 / scale).floor() as u32,
          (((x + 1) as f32 / scale).ceil() as u32).min(self.width),
        );
        let mut sum = [0_u32; 4];
        let mut count = 0;

        for sy in y0..y1.max(y0 + 1).min(self.height) {
          for sx in x0..x1.max(x0 + 1).min(self.width) {
            let at = (sy * self.width + sx) as usize * channels;

            for channel in 0..channels {
              sum[channel] += u32::from(source[at + channel]);
            }

            count += 1;
          }
        }

        let at = (y * width + x) as usize * channels;

        for channel in 0..channels {
          pixels[at + channel] = (sum[channel] / count.max(1)) as u8;
        }
      }
    }

    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_precision_loss,
      reason = "a glyph's offsets in pixels"
    )]
    let offset = |value: i32| (value as f32 * scale).round() as i32;

    Self {
      width,
      height,
      left: offset(self.left),
      top: offset(self.top),
      pixels: match self.pixels {
        MaskPixels::Alpha(_) => MaskPixels::Alpha(pixels),
        MaskPixels::Color(_) => MaskPixels::Color(pixels),
      },
    }
  }
}
