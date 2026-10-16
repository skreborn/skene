//! Parley's layout of a variant: shaping once, breaking at the variant's width, the marks
//! Parley leaves to its caller (a soft hyphen's hyphen, the ellipsis of a cut text), and the
//! record and glyph runs of the result.
//!
//! The breaking around marks is the drawn Linux prototype's (`spikes/linux/parley`): a line
//! that ends at a soft hyphen, or a cut text's last line, is broken again with room for the
//! glyph it draws.

use std::borrow::Cow;

use parley::{
  Alignment as ParleyAlignment,
  AlignmentOptions,
  BaseDirection,
  BreakReason,
  FontData,
  FontFamily,
  FontFamilyName,
  FontStyle,
  FontWeight,
  GenericFamily,
  Layout,
  LayoutContext,
  LineBreak as ParleyLineBreak,
  LineHeight,
  OverflowWrap,
  PositionedLayoutItem,
  StyleProperty,
  TextWrapMode,
  WordBreak as ParleyWordBreak,
  fontique::{Language, Synthesis},
};
use skrifa::{
  FontRef,
  MetadataProvider as _,
  instance::{LocationRef, NormalizedCoord, Size},
  raw::types::F2Dot14,
};

use crate::{
  corpus::{Alignment, Direction, Hyphenation, LineBreak, Variant, WordBreak},
  fonts::Fonts,
  record::{Cluster, GlyphRecord, GlyphRunRecord, Intrinsic, Line, Range, Record, Run, Variation},
  text::{Offsets, is_invisible},
};

/// How Parley is asked to lay out, beyond what a case says.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Options {
  /// Round line metrics to whole logical pixels (Parley's `quantize`).
  pub quantize: bool,
  /// Leave emoji-presentation clusters to Parley's own fallback, the emoji family after the
  /// run's families, rather than asking the emoji family first ([`crate::emoji`]).
  pub plain_emoji: bool,
}

/// Parley with its contexts, over a font collection.
pub struct ParleyEngine {
  /// The fonts.
  pub fonts: Fonts,
  layouts: LayoutContext<()>,
  options: Options,
}

/// A variant shaped and not yet broken.
pub struct Shaped {
  layout: Layout<()>,
  text: String,
  max_lines: Option<usize>,
  ellipsis: bool,
  alignment: Alignment,
  notes: Vec<String>,
}

/// A glyph Parley leaves to its caller: the hyphen at a soft hyphen's break, or an ellipsis.
#[derive(Clone, Debug)]
pub struct Mark {
  /// The font.
  pub font: FontData,
  /// The font size.
  pub size: f32,
  /// The variation coordinates, normalized.
  pub coords: Vec<i16>,
  /// The run's synthesis.
  pub synthesis: Synthesis,
  /// The glyphs and their pen positions.
  pub glyphs: Vec<(u32, f32)>,
  /// Their advance in all.
  pub advance: f32,
  /// The baseline.
  pub baseline: f32,
  /// The byte it stands for.
  pub at: usize,
  /// The line it ends.
  pub line: usize,
  /// Whether it is the ellipsis.
  pub ellipsis: bool,
}

/// A variant laid out at its width.
pub struct Laid {
  /// Parley's layout.
  pub layout: Layout<()>,
  /// The text.
  pub text: String,
  /// The marks drawn beyond the text.
  pub marks: Vec<Mark>,
  /// Whether `max_lines` cut text off.
  pub truncated: bool,
  /// What Parley could not honour.
  pub notes: Vec<String>,
}

/// One glyph, positioned in the layout.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PositionedGlyph {
  /// The glyph's id.
  pub id: u32,
  /// Its origin's x.
  pub x: f32,
  /// Its baseline's y.
  pub y: f32,
  /// The byte where its cluster starts.
  pub cluster: usize,
}

/// A run of glyphs in one font, as a renderer draws it.
#[derive(Clone, Debug)]
pub struct GlyphRun {
  /// The line it is on.
  pub line: usize,
  /// The font.
  pub font: FontData,
  /// The font size.
  pub size: f32,
  /// The variation coordinates, normalized (F2Dot14 bits).
  pub coords: Vec<i16>,
  /// The synthesis fontique suggests: embolden, skew, variation settings.
  pub synthesis: Synthesis,
  /// The run's baseline, from the layout's top: what a synthetic slant shears about.
  pub baseline: f32,
  /// The glyphs.
  pub glyphs: Vec<PositionedGlyph>,
}

/// The family list a case's `family` resolves to.
fn families(family: &str) -> Vec<FontFamilyName<'static>> {
  match family {
    "system" => vec![
      FontFamilyName::Generic(GenericFamily::SystemUi),
      FontFamilyName::Generic(GenericFamily::SansSerif),
    ],
    "serif" => vec![FontFamilyName::Generic(GenericFamily::Serif)],
    "monospace" => vec![FontFamilyName::Generic(GenericFamily::Monospace)],
    named => vec![
      FontFamilyName::Named(Cow::Owned(named.to_owned())),
      FontFamilyName::Generic(GenericFamily::SystemUi),
    ],
  }
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

/// The advance of the blanks (and format characters) that end `line` logically: what its
/// width leaves out.
fn trailing_blank(text: &str, line: &parley::Line<'_, ()>) -> f32 {
  let mut clusters: Vec<(std::ops::Range<usize>, f32)> = line
    .runs()
    .flat_map(|run| {
      run
        .clusters()
        .map(|cluster| (cluster.text_range(), cluster.advance()))
        .collect::<Vec<_>>()
    })
    .collect();
  let mut blank = 0.;

  clusters.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));

  for (range, advance) in clusters {
    let end = range.end.min(text.len());

    if !text[range.start.min(end)..end].chars().all(is_invisible) {
      break;
    }

    blank += advance;
  }

  blank
}

impl ParleyEngine {
  /// The engine over `fonts`.
  #[must_use]
  pub fn new(fonts: Fonts, options: Options) -> Self {
    Self {
      fonts,
      layouts: LayoutContext::new(),
      options,
    }
  }

  /// Shapes `variant`'s text in its case's style.
  pub fn shape(&mut self, variant: &Variant<'_>) -> Shaped {
    self.shape_with(variant, None)
  }

  /// Shapes `variant`'s text, a break allowed between any two grapheme clusters from byte
  /// `anywhere_from` on: the last visible line of a cut text, which an ellipsis ends mid-word
  /// as TextKit's tail truncation, CSS's `text-overflow` and Android's `TextUtils.ellipsize` do.
  fn shape_with(&mut self, variant: &Variant<'_>, anywhere_from: Option<usize>) -> Shaped {
    let case = variant.case;
    let mut notes = Vec::new();
    let mut builder = self.layouts.ranged_builder(
      &mut self.fonts.context,
      &case.text,
      1.,
      self.options.quantize,
    );

    builder.push_default(StyleProperty::FontFamily(FontFamily::List(Cow::Owned(
      families(&case.family),
    ))));
    builder.push_default(StyleProperty::FontSize(variant.size));
    builder.push_default(StyleProperty::FontWeight(FontWeight::new(f32::from(
      case.weight,
    ))));
    builder.push_default(StyleProperty::FontStyle(if case.italic {
      FontStyle::Italic
    }
    else {
      FontStyle::Normal
    }));
    builder.push_default(StyleProperty::LineHeight(LineHeight::MetricsRelative(1.)));
    builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::BreakWord));
    builder.push_default(StyleProperty::TextWrapMode(TextWrapMode::Wrap));

    match Language::parse(&case.locale) {
      Ok(language) => builder.push_default(StyleProperty::Locale(Some(language))),
      Err(_) => notes.push(format!("locale {:?} does not parse", case.locale)),
    }

    builder.push_default(StyleProperty::LineBreak(match case.line_break {
      LineBreak::Normal => ParleyLineBreak::Normal,
      LineBreak::Strict => ParleyLineBreak::Strict,
      LineBreak::Loose => ParleyLineBreak::Loose,
      LineBreak::Anywhere => ParleyLineBreak::Anywhere,
    }));
    builder.push_default(StyleProperty::WordBreak(match case.word_break {
      WordBreak::Normal => ParleyWordBreak::Normal,
      WordBreak::BreakAll => ParleyWordBreak::BreakAll,
      WordBreak::KeepAll => ParleyWordBreak::KeepAll,
      WordBreak::Phrase => {
        notes.push("word_break phrase unsupported: broken as normal".to_owned());

        ParleyWordBreak::Normal
      }
    }));

    if case.hyphenation == Hyphenation::Auto {
      notes.push("automatic hyphenation unsupported: soft hyphens only".to_owned());
    }

    if !self.options.plain_emoji {
      for range in crate::emoji::emoji_ranges(&case.text) {
        let mut emoji_first = vec![FontFamilyName::Generic(GenericFamily::Emoji)];

        emoji_first.extend(families(&case.family));
        builder.push(
          StyleProperty::FontFamily(FontFamily::List(Cow::Owned(emoji_first))),
          range,
        );
      }
    }

    if let Some(from) = anywhere_from {
      builder.push(
        StyleProperty::LineBreak(ParleyLineBreak::Anywhere),
        from..case.text.len(),
      );
    }

    builder.set_base_direction(match case.direction {
      Direction::Auto => BaseDirection::Auto,
      Direction::Ltr => BaseDirection::Ltr,
      Direction::Rtl => BaseDirection::Rtl,
    });

    Shaped {
      layout: builder.build(&case.text),
      text: case.text.clone(),
      max_lines: case.max_lines.map(|lines| lines.max(1) as usize),
      ellipsis: case.ellipsis,
      alignment: case.alignment,
      notes,
    }
  }

  /// Shapes and breaks `variant` at its width; a text cut with an ellipsis is shaped again so
  /// that its last line breaks at a grapheme cluster.
  pub fn lay_out(&mut self, variant: &Variant<'_>) -> Laid {
    let laid = self.shape(variant).break_at(variant.width);

    if !(laid.truncated && variant.case.ellipsis) {
      return laid;
    }

    let last = laid.layout.len().saturating_sub(1);
    let Some(tail) = laid.layout.get(last).map(|line| line.text_range().start)
    else {
      return laid;
    };
    // The lines above the last keep their breaks: each is given just the room it used.
    let caps: Vec<f32> = laid
      .layout
      .lines()
      .take(last)
      .map(|line| line.metrics().advance - line.metrics().hanging_advance + 0.01)
      .collect();
    let again = self
      .shape_with(variant, Some(tail))
      .break_capped(variant.width, &caps);
    let starts = |laid: &Laid| -> Vec<usize> {
      laid
        .layout
        .lines()
        .map(|line| line.text_range().start)
        .collect()
    };

    if starts(&again) == starts(&laid) {
      again
    }
    else {
      let mut laid = laid;

      laid
        .notes
        .push("ellipsis at a word: the grapheme cut moved an earlier break".to_owned());

      laid
    }
  }
}

impl Shaped {
  /// Min- and max-content widths.
  #[must_use]
  pub fn intrinsic(&self) -> Intrinsic {
    let widths = self.layout.calculate_content_widths();

    Intrinsic {
      min: widths.min,
      max: widths.max,
    }
  }

  /// The font, size, coordinates and synthesis of the run holding byte `at` on line `line`.
  fn run_font(
    layout: &Layout<()>,
    line: usize,
    at: usize,
  ) -> Option<(FontData, f32, Vec<i16>, Synthesis)> {
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
            run.synthesis(),
          ));
        }
      }
    }

    found
  }

  /// The visual left and right of line `line`'s content, trailing blanks left out.
  fn content_edges(text: &str, layout: &Layout<()>, line: usize) -> Option<(f32, f32)> {
    let line = layout.get(line)?;
    let metrics = *line.metrics();
    let blank = trailing_blank(text, &line);
    let left = metrics.offset + if layout.is_rtl() { blank } else { 0. };

    Some((left, left + metrics.advance - blank))
  }

  /// The mark of `glyphs` (each with its advance) ending line `line`, standing for byte `at`.
  fn mark(
    text: &str,
    layout: &Layout<()>,
    line: usize,
    at: usize,
    ellipsis: bool,
    pick: impl Fn(&FontData, &[i16], f32) -> Option<Vec<(u32, f32)>>,
  ) -> Option<Mark> {
    let (font, size, coords, synthesis) = Self::run_font(layout, line, at)?;
    let glyphs = pick(&font, &coords, size)?;
    let advance: f32 = glyphs.iter().map(|(_, advance)| advance).sum();
    let (left, right) = Self::content_edges(text, layout, line)?;
    let mut x = if layout.is_rtl() {
      left - advance
    }
    else {
      right
    };
    let placed = glyphs
      .into_iter()
      .map(|(glyph, advance)| {
        let at = x;

        x += advance;

        (glyph, at)
      })
      .collect();

    Some(Mark {
      font,
      size,
      coords,
      synthesis,
      glyphs: placed,
      advance,
      baseline: layout.get(line)?.metrics().baseline,
      at,
      line,
      ellipsis,
    })
  }

  /// The hyphen drawn after line `line` where it breaks at a soft hyphen.
  fn hyphen(&self, layout: &Layout<()>, line: usize) -> Option<Mark> {
    let info = layout.get(line)?;
    let range = info.text_range();

    if info.break_reason() != BreakReason::Regular || !self.text[range.clone()].ends_with('\u{AD}')
    {
      return None;
    }

    Self::mark(
      &self.text,
      layout,
      line,
      range.end - 2,
      false,
      |font, coords, size| {
        glyph_of(font, coords, size, '\u{2010}')
          .or_else(|| glyph_of(font, coords, size, '-'))
          .map(|glyph| vec![glyph])
      },
    )
  }

  /// The ellipsis ending line `line`.
  fn ellipsis_mark(&self, layout: &Layout<()>, line: usize) -> Option<Mark> {
    let info = layout.get(line)?;
    let end = info.text_range().end;
    let at = self.text[..end]
      .char_indices()
      .next_back()
      .map_or(0, |(at, _)| at);

    Self::mark(&self.text, layout, line, at, true, |font, coords, size| {
      glyph_of(font, coords, size, '\u{2026}')
        .map(|glyph| vec![glyph])
        .or_else(|| glyph_of(font, coords, size, '.').map(|dot| vec![dot; 3]))
    })
  }

  /// Breaks `layout` within `limit`, each line in `reserve` given that much less room and line
  /// `n` no more than `caps[n]`, the lines cut after `max`: whether text was left over.
  fn break_into(
    layout: &mut Layout<()>,
    limit: f32,
    (reserve, caps): (&[(usize, f32)], &[f32]),
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
        .map_or(limit, |(_, amount)| (limit - amount).max(0.))
        .min(caps.get(lines).copied().unwrap_or(f32::MAX));

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

  /// The layout at `width` (`None` is unconstrained), marks included.
  #[must_use]
  pub fn break_at(&self, width: Option<f32>) -> Laid {
    self.break_capped(width, &[])
  }

  /// The layout at `width`, line `n` given no more room than `caps[n]`.
  fn break_capped(&self, width: Option<f32>, caps: &[f32]) -> Laid {
    let limit = width.unwrap_or(f32::MAX);
    let mut layout = self.layout.clone();
    let mut reserve: Vec<(usize, f32)> = Vec::new();
    let mut left = false;

    for _ in 0..4 {
      left = Self::break_into(&mut layout, limit, (&reserve, caps), self.max_lines);

      let mut asked = false;

      for line in 0..layout.len() {
        if reserve.iter().any(|(at, _)| *at == line) {
          continue;
        }

        let mark = if left && self.ellipsis && Some(line + 1) == self.max_lines {
          self.ellipsis_mark(&layout, line)
        }
        else {
          self.hyphen(&layout, line)
        };
        let Some(mark) = mark
        else {
          continue;
        };
        let used = layout.get(line).map_or(0., |line| {
          line.metrics().advance - trailing_blank(&self.text, &line)
        });

        if used + mark.advance > limit {
          reserve.push((line, mark.advance));
          asked = true;

          break;
        }
      }

      if !asked {
        break;
      }
    }

    let truncated = left
      && self.max_lines.is_some_and(|max| layout.len() >= max)
      && layout
        .get(layout.len().saturating_sub(1))
        .is_some_and(|line| line.text_range().end < self.text.len());

    layout.align(
      match self.alignment {
        Alignment::Start => ParleyAlignment::Start,
        Alignment::Center => ParleyAlignment::Center,
        Alignment::End => ParleyAlignment::End,
        Alignment::Justify => ParleyAlignment::Justify,
      },
      AlignmentOptions::default(),
    );

    let mut marks = Vec::new();

    for line in 0..layout.len() {
      let mark = if truncated && self.ellipsis && line + 1 == layout.len() {
        self.ellipsis_mark(&layout, line)
      }
      else {
        self.hyphen(&layout, line)
      };

      marks.extend(mark);
    }

    Laid {
      layout,
      text: self.text.clone(),
      marks,
      truncated,
      notes: self.notes.clone(),
    }
  }
}

impl Laid {
  /// The glyphs to draw, run by run, the marks after the text's.
  #[must_use]
  pub fn glyph_runs(&self) -> Vec<GlyphRun> {
    let mut runs = Vec::new();

    for (index, line) in self.layout.lines().enumerate() {
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
              y: baseline - glyph.y,
              cluster: at,
            });
            x += glyph.advance;
          }
        }

        runs.push(GlyphRun {
          line: index,
          font: run.font().clone(),
          size: run.font_size(),
          coords: run
            .normalized_coords()
            .iter()
            .map(|coord| coord.to_bits())
            .collect(),
          synthesis: run.synthesis(),
          baseline,
          glyphs,
        });
      }
    }

    for mark in &self.marks {
      runs.push(GlyphRun {
        line: mark.line,
        font: mark.font.clone(),
        size: mark.size,
        coords: mark.coords.clone(),
        synthesis: mark.synthesis,
        baseline: mark.baseline,
        glyphs: mark
          .glyphs
          .iter()
          .map(|(id, x)| PositionedGlyph {
            id: *id,
            x: *x,
            y: mark.baseline,
            cluster: mark.at,
          })
          .collect(),
      });
    }

    runs
  }

  /// The layout's width: its widest line, marks included.
  #[must_use]
  pub fn width(&self) -> f32 {
    self
      .lines(&Offsets::new(&self.text))
      .iter()
      .map(|line| line.width)
      .fold(0., f32::max)
  }

  /// The lines as the record has them.
  #[must_use]
  pub fn lines(&self, offsets: &Offsets) -> Vec<Line> {
    let rtl = self.layout.is_rtl();

    self
      .layout
      .lines()
      .enumerate()
      .map(|(index, line)| {
        let metrics = line.metrics();
        let range = line.text_range();
        let marks: Vec<&Mark> = self
          .marks
          .iter()
          .filter(|mark| mark.line == index)
          .collect();
        let extra: f32 = marks.iter().map(|mark| mark.advance).sum();
        let blank = trailing_blank(&self.text, &line);
        let left = metrics.offset + if rtl { blank } else { 0. };
        let ascent = metrics.baseline - metrics.content_block_min_coord;
        let descent = metrics.content_block_max_coord - metrics.baseline;
        let height = metrics.block_max_coord - metrics.block_min_coord;

        let end = range.end.min(self.text.len());

        Line {
          range: Range::of([range.start.min(end), end], offsets),
          x: if rtl { left - extra } else { left },
          width: metrics.advance - blank + extra,
          top: metrics.block_min_coord,
          height,
          baseline: metrics.baseline,
          ascent,
          descent,
          leading: height - ascent - descent,
          hyphen: marks.iter().any(|mark| !mark.ellipsis),
          ellipsis: marks.iter().any(|mark| mark.ellipsis),
        }
      })
      .collect()
  }

  /// The record of this layout as `variant`, glyph runs included where `glyphs` asks.
  pub fn record(
    &self,
    variant: &Variant<'_>,
    fonts: &mut Fonts,
    intrinsic: Option<Intrinsic>,
    glyphs: bool,
  ) -> Record {
    let offsets = Offsets::new(&self.text);
    let lines = self.lines(&offsets);
    let mut runs = Vec::new();
    let mut clusters = Vec::new();

    for (index, line) in self.layout.lines().enumerate() {
      for item in line.items() {
        let PositionedLayoutItem::GlyphRun(glyph_run) = item
        else {
          continue;
        };
        let run = glyph_run.run();
        let identity = fonts.identity(run.font());
        let synthesis = run.synthesis();
        let range = run.text_range();
        let mut x = glyph_run.offset();

        runs.push(Run {
          range: Range::of([range.start, range.end], &offsets),
          line: index,
          family: Some(identity.family),
          postscript: Some(identity.postscript),
          path: identity.path,
          index: Some(identity.index),
          size: run.font_size(),
          weight: Some(run.font_attrs().weight.value()),
          italic: Some(run.font_attrs().style != parley::fontique::FontStyle::Normal),
          synthetic_bold: synthesis.embolden(),
          synthetic_italic: synthesis.skew().is_some(),
          rtl: run.is_rtl(),
        });

        for cluster in run.visual_clusters() {
          let range = cluster.text_range();

          clusters.push(Cluster {
            range: Range::of([range.start, range.end], &offsets),
            line: index,
            x,
            advance: cluster.advance(),
          });
          x += cluster.advance();
        }
      }
    }

    Record {
      variant: variant.id(),
      width: lines.iter().map(|line| line.width).fold(0., f32::max),
      height: self.layout.height(),
      lines,
      truncated: self.truncated,
      runs,
      clusters,
      intrinsic,
      timings: None,
      images: std::collections::BTreeMap::new(),
      glyph_runs: if glyphs {
        self.glyph_run_records(fonts)
      }
      else {
        Vec::new()
      },
      notes: self.notes.clone(),
    }
  }

  /// The glyph runs as records: what a platform rasterizer draws.
  pub fn glyph_run_records(&self, fonts: &mut Fonts) -> Vec<GlyphRunRecord> {
    self
      .glyph_runs()
      .into_iter()
      .map(|run| GlyphRunRecord {
        line: run.line,
        font: fonts.identity(&run.font),
        size: run.size,
        coords: run.coords,
        variations: run
          .synthesis
          .variation_settings()
          .iter()
          .map(|(tag, value)| Variation {
            tag: tag.to_string(),
            value: *value,
          })
          .collect(),
        embolden: run.synthesis.embolden(),
        skew: run.synthesis.skew(),
        glyphs: run
          .glyphs
          .iter()
          .map(|glyph| GlyphRecord {
            id: glyph.id,
            x: glyph.x,
            y: glyph.y,
            cluster: glyph.cluster,
          })
          .collect(),
      })
      .collect()
  }
}
