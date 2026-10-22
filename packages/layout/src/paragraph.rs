//! A paragraph: inline runs laid out as one text by Parley, wrapping across them, each
//! shaped in its typeface, and the boxes laid out among them.

use std::{
  cell::{Cell, OnceCell, RefCell},
  ops::Range,
  sync::{Arc, OnceLock},
};

use parley::{
  FontContext,
  FontFamily,
  InlineBox,
  InlineBoxKind,
  Layout,
  LayoutContext,
  LineHeight,
  OverflowWrap,
  PositionedLayoutItem,
  StyleProperty,
  TabSize,
  VerticalAlign,
  fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, SourceCache},
};
use skene_foundation::{NodeId, Point, Rect, Size};
use skrifa::{FontRef, MetadataProvider, instance::LocationRef};
use taffy::prelude::AvailableSpace;
use unicode_segmentation::UnicodeSegmentation;

use crate::Leaf;

/// A typeface a paragraph's text is shaped in: an OpenType font's data and the family
/// it is known by, each family one typeface.
pub struct Typeface {
  family: &'static str,
  data: &'static [u8],
  cap: OnceLock<f32>,
}

impl Typeface {
  /// The typeface `data` holds, known by `family`.
  #[inline(always)]
  #[must_use]
  pub const fn new(family: &'static str, data: &'static [u8]) -> Self {
    Self {
      family,
      data,
      cap: OnceLock::new(),
    }
  }

  /// The family the typeface is known by.
  #[inline(always)]
  #[must_use]
  pub const fn family(&self) -> &'static str {
    self.family
  }

  /// How far above the baseline the typeface's capital letters reach at a font size of
  /// one: its cap height, or seven tenths of its ascent where it states none.
  #[must_use]
  pub fn cap_height(&self) -> f32 {
    *self.cap.get_or_init(|| {
      FontRef::new(self.data).map_or(0.7, |font| {
        let metrics = font.metrics(skrifa::instance::Size::unscaled(), LocationRef::default());

        metrics.cap_height.unwrap_or(metrics.ascent * 0.7) / f32::from(metrics.units_per_em)
      })
    })
  }
}

impl std::fmt::Debug for Typeface {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Typeface")
      .field("family", &self.family)
      .finish_non_exhaustive()
  }
}

impl PartialEq for Typeface {
  #[inline(always)]
  fn eq(&self, other: &Self) -> bool {
    self.family == other.family
  }
}

impl Eq for Typeface {}

/// How a run of a paragraph shapes and measures its text: its typeface at a font size,
/// the height of its line box and the spacing past each cluster.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
  /// The typeface.
  pub typeface: &'static Typeface,
  /// The font size.
  pub size: f32,
  /// The height of the run's line box, its glyphs centered in it as CSS leads a line.
  pub line_height: f32,
  /// The spacing past each cluster.
  pub letter_spacing: f32,
}

/// One run of a paragraph: the node `N` it lays out, the groups around it, what it flows
/// and the style it shapes its text in.
#[derive(Debug, Clone, PartialEq)]
pub struct Inline<N = NodeId> {
  /// The run's node.
  pub node: N,
  /// The groups around the run within the paragraph, outermost first, each laid out
  /// where its runs are.
  pub groups: Vec<N>,
  /// What the run flows among the paragraph's text.
  pub content: Content,
  /// The style the run shapes its text in.
  pub style: TextStyle,
}

/// What a run of a paragraph flows among its text.
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
  /// Text, wrapped between its words.
  Text(String),
  /// A box laid out on its own at the size it holds, which its tree sets before the
  /// paragraph measures, flowing as one piece that a line may end before or after.
  Box(Size<f32>),
}

/// Inline runs laid out as one text by Parley, each run shaped in its style, its node an
/// `N`, each line's room at least the root style's, as CSS's strut gives it.
///
/// The text keeps its blanks and newlines and wraps as CSS's `white-space: pre-wrap`
/// with `overflow-wrap: break-word` wraps it: where the Unicode line breaking algorithm
/// allows, at each newline and around each box, a word too wide for a line of its own
/// broken where the line fills (never in its min-content width). The blanks a line ends
/// in hang past its end where it wraps, and past the line's limit alone before a newline
/// or at the text's end. A newline ending the text opens a line, and an empty text lays
/// out one. A tab advances to the next of the stops its tab size sets, eight blanks apart
/// unless set, as CSS's `tab-size` sets them.
///
/// Each run's line box sits on its line's baseline, its glyphs centered in it, a box's
/// bottom on the baseline, and a line spans the highest of them above the baseline to
/// the deepest below it.
///
/// The text is shaped once, and its size at its widest and its narrowest measured once,
/// until a box it holds is sized afresh.
#[derive(Debug, Clone)]
#[must_use]
pub struct Paragraph<N = NodeId> {
  root: TextStyle,
  runs: Vec<Inline<N>>,
  trimmed: bool,
  tab_size: TabSize,
  shaped: OnceCell<Box<Shaped>>,
  widest: OnceCell<Size<f32>>,
  narrowest: OnceCell<Size<f32>>,
}

impl<N: PartialEq> PartialEq for Paragraph<N> {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
      && self.runs == other.runs
      && self.trimmed == other.trimmed
      && self.tab_size == other.tab_size
  }
}

/// One line a paragraph wraps into.
#[derive(Debug, Clone, PartialEq)]
pub struct WrappedLine {
  /// The clusters on the line, the newline ending it and the blanks it ends in included.
  pub clusters: Range<usize>,
  /// The clusters the line shows: without the newline ending it, and without the blanks
  /// it ends in that hang past its end.
  pub shown: Range<usize>,
  /// How tall the line is.
  pub height: f32,
  /// How far below the line's top its baseline lies.
  pub baseline: f32,
}

/// One stretch of a run's glyphs on a line, where its paragraph's layout places it.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphRun<N = NodeId> {
  /// The node of the run it belongs to.
  pub node: N,
  /// The run's font size.
  pub size: f32,
  /// Where the stretch starts on its baseline.
  pub origin: Point<f32>,
  /// How far the stretch reaches along its line, short of the blanks hanging past it.
  pub width: f32,
  /// How far above the baseline the typeface's ascent reaches.
  pub ascent: f32,
  /// The typeface's underline.
  pub underline: Stroke,
  /// The typeface's strikethrough.
  pub strikethrough: Stroke,
  /// The glyphs, each where it is drawn.
  pub glyphs: Vec<Glyph>,
}

/// One glyph of a run, where it is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
  /// The glyph's identifier in its typeface.
  pub id: u32,
  /// Where its origin lies.
  pub origin: Point<f32>,
}

/// A line a typeface draws across its text: its top's height above the baseline, and its
/// thickness.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
  /// The top's height above the baseline; negative below it.
  pub offset: f32,
  /// The thickness.
  pub thickness: f32,
}

/// One thing a paragraph draws, in the order it draws them.
#[derive(Debug, Clone, PartialEq)]
pub enum TextItem<N = NodeId> {
  /// A stretch of a run's glyphs.
  Glyphs(GlyphRun<N>),
  /// An inline box, which its node draws at its frame.
  Box(N),
}

/// The paragraph's text as Parley laid it out, whether it broke its lines and the limit
/// it last broke them within, and where each of its clusters lies.
#[derive(Clone)]
struct Shaped {
  layout: RefCell<Layout<usize>>,
  broken: Cell<bool>,
  limit: Cell<Option<f32>>,
  clusters: Vec<Cluster>,
}

impl std::fmt::Debug for Shaped {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Shaped")
      .field("broken", &self.broken)
      .field("limit", &self.limit)
      .field("clusters", &self.clusters)
      .finish_non_exhaustive()
  }
}

/// One cluster of a paragraph's text, or one of its boxes: the run it belongs to, its
/// bytes in the run's text, and where it starts in the text Parley lays out.
#[derive(Debug, Clone)]
struct Cluster {
  run: usize,
  bytes: Range<usize>,
  at: usize,
}

/// A line of a paragraph as Parley laid it out: the clusters it holds and shows, its top,
/// height and baseline, how far its highest cap height rises above that baseline, and
/// the stretches of each run on it.
struct Line {
  clusters: Range<usize>,
  shown: Range<usize>,
  top: f32,
  height: f32,
  baseline: f32,
  cap: f32,
  pieces: Vec<Piece>,
}

/// A stretch of one run on a line: the run's index and the rectangle it takes, relative
/// to the paragraph's top left.
struct Piece {
  run: usize,
  rect: Rect<f32>,
}

thread_local! {
  static ENGINE: RefCell<Engine> = RefCell::new(Engine::default());
}

/// The contexts Parley shapes and lays text out in: the fonts of each typeface a root
/// style names, and the layout context they share.
#[derive(Default)]
struct Engine {
  fonts: Vec<Fonts>,
  layouts: LayoutContext<usize>,
}

/// The font context of the paragraphs whose root style is in one typeface: the typefaces
/// their runs shape in and nothing else.
struct Fonts {
  root: &'static str,
  context: FontContext,
  registered: Vec<&'static str>,
}

impl Fonts {
  /// The font context among `fonts` of paragraphs whose root style is in `root`, made
  /// where none is, holding each of `typefaces`.
  fn find<'a>(
    fonts: &'a mut Vec<Self>,
    root: &Typeface,
    typefaces: &[&'static Typeface],
  ) -> &'a mut FontContext {
    let index = fonts
      .iter()
      .position(|fonts| fonts.root == root.family)
      .unwrap_or_else(|| {
        fonts.push(Self {
          root: root.family,
          context: FontContext {
            collection: Collection::new(CollectionOptions {
              shared: false,
              system_fonts: false,
            }),
            source_cache: SourceCache::default(),
          },
          registered: Vec::new(),
        });

        fonts.len() - 1
      });
    let found = &mut fonts[index];

    for typeface in typefaces {
      if !found.registered.contains(&typeface.family) {
        found.context.collection.register_fonts(
          Blob::new(Arc::new(typeface.data)),
          Some(FontInfoOverride {
            family_name: Some(typeface.family),
            ..Default::default()
          }),
        );
        found.registered.push(typeface.family);
      }
    }

    &mut found.context
  }
}

impl<N: Copy> Paragraph<N> {
  /// The paragraph of `runs`, in order, each line at least as tall as `root`'s line box.
  pub const fn new(root: TextStyle, runs: Vec<Inline<N>>) -> Self {
    Self {
      root,
      runs,
      trimmed: false,
      tab_size: TabSize::Spaces(8.),
      shaped: OnceCell::new(),
      widest: OnceCell::new(),
      narrowest: OnceCell::new(),
    }
  }

  /// The paragraph with its first line trimmed to its highest cap height, the root
  /// style's where the line holds no run, and its last line to its baseline, as CSS's
  /// `text-box-trim: trim-both` with `text-box-edge: cap alphabetic` trims a block's text.
  pub fn trimmed(mut self) -> Self {
    self.trimmed = true;
    self.widest.take();
    self.narrowest.take();

    self
  }

  /// The paragraph with its tab stops `tab_size` apart.
  pub fn set_tab_size(mut self, tab_size: TabSize) -> Self {
    self.tab_size = tab_size;
    self.shaped.take();
    self.widest.take();
    self.narrowest.take();

    self
  }

  /// How wide and tall the paragraph lays out wrapped within `width`, or along its
  /// newlines alone without one.
  pub fn size_within(&self, width: Option<f32>) -> Size<f32> {
    let widest = *self.widest.get_or_init(|| self.size_of(None));

    match width {
      Some(width) if width < widest.width => self.size_of(Some(width)),
      _ => widest,
    }
  }

  /// How wide and tall the paragraph lays out at its narrowest: as wide as its widest
  /// word, no word broken, and wrapped there, as CSS lays out a min-content size.
  pub fn narrowest(&self) -> Size<f32> {
    *self.narrowest.get_or_init(|| {
      let widths = self.shaped().layout.borrow().calculate_content_widths();

      self.size_within(Some(widths.min))
    })
  }

  /// The lines the paragraph wraps into within `width`, or along its newlines alone
  /// without one.
  #[must_use]
  pub fn lines_within(&self, width: Option<f32>) -> Vec<WrappedLine> {
    self
      .lines(width)
      .into_iter()
      .map(|line| WrappedLine {
        clusters: line.clusters,
        shown: line.shown,
        height: line.height,
        baseline: line.baseline,
      })
      .collect()
  }

  /// The text of the clusters `range`, a box's as the object replacement character.
  #[must_use]
  pub fn text(&self, range: Range<usize>) -> String {
    self.shaped().clusters[range]
      .iter()
      .map(|cluster| match &self.runs[cluster.run].content {
        Content::Text(text) => &text[cluster.bytes.clone()],
        Content::Box(_) => OBJECT_TEXT,
      })
      .collect()
  }

  /// What the paragraph draws laid out in a content box at `content`, in order: each
  /// stretch of a run's glyphs, and each inline box.
  #[must_use]
  pub fn items(&self, content: Rect<f32>) -> Vec<TextItem<N>> {
    let shaped = self.shaped();

    self.break_at(Some(content.size.width));

    let layout = shaped.layout.borrow();
    let origin = Point::new(
      content.origin.x,
      content.origin.y - self.shift(&self.lines_of(&layout)),
    );
    let mut items = Vec::new();

    for line in layout.lines() {
      let metrics = line.metrics();
      let visible = metrics.advance - metrics.hanging_advance;

      for item in line.items() {
        match item {
          PositionedLayoutItem::GlyphRun(glyphs) => {
            let Some(Inline {
              node,
              content: Content::Text(_),
              ..
            }) = self.runs.get(glyphs.style().brush)
            else {
              continue;
            };
            let run = glyphs.run();
            let font = run.font_metrics();

            items.push(TextItem::Glyphs(GlyphRun {
              node: *node,
              size: run.font_size(),
              origin: Point::new(origin.x + glyphs.offset(), origin.y + glyphs.baseline()),
              width: (visible - glyphs.offset()).clamp(0., glyphs.advance()),
              ascent: font.ascent,
              underline: Stroke {
                offset: font.underline_offset,
                thickness: font.underline_size,
              },
              strikethrough: Stroke {
                offset: font.strikethrough_offset,
                thickness: font.strikethrough_size,
              },
              glyphs: glyphs
                .positioned_glyphs()
                .map(|glyph| Glyph {
                  id: glyph.id,
                  origin: Point::new(origin.x + glyph.x, origin.y + glyph.y),
                })
                .collect(),
            }));
          }
          PositionedLayoutItem::InlineBox(inline) => {
            if let Some(run) = usize::try_from(inline.id)
              .ok()
              .and_then(|index| self.runs.get(index))
            {
              items.push(TextItem::Box(run.node));
            }
          }
        }
      }
    }

    items
  }

  /// How wide and tall the paragraph lays out wrapped within `limit`.
  fn size_of(&self, limit: Option<f32>) -> Size<f32> {
    let shaped = self.shaped();

    self.break_at(limit);

    let layout = shaped.layout.borrow();
    let lines = self.lines_of(&layout);
    let (top, bottom) = self.trims(&lines);
    let height = match (lines.first(), lines.last()) {
      (Some(first), Some(last)) => last.top + last.height - first.top,
      _ => 0.,
    };

    Size::new(layout.width(), height - top - bottom)
  }

  /// The lines the paragraph wraps into within `limit`.
  fn lines(&self, limit: Option<f32>) -> Vec<Line> {
    let shaped = self.shaped();

    self.break_at(limit);

    self.lines_of(&shaped.layout.borrow())
  }

  /// Breaks the paragraph's lines within `limit`, unless they already are.
  fn break_at(&self, limit: Option<f32>) {
    let shaped = self.shaped();

    if !shaped.broken.get() || shaped.limit.get() != limit {
      shaped.layout.borrow_mut().break_all_lines(limit);
      shaped.broken.set(true);
      shaped.limit.set(limit);
    }
  }

  /// How far the first of `lines` is trimmed at its top and the last at its bottom.
  fn trims(&self, lines: &[Line]) -> (f32, f32) {
    match (self.trimmed, lines.first(), lines.last()) {
      (true, Some(first), Some(last)) => (
        (first.baseline - first.cap).max(0.),
        last.height - last.baseline,
      ),
      _ => (0., 0.),
    }
  }

  /// How far above a content box's top the layout of `lines` starts: its first line's
  /// top, trimmed.
  fn shift(&self, lines: &[Line]) -> f32 {
    lines.first().map_or(0., |first| first.top) + self.trims(lines).0
  }

  /// The lines of `layout`, broken as it is: the clusters each holds and shows, where it
  /// lies, how far its highest cap height rises, and the stretches of each run on it,
  /// relative to the layout's top left.
  fn lines_of(&self, layout: &Layout<usize>) -> Vec<Line> {
    let clusters = &self.shaped().clusters;
    let mut lines: Vec<Line> = Vec::new();
    let mut hidden = Vec::new();

    for line in layout.lines() {
      let metrics = line.metrics();
      let visible = metrics.advance - metrics.hanging_advance;
      let text = line.text_range();
      let mut start = clusters.partition_point(|cluster| {
        cluster.at < text.start || (cluster.at == text.start && self.is_box(cluster))
      });
      let mut pieces: Vec<Piece> = Vec::new();
      let mut cap: Option<f32> = None;

      for item in line.items() {
        let Some((run, rect, above)) = self.piece_of(&item, visible)
        else {
          continue;
        };

        if let PositionedLayoutItem::InlineBox(_) = item
          && let Some(index) = clusters.iter().position(|cluster| cluster.run == run)
        {
          start = start.min(index);
        }

        cap = Some(cap.map_or(above, |cap| cap.max(above)));

        match pieces.last_mut() {
          Some(piece) if piece.run == run => {
            let right =
              (piece.rect.origin.x + piece.rect.size.width).max(rect.origin.x + rect.size.width);

            piece.rect.size.width = right - piece.rect.origin.x;
          }
          _ => pieces.push(Piece { run, rect }),
        }
      }

      hidden.push(self.hidden_at_end(&line));
      lines.push(Line {
        clusters: start..start,
        shown: start..start,
        top: metrics.block_min_coord,
        height: metrics.block_max_coord - metrics.block_min_coord,
        baseline: metrics.baseline - metrics.block_min_coord,
        cap: cap.unwrap_or_else(|| self.root.typeface.cap_height() * self.root.size),
        pieces,
      });
    }

    for index in 0..lines.len() {
      let start = lines[index].clusters.start;
      let end = lines
        .get(index + 1)
        .map_or(clusters.len(), |next| next.clusters.start)
        .max(start);
      let line = &mut lines[index];

      line.clusters = start..end;
      line.shown = start..end.saturating_sub(hidden[index]).max(start);
    }

    lines
  }

  /// The run `item` on a line belongs to, the rectangle it takes short of the blanks
  /// hanging past `visible`, its run's line box for a stretch of glyphs, and how far its
  /// cap height rises above its baseline.
  fn piece_of(
    &self,
    item: &PositionedLayoutItem<'_, usize>,
    visible: f32,
  ) -> Option<(usize, Rect<f32>, f32)> {
    match item {
      PositionedLayoutItem::GlyphRun(glyphs) => {
        let run = glyphs.style().brush;
        let style = self.runs.get(run)?.style;
        let font = glyphs.run().font_metrics();
        let ascent = font.ascent + (style.line_height - font.ascent - font.descent) / 2.;
        let left = glyphs.offset().min(visible);
        let right = (glyphs.offset() + glyphs.advance()).min(visible);

        Some((
          run,
          Rect {
            origin: Point::new(left, glyphs.baseline() - ascent),
            size: Size::new(right - left, style.line_height),
          },
          style.typeface.cap_height() * style.size,
        ))
      }
      PositionedLayoutItem::InlineBox(inline) => Some((
        usize::try_from(inline.id).ok()?,
        Rect {
          origin: Point::new(inline.x, inline.y),
          size: Size::new(inline.width, inline.height),
        },
        inline.height,
      )),
    }
  }

  /// How many clusters `line` ends in that it does not show: the newline ending it, and
  /// the blanks before that hanging past its end.
  fn hidden_at_end(&self, line: &parley::Line<'_, usize>) -> usize {
    let metrics = line.metrics();
    let visible = metrics.advance - metrics.hanging_advance;
    let mut ending: Vec<(usize, f32, bool)> = line
      .runs()
      .flat_map(|run| {
        run
          .clusters()
          .map(|cluster| {
            (
              cluster.text_range().start,
              cluster.advance(),
              cluster.is_hard_line_break(),
            )
          })
          .collect::<Vec<_>>()
      })
      .collect();
    let mut edge = metrics.advance;
    let mut hidden = 0;

    ending.sort_by_key(|(at, ..)| *at);

    for (at, advance, newline) in ending.into_iter().rev() {
      if newline {
        hidden += 1;
      }
      else if edge > visible + 1e-3 && self.is_blank(at) {
        hidden += 1;
        edge -= advance;
      }
      else {
        break;
      }
    }

    hidden
  }

  /// Whether `cluster` is one of the paragraph's boxes.
  fn is_box(&self, cluster: &Cluster) -> bool {
    matches!(self.runs[cluster.run].content, Content::Box(_))
  }

  /// Whether the cluster starting at the byte `at` of the text Parley lays out is a
  /// blank.
  fn is_blank(&self, at: usize) -> bool {
    let clusters = &self.shaped().clusters;
    let index = clusters.partition_point(|cluster| cluster.at < at);

    clusters
      .get(index)
      .is_some_and(|cluster| match &self.runs[cluster.run].content {
        Content::Text(text) => text[cluster.bytes.clone()]
          .chars()
          .all(|character| character.is_whitespace() && !matches!(character, '\n' | '\r')),
        Content::Box(_) => false,
      })
  }

  /// The paragraph shaped, once.
  fn shaped(&self) -> &Shaped {
    self.shaped.get_or_init(|| Box::new(self.shape()))
  }

  /// Shapes the paragraph's text through Parley, each run's text in its style and each
  /// box in place, and finds where each cluster starts in it.
  fn shape(&self) -> Shaped {
    let mut text = String::new();
    let mut clusters = Vec::new();
    let mut typefaces = vec![self.root.typeface];

    for (index, run) in self.runs.iter().enumerate() {
      if !typefaces.contains(&run.style.typeface) {
        typefaces.push(run.style.typeface);
      }

      match &run.content {
        Content::Text(content) => {
          for (byte, cluster) in content.grapheme_indices(true) {
            clusters.push(Cluster {
              run: index,
              bytes: byte..byte + cluster.len(),
              at: text.len() + byte,
            });
          }

          text.push_str(content);
        }
        Content::Box(_) => clusters.push(Cluster {
          run: index,
          bytes: 0..0,
          at: text.len(),
        }),
      }
    }

    let layout = ENGINE.with_borrow_mut(|engine| {
      let Engine { fonts, layouts } = engine;
      let fonts = Fonts::find(fonts, self.root.typeface, &typefaces);
      let mut builder = layouts.ranged_builder(fonts, &text, 1., false);
      let mut offset = 0;

      Self::push_style(&mut |property| builder.push_default(property), &self.root);
      builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::BreakWord));
      builder.push_default(StyleProperty::TabSize(self.tab_size));
      builder.push_default(StyleProperty::Brush(usize::MAX));

      for (index, run) in self.runs.iter().enumerate() {
        match &run.content {
          Content::Text(content) => {
            let range = offset..offset + content.len();

            Self::push_style(
              &mut |property| builder.push(property, range.clone()),
              &run.style,
            );
            builder.push(StyleProperty::Brush(index), range);
            offset += content.len();
          }
          Content::Box(size) => builder.push_inline_box(InlineBox {
            id: index as u64,
            kind: InlineBoxKind::InFlow,
            index: offset,
            width: size.width,
            height: size.height,
            baseline: None,
            vertical_align: VerticalAlign::BASELINE,
          }),
        }
      }

      builder.build(&text)
    });

    Shaped {
      layout: RefCell::new(layout),
      broken: Cell::new(false),
      limit: Cell::new(None),
      clusters,
    }
  }

  /// Pushes each property of `style` through `push`.
  fn push_style(push: &mut dyn FnMut(StyleProperty<'static, usize>), style: &TextStyle) {
    push(StyleProperty::FontFamily(FontFamily::named(
      style.typeface.family,
    )));
    push(StyleProperty::FontSize(style.size));
    push(StyleProperty::LineHeight(LineHeight::Absolute(
      style.line_height,
    )));
    push(StyleProperty::LetterSpacing(style.letter_spacing));
  }
}

/// The character a box stands for in its paragraph's text.
const OBJECT_TEXT: &str = "\u{fffc}";

impl Leaf for Paragraph {
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    let size = match known.width {
      Some(width) => self.size_within(Some(width)),
      None => match available.width {
        AvailableSpace::Definite(width) => self.size_within(Some(width)),
        AvailableSpace::MinContent => self.narrowest(),
        AvailableSpace::MaxContent => self.size_within(None),
      },
    };

    Size::new(
      known.width.unwrap_or(size.width),
      known.height.unwrap_or(size.height),
    )
  }

  fn place(&self, content: Rect<f32>) -> Vec<(NodeId, Rect<f32>)> {
    let lines = self.lines(Some(content.size.width));
    let origin = Point::new(content.origin.x, content.origin.y - self.shift(&lines));
    let mut placed = Vec::new();

    for line in &lines {
      for piece in &line.pieces {
        let Some(run) = self.runs.get(piece.run)
        else {
          continue;
        };
        let rect = Rect {
          origin: Point::new(
            origin.x + piece.rect.origin.x,
            origin.y + piece.rect.origin.y,
          ),
          size: piece.rect.size,
        };

        placed.extend(run.groups.iter().map(|group| (*group, rect)));
        placed.push((run.node, rect));
      }
    }

    placed
  }

  fn boxes(&self) -> Vec<NodeId> {
    self
      .runs
      .iter()
      .filter(|run| matches!(run.content, Content::Box(_)))
      .map(|run| run.node)
      .collect()
  }

  fn size_box(&mut self, node: NodeId, size: Size<f32>) {
    for (index, run) in self.runs.iter_mut().enumerate() {
      if run.node == node && matches!(run.content, Content::Box(_)) {
        run.content = Content::Box(size);

        if let Some(shaped) = self.shaped.get_mut() {
          for inline in shaped.layout.get_mut().inline_boxes_mut() {
            if inline.id == index as u64 {
              inline.width = size.width;
              inline.height = size.height;
            }
          }

          shaped.broken.set(false);
        }
      }
    }

    self.widest.take();
    self.narrowest.take();
  }
}
