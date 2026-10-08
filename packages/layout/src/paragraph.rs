//! A paragraph: inline runs laid out as one text, wrapping across them, each measured by
//! its platform's metrics, and the boxes laid out among them.

use std::ops::Range;

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

/// One run of a paragraph: the node it lays out, the groups around it, what it flows and
/// the metrics it measures its text by.
#[derive(Debug, Clone, PartialEq)]
pub struct Inline<M> {
  /// The run's node.
  pub node: NodeId,
  /// The groups around the run within the paragraph, outermost first, each laid out
  /// where its runs are.
  pub groups: Vec<NodeId>,
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

/// Inline runs laid out as one text, each run's clusters measured by its metrics `M`.
///
/// The text wraps where the Unicode line breaking algorithm allows, as after the blanks
/// ending a word or a hyphen, at each newline and around each box; a word runs across
/// runs where nothing breaks it. Each run's line box sits on its line's baseline, a box's
/// bottom on it, and a line spans the highest of them above the baseline to the deepest
/// below it; it is as wide as its text without the blanks it ends in, which take no room
/// on it.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Paragraph<M> {
  runs: Vec<Inline<M>>,
  trimmed: bool,
}

/// One line of a paragraph: the stretches of each run on it, the clusters it holds, how
/// wide its text is short of the blanks it ends in, how far it reaches above its
/// baseline and below it, and how far its highest cap height rises above it.
struct Line {
  pieces: Vec<Piece>,
  clusters: Range<usize>,
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

  /// The line ending before the cluster at `end`, the blanks it ends in removed from its
  /// pieces, its text advancing `advance` with them.
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
}

/// A stretch of one run on a line: the run's index, where it starts along the line, and
/// how wide it is.
struct Piece {
  run: usize,
  start: f32,
  width: f32,
}

impl<M: Metrics> Paragraph<M> {
  /// The paragraph of `runs`, in order.
  pub const fn new(runs: Vec<Inline<M>>) -> Self {
    Self {
      runs,
      trimmed: false,
    }
  }

  /// The paragraph with its first line trimmed to its cap height and its last line to
  /// its baseline, as CSS's `text-box-trim: trim-both` with `text-box-edge: cap
  /// alphabetic` trims a block's text.
  pub const fn trimmed(mut self) -> Self {
    self.trimmed = true;

    self
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
  pub fn clusters(&self, content: Rect<f32>) -> Vec<Placed<'_>> {
    let (clusters, breaks) = self.shaped();
    let lines = self.lines(&clusters, &breaks, Some(content.size.width));
    let mut placed = Vec::new();
    let mut top = content.origin.y - self.trims(&lines).0;

    for line in &lines {
      let mut advance = 0_f32;

      for cluster in &clusters[line.clusters.clone()] {
        let (height, baseline, _) = self.extent(cluster.run);

        placed.push(Placed {
          node: self.runs[cluster.run].node,
          text: cluster.text,
          rect: Rect {
            origin: Point::new(content.origin.x + advance, top + line.above - baseline),
            size: Size::new(cluster.advance, height),
          },
        });
        advance += cluster.advance;
      }

      top += line.height();
    }

    placed
  }

  /// The paragraph's clusters, each box one of them, and whether a line may end before
  /// each.
  fn shaped(&self) -> (Vec<Cluster<'_>>, Vec<bool>) {
    let mut text = String::new();
    let mut clusters = Vec::new();

    for (index, run) in self.runs.iter().enumerate() {
      match &run.content {
        Content::Text(content) => {
          for (byte, cluster) in content.grapheme_indices(true) {
            clusters.push(Cluster {
              run: index,
              text: cluster,
              advance: run.metrics.advance(cluster),
              byte: text.len() + byte,
              boxed: false,
            });
          }

          text.push_str(content);
        }
        Content::Box(size) => {
          clusters.push(Cluster {
            run: index,
            text: "",
            advance: size.width,
            byte: text.len(),
            boxed: true,
          });
          text.push(OBJECT);
        }
      }
    }

    let breaks = Self::breaks(&text, &clusters);

    (clusters, breaks)
  }

  /// The lines `clusters` wrap into within `limit`, or unwrapped but at their newlines,
  /// a line ending only where `breaks` allows it.
  fn lines(&self, clusters: &[Cluster<'_>], breaks: &[bool], limit: Option<f32>) -> Vec<Line> {
    let mut lines = Vec::new();
    let Some(first) = clusters.first()
    else {
      return lines;
    };
    let mut line = self.line(first.run, 0);
    let mut advance = 0_f32;
    let mut index = 0;

    while index < clusters.len() {
      let Cluster { run, text, .. } = clusters[index];

      if matches!(text, "\n" | "\r\n") {
        lines
          .push(std::mem::replace(&mut line, self.line(run, index + 1)).ended(index + 1, advance));
        advance = 0.;
        index += 1;

        continue;
      }

      let start = index;

      index += 1;

      while index < clusters.len()
        && !breaks[index]
        && !matches!(clusters[index].text, "\n" | "\r\n")
      {
        index += 1;
      }

      let mut end = index;

      while end > start && !clusters[end - 1].boxed && clusters[end - 1].text == " " {
        end -= 1;
      }

      let ink: f32 = clusters[start..end]
        .iter()
        .map(|cluster| cluster.advance)
        .sum();

      if !line.pieces.is_empty() && limit.is_some_and(|limit| advance + ink > limit) {
        lines.push(std::mem::replace(&mut line, self.line(run, start)).ended(start, advance));
        advance = 0.;
      }

      line.width = advance + ink;

      for &Cluster {
        run,
        advance: width,
        ..
      } in &clusters[start..index]
      {
        match line.pieces.last_mut() {
          Some(piece) if piece.run == run => piece.width += width,
          _ => {
            let (height, baseline, cap) = self.extent(run);

            if line.pieces.is_empty() {
              (line.above, line.below, line.cap) = (baseline, height - baseline, cap);
            }
            else {
              line.above = line.above.max(baseline);
              line.below = line.below.max(height - baseline);
              line.cap = line.cap.max(cap);
            }

            line.pieces.push(Piece {
              run,
              start: advance,
              width,
            });
          }
        }

        advance += width;
      }
    }

    lines.push(line.ended(clusters.len(), advance));

    lines
  }

  /// Whether a line may end before each of `clusters` of `text`: where the Unicode line
  /// breaking algorithm allows it, and around each box.
  fn breaks(text: &str, clusters: &[Cluster<'_>]) -> Vec<bool> {
    let mut breaks = vec![false; clusters.len() + 1];

    for (byte, _) in unicode_linebreak::linebreaks(text) {
      if let Ok(index) = clusters.binary_search_by_key(&byte, |cluster| cluster.byte) {
        breaks[index] = true;
      }
    }

    for (index, cluster) in clusters.iter().enumerate() {
      if cluster.boxed {
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
      width: 0.,
      above: baseline,
      below: height - baseline,
      cap,
    }
  }
}

/// One cluster of a paragraph's text, or one of its boxes, where its layout places it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed<'a> {
  /// The node of the run it belongs to.
  pub node: NodeId,
  /// Its text; empty for a box.
  pub text: &'a str,
  /// The rectangle it advances across, its run's line box on its line's baseline.
  pub rect: Rect<f32>,
}

/// The character a box stands for in its paragraph's text, as the line breaking
/// algorithm reads it.
const OBJECT: char = '\u{fffc}';

/// One cluster of a paragraph's text, or one of its boxes: the run it belongs to, its
/// text, how far it advances its line, the byte it starts at in the paragraph's text,
/// and whether it is a box.
#[derive(Clone, Copy)]
struct Cluster<'a> {
  run: usize,
  text: &'a str,
  advance: f32,
  byte: usize,
  boxed: bool,
}

impl<M: Metrics> Leaf for Paragraph<M> {
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    let limit = known.width.or(match available.width {
      AvailableSpace::Definite(width) => Some(width),
      AvailableSpace::MinContent => Some(0.),
      AvailableSpace::MaxContent => None,
    });
    let (clusters, breaks) = self.shaped();
    let lines = self.lines(&clusters, &breaks, limit);
    let (top, bottom) = self.trims(&lines);

    Size::new(
      known
        .width
        .unwrap_or_else(|| lines.iter().map(|line| line.width).fold(0., f32::max)),
      known
        .height
        .unwrap_or_else(|| lines.iter().map(Line::height).sum::<f32>() - top - bottom),
    )
  }

  fn place(&self, content: Rect<f32>) -> Vec<(NodeId, Rect<f32>)> {
    let (clusters, breaks) = self.shaped();
    let lines = self.lines(&clusters, &breaks, Some(content.size.width));
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
  }
}
