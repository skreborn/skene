//! Parley as a text engine: shaping once per paragraph, breaking per width into a layout whose
//! geometry never changes and whose paint is the paragraph's, and the geometry and movement
//! Parley's editing primitives give.

use std::{borrow::Cow, cell::RefCell, ops::Range, rc::Rc, sync::Arc};

use parley::{
  Affinity as ParleyAffinity,
  Alignment,
  AlignmentOptions,
  BreakReason,
  Cursor,
  FontContext,
  FontData,
  FontFamily as ParleyFamilies,
  FontFamilyName,
  FontStyle as ParleySlant,
  FontWeight as ParleyWeight,
  GenericFamily,
  InlineBox as ParleyBox,
  InlineBoxKind,
  Layout,
  LayoutContext,
  LineHeight as ParleyLineHeight,
  OverflowWrap,
  PositionedLayoutItem,
  Selection,
  StyleProperty,
  TabSize,
  TextWrapMode,
  VerticalAlign,
  fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, SourceCache},
};
use skene_foundation::{
  Point,
  Rect,
  Size,
  text::{
    Affinity,
    AttributedText,
    BaseDirection,
    Caret,
    Carets,
    Direction,
    FontFamily,
    FontSlant,
    Hit,
    Intrinsic,
    Laid,
    LineHeight,
    LineInfo,
    PaintStyle,
    Paragraph,
    ParagraphStyle,
    Position,
    Round,
    RunExtent,
    Segments,
    TextAlignment,
    TextBoundary,
    TextEngine,
    TextIndex,
    TextSize,
    TextStyle,
    TextWrap,
    VisualMove,
  },
};
use skrifa::{
  FontRef,
  MetadataProvider as _,
  instance::{LocationRef, NormalizedCoord, Size as FontSize},
  raw::types::F2Dot14,
};

use crate::shaping::{Job, Shaped};
#[cfg(not(target_arch = "wasm32"))]
use crate::shaping::{PARALLEL, SHARE, Shapers};

/// A typeface an engine shapes in alone, with no system font: its family name and its data.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Typeface {
  /// The family it is known by.
  pub family: &'static str,
  /// Its OpenType data.
  pub data: &'static [u8],
}

impl Typeface {
  /// The typeface `family` of the OpenType `data`.
  #[inline(always)]
  pub const fn new(family: &'static str, data: &'static [u8]) -> Self {
    Self { family, data }
  }
}

/// The shaping and layout contexts an engine's paragraphs share.
struct Contexts {
  /// The fonts and their shaping caches.
  fonts: FontContext,
  /// Where paragraphs are built and laid out.
  layouts: LayoutContext<()>,
  /// The family a run falls back to past its own: the engine's first typeface.
  first: Option<&'static str>,
  /// The threads shaping a batch beside this one.
  #[cfg(not(target_arch = "wasm32"))]
  shapers: Threads,
  /// How the paragraphs made from now on lay out.
  settings: Settings,
  /// How many shaped words each context keeps for later paragraphs.
  shape_cache: usize,
}

/// How an engine's paragraphs lay out beyond their text and style.
#[derive(Clone, Copy, Debug)]
struct Settings {
  /// How far apart a tab's stops are.
  tab_size: TabSize,
  /// Whether line metrics are rounded to whole units.
  quantize: bool,
}

/// The threads shaping a batch beside an engine's own.
#[cfg(not(target_arch = "wasm32"))]
enum Threads {
  /// None asked for yet: the first batch starts them.
  Unstarted,
  /// Those that started.
  Started(Shapers),
  /// None would start.
  Unavailable,
}

/// How many shaped words an engine's contexts keep for later paragraphs unless told otherwise,
/// each of its shaping threads as many: Minikin's `LayoutCache` keeps 5000.
pub const SHAPE_CACHE: usize = 4096;

/// Parley as a [`TextEngine`], over typefaces of its own.
///
/// A run's family is looked up among the engine's typefaces, then falls back to the first;
/// an unmapped character draws its typeface's `.notdef`. Clones share their contexts.
#[derive(Clone)]
#[must_use]
pub struct ParleyEngine {
  contexts: Rc<RefCell<Contexts>>,
}

impl std::fmt::Debug for ParleyEngine {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut debug = formatter.debug_struct("ParleyEngine");

    if let Ok(contexts) = self.contexts.try_borrow() {
      debug
        .field("first", &contexts.first)
        .field("settings", &contexts.settings)
        .field("shape_cache", &contexts.shape_cache);
    }

    debug.finish_non_exhaustive()
  }
}

impl ParleyEngine {
  /// The engine over `typefaces` alone: no system font is read, and nothing falls back past
  /// them. Its tab stops are eight spaces apart, its line metrics rounded to whole units, and
  /// it keeps [`SHAPE_CACHE`] shaped words.
  pub fn with_typefaces(typefaces: &[Typeface]) -> Self {
    let mut collection = Collection::new(CollectionOptions {
      shared: false,
      system_fonts: false,
    });

    for typeface in typefaces {
      collection.register_fonts(
        Blob::new(Arc::new(typeface.data)),
        Some(FontInfoOverride {
          family_name: Some(typeface.family),
          ..FontInfoOverride::default()
        }),
      );
    }

    let mut layouts = LayoutContext::new();

    layouts.set_shape_cache_capacity(SHAPE_CACHE);

    Self {
      contexts: Rc::new(RefCell::new(Contexts {
        fonts: FontContext {
          collection,
          source_cache: SourceCache::new_shared(),
        },
        layouts,
        first: typefaces.first().map(|typeface| typeface.family),
        #[cfg(not(target_arch = "wasm32"))]
        shapers: Threads::Unstarted,
        settings: Settings {
          tab_size: TabSize::default(),
          quantize: true,
        },
        shape_cache: SHAPE_CACHE,
      })),
    }
  }

  /// Sets how far apart a tab's stops are for the paragraphs made from now on (eight spaces
  /// unless set, a browser's `tab-size`).
  pub fn set_tab_size(&self, tab_size: TabSize) {
    self.contexts.borrow_mut().settings.tab_size = tab_size;
  }

  /// Sets whether the paragraphs made from now on round their line metrics (each run's
  /// ascent, descent and line height) to whole units, as a renderer aligning lines to its
  /// pixels does (rounded unless set), or keep them fractional, as a browser lays out
  /// lines.
  pub fn set_quantize(&self, quantize: bool) {
    self.contexts.borrow_mut().settings.quantize = quantize;
  }

  /// Keeps at most `words` shaped words for later paragraphs to reuse, in this engine's
  /// context and in each shaping thread it starts from now on ([`SHAPE_CACHE`] unless set):
  /// none where it is zero, which shapes every run whole.
  pub fn set_shape_cache(&self, words: usize) {
    let mut contexts = self.contexts.borrow_mut();

    contexts.shape_cache = words;
    contexts.layouts.set_shape_cache_capacity(words);
  }

  /// How many words this engine's own context found in its shape cache, and how many it
  /// shaped.
  #[must_use]
  pub fn shape_cache_stats(&self) -> (u64, u64) {
    self.contexts.borrow().layouts.shape_cache_stats()
  }

  /// A family as Parley names it.
  fn family_of(family: &FontFamily) -> FontFamilyName<'static> {
    match family {
      FontFamily::System => FontFamilyName::Generic(GenericFamily::SansSerif),
      FontFamily::Monospace => FontFamilyName::Generic(GenericFamily::Monospace),
      FontFamily::Named(name) => FontFamilyName::Named(Cow::Owned(name.to_string())),
    }
  }

  /// The properties of a run as Parley takes them, its family falling back to `first`, its
  /// line height where it sets one.
  fn properties(style: &TextStyle, first: Option<&'static str>) -> Vec<StyleProperty<'static, ()>> {
    let mut families = vec![Self::family_of(&style.family)];

    if let Some(first) = first
      && !matches!(&style.family, FontFamily::Named(name) if &**name == first)
    {
      families.push(FontFamilyName::Named(Cow::Borrowed(first)));
    }

    let mut properties = vec![
      StyleProperty::FontFamily(ParleyFamilies::List(Cow::Owned(families))),
      StyleProperty::FontSize(style.size),
      StyleProperty::FontWeight(ParleyWeight::new(style.weight.0)),
      StyleProperty::FontStyle(match style.slant {
        FontSlant::Upright => ParleySlant::Normal,
        FontSlant::Italic => ParleySlant::Italic,
      }),
      StyleProperty::LetterSpacing(style.letter_spacing),
    ];

    if let Some(height) = style.line_height {
      properties.push(StyleProperty::LineHeight(Self::line_height_of(height)));
    }

    properties
  }

  /// A line height as Parley takes it.
  const fn line_height_of(height: LineHeight) -> ParleyLineHeight {
    match height {
      LineHeight::Normal => ParleyLineHeight::MetricsRelative(1.),
      LineHeight::Factor(factor) => ParleyLineHeight::FontSizeRelative(factor),
      LineHeight::Absolute(height) => ParleyLineHeight::Absolute(height),
    }
  }

  /// What shaping `text` in `style` takes, its families falling back to `first`, laid out
  /// under `settings`.
  fn job(
    first: Option<&'static str>,
    text: &AttributedText,
    style: &ParagraphStyle,
    settings: Settings,
  ) -> Job {
    let mut defaults = vec![StyleProperty::LineHeight(Self::line_height_of(
      style.line_height,
    ))];

    defaults.extend(Self::properties(text.base(), first));
    defaults.push(StyleProperty::OverflowWrap(OverflowWrap::BreakWord));
    defaults.push(StyleProperty::TabSize(settings.tab_size));
    defaults.push(StyleProperty::TextWrapMode(match style.wrap {
      TextWrap::Words => TextWrapMode::Wrap,
      TextWrap::None => TextWrapMode::NoWrap,
    }));

    let ranged = text
      .runs()
      .iter()
      .flat_map(|run| {
        Self::properties(&run.style, first)
          .into_iter()
          .map(|property| (property, run.range.clone()))
      })
      .collect();
    let boxes = text
      .boxes()
      .iter()
      .zip(0_u64..)
      .map(|(inline, id)| ParleyBox {
        id,
        kind: InlineBoxKind::InFlow,
        index: inline.index,
        width: inline.width,
        height: inline.height,
        baseline: None,
        vertical_align: VerticalAlign::BASELINE,
      })
      .collect();

    Job {
      text: text.value().to_owned(),
      defaults,
      ranged,
      boxes,
      quantize: settings.quantize,
      direction: match style.direction {
        BaseDirection::Auto => parley::BaseDirection::Auto,
        BaseDirection::LeftToRight => parley::BaseDirection::Ltr,
        BaseDirection::RightToLeft => parley::BaseDirection::Rtl,
      },
      // Lines no width cuts break where the text breaks them and nowhere else: the layout
      // every paragraph wider than its text keeps.
      unbounded: style
        .max_lines
        .is_none()
        .then(|| ParleyParagraph::alignment(style)),
    }
  }

  /// The paragraph of `text` in `style`, shaped as `shaped`.
  fn wrap(text: &AttributedText, style: &ParagraphStyle, shaped: Shaped) -> ParleyParagraph {
    let value = text.shared_value();

    ParleyParagraph {
      index: Rc::new(TextIndex::new(&value)),
      text: value,
      style: *style,
      right_to_left: shaped.layout.is_rtl(),
      boxes: text.boxes().len(),
      widths: shaped.widths,
      shaped: RefCell::new(Shape::Fresh(shaped.layout, shaped.unbounded)),
      paints: Rc::new(RefCell::new(PaintRuns::of(text))),
    }
  }

  /// The threads shaping beside this one over `fonts`: one fewer than the cores the system
  /// offers and at most seven, each warmed by a sample in `first` laid out under `settings`
  /// and keeping `shape_cache` shaped words.
  #[cfg(not(target_arch = "wasm32"))]
  fn start(
    fonts: &FontContext,
    first: Option<&'static str>,
    settings: Settings,
    shape_cache: usize,
  ) -> Option<Shapers> {
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let sample = AttributedText::plain("Ag", TextStyle::new(13.));
    let warm = Self::job(first, &sample, &ParagraphStyle::default(), settings);

    Shapers::start(fonts, cores.saturating_sub(1).min(7), &warm, shape_cache)
  }
}

impl TextEngine for ParleyEngine {
  type Paragraph = ParleyParagraph;

  fn paragraph(&self, text: &AttributedText, style: &ParagraphStyle) -> ParleyParagraph {
    let mut contexts = self.contexts.borrow_mut();
    let contexts = &mut *contexts;
    let job = Self::job(contexts.first, text, style, contexts.settings);
    let shaped = job.shape(&mut contexts.fonts, &mut contexts.layouts);

    Self::wrap(text, style, shaped)
  }

  /// Shapes a batch of [`PARALLEL`] paragraphs or more on threads beside this one as well,
  /// one fewer than the cores the system offers: the texts are taken as `texts` yields them,
  /// each share queued for the threads as soon as it is whole, so they shape while the next
  /// texts are produced, and this thread shapes what is left once the last is taken.
  #[cfg(not(target_arch = "wasm32"))]
  fn paragraphs(
    &self,
    texts: impl IntoIterator<Item = (AttributedText, ParagraphStyle)>,
  ) -> Vec<skene_foundation::text::Made<ParleyParagraph>> {
    use skene_foundation::text::Made;

    let mut texts = texts.into_iter();
    let first: Vec<(AttributedText, ParagraphStyle)> = texts.by_ref().take(PARALLEL).collect();
    let alone = |(text, style): (AttributedText, ParagraphStyle)| {
      let paragraph = self.paragraph(&text, &style);

      Made {
        text,
        style,
        paragraph,
      }
    };

    if first.len() < PARALLEL {
      return first.into_iter().map(alone).collect();
    }

    let mut contexts = self.contexts.borrow_mut();
    let Contexts {
      fonts,
      layouts,
      first: family,
      shapers,
      settings,
      shape_cache,
    } = &mut *contexts;
    let (family, settings, shape_cache) = (*family, *settings, *shape_cache);

    if matches!(shapers, Threads::Unstarted) {
      *shapers = Self::start(fonts, family, settings, shape_cache)
        .map_or(Threads::Unavailable, Threads::Started);
    }

    let mut kept = Vec::with_capacity(first.len() * 2);
    let jobs = first.into_iter().chain(texts).map(|(text, style)| {
      let job = Self::job(family, &text, &style, settings);

      kept.push((text, style));

      job
    });
    let shaped: Vec<Shaped> = match shapers {
      // Shares small enough that the threads finish close together, each queued as soon as
      // its jobs are built, so the threads shape while the next texts are produced.
      Threads::Started(shapers) => shapers.shape(jobs, SHARE, |job| job.shape(fonts, layouts)),
      Threads::Unstarted | Threads::Unavailable => {
        jobs.map(|job| job.shape(fonts, layouts)).collect()
      }
    };

    kept
      .into_iter()
      .zip(shaped)
      .map(|((text, style), shaped)| {
        let paragraph = Self::wrap(&text, &style, shaped);

        Made {
          text,
          style,
          paragraph,
        }
      })
      .collect()
  }
}

/// What paints each stretch of a paragraph's text: the paragraph's, shared with its layouts,
/// so a repaint shows in every layout already handed out and none is made again.
#[derive(Clone, PartialEq, Debug, Default)]
#[must_use]
struct PaintRuns {
  runs: Vec<(Range<usize>, PaintStyle)>,
}

impl PaintRuns {
  /// The paint of `text`'s spans.
  fn of(text: &AttributedText) -> Self {
    Self {
      runs: text
        .spans()
        .into_iter()
        .map(|(range, style)| (range, style.paint))
        .collect(),
    }
  }

  /// The paint at byte `at`: the last stretch's past the text's end.
  fn at(&self, at: usize) -> PaintStyle {
    self
      .runs
      .iter()
      .find(|(range, _)| range.contains(&at))
      .or_else(|| self.runs.last())
      .map_or_else(PaintStyle::default, |(_, paint)| *paint)
  }

  /// Paints `range` (bytes of the text) with `paint`.
  fn repaint(&mut self, range: Range<usize>, paint: PaintStyle) {
    let mut next = Vec::with_capacity(self.runs.len() + 2);

    for (span, style) in self.runs.drain(..) {
      if span.start < range.start {
        next.push((span.start..span.end.min(range.start), style));
      }

      if span.end > range.end {
        next.push((span.start.max(range.end)..span.end, style));
      }
    }

    next.push((range, paint));
    next.sort_by_key(|(span, _)| span.start);
    self.runs = next;
  }
}

/// A paragraph Parley shaped.
#[must_use]
pub struct ParleyParagraph {
  text: Rc<str>,
  index: Rc<TextIndex>,
  style: ParagraphStyle,
  right_to_left: bool,
  boxes: usize,
  widths: Intrinsic,
  shaped: RefCell<Shape>,
  paints: Rc<RefCell<PaintRuns>>,
}

impl std::fmt::Debug for ParleyParagraph {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ParleyParagraph")
      .field("text", &self.text)
      .field("style", &self.style)
      .field("widths", &self.widths)
      .finish_non_exhaustive()
  }
}

/// A paragraph's shaped text: as shaped, until the first layout breaks its lines in place,
/// and then that layout's, which every later layout copies and breaks again.
#[expect(
  clippy::large_enum_variant,
  reason = "one per paragraph, the shaped layout moved in and out of it whole"
)]
enum Shape {
  /// As shaped, its lines broken and aligned unbounded where it says so, else none broken.
  Fresh(Layout<()>, bool),
  /// The first layout's.
  Laid(Rc<LaidInner>),
  /// Being broken into the first layout.
  Taken,
}

impl Shape {
  /// The shaped layout to break lines in: the fresh one itself, taken, or a copy of the
  /// first layout's.
  ///
  /// Where it is broken and aligned unbounded already, the layout comes with `true`.
  fn take_layout(&mut self) -> (Layout<()>, bool) {
    match std::mem::replace(self, Self::Taken) {
      Self::Fresh(layout, unbounded) => (layout, unbounded),
      Self::Laid(inner) => {
        let layout = inner.layout.clone();

        *self = Self::Laid(inner);

        (layout, false)
      }
      Self::Taken => unreachable!("a layout is never made while another is"),
    }
  }
}

/// A glyph drawn beyond the text: an ellipsis (Parley draws a soft hyphen's hyphen itself).
#[derive(Clone, Debug)]
struct Mark {
  /// The font.
  font: FontData,
  /// The font size.
  size: f32,
  /// The variation coordinates, normalized (`F2Dot14` bits).
  coords: Vec<i16>,
  /// The glyphs, each with its horizontal position.
  glyphs: Vec<(u32, f32)>,
  /// The baseline.
  baseline: f32,
  /// The byte whose paint it takes.
  at: usize,
  /// The line it ends.
  line: usize,
}

impl Mark {
  /// The glyph `character` maps to in `font`, with its advance at `size`.
  fn glyph_of(font: &FontData, coords: &[i16], size: f32, character: char) -> Option<(u32, f32)> {
    let font = FontRef::from_index(font.data.data(), font.index).ok()?;
    let coords: Vec<NormalizedCoord> = coords
      .iter()
      .map(|bits| F2Dot14::from_bits(*bits))
      .collect();
    let glyph = font.charmap().map(character)?;
    let advance = font
      .glyph_metrics(FontSize::new(size), LocationRef::new(&coords))
      .advance_width(glyph)?;

    Some((glyph.to_u32(), advance))
  }
}

impl ParleyParagraph {
  /// The font, size and coordinates of the run holding byte `at` on line `line`.
  fn run_font(layout: &Layout<()>, line: usize, at: usize) -> Option<(FontData, f32, Vec<i16>)> {
    let line = layout.get(line)?;
    let mut found = None;

    for item in line.items() {
      if let PositionedLayoutItem::GlyphRun(glyphs) = item {
        let run = glyphs.run();
        let fits = run.text_range().contains(&at) || run.text_range().end == at;

        if fits || found.is_none() {
          found = Some((
            run.font().clone(),
            run.font_size(),
            run
              .normalized_coords()
              .iter()
              .map(|coord| coord.to_bits())
              .collect(),
          ));
        }
      }
    }

    found
  }

  /// The ellipsis ending line `line`, and its advance: U+2026, or three full stops where the
  /// font has none.
  fn ellipsis(&self, layout: &Layout<()>, line: usize) -> Option<(Mark, f32)> {
    let info = layout.get(line)?;
    let at = info.text_range().end.saturating_sub(1).min(self.text.len());
    let at = self.index.snap_to_grapheme(at, Round::Down);
    let (font, size, coords) = Self::run_font(layout, line, at)?;
    let metrics = info.metrics();
    let start = metrics.offset + metrics.advance - metrics.hanging_advance;
    let glyphs = if let Some((glyph, advance)) = Mark::glyph_of(&font, &coords, size, '\u{2026}') {
      vec![(glyph, start, advance)]
    }
    else {
      let (glyph, advance) = Mark::glyph_of(&font, &coords, size, '.')?;

      (0..3_u8)
        .map(|at| (glyph, start + advance * f32::from(at), advance))
        .collect()
    };
    let advance = glyphs.iter().map(|(_, _, advance)| advance).sum();

    Some((
      Mark {
        font,
        size,
        coords,
        glyphs: glyphs.into_iter().map(|(glyph, x, _)| (glyph, x)).collect(),
        baseline: metrics.baseline,
        at,
        line,
      },
      advance,
    ))
  }

  /// How `style` aligns lines, as Parley names it.
  pub(crate) const fn alignment(style: &ParagraphStyle) -> Alignment {
    match style.alignment {
      TextAlignment::Start => Alignment::Start,
      TextAlignment::End => Alignment::End,
      TextAlignment::Left => Alignment::Left,
      TextAlignment::Right => Alignment::Right,
      TextAlignment::Center => Alignment::Center,
    }
  }

  /// Breaks `layout` within `limit`, each line in `reserve` given that much less room, the
  /// lines cut after `max`: whether text was left over.
  pub(crate) fn break_into(
    layout: &mut Layout<()>,
    limit: f32,
    reserve: &[(usize, f32)],
    max: Option<usize>,
  ) -> bool {
    let mut breaker = layout.break_lines();
    let mut lines = 0;

    breaker.state_mut().set_layout_max_advance(limit);

    loop {
      if max.is_some_and(|max| lines == max) {
        break;
      }

      let room = reserve
        .iter()
        .find(|(line, _)| *line == lines)
        .map_or(limit, |(_, amount)| (limit - amount).max(0.));

      breaker.state_mut().set_line_max_advance(room);

      if breaker.break_next().is_none() {
        break;
      }

      lines += 1;
    }

    let left = !breaker.is_done();

    breaker.finish();

    left
  }

  /// Breaks `layout` within `limit` into at most `max` lines, a cut text's last line making
  /// room for its ellipsis: whether the text was cut.
  fn break_with_room(&self, layout: &mut Layout<()>, limit: f32, max: Option<usize>) -> bool {
    let mut reserve: Vec<(usize, f32)> = Vec::new();

    // A cut text's last line makes room for the ellipsis: break again with that room taken.
    for _ in 0..2 {
      let truncated = Self::break_into(layout, limit, &reserve, max);
      let mut asked = false;

      for line in 0..layout.len() {
        if reserve.iter().any(|(at, _)| *at == line) {
          continue;
        }

        let need = if truncated && self.style.ellipsis && Some(line + 1) == max {
          self.ellipsis(layout, line).map(|(_, advance)| advance)
        }
        else {
          None
        };
        let Some(need) = need
        else {
          continue;
        };
        let used = layout.get(line).map_or(0., |line| {
          line.metrics().advance - line.metrics().hanging_advance
        });

        if used + need > limit {
          reserve.push((line, need));
          asked = true;

          break;
        }
      }

      if !asked {
        break;
      }
    }

    max.is_some_and(|max| layout.len() >= max) && {
      let shown = layout
        .get(layout.len().saturating_sub(1))
        .map_or(0, |line| line.text_range().end);

      shown < self.text.len()
    }
  }

  /// The frame of each of the text's inline boxes as `layout` places them, in the text's
  /// order: one of no size at the origin for a box no line holds.
  fn box_frames(&self, layout: &Layout<()>) -> Vec<Rect<f32>> {
    let mut frames = vec![Rect::ZERO; self.boxes];

    for line in layout.lines() {
      for item in line.items() {
        if let PositionedLayoutItem::InlineBox(inline) = item
          && let Some(frame) = usize::try_from(inline.id)
            .ok()
            .and_then(|id| frames.get_mut(id))
        {
          *frame = Rect::new(
            Point::new(inline.x, inline.y),
            Size::new(inline.width, inline.height),
          );
        }
      }
    }

    frames
  }
}

impl Paragraph for ParleyParagraph {
  type Laid = ParleyLaid;

  fn text(&self) -> &str {
    &self.text
  }

  fn intrinsic(&self) -> Intrinsic {
    self.widths
  }

  fn layout(&self, width: Option<f32>) -> ParleyLaid {
    let limit = match self.style.wrap {
      TextWrap::Words => width.unwrap_or(f32::MAX),
      TextWrap::None => f32::MAX,
    };
    let max = self.style.max_lines.map(|lines| lines.max(1) as usize);
    let (mut layout, unbounded) = self.shaped.borrow_mut().take_layout();
    // Laid out unbounded already: no line is cut, and none draws an ellipsis.
    let laid = unbounded && limit >= f32::MAX && max.is_none();
    let truncated = !laid && self.break_with_room(&mut layout, limit, max);

    if !laid {
      layout.align(Self::alignment(&self.style), AlignmentOptions::default());
    }

    let marks: Vec<(Mark, f32)> = if truncated && self.style.ellipsis {
      self
        .ellipsis(&layout, layout.len().saturating_sub(1))
        .into_iter()
        .collect()
    }
    else {
      Vec::new()
    };
    let lines = layout
      .lines()
      .enumerate()
      .map(|(at, line)| {
        let metrics = line.metrics();
        let extra: f32 = marks
          .iter()
          .filter(|(mark, _)| mark.line == at)
          .map(|(_, advance)| advance)
          .sum();

        LineInfo {
          range: line.text_range(),
          top: metrics.block_min_coord,
          height: metrics.block_max_coord - metrics.block_min_coord,
          baseline: metrics.baseline,
          left: metrics.offset,
          width: metrics.advance - metrics.hanging_advance + extra,
          hard_break: line.break_reason() == BreakReason::Explicit,
        }
      })
      .collect::<Vec<_>>();
    let width = lines
      .iter()
      .map(|line| line.width)
      .fold(layout.width(), f32::max);
    let laid = ParleyLaid {
      inner: Rc::new(LaidInner {
        width,
        height: layout.height(),
        boxes: self.box_frames(&layout),
        layout,
        text: Rc::clone(&self.text),
        index: Rc::clone(&self.index),
        lines,
        truncated,
        marks: marks.into_iter().map(|(mark, _)| mark).collect(),
        paints: Rc::clone(&self.paints),
      }),
    };
    let mut shaped = self.shaped.borrow_mut();

    if matches!(*shaped, Shape::Taken) {
      *shaped = Shape::Laid(Rc::clone(&laid.inner));
    }

    laid
  }

  fn is_left_anchored(&self) -> bool {
    match self.style.alignment {
      TextAlignment::Left => true,
      TextAlignment::Right | TextAlignment::Center => false,
      TextAlignment::Start => !self.right_to_left,
      TextAlignment::End => self.right_to_left,
    }
  }

  fn repaint(&mut self, range: Range<usize>, paint: &PaintStyle) -> bool {
    self.paints.borrow_mut().repaint(range, *paint);

    true
  }

  fn boundary(&self, index: usize, unit: TextBoundary, direction: Direction) -> usize {
    Segments::new(&self.text).boundary(&self.index, index, unit, direction)
  }
}

/// What a layout holds, shared by its clones.
struct LaidInner {
  layout: Layout<()>,
  text: Rc<str>,
  index: Rc<TextIndex>,
  width: f32,
  height: f32,
  lines: Vec<LineInfo>,
  boxes: Vec<Rect<f32>>,
  truncated: bool,
  marks: Vec<Mark>,
  paints: Rc<RefCell<PaintRuns>>,
}

/// A paragraph Parley laid out at one width, shared by its clones: its geometry never changes,
/// and it paints as its paragraph paints now.
#[derive(Clone)]
#[must_use]
pub struct ParleyLaid {
  inner: Rc<LaidInner>,
}

impl std::fmt::Debug for ParleyLaid {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ParleyLaid")
      .field("text", &self.inner.text)
      .field("width", &self.inner.width)
      .field("height", &self.inner.height)
      .field("lines", &self.inner.lines)
      .finish_non_exhaustive()
  }
}

/// One glyph to draw.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub struct PositionedGlyph {
  /// The glyph's id in its font.
  pub id: u32,
  /// Its pen position, horizontally.
  pub x: f32,
  /// Its baseline.
  pub y: f32,
  /// The byte of the text whose paint it takes.
  pub at: usize,
}

/// A band holding every line: the whole text.
const EVERYWHERE: (f32, f32) = (f32::NEG_INFINITY, f32::INFINITY);

/// A run of glyphs in one font, as a renderer draws them.
#[derive(Clone, Debug)]
#[must_use]
pub struct GlyphRun {
  /// The font.
  pub font: FontData,
  /// The font size.
  pub size: f32,
  /// The variation coordinates, normalized (`F2Dot14` bits).
  pub coords: Vec<i16>,
  /// The glyphs.
  pub glyphs: Vec<PositionedGlyph>,
}

/// An affinity as Parley names it.
const fn to_parley(affinity: Affinity) -> ParleyAffinity {
  match affinity {
    Affinity::Downstream => ParleyAffinity::Downstream,
    Affinity::Upstream => ParleyAffinity::Upstream,
  }
}

/// Parley's affinity as the foundation names it.
const fn from_parley(affinity: ParleyAffinity) -> Affinity {
  match affinity {
    ParleyAffinity::Downstream => Affinity::Downstream,
    ParleyAffinity::Upstream => Affinity::Upstream,
  }
}

/// A rectangle of Parley's as the foundation's.
#[expect(clippy::cast_possible_truncation, reason = "layout coordinates")]
fn rect_of(rect: parley::BoundingBox) -> Rect<f32> {
  Rect::new(
    Point::new(rect.x0 as f32, rect.y0 as f32),
    Size::new((rect.x1 - rect.x0) as f32, (rect.y1 - rect.y0) as f32),
  )
}

impl ParleyLaid {
  /// Parley's cursor at `position`.
  fn cursor(&self, position: Position) -> Cursor {
    Cursor::from_byte_index(
      &self.inner.layout,
      position.index,
      to_parley(position.affinity),
    )
  }

  /// The position at Parley's `cursor`.
  fn position(cursor: Cursor) -> Position {
    Position::with(cursor.index(), from_parley(cursor.affinity()))
  }

  /// Parley's layout.
  #[must_use]
  pub fn layout(&self) -> &Layout<()> {
    &self.inner.layout
  }

  /// The text's index.
  #[must_use]
  pub fn index(&self) -> &TextIndex {
    &self.inner.index
  }

  /// The paint at byte `at`, as it is now.
  pub fn paint_at(&self, at: usize) -> PaintStyle {
    self.inner.paints.borrow().at(at)
  }

  /// Whether a line of `metrics` may show between `band`'s top and bottom (the layout's
  /// units): its line box, grown by its height each way for the glyphs that overflow it.
  fn line_in(metrics: &parley::LineMetrics, band: (f32, f32)) -> bool {
    let reach = metrics.line_height;

    metrics.block_max_coord + reach >= band.0 && metrics.block_min_coord - reach <= band.1
  }

  /// The stretches painted alike of the lines that may show between `band`'s top and bottom
  /// (the layout's units), as [`Laid::run_extents`] gives them for every line: what a box
  /// showing that band of the text decorates.
  #[must_use]
  pub fn run_extents_in(&self, band: (f32, f32)) -> Vec<RunExtent> {
    let paints = self.inner.paints.borrow();
    let mut extents: Vec<RunExtent> = Vec::new();

    for (index, line) in self.inner.layout.lines().enumerate() {
      if !Self::line_in(line.metrics(), band) {
        continue;
      }

      for item in line.items() {
        let PositionedLayoutItem::GlyphRun(glyph_run) = item
        else {
          continue;
        };
        let run = glyph_run.run();
        let metrics = run.font_metrics();
        let mut x = glyph_run.offset();

        for cluster in run.visual_clusters() {
          let range = cluster.text_range();
          let advance = cluster.advance();
          let paint = paints.at(range.start);

          match extents.last_mut() {
            Some(last)
              if last.line == index
                && last.paint == paint
                && (last.x + last.width - x).abs() < 0.01
                && Self::measured_alike(last, glyph_run.baseline(), metrics) =>
            {
              last.width += advance;
              last.range = last.range.start.min(range.start)..last.range.end.max(range.end);
            }
            _ => extents.push(RunExtent {
              range,
              line: index,
              x,
              width: advance,
              baseline: glyph_run.baseline(),
              ascent: metrics.ascent,
              descent: metrics.descent,
              underline_offset: metrics.underline_offset,
              underline_thickness: metrics.underline_size,
              strikethrough_offset: metrics.strikethrough_offset,
              strikethrough_thickness: metrics.strikethrough_size,
              paint,
            }),
          }

          x += advance;
        }
      }
    }

    extents
  }

  /// Whether `extent` sits on `baseline` with a run's font `metrics`, so a stretch of that
  /// run joins it.
  const fn measured_alike(
    extent: &RunExtent,
    baseline: f32,
    metrics: &parley::layout::FontMetrics,
  ) -> bool {
    extent.baseline.to_bits() == baseline.to_bits()
      && extent.ascent.to_bits() == metrics.ascent.to_bits()
      && extent.descent.to_bits() == metrics.descent.to_bits()
      && extent.underline_offset.to_bits() == metrics.underline_offset.to_bits()
      && extent.underline_thickness.to_bits() == metrics.underline_size.to_bits()
      && extent.strikethrough_offset.to_bits() == metrics.strikethrough_offset.to_bits()
      && extent.strikethrough_thickness.to_bits() == metrics.strikethrough_size.to_bits()
  }

  /// The glyphs to draw, run by run, the hyphens and the ellipsis included.
  #[must_use]
  pub fn glyph_runs(&self) -> Vec<GlyphRun> {
    self.glyph_runs_in(EVERYWHERE)
  }

  /// The glyphs to draw of the lines that may show between `band`'s top and bottom (the
  /// layout's units), run by run: what of [`glyph_runs`](Self::glyph_runs) a box showing
  /// that band of the text draws.
  #[must_use]
  pub fn glyph_runs_in(&self, band: (f32, f32)) -> Vec<GlyphRun> {
    let mut runs = Vec::new();

    for line in self.inner.layout.lines() {
      if !Self::line_in(line.metrics(), band) {
        continue;
      }

      for item in line.items() {
        let PositionedLayoutItem::GlyphRun(glyph_run) = item
        else {
          continue;
        };
        let run = glyph_run.run();
        let baseline = glyph_run.baseline();
        let mut x = glyph_run.offset();
        let mut glyphs = Vec::new();

        for cluster in run.visual_clusters() {
          let at = cluster.text_range().start;

          for glyph in cluster.glyphs() {
            glyphs.push(PositionedGlyph {
              id: glyph.id,
              x: x + glyph.x,
              y: baseline + glyph.y,
              at,
            });
            x += glyph.advance;
          }
        }

        runs.push(GlyphRun {
          font: run.font().clone(),
          size: run.font_size(),
          coords: run
            .normalized_coords()
            .iter()
            .map(|coord| coord.to_bits())
            .collect(),
          glyphs,
        });
      }
    }

    for mark in &self.inner.marks {
      if mark.baseline + mark.size * 2. < band.0 || mark.baseline - mark.size * 2. > band.1 {
        continue;
      }

      runs.push(GlyphRun {
        font: mark.font.clone(),
        size: mark.size,
        coords: mark.coords.clone(),
        glyphs: mark
          .glyphs
          .iter()
          .map(|(id, x)| PositionedGlyph {
            id: *id,
            x: *x,
            y: mark.baseline,
            at: mark.at,
          })
          .collect(),
      });
    }

    runs
  }
}

impl Laid for ParleyLaid {
  fn size(&self) -> TextSize {
    TextSize {
      width: self.inner.width,
      height: self.inner.height,
    }
  }

  fn first_baseline(&self) -> f32 {
    self.inner.layout.first_baseline().unwrap_or_default()
  }

  fn last_baseline(&self) -> f32 {
    self.inner.layout.last_baseline().unwrap_or_default()
  }

  fn line_count(&self) -> usize {
    self.inner.lines.len()
  }

  fn truncated(&self) -> bool {
    self.inner.truncated
  }

  fn lines(&self) -> Vec<LineInfo> {
    self.inner.lines.clone()
  }

  fn hit_test(&self, x: f32, y: f32) -> Hit {
    let cursor = Cursor::from_point(&self.inner.layout, x, y);
    let inside = y >= 0.
      && y < self.inner.height
      && self.inner.lines.iter().any(|line| {
        y >= line.top && y < line.top + line.height && x >= line.left && x < line.left + line.width
      });

    Hit {
      position: Self::position(cursor),
      inside,
    }
  }

  fn caret(&self, position: Position) -> Carets {
    let layout = &self.inner.layout;
    let caret = |cursor: Cursor| {
      let rect = rect_of(cursor.geometry(layout, 1.));
      let right_to_left = match cursor.affinity() {
        ParleyAffinity::Upstream => cursor.upstream_cluster(layout),
        ParleyAffinity::Downstream => cursor.downstream_cluster(layout),
      }
      .is_some_and(|cluster| cluster.is_rtl());

      Caret {
        x: rect.origin.x,
        y: rect.origin.y,
        height: rect.size.height,
        right_to_left,
      }
    };
    let primary = caret(self.cursor(position));
    let secondary = caret(Cursor::from_byte_index(
      layout,
      position.index,
      match position.affinity {
        Affinity::Downstream => ParleyAffinity::Upstream,
        Affinity::Upstream => ParleyAffinity::Downstream,
      },
    ));
    let same_line = (secondary.y - primary.y).abs() < 0.5;

    Carets {
      primary,
      secondary: (same_line && (secondary.x - primary.x).abs() > 0.5).then_some(secondary),
    }
  }

  fn range_rects(&self, range: Range<usize>) -> Vec<Rect<f32>> {
    let layout = &self.inner.layout;
    let selection = Selection::new(
      Cursor::from_byte_index(layout, range.start, ParleyAffinity::Downstream),
      Cursor::from_byte_index(layout, range.end, ParleyAffinity::Upstream),
    );

    selection
      .geometry(layout)
      .into_iter()
      .map(|(rect, _)| rect_of(rect))
      .collect()
  }

  fn inline_boxes(&self) -> Vec<Rect<f32>> {
    self.inner.boxes.clone()
  }

  fn run_extents(&self) -> Vec<RunExtent> {
    self.run_extents_in(EVERYWHERE)
  }

  fn move_visual(&self, position: Position, movement: VisualMove) -> Position {
    let layout = &self.inner.layout;
    let cursor = self.cursor(position);
    let selection = Selection::from(cursor);

    Self::position(match movement {
      VisualMove::Left => cursor.previous_visual(layout),
      VisualMove::Right => cursor.next_visual(layout),
      VisualMove::WordLeft => cursor.previous_visual_word(layout),
      VisualMove::WordRight => cursor.next_visual_word(layout),
      VisualMove::LineStart => selection.line_start(layout, false).focus(),
      VisualMove::LineEnd => selection.line_end(layout, false).focus(),
    })
  }

  fn move_vertical(&self, position: Position, lines: isize, goal: Option<f32>) -> (Position, f32) {
    let carets = self.caret(position);
    let goal = goal.unwrap_or(carets.primary.x);
    let middle = carets.primary.y + carets.primary.height / 2.;
    let current = self
      .inner
      .lines
      .iter()
      .position(|line| middle >= line.top && middle < line.top + line.height)
      .unwrap_or(0);

    match current.checked_add_signed(lines) {
      Some(target) if target < self.inner.lines.len() => {
        let line = &self.inner.lines[target];
        let cursor = Cursor::from_point(&self.inner.layout, goal, line.top + line.height / 2.);

        (Self::position(cursor), goal)
      }
      _ if lines < 0 => (Position::new(0), goal),
      _ => (Position::new(self.inner.text.len()), goal),
    }
  }
}
