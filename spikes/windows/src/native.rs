//! A: the corpus laid out by DirectWrite's `IDWriteTextLayout` and read into the harness's
//! records, from the layout's line metrics and the glyph runs it draws (their descriptions give
//! each run's text and each code unit's glyph, so clusters, fonts and line extents follow).
//!
//! The policy is the decided realization's: Segoe UI for `system` (fontique's `system-ui` on
//! Windows), the case's locale on the format, `EMERGENCY_BREAK` wrapping (a word too wide breaks
//! where the line fills), the paragraph's direction from its first strong character, and a cap of
//! `max_lines` by `SetMaxHeight` with the ellipsis trimming sign (no sign for a plain cut).

use std::{cell::RefCell, collections::HashMap, time::Instant};

use icu_properties::{CodePointMapData, props::BidiClass};
use text_parity::{
  corpus::{Alignment, Case, Direction, Hyphenation, LineBreak, Variant, WordBreak},
  record::{Cluster, Intrinsic, Line, Range, Record, Run, VariantTimings},
  text::Offsets,
};
use windows::Win32::Graphics::DirectWrite::*;
use windows_core::{Interface, PCWSTR, Result};

use crate::{
  engine::{Collected, collect, line_metrics, wide},
  paint::face_path,
};

/// The family `system` names on Windows: fontique's `system-ui`, and Windows 10's UI font.
pub const SYSTEM_FAMILY: &str = "Segoe UI";

/// A huge extent: an unbounded width or height.
const UNBOUNDED: f32 = 1.0e6;

/// The family a case's `family` names.
pub fn family_of(case: &Case) -> String {
  match case.family.as_str() {
    "system" => SYSTEM_FAMILY.to_owned(),
    "serif" => "Times New Roman".to_owned(),
    "monospace" => "Consolas".to_owned(),
    named => named.to_owned(),
  }
}

/// Whether the paragraph runs right to left: its direction, or its first strong character's.
pub fn is_rtl(case: &Case) -> bool {
  match case.direction {
    Direction::Ltr => false,
    Direction::Rtl => true,
    Direction::Auto => {
      let classes = CodePointMapData::<BidiClass>::new();

      for c in case.text.chars() {
        match classes.get(c) {
          BidiClass::LeftToRight => return false,
          BidiClass::RightToLeft | BidiClass::ArabicLetter => return true,
          _ => {}
        }
      }

      false
    }
  }
}

/// A face's ascent, descent and line gap at `em`.
pub fn face_metrics(face: &IDWriteFontFace, em: f32) -> (f32, f32, f32) {
  let mut m = DWRITE_FONT_METRICS::default();
  unsafe { face.GetMetrics(&mut m) };
  let per = em / f32::from(m.designUnitsPerEm.max(1));

  (
    f32::from(m.ascent) * per,
    f32::from(m.descent) * per,
    f32::from(m.lineGap as u16) * per,
  )
}

/// DirectWrite's layout of one variant.
pub struct NativeLaid {
  pub layout: IDWriteTextLayout,
  pub format: IDWriteTextFormat,
  // The lines shown.
  pub lines: Vec<DWRITE_LINE_METRICS>,
  pub truncated: bool,
}

/// DirectWrite over the shared factory, its formats cached per case, size and wrapping.
pub struct Native {
  pub factory: IDWriteFactory4,
  formats: RefCell<HashMap<(String, u32, bool), IDWriteTextFormat>>,
  primary: RefCell<HashMap<(String, u32), (f32, f32, f32)>>,
}

impl Native {
  pub fn new(factory: &IDWriteFactory4) -> Self {
    Self {
      factory: factory.clone(),
      formats: RefCell::new(HashMap::new()),
      primary: RefCell::new(HashMap::new()),
    }
  }

  /// The format of `case` at `size`, wrapping or not.
  pub fn format(&self, case: &Case, size: f32, wrap: bool) -> Result<IDWriteTextFormat> {
    let key = (case.id.clone(), size.to_bits(), wrap);

    if let Some(format) = self.formats.borrow().get(&key) {
      return Ok(format.clone());
    }

    let family = wide(&family_of(case));
    let locale = wide(&case.locale);
    let format = unsafe {
      IDWriteFactory::CreateTextFormat(
        &self.factory,
        PCWSTR(family.as_ptr()),
        None,
        DWRITE_FONT_WEIGHT(i32::from(case.weight)),
        if case.italic {
          DWRITE_FONT_STYLE_ITALIC
        }
        else {
          DWRITE_FONT_STYLE_NORMAL
        },
        DWRITE_FONT_STRETCH_NORMAL,
        size,
        PCWSTR(locale.as_ptr()),
      )?
    };

    unsafe {
      format.SetReadingDirection(if is_rtl(case) {
        DWRITE_READING_DIRECTION_RIGHT_TO_LEFT
      }
      else {
        DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
      })?;
      format.SetTextAlignment(match case.alignment {
        Alignment::Start => DWRITE_TEXT_ALIGNMENT_LEADING,
        Alignment::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
        Alignment::End => DWRITE_TEXT_ALIGNMENT_TRAILING,
        Alignment::Justify => DWRITE_TEXT_ALIGNMENT_JUSTIFIED,
      })?;
      format.SetWordWrapping(if wrap {
        DWRITE_WORD_WRAPPING_EMERGENCY_BREAK
      }
      else {
        DWRITE_WORD_WRAPPING_NO_WRAP
      })?;
    }

    self.formats.borrow_mut().insert(key, format.clone());
    Ok(format)
  }

  /// The ascent, descent and line gap of the format's own font at `size`: an empty line's.
  fn primary_metrics(&self, case: &Case, format: &IDWriteTextFormat, size: f32) -> (f32, f32, f32) {
    let key = (case.id.clone(), size.to_bits());

    if let Some(m) = self.primary.borrow().get(&key) {
      return *m;
    }

    let found = (|| -> Result<(f32, f32, f32)> {
      let mut collection = None;
      unsafe { IDWriteFactory::GetSystemFontCollection(&self.factory, &mut collection, false)? };
      let collection = collection.ok_or_else(windows_core::Error::empty)?;
      let name = wide(&family_of(case));
      let (mut index, mut exists) = (0u32, windows_core::BOOL(0));
      unsafe { collection.FindFamilyName(PCWSTR(name.as_ptr()), &mut index, &mut exists)? };
      let family = unsafe { collection.GetFontFamily(index)? };
      let font = unsafe {
        family.GetFirstMatchingFont(
          format.GetFontWeight(),
          format.GetFontStretch(),
          format.GetFontStyle(),
        )?
      };
      let face = unsafe { font.CreateFontFace()? };
      Ok(face_metrics(&face, size))
    })()
    .unwrap_or((size, size * 0.25, 0.0));

    self.primary.borrow_mut().insert(key, found);
    found
  }

  fn text_layout(
    &self,
    utf16: &[u16],
    format: &IDWriteTextFormat,
    width: f32,
  ) -> Result<IDWriteTextLayout> {
    unsafe { IDWriteFactory::CreateTextLayout(&self.factory, utf16, format, width, UNBOUNDED) }
  }

  /// `variant` laid out at its width, capped at its case's `max_lines`.
  pub fn lay_out(&self, variant: &Variant<'_>) -> Result<NativeLaid> {
    let case = variant.case;
    let utf16: Vec<u16> = case.text.encode_utf16().collect();
    let (format, width) = match variant.width {
      Some(width) => (self.format(case, variant.size, true)?, width),
      None => {
        let format = self.format(case, variant.size, false)?;
        let probe = self.text_layout(&utf16, &format, UNBOUNDED)?;
        let mut m = DWRITE_TEXT_METRICS::default();
        // A justified or centred probe would report the unbounded box.
        unsafe { probe.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)? };
        unsafe { probe.GetMetrics(&mut m)? };
        (format, m.widthIncludingTrailingWhitespace)
      }
    };
    let layout = self.text_layout(&utf16, &format, width)?;
    let mut lines = line_metrics(&layout)?;
    let mut truncated = false;

    if let Some(max) = case.max_lines.map(|m| m.max(1) as usize)
      && lines.len() > max
    {
      let height: f32 = lines[..max].iter().map(|l| l.height).sum();
      let trimming = DWRITE_TRIMMING {
        granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
        delimiter: 0,
        delimiterCount: 0,
      };
      let sign = if case.ellipsis {
        Some(unsafe { self.factory.CreateEllipsisTrimmingSign(&format)? })
      }
      else {
        None
      };

      unsafe {
        layout.SetMaxHeight(height + 0.01)?;
        layout.SetTrimming(&trimming, sign.as_ref())?;
      }

      lines = line_metrics(&layout)?;
      lines.truncate(max);
      truncated = true;
    }

    Ok(NativeLaid {
      layout,
      format,
      lines,
      truncated,
    })
  }

  /// `variant` laid out once cold and `repeats` times warm, each from the text again.
  pub fn time(
    &self,
    variant: &Variant<'_>,
    repeats: usize,
  ) -> Result<(NativeLaid, VariantTimings)> {
    let started = Instant::now();
    let laid = self.lay_out(variant)?;
    let cold_ms = started.elapsed().as_secs_f64() * 1e3;
    let mut warm = Vec::new();

    for _ in 0..repeats {
      let started = Instant::now();
      let _ = self.lay_out(variant)?;
      warm.push(started.elapsed().as_secs_f64() * 1e3);
    }

    warm.sort_by(f64::total_cmp);

    Ok((
      laid,
      VariantTimings {
        cold_ms,
        warm_ms: warm.get(warm.len() / 2).copied().unwrap_or_default(),
      },
    ))
  }

  /// Min- and max-content widths: `DetermineMinWidth` and the unwrapped width.
  pub fn intrinsic(&self, variant: &Variant<'_>) -> Result<Intrinsic> {
    let utf16: Vec<u16> = variant.case.text.encode_utf16().collect();
    let wrap = self.text_layout(
      &utf16,
      &self.format(variant.case, variant.size, true)?,
      UNBOUNDED,
    )?;
    let min = unsafe { wrap.DetermineMinWidth()? };
    let nowrap = self.text_layout(
      &utf16,
      &self.format(variant.case, variant.size, false)?,
      UNBOUNDED,
    )?;
    let mut m = DWRITE_TEXT_METRICS::default();
    unsafe { nowrap.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)? };
    unsafe { nowrap.GetMetrics(&mut m)? };

    Ok(Intrinsic { min, max: m.width })
  }

  /// The record of `laid` as `variant`.
  pub fn record(&self, variant: &Variant<'_>, laid: &NativeLaid) -> Result<Record> {
    let case = variant.case;
    let text = &case.text;
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let offsets = Offsets::new(text);
    let collected = collect(&laid.layout, 0.0, 0.0, 1.0, false)?;
    let range = |s: usize, e: usize| {
      let e = e.min(utf16.len());
      let s = s.min(e);

      Range {
        utf8: Some(offsets.range_utf8([s, e])),
        utf16: Some([s, e]),
      }
    };

    struct Acc {
      start: u32,
      end: u32,
      visible_end: u32,
      top: f32,
      left: f32,
      right: f32,
      inked: bool,
      ascent: f32,
      descent: f32,
      ellipsis: bool,
      last_visible: Option<(u32, f32)>,
    }

    let mut accs = Vec::new();
    let mut pos = 0u32;
    let mut top = 0.0f32;

    for line in &laid.lines {
      accs.push(Acc {
        start: pos,
        end: pos + line.length,
        visible_end: pos + line.length - line.trailingWhitespaceLength,
        top,
        left: f32::MAX,
        right: f32::MIN,
        inked: false,
        ascent: 0.0,
        descent: 0.0,
        ellipsis: false,
        last_visible: None,
      });
      pos += line.length;
      top += line.height;
    }

    let baselines: Vec<f32> = accs
      .iter()
      .zip(&laid.lines)
      .map(|(a, l)| a.top + l.baseline)
      .collect();
    let mut runs = Vec::new();
    let mut clusters = Vec::new();
    let mut notes = Vec::new();

    for run in &collected.runs {
      let rtl = run.bidi % 2 == 1;
      let total: f32 = run.advances.iter().sum();
      let (lo, hi) = if rtl {
        (run.origin.0 - total, run.origin.0)
      }
      else {
        (run.origin.0, run.origin.0 + total)
      };

      if run.inline {
        let at = baselines
          .iter()
          .rposition(|b| (b - run.origin.1).abs() < 1.0)
          .unwrap_or(accs.len().saturating_sub(1));

        if let Some(acc) = accs.get_mut(at) {
          acc.ellipsis = true;
          acc.left = acc.left.min(lo);
          acc.right = acc.right.max(hi);
          acc.inked = true;
        }

        continue;
      }

      let tp = run.text_position;
      let Some(line) = accs
        .iter()
        .position(|a| tp >= a.start && tp < a.end.max(a.start + 1))
      else {
        notes.push(format!("a glyph run at {tp} lies on no shown line"));
        continue;
      };
      let (ascent, descent, _) = face_metrics(&run.face, run.em);
      let face3 = run.face.cast::<IDWriteFontFace3>().ok();
      let simulations = unsafe { run.face.GetSimulations() };
      let n = run.cluster_map.len();

      {
        let acc = &mut accs[line];
        acc.ascent = acc.ascent.max(ascent);
        acc.descent = acc.descent.max(descent);
      }

      runs.push(Run {
        range: range(tp as usize, tp as usize + n),
        line,
        family: None,
        postscript: None,
        path: face_path(&run.face).ok(),
        index: Some(unsafe { run.face.GetIndex() }),
        size: run.em,
        weight: face3.as_ref().map(|f| unsafe { f.GetWeight() }.0 as f32),
        italic: face3
          .as_ref()
          .map(|f| unsafe { f.GetStyle() } != DWRITE_FONT_STYLE_NORMAL),
        synthetic_bold: simulations.0 & DWRITE_FONT_SIMULATIONS_BOLD.0 != 0,
        synthetic_italic: simulations.0 & DWRITE_FONT_SIMULATIONS_OBLIQUE.0 != 0,
        rtl,
      });

      if runs.last().is_some_and(|r| r.path.is_none()) {
        let family = face3
          .as_ref()
          .and_then(|f| family_name(f).ok())
          .unwrap_or_else(|| "?".to_owned());
        runs.last_mut().unwrap().family = Some(family);
      }

      let glyphs = run.glyphs.len();
      let mut j = 0;

      while j < n {
        let g0 = run.cluster_map[j] as usize;
        let mut k = j + 1;

        while k < n && run.cluster_map[k] as usize == g0 {
          k += 1;
        }

        let g1 = if k < n {
          (run.cluster_map[k] as usize).max(g0)
        }
        else {
          glyphs
        };
        let before: f32 = run.advances[..g0.min(glyphs)].iter().sum();
        let advance: f32 = run.advances[g0.min(glyphs)..g1.min(glyphs)].iter().sum();
        let left = if rtl {
          run.origin.0 - before - advance
        }
        else {
          run.origin.0 + before
        };
        let start = tp + j as u32;

        clusters.push(Cluster {
          range: range(start as usize, (tp as usize) + k),
          line,
          x: left,
          advance,
        });

        let acc = &mut accs[line];

        if start < acc.visible_end {
          if advance > 0.0 || !acc.inked {
            acc.left = acc.left.min(left);
            acc.right = acc.right.max(left + advance);
            acc.inked = true;
          }

          if acc.last_visible.is_none_or(|(at, _)| start > at) {
            acc.last_visible = Some((start, advance));
          }
        }

        j = k;
      }
    }

    let (pa, pd, _) = self.primary_metrics(case, &laid.format, variant.size);
    let mut lines = Vec::new();

    for (at, acc) in accs.iter().enumerate() {
      let lm = &laid.lines[at];
      let x = if acc.inked {
        acc.left
      }
      else {
        let (mut x, mut y) = (0.0, 0.0);
        let mut m = DWRITE_HIT_TEST_METRICS::default();
        unsafe {
          laid
            .layout
            .HitTestTextPosition(acc.start, false, &mut x, &mut y, &mut m)?
        };
        x
      };
      let (ascent, descent) = if acc.ascent > 0.0 {
        (acc.ascent, acc.descent)
      }
      else {
        (pa, pd)
      };
      let hyphen = acc
        .last_visible
        .is_some_and(|(at, advance)| utf16.get(at as usize) == Some(&0x00AD) && advance > 0.0);

      lines.push(Line {
        range: range(acc.start as usize, acc.end as usize),
        x,
        width: if acc.inked {
          (acc.right - acc.left).max(0.0)
        }
        else {
          0.0
        },
        top: acc.top,
        height: lm.height,
        baseline: acc.top + lm.baseline,
        ascent,
        descent,
        leading: lm.height - ascent - descent,
        hyphen,
        ellipsis: acc.ellipsis,
      });
    }

    if case.hyphenation == Hyphenation::Auto {
      notes.push("automatic hyphenation: DirectWrite has none".to_owned());
    }

    if case.line_break != LineBreak::Normal {
      notes.push(format!(
        "line_break {:?}: DirectWrite has no strictness setting",
        case.line_break
      ));
    }

    if case.word_break != WordBreak::Normal {
      notes.push(format!(
        "word_break {:?}: DirectWrite has no word-break setting",
        case.word_break
      ));
    }

    let height = lines.last().map_or(0.0, |l| l.top + l.height);

    Ok(Record {
      variant: variant.id(),
      width: lines.iter().map(|l| l.width).fold(0.0, f32::max),
      height,
      lines,
      truncated: laid.truncated,
      runs,
      clusters,
      intrinsic: None,
      timings: None,
      images: std::collections::BTreeMap::new(),
      glyph_runs: Vec::new(),
      notes,
    })
  }

  /// The glyph runs `laid` draws on a grid of `scale` pixels per DIP, scaled to pixels.
  pub fn drawn(&self, laid: &NativeLaid, scale: f32) -> Result<Collected> {
    Ok(collect(&laid.layout, 0.0, 0.0, scale, true)?.scaled(scale))
  }
}

/// A face's family name, in English where it has one.
pub fn family_name(face: &IDWriteFontFace3) -> Result<String> {
  let names = unsafe { face.GetFamilyNames()? };
  localized(&names)
}

/// The English string of `names`, or its first.
pub fn localized(names: &IDWriteLocalizedStrings) -> Result<String> {
  let en = wide("en-us");
  let (mut index, mut exists) = (0u32, windows_core::BOOL(0));
  unsafe { names.FindLocaleName(PCWSTR(en.as_ptr()), &mut index, &mut exists)? };

  if !exists.as_bool() {
    index = 0;
  }

  let len = unsafe { names.GetStringLength(index)? };
  let mut buf = vec![0u16; len as usize + 1];
  unsafe { names.GetString(index, &mut buf)? };
  Ok(String::from_utf16_lossy(&buf[..len as usize]))
}
