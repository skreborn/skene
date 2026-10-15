//! A TextKit 2 paragraph measured by taffy through the corrected leaf, driven from Rust.
//!
//! Every Objective-C message this crate sends goes through `oc!`, which counts it, so a pass
//! reports how many crossings each leaf cost.

use std::{
  cell::{Cell, OnceCell, RefCell},
  ptr::NonNull,
  sync::atomic::{AtomicU64, Ordering},
};

use block2::RcBlock;
use objc2::{
  AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
  rc::Retained,
  runtime::{AnyObject, Bool, ProtocolObject},
};
use objc2_app_kit::{
  NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSGraphicsContext,
  NSLineBreakMode, NSLineBreakStrategy, NSMutableParagraphStyle, NSParagraphStyleAttributeName,
  NSTextContainer, NSTextContentStorage, NSTextElementProvider, NSTextLayoutFragment,
  NSTextLayoutFragmentEnumerationOptions, NSTextLayoutManager, NSTextLayoutManagerSegmentOptions,
  NSTextLayoutManagerSegmentType, NSTextLocation, NSTextRange, NSView,
};
use objc2_core_foundation::{CFRange, CFRetained, CFString, CFStringTokenizer, CGFloat, CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGContext;
use objc2_foundation::{NSMutableAttributedString, NSRange, NSRect, NSString};

/// Objective-C messages sent since the process started.
pub static CALLS: AtomicU64 = AtomicU64::new(0);

/// Counts one Objective-C message and evaluates it.
#[macro_export]
macro_rules! oc {
  ($e:expr) => {{
    $crate::CALLS.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed);
    $e
  }};
}

/// The calls counted so far.
pub fn calls() -> u64 {
  CALLS.load(Ordering::Relaxed)
}

/// A container height no paragraph reaches.
const UNBOUNDED: CGFloat = 1.0e7;

/// An immutable paragraph: its attributed text in one content storage.
pub struct Paragraph {
  storage: Retained<NSTextContentStorage>,
  text: String,
  max_lines: usize,
  intrinsic: OnceCell<(f64, f64)>,
}

/// One layout of a paragraph at one width, never re-wrapped after it is made.
pub struct Laid {
  /// The container width it was laid out at.
  pub width: f64,
  /// The used size.
  pub size: (f64, f64),
  /// The used rectangle's left edge inside the container.
  pub min_x: f64,
  /// The first line's baseline from the top.
  pub first_baseline: f64,
  /// The last line's baseline from the top.
  pub last_baseline: f64,
  /// The lines laid out.
  pub line_count: usize,
  /// Whether the line limit cut text off.
  pub truncated: bool,
  /// Each line's first UTF-16 offset.
  pub line_starts: Vec<usize>,
  /// Each line's typographic width.
  pub line_widths: Vec<f64>,
  /// The union of the fragments' rendering surfaces, in container coordinates.
  pub surface: CGRect,
  storage: Retained<NSTextContentStorage>,
  manager: Retained<NSTextLayoutManager>,
}

impl Drop for Laid {
  fn drop(&mut self) {
    oc!(self.storage.removeTextLayoutManager(&self.manager));
  }
}

impl Paragraph {
  /// Builds a paragraph of `text` in `font`, black, wrapping by the standard strategy.
  pub fn new(text: &str, font: &NSFont, max_lines: usize) -> Self {
    let string = oc!(NSString::from_str(text));
    let attributed = oc!(NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &string));
    let range = NSRange::new(0, oc!(string.length()));
    let style = oc!(NSMutableParagraphStyle::new());
    oc!(style.setLineBreakStrategy(NSLineBreakStrategy::Standard));
    let black = oc!(NSColor::blackColor());

    unsafe {
      let font: &AnyObject = font;
      oc!(attributed.addAttribute_value_range(NSFontAttributeName, font, range));
      let black: &AnyObject = &black;
      oc!(attributed.addAttribute_value_range(NSForegroundColorAttributeName, black, range));
      let style: &AnyObject = &style;
      oc!(attributed.addAttribute_value_range(NSParagraphStyleAttributeName, style, range));
    }

    let storage = oc!(NSTextContentStorage::new());
    oc!(storage.setAttributedString(Some(&attributed)));

    Self { storage, text: text.to_owned(), max_lines, intrinsic: OnceCell::new() }
  }

  /// The text.
  pub fn text(&self) -> &str {
    &self.text
  }

  /// Lays the paragraph out at `width` with its line limit.
  pub fn layout(&self, width: f64) -> Laid {
    self.layout_limited(width, self.max_lines)
  }

  fn layout_limited(&self, width: f64, max_lines: usize) -> Laid {
    let manager = oc!(NSTextLayoutManager::new());
    let container = oc!(NSTextContainer::initWithSize(NSTextContainer::alloc(), CGSize::new(width, UNBOUNDED)));
    oc!(container.setLineFragmentPadding(0.0));

    if max_lines > 0 {
      oc!(container.setMaximumNumberOfLines(max_lines));
      oc!(container.setLineBreakMode(NSLineBreakMode::ByTruncatingTail));
    }

    oc!(manager.setTextContainer(Some(&container)));
    oc!(self.storage.addTextLayoutManager(&manager));
    let document = oc!(self.storage.documentRange());
    oc!(manager.ensureLayoutForRange(&document));
    let usage = oc!(manager.usageBoundsForTextContainer());
    let start = oc!(document.location());
    let lines = RefCell::new(Vec::<(f64, usize, f64)>::new());
    let surface = Cell::new(None::<CGRect>);

    let block = RcBlock::new(|fragment: NonNull<NSTextLayoutFragment>| -> Bool {
      let fragment = unsafe { fragment.as_ref() };
      let frame = oc!(fragment.layoutFragmentFrame());
      let bounds = oc!(fragment.renderingSurfaceBounds());
      let bounds = CGRect::new(
        CGPoint::new(bounds.origin.x + frame.origin.x, bounds.origin.y + frame.origin.y),
        bounds.size,
      );
      surface.set(Some(match surface.get() {
        None => bounds,
        Some(rect) => {
          let x0 = rect.origin.x.min(bounds.origin.x);
          let y0 = rect.origin.y.min(bounds.origin.y);
          let x1 = (rect.origin.x + rect.size.width).max(bounds.origin.x + bounds.size.width);
          let y1 = (rect.origin.y + rect.size.height).max(bounds.origin.y + bounds.size.height);
          CGRect::new(CGPoint::new(x0, y0), CGSize::new(x1 - x0, y1 - y0))
        }
      }));
      let element = oc!(fragment.rangeInElement());
      let element_start = oc!(element.location());
      let offset = oc!(self.storage.offsetFromLocation_toLocation(&start, &element_start)) as usize;
      let fragments = oc!(fragment.textLineFragments());

      for line in fragments.iter() {
        let bounds = oc!(line.typographicBounds());
        let origin = oc!(line.glyphOrigin());
        let range = oc!(line.characterRange());
        lines.borrow_mut().push((
          frame.origin.y + bounds.origin.y + origin.y,
          offset + range.location,
          bounds.size.width,
        ));
      }

      Bool::YES
    });

    oc!(manager.enumerateTextLayoutFragmentsFromLocation_options_usingBlock(
      Some(&start),
      NSTextLayoutFragmentEnumerationOptions::EnsuresLayout,
      &block,
    ));

    drop(block);
    let lines = lines.into_inner();

    let truncated = max_lines > 0 && lines.len() >= max_lines && {
      let unlimited = self.layout_limited(width, 0);
      unlimited.line_count > max_lines
    };

    Laid {
      width,
      size: (usage.size.width, usage.size.height),
      min_x: usage.origin.x,
      first_baseline: lines.first().map_or(0.0, |line| line.0),
      last_baseline: lines.last().map_or(0.0, |line| line.0),
      line_count: lines.len(),
      truncated,
      line_starts: lines.iter().map(|line| line.1).collect(),
      line_widths: lines.iter().map(|line| line.2).collect(),
      surface: surface.get().unwrap_or(CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(0.0, 0.0))),
      storage: self.storage.clone(),
      manager,
    }
  }

  /// The UTF-16 offsets where a line may break, from the system's line-break tokenizer.
  pub fn break_opportunities(&self) -> Vec<usize> {
    let string = CFString::from_str(&self.text);
    let length = string.length();

    let tokenizer: CFRetained<CFStringTokenizer> = unsafe {
      CFStringTokenizer::new(
        None,
        Some(&string),
        CFRange { location: 0, length },
        objc2_core_foundation::kCFStringTokenizerUnitLineBreak,
        None,
      )
    }
    .expect("a tokenizer");

    let mut breaks = vec![0usize, length as usize];

    loop {
      let kind = tokenizer.advance_to_next_token();

      if kind.0 == 0 {
        break;
      }

      let range = tokenizer.current_token_range();
      breaks.push(range.location as usize);
      breaks.push((range.location + range.length) as usize);
    }

    breaks.sort_unstable();
    breaks.dedup();
    breaks
  }

  /// The minimum and maximum content widths.
  ///
  /// The maximum is the width laid out without a limit; the minimum is the widest run between
  /// two break opportunities, its trailing blanks hanging, read from that same layout.
  pub fn intrinsic(&self) -> (f64, f64) {
    *self.intrinsic.get_or_init(|| {
      let laid = self.layout_limited(UNBOUNDED, 0);
      let max = laid.size.0;
      let units: Vec<u16> = self.text.encode_utf16().collect();
      let breaks = self.break_opportunities();
      let mut min = 0.0f64;

      for pair in breaks.windows(2) {
        let (from, mut to) = (pair[0], pair[1]);

        while to > from && char::from_u32(units[to - 1] as u32).is_some_and(char::is_whitespace) {
          to -= 1;
        }

        if to > from {
          min = min.max(laid.range_width(from, to));
        }
      }

      (min.min(max), max)
    })
  }
}

impl Laid {
  fn location(&self, offset: usize) -> Retained<ProtocolObject<dyn NSTextLocation>> {
    let document = oc!(self.storage.documentRange());
    let start = oc!(document.location());
    oc!(self.storage.locationFromLocation_withOffset(&start, offset as isize)).expect("a location")
  }

  fn range(&self, from: usize, to: usize) -> Retained<NSTextRange> {
    let from = self.location(from);
    let to = self.location(to);
    oc!(NSTextRange::initWithLocation_endLocation(NSTextRange::alloc(), &from, Some(&to))).expect("a range")
  }

  /// The rectangles a UTF-16 range covers.
  pub fn range_rects(&self, from: usize, to: usize) -> Vec<CGRect> {
    let range = self.range(from, to);
    let rects = RefCell::new(Vec::new());

    let block = RcBlock::new(|_: *mut NSTextRange, frame: CGRect, _: CGFloat, _: NonNull<NSTextContainer>| -> Bool {
      rects.borrow_mut().push(frame);
      Bool::YES
    });

    oc!(self.manager.enumerateTextSegmentsInRange_type_options_usingBlock(
      &range,
      NSTextLayoutManagerSegmentType::Standard,
      NSTextLayoutManagerSegmentOptions::RangeNotRequired,
      &block,
    ));

    drop(block);
    rects.into_inner()
  }

  /// The width a UTF-16 range covers: the union of its rectangles on each line, summed over lines.
  pub fn range_width(&self, from: usize, to: usize) -> f64 {
    let mut lines: Vec<(f64, f64, f64)> = Vec::new();

    for rect in self.range_rects(from, to) {
      let key = (rect.origin.y * 8.0).round() / 8.0;
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

  /// Draws every fragment into `context`, the used rectangle's origin at `origin`.
  pub fn draw(&self, context: &CGContext, origin: CGPoint) {
    let document = oc!(self.storage.documentRange());
    let start = oc!(document.location());

    let block = RcBlock::new(|fragment: NonNull<NSTextLayoutFragment>| -> Bool {
      let fragment = unsafe { fragment.as_ref() };
      let frame = oc!(fragment.layoutFragmentFrame());
      let at = CGPoint::new(frame.origin.x - self.min_x + origin.x, frame.origin.y + origin.y);
      oc!(fragment.drawAtPoint_inContext(at, context));
      Bool::YES
    });

    oc!(self.manager.enumerateTextLayoutFragmentsFromLocation_options_usingBlock(
      Some(&start),
      NSTextLayoutFragmentEnumerationOptions::EnsuresLayout,
      &block,
    ));
  }
}

/// A text node: its paragraph, the layouts it has made by width, and its measure.
pub struct TextNode {
  /// The paragraph.
  pub paragraph: Paragraph,
  cache: RefCell<Vec<std::rc::Rc<Laid>>>,
  /// Layouts made.
  pub layouts: Cell<usize>,
  /// Measure calls answered.
  pub measures: Cell<usize>,
}

/// What a measure answers: the used size and the baselines.
#[derive(Clone, Copy, Debug)]
pub struct Measured {
  /// The width.
  pub width: f64,
  /// The height.
  pub height: f64,
  /// The first baseline.
  pub first_baseline: f64,
  /// The last baseline.
  pub last_baseline: f64,
}

/// The width a measure is asked under.
#[derive(Clone, Copy, Debug)]
pub enum Available {
  /// The min-content constraint.
  MinContent,
  /// The max-content constraint.
  MaxContent,
  /// A definite available width.
  Definite(f64),
  /// A width the parent already fixed.
  Known(f64),
}

impl TextNode {
  /// A node over `paragraph`.
  pub fn new(paragraph: Paragraph) -> Self {
    Self { paragraph, cache: RefCell::new(Vec::new()), layouts: Cell::new(0), measures: Cell::new(0) }
  }

  /// The layout at `width`, from the cache when one serves it.
  ///
  /// A layout at width `w` that used `u` serves any width in `[u, w]`, drawn shifted by its used
  /// rectangle's left edge; the cache keeps three.
  pub fn laid(&self, width: f64) -> std::rc::Rc<Laid> {
    let mut cache = self.cache.borrow_mut();

    if let Some(index) = cache.iter().position(|laid| laid.width == width || (laid.size.0 <= width + 1e-6 && width <= laid.width)) {
      let laid = cache.remove(index);
      cache.insert(0, laid.clone());
      return laid;
    }

    self.layouts.set(self.layouts.get() + 1);
    let laid = std::rc::Rc::new(self.paragraph.layout(width));
    cache.insert(0, laid.clone());
    cache.truncate(3);
    laid
  }

  /// Drops every cached layout.
  pub fn clear(&self) {
    self.cache.borrow_mut().clear();
  }

  /// The corrected leaf: min-content and max-content are the intrinsic widths, a definite width
  /// lays out there and reports the width used, a known width is kept.
  pub fn measure(&self, available: Available) -> Measured {
    self.measures.set(self.measures.get() + 1);

    let (request, keep) = match available {
      Available::MinContent => (self.paragraph.intrinsic().0, true),
      Available::MaxContent => (self.paragraph.intrinsic().1, true),
      Available::Definite(width) => (width, false),
      Available::Known(width) => (width, true),
    };

    let laid = self.laid(request);

    Measured {
      width: if keep { request } else { laid.size.0.min(request) },
      height: laid.size.1,
      first_baseline: laid.first_baseline,
      last_baseline: laid.last_baseline,
    }
  }
}

/// The ivars of a node's view.
pub struct NodeViewIvars {
  laid: RefCell<Option<std::rc::Rc<Laid>>>,
  offset: Cell<(f64, f64)>,
}

define_class!(
  // SAFETY: NSView has no subclassing requirements beyond the main thread, and the class has no Drop.
  #[unsafe(super(NSView))]
  #[thread_kind = MainThreadOnly]
  #[ivars = NodeViewIvars]
  /// A flipped view drawing one laid paragraph in `drawRect:`.
  pub struct NodeView;

  impl NodeView {
    #[unsafe(method(isFlipped))]
    fn is_flipped(&self) -> bool {
      true
    }

    #[unsafe(method(drawRect:))]
    fn draw_rect(&self, _dirty: NSRect) {
      let Some(laid) = self.ivars().laid.borrow().clone() else {
        return;
      };

      let Some(context) = oc!(NSGraphicsContext::currentContext()) else {
        return;
      };

      let cg = oc!(context.CGContext());
      let (x, y) = self.ivars().offset.get();
      laid.draw(&cg, CGPoint::new(x, y));
    }
  }
);

impl NodeView {
  /// A view of `frame` drawing `laid`.
  pub fn new(mtm: MainThreadMarker, frame: NSRect, laid: std::rc::Rc<Laid>) -> Retained<Self> {
    let this = Self::alloc(mtm).set_ivars(NodeViewIvars { laid: RefCell::new(Some(laid)), offset: Cell::new((0.0, 0.0)) });
    unsafe { msg_send![super(this), initWithFrame: frame] }
  }

  /// Moves the drawing by `(x, y)` points.
  pub fn set_offset(&self, x: f64, y: f64) {
    self.ivars().offset.set((x, y));
  }
}

/// An RGB image composited over white, one byte per channel.
pub struct Image {
  /// Pixels across.
  pub width: usize,
  /// Pixels down.
  pub height: usize,
  /// Device pixels per point.
  pub scale: f64,
  /// Rows of RGB triples, top row first.
  pub pixels: Vec<u8>,
}

/// How far two images are apart.
#[derive(Clone, Copy, Debug, Default)]
pub struct Diff {
  /// The largest channel difference.
  pub max: u8,
  /// The mean channel difference over every pixel either image inks.
  pub mean: f64,
  /// Pixels differing by more than 64 in some channel.
  pub over64: usize,
  /// Pixels either image inks.
  pub inked: usize,
  /// The shift applied to the second image, in pixels.
  pub shift: (i32, i32),
}

impl std::fmt::Display for Diff {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "max={} mean={:.2} over64={} ({:.2}% of {} inked px) shift=({},{})",
      self.max,
      self.mean,
      self.over64,
      if self.inked == 0 { 0.0 } else { 100.0 * self.over64 as f64 / self.inked as f64 },
      self.inked,
      self.shift.0,
      self.shift.1
    )
  }
}

impl Image {
  /// Writes a binary PPM.
  pub fn write_ppm(&self, path: &std::path::Path) -> std::io::Result<()> {
    let mut bytes = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
    bytes.extend_from_slice(&self.pixels);
    std::fs::write(path, bytes)
  }

  fn at(&self, x: i32, y: i32) -> [u8; 3] {
    if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
      return [255; 3];
    }

    let index = (y as usize * self.width + x as usize) * 3;
    [self.pixels[index], self.pixels[index + 1], self.pixels[index + 2]]
  }

  /// The inked pixels' bounds as `(min_x, min_y, max_x, max_y)` exclusive, in pixels.
  pub fn ink(&self) -> Option<(usize, usize, usize, usize)> {
    let mut bounds: Option<(usize, usize, usize, usize)> = None;

    for y in 0..self.height {
      for x in 0..self.width {
        let pixel = self.at(x as i32, y as i32);

        if pixel.iter().any(|&channel| channel < 250) {
          let b = bounds.get_or_insert((x, y, x + 1, y + 1));
          b.0 = b.0.min(x);
          b.1 = b.1.min(y);
          b.2 = b.2.max(x + 1);
          b.3 = b.3.max(y + 1);
        }
      }
    }

    bounds
  }

  /// The difference with `other` shifted by `(dx, dy)` pixels.
  pub fn diff_shifted(&self, other: &Self, dx: i32, dy: i32) -> Diff {
    let width = self.width.max(other.width) as i32;
    let height = self.height.max(other.height) as i32;
    let mut diff = Diff { shift: (dx, dy), ..Diff::default() };
    let mut total = 0u64;

    for y in 0..height {
      for x in 0..width {
        let a = self.at(x, y);
        let b = other.at(x - dx, y - dy);

        if a.iter().all(|&c| c >= 250) && b.iter().all(|&c| c >= 250) {
          continue;
        }

        diff.inked += 1;
        let d = (0..3).map(|i| a[i].abs_diff(b[i])).max().unwrap_or(0);
        diff.max = diff.max.max(d);
        total += d as u64;

        if d > 64 {
          diff.over64 += 1;
        }
      }
    }

    diff.mean = if diff.inked == 0 { 0.0 } else { total as f64 / diff.inked as f64 };
    diff
  }

  /// The raw difference, and the least one over shifts of up to `reach` pixels.
  pub fn diff(&self, other: &Self, reach: i32) -> (Diff, Diff) {
    let raw = self.diff_shifted(other, 0, 0);
    let mut best = raw;

    for dy in -reach..=reach {
      for dx in -reach..=reach {
        let diff = self.diff_shifted(other, dx, dy);

        if diff.mean < best.mean {
          best = diff;
        }
      }
    }

    (raw, best)
  }
}
