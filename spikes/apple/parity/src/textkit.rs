//! Option A: a variant laid out by standalone TextKit 2 (an `NSTextContentStorage`, an
//! `NSTextLayoutManager` and an `NSTextContainer`, line fragment padding 0), read into the
//! harness's record, and drawn by its fragments into a Core Graphics bitmap.
//!
//! The same source serves AppKit and UIKit: TextKit 2's classes have the same names and
//! methods in both ([`crate::platform`]).
#![allow(unused_unsafe)]

use std::{
  cell::{Cell, RefCell},
  ptr::NonNull,
  time::Instant,
};

use block2::RcBlock;
use objc2::{
  AnyThread,
  rc::Retained,
  runtime::{AnyObject, Bool, ProtocolObject},
};
use objc2_core_foundation::{
  CFAttributedString,
  CFRange,
  CFRetained,
  CFString,
  CFStringTokenizer,
  CGFloat,
  CGPoint,
  CGRect,
  CGSize,
  kCFStringTokenizerUnitLineBreak,
};
use objc2_core_text::kCTLanguageAttributeName;
use objc2_foundation::{NSAttributedString, NSMutableAttributedString, NSRange, NSString};
use text_parity::{
  corpus::{Alignment, Direction, Hyphenation, Variant, WordBreak},
  record::{Intrinsic, Line, Range, Record, VariantTimings},
  text::{Offsets, is_invisible},
};

use crate::{
  cg::Canvas,
  coretext,
  platform::{
    Font,
    NSFontAttributeName,
    NSForegroundColorAttributeName,
    NSLineBreakMode,
    NSLineBreakStrategy,
    NSMutableParagraphStyle,
    NSParagraphStyleAttributeName,
    NSTextAlignment,
    NSTextContainer,
    NSTextContentStorage,
    NSTextElementProvider as _,
    NSTextLayoutFragment,
    NSTextLayoutFragmentEnumerationOptions,
    NSTextLayoutManager,
    NSTextLayoutManagerSegmentOptions,
    NSTextLayoutManagerSegmentType,
    NSTextLocation,
    NSTextRange,
    NSWritingDirection,
    as_ct,
    gray,
    system_font,
  },
};

/// A container height (and an unconstrained width) no paragraph reaches.
pub const UNBOUNDED: CGFloat = 1.0e7;

/// Whether a paragraph of `text` runs right to left under `direction`: its first strong
/// character's direction where `auto`.
#[must_use]
pub fn is_rtl(text: &str, direction: Direction) -> bool {
  match direction {
    Direction::Ltr => false,
    Direction::Rtl => true,
    Direction::Auto => {
      for character in text.chars() {
        let code = u32::from(character);

        if matches!(code, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
        {
          return true;
        }

        if character.is_alphabetic() {
          return false;
        }
      }

      false
    }
  }
}

/// How a variant is set beyond what its case says.
#[derive(Clone)]
pub struct Setup {
  /// The base font; the system font at the case's weight where `None`.
  pub font: Option<Retained<Font>>,
  /// White text (on black) rather than black.
  pub white: bool,
  /// The line-break strategy; `standard` (plus Hangul word priority for `keep-all`) where
  /// `None`.
  pub strategy: Option<NSLineBreakStrategy>,
  /// The hyphenation factor; 1 for `auto` hyphenation and 0 otherwise where `None`.
  pub hyphenation: Option<f32>,
}

impl Default for Setup {
  fn default() -> Self {
    Self {
      font: None,
      white: false,
      strategy: None,
      hyphenation: None,
    }
  }
}

/// The font a variant is set in.
#[must_use]
pub fn font_of(variant: &Variant<'_>, setup: &Setup) -> Retained<Font> {
  setup.font.clone().unwrap_or_else(|| {
    system_font(
      f64::from(variant.size),
      variant.case.weight,
      variant.case.italic,
    )
  })
}

/// The variant's text with its attributes: font, colour, language and paragraph style.
#[must_use]
pub fn attributed(variant: &Variant<'_>, setup: &Setup) -> Retained<NSMutableAttributedString> {
  let case = variant.case;
  let string = NSString::from_str(&case.text);
  let attributed =
    NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &string);
  let range = NSRange::new(0, string.length());
  let style = NSMutableParagraphStyle::new();
  let rtl = is_rtl(&case.text, case.direction);

  unsafe {
    style.setAlignment(match case.alignment {
      Alignment::Start => NSTextAlignment::Natural,
      Alignment::Center => NSTextAlignment::Center,
      Alignment::End => {
        if rtl {
          NSTextAlignment::Left
        }
        else {
          NSTextAlignment::Right
        }
      }
      Alignment::Justify => NSTextAlignment::Justified,
    });
    style.setBaseWritingDirection(match case.direction {
      Direction::Auto => NSWritingDirection::Natural,
      Direction::Ltr => NSWritingDirection::LeftToRight,
      Direction::Rtl => NSWritingDirection::RightToLeft,
    });
    style.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
    style.setLineBreakStrategy(setup.strategy.unwrap_or(
      if case.word_break == WordBreak::KeepAll {
        NSLineBreakStrategy::Standard | NSLineBreakStrategy::HangulWordPriority
      }
      else {
        NSLineBreakStrategy::Standard
      },
    ));

    let factor = setup
      .hyphenation
      .unwrap_or(if case.hyphenation == Hyphenation::Auto {
        1.
      }
      else {
        0.
      });

    style.setHyphenationFactor(factor);
  }

  let font = font_of(variant, setup);
  let color = gray(setup.white);
  let language = NSString::from_str(&case.locale);

  unsafe {
    let font: &AnyObject = &font;
    attributed.addAttribute_value_range(NSFontAttributeName, font, range);
    let color: &AnyObject = &color;
    attributed.addAttribute_value_range(NSForegroundColorAttributeName, color, range);
    let style: &AnyObject = &style;
    attributed.addAttribute_value_range(NSParagraphStyleAttributeName, style, range);
    let key: &NSString = &*std::ptr::from_ref(kCTLanguageAttributeName).cast::<NSString>();
    let language: &AnyObject = &language;
    attributed.addAttribute_value_range(key, language, range);
  }

  attributed
}

/// The notes a variant's TextKit record carries: what TextKit has no setting for.
#[must_use]
pub fn notes(variant: &Variant<'_>) -> Vec<String> {
  let case = variant.case;
  let mut notes = Vec::new();

  match case.line_break {
    text_parity::corpus::LineBreak::Normal => {}
    other => notes.push(format!(
      "line_break {other:?} has no TextKit setting: broken as normal"
    )),
  }

  if case.word_break == WordBreak::Phrase {
    notes.push("word_break phrase has no TextKit setting: broken as standard".to_owned());
  }

  if case.word_break == WordBreak::BreakAll {
    notes.push("word_break break-all has no TextKit setting".to_owned());
  }

  notes
}

/// One line fragment as TextKit laid it out.
pub struct TkLine {
  /// Its first UTF-16 offset in the text.
  pub start: usize,
  /// Its UTF-16 length.
  pub length: usize,
  /// The fragment's frame origin.
  pub frame: CGPoint,
  /// The line's typographic bounds in the fragment.
  pub bounds: CGRect,
  /// The glyph origin in the line.
  pub glyph_origin: CGPoint,
  /// The line's text and its range in it.
  pub attributed: Retained<NSAttributedString>,
  /// The range in `attributed`.
  pub local: NSRange,
}

/// A variant laid out at one width.
pub struct TkLaid {
  /// The content storage.
  pub storage: Retained<NSTextContentStorage>,
  /// The layout manager.
  pub manager: Retained<NSTextLayoutManager>,
  /// The container width.
  pub width: f64,
  /// The lines, top to bottom.
  pub lines: Vec<TkLine>,
  /// The used rectangle.
  pub usage: CGRect,
}

impl Drop for TkLaid {
  fn drop(&mut self) {
    unsafe { self.storage.removeTextLayoutManager(&self.manager) };
  }
}

/// Lays `attributed` out at `width` (points), at most `max_lines` lines (0: no limit), the
/// last cut with an ellipsis where `truncate`.
#[must_use]
pub fn lay_out(
  attributed: &NSAttributedString,
  width: f64,
  max_lines: usize,
  truncate: bool,
) -> TkLaid {
  let storage = unsafe { NSTextContentStorage::new() };

  unsafe { storage.setAttributedString(Some(attributed)) };

  let manager = unsafe { NSTextLayoutManager::new() };
  let container = unsafe {
    NSTextContainer::initWithSize(NSTextContainer::alloc(), CGSize::new(width, UNBOUNDED))
  };

  unsafe {
    container.setLineFragmentPadding(0.);

    if max_lines > 0 {
      container.setMaximumNumberOfLines(max_lines);
      container.setLineBreakMode(if truncate {
        NSLineBreakMode::ByTruncatingTail
      }
      else {
        NSLineBreakMode::ByWordWrapping
      });
    }

    manager.setTextContainer(Some(&container));
    storage.addTextLayoutManager(&manager);
  }

  let document = unsafe { storage.documentRange() };

  unsafe { manager.ensureLayoutForRange(&document) };

  let usage = unsafe { manager.usageBoundsForTextContainer() };
  let start = unsafe { document.location() };
  let lines = RefCell::new(Vec::new());
  let block = RcBlock::new(|fragment: NonNull<NSTextLayoutFragment>| -> Bool {
    let fragment = unsafe { fragment.as_ref() };
    let frame = unsafe { fragment.layoutFragmentFrame() };
    let element = unsafe { fragment.rangeInElement() };
    let element_start = unsafe { element.location() };
    let offset =
      unsafe { storage.offsetFromLocation_toLocation(&start, &element_start) }.max(0) as usize;
    let fragments = unsafe { fragment.textLineFragments() };

    for line in fragments.iter() {
      let range = unsafe { line.characterRange() };

      lines.borrow_mut().push(TkLine {
        start: offset + range.location,
        length: range.length,
        frame: frame.origin,
        bounds: unsafe { line.typographicBounds() },
        glyph_origin: unsafe { line.glyphOrigin() },
        attributed: unsafe { line.attributedString() },
        local: range,
      });
    }

    Bool::YES
  });

  unsafe {
    manager.enumerateTextLayoutFragmentsFromLocation_options_usingBlock(
      Some(&start),
      NSTextLayoutFragmentEnumerationOptions::EnsuresLayout,
      &block,
    )
  };
  drop(block);

  TkLaid {
    storage,
    manager,
    width,
    lines: lines.into_inner(),
    usage,
  }
}

impl TkLaid {
  fn location(&self, offset: usize) -> Option<Retained<ProtocolObject<dyn NSTextLocation>>> {
    let document = unsafe { self.storage.documentRange() };
    let start = unsafe { document.location() };

    unsafe {
      self
        .storage
        .locationFromLocation_withOffset(&start, offset as isize)
    }
  }

  /// The rectangles TextKit gives a UTF-16 range (in container coordinates).
  #[must_use]
  pub fn range_rects(&self, from: usize, to: usize) -> Vec<CGRect> {
    let (Some(from), Some(to)) = (self.location(from), self.location(to))
    else {
      return Vec::new();
    };
    let Some(range) = (unsafe {
      NSTextRange::initWithLocation_endLocation(NSTextRange::alloc(), &from, Some(&to))
    })
    else {
      return Vec::new();
    };
    let rects = RefCell::new(Vec::new());
    let block = RcBlock::new(
      |_: *mut NSTextRange, frame: CGRect, _: CGFloat, _: NonNull<NSTextContainer>| -> Bool {
        rects.borrow_mut().push(frame);
        Bool::YES
      },
    );

    unsafe {
      self
        .manager
        .enumerateTextSegmentsInRange_type_options_usingBlock(
          &range,
          NSTextLayoutManagerSegmentType::Standard,
          NSTextLayoutManagerSegmentOptions::RangeNotRequired,
          &block,
        )
    };
    drop(block);

    rects.into_inner()
  }

  /// The width a UTF-16 range covers: per line the union of its rectangles, summed.
  #[must_use]
  pub fn range_width(&self, from: usize, to: usize) -> f64 {
    let mut lines: Vec<(f64, f64, f64)> = Vec::new();

    for rect in self.range_rects(from, to) {
      let key = (rect.origin.y * 8.).round() / 8.;
      let (x0, x1) = (rect.origin.x, rect.origin.x + rect.size.width);

      match lines.iter_mut().find(|line| line.0 == key) {
        Some(line) => {
          line.1 = line.1.min(x0);
          line.2 = line.2.max(x1);
        }
        None => lines.push((key, x0, x1)),
      }
    }

    lines.iter().map(|line| line.2 - line.1).sum()
  }

  /// The horizontal extent of a UTF-16 range on its line(s).
  #[must_use]
  pub fn extent(&self, from: usize, to: usize) -> Option<(f64, f64)> {
    self
      .range_rects(from, to)
      .iter()
      .filter(|rect| rect.size.width > 0.)
      .fold(None, |extent: Option<(f64, f64)>, rect| {
        let (x0, x1) = (rect.origin.x, rect.origin.x + rect.size.width);

        Some(extent.map_or((x0, x1), |(a, b)| (a.min(x0), b.max(x1))))
      })
  }

  /// Lays the same storage out again at `width` (the container resized, its layout
  /// invalidated), answering the lines laid out.
  pub fn relayout(&mut self, width: f64) -> usize {
    if let Some(container) = unsafe { self.manager.textContainer() } {
      unsafe { container.setSize(CGSize::new(width, UNBOUNDED)) };
    }

    let document = unsafe { self.storage.documentRange() };

    unsafe {
      self.manager.invalidateLayoutForRange(&document);
      self.manager.ensureLayoutForRange(&document);
    }

    self.width = width;

    let start = unsafe { document.location() };
    let count = Cell::new(0usize);
    let block = RcBlock::new(|fragment: NonNull<NSTextLayoutFragment>| -> Bool {
      let fragment = unsafe { fragment.as_ref() };

      count.set(count.get() + unsafe { fragment.textLineFragments() }.count());

      Bool::YES
    });

    unsafe {
      self
        .manager
        .enumerateTextLayoutFragmentsFromLocation_options_usingBlock(
          Some(&start),
          NSTextLayoutFragmentEnumerationOptions::EnsuresLayout,
          &block,
        )
    };
    drop(block);

    count.get()
  }

  /// Draws every fragment into `canvas` (already flipped to the layout), the container's left
  /// edge `shift` points left of the layout's.
  pub fn draw(&self, canvas: &Canvas, shift: f64) {
    let context = &*canvas.context;

    crate::platform::with_context(context, || self.draw_in(context, -shift, 0.));
  }

  /// Draws every fragment into `context` (current, flipped), the container's origin at
  /// `(x, y)`.
  pub fn draw_in(&self, context: &objc2_core_graphics::CGContext, x: f64, y: f64) {
    let document = unsafe { self.storage.documentRange() };
    let start = unsafe { document.location() };
    let block = RcBlock::new(|fragment: NonNull<NSTextLayoutFragment>| -> Bool {
      let fragment = unsafe { fragment.as_ref() };
      let frame = unsafe { fragment.layoutFragmentFrame() };

      unsafe {
        fragment.drawAtPoint_inContext(
          CGPoint::new(frame.origin.x + x, frame.origin.y + y),
          context,
        )
      };

      Bool::YES
    });

    unsafe {
      self
        .manager
        .enumerateTextLayoutFragmentsFromLocation_options_usingBlock(
          Some(&start),
          NSTextLayoutFragmentEnumerationOptions::EnsuresLayout,
          &block,
        )
    };
  }
}

/// The UTF-16 offsets where a line may break, from the system's line-break tokenizer.
#[must_use]
pub fn break_opportunities(text: &str, locale: &str) -> Vec<usize> {
  let string = CFString::from_str(text);
  let length = string.length();
  let locale_id = CFString::from_str(locale);
  let locale = unsafe { objc2_core_foundation::CFLocale::new(None, Some(&locale_id)) };
  let tokenizer: Option<CFRetained<CFStringTokenizer>> = unsafe {
    CFStringTokenizer::new(
      None,
      Some(&string),
      CFRange {
        location: 0,
        length,
      },
      kCFStringTokenizerUnitLineBreak,
      locale.as_deref(),
    )
  };
  let mut breaks = vec![0usize, length as usize];

  if let Some(tokenizer) = tokenizer {
    loop {
      let kind = unsafe { tokenizer.advance_to_next_token() };

      if kind.0 == 0 {
        break;
      }

      let range = unsafe { tokenizer.current_token_range() };

      breaks.push(range.location as usize);
      breaks.push((range.location + range.length) as usize);
    }
  }

  breaks.sort_unstable();
  breaks.dedup();
  breaks
}

/// What one TextKit layout of a variant gives.
pub struct TkVariant {
  /// The layout as drawn (at the variant's width, or its own max-content width).
  pub laid: TkLaid,
  /// The record.
  pub record: Record,
  /// The container's left edge relative to the layout's.
  pub shift: f64,
}

/// Lays `variant` out with TextKit 2 and reads its record; `timing` lays it out again that
/// many times for the warm median.
#[must_use]
pub fn variant(variant: &Variant<'_>, setup: &Setup, repeats: usize) -> TkVariant {
  let case = variant.case;
  let max_lines = case.max_lines.map_or(0, |lines| lines.max(1) as usize);
  let lay = |width: Option<f64>| -> TkLaid {
    let attributed = attributed(variant, setup);

    match width {
      Some(width) => lay_out(&attributed, width, max_lines, case.ellipsis),
      None => {
        let wide = lay_out(&attributed, UNBOUNDED, max_lines, case.ellipsis);
        let used = wide.usage.size.width;

        drop(wide);

        lay_out(
          &attributed,
          if used > 0. { used + 0.01 } else { UNBOUNDED },
          max_lines,
          case.ellipsis,
        )
      }
    }
  };
  let width = variant.width.map(f64::from);
  // Timed: one layout from the text, at the width or unconstrained, as Parley's are.
  let once = || {
    let attributed = attributed(variant, setup);
    let laid = lay_out(
      &attributed,
      width.unwrap_or(UNBOUNDED),
      max_lines,
      case.ellipsis,
    );

    std::hint::black_box(laid.lines.len());
  };
  let started = Instant::now();

  once();

  let cold_ms = started.elapsed().as_secs_f64() * 1e3;
  let mut warm: Vec<f64> = (0..repeats)
    .map(|_| {
      let started = Instant::now();

      once();

      started.elapsed().as_secs_f64() * 1e3
    })
    .collect();
  let laid = lay(width);

  warm.sort_by(f64::total_cmp);

  let truncated = max_lines > 0 && laid.lines.len() >= max_lines && {
    let attributed = attributed(variant, setup);
    let all = lay_out(&attributed, laid.width, 0, false);

    all.lines.len() > max_lines
  };
  let intrinsic = intrinsic(variant, setup);
  let mut record = read_record(variant, setup, &laid, truncated);

  record.intrinsic = Some(intrinsic);
  record.timings = Some(VariantTimings {
    cold_ms,
    warm_ms: warm.get(warm.len() / 2).copied().unwrap_or_default(),
  });

  TkVariant {
    laid,
    record,
    shift: 0.,
  }
}

/// Min-content (the widest run between two break opportunities, trailing blanks hanging) and
/// max-content (laid out unconstrained).
#[must_use]
pub fn intrinsic(variant: &Variant<'_>, setup: &Setup) -> Intrinsic {
  let attributed = attributed(variant, setup);
  let laid = lay_out(&attributed, UNBOUNDED, 0, false);
  let max = laid.usage.size.width;
  let units: Vec<u16> = variant.case.text.encode_utf16().collect();
  let breaks = break_opportunities(&variant.case.text, &variant.case.locale);
  let mut min = 0f64;

  for pair in breaks.windows(2) {
    let (from, mut to) = (pair[0], pair[1]);

    while to > from && char::from_u32(u32::from(units[to - 1])).is_some_and(char::is_whitespace) {
      to -= 1;
    }

    if to > from {
      min = min.max(laid.range_width(from, to));
    }
  }

  Intrinsic {
    min: min.min(max) as f32,
    max: max as f32,
  }
}

/// The record of `laid`.
#[must_use]
pub fn read_record(variant: &Variant<'_>, setup: &Setup, laid: &TkLaid, truncated: bool) -> Record {
  read_record_with(variant, setup, laid, truncated)
}

/// The x where TextKit draws line `line`'s glyphs from: its fragment's frame, its typographic
/// bounds and its glyph origin.
#[must_use]
pub fn origin_x(line: &TkLine) -> f64 {
  line.frame.x + line.bounds.origin.x + line.glyph_origin.x
}

/// A Core Text line of what line `index` of `laid` draws: its text, justified to the container
/// where TextKit justifies it, and cut with an ellipsis where it is the truncated last line.
#[must_use]
pub fn drawn_line(
  laid: &TkLaid,
  index: usize,
  case: &text_parity::corpus::Case,
  truncated: bool,
) -> CFRetained<objc2_core_text::CTLine> {
  let line = &laid.lines[index];
  let sub = unsafe { line.attributed.attributedSubstringFromRange(line.local) };
  let cf: &CFAttributedString = unsafe { &*std::ptr::from_ref::<NSAttributedString>(&sub).cast() };
  let plain = coretext::line_of(cf);
  let last = index + 1 == laid.lines.len();
  let ends_paragraph = sub.string().to_string().ends_with('\n');

  if truncated && case.ellipsis && last && sub.length() > 0 {
    let attributes = unsafe { sub.attributesAtIndex_effectiveRange(0, std::ptr::null_mut()) };
    let token = unsafe {
      NSAttributedString::initWithString_attributes(
        NSAttributedString::alloc(),
        &NSString::from_str("\u{2026}"),
        Some(&attributes),
      )
    };
    let token_cf: &CFAttributedString =
      unsafe { &*std::ptr::from_ref::<NSAttributedString>(&token).cast() };
    let token_line = coretext::line_of(token_cf);

    if let Some(cut) = unsafe {
      plain.truncated_line(
        laid.width,
        objc2_core_text::CTLineTruncationType::End,
        Some(&token_line),
      )
    } {
      return cut;
    }
  }

  if case.alignment == Alignment::Justify && !last && !ends_paragraph {
    if let Some(justified) = unsafe { plain.justified_line(1., laid.width) } {
      return justified;
    }
  }

  plain
}

fn read_record_with(
  variant: &Variant<'_>,
  setup: &Setup,
  laid: &TkLaid,
  truncated: bool,
) -> Record {
  let case = variant.case;
  let text = &case.text;
  let offsets = Offsets::new(text);
  let units: Vec<u16> = text.encode_utf16().collect();
  let rtl = is_rtl(text, case.direction);
  let hyphenating = setup
    .hyphenation
    .unwrap_or(if case.hyphenation == Hyphenation::Auto {
      1.
    }
    else {
      0.
    })
    > 0.;
  let mut lines = Vec::new();
  let mut runs = Vec::new();
  let mut clusters = Vec::new();
  let mut notes = notes(variant);
  let count = laid.lines.len();

  for (index, line) in laid.lines.iter().enumerate() {
    let sub = unsafe { line.attributed.attributedSubstringFromRange(line.local) };
    let cf: &CFAttributedString =
      unsafe { &*std::ptr::from_ref::<NSAttributedString>(&sub).cast() };
    let ct = coretext::line_of(cf);
    let (_, ascent, descent, _) = coretext::line_metrics(&ct);
    let trailing = unsafe { ct.trailing_whitespace_width() };
    let end = (line.start + line.length).min(units.len());
    let mut trimmed = end;

    while trimmed > line.start
      && char::decode_utf16([units[trimmed - 1]])
        .next()
        .and_then(Result::ok)
        .is_some_and(is_invisible)
    {
      trimmed -= 1;
    }

    let top = line.frame.y + line.bounds.origin.y;
    let height = line.bounds.size.height;
    let baseline = top + line.glyph_origin.y;
    let width = (line.bounds.size.width - trailing).max(0.);
    let extent = laid.extent(line.start, trimmed);
    let x = match (extent, rtl) {
      (Some((_, right)), true) => right - width,
      (Some((left, _)), false) => left,
      (None, true) => laid.width - width,
      (None, false) => 0.,
    };
    let ends_soft = trimmed > line.start && units[trimmed - 1] == 0xAD;
    let next_letter = laid
      .lines
      .get(index + 1)
      .and_then(|next| units.get(next.start))
      .and_then(|unit| char::from_u32(u32::from(*unit)))
      .is_some_and(char::is_alphabetic);
    let last_letter = trimmed > line.start
      && trimmed == end
      && char::from_u32(u32::from(units[trimmed - 1])).is_some_and(char::is_alphabetic);
    let ellipsis = truncated && case.ellipsis && index + 1 == count;

    lines.push(Line {
      range: Range {
        utf8: Some(offsets.range_utf8([line.start, end])),
        utf16: Some([line.start, end]),
      },
      x: x as f32,
      width: width as f32,
      top: top as f32,
      height: height as f32,
      baseline: baseline as f32,
      ascent: ascent as f32,
      descent: descent as f32,
      leading: (height - ascent - descent) as f32,
      hyphen: !ellipsis && (ends_soft || (hyphenating && last_letter && next_letter)),
      ellipsis,
    });

    let parts = coretext::runs(&drawn_line(laid, index, case, truncated));
    let x0 = origin_x(line);
    let (line_runs, line_clusters) =
      coretext::record_parts(&parts, line.start, index, x0, &offsets);

    runs.extend(line_runs);
    clusters.extend(line_clusters);
  }

  if lines.is_empty() {
    let font = font_of(variant, setup);
    let ct = as_ct(&font);
    let (ascent, descent, leading) = unsafe { (ct.ascent(), ct.descent(), ct.leading()) };

    notes.push("TextKit laid out no line: one synthesized from the font's metrics".to_owned());
    lines.push(Line {
      range: Range {
        utf8: Some([0, 0]),
        utf16: Some([0, 0]),
      },
      x: 0.,
      width: 0.,
      top: 0.,
      height: (ascent + descent + leading) as f32,
      baseline: ascent as f32,
      ascent: ascent as f32,
      descent: descent as f32,
      leading: leading as f32,
      hyphen: false,
      ellipsis: false,
    });
  }

  let height = lines
    .iter()
    .map(|line| line.top + line.height)
    .fold(0f32, f32::max);
  let width = lines.iter().map(|line| line.width).fold(0f32, f32::max);

  Record {
    variant: variant.id(),
    lines,
    width,
    height,
    truncated,
    runs,
    clusters,
    intrinsic: None,
    timings: None,
    images: std::collections::BTreeMap::new(),
    glyph_runs: Vec::new(),
    notes,
  }
}

/// TextKit's usage rectangle height, and its first line's height: what UIKit's own sizing
/// reports beside the line boxes.
#[must_use]
pub fn usage(laid: &TkLaid) -> (f64, f64) {
  (laid.usage.size.width, laid.usage.size.height)
}

/// A counter for the label timings: how many layouts were made.
pub static LAYOUTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Draws `laid` into a new canvas for a box of `width` by `height`.
#[must_use]
pub fn image(
  laid: &TkLaid,
  width: f32,
  height: f32,
  scale: f32,
  ground: crate::cg::Ground,
  smoothing: crate::cg::Smoothing,
) -> text_parity::image::Rgba {
  let canvas = Canvas::for_box(width, height, scale, ground, smoothing);

  canvas.flip_to_layout();
  laid.draw(&canvas, 0.);

  canvas.to_rgba()
}

thread_local! {
  /// Fragments drawn, for the label timing.
  pub static DRAWN: Cell<usize> = const { Cell::new(0) };
}
