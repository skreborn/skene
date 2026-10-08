//! A paragraph: inline runs laid out as one text, wrapping across them, each measured by
//! its platform's metrics, and the boxes laid out among them.

use std::{cell::OnceCell, ops::Range};

use skene_foundation::{NodeId, Point, Rect, Size};
use taffy::prelude::AvailableSpace;
use unicode_segmentation::UnicodeSegmentation;

use crate::Leaf;

/// How a run of a paragraph measures its text.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not measure a paragraph's text",
  note = "a paragraph's run measures each cluster's advance and its line's height"
)]
pub trait Metrics {
  /// How far `cluster`, one extended grapheme cluster, advances its line.
  fn advance(&self, cluster: &str) -> f32;

  /// How tall a line holding the run's text is at least.
  fn line_height(&self) -> f32;

  /// How far below the top of a line box of the run's text its baseline lies; at its
  /// bottom by default.
  fn baseline(&self) -> f32 {
    self.line_height()
  }

  /// How far above its baseline the run's capital letters reach, which a trimmed
  /// paragraph's first line is trimmed to; to the top of its line box by default.
  fn cap(&self) -> f32 {
    self.baseline()
  }
}

/// One run of a paragraph: the node `N` it lays out, the groups around it, what it flows
/// and the metrics it measures its text by.
#[derive(Debug, Clone, PartialEq)]
pub struct Inline<M, N = NodeId> {
  /// The run's node.
  pub node: N,
  /// The groups around the run within the paragraph, outermost first, each laid out
  /// where its runs are.
  pub groups: Vec<N>,
  /// What the run flows among the paragraph's text.
  pub content: Content,
  /// What measures the run's text.
  pub metrics: M,
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

/// Inline runs laid out as one text, each run's clusters measured by its metrics `M`
/// and its node an `N`.
///
/// The text keeps its blanks and newlines and wraps as CSS's `white-space: pre-wrap`
/// with `overflow-wrap: break-word` wraps it in Chromium: where the Unicode line breaking
/// algorithm allows, as after the blanks ending a word or a hyphen, at each newline and
/// around each box, a word too wide for a line of its own broken where the line fills
/// (never in its min-content width); a word runs across runs where nothing breaks it. A
/// tab advances to the next stop past half a zero, the stops eight of its run's spaces
/// apart from the line's start. The blanks a line ends in take no room on it where it
/// wraps, and room up to the line's limit before a newline or at the text's end; a
/// newline ending the text opens no line, and an empty text has none.
///
/// Each run's line box sits on its line's baseline, a box's bottom on it, and a line
/// spans the highest of them above the baseline to the deepest below it.
///
/// The text's clusters and the places a line may end are found once, and its size at its
/// widest and its narrowest measured once, until a box it holds is sized afresh.
#[derive(Debug, Clone)]
#[must_use]
pub struct Paragraph<M, N = NodeId> {
  runs: Vec<Inline<M, N>>,
  trimmed: bool,
  shape: OnceCell<Shape>,
  widest: OnceCell<Size<f32>>,
  narrowest: OnceCell<Size<f32>>,
}

impl<M: PartialEq, N: PartialEq> PartialEq for Paragraph<M, N> {
  fn eq(&self, other: &Self) -> bool {
    self.runs == other.runs && self.trimmed == other.trimmed
  }
}

/// One line a paragraph wraps into.
#[derive(Debug, Clone, PartialEq)]
pub struct WrappedLine {
  /// The clusters on the line, the newline ending it and the blanks it ends in included.
  pub clusters: Range<usize>,
  /// The clusters the line shows: without the newline ending it, and without the blanks
  /// it ends in where it wraps or where they pass its width.
  pub shown: Range<usize>,
  /// How tall the line is.
  pub height: f32,
  /// How far below the line's top its baseline lies.
  pub baseline: f32,
}

/// One line of a paragraph: the stretches of each run on it, the clusters it holds and
/// shows, how wide its text is short of the blanks it ends in, how far it reaches above
/// its baseline and below it, and how far its highest cap height rises above it.
struct Line {
  pieces: Vec<Piece>,
  clusters: Range<usize>,
  shown: Range<usize>,
  width: f32,
  above: f32,
  below: f32,
  cap: f32,
}

impl Line {
  /// How tall the line is.
  fn height(&self) -> f32 {
    self.above + self.below
  }

  /// The line wrapping before the cluster at `end`, the blanks it ends in removed from
  /// its pieces, its text advancing `advance` with them.
  fn ended(mut self, end: usize, advance: f32) -> Self {
    let mut blanks = advance - self.width;

    self.clusters.end = end;

    for piece in self.pieces.iter_mut().rev() {
      let cut = blanks.min(piece.width);

      piece.width -= cut;
      blanks -= cut;

      if blanks <= 0. {
        break;
      }
    }

    self
  }

  /// The line ending before the cluster at `end` at a newline or the text's end.
  const fn closed(mut self, end: usize) -> Self {
    self.clusters.end = end;

    self
  }
}

/// A stretch of one run on a line: the run's index, where it starts along the line, and
/// how wide it is.
struct Piece {
  run: usize,
  start: f32,
  width: f32,
}

impl<M: Metrics, N: Copy> Paragraph<M, N> {
  /// The paragraph of `runs`, in order.
  pub const fn new(runs: Vec<Inline<M, N>>) -> Self {
    Self {
      runs,
      trimmed: false,
      shape: OnceCell::new(),
      widest: OnceCell::new(),
      narrowest: OnceCell::new(),
    }
  }

  /// The paragraph with its first line trimmed to its cap height and its last line to
  /// its baseline, as CSS's `text-box-trim: trim-both` with `text-box-edge: cap
  /// alphabetic` trims a block's text.
  pub fn trimmed(mut self) -> Self {
    self.trimmed = true;
    self.forget();

    self
  }

  /// Forgets the clusters found and the sizes measured, after what measures them changed.
  pub fn forget(&mut self) {
    self.shape.take();
    self.widest.take();
    self.narrowest.take();
  }

  /// How wide and tall the paragraph lays out wrapped within `width`, or along its
  /// newlines alone without one.
  pub fn size_within(&self, width: Option<f32>) -> Size<f32> {
    let widest = *self.widest.get_or_init(|| self.size_of(None, true));

    match width {
      Some(width) if width < widest.width => self.size_of(Some(width), true),
      _ => widest,
    }
  }

  /// How wide and tall the paragraph lays out at its narrowest: as wide as its widest
  /// word, no word broken, and wrapped there, as CSS lays out a min-content size.
  pub fn narrowest(&self) -> Size<f32> {
    *self.narrowest.get_or_init(|| {
      let widest_word = self.size_of(Some(0.), false).width;

      self.size_within(Some(widest_word))
    })
  }

  /// The lines the paragraph wraps into within `width`, or along its newlines alone
  /// without one.
  #[must_use]
  pub fn lines_within(&self, width: Option<f32>) -> Vec<WrappedLine> {
    self
      .lines(width, true)
      .into_iter()
      .map(|line| WrappedLine {
        clusters: line.clusters,
        shown: line.shown,
        height: line.above + line.below,
        baseline: line.above,
      })
      .collect()
  }

  /// The text of the clusters `range`, a box's as the object replacement character.
  #[must_use]
  pub fn text(&self, range: Range<usize>) -> String {
    self.shaped().clusters[range]
      .iter()
      .map(|cluster| self.text_of(cluster))
      .collect()
  }

  /// How wide and tall the paragraph lays out wrapped within `limit`, a word broken where
  /// `split` allows it.
  fn size_of(&self, limit: Option<f32>, split: bool) -> Size<f32> {
    let lines = self.lines(limit, split);
    let (top, bottom) = self.trims(&lines);

    Size::new(
      lines.iter().map(|line| line.width).fold(0., f32::max),
      lines.iter().map(Line::height).sum::<f32>() - top - bottom,
    )
  }

  /// How tall the line box of the run at `index` is, how far below its top its baseline
  /// lies, and how far its cap height rises above that baseline: its text's, or for a
  /// box its own height with its bottom on the baseline.
  fn extent(&self, index: usize) -> (f32, f32, f32) {
    let run = &self.runs[index];

    match run.content {
      Content::Text(_) => (
        run.metrics.line_height(),
        run.metrics.baseline(),
        run.metrics.cap(),
      ),
      Content::Box(size) => (size.height, size.height, size.height),
    }
  }

  /// How far the first of `lines` is trimmed at its top and the last at its bottom.
  fn trims(&self, lines: &[Line]) -> (f32, f32) {
    match (self.trimmed, lines.first(), lines.last()) {
      (true, Some(first), Some(last)) => (first.above - first.cap, last.below),
      _ => (0., 0.),
    }
  }

  /// Every cluster and box of the paragraph laid out in a content box at `content`,
  /// where it places each, in order: what a platform draws its text from.
  #[must_use]
  pub fn clusters(&self, content: Rect<f32>) -> Vec<Placed<'_, N>> {
    let clusters = &self.shaped().clusters;
    let lines = self.lines(Some(content.size.width), true);
    let mut placed = Vec::new();
    let mut top = content.origin.y - self.trims(&lines).0;

    for line in &lines {
      let mut advance = 0_f32;

      for cluster in &clusters[line.clusters.clone()] {
        let (height, baseline, _) = self.extent(cluster.run);
        let width = self.advance_at(cluster, advance);

        placed.push(Placed {
          node: self.runs[cluster.run].node,
          text: self.text_of(cluster),
          rect: Rect {
            origin: Point::new(content.origin.x + advance, top + line.above - baseline),
            size: Size::new(width, height),
          },
        });
        advance += width;
      }

      top += line.height();
    }

    placed
  }

  /// The paragraph's clusters, each box one of them, and whether a line may end before
  /// each.
  fn shaped(&self) -> &Shape {
    self.shape.get_or_init(|| self.shape())
  }

  /// The text of `cluster`: its run's, or the object replacement character for a box.
  fn text_of(&self, cluster: &Shaped) -> &str {
    match &self.runs[cluster.run].content {
      Content::Text(text) => &text[cluster.bytes.clone()],
      Content::Box(_) => OBJECT_TEXT,
    }
  }

  /// Finds the paragraph's clusters, each box one of them, how far each advances its
  /// line, and whether a line may end before each.
  fn shape(&self) -> Shape {
    let mut clusters = Vec::new();
    let mut starts = Vec::new();
    let mut offset = 0;

    for (index, run) in self.runs.iter().enumerate() {
      match &run.content {
        Content::Text(content) => {
          for (byte, cluster) in content.grapheme_indices(true) {
            let kind = ClusterKind::of(cluster);

            starts.push(offset + byte);
            clusters.push(Shaped {
              run: index,
              bytes: byte..byte + cluster.len(),
              advance: if kind == ClusterKind::Newline {
                0.
              }
              else {
                run.metrics.advance(cluster)
              },
              kind,
            });
          }

          offset += content.len();
        }
        Content::Box(size) => {
          starts.push(offset);
          clusters.push(Shaped {
            run: index,
            bytes: 0..0,
            advance: size.width,
            kind: ClusterKind::Box,
          });
          offset += OBJECT_TEXT.len();
        }
      }
    }

    let breaks = if let [
      Inline {
        content: Content::Text(text),
        ..
      },
    ] = self.runs.as_slice()
    {
      Self::breaks(text, &starts, &clusters)
    }
    else {
      let text: String = self
        .runs
        .iter()
        .map(|run| match &run.content {
          Content::Text(text) => text.as_str(),
          Content::Box(_) => OBJECT_TEXT,
        })
        .collect();

      Self::breaks(&text, &starts, &clusters)
    };

    Shape { clusters, breaks }
  }

  /// The lines the paragraph's clusters wrap into within `limit`, or unwrapped but at
  /// their newlines, a line ending only where the shape allows it, or within a word too
  /// wide for a line of its own where `split` allows it.
  fn lines(&self, limit: Option<f32>, split: bool) -> Vec<Line> {
    let Shape { clusters, breaks } = self.shaped();
    let mut lines = Vec::new();
    let Some(first) = clusters.first()
    else {
      return lines;
    };
    let mut line = self.line(first.run, 0);
    let mut advance = 0_f32;
    let mut index = 0;

    while index < clusters.len() {
      let Shaped { run, kind, .. } = clusters[index];

      if kind == ClusterKind::Newline {
        let next = self.line(run, index + 1);

        self.show_blanks(&mut line, clusters, index, limit);
        lines.push(std::mem::replace(&mut line, next).closed(index + 1));
        advance = 0.;
        index += 1;

        if index == clusters.len() {
          return lines;
        }

        continue;
      }

      let start = index;

      index += 1;

      while index < clusters.len() && !breaks[index] && clusters[index].kind != ClusterKind::Newline
      {
        index += 1;
      }

      let mut end = index;

      while end > start && clusters[end - 1].kind.is_blank() {
        end -= 1;
      }

      let ink = self.span(&clusters[start..end], advance);

      if !line.pieces.is_empty() && limit.is_some_and(|limit| advance + ink > limit) {
        lines.push(std::mem::replace(&mut line, self.line(run, start)).ended(start, advance));
        advance = 0.;
      }

      for (position, cluster) in clusters.iter().enumerate().take(index).skip(start) {
        let mut width = self.advance_at(cluster, advance);

        if split
          && position < end
          && !line.pieces.is_empty()
          && limit.is_some_and(|limit| advance + width > limit)
        {
          lines.push(
            std::mem::replace(&mut line, self.line(cluster.run, position)).ended(position, advance),
          );
          advance = 0.;
          width = self.advance_at(cluster, advance);
        }

        if position < end {
          line.width = advance + width;
          line.shown.end = position + 1;
        }

        match line.pieces.last_mut() {
          Some(piece) if piece.run == cluster.run => piece.width += width,
          _ => {
            let (height, baseline, cap) = self.extent(cluster.run);

            if line.pieces.is_empty() {
              (line.above, line.below, line.cap) = (baseline, height - baseline, cap);
            }
            else {
              line.above = line.above.max(baseline);
              line.below = line.below.max(height - baseline);
              line.cap = line.cap.max(cap);
            }

            line.pieces.push(Piece {
              run: cluster.run,
              start: advance,
              width,
            });
          }
        }

        advance += width;
      }
    }

    self.show_blanks(&mut line, clusters, clusters.len(), limit);
    lines.push(line.closed(clusters.len()));

    lines
  }

  /// Shows the blanks `line` ends in before the cluster at `end`, a newline or the text's
  /// end, each taking its room on the line as far as they fit within `limit`.
  fn show_blanks(&self, line: &mut Line, clusters: &[Shaped], end: usize, limit: Option<f32>) {
    for (position, cluster) in clusters.iter().enumerate().take(end).skip(line.shown.end) {
      let width = self.advance_at(cluster, line.width);

      if limit.is_some_and(|limit| line.width + width > limit) {
        break;
      }

      line.width += width;
      line.shown.end = position + 1;
    }
  }

  /// How far `clusters` advance a line from `position`.
  fn span(&self, clusters: &[Shaped], position: f32) -> f32 {
    clusters.iter().fold(position, |advance, cluster| {
      advance + self.advance_at(cluster, advance)
    }) - position
  }

  /// How far `cluster` advances its line from `position`: a tab to the next stop at least
  /// half its run's zero beyond, the stops eight of its run's spaces apart, as CSS's
  /// `tab-size: 8` does; anything else as far as its run measures it.
  fn advance_at(&self, cluster: &Shaped, position: f32) -> f32 {
    if cluster.kind != ClusterKind::Tab {
      return cluster.advance;
    }

    let metrics = &self.runs[cluster.run].metrics;
    let interval = 8. * metrics.advance(" ");

    if interval <= 0. {
      return 0.;
    }

    let mut stop = ((position / interval).floor() + 1.) * interval;

    if stop - position < metrics.advance("0") / 2. {
      stop += interval;
    }

    stop - position
  }

  /// Whether a line may end before each of `clusters` of `text`, starting at the bytes
  /// `starts`: where the Unicode line breaking algorithm allows it, and around each box.
  fn breaks(text: &str, starts: &[usize], clusters: &[Shaped]) -> Vec<bool> {
    let mut breaks = vec![false; clusters.len() + 1];

    for (byte, _) in unicode_linebreak::linebreaks(text) {
      if let Ok(index) = starts.binary_search(&byte) {
        breaks[index] = true;
      }
    }

    for (index, cluster) in clusters.iter().enumerate() {
      if cluster.kind == ClusterKind::Box {
        breaks[index] = true;
        breaks[index + 1] = true;
      }
    }

    breaks
  }

  /// An empty line starting at the cluster at `start`, as tall as a line of the run at
  /// `index` until a run's text is on it.
  fn line(&self, index: usize, start: usize) -> Line {
    let (height, baseline, cap) = self.extent(index);

    Line {
      pieces: Vec::new(),
      clusters: start..start,
      shown: start..start,
      width: 0.,
      above: baseline,
      below: height - baseline,
      cap,
    }
  }
}

/// One cluster of a paragraph's text, or one of its boxes, where its layout places it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed<'a, N = NodeId> {
  /// The node of the run it belongs to.
  pub node: N,
  /// Its text; empty for a box.
  pub text: &'a str,
  /// The rectangle it advances across, its run's line box on its line's baseline.
  pub rect: Rect<f32>,
}

/// The character a box stands for in its paragraph's text, as the line breaking
/// algorithm reads it.
const OBJECT_TEXT: &str = "\u{fffc}";

/// A paragraph's clusters, found once, and whether a line may end before each and at the
/// end.
#[derive(Debug, Clone)]
struct Shape {
  clusters: Vec<Shaped>,
  breaks: Vec<bool>,
}

/// One cluster as found: the run it belongs to, its bytes in the run's text, how far it
/// advances its line away from a tab stop, and what it is to a line.
#[derive(Debug, Clone)]
struct Shaped {
  run: usize,
  bytes: Range<usize>,
  advance: f32,
  kind: ClusterKind,
}

/// What a cluster is to a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClusterKind {
  /// A newline, ending its line.
  Newline,
  /// A space, hanging at a line's end.
  Space,
  /// A tab, advancing to its stop and hanging at a line's end.
  Tab,
  /// A box.
  Box,
  /// Anything else.
  Other,
}

impl ClusterKind {
  /// What `cluster`, one cluster of text, is to a line.
  fn of(cluster: &str) -> Self {
    match cluster {
      "\n" | "\r\n" => Self::Newline,
      " " => Self::Space,
      "\t" => Self::Tab,
      _ => Self::Other,
    }
  }

  /// Whether the cluster is a blank, a space or a tab, which hangs at a line's end.
  const fn is_blank(self) -> bool {
    matches!(self, Self::Space | Self::Tab)
  }
}

impl<M: Metrics> Leaf for Paragraph<M> {
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
    let lines = self.lines(Some(content.size.width), true);
    let mut placed = Vec::new();
    let mut top = content.origin.y - self.trims(&lines).0;

    for line in &lines {
      for piece in &line.pieces {
        let run = &self.runs[piece.run];
        let (height, baseline, _) = self.extent(piece.run);
        let rect = Rect {
          origin: Point::new(content.origin.x + piece.start, top + line.above - baseline),
          size: Size::new(piece.width, height),
        };

        placed.extend(run.groups.iter().map(|group| (*group, rect)));
        placed.push((run.node, rect));
      }

      top += line.height();
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
    for run in &mut self.runs {
      if run.node == node && matches!(run.content, Content::Box(_)) {
        run.content = Content::Box(size);
      }
    }

    self.forget();
  }
}
