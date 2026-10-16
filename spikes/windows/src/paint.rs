//! Three ways to put the same display list on pixels: vello_cpu with the glyphs as outlines
//! from the face's font file (the floor), vello_cpu compositing a DirectWrite-rasterized
//! grayscale atlas (the upgrade), and Direct2D drawing straight into a WIC bitmap (the
//! reference).

use std::{collections::HashMap, mem::ManuallyDrop, rc::Rc, sync::Arc};

use vello_cpu::{
  Glyph,
  Image,
  ImageSource,
  Pixmap,
  RasterizerSettings,
  RenderContext,
  RenderMode,
  RenderSettings,
  Resources,
  TargetInit,
  Tint,
  TintMode,
  color::{AlphaColor, Srgb},
  kurbo::{Affine, Rect, Shape},
  peniko::{Blob, Extend, FontData, ImageQuality, ImageSampler},
};
use windows::Win32::{
  Foundation::DWRITE_E_NOCOLOR,
  Graphics::{
    Direct2D::{Common::*, *},
    DirectWrite::*,
    Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
    Imaging::*,
  },
  System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
};
use windows_core::{Interface, Result};
use windows_numerics::{Matrix3x2, Vector2};

use crate::{
  engine::{Collected, GlyphRunRec, Laid, glyph_positions},
  image::Img,
};

#[derive(Clone)]
pub enum Item {
  Fill {
    rect: [f32; 4],
    color: [f32; 4],
  },
  // A paragraph with its origin snapped to whole pixels.
  Text {
    laid: Rc<Laid>,
    x: f32,
    y: f32,
    color: [f32; 4],
  },
  // Glyph runs already collected (from any engine), drawn at an offset.
  Glyphs {
    runs: Rc<Collected>,
    x: f32,
    y: f32,
    color: [f32; 4],
  },
  // A DirectWrite layout, drawn by the reference alone.
  Layout {
    layout: IDWriteTextLayout,
    x: f32,
    y: f32,
    color: [f32; 4],
  },
  PushClip([f32; 4]),
  PushOpacity(f32),
  Pop,
}

const fn rgba(c: [f32; 4]) -> AlphaColor<Srgb> {
  AlphaColor::new(c)
}

fn rect(r: [f32; 4]) -> Rect {
  Rect::new(
    r[0] as f64,
    r[1] as f64,
    (r[0] + r[2]) as f64,
    (r[1] + r[3]) as f64,
  )
}

pub fn new_context(w: u16, h: u16, threads: u16) -> RenderContext {
  RenderContext::new_with(
    w,
    h,
    RenderSettings {
      num_threads: threads,
      ..Default::default()
    },
  )
}

fn rasterize(ctx: &mut RenderContext, res: &mut Resources) -> Pixmap {
  ctx.flush();
  let mut pixmap = Pixmap::new(ctx.width(), ctx.height());
  ctx.render_with(
    &mut pixmap,
    res,
    RasterizerSettings {
      render_mode: RenderMode::OptimizeSpeed,
      target_init: TargetInit::Clear(AlphaColor::WHITE),
      ..Default::default()
    },
  );
  pixmap
}

pub fn to_img(pixmap: &Pixmap) -> Img {
  Img::from_rgba(
    pixmap.width() as usize,
    pixmap.height() as usize,
    pixmap.data_as_u8_slice(),
  )
}

/// The glyph runs of a text item (a paragraph's, collected once, or given), with its offset and
/// colour; `None` for every other item.
type TextRuns = Option<(Rc<Collected>, f32, f32, [f32; 4])>;

fn text_runs(item: &Item) -> Result<TextRuns> {
  match item {
    Item::Text { laid, x, y, color } => Ok(Some((laid.runs()?, *x, *y, *color))),
    Item::Glyphs { runs, x, y, color } => Ok(Some((runs.clone(), *x, *y, *color))),
    _ => Ok(None),
  }
}

/// Replays the structure of a display list into vello, handing the index of each text item to
/// `text`.
fn replay(
  ctx: &mut RenderContext,
  items: &[Item],
  mut text: impl FnMut(&mut RenderContext, usize),
) {
  for (at, item) in items.iter().enumerate() {
    match item {
      Item::Fill { rect: r, color } => {
        ctx.set_paint(rgba(*color));
        ctx.fill_rect(&rect(*r));
      }
      Item::Text { .. } | Item::Glyphs { .. } => text(ctx, at),
      Item::Layout { .. } => {}
      Item::PushClip(r) => ctx.push_clip_layer(&rect(*r).to_path(0.1)),
      Item::PushOpacity(o) => ctx.push_opacity_layer(*o),
      Item::Pop => ctx.pop_layer(),
    }
  }
}

fn draw_lines(ctx: &mut RenderContext, runs: &Collected, dx: f32, dy: f32, color: [f32; 4]) {
  ctx.set_paint(rgba(color));

  for l in &runs.lines {
    ctx.fill_rect(&rect([l[0] + dx, l[1] + dy, l[2], l[3]]));
  }
}

// ---------------------------------------------------------------------------------------------
// Font files behind DirectWrite's faces.

/// The file path of a face loaded by the local font file loader.
pub fn face_path(face: &IDWriteFontFace) -> Result<String> {
  unsafe {
    let mut n = 0u32;
    face.GetFiles(&mut n, None)?;
    let mut files: Vec<Option<IDWriteFontFile>> = vec![None; n as usize];
    face.GetFiles(&mut n, Some(files.as_mut_ptr()))?;
    let file = files
      .into_iter()
      .flatten()
      .next()
      .ok_or_else(windows_core::Error::empty)?;
    let mut key = std::ptr::null_mut();
    let mut size = 0u32;
    file.GetReferenceKey(&mut key, &mut size)?;
    let loader: IDWriteLocalFontFileLoader = file.GetLoader()?.cast()?;
    let len = loader.GetFilePathLengthFromKey(key, size)?;
    let mut buf = vec![0u16; len as usize + 1];
    loader.GetFilePathFromKey(key, size, &mut buf)?;
    Ok(String::from_utf16_lossy(&buf[..len as usize]))
  }
}

#[derive(Default)]
pub struct Fonts {
  by_face: HashMap<usize, Option<FontData>>,
  by_path: HashMap<String, Blob<u8>>,
  // Faces kept alive so a face's address never names another face.
  keep: Vec<IDWriteFontFace>,
  pub simulated_runs: u64,
  pub unresolved: u64,
}

impl Fonts {
  pub fn font(&mut self, face: &IDWriteFontFace) -> Option<FontData> {
    let key = face.as_raw() as usize;

    if let Some(font) = self.by_face.get(&key) {
      return font.clone();
    }

    let font = face_path(face).ok().and_then(|path| {
      let blob = match self.by_path.get(&path) {
        Some(blob) => blob.clone(),
        None => {
          let bytes = std::fs::read(&path).ok()?;
          let blob = Blob::new(Arc::new(bytes));
          self.by_path.insert(path, blob.clone());
          blob
        }
      };
      Some(FontData::new(blob, unsafe { face.GetIndex() }))
    });

    if font.is_none() {
      self.unresolved += 1;
    }

    self.keep.push(face.clone());
    self.by_face.insert(key, font.clone());
    font
  }
}

// ---------------------------------------------------------------------------------------------
// The floor: vello draws each glyph as an outline from the face's font file.

pub struct Floor {
  pub fonts: Fonts,
  pub res: Resources,
  pub hint: bool,
  pub glyph_errors: u64,
}

impl Floor {
  pub fn new() -> Self {
    Self {
      fonts: Fonts::default(),
      res: Resources::new(),
      hint: true,
      glyph_errors: 0,
    }
  }

  pub fn render(&mut self, ctx: &mut RenderContext, items: &[Item]) -> Result<Pixmap> {
    ctx.reset();
    let texts: Vec<TextRuns> = items.iter().map(text_runs).collect::<Result<_>>()?;

    replay(ctx, items, |ctx, at| {
      if let Some((runs, dx, dy, color)) = &texts[at] {
        for run in &runs.runs {
          self.run(ctx, run, *dx, *dy, *color);
        }

        draw_lines(ctx, runs, *dx, *dy, *color);
      }
    });

    Ok(rasterize(ctx, &mut self.res))
  }

  fn run(&mut self, ctx: &mut RenderContext, run: &GlyphRunRec, dx: f32, dy: f32, color: [f32; 4]) {
    let Some(font) = self.fonts.font(&run.face)
    else {
      return;
    };
    let simulations = unsafe { run.face.GetSimulations() };

    if simulations != DWRITE_FONT_SIMULATIONS_NONE {
      self.fonts.simulated_runs += 1;
    }

    let positions = run.positions();
    let glyphs: Vec<Glyph> = run
      .glyphs
      .iter()
      .zip(&positions)
      .map(|(&id, &(x, y))| Glyph {
        id: id as u32,
        x: x + dx,
        y: y + dy,
      })
      .collect();

    ctx.set_paint(rgba(color));

    // A simulated face drawn as Skene's renderer draws Parley's synthesis: Skia's emboldening,
    // and a 14-degree slant about the run's baseline.
    let oblique = simulations.0 & DWRITE_FONT_SIMULATIONS_OBLIQUE.0 != 0;
    let bold = simulations.0 & DWRITE_FONT_SIMULATIONS_BOLD.0 != 0;

    if oblique {
      let baseline = f64::from(run.origin.1 + dy);
      ctx.set_transform(
        Affine::translate((0., baseline))
          * Affine::new([1., 0., -(14f64.to_radians().tan()), 1., 0., 0.])
          * Affine::translate((0., -baseline)),
      );
    }

    let mut builder = ctx
      .glyph_run(&mut self.res, &font)
      .font_size(run.em)
      .hint(self.hint);

    if bold {
      let per_side = text_parity::render::embolden_per_side(run.em);
      let upem = units_per_em(&font);
      let amount = f64::from(per_side * upem / run.em);
      builder = builder.font_embolden(glifo::FontEmbolden::new(vello_cpu::kurbo::Diagonal2::new(
        amount, amount,
      )));
    }

    if builder.fill_glyphs(glyphs.into_iter()).is_err() {
      self.glyph_errors += 1;
    }

    if oblique {
      ctx.reset_transform();
    }
  }
}

// ---------------------------------------------------------------------------------------------
// The upgrade: DirectWrite rasterizes each glyph to grayscale coverage in an atlas, and vello
// composites atlas cells as tinted images.

const ATLAS: u16 = 2048;
const SUBPIXEL: f32 = 4.0;

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct GlyphKey {
  face: usize,
  glyph: u16,
  em: u32,
  sub: u8,
  // The text colour a Direct2D colour cell was drawn with; 0 for a coverage cell.
  colour: u32,
}

#[derive(Clone, Copy)]
struct Cell {
  ax: u16,
  ay: u16,
  w: u16,
  h: u16,
  left: i32,
  top: i32,
  // In the colour atlas (premultiplied colour), not the coverage atlas.
  colour: bool,
}

struct Draw {
  cell: Cell,
  x: i32,
  y: i32,
  color: [f32; 4],
}

/// How the atlas path draws a colour font's glyphs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColourPath {
  /// `TranslateColorGlyphRun` into COLR v0 layers, each a tinted coverage cell.
  Layers,
  /// Direct2D's `DrawGlyphRunWithColorSupport` (Windows 11) into a colour cell.
  Direct2D,
}

const COLOUR_CANVAS: u32 = 256;

/// A small Direct2D target that draws one colour glyph at a time.
struct ColourRaster {
  bitmap: IWICBitmap,
  rt: ID2D1RenderTarget,
  dc7: ID2D1DeviceContext7,
  brush: ID2D1SolidColorBrush,
}

impl ColourRaster {
  fn new() -> Result<Self> {
    let d2d: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
    let wic: IWICImagingFactory =
      unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };
    let bitmap = unsafe {
      wic.CreateBitmap(
        COLOUR_CANVAS,
        COLOUR_CANVAS,
        &GUID_WICPixelFormat32bppPBGRA,
        WICBitmapCacheOnLoad,
      )?
    };
    let rt = unsafe { d2d.CreateWicBitmapRenderTarget(&bitmap, &target_properties())? };
    let dc7: ID2D1DeviceContext7 = rt.cast()?;
    let brush = unsafe {
      rt.CreateSolidColorBrush(
        &D2D1_COLOR_F {
          r: 0.0,
          g: 0.0,
          b: 0.0,
          a: 1.0,
        },
        None,
      )?
    };
    Ok(Self {
      bitmap,
      rt,
      dc7,
      brush,
    })
  }

  /// Draws one glyph with its baseline origin at a fixed point of the canvas and returns the
  /// premultiplied RGBA pixels with the origin's position in them.
  fn draw(&self, run: &DWRITE_GLYPH_RUN, sub: f32, color: [f32; 4]) -> Result<(Vec<u8>, i32, i32)> {
    let ox = (COLOUR_CANVAS / 4) as i32;
    let oy = (COLOUR_CANVAS * 5 / 8) as i32;

    unsafe {
      self.brush.SetColor(&d2d_color(color));
      self.rt.BeginDraw();
      self
        .rt
        .SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
      self.rt.Clear(Some(&D2D1_COLOR_F {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
      }));
      self.dc7.DrawGlyphRunWithColorSupport(
        Vector2 {
          X: ox as f32 + sub,
          Y: oy as f32,
        },
        run,
        None,
        &self.brush,
        None,
        0,
        DWRITE_MEASURING_MODE_NATURAL,
        D2D1_COLOR_BITMAP_GLYPH_SNAP_OPTION_DEFAULT,
      );
      self.rt.EndDraw(None, None)?;
    }

    let mut bgra = vec![0u8; (COLOUR_CANVAS * COLOUR_CANVAS * 4) as usize];
    unsafe {
      self
        .bitmap
        .CopyPixels(std::ptr::null(), COLOUR_CANVAS * 4, &mut bgra)?
    };
    Ok((bgra, ox, oy))
  }
}

pub struct AtlasPainter {
  factory: IDWriteFactory4,
  atlas: Arc<Pixmap>,
  cursor: (u16, u16, u16),
  colour_atlas: Arc<Pixmap>,
  colour_cursor: (u16, u16, u16),
  raster: Option<ColourRaster>,
  pub colour_path: ColourPath,
  // Why the Direct2D colour path is unavailable, where it is.
  pub colour_unavailable: Option<String>,
  pub colour_cells: u64,
  cells: HashMap<GlyphKey, Cell>,
  keep: Vec<IDWriteFontFace>,
  color_faces: HashMap<usize, bool>,
  pub contrast: f32,
  pub res: Resources,
  pub rasterized: u64,
  pub color_layers: u64,
  pub unsupported_layers: u64,
  pub raster_errors: u64,
  pub first_error: Option<String>,
  // The image formats of the layers this painter could not draw.
  pub layer_formats: Vec<i32>,
  // The image formats of every layer seen.
  pub seen_formats: Vec<i32>,
}

impl AtlasPainter {
  pub fn new(factory: &IDWriteFactory4) -> Result<Self> {
    let params: IDWriteRenderingParams1 = unsafe { factory.CreateRenderingParams()? }.cast()?;
    let (raster, colour_unavailable) = match ColourRaster::new() {
      Ok(raster) => (Some(raster), None),
      Err(e) => (None, Some(format!("{e}"))),
    };
    Ok(Self {
      factory: factory.clone(),
      atlas: Arc::new(Pixmap::new(ATLAS, ATLAS)),
      cursor: (0, 0, 0),
      colour_atlas: Arc::new(Pixmap::new(ATLAS, ATLAS)),
      colour_cursor: (0, 0, 0),
      colour_path: if raster.is_some() {
        ColourPath::Direct2D
      }
      else {
        ColourPath::Layers
      },
      raster,
      colour_unavailable,
      colour_cells: 0,
      cells: HashMap::new(),
      keep: Vec::new(),
      color_faces: HashMap::new(),
      contrast: unsafe { params.GetGrayscaleEnhancedContrast() },
      res: Resources::new(),
      rasterized: 0,
      color_layers: 0,
      unsupported_layers: 0,
      raster_errors: 0,
      first_error: None,
      layer_formats: Vec::new(),
      seen_formats: Vec::new(),
    })
  }

  pub fn used_rows(&self) -> u16 {
    self.cursor.1 + self.cursor.2
  }

  /// Forgets every cell, so the atlases fill again from the top: one image at a time.
  pub fn reset(&mut self) {
    self.cells.clear();
    self.keep.clear();
    self.cursor = (0, 0, 0);
    self.colour_cursor = (0, 0, 0);
  }

  fn cell(&mut self, face: &IDWriteFontFace, em: f32, glyph: u16, sub: u8) -> Option<Cell> {
    let key = GlyphKey {
      face: face.as_raw() as usize,
      glyph,
      em: em.to_bits(),
      sub,
      colour: 0,
    };

    if let Some(cell) = self.cells.get(&key) {
      return Some(*cell);
    }

    let cell = match self.rasterize(face, em, glyph, sub) {
      Ok(cell) => cell,
      Err(e) => {
        self.raster_errors += 1;
        self.first_error.get_or_insert_with(|| format!("{e}"));
        return None;
      }
    };

    self.keep.push(face.clone());
    self.cells.insert(key, cell);
    Some(cell)
  }

  fn rasterize(&mut self, face: &IDWriteFontFace, em: f32, glyph: u16, sub: u8) -> Result<Cell> {
    self.rasterized += 1;
    let advance = 0.0f32;
    let offset = DWRITE_GLYPH_OFFSET::default();
    let run = ManuallyDrop::new(DWRITE_GLYPH_RUN {
      fontFace: ManuallyDrop::new(Some(unsafe { std::mem::transmute_copy(face) })),
      fontEmSize: em,
      glyphCount: 1,
      glyphIndices: &glyph,
      glyphAdvances: &advance,
      glyphOffsets: &offset,
      isSideways: false.into(),
      bidiLevel: 0,
    });
    let analysis = unsafe {
      IDWriteFactory2::CreateGlyphRunAnalysis(
        &self.factory,
        &*run,
        None,
        DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
        DWRITE_MEASURING_MODE_NATURAL,
        DWRITE_GRID_FIT_MODE_DEFAULT,
        DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
        sub as f32 / SUBPIXEL,
        0.0,
      )?
    };
    let bounds = unsafe { analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_ALIASED_1x1)? };
    let w = (bounds.right - bounds.left).max(0) as u16;
    let h = (bounds.bottom - bounds.top).max(0) as u16;
    let mut cell = Cell {
      ax: 0,
      ay: 0,
      w,
      h,
      left: bounds.left,
      top: bounds.top,
      colour: false,
    };

    if w == 0 || h == 0 {
      return Ok(cell);
    }

    let mut alpha = vec![0u8; w as usize * h as usize];
    unsafe { analysis.CreateAlphaTexture(DWRITE_TEXTURE_ALIASED_1x1, &bounds, &mut alpha)? };

    let (x, y) = shelf(&mut self.cursor, w, h);
    cell.ax = x;
    cell.ay = y;

    let k = self.contrast;
    let atlas = Arc::make_mut(&mut self.atlas);

    for row in 0..h {
      for col in 0..w {
        let a = alpha[row as usize * w as usize + col as usize] as f32 / 255.0;
        // DirectWrite's grayscale enhanced contrast, as its own blending applies it.
        let a = ((a * (k + 1.0) / (a * k + 1.0)) * 255.0).round() as u8;
        atlas.set_pixel(
          x + col,
          y + row,
          vello_cpu::color::PremulRgba8 {
            r: a,
            g: a,
            b: a,
            a,
          },
        );
      }
    }

    Ok(cell)
  }

  fn colour_cell(
    &mut self,
    face: &IDWriteFontFace,
    em: f32,
    glyph: u16,
    sub: u8,
    color: [f32; 4],
  ) -> Option<Cell> {
    let packed = u32::from_le_bytes(color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)) | 1;
    let key = GlyphKey {
      face: face.as_raw() as usize,
      glyph,
      em: em.to_bits(),
      sub,
      colour: packed,
    };

    if let Some(cell) = self.cells.get(&key) {
      return Some(*cell);
    }

    let advance = 0.0f32;
    let offset = DWRITE_GLYPH_OFFSET::default();
    let run = ManuallyDrop::new(DWRITE_GLYPH_RUN {
      fontFace: ManuallyDrop::new(Some(unsafe { std::mem::transmute_copy(face) })),
      fontEmSize: em,
      glyphCount: 1,
      glyphIndices: &glyph,
      glyphAdvances: &advance,
      glyphOffsets: &offset,
      isSideways: false.into(),
      bidiLevel: 0,
    });
    let drawn = self
      .raster
      .as_ref()?
      .draw(&run, sub as f32 / SUBPIXEL, color);
    let (bgra, ox, oy) = match drawn {
      Ok(drawn) => drawn,
      Err(e) => {
        self.raster_errors += 1;
        self.first_error.get_or_insert_with(|| format!("{e}"));
        return None;
      }
    };
    self.colour_cells += 1;
    let size = COLOUR_CANVAS as usize;
    let (mut x0, mut y0, mut x1, mut y1) = (size, size, 0, 0);

    for y in 0..size {
      for x in 0..size {
        if bgra[(y * size + x) * 4 + 3] != 0 {
          x0 = x0.min(x);
          y0 = y0.min(y);
          x1 = x1.max(x + 1);
          y1 = y1.max(y + 1);
        }
      }
    }

    let mut cell = Cell {
      ax: 0,
      ay: 0,
      w: x1.saturating_sub(x0) as u16,
      h: y1.saturating_sub(y0) as u16,
      left: x0 as i32 - ox,
      top: y0 as i32 - oy,
      colour: true,
    };

    if cell.w > 0 && cell.h > 0 {
      let (ax, ay) = shelf(&mut self.colour_cursor, cell.w, cell.h);
      cell.ax = ax;
      cell.ay = ay;
      let atlas = Arc::make_mut(&mut self.colour_atlas);

      for y in y0..y1 {
        for x in x0..x1 {
          let p = &bgra[(y * size + x) * 4..][..4];
          atlas.set_pixel(
            ax + (x - x0) as u16,
            ay + (y - y0) as u16,
            vello_cpu::color::PremulRgba8 {
              r: p[2],
              g: p[1],
              b: p[0],
              a: p[3],
            },
          );
        }
      }
    }

    self.keep.push(face.clone());
    self.cells.insert(key, cell);
    Some(cell)
  }

  fn is_color(&mut self, face: &IDWriteFontFace) -> bool {
    let key = face.as_raw() as usize;
    *self.color_faces.entry(key).or_insert_with(|| {
      face
        .cast::<IDWriteFontFace2>()
        .map(|f| unsafe { f.IsColorFont() }.as_bool())
        .unwrap_or(false)
    })
  }

  #[expect(clippy::too_many_arguments)]
  fn glyphs(
    &mut self,
    out: &mut Vec<Draw>,
    face: &IDWriteFontFace,
    em: f32,
    glyphs: &[u16],
    positions: &[(f32, f32)],
    color: [f32; 4],
    colour: bool,
  ) {
    for (&glyph, &(x, y)) in glyphs.iter().zip(positions) {
      let mut ox = x.floor();
      let mut sub = ((x - ox) * SUBPIXEL).round();

      if sub >= SUBPIXEL {
        ox += 1.0;
        sub = 0.0;
      }

      let cell = if colour {
        self.colour_cell(face, em, glyph, sub as u8, color)
      }
      else {
        self.cell(face, em, glyph, sub as u8)
      };

      if let Some(cell) = cell
        && cell.w > 0
      {
        out.push(Draw {
          cell,
          x: ox as i32 + cell.left,
          y: y.round() as i32 + cell.top,
          color,
        });
      }
    }
  }

  fn run(
    &mut self,
    out: &mut Vec<Draw>,
    run: &GlyphRunRec,
    dx: f32,
    dy: f32,
    color: [f32; 4],
  ) -> Result<()> {
    let origin = (run.origin.0 + dx, run.origin.1 + dy);

    if self.is_color(&run.face) && self.colour_path == ColourPath::Direct2D && self.raster.is_some()
    {
      let positions = glyph_positions(origin, run.bidi, &run.advances, &run.offsets);
      self.glyphs(out, &run.face, run.em, &run.glyphs, &positions, color, true);
      return Ok(());
    }

    if !self.is_color(&run.face) {
      let positions = glyph_positions(origin, run.bidi, &run.advances, &run.offsets);
      self.glyphs(
        out,
        &run.face,
        run.em,
        &run.glyphs,
        &positions,
        color,
        false,
      );
      return Ok(());
    }

    let glyph_run = run.as_run();
    let formats = DWRITE_GLYPH_IMAGE_FORMATS_TRUETYPE
      | DWRITE_GLYPH_IMAGE_FORMATS_CFF
      | DWRITE_GLYPH_IMAGE_FORMATS_COLR;
    let layers = match unsafe {
      self.factory.TranslateColorGlyphRun(
        Vector2 {
          X: origin.0,
          Y: origin.1,
        },
        &*glyph_run,
        None,
        formats,
        DWRITE_MEASURING_MODE_NATURAL,
        None,
        0,
      )
    } {
      Ok(layers) => layers,
      Err(e) if e.code() == DWRITE_E_NOCOLOR => {
        let positions = glyph_positions(origin, run.bidi, &run.advances, &run.offsets);
        self.glyphs(
          out,
          &run.face,
          run.em,
          &run.glyphs,
          &positions,
          color,
          false,
        );
        return Ok(());
      }
      Err(e) => return Err(e),
    };

    while unsafe { layers.MoveNext()? }.as_bool() {
      let layer = unsafe { &*layers.GetCurrentRun()? };
      let base = &layer.Base;
      let g = &base.glyphRun;
      let n = g.glyphCount as usize;

      let outline = (DWRITE_GLYPH_IMAGE_FORMATS_TRUETYPE
        | DWRITE_GLYPH_IMAGE_FORMATS_CFF
        | DWRITE_GLYPH_IMAGE_FORMATS_COLR)
        .0;
      let format = layer.glyphImageFormat.0;

      if !self.seen_formats.contains(&format) {
        self.seen_formats.push(format);
      }

      if format == 0 || format & !outline != 0 {
        self.unsupported_layers += 1;
        let format = layer.glyphImageFormat.0;

        if !self.layer_formats.contains(&format) {
          self.layer_formats.push(format);
        }
        continue;
      }

      self.color_layers += 1;
      let layer_color = if base.paletteIndex == 0xFFFF {
        color
      }
      else {
        [
          base.runColor.r,
          base.runColor.g,
          base.runColor.b,
          base.runColor.a * color[3],
        ]
      };
      let glyphs = unsafe { std::slice::from_raw_parts(g.glyphIndices, n) }.to_vec();
      let advances = if g.glyphAdvances.is_null() {
        vec![0.0; n]
      }
      else {
        unsafe { std::slice::from_raw_parts(g.glyphAdvances, n) }.to_vec()
      };
      let offsets = if g.glyphOffsets.is_null() {
        vec![DWRITE_GLYPH_OFFSET::default(); n]
      }
      else {
        unsafe { std::slice::from_raw_parts(g.glyphOffsets, n) }.to_vec()
      };
      let positions = glyph_positions(
        (base.baselineOriginX, base.baselineOriginY),
        g.bidiLevel,
        &advances,
        &offsets,
      );
      let face = g.fontFace.as_ref().unwrap_or(&run.face).clone();
      self.glyphs(
        out,
        &face,
        g.fontEmSize,
        &glyphs,
        &positions,
        layer_color,
        false,
      );
    }

    Ok(())
  }

  pub fn render(&mut self, ctx: &mut RenderContext, items: &[Item]) -> Result<Pixmap> {
    // The context holds the previous frame's references to the atlas until it is reset, so
    // reset first and rasterize every missing glyph before drawing anything.
    ctx.reset();
    let mut draws: Vec<Vec<Draw>> = Vec::with_capacity(items.len());
    let mut lines: Vec<TextRuns> = Vec::with_capacity(items.len());

    for item in items {
      let mut out = Vec::new();
      let text = text_runs(item)?;

      if let Some((runs, x, y, color)) = &text {
        for run in &runs.runs {
          self.run(&mut out, run, *x, *y, *color)?;
        }
      }

      draws.push(out);
      lines.push(text);
    }

    let atlas = self.atlas.clone();
    let colour_atlas = self.colour_atlas.clone();
    let image = |atlas: &Arc<Pixmap>| Image {
      image: ImageSource::Pixmap(atlas.clone()),
      sampler: ImageSampler {
        x_extend: Extend::Pad,
        y_extend: Extend::Pad,
        quality: ImageQuality::Low,
        alpha: 1.0,
      },
    };
    replay(ctx, items, |ctx, at| {
      for d in &draws[at] {
        if d.cell.colour {
          ctx.set_paint(image(&colour_atlas));
          ctx.reset_tint();
        }
        else {
          ctx.set_paint(image(&atlas));
          ctx.set_tint(Some(Tint {
            color: rgba(d.color),
            mode: TintMode::AlphaMask,
          }));
        }

        ctx.set_paint_transform(Affine::translate((
          d.x as f64 - d.cell.ax as f64,
          d.y as f64 - d.cell.ay as f64,
        )));
        ctx.fill_rect(&Rect::new(
          d.x as f64,
          d.y as f64,
          (d.x + d.cell.w as i32) as f64,
          (d.y + d.cell.h as i32) as f64,
        ));
      }

      ctx.reset_tint();
      ctx.reset_paint_transform();

      if let Some((runs, dx, dy, color)) = &lines[at] {
        draw_lines(ctx, runs, *dx, *dy, *color);
      }
    });

    drop(atlas);
    drop(colour_atlas);
    Ok(rasterize(ctx, &mut self.res))
  }

  pub fn save_atlas(&self, path: &std::path::Path) {
    let rows = self.used_rows().max(1).min(ATLAS);
    let img = to_img(&self.atlas);
    let region = crate::image::Region {
      x0: 0,
      y0: 0,
      x1: ATLAS as usize,
      y1: rows as usize,
    };
    let mut crop = img.crop(region);

    for p in &mut crop.px {
      let a = p[3];
      *p = [255 - a, 255 - a, 255 - a, 255];
    }

    crop.save(path);
  }
}

// ---------------------------------------------------------------------------------------------
// The reference: Direct2D's own text drawing into a WIC bitmap (software, grayscale).

pub struct Reference {
  d2d: ID2D1Factory,
  wic: IWICImagingFactory,
}

impl Reference {
  pub fn new() -> Result<Self> {
    let d2d: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
    let wic: IWICImagingFactory =
      unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };
    Ok(Self { d2d, wic })
  }

  pub fn render(&self, items: &[Item], w: u32, h: u32) -> Result<Img> {
    self.render_at(items, w, h, 1.0)
  }

  /// `items`, in DIPs, drawn into `w` by `h` pixels at `scale` pixels per DIP.
  pub fn render_at(&self, items: &[Item], w: u32, h: u32, scale: f32) -> Result<Img> {
    let bitmap = unsafe {
      self
        .wic
        .CreateBitmap(w, h, &GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad)?
    };
    let props = D2D1_RENDER_TARGET_PROPERTIES {
      r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
      pixelFormat: D2D1_PIXEL_FORMAT {
        format: DXGI_FORMAT_B8G8R8A8_UNORM,
        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
      },
      dpiX: 96.0 * scale,
      dpiY: 96.0 * scale,
      usage: D2D1_RENDER_TARGET_USAGE_NONE,
      minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };
    let rt = unsafe { self.d2d.CreateWicBitmapRenderTarget(&bitmap, &props)? };
    let dc7 = rt.cast::<ID2D1DeviceContext7>().ok();
    let brush = unsafe {
      rt.CreateSolidColorBrush(
        &D2D1_COLOR_F {
          r: 0.0,
          g: 0.0,
          b: 0.0,
          a: 1.0,
        },
        None,
      )?
    };
    let mut stack = Vec::new();

    unsafe {
      rt.BeginDraw();
      rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
      rt.Clear(Some(&D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
      }));

      for item in items {
        match item {
          Item::Fill { rect: r, color } => {
            brush.SetColor(&d2d_color(*color));
            rt.FillRectangle(&d2d_rect(*r), &brush);
          }
          Item::Text { laid, x, y, color } => {
            brush.SetColor(&d2d_color(*color));

            for piece in laid.pieces() {
              rt.DrawTextLayout(
                Vector2 {
                  X: *x,
                  Y: *y + piece.y,
                },
                &piece.layout,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
              );
            }
          }
          Item::Layout {
            layout,
            x,
            y,
            color,
          } => {
            brush.SetColor(&d2d_color(*color));
            rt.DrawTextLayout(
              Vector2 { X: *x, Y: *y },
              layout,
              &brush,
              D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            );
          }
          Item::Glyphs { runs, x, y, color } => {
            brush.SetColor(&d2d_color(*color));

            for run in &runs.runs {
              let glyph_run = run.as_run();
              let origin = Vector2 {
                X: run.origin.0 + *x,
                Y: run.origin.1 + *y,
              };

              match &dc7 {
                Some(dc7) => dc7.DrawGlyphRunWithColorSupport(
                  origin,
                  &*glyph_run,
                  None,
                  &brush,
                  None,
                  0,
                  DWRITE_MEASURING_MODE_NATURAL,
                  D2D1_COLOR_BITMAP_GLYPH_SNAP_OPTION_DEFAULT,
                ),
                None => rt.DrawGlyphRun(origin, &*glyph_run, &brush, DWRITE_MEASURING_MODE_NATURAL),
              }
            }
          }
          Item::PushClip(r) => {
            rt.PushAxisAlignedClip(&d2d_rect(*r), D2D1_ANTIALIAS_MODE_ALIASED);
            stack.push(false);
          }
          Item::PushOpacity(o) => {
            let layer = rt.CreateLayer(None)?;
            let params = D2D1_LAYER_PARAMETERS {
              contentBounds: D2D_RECT_F {
                left: -1e7,
                top: -1e7,
                right: 1e7,
                bottom: 1e7,
              },
              geometricMask: ManuallyDrop::new(None),
              maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
              maskTransform: Matrix3x2::identity(),
              opacity: *o,
              opacityBrush: ManuallyDrop::new(None),
              layerOptions: D2D1_LAYER_OPTIONS_NONE,
            };
            rt.PushLayer(&params, &layer);
            stack.push(true);
          }
          Item::Pop => {
            if stack.pop() == Some(true) {
              rt.PopLayer();
            }
            else {
              rt.PopAxisAlignedClip();
            }
          }
        }
      }

      rt.EndDraw(None, None)?;
    }

    let mut bytes = vec![0u8; (w * h * 4) as usize];
    unsafe { bitmap.CopyPixels(std::ptr::null(), w * 4, &mut bytes)? };
    Ok(Img::from_bgra(w as usize, h as usize, &bytes))
  }
}

/// The units per em of `font`.
pub fn units_per_em(font: &FontData) -> f32 {
  use skrifa::MetadataProvider as _;

  skrifa::FontRef::from_index(font.data.data(), font.index)
    .map(|f| {
      f32::from(
        f.metrics(
          skrifa::instance::Size::unscaled(),
          skrifa::instance::LocationRef::default(),
        )
        .units_per_em,
      )
    })
    .unwrap_or(2048.0)
}

const fn d2d_color(c: [f32; 4]) -> D2D1_COLOR_F {
  D2D1_COLOR_F {
    r: c[0],
    g: c[1],
    b: c[2],
    a: c[3],
  }
}

fn d2d_rect(r: [f32; 4]) -> D2D_RECT_F {
  D2D_RECT_F {
    left: r[0],
    top: r[1],
    right: r[0] + r[2],
    bottom: r[1] + r[3],
  }
}

fn target_properties() -> D2D1_RENDER_TARGET_PROPERTIES {
  D2D1_RENDER_TARGET_PROPERTIES {
    r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
    pixelFormat: D2D1_PIXEL_FORMAT {
      format: DXGI_FORMAT_B8G8R8A8_UNORM,
      alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
    },
    dpiX: 96.0,
    dpiY: 96.0,
    usage: D2D1_RENDER_TARGET_USAGE_NONE,
    minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
  }
}

/// Shelf packing with one pixel of padding: the cell's position in its atlas.
fn shelf(cursor: &mut (u16, u16, u16), w: u16, h: u16) -> (u16, u16) {
  let (mut x, mut y, mut row) = *cursor;

  if x + w + 1 > ATLAS {
    x = 0;
    y += row + 1;
    row = 0;
  }

  assert!(y + h < ATLAS, "the spike's atlas is full");
  *cursor = (x + w + 1, y, row.max(h));
  (x, y)
}
