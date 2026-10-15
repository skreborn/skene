//! DirectWrite as the text engine: a paragraph with its intrinsic widths, an immutable
//! `Laid` per width (each owning its own `IDWriteTextLayout`, never re-wrapped), the taffy
//! leaf measure, and a custom `IDWriteTextRenderer` that collects glyph runs for Skene's
//! own renderer.

use std::{
  cell::{Cell, OnceCell, RefCell},
  collections::HashMap,
  mem::ManuallyDrop,
  rc::Rc,
};

use taffy::{Baselines, LayoutInput, LayoutOutput, Rect as TRect, prelude::*};
use windows::Win32::Graphics::DirectWrite::*;
use windows_core::{BOOL, HRESULT, IUnknown, IUnknownImpl, PCWSTR, Ref, Result, implement};

/// HRESULT_FROM_WIN32(ERROR_INSUFFICIENT_BUFFER): the size-query answer of DirectWrite's
/// two-call getters.
const E_NOT_SUFFICIENT_BUFFER: HRESULT = HRESULT(0x8007007A_u32 as i32);

pub fn wide(s: &str) -> Vec<u16> {
  s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// How a paragraph capped at `max_lines` ends its last visible line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EllipsisStrategy {
  /// One layout: `SetMaxHeight` to the first N lines plus the trimming sign on the layout.
  Native,
  /// Two layouts: the first N-1 lines wrapped, then the rest on one `NO_WRAP` line trimmed
  /// with the sign.
  Split,
}

#[derive(Clone, Debug)]
pub struct ParagraphStyle {
  pub family: String,
  pub size: f32,
  pub weight: i32,
  pub wrap: bool,
  pub max_lines: Option<u32>,
  pub ellipsis: EllipsisStrategy,
  pub locale: String,
}

impl Default for ParagraphStyle {
  fn default() -> Self {
    Self {
      family: "Segoe UI".into(),
      size: 16.0,
      weight: 400,
      wrap: true,
      max_lines: None,
      ellipsis: EllipsisStrategy::Native,
      locale: "en-US".into(),
    }
  }
}

pub struct Counters {
  pub laid_created: Cell<u64>,
  pub laid_cache_hits: Cell<u64>,
  pub measure_calls: Cell<u64>,
}

pub struct Engine {
  pub factory: IDWriteFactory4,
  formats: RefCell<HashMap<(String, u32, i32, bool, String), IDWriteTextFormat>>,
  pub counters: Counters,
}

impl Engine {
  pub fn new() -> Result<Rc<Self>> {
    let factory: IDWriteFactory4 = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
    Ok(Rc::new(Self {
      factory,
      formats: RefCell::new(HashMap::new()),
      counters: Counters {
        laid_created: Cell::new(0),
        laid_cache_hits: Cell::new(0),
        measure_calls: Cell::new(0),
      },
    }))
  }

  fn format(&self, s: &ParagraphStyle) -> Result<IDWriteTextFormat> {
    let key = (
      s.family.clone(),
      s.size.to_bits(),
      s.weight,
      s.wrap,
      s.locale.clone(),
    );

    if let Some(f) = self.formats.borrow().get(&key) {
      return Ok(f.clone());
    }

    let family = wide(&s.family);
    let locale = wide(&s.locale);
    let format = unsafe {
      IDWriteFactory::CreateTextFormat(
        &self.factory,
        PCWSTR(family.as_ptr()),
        None,
        DWRITE_FONT_WEIGHT(s.weight),
        DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_STRETCH_NORMAL,
        s.size,
        PCWSTR(locale.as_ptr()),
      )?
    };

    unsafe {
      format.SetWordWrapping(if s.wrap {
        DWRITE_WORD_WRAPPING_EMERGENCY_BREAK
      }
      else {
        DWRITE_WORD_WRAPPING_NO_WRAP
      })?;
    }

    self.formats.borrow_mut().insert(key, format.clone());
    Ok(format)
  }

  pub fn paragraph(self: &Rc<Self>, text: &str, style: ParagraphStyle) -> Result<Paragraph> {
    let format = self.format(&style)?;
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let sign = if style.max_lines.is_some() {
      Some(unsafe { self.factory.CreateEllipsisTrimmingSign(&format)? })
    }
    else {
      None
    };
    let probe = unsafe {
      IDWriteFactory::CreateTextLayout(&self.factory, &utf16, &format, f32::MAX, f32::MAX)?
    };
    let min = unsafe { probe.DetermineMinWidth()? };
    let mut metrics = DWRITE_TEXT_METRICS::default();
    unsafe { probe.GetMetrics(&mut metrics)? };

    Ok(Paragraph {
      engine: self.clone(),
      utf16,
      format,
      sign,
      style,
      min,
      max: metrics.widthIncludingTrailingWhitespace,
      cache: RefCell::new(Vec::new()),
      probe,
    })
  }
}

// ---------------------------------------------------------------------------------------------
// The paragraph: intrinsic widths once, then an immutable `Laid` per width.

pub struct Paragraph {
  engine: Rc<Engine>,
  pub utf16: Vec<u16>,
  format: IDWriteTextFormat,
  sign: Option<IDWriteInlineObject>,
  pub style: ParagraphStyle,
  pub min: f32,
  pub max: f32,
  /// A small per-node cache of laid values by width (`None` is unbounded).
  cache: RefCell<Vec<(Option<u32>, Rc<Laid>)>>,
  /// Only for the comparison with the old design: one layout re-wrapped by `SetMaxWidth`.
  probe: IDWriteTextLayout,
}

const CACHE_ENTRIES: usize = 3;

impl Paragraph {
  pub const fn intrinsic(&self) -> (f32, f32) {
    (self.min, self.max)
  }

  /// The paragraph laid out at `width` (`None`: unbounded), as an immutable value.
  pub fn layout(&self, width: Option<f32>) -> Result<Rc<Laid>> {
    let key = width.map(f32::to_bits);

    if let Some((_, laid)) = self.cache.borrow().iter().find(|(k, _)| *k == key) {
      self
        .engine
        .counters
        .laid_cache_hits
        .set(self.engine.counters.laid_cache_hits.get() + 1);
      return Ok(laid.clone());
    }

    let laid = Rc::new(self.build(width)?);
    let mut cache = self.cache.borrow_mut();

    if cache.len() == CACHE_ENTRIES {
      cache.remove(0);
    }

    cache.push((key, laid.clone()));
    Ok(laid)
  }

  /// A fresh layout object for `text`; no other `Laid` shares it.
  fn fresh(&self, text: &[u16], width: f32) -> Result<IDWriteTextLayout> {
    self
      .engine
      .counters
      .laid_created
      .set(self.engine.counters.laid_created.get() + 1);
    unsafe {
      IDWriteFactory::CreateTextLayout(&self.engine.factory, text, &self.format, width, f32::MAX)
    }
  }

  fn build(&self, width: Option<f32>) -> Result<Laid> {
    let box_width = width.unwrap_or(f32::MAX);
    let layout = self.fresh(&self.utf16, box_width)?;
    let lines = line_metrics(&layout)?;
    let cap = self
      .style
      .max_lines
      .map(|m| m as usize)
      .filter(|&m| m < lines.len());

    let Some(cap) = cap
    else {
      return Laid::new(vec![Piece { layout, y: 0.0 }], box_width, lines, false);
    };

    let sign = self
      .sign
      .as_ref()
      .expect("a capped paragraph has a trimming sign");
    let trimming = DWRITE_TRIMMING {
      granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
      delimiter: 0,
      delimiterCount: 0,
    };

    match self.style.ellipsis {
      EllipsisStrategy::Native => {
        let height: f32 = lines[..cap].iter().map(|l| l.height).sum();

        unsafe {
          layout.SetTrimming(&trimming, sign)?;
          layout.SetMaxHeight(height)?;
        }

        let shown = line_metrics(&layout)?;
        Laid::new(vec![Piece { layout, y: 0.0 }], box_width, shown, true)
      }
      EllipsisStrategy::Split => {
        let start: u32 = lines[..cap - 1].iter().map(|l| l.length).sum();
        let head = self.fresh(&self.utf16[..start as usize], box_width)?;
        let tail = self.fresh(&self.utf16[start as usize..], box_width)?;

        unsafe {
          tail.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
          tail.SetTrimming(&trimming, sign)?;
        }

        let mut shown = if start == 0 {
          Vec::new()
        }
        else {
          line_metrics(&head)?
        };
        let y: f32 = shown.iter().map(|l| l.height).sum();
        shown.extend(line_metrics(&tail)?);
        let mut pieces = Vec::new();

        if start > 0 {
          pieces.push(Piece {
            layout: head,
            y: 0.0,
          });
        }

        pieces.push(Piece { layout: tail, y });
        Laid::new(pieces, box_width, shown, true)
      }
    }
  }

  /// The taffy leaf: min-content is the intrinsic minimum, max-content the intrinsic maximum,
  /// and a definite or known width lays out at that width and reports the width it used.
  pub fn measure(&self, input: LayoutInput) -> LayoutOutput {
    self
      .engine
      .counters
      .measure_calls
      .set(self.engine.counters.measure_calls.get() + 1);

    let laid = match (input.known_dimensions.width, input.available_space.width) {
      (Some(w), _) => self.layout(Some(w)),
      (None, AvailableSpace::MinContent) => self.layout(Some(self.min)),
      (None, AvailableSpace::MaxContent) => self.layout(None),
      (None, AvailableSpace::Definite(w)) if w >= self.max => self.layout(None),
      (None, AvailableSpace::Definite(w)) => self.layout(Some(w)),
    };

    let Ok(laid) = laid
    else {
      return LayoutOutput::HIDDEN;
    };

    let width = match (input.known_dimensions.width, input.available_space.width) {
      (Some(w), _) => w,
      (None, AvailableSpace::MinContent) => self.min,
      (None, AvailableSpace::MaxContent) => self.max,
      (None, AvailableSpace::Definite(_)) => laid.used_width,
    };
    let size = Size {
      width,
      height: input.known_dimensions.height.unwrap_or(laid.height),
    };

    LayoutOutput::from_sizes_and_baselines(
      size,
      TRect::zero(),
      Baselines {
        first: Some(laid.first_baseline),
        last: Some(laid.last_baseline),
      },
    )
  }

  /// The old design's measure for the comparison: re-wrap the one shared layout.
  pub fn rewrap_height(&self, width: f32) -> Result<f32> {
    let mut metrics = DWRITE_TEXT_METRICS::default();

    unsafe {
      self.probe.SetMaxWidth(width)?;
      self.probe.GetMetrics(&mut metrics)?;
    }

    Ok(metrics.height)
  }
}

pub fn line_metrics(layout: &IDWriteTextLayout) -> Result<Vec<DWRITE_LINE_METRICS>> {
  let mut count = 0u32;

  if let Err(e) = unsafe { layout.GetLineMetrics(None, &mut count) }
    && e.code() != E_NOT_SUFFICIENT_BUFFER
  {
    return Err(e);
  }

  let mut lines = vec![DWRITE_LINE_METRICS::default(); count as usize];
  unsafe { layout.GetLineMetrics(Some(&mut lines), &mut count)? };
  lines.truncate(count as usize);
  Ok(lines)
}

// ---------------------------------------------------------------------------------------------
// The laid paragraph: immutable once built. Every query takes `&self` and reads only.

pub struct Piece {
  pub layout: IDWriteTextLayout,
  pub y: f32,
}

pub struct Laid {
  pieces: Vec<Piece>,
  pub box_width: f32,
  pub used_width: f32,
  pub height: f32,
  pub first_baseline: f32,
  pub last_baseline: f32,
  /// Lines drawn: what fits under a cap, every line otherwise.
  pub line_count: usize,
  pub truncated: bool,
  pub line_baselines: Vec<f32>,
  runs: OnceCell<Rc<Collected>>,
}

impl Laid {
  fn new(
    pieces: Vec<Piece>,
    box_width: f32,
    lines: Vec<DWRITE_LINE_METRICS>,
    capped: bool,
  ) -> Result<Self> {
    let mut used_width = 0.0f32;

    for piece in &pieces {
      let mut metrics = DWRITE_TEXT_METRICS::default();
      unsafe { piece.layout.GetMetrics(&mut metrics)? };
      used_width = used_width.max(metrics.widthIncludingTrailingWhitespace);
    }

    let mut y = 0.0;
    let mut baselines = Vec::new();
    let max_height = if capped && pieces.len() == 1 {
      unsafe { pieces[0].layout.GetMaxHeight() }
    }
    else {
      f32::MAX
    };

    for line in &lines {
      // Under a height cap DirectWrite reports the lines it trims away as empty lines at
      // the cap, so a line counts only while it starts above the cap and fits under it.
      if y + line.height > max_height + 0.01 || (capped && y >= max_height - 0.01) {
        break;
      }

      baselines.push(y + line.baseline);
      y += line.height;
    }

    Ok(Self {
      pieces,
      box_width,
      used_width: used_width.min(box_width),
      height: y,
      first_baseline: baselines.first().copied().unwrap_or(0.0),
      last_baseline: baselines.last().copied().unwrap_or(0.0),
      line_count: baselines.len(),
      truncated: capped,
      line_baselines: baselines,
      runs: OnceCell::new(),
    })
  }

  pub fn pieces(&self) -> &[Piece] {
    &self.pieces
  }

  /// The UTF-16 position a point lands on, and whether it lands inside the text.
  pub fn hit_test(&self, x: f32, y: f32) -> Result<(u32, bool)> {
    let piece = &self.pieces[0];
    let mut trailing = BOOL(0);
    let mut inside = BOOL(0);
    let mut m = DWRITE_HIT_TEST_METRICS::default();
    unsafe {
      piece
        .layout
        .HitTestPoint(x, y - piece.y, &mut trailing, &mut inside, &mut m)?
    };
    let pos = if trailing.as_bool() {
      m.textPosition + m.length
    }
    else {
      m.textPosition
    };
    Ok((pos, inside.as_bool()))
  }

  /// The caret at a UTF-16 position: (x, y, height).
  pub fn caret(&self, pos: u32) -> Result<(f32, f32, f32)> {
    let piece = &self.pieces[0];
    let (mut x, mut y) = (0.0, 0.0);
    let mut m = DWRITE_HIT_TEST_METRICS::default();
    unsafe {
      piece
        .layout
        .HitTestTextPosition(pos, false, &mut x, &mut y, &mut m)?
    };
    Ok((x, y + piece.y, m.height))
  }

  /// One rectangle (x, y, w, h) per line segment and bidi run of a UTF-16 range.
  pub fn range_rects(&self, start: u32, len: u32) -> Result<Vec<[f32; 4]>> {
    let piece = &self.pieces[0];
    let mut count = 0u32;

    if let Err(e) = unsafe {
      piece
        .layout
        .HitTestTextRange(start, len, 0.0, piece.y, None, &mut count)
    } && e.code() != E_NOT_SUFFICIENT_BUFFER
    {
      return Err(e);
    }

    let mut ms = vec![DWRITE_HIT_TEST_METRICS::default(); count as usize];
    unsafe {
      piece
        .layout
        .HitTestTextRange(start, len, 0.0, piece.y, Some(&mut ms), &mut count)?
    };
    Ok(
      ms.iter()
        .map(|m| [m.left, m.top, m.width, m.height])
        .collect(),
    )
  }

  /// The glyph runs at the origin, collected once: a text node's origin is snapped to whole
  /// pixels, so they translate unchanged.
  pub fn runs(&self) -> Result<Rc<Collected>> {
    if let Some(runs) = self.runs.get() {
      return Ok(runs.clone());
    }

    let runs = Rc::new(self.glyph_runs(0.0, 0.0)?);
    let _ = self.runs.set(runs.clone());
    Ok(runs)
  }

  /// The glyph runs DirectWrite draws for this paragraph with its origin at (x, y).
  pub fn glyph_runs(&self, x: f32, y: f32) -> Result<Collected> {
    let out = Rc::new(RefCell::new(Collected::default()));
    let renderer: IDWriteTextRenderer = Collector {
      out: out.clone(),
      inline: Cell::new(false),
    }
    .into();

    for piece in &self.pieces {
      unsafe { piece.layout.Draw(None, &renderer, x, y + piece.y)? };
    }

    drop(renderer);
    Ok(std::mem::take(&mut *out.borrow_mut()))
  }
}

// ---------------------------------------------------------------------------------------------
// The collecting renderer: DirectWrite's layout calls it per glyph run and per decoration.

pub struct GlyphRunRec {
  pub face: IDWriteFontFace,
  pub em: f32,
  pub glyphs: Vec<u16>,
  pub advances: Vec<f32>,
  pub offsets: Vec<DWRITE_GLYPH_OFFSET>,
  pub origin: (f32, f32),
  pub bidi: u32,
  pub sideways: bool,
  /// Drawn by an inline object (the ellipsis trimming sign).
  pub inline: bool,
}

impl GlyphRunRec {
  /// Each glyph's pen position (x, y) in the run's coordinate space.
  pub fn positions(&self) -> Vec<(f32, f32)> {
    glyph_positions(self.origin, self.bidi, &self.advances, &self.offsets)
  }

  /// A borrowed `DWRITE_GLYPH_RUN` over this record's arrays.
  pub fn as_run(&self) -> ManuallyDrop<DWRITE_GLYPH_RUN> {
    ManuallyDrop::new(DWRITE_GLYPH_RUN {
      // Borrowed without an AddRef: the record keeps the face alive and the run is never
      // dropped.
      fontFace: ManuallyDrop::new(Some(unsafe { std::mem::transmute_copy(&self.face) })),
      fontEmSize: self.em,
      glyphCount: self.glyphs.len() as u32,
      glyphIndices: self.glyphs.as_ptr(),
      glyphAdvances: self.advances.as_ptr(),
      glyphOffsets: self.offsets.as_ptr(),
      isSideways: self.sideways.into(),
      bidiLevel: self.bidi,
    })
  }

  /// The right edge of the run's ink advance, for a left-to-right run.
  pub fn right(&self) -> f32 {
    self.origin.0 + self.advances.iter().sum::<f32>()
  }
}

pub fn glyph_positions(
  origin: (f32, f32),
  bidi: u32,
  advances: &[f32],
  offsets: &[DWRITE_GLYPH_OFFSET],
) -> Vec<(f32, f32)> {
  let rtl = bidi % 2 == 1;
  let mut pen = origin.0;
  let mut out = Vec::with_capacity(advances.len());

  for (i, advance) in advances.iter().enumerate() {
    let offset = offsets.get(i).copied().unwrap_or_default();

    if rtl {
      pen -= advance;
      out.push((pen - offset.advanceOffset, origin.1 - offset.ascenderOffset));
    }
    else {
      out.push((pen + offset.advanceOffset, origin.1 - offset.ascenderOffset));
      pen += advance;
    }
  }

  out
}

#[derive(Default)]
pub struct Collected {
  pub runs: Vec<GlyphRunRec>,
  /// Underlines and strikethroughs as (x, y, width, thickness).
  pub lines: Vec<[f32; 4]>,
}

#[implement(IDWriteTextRenderer)]
struct Collector {
  out: Rc<RefCell<Collected>>,
  inline: Cell<bool>,
}

impl IDWritePixelSnapping_Impl for Collector_Impl {
  fn IsPixelSnappingDisabled(&self, _ctx: *const core::ffi::c_void) -> Result<BOOL> {
    Ok(false.into())
  }

  fn GetCurrentTransform(
    &self,
    _ctx: *const core::ffi::c_void,
    transform: *mut DWRITE_MATRIX,
  ) -> Result<()> {
    unsafe {
      *transform = DWRITE_MATRIX {
        m11: 1.0,
        m12: 0.0,
        m21: 0.0,
        m22: 1.0,
        dx: 0.0,
        dy: 0.0,
      };
    }

    Ok(())
  }

  fn GetPixelsPerDip(&self, _ctx: *const core::ffi::c_void) -> Result<f32> {
    Ok(1.0)
  }
}

impl IDWriteTextRenderer_Impl for Collector_Impl {
  fn DrawGlyphRun(
    &self,
    _ctx: *const core::ffi::c_void,
    x: f32,
    y: f32,
    _mode: DWRITE_MEASURING_MODE,
    run: *const DWRITE_GLYPH_RUN,
    _desc: *const DWRITE_GLYPH_RUN_DESCRIPTION,
    _effect: Ref<IUnknown>,
  ) -> Result<()> {
    let run = unsafe { &*run };
    let n = run.glyphCount as usize;
    let Some(face) = run.fontFace.as_ref()
    else {
      return Ok(());
    };
    let slice = |p: *const f32| {
      if p.is_null() {
        vec![0.0; n]
      }
      else {
        unsafe { std::slice::from_raw_parts(p, n) }.to_vec()
      }
    };

    self.out.borrow_mut().runs.push(GlyphRunRec {
      face: face.clone(),
      em: run.fontEmSize,
      glyphs: unsafe { std::slice::from_raw_parts(run.glyphIndices, n) }.to_vec(),
      advances: slice(run.glyphAdvances),
      offsets: if run.glyphOffsets.is_null() {
        vec![DWRITE_GLYPH_OFFSET::default(); n]
      }
      else {
        unsafe { std::slice::from_raw_parts(run.glyphOffsets, n) }.to_vec()
      },
      origin: (x, y),
      bidi: run.bidiLevel,
      sideways: run.isSideways.as_bool(),
      inline: self.inline.get(),
    });
    Ok(())
  }

  fn DrawUnderline(
    &self,
    _ctx: *const core::ffi::c_void,
    x: f32,
    y: f32,
    u: *const DWRITE_UNDERLINE,
    _effect: Ref<IUnknown>,
  ) -> Result<()> {
    let u = unsafe { &*u };
    self
      .out
      .borrow_mut()
      .lines
      .push([x, y + u.offset, u.width, u.thickness]);
    Ok(())
  }

  fn DrawStrikethrough(
    &self,
    _ctx: *const core::ffi::c_void,
    x: f32,
    y: f32,
    s: *const DWRITE_STRIKETHROUGH,
    _effect: Ref<IUnknown>,
  ) -> Result<()> {
    let s = unsafe { &*s };
    self
      .out
      .borrow_mut()
      .lines
      .push([x, y + s.offset, s.width, s.thickness]);
    Ok(())
  }

  fn DrawInlineObject(
    &self,
    ctx: *const core::ffi::c_void,
    x: f32,
    y: f32,
    object: Ref<IDWriteInlineObject>,
    sideways: BOOL,
    rtl: BOOL,
    effect: Ref<IUnknown>,
  ) -> Result<()> {
    let me: IDWriteTextRenderer = self.to_interface();

    if let Some(o) = object.as_ref() {
      self.inline.set(true);
      let result = unsafe {
        o.Draw(
          Some(ctx),
          &me,
          x,
          y,
          sideways.as_bool(),
          rtl.as_bool(),
          effect.as_ref(),
        )
      };
      self.inline.set(false);
      result?;
    }

    Ok(())
  }
}
