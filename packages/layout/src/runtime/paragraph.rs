//! The paragraph structure the runtime reads from its records and the scene: which boxes
//! lay their runs out as one paragraph, the runs flowing in one, and a text node's
//! source text with where each of its runs lies in it ([`Source`]).

use std::ops::Range;

use skene_foundation::{
  Layout,
  NodeId,
  RunPlacement,
  text::{Round, TextIndex},
};

use super::{Drawn, Kind, Record, Runtime};

impl<P: Drawn> Runtime<P> {
  /// Whether `node` lays its inline runs out as one paragraph: a flow box whose every
  /// laid-out child is an inline run ([`is_inline_run`](Self::is_inline_run)).
  #[must_use]
  pub fn is_paragraph(&self, node: NodeId) -> bool {
    if self.record(node).and_then(Record::layout) != Some(Layout::Document) {
      return false;
    }

    let children = self.layout_children(node);

    !children.is_empty() && children.iter().all(|child| self.is_inline_run(*child))
  }

  /// Whether `node` is an inline box: a run laid out as a box among its paragraph's
  /// text, its text one placeholder cluster as large as the box.
  #[must_use]
  pub fn is_inline_box(&self, node: NodeId) -> bool {
    matches!(
      self.record(node).map(|record| &record.kind),
      Some(Kind::Run { facts, .. }) if facts.placement == RunPlacement::Embedded
    )
  }

  /// Whether `node` is an inline run: text, a group or an inline box flowing in a
  /// paragraph.
  #[must_use]
  pub fn is_inline_run(&self, node: NodeId) -> bool {
    matches!(
      self.record(node).map(|record| &record.kind),
      Some(Kind::Run { facts, .. }) if facts.placement != RunPlacement::Line
    )
  }

  /// The paragraph the inline run `node` flows in, through any groups around it.
  #[must_use]
  pub fn enclosing_paragraph(&self, node: NodeId) -> Option<NodeId> {
    let mut current = self.layout_parent(node)?;

    while !self.is_paragraph(current) {
      if !self.is_inline_run(current) {
        return None;
      }

      current = self.layout_parent(current)?;
    }

    Some(current)
  }

  /// The text of the paragraph `node`: every inline run beneath it, in order.
  #[must_use]
  pub fn paragraph_text(&self, node: NodeId) -> String {
    let mut text = String::new();

    for run in self.paragraph_runs(node) {
      if let Some(record) = self.record(run) {
        text.push_str(&record.text);
      }
    }

    text
  }

  /// The leaf runs of the paragraph `node`, in order: a group run contributes its runs,
  /// never its own (empty) text.
  #[must_use]
  pub fn paragraph_runs(&self, node: NodeId) -> Vec<NodeId> {
    let mut runs = Vec::new();

    self.visit_runs(node, &mut Vec::new(), &mut |run, _| {
      runs.push(run);
    });

    runs
  }

  /// Visits each leaf run beneath `node` in order, with the group runs around it beneath
  /// the paragraph, outermost first, `groups` being those around `node`.
  pub fn visit_runs(
    &self,
    node: NodeId,
    groups: &mut Vec<NodeId>,
    visit: &mut impl FnMut(NodeId, &[NodeId]),
  ) {
    for child in self.layout_children(node) {
      let is_run = matches!(
        self.record(child).map(|record| &record.kind),
        Some(Kind::Run { .. })
      );

      if !is_run {
        continue;
      }

      if self.is_inline_box(child) || self.layout_children(child).is_empty() {
        visit(child, groups);
      }
      else {
        groups.push(child);
        self.visit_runs(child, groups, visit);
        groups.pop();
      }
    }
  }
}

/// One run of a text node's source: its node, the bytes of the source it holds, and the
/// byte of the flow its text starts at, `None` for an inline box, which the flow holds
/// as a box rather than as its placeholder.
#[derive(Debug, Clone)]
struct SourceRun {
  node: NodeId,
  source: Range<usize>,
  flow: Option<usize>,
}

/// A text node's source text, its clusters and where each of its runs lies.
///
/// The runs are a paragraph's leaf runs, or a run flowing in none; the text is what they
/// hold, an inline box its placeholder, and each run lies in it and in the flow the
/// layout lays out.
#[derive(Debug, Clone)]
#[must_use]
pub struct Source {
  /// The text of every run, in order, an inline box its placeholder
  /// ([`INLINE_BOX`](super::INLINE_BOX)).
  pub text: String,
  /// The clusters of [`text`](Self::text).
  pub index: TextIndex,
  runs: Vec<SourceRun>,
}

impl Source {
  /// The source of the text node `node`: a paragraph's leaf runs, or the run itself.
  pub fn of<P: Drawn>(runtime: &Runtime<P>, node: NodeId) -> Self {
    let runs = if runtime.is_paragraph(node) {
      runtime.paragraph_runs(node)
    }
    else {
      vec![node]
    };
    let mut text = String::new();
    let mut flow = 0;
    let mut source = Vec::new();

    for run in runs {
      let Some(record) = runtime.record(run)
      else {
        continue;
      };
      let at = text.len();
      let boxed = runtime.is_inline_box(run);

      text.push_str(&record.text);
      source.push(SourceRun {
        node: run,
        source: at..text.len(),
        flow: (!boxed).then_some(flow),
      });

      if !boxed {
        flow += record.text.len();
      }
    }

    Self {
      index: TextIndex::new(&text),
      text,
      runs: source,
    }
  }

  /// The source's runs in order: each node and the bytes of the source it holds.
  pub fn runs(&self) -> impl Iterator<Item = (NodeId, Range<usize>)> + '_ {
    self.runs.iter().map(|run| (run.node, run.source.clone()))
  }

  /// The cluster of the source a caret before byte `offset` of `node`'s text stands
  /// before; `None` for a node outside the source.
  #[must_use]
  pub fn cluster_of(&self, node: NodeId, offset: usize) -> Option<usize> {
    let run = self.runs.iter().find(|run| run.node == node)?;
    let byte = (run.source.start + offset).min(run.source.end);

    Some(self.index.grapheme_at_byte(byte, Round::Down))
  }

  /// The run whose text holds byte `at` of the flow, and the cluster of the source it
  /// lies in.
  #[must_use]
  pub fn at_flow(&self, at: usize) -> Option<(NodeId, usize)> {
    self.runs.iter().find_map(|run| {
      let start = run.flow?;
      let length = run.source.len();

      (start <= at && at < start + length).then(|| {
        (
          run.node,
          self
            .index
            .grapheme_at_byte(run.source.start + at - start, Round::Down),
        )
      })
    })
  }
}
