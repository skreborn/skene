//! Parley as a text engine: shaping once per paragraph, breaking per width into an immutable
//! layout, and the geometry and movement Parley's editing primitives give.

use std::{
  borrow::Cow,
  cell::{Ref, RefCell, RefMut},
  ops::Range,
  rc::Rc,
  sync::Arc,
};

use crate::shaping::{Job, PARALLEL, Shaped, Shapers};
use parley::{
  Affinity as ParleyAffinity, Alignment, AlignmentOptions, BreakReason, Cursor, FontContext,
  FontData, FontFamily as ParleyFamily, FontFamilyName, FontStyle as ParleySlant, FontWeight,
  Layout, LayoutContext, LineHeight as ParleyLineHeight, OverflowWrap, PositionedLayoutItem,
  Selection, StyleProperty, TextWrapMode,
  fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, SourceCache},
};
use skene_foundation::{
  layout::Rect,
  text::{
    Affinity, AttributedText, BaseDirection, Caret, Carets, Direction, Hit, Intrinsic, Laid,
    LineHeight, LineInfo, PaintStyle, Paragraph, ParagraphStyle, Position, RunExtent, RunStyle,
    Segments, TextBoundary, TextEngine, TextIndex, TextSize, VisualMove, Wrap,
  },
  value::{FontFamily, FontStyle, TextAlignment},
};
use skrifa::{
  FontRef, MetadataProvider as _,
  instance::{LocationRef, NormalizedCoord, Size},
  raw::types::F2Dot14,
};

/// A typeface an engine shapes in alone, with no system font: its family name and its data.
#[derive(Clone, Copy, Debug)]
pub struct Typeface {
  /// The family it is known by.
  pub family: &'static str,
  /// Its OpenType data.
  pub data: &'static [u8],
}

/// What resolves a run's family per character, where a platform has one.
#[cfg(target_os = "linux")]
type Fallback = Option<crate::linux::Fallback>;
/// What resolves a run's family per character, where a platform has one.
#[cfg(not(target_os = "linux"))]
type Fallback = ();

/// The shaping and layout contexts an engine's paragraphs share.
struct Contexts {
  fonts: FontContext,
  layouts: LayoutContext<()>,
  fallback: Fallback,
  /// The threads shaping a batch beside this one: `None` until a batch asks for them, then
  /// what started.
  shapers: Option<Option<Shapers>>,
}

/// Parley as a [`TextEngine`]: over the system's fonts (and on Linux, Skene's fontconfig
/// fallback), or over typefaces of its own.
///
/// Clones share their contexts.
#[derive(Clone)]
pub struct ParleyEngine {
  contexts: Rc<RefCell<Contexts>>,
}

impl ParleyEngine {
  /// The engine over the system's fonts, Linux's fontconfig resolving each character's
  /// family.
  #[must_use]
  pub fn system() -> Self {
    Self {
      contexts: Rc::new(RefCell::new(Contexts {
        // The loaded fonts are shared with the threads that shape beside this one.
        fonts: FontContext {
          collection: Collection::new(CollectionOptions::default()),
          source_cache: SourceCache::new_shared(),
        },
        layouts: LayoutContext::new(),
        #[cfg(target_os = "linux")]
        fallback: crate::linux::Fallback::load(),
        #[cfg(not(target_os = "linux"))]
        fallback: (),
        shapers: None,
      })),
    }
  }

  /// The engine over `typefaces` alone: no system font is read, and nothing falls back
  /// past them.
  #[must_use]
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

    Self {
      contexts: Rc::new(RefCell::new(Contexts {
        fonts: FontContext {
          collection,
          source_cache: SourceCache::new_shared(),
        },
        layouts: LayoutContext::new(),
        #[cfg(target_os = "linux")]
        fallback: None,
        #[cfg(not(target_os = "linux"))]
        fallback: (),
        shapers: None,
      })),
    }
  }

  /// The fontconfig fallback, where the engine resolves through it.
  #[cfg(target_os = "linux")]
  #[must_use]
  pub fn fallback(&self) -> Option<RefMut<'_, crate::linux::Fallback>> {
    RefMut::filter_map(self.contexts.borrow_mut(), |contexts| {
      contexts.fallback.as_mut()
    })
    .ok()
  }

  /// The font context, for a renderer that reads it.
  #[must_use]
  pub fn fonts(&self) -> Ref<'_, FontContext> {
    Ref::map(self.contexts.borrow(), |contexts| &contexts.fonts)
  }
}

/// A family as Parley names it.
fn family_of(family: &FontFamily) -> FontFamilyName<'static> {
  match family {
    FontFamily::System => FontFamilyName::Generic(parley::GenericFamily::SansSerif),
    FontFamily::Monospace => FontFamilyName::Generic(parley::GenericFamily::Monospace),
    FontFamily::Named(name) => FontFamilyName::Named(Cow::Owned(name.to_string())),
  }
}

/// The properties of a run as Parley takes them, its family named first by `resolved` where
/// a fallback resolved one.
fn properties(style: &RunStyle, resolved: Option<&str>) -> Vec<StyleProperty<'static, ()>> {
  let mut families = Vec::new();

  if let Some(resolved) = resolved {
    families.push(FontFamilyName::Named(Cow::Owned(resolved.to_owned())));
  }

  families.push(family_of(&style.family));

  vec![
    StyleProperty::FontFamily(ParleyFamily::List(Cow::Owned(families))),
    StyleProperty::FontSize(style.size),
    StyleProperty::FontWeight(FontWeight::new(f32::from(style.weight.0))),
    StyleProperty::FontStyle(match style.slant {
      FontStyle::Normal => ParleySlant::Normal,
      FontStyle::Italic => ParleySlant::Italic,
    }),
    StyleProperty::LetterSpacing(style.letter_spacing),
  ]
}

impl ParleyEngine {
  /// Starts the threads that shape a batch of paragraphs beside this one, where none run yet:
  /// one fewer than the cores the system offers, each shaping a sample of the system's font
  /// as it starts, so its fonts are ready when a batch comes.
  pub fn start_shapers(&self) {
    let mut contexts = self.contexts.borrow_mut();
    let Contexts {
      fonts,
      fallback,
      shapers,
      ..
    } = &mut *contexts;

    if shapers.is_none() {
      *shapers = Some(Self::start(fonts, fallback));
    }
  }

  /// The threads shaping beside this one, over `fonts`, warmed by a sample `fallback`
  /// resolves.
  fn start(fonts: &FontContext, fallback: &mut Fallback) -> Option<Shapers> {
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let sample = AttributedText::plain("Ag", RunStyle::new(13.));
    let warm = Self::job(fallback, &sample, &ParagraphStyle::default());

    Shapers::start(fonts, cores.saturating_sub(1).min(7), &warm)
  }

  /// What shaping `text` in `style` takes, its families resolved by `fallback` first.
  fn job(fallback: &mut Fallback, text: &AttributedText, style: &ParagraphStyle) -> Job {
    #[cfg(target_os = "linux")]
    let resolved = fallback
      .as_mut()
      .map(|fallback| fallback.resolve(text))
      .unwrap_or_default();
    #[cfg(not(target_os = "linux"))]
    let resolved: Vec<(Range<usize>, Rc<str>)> = {
      let () = fallback;

      Vec::new()
    };
    let mut defaults = properties(text.base(), None);

    defaults.push(StyleProperty::LineHeight(match style.line_height {
      LineHeight::Normal => ParleyLineHeight::MetricsRelative(1.),
      LineHeight::Factor(factor) => ParleyLineHeight::FontSizeRelative(factor),
      LineHeight::Absolute(height) => ParleyLineHeight::Absolute(height),
    }));
    defaults.push(StyleProperty::OverflowWrap(OverflowWrap::BreakWord));
    defaults.push(StyleProperty::TextWrapMode(match style.wrap {
      Wrap::Words => TextWrapMode::Wrap,
      Wrap::None => TextWrapMode::NoWrap,
    }));

    let mut ranged = Vec::new();

    for run in text.runs() {
      for property in properties(&run.style, None) {
        ranged.push((property, run.range.clone()));
      }
    }

    for (range, family) in &resolved {
      let style = text
        .runs()
        .iter()
        .find(|run| run.range.start <= range.start && range.end <= run.range.end)
        .map_or(text.base(), |run| &run.style);

      ranged.push((
        properties(style, Some(family)).swap_remove(0),
        range.clone(),
      ));
    }

    Job {
      text: text.value().to_owned(),
      defaults,
      ranged,
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
      widths: shaped.widths,
      shaped: RefCell::new(Shape::Fresh(shaped.layout, shaped.unbounded)),
      paints: Rc::new(RefCell::new(PaintRuns::of(text))),
    }
  }
}

impl TextEngine for ParleyEngine {
  type Paragraph = ParleyParagraph;

  fn paragraph(&self, text: &AttributedText, style: &ParagraphStyle) -> ParleyParagraph {
    let mut contexts = self.contexts.borrow_mut();
    let contexts = &mut *contexts;
    let job = Self::job(&mut contexts.fallback, text, style);
    let shaped = job.shape(&mut contexts.fonts, &mut contexts.layouts);

    Self::wrap(text, style, shaped)
  }

  /// Shapes a batch of [`PARALLEL`] paragraphs or more on threads beside this one as well,
  /// one fewer than the cores the system offers, each a share.
  fn paragraphs(&self, texts: &[(AttributedText, ParagraphStyle)]) -> Vec<ParleyParagraph> {
    if texts.len() < PARALLEL {
      return texts
        .iter()
        .map(|(text, style)| self.paragraph(text, style))
        .collect();
    }

    let mut contexts = self.contexts.borrow_mut();
    let Contexts {
      fonts,
      layouts,
      fallback,
      shapers,
    } = &mut *contexts;
    let shapers = shapers.get_or_insert_with(|| Self::start(fonts, fallback));
    let jobs = texts
      .iter()
      .map(|(text, style)| Self::job(fallback, text, style));
    let shaped: Vec<Shaped> = match shapers {
      // Shares small enough that the threads finish close together, each queued as its jobs
      // are built, so they shape while the next are.
      Some(shapers) => shapers.shape(
        jobs,
        texts.len().div_ceil(4 * (shapers.count() + 1)).max(16),
        |job| job.shape(fonts, layouts),
      ),
      None => jobs.map(|job| job.shape(fonts, layouts)).collect(),
    };

    texts
      .iter()
      .zip(shaped)
      .map(|((text, style), shaped)| Self::wrap(text, style, shaped))
      .collect()
  }
}

/// What paints each stretch of a paragraph's text: kept beside its layouts, so a repaint
/// reaches every layout made and none is made again.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct PaintRuns {
  runs: Vec<(Range<usize>, PaintStyle)>,
}

impl PaintRuns {
  /// The paint of `text`'s spans.
  #[must_use]
  pub fn of(text: &AttributedText) -> Self {
    Self {
      runs: text
        .spans()
        .into_iter()
        .map(|(range, style)| (range, style.paint))
        .collect(),
    }
  }

  /// The paint at byte `at`.
  pub fn at(&self, at: usize) -> PaintStyle {
    self
      .runs
      .iter()
      .find(|(range, _)| range.contains(&at))
      .or_else(|| self.runs.last())
      .map_or_else(PaintStyle::default, |(_, paint)| *paint)
  }

  /// Paints `range` with `paint`.
  pub fn repaint(&mut self, range: Range<usize>, paint: PaintStyle) {
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
pub struct ParleyParagraph {
  text: Rc<str>,
  index: Rc<TextIndex>,
  style: ParagraphStyle,
  widths: Intrinsic,
  shaped: RefCell<Shape>,
  paints: Rc<RefCell<PaintRuns>>,
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

/// A glyph drawn beyond the text: the hyphen at a soft hyphen's break, or an ellipsis.
#[derive(Clone, Debug)]
pub struct Mark {
  /// The font.
  pub font: FontData,
  /// The font size.
  pub size: f32,
  /// The variation coordinates, normalized (F2Dot14 bits).
  pub coords: Vec<i16>,
  /// The glyphs, each with its horizontal position.
  pub glyphs: Vec<(u32, f32)>,
  /// The baseline.
  pub baseline: f32,
  /// The byte whose paint it takes.
  pub at: usize,
  /// The line it ends.
  pub line: usize,
}

/// The glyph `character` maps to in `font`, with its advance at `size`.
fn glyph_of(font: &FontData, coords: &[i16], size: f32, character: char) -> Option<(u32, f32)> {
  let font = FontRef::from_index(font.data.data(), font.index).ok()?;
  let coords: Vec<NormalizedCoord> = coords
    .iter()
    .map(|bits| F2Dot14::from_bits(*bits))
    .collect();
  let glyph = font.charmap().map(character)?;
  let advance = font
    .glyph_metrics(Size::new(size), LocationRef::new(&coords))
    .advance_width(glyph)?;

  Some((glyph.to_u32(), advance))
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

  /// The hyphen drawn after line `line` of `layout` where it breaks at a soft hyphen, with
  /// its advance.
  fn hyphen(&self, layout: &Layout<()>, line: usize) -> Option<(Mark, f32)> {
    let info = layout.get(line)?;

    if info.break_reason() != BreakReason::Regular {
      return None;
    }

    let range = info.text_range();

    if !self.text[range.clone()].ends_with('\u{AD}') {
      return None;
    }

    let at = range.end - '\u{AD}'.len_utf8();
    let (font, size, coords) = Self::run_font(layout, line, at)?;
    let (glyph, advance) =
      glyph_of(&font, &coords, size, '\u{2010}').or_else(|| glyph_of(&font, &coords, size, '-'))?;
    let metrics = info.metrics();

    Some((
      Mark {
        font,
        size,
        coords,
        glyphs: vec![(
          glyph,
          metrics.offset + metrics.advance - metrics.hanging_advance,
        )],
        baseline: metrics.baseline,
        at,
        line,
      },
      advance,
    ))
  }

  /// The ellipsis ending line `line`, and its advance.
  fn ellipsis(&self, layout: &Layout<()>, line: usize) -> Option<(Mark, f32)> {
    let info = layout.get(line)?;
    let at = info.text_range().end.saturating_sub(1).min(self.text.len());
    let at = self
      .index
      .snap_to_grapheme(at, skene_foundation::text::Round::Down);
    let (font, size, coords) = Self::run_font(layout, line, at)?;
    let metrics = info.metrics();
    let start = metrics.offset + metrics.advance - metrics.hanging_advance;
    let glyphs = match glyph_of(&font, &coords, size, '\u{2026}') {
      Some((glyph, advance)) => vec![(glyph, start, advance)],
      None => {
        let (glyph, advance) = glyph_of(&font, &coords, size, '.')?;

        (0..3)
          .map(|at| (glyph, start + advance * at as f32, advance))
          .collect()
      }
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
      TextAlignment::Leading => Alignment::Start,
      TextAlignment::Center => Alignment::Center,
      TextAlignment::Trailing => Alignment::End,
      TextAlignment::Justify => Alignment::Justify,
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
      Wrap::Words => width.unwrap_or(f32::MAX),
      Wrap::None => f32::MAX,
    };
    let max = self.style.max_lines.map(|lines| lines.max(1) as usize);
    let (mut layout, unbounded) = self.shaped.borrow_mut().take_layout();
    let mut reserve: Vec<(usize, f32)> = Vec::new();
    let mut truncated;
    // Laid out unbounded already: no line breaks at a soft hyphen, and none is cut.
    let laid = unbounded && limit == f32::MAX && max.is_none();

    // A line ending in a soft hyphen makes room for the hyphen it draws, and a cut text's
    // last line for the ellipsis: break again with that room taken, until no line asks.
    for _ in 0..if laid { 0 } else { 4 } {
      truncated = Self::break_into(&mut layout, limit, &reserve, max);

      let mut asked = false;

      for line in 0..layout.len() {
        if reserve.iter().any(|(at, _)| *at == line) {
          continue;
        }

        let need = if truncated && self.style.ellipsis && Some(line + 1) == max {
          self.ellipsis(&layout, line).map(|(_, advance)| advance)
        } else {
          self.hyphen(&layout, line).map(|(_, advance)| advance)
        };
        let Some(need) = need else {
          continue;
        };
        let metrics = layout.get(line).map(|line| *line.metrics());
        let used = metrics.map_or(0., |metrics| metrics.advance - metrics.hanging_advance);

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

    truncated = max.is_some_and(|max| layout.len() >= max) && {
      let shown = layout
        .get(layout.len().saturating_sub(1))
        .map_or(0, |line| line.text_range().end);

      shown < self.text.len()
    };

    if !laid {
      layout.align(Self::alignment(&self.style), AlignmentOptions::default());
    }

    let mut marks = Vec::new();

    // Laid out unbounded, no line breaks at a soft hyphen nor is cut: none draws a mark.
    for line in 0..if laid { 0 } else { layout.len() } {
      let mark = if truncated && self.style.ellipsis && line + 1 == layout.len() {
        self.ellipsis(&layout, line)
      } else {
        self.hyphen(&layout, line)
      };

      if let Some((mark, advance)) = mark {
        marks.push((mark, advance));
      }
    }

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

  fn repaint(&mut self, range: Range<usize>, paint: &PaintStyle) -> bool {
    self.paints.borrow_mut().repaint(range, *paint);

    true
  }

  fn boundary(&self, index: usize, unit: TextBoundary, direction: Direction) -> usize {
    Segments::new(&self.text).boundary(&self.index, index, unit, direction)
  }
}

struct LaidInner {
  layout: Layout<()>,
  text: Rc<str>,
  index: Rc<TextIndex>,
  width: f32,
  height: f32,
  lines: Vec<LineInfo>,
  truncated: bool,
  marks: Vec<Mark>,
  paints: Rc<RefCell<PaintRuns>>,
}

/// A paragraph Parley laid out at one width: immutable, shared by its clones.
#[derive(Clone)]
pub struct ParleyLaid {
  inner: Rc<LaidInner>,
}

/// One glyph to draw.
#[derive(Clone, Copy, PartialEq, Debug)]
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

/// A run of glyphs in one font, as a renderer draws them.
#[derive(Clone, Debug)]
pub struct GlyphRun {
  /// The font.
  pub font: FontData,
  /// The font size.
  pub size: f32,
  /// The variation coordinates, normalized (F2Dot14 bits).
  pub coords: Vec<i16>,
  /// The glyphs.
  pub glyphs: Vec<PositionedGlyph>,
}

const fn to_parley(affinity: Affinity) -> ParleyAffinity {
  match affinity {
    Affinity::Downstream => ParleyAffinity::Downstream,
    Affinity::Upstream => ParleyAffinity::Upstream,
  }
}

const fn from_parley(affinity: ParleyAffinity) -> Affinity {
  match affinity {
    ParleyAffinity::Downstream => Affinity::Downstream,
    ParleyAffinity::Upstream => Affinity::Upstream,
  }
}

impl ParleyLaid {
  fn cursor(&self, position: Position) -> Cursor {
    Cursor::from_byte_index(
      &self.inner.layout,
      position.index,
      to_parley(position.affinity),
    )
  }

  fn position(cursor: Cursor) -> Position {
    Position::with(cursor.index(), from_parley(cursor.affinity()))
  }

  /// Parley's layout.
  #[must_use]
  pub fn layout(&self) -> &Layout<()> {
    &self.inner.layout
  }

  /// The paint at byte `at`, as it is now.
  pub fn paint_at(&self, at: usize) -> PaintStyle {
    self.inner.paints.borrow().at(at)
  }

  /// The glyphs to draw, run by run, the hyphens and the ellipsis included.
  #[must_use]
  pub fn glyph_runs(&self) -> Vec<GlyphRun> {
    let mut runs = Vec::new();

    for line in self.inner.layout.lines() {
      for item in line.items() {
        let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
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
              y: baseline - glyph.y,
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

  /// The family of each run's font, with the bytes of the text it draws, in visual order:
  /// what a test of fallback reads.
  #[must_use]
  pub fn run_families(&self) -> Vec<(Range<usize>, String)> {
    let mut families = Vec::new();

    for line in self.inner.layout.lines() {
      for item in line.items() {
        if let PositionedLayoutItem::GlyphRun(glyphs) = item {
          let run = glyphs.run();
          let font = run.font();
          let family = FontRef::from_index(font.data.data(), font.index)
            .ok()
            .and_then(|font| {
              font
                .localized_strings(skrifa::string::StringId::FAMILY_NAME)
                .english_or_first()
                .map(|name| name.to_string())
            })
            .unwrap_or_default();

          families.push((run.text_range(), family));
        }
      }
    }

    families
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
    let cursor = self.cursor(position);
    let caret = |cursor: Cursor| {
      let rect = cursor.geometry(layout, 1.);
      let right_to_left = match cursor.affinity() {
        ParleyAffinity::Upstream => cursor.upstream_cluster(layout),
        ParleyAffinity::Downstream => cursor.downstream_cluster(layout),
      }
      .is_some_and(|cluster| cluster.is_rtl());

      #[expect(clippy::cast_possible_truncation, reason = "layout coordinates")]
      Caret {
        x: rect.x0 as f32,
        y: rect.y0 as f32,
        height: (rect.y1 - rect.y0) as f32,
        right_to_left,
      }
    };
    let primary = caret(cursor);
    let other = Cursor::from_byte_index(
      layout,
      position.index,
      match position.affinity {
        Affinity::Downstream => ParleyAffinity::Upstream,
        Affinity::Upstream => ParleyAffinity::Downstream,
      },
    );
    let secondary = caret(other);
    let same_line = (secondary.y - primary.y).abs() < 0.5;

    Carets {
      primary,
      secondary: (same_line && (secondary.x - primary.x).abs() > 0.5).then_some(secondary),
    }
  }

  fn range_rects(&self, range: Range<usize>) -> Vec<Rect> {
    let layout = &self.inner.layout;
    let selection = Selection::new(
      Cursor::from_byte_index(layout, range.start, ParleyAffinity::Downstream),
      Cursor::from_byte_index(layout, range.end, ParleyAffinity::Upstream),
    );

    #[expect(clippy::cast_possible_truncation, reason = "layout coordinates")]
    selection
      .geometry(layout)
      .into_iter()
      .map(|(rect, _)| Rect {
        x: rect.x0 as f32,
        y: rect.y0 as f32,
        width: (rect.x1 - rect.x0) as f32,
        height: (rect.y1 - rect.y0) as f32,
      })
      .collect()
  }

  fn run_extents(&self) -> Vec<RunExtent> {
    let paints = self.inner.paints.borrow();
    let mut extents: Vec<RunExtent> = Vec::new();

    for (index, line) in self.inner.layout.lines().enumerate() {
      for item in line.items() {
        let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
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
                && (last.x + last.width - x).abs() < 0.01 =>
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
    let current = self
      .inner
      .lines
      .iter()
      .position(|line| {
        carets.primary.y + carets.primary.height / 2. >= line.top
          && carets.primary.y + carets.primary.height / 2. < line.top + line.height
      })
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

impl ParleyLaid {
  /// The text's index.
  #[must_use]
  pub fn index(&self) -> &TextIndex {
    &self.inner.index
  }
}
