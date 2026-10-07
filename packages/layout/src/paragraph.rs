//! A paragraph: inline runs laid out as one text, wrapping across them, each measured by
//! its platform's metrics.

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
}

/// One run of a paragraph: the node it lays out, the groups around it, its text and the
/// metrics it measures by.
#[derive(Debug, Clone, PartialEq)]
pub struct Inline<M> {
  /// The run's node.
  pub node: NodeId,
  /// The groups around the run within the paragraph, outermost first, each laid out
  /// where its runs are.
  pub groups: Vec<NodeId>,
  /// The run's text.
  pub text: String,
  /// What measures the run's text.
  pub metrics: M,
}

/// Inline runs laid out as one text, each run's clusters measured by its metrics `M`.
///
/// The text wraps between words, after the blanks that end them, and at each newline; a
/// word runs across runs where nothing breaks it. A line is as tall as the tallest run
/// on it, each run sitting on its bottom, and as wide as its text without the blanks it
/// ends in, which take no room on it.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Paragraph<M> {
  runs: Vec<Inline<M>>,
}

/// One line of a paragraph: the stretches of each run on it, how wide its text is short
/// of the blanks it ends in, and how tall it is.
struct Line {
  pieces: Vec<Piece>,
  width: f32,
  height: f32,
}

impl Line {
  /// The line with the blanks it ends in removed from its pieces, its text advancing
  /// `advance` with them.
  fn trimmed(mut self, advance: f32) -> Self {
    let mut blanks = advance - self.width;

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
    Self { runs }
  }

  /// The lines the text wraps into within `limit`, or unwrapped but at its newlines.
  fn lines(&self, limit: Option<f32>) -> Vec<Line> {
    let clusters: Vec<(usize, &str, f32)> = self
      .runs
      .iter()
      .enumerate()
      .flat_map(|(index, run)| {
        run
          .text
          .graphemes(true)
          .map(move |cluster| (index, cluster, run.metrics.advance(cluster)))
      })
      .collect();
    let mut lines = Vec::new();
    let Some(&(first, ..)) = clusters.first()
    else {
      return lines;
    };
    let mut line = self.line(first);
    let mut advance = 0_f32;
    let mut index = 0;

    while index < clusters.len() {
      let (run, cluster, _) = clusters[index];

      if matches!(cluster, "\n" | "\r\n") {
        lines.push(std::mem::replace(&mut line, self.line(run)).trimmed(advance));
        advance = 0.;
        index += 1;

        continue;
      }

      let start = index;

      while index < clusters.len() && !matches!(clusters[index].1, " " | "\n" | "\r\n") {
        index += 1;
      }

      let ink: f32 = clusters[start..index].iter().map(|cluster| cluster.2).sum();

      while index < clusters.len() && clusters[index].1 == " " {
        index += 1;
      }

      if !line.pieces.is_empty() && limit.is_some_and(|limit| advance + ink > limit) {
        lines.push(std::mem::replace(&mut line, self.line(run)).trimmed(advance));
        advance = 0.;
      }

      line.width = advance + ink;

      for &(run, _, width) in &clusters[start..index] {
        match line.pieces.last_mut() {
          Some(piece) if piece.run == run => piece.width += width,
          _ => {
            let height = self.runs[run].metrics.line_height();

            line.height = if line.pieces.is_empty() {
              height
            }
            else {
              line.height.max(height)
            };
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

    lines.push(line.trimmed(advance));

    lines
  }

  /// An empty line, as tall as a line of the run at `index` until a run's text is on it.
  fn line(&self, index: usize) -> Line {
    Line {
      pieces: Vec::new(),
      width: 0.,
      height: self.runs[index].metrics.line_height(),
    }
  }
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
    let lines = self.lines(limit);

    Size::new(
      known
        .width
        .unwrap_or_else(|| lines.iter().map(|line| line.width).fold(0., f32::max)),
      known
        .height
        .unwrap_or_else(|| lines.iter().map(|line| line.height).sum()),
    )
  }

  fn place(&self, content: Rect<f32>) -> Vec<(NodeId, Rect<f32>)> {
    let mut placed = Vec::new();
    let mut top = content.origin.y;

    for line in self.lines(Some(content.size.width)) {
      for piece in &line.pieces {
        let run = &self.runs[piece.run];
        let height = run.metrics.line_height();
        let rect = Rect {
          origin: Point::new(content.origin.x + piece.start, top + line.height - height),
          size: Size::new(piece.width, height),
        };

        placed.extend(run.groups.iter().map(|group| (*group, rect)));
        placed.push((run.node, rect));
      }

      top += line.height;
    }

    placed
  }
}
