//! A text leaf of the layout: a paragraph's runs and inline boxes laid out as one text
//! through a realization's text engine, measured as taffy asks through the foundation's
//! text leaf, placing each run and box where the engine lays it out, and answering which run
//! a point falls on.

use std::{cell::OnceCell, ops::Range};

use skene_foundation::{
  NodeId,
  Point,
  Rect,
  Size,
  text::{
    Affinity,
    AttributedText,
    InlineBox,
    Laid,
    LineHeight,
    LineInfo,
    MeasureText,
    Paragraph,
    ParagraphStyle,
    RunExtent,
    Space,
    TextEngine,
    TextLeaf,
    TextRun,
    TextStyle,
  },
};
use taffy::prelude::AvailableSpace;

use crate::Leaf;

/// What flows in a paragraph, in the text's order.
#[derive(Clone, Debug, PartialEq)]
enum Item<N> {
  /// A run of text.
  Run(FlowRun<N>),
  /// An inline box.
  Box(FlowBox<N>),
}

impl<N> Item<N> {
  /// The groups around the item, outermost first.
  fn groups(&self) -> &[N] {
    match self {
      Self::Run(run) => &run.groups,
      Self::Box(inline) => &inline.groups,
    }
  }

  /// The item's node.
  const fn node(&self) -> &N {
    match self {
      Self::Run(run) => &run.node,
      Self::Box(inline) => &inline.node,
    }
  }
}

/// A run of a flow: its node, the groups around it, the bytes of the flow's text it holds
/// and its style.
#[derive(Clone, Debug, PartialEq)]
struct FlowRun<N> {
  node: N,
  groups: Vec<N>,
  range: Range<usize>,
  style: TextStyle,
}

/// An inline box of a flow: its node, the groups around it, the byte of the flow's text it
/// stands before, and its size.
#[derive(Clone, Debug, PartialEq)]
struct FlowBox<N> {
  node: N,
  groups: Vec<N>,
  index: usize,
  size: Size<f32>,
}

/// Where a point falls in a flow: a run, and the byte of the run's text a caret there
/// stands before.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[must_use]
pub struct RunHit<N> {
  /// The run's node.
  pub node: N,
  /// The byte of the run's text, on a character boundary; the text's length past its end.
  pub offset: usize,
}

/// A paragraph's runs and inline boxes laid out as one text through the engine `E`, each run
/// and box a node `N`: a leaf of the layout.
///
/// Taffy measures it through the foundation's [`TextLeaf`]: its min-content width is its
/// widest unbreakable piece, its max-content width its lines unwrapped, and a definite width
/// lays it out wrapped where the line fills. Each run lies on each line it reaches as its
/// line box there, its glyphs centered in it, short of the blanks hanging past the line's
/// end; each group around a run lies where the run does, and each box where the engine
/// places it. The text is made into a paragraph once, until a box it holds is sized afresh.
#[must_use]
pub struct TextFlow<E: TextEngine, N = NodeId> {
  engine: E,
  base: TextStyle,
  style: ParagraphStyle,
  text: String,
  items: Vec<Item<N>>,
  leaf: OnceCell<Box<TextLeaf<E::Paragraph>>>,
}

impl<E: TextEngine, N: std::fmt::Debug> std::fmt::Debug for TextFlow<E, N> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("TextFlow")
      .field("base", &self.base)
      .field("style", &self.style)
      .field("text", &self.text)
      .field("items", &self.items)
      .finish_non_exhaustive()
  }
}

impl<E: TextEngine, N: PartialEq> PartialEq for TextFlow<E, N> {
  fn eq(&self, other: &Self) -> bool {
    self.base == other.base
      && self.style == other.style
      && self.text == other.text
      && self.items == other.items
  }
}

impl<E: TextEngine, N: Clone + PartialEq> TextFlow<E, N> {
  /// Whether `held` and `next` lay text out alike: everything but their paint the same.
  fn lays_out_alike(held: &TextStyle, next: &TextStyle) -> bool {
    let TextStyle {
      family,
      size,
      weight,
      slant,
      letter_spacing,
      line_height,
      paint: _,
    } = held;

    *family == next.family
      && size.to_bits() == next.size.to_bits()
      && *weight == next.weight
      && *slant == next.slant
      && letter_spacing.to_bits() == next.letter_spacing.to_bits()
      && *line_height == next.line_height
  }

  /// An empty flow laid out through `engine` in `style`, a line holding no run laid out in
  /// `base`.
  pub const fn new(engine: E, base: TextStyle, style: ParagraphStyle) -> Self {
    Self {
      engine,
      base,
      style,
      text: String::new(),
      items: Vec::new(),
      leaf: OnceCell::new(),
    }
  }

  /// Adds the run `node` holding `text` in `style`, inside `groups`, outermost first, after
  /// everything the flow holds.
  pub fn add_run(&mut self, node: N, groups: &[N], text: &str, style: TextStyle) {
    let start = self.text.len();

    self.text.push_str(text);
    self.items.push(Item::Run(FlowRun {
      node,
      groups: groups.to_vec(),
      range: start..self.text.len(),
      style,
    }));
    self.leaf = OnceCell::new();
  }

  /// Adds the inline box `node` of `size`, inside `groups`, outermost first, after
  /// everything the flow holds: one piece a line may end before or after, standing on its
  /// line's baseline.
  pub fn add_box(&mut self, node: N, groups: &[N], size: Size<f32>) {
    self.items.push(Item::Box(FlowBox {
      node,
      groups: groups.to_vec(),
      index: self.text.len(),
      size,
    }));
    self.leaf = OnceCell::new();
  }

  /// The text of every run, in order.
  #[must_use]
  pub fn text(&self) -> &str {
    &self.text
  }

  /// The flow's leaf: its paragraph, made once, measured and laid out at widths.
  pub fn leaf(&self) -> &TextLeaf<E::Paragraph> {
    self.leaf.get_or_init(|| {
      let mut runs = Vec::new();
      let mut boxes = Vec::new();

      for item in &self.items {
        match item {
          Item::Run(run) => runs.push(TextRun {
            range: run.range.clone(),
            style: run.style.clone(),
          }),
          Item::Box(inline) => boxes.push(InlineBox::new(
            inline.index,
            inline.size.width,
            inline.size.height,
          )),
        }
      }

      let text = AttributedText::new(self.text.as_str(), self.base.clone(), runs).with_boxes(boxes);

      Box::new(TextLeaf::new(self.engine.paragraph(&text, &self.style)))
    })
  }

  /// The flow laid out in a content box at `content`, and where the layout's top left lies,
  /// its first line's top at the content box's top.
  pub fn layout_in(&self, content: Rect<f32>) -> (<E::Paragraph as Paragraph>::Laid, Point<f32>) {
    let laid = self.leaf().laid(Some(content.size.width));
    let shift = laid.lines().first().map_or(0., |first| first.top);

    (laid, Point::new(content.origin.x, content.origin.y - shift))
  }

  /// The run drawn at `point` in the flow laid out in a content box at `content`, and the
  /// byte of its text a caret placed there stands before; `None` beside every run.
  pub fn run_at(&self, content: Rect<f32>, point: Point<f32>) -> Option<RunHit<N>> {
    let (laid, origin) = self.layout_in(content);
    let local = Point::new(point.x - origin.x, point.y - origin.y);
    let run = self
      .pieces(&laid)
      .into_iter()
      .rev()
      .find_map(|(item, rect)| match &self.items[item] {
        Item::Run(run) if Self::holds(rect, local) => Some(run),
        _ => None,
      })?;
    let index = laid.hit_test(local.x, local.y).position.index;

    Some(RunHit {
      node: run.node.clone(),
      offset: index.clamp(run.range.start, run.range.end) - run.range.start,
    })
  }

  /// The run nearest `point` in the flow laid out in a content box at `content`, and the
  /// byte of its text a caret placed there stands before: a point beside the text finds
  /// the caret nearest it, on the run on the caret's side; `None` where the flow holds no
  /// text.
  pub fn nearest(&self, content: Rect<f32>, point: Point<f32>) -> Option<RunHit<N>> {
    let (laid, origin) = self.layout_in(content);
    let position = laid
      .hit_test(point.x - origin.x, point.y - origin.y)
      .position;
    let index = position.index;
    let upstream = position.affinity == Affinity::Upstream;
    let runs: Vec<&FlowRun<N>> = self
      .runs()
      .map(|(run, _)| run)
      .filter(|run| !run.range.is_empty())
      .collect();
    let run = runs
      .iter()
      .find(|run| {
        if upstream {
          run.range.start < index && index <= run.range.end
        }
        else {
          run.range.contains(&index)
        }
      })
      .or_else(|| runs.iter().rev().find(|run| run.range.start <= index))
      .or_else(|| runs.first())?;

    Some(RunHit {
      node: run.node.clone(),
      offset: index.clamp(run.range.start, run.range.end) - run.range.start,
    })
  }

  /// Each run, with its place among the items.
  fn runs(&self) -> impl Iterator<Item = (&FlowRun<N>, usize)> {
    self
      .items
      .iter()
      .enumerate()
      .filter_map(|(at, item)| match item {
        Item::Run(run) => Some((run, at)),
        Item::Box(_) => None,
      })
  }

  /// Whether `rect` holds `point`.
  fn holds(rect: Rect<f32>, point: Point<f32>) -> bool {
    point.x >= rect.origin.x
      && point.x < rect.origin.x + rect.size.width
      && point.y >= rect.origin.y
      && point.y < rect.origin.y + rect.size.height
  }

  /// The line of `lines` holding the middle of `rect`.
  fn line_of(lines: &[LineInfo], rect: Rect<f32>) -> Option<usize> {
    let middle = rect.origin.y + rect.size.height / 2.;

    lines
      .iter()
      .position(|line| middle >= line.top && middle < line.top + line.height)
  }

  /// Where each item lies in `laid`, from the layout's top left, in the order they paint:
  /// line by line, each run on a line as its line box there and each box at its frame.
  fn pieces(&self, laid: &<E::Paragraph as Paragraph>::Laid) -> Vec<(usize, Rect<f32>)> {
    let lines = laid.lines();
    let extents = laid.run_extents();
    let frames = laid.inline_boxes();
    let around = Around {
      lines: &lines,
      extents: &extents,
      frames: &frames,
    };
    let mut on_lines: Vec<Vec<(usize, Rect<f32>)>> = vec![Vec::new(); lines.len().max(1)];
    let mut boxes = 0;

    for (at, item) in self.items.iter().enumerate() {
      match item {
        Item::Run(run) => {
          if run.range.is_empty() {
            continue;
          }

          for (line, rect) in self.run_pieces(run, laid, &around) {
            on_lines[line].push((at, rect));
          }
        }
        Item::Box(inline) => {
          let frame = frames.get(boxes).copied().unwrap_or(Rect::ZERO);
          let line = lines
            .iter()
            .position(|line| line.range.contains(&inline.index))
            .unwrap_or(on_lines.len() - 1);

          boxes += 1;
          on_lines[line].push((at, frame));
        }
      }
    }

    on_lines.into_iter().flatten().collect()
  }

  /// Where `run` lies on each line of `laid` it reaches, `around` being what `laid` holds:
  /// across the bytes it covers there, short of the boxes beside them and of the blanks
  /// hanging past the line's end, and as tall as its line box, centered on the line's
  /// baseline as its glyphs are.
  fn run_pieces(
    &self,
    run: &FlowRun<N>,
    laid: &<E::Paragraph as Paragraph>::Laid,
    around: &Around<'_>,
  ) -> Vec<(usize, Rect<f32>)> {
    let Around {
      lines,
      extents,
      frames,
    } = *around;
    let mut spans: Vec<Option<(f32, f32)>> = vec![None; lines.len()];

    for rect in laid.range_rects(run.range.clone()) {
      let Some(line) = Self::line_of(lines, rect)
      else {
        continue;
      };
      let (mut left, mut right) = (rect.origin.x, rect.origin.x + rect.size.width);

      for frame in frames
        .iter()
        .filter(|frame| Self::line_of(lines, **frame) == Some(line))
      {
        let (start, end) = (frame.origin.x, frame.origin.x + frame.size.width);

        if start <= left && left < end {
          left = end.min(right);
        }

        if start < right && right <= end {
          right = start.max(left);
        }
      }

      spans[line] = Some(spans[line].map_or((left, right), |(start, end)| {
        (start.min(left), end.max(right))
      }));
    }

    spans
      .into_iter()
      .enumerate()
      .filter_map(|(at, span)| {
        let (left, right) = span?;
        let line = &lines[at];
        let visible = line.left + line.width;
        let right = right.min(visible);
        let left = left.min(right);
        let (baseline, ascent, descent) = extents
          .iter()
          .find(|extent| {
            extent.line == at
              && extent.range.start < run.range.end
              && run.range.start < extent.range.end
          })
          .map_or(
            (
              line.baseline,
              line.baseline - line.top,
              line.top + line.height - line.baseline,
            ),
            |extent| (extent.baseline, extent.ascent, extent.descent),
          );
        let height = match run.style.line_height.unwrap_or(self.style.line_height) {
          LineHeight::Normal => ascent + descent,
          LineHeight::Factor(factor) => factor * run.style.size,
          LineHeight::Absolute(height) => height,
        };
        let above = ascent + (height - ascent - descent) / 2.;

        Some((
          at,
          Rect {
            origin: Point::new(left, baseline - above),
            size: Size::new(right - left, height),
          },
        ))
      })
      .collect()
  }
}

/// What a layout holds that places a run on its lines: its lines, its stretches painted
/// alike and its boxes' frames.
#[derive(Clone, Copy)]
struct Around<'a> {
  lines: &'a [LineInfo],
  extents: &'a [RunExtent],
  frames: &'a [Rect<f32>],
}

impl<E: TextEngine> Leaf for TextFlow<E, NodeId> {
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    let space = |available| match available {
      AvailableSpace::Definite(width) => Space::Definite(width),
      AvailableSpace::MinContent => Space::MinContent,
      AvailableSpace::MaxContent => Space::MaxContent,
    };
    let measured = self.leaf().measure(
      (known.width, known.height),
      (space(available.width), space(available.height)),
    );

    Size::new(measured.width, measured.height)
  }

  fn place(&self, content: Rect<f32>) -> Vec<(NodeId, Rect<f32>)> {
    let (laid, origin) = self.layout_in(content);
    let mut placed = Vec::new();

    for (item, rect) in self.pieces(&laid) {
      let item = &self.items[item];
      let rect = Rect {
        origin: Point::new(origin.x + rect.origin.x, origin.y + rect.origin.y),
        size: rect.size,
      };

      placed.extend(item.groups().iter().map(|group| (*group, rect)));
      placed.push((*item.node(), rect));
    }

    placed
  }

  fn boxes(&self) -> Vec<NodeId> {
    self
      .items
      .iter()
      .filter_map(|item| match item {
        Item::Box(inline) => Some(inline.node),
        Item::Run(_) => None,
      })
      .collect()
  }

  fn size_box(&mut self, node: NodeId, size: Size<f32>) {
    for item in &mut self.items {
      if let Item::Box(inline) = item
        && inline.node == node
        && inline.size != size
      {
        inline.size = size;
        self.leaf = OnceCell::new();
      }
    }
  }

  fn repaint_as(&mut self, next: &Self) -> bool {
    let alike = self.base == next.base
      && self.style == next.style
      && self.text == next.text
      && self.items.len() == next.items.len()
      && self.items.iter().zip(&next.items).all(|pair| match pair {
        (Item::Run(held), Item::Run(run)) => {
          held.node == run.node
            && held.groups == run.groups
            && held.range == run.range
            && Self::lays_out_alike(&held.style, &run.style)
        }
        (Item::Box(held), Item::Box(inline)) => held == inline,
        _ => false,
      });

    if !alike {
      return false;
    }

    for pair in self.items.iter_mut().zip(&next.items) {
      let (Item::Run(held), Item::Run(run)) = pair
      else {
        continue;
      };

      if held.style.paint == run.style.paint {
        continue;
      }

      if let Some(leaf) = self.leaf.get_mut()
        && !leaf.repaint(run.range.clone(), &run.style.paint)
      {
        return false;
      }

      held.style.paint = run.style.paint;
    }

    true
  }
}

#[cfg(test)]
mod tests {
  use std::{
    collections::HashMap,
    sync::atomic::{AtomicU32, Ordering},
  };

  use skene_foundation::{
    NodeId,
    Point,
    Rect,
    Size,
    text::{Laid as _, ParagraphStyle, TextStyle},
  };
  use skene_parley::{ParleyEngine, Typeface};
  use taffy::{
    prelude::{AvailableSpace, length},
    style::{AlignItems, FlexDirection, Style},
  };

  use super::{RunHit, TextFlow};
  use crate::Tree;

  /// The face the tests lay text out in.
  const INTER: Typeface = Typeface::new(
    "Inter",
    include_bytes!("../../parley/fonts/inter-regular.otf"),
  );

  /// An engine over the face alone, its line metrics fractional.
  fn engine() -> ParleyEngine {
    let engine = ParleyEngine::with_typefaces(&[INTER]);

    engine.set_quantize(false);

    engine
  }

  /// The style of the tests' text: the face at 16, its lines 24 tall.
  fn style() -> TextStyle {
    TextStyle::new(16.).with_line_height(skene_foundation::text::LineHeight::Factor(1.5))
  }

  /// A flow of one run, `node`, holding `text`.
  fn flow<N: Clone + PartialEq>(node: N, text: &str) -> TextFlow<ParleyEngine, N> {
    let mut flow = TextFlow::new(engine(), style(), ParagraphStyle::default());

    flow.add_run(node, &[], text, style());

    flow
  }

  /// How wide `text` lays out on one line.
  fn width(text: &str) -> f32 {
    flow((), text).leaf().laid(None).size().width
  }

  /// The style of the tests' text at `size`, its lines one and a half times it tall.
  fn style_at(size: f32) -> TextStyle {
    TextStyle::new(size).with_line_height(skene_foundation::text::LineHeight::Factor(1.5))
  }

  /// How wide `text` lays out on one line at `size`.
  fn width_at(text: &str, size: f32) -> f32 {
    let mut flow = TextFlow::new(engine(), style_at(size), ParagraphStyle::default());

    flow.add_run((), &[], text, style_at(size));
    flow.leaf().laid(None).size().width
  }

  /// How far below the top of a line box `height` tall the face's baseline lies at
  /// `size`: the box's room beyond the face's ascent and descent split above and below.
  fn baseline(height: f32, size: f32) -> f32 {
    let scale = size / 2048.;

    (height - 2478. * scale) / 2. + 1984. * scale
  }

  /// The rectangle at (`x`, `y`) of `width` by `height`.
  const fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect<f32> {
    Rect {
      origin: Point::new(x, y),
      size: Size::new(width, height),
    }
  }

  /// Asserts that `actual` is `expected`, each edge within a ten-thousandth of a unit.
  #[track_caller]
  fn assert_near(actual: Option<Rect<f32>>, expected: Rect<f32>) {
    let near = actual.is_some_and(|actual| {
      [
        (actual.origin.x, expected.origin.x),
        (actual.origin.y, expected.origin.y),
        (actual.size.width, expected.size.width),
        (actual.size.height, expected.size.height),
      ]
      .iter()
      .all(|(actual, expected)| (actual - expected).abs() < 1e-4)
    });

    assert!(near, "{actual:?} is not {expected:?}");
  }

  /// A paragraph of the tests' text holding `runs`, each a node, the groups
  /// around it, its text and its font size.
  fn paragraph(runs: &[(NodeId, &[NodeId], &str, f32)]) -> TextFlow<ParleyEngine> {
    let mut flow = TextFlow::new(engine(), style(), ParagraphStyle::default());

    for (node, groups, text, size) in runs {
      flow.add_run(*node, groups, text, style_at(*size));
    }

    flow
  }

  /// Each node's rectangle from one layout of a page, a node laid out in several
  /// spanning them all.
  #[derive(Default)]
  struct Rects(HashMap<NodeId, Rect<f32>>);

  impl Rects {
    /// The rectangle `node` spans; `None` for a node the layout never reached.
    fn rect(&self, node: NodeId) -> Option<Rect<f32>> {
      self.0.get(&node).copied()
    }

    /// Records that `node` covers `rect`, spanning everything it covered before.
    fn add(&mut self, node: NodeId, rect: Rect<f32>) {
      let spanned = self.0.get(&node).map_or(rect, |held| {
        let min = Point::new(
          held.origin.x.min(rect.origin.x),
          held.origin.y.min(rect.origin.y),
        );
        let max = Point::new(
          (held.origin.x + held.size.width).max(rect.origin.x + rect.size.width),
          (held.origin.y + held.size.height).max(rect.origin.y + rect.size.height),
        );

        Rect {
          origin: min,
          size: Size::new(max.x - min.x, max.y - min.y),
        }
      });

      self.0.insert(node, spanned);
    }
  }

  /// A page `width` wide and 768 tall laying its items out down it, each a text leaf or
  /// a row holding one, with the view each inline box holds.
  struct Page {
    tree: Tree<TextFlow<ParleyEngine>>,
    viewport: NodeId,
    width: f32,
    items: Vec<(NodeId, NodeId)>,
    views: HashMap<NodeId, NodeId>,
  }

  impl Page {
    /// An empty page `width` wide.
    fn new(width: f32) -> Self {
      let mut tree = Tree::new(false);
      let viewport = node();

      tree.insert(viewport);

      tree.set_style(
        viewport,
        Style {
          display: taffy::style::Display::Flex,
          flex_direction: FlexDirection::Column,
          size: taffy::geometry::Size {
            width: length(width),
            height: length(768.),
          },
          ..Style::default()
        },
      );

      Self {
        tree,
        viewport,
        width,
        items: Vec::new(),
        views: HashMap::new(),
      }
    }

    /// Adds the leaf `leaf` holding `flow` as the page's next item, inside a row of its
    /// own where `row` is given.
    fn add(&mut self, leaf: NodeId, flow: TextFlow<ParleyEngine>, row: Option<NodeId>) {
      self.tree.insert(leaf);
      self.tree.set_context(leaf, Some(flow));

      if let Some(row) = row {
        self.tree.insert(row);

        self.tree.set_style(
          row,
          Style {
            flex_direction: FlexDirection::Row,
            ..Style::default()
          },
        );

        self.tree.set_children(row, [leaf]);
      }

      self.items.push((row.unwrap_or(leaf), leaf));
    }

    /// Adds the inline box `inline` flowing in `leaf`, holding `view`, a box of `size`,
    /// and gives the leaf the size it lays out at, as its host does.
    fn add_box(&mut self, leaf: NodeId, inline: NodeId, view: NodeId, size: Size<f32>) {
      self.tree.insert(inline);

      self.tree.set_style(
        inline,
        Style {
          display: taffy::style::Display::Flex,
          ..Style::default()
        },
      );

      self.tree.insert(view);

      self.tree.set_style(
        view,
        Style {
          size: taffy::geometry::Size {
            width: length(size.width),
            height: length(size.height),
          },
          ..Style::default()
        },
      );

      self.tree.set_children(inline, [view]);
      self.views.insert(inline, view);

      self.tree.compute(
        inline,
        taffy::geometry::Size {
          width: AvailableSpace::MaxContent,
          height: AvailableSpace::MaxContent,
        },
      );

      let laid = self
        .tree
        .layout(inline)
        .map(|layout| Size::new(layout.size.width, layout.size.height))
        .expect("the box laid out");
      let flow = self.tree.context_mut(leaf).expect("the leaf's flow");

      flow.add_box(inline, &[], Size::new(0., 0.));
      crate::Leaf::size_box(flow, inline, laid);
      self.tree.mark_dirty(leaf);
    }

    /// Lays the page out and returns every node's rectangle: each item's and leaf's
    /// own, and each run's, group's and inline box's where its leaf places it.
    fn lay_out(&mut self) -> Rects {
      self
        .tree
        .set_children(self.viewport, self.items.iter().map(|(item, _)| *item));

      self.tree.compute(
        self.viewport,
        taffy::geometry::Size {
          width: AvailableSpace::Definite(self.width),
          height: AvailableSpace::Definite(768.),
        },
      );

      let mut rects = Rects::default();

      for (item, leaf) in self.items.clone() {
        let content = self.content(leaf);

        rects.add(item, self.laid_out(item, Point::ZERO));
        rects.add(leaf, content);

        let flow = self.tree.context(leaf).expect("the leaf's flow");

        for (node, rect) in crate::Leaf::place(flow, content) {
          if node == leaf {
            continue;
          }

          if !self.tree.contains(node) {
            rects.add(node, rect);

            continue;
          }

          let size = self.laid_out(node, Point::ZERO).size;

          rects.add(
            node,
            Rect {
              origin: rect.origin,
              size,
            },
          );

          if let Some(view) = self.views.get(&node) {
            rects.add(*view, self.laid_out(*view, rect.origin));
          }
        }
      }

      rects
    }

    /// The rectangle `node` lays out at in its parent, its parent's origin at `origin`.
    fn laid_out(&self, node: NodeId, origin: Point<f32>) -> Rect<f32> {
      let layout = self.tree.layout(node).expect("laid out");

      Rect {
        origin: Point::new(origin.x + layout.location.x, origin.y + layout.location.y),
        size: Size::new(layout.size.width, layout.size.height),
      }
    }

    /// The rectangle the leaf `leaf` lays out at on the page, its location summed through
    /// the row holding it where there is one.
    fn content(&self, leaf: NodeId) -> Rect<f32> {
      let item = self
        .items
        .iter()
        .find_map(|(item, held)| (*held == leaf).then_some(*item))
        .expect("the page's leaf");
      let origin = if item == leaf {
        Point::ZERO
      }
      else {
        self.laid_out(item, Point::ZERO).origin
      };

      self.laid_out(leaf, origin)
    }

    /// The run of the leaf `leaf` drawn at `point`; `None` beside every run.
    fn run_at(&self, leaf: NodeId, point: Point<f32>) -> Option<NodeId> {
      let flow = self.tree.context(leaf).expect("the leaf's flow");

      flow.run_at(self.content(leaf), point).map(|hit| hit.node)
    }
  }

  /// A paragraph's spans flow on one line, each where its text is, a group spanning its
  /// own, and a point finds the span drawn there.
  #[test]
  fn a_paragraphs_spans_flow_on_one_line() {
    let (text, hello, group, wor, ld) = (node(), node(), node(), node(), node());
    let mut page = Page::new(1024.);

    page.add(
      text,
      paragraph(&[
        (hello, &[], "Hello ", 16.),
        (wor, &[group], "wor", 16.),
        (ld, &[group], "ld", 16.),
      ]),
      None,
    );

    let rects = page.lay_out();
    let start = width("Hello ");
    let end = start + width("world");
    let middle = end - width("ld");

    assert_near(rects.rect(text), rect(0., 0., 1024., 24.));
    assert_near(rects.rect(hello), rect(0., 0., start, 24.));
    assert_near(rects.rect(group), rect(start, 0., end - start, 24.));
    assert_near(rects.rect(wor), rect(start, 0., middle - start, 24.));
    assert_near(rects.rect(ld), rect(middle, 0., width("ld"), 24.));
    assert_eq!(page.run_at(text, Point::new(start + 2., 10.)), Some(wor));

    assert_eq!(
      page.run_at(text, Point::new(200., 10.)),
      None,
      "beyond the text"
    );
  }

  /// A paragraph wraps across its spans, and a span broken across lines covers each of
  /// its pieces, spanning them, but takes a hit only where a piece is.
  #[test]
  fn a_paragraph_wraps_across_its_spans() {
    let (text, before, broken) = (node(), node(), node());
    let narrow = width("c aaa");
    let mut page = Page::new(narrow);

    page.add(
      text,
      paragraph(&[(before, &[], "c ", 16.), (broken, &[], "aaa bb", 16.)]),
      None,
    );

    let rects = page.lay_out();
    let start = width("c ");

    assert_near(rects.rect(text), rect(0., 0., narrow, 48.));
    assert_near(rects.rect(before), rect(0., 0., start, 24.));
    assert_near(rects.rect(broken), rect(0., 0., narrow, 48.));

    assert_eq!(
      page.run_at(text, Point::new(start + 2., 10.)),
      Some(broken),
      "its first piece"
    );

    assert_eq!(
      page.run_at(text, Point::new(2., 30.)),
      Some(broken),
      "its second piece"
    );

    assert_eq!(
      page.run_at(text, Point::new(width("bb") + 4., 30.)),
      None,
      "beside its second piece"
    );
  }

  /// A line is as tall as its tallest span, a smaller span sitting on its baseline.
  #[test]
  fn a_line_is_as_tall_as_its_tallest_span() {
    let (text, big, small) = (node(), node(), node());
    let mut page = Page::new(1024.);

    page.add(
      text,
      paragraph(&[(big, &[], "big ", 32.), (small, &[], "small", 16.)]),
      None,
    );

    let rects = page.lay_out();
    let start = width_at("big ", 32.);

    assert_near(rects.rect(text), rect(0., 0., 1024., 48.));
    assert_near(rects.rect(big), rect(0., 0., start, 48.));

    assert_near(
      rects.rect(small),
      rect(
        start,
        baseline(48., 32.) - baseline(24., 16.),
        width("small"),
        24.,
      ),
    );
  }

  /// An inline box lays out at its content's size among its paragraph's text, its line
  /// as tall as the box, and a point on it finds no run but lies in the view inside it.
  #[test]
  fn an_inline_box_flows_among_its_paragraphs_text() {
    let (text, before, inline, view, after) = (node(), node(), node(), node(), node());
    let mut page = Page::new(1024.);

    page.add(text, paragraph(&[(before, &[], "ab ", 16.)]), None);
    page.add_box(text, inline, view, Size::new(20., 30.));
    page
      .tree
      .context_mut(text)
      .expect("the leaf's flow")
      .add_run(after, &[], " cd", style());

    let rects = page.lay_out();
    let start = width("ab ");
    let top = 30. - baseline(24., 16.);
    let descent = 24. - baseline(24., 16.);

    assert_near(rects.rect(text), rect(0., 0., 1024., 30. + descent));
    assert_near(rects.rect(before), rect(0., top, start, 24.));
    assert_near(rects.rect(inline), rect(start, 0., 20., 30.));
    assert_near(rects.rect(view), rect(start, 0., 20., 30.));

    assert_near(rects.rect(after), rect(start + 20., top, width(" cd"), 24.));

    let on_view = Point::new(start + 6., 10.);

    assert_eq!(
      page.run_at(text, on_view),
      None,
      "no run is drawn on the box"
    );

    assert!(
      rects
        .rect(view)
        .is_some_and(|view| TextFlow::<ParleyEngine>::holds(view, on_view)),
      "the view inside it holds the point"
    );
  }

  /// A line may end before an inline box with no blank between them.
  #[test]
  fn a_line_may_end_before_an_inline_box() {
    let (text, run, inline, view) = (node(), node(), node(), node());
    let mut page = Page::new(30.);

    page.add(text, paragraph(&[(run, &[], "abc", 16.)]), None);
    page.add_box(text, inline, view, Size::new(20., 30.));

    let rects = page.lay_out();
    let descent = 24. - baseline(24., 16.);

    assert_near(rects.rect(text), rect(0., 0., 30., 24. + 30. + descent));
    assert_near(rects.rect(inline), rect(0., 24., 20., 30.));
  }

  /// A paragraph breaks where the Unicode line breaking algorithm allows, after a
  /// hyphen as after a blank.
  #[test]
  fn a_paragraph_breaks_after_a_hyphen() {
    let (text, span) = (node(), node());
    let mut page = Page::new(56.);

    page.add(text, paragraph(&[(span, &[], "well-known", 16.)]), None);

    let rects = page.lay_out();

    assert_near(rects.rect(text), rect(0., 0., 56., 48.));
    assert_near(rects.rect(span), rect(0., 0., width("known"), 48.));
  }

  /// A run's letter spacing widens each of its clusters by it.
  #[test]
  fn letter_spacing_widens_each_cluster() {
    let (text, row) = (node(), node());
    let mut page = Page::new(1024.);
    let mut flow = TextFlow::new(engine(), style(), ParagraphStyle::default());

    flow.add_run(text, &[], "abc", style().with_letter_spacing(2.));
    page.add(text, flow, Some(row));

    let rects = page.lay_out();

    assert_eq!(
      rects.rect(text).map(|rect| rect.size.width),
      Some(width("abc") + 6.)
    );
  }

  /// A text keeps its blanks as CSS's `white-space: pre-wrap` keeps them: the blanks
  /// starting or ending the text take their room.
  #[test]
  fn text_keeps_its_blanks() {
    let (trailing, leading) = (node(), node());
    let mut page = Page::new(1024.);

    page.add(
      trailing,
      paragraph(&[(trailing, &[], "ab   ", 16.)]),
      Some(node()),
    );

    page.add(
      leading,
      paragraph(&[(leading, &[], "   ab", 16.)]),
      Some(node()),
    );

    let rects = page.lay_out();
    let kept = width("ab") + 3. * width(" ");

    for run in [trailing, leading] {
      assert!(
        rects
          .rect(run)
          .is_some_and(|rect| (rect.size.width - kept).abs() < 1e-3),
        "{:?} takes its three blanks' room",
        rects.rect(run)
      );
    }
  }

  /// A newline breaks its line, a newline ending the text opens a line, and an empty text
  /// lays out one.
  #[test]
  fn a_newline_breaks_its_line_and_a_final_one_opens_another() {
    let mut page = Page::new(1024.);
    let runs = ["a", "a\n", "a\n\n", ""].map(|text| {
      let run = node();

      page.add(run, paragraph(&[(run, &[], text, 16.)]), Some(node()));

      run
    });
    let rects = page.lay_out();

    assert_eq!(
      runs.map(|run| rects.rect(run).map(|rect| rect.size.height)),
      [Some(24.), Some(48.), Some(72.), Some(24.)]
    );
  }

  /// The blanks ending a line where it wraps take no room on it.
  #[test]
  fn blanks_hang_where_a_line_wraps() {
    let hung = node();
    let mut page = Page::new(40.);

    page.add(hung, paragraph(&[(hung, &[], "aaa     bbb", 16.)]), None);

    let rects = page.lay_out();

    assert_eq!(rects.rect(hung).map(|rect| rect.size.height), Some(48.));
  }

  /// A word too wide for a line of its own breaks where the line fills, though its
  /// min-content width, which a row's item keeps, is the widest word's.
  #[test]
  fn a_word_too_wide_for_its_line_breaks_where_it_fills() {
    let (broken, kept) = (node(), node());
    let mut page = Page::new(40.);

    page.add(
      broken,
      paragraph(&[(broken, &[], "aaaaaaaaaaaaaaaaaaaa", 16.)]),
      None,
    );

    page.add(
      kept,
      paragraph(&[(kept, &[], "aa aaaaaaaaaa b", 16.)]),
      Some(node()),
    );

    let rects = page.lay_out();

    assert!(
      rects
        .rect(broken)
        .is_some_and(|rect| (rect.size.height - 5. * 24.).abs() < 1e-4),
      "five lines of four letters each"
    );

    assert_eq!(
      rects.rect(kept).map(|rect| rect.size.width),
      Some(width("aaaaaaaaaa"))
    );
  }

  /// A fresh id to key the layout by, each call's distinct from every other's.
  fn node() -> NodeId {
    static NEXT: AtomicU32 = AtomicU32::new(0);

    NodeId::new(NEXT.fetch_add(1, Ordering::Relaxed), 0)
  }

  /// A tree holding `parent` styled `style`, its children each a text leaf of the text
  /// beside it.
  fn tree(
    parent: NodeId,
    style: Style,
    children: &[(NodeId, &str)],
  ) -> Tree<TextFlow<ParleyEngine>> {
    let mut tree = Tree::new(false);

    tree.insert(parent);
    tree.set_style(parent, style);

    for (child, text) in children {
      tree.insert(*child);
      tree.set_context(*child, Some(flow(*child, text)));
    }

    tree.set_children(parent, children.iter().map(|(child, _)| *child));

    tree
  }

  #[test]
  fn a_change_of_paint_alone_repaints_without_laying_out_again() {
    use skene_foundation::{Color, text::PaintStyle};

    let (column, text) = (node(), node());
    let mut tree = tree(column, Style::default(), &[(text, "alpha beta")]);
    let space = taffy::geometry::Size {
      width: AvailableSpace::MaxContent,
      height: AvailableSpace::MaxContent,
    };

    tree.compute(column, space);

    let laid = tree.context(text).expect("a flow").leaf().laid(None);
    let red = PaintStyle::new(Color::oklch(0.6, 0.2, 30.));
    let mut repainted = TextFlow::new(engine(), style(), ParagraphStyle::default());

    repainted.add_run(text, &[], "alpha beta", style().with_paint(red));

    assert!(
      !tree.set_context(text, Some(repainted)),
      "nothing it measures to changed"
    );

    assert!(!tree.is_dirty(text), "the leaf is not measured again");

    assert_eq!(
      laid.paint_at(0),
      red,
      "the layout handed out shows the paint"
    );

    let mut resized = TextFlow::new(engine(), style(), ParagraphStyle::default());

    resized.add_run(text, &[], "alpha beta", TextStyle::new(20.).with_paint(red));

    assert!(tree.set_context(text, Some(resized)));
    assert!(tree.is_dirty(text), "a change of size lays out again");
  }

  #[test]
  fn a_row_shrinks_text_leaves_and_they_wrap() {
    let (row, first, second) = (node(), node(), node());
    let mut tree = tree(
      row,
      Style {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::START,
        ..Style::default()
      },
      &[(first, "alpha beta"), (second, "gamma delta")],
    );

    let size = |tree: &Tree<_>, node| {
      let layout: &taffy::tree::Layout = tree.layout(node).expect("laid out");

      (layout.location.x, layout.size.width, layout.size.height)
    };

    tree.compute(
      row,
      taffy::geometry::Size {
        width: AvailableSpace::MinContent,
        height: AvailableSpace::MaxContent,
      },
    );

    assert_eq!(
      tree.layout(row).map(|layout| layout.size.width),
      Some(width("alpha").max(width("beta")) + width("gamma").max(width("delta")))
    );

    tree.compute(
      row,
      taffy::geometry::Size {
        width: AvailableSpace::MaxContent,
        height: AvailableSpace::MaxContent,
      },
    );

    assert_eq!(
      tree.layout(row).map(|layout| layout.size.width),
      Some(width("alpha beta") + width("gamma delta"))
    );

    let narrow = (width("alpha beta") + width("gamma delta")) * 0.75;

    tree.set_style(
      row,
      Style {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::START,
        size: taffy::geometry::Size {
          width: length(narrow),
          height: taffy::style::Dimension::auto(),
        },
        ..Style::default()
      },
    );

    tree.compute(
      row,
      taffy::geometry::Size {
        width: AvailableSpace::Definite(narrow),
        height: AvailableSpace::MaxContent,
      },
    );

    let ((first_x, first_width, first_height), (second_x, second_width, second_height)) =
      (size(&tree, first), size(&tree, second));

    assert_eq!(
      (first_x, first_height, second_height),
      (0., 48., 48.),
      "two lines each"
    );

    assert_eq!(second_x, first_width);

    assert!(
      (first_width + second_width - narrow).abs() < 1.,
      "{first_width} + {second_width}"
    );
  }

  #[test]
  fn a_box_takes_its_texts_lines_under_its_padding() {
    let (boxed, text) = (node(), node());
    let mut tree = tree(
      boxed,
      Style {
        flex_direction: FlexDirection::Column,
        padding: taffy::geometry::Rect {
          left: length(0.),
          right: length(0.),
          top: length(10.),
          bottom: length(0.),
        },
        size: taffy::geometry::Size {
          width: length(width("alpha beta")),
          height: taffy::style::Dimension::auto(),
        },
        ..Style::default()
      },
      &[(text, "alpha beta gamma")],
    );

    tree.compute(
      boxed,
      taffy::geometry::Size {
        width: AvailableSpace::MaxContent,
        height: AvailableSpace::MaxContent,
      },
    );

    assert_eq!(
      tree.layout(boxed).map(|layout| layout.size.height),
      Some(58.)
    );

    assert_eq!(
      tree
        .layout(text)
        .map(|layout| (layout.location.y, layout.size.height)),
      Some((10., 48.)),
      "two lines under the padding"
    );
  }

  #[test]
  fn a_point_finds_the_run_drawn_there_and_the_nearest_beside_the_text() {
    let mut flow = TextFlow::new(engine(), style(), ParagraphStyle::default());

    flow.add_run(1_u8, &[], "Hello ", style());
    flow.add_box(2, &[], Size::new(20., 10.));
    flow.add_run(3, &[], "world", style());

    let content = Rect::new(Point::new(10., 20.), Size::new(500., 24.));
    let start = 10. + width("Hello ") + 20.;
    let hit = |x: f32, y: f32| flow.run_at(content, Point::new(x, y));

    assert_eq!(hit(12., 30.), Some(RunHit { node: 1, offset: 0 }));
    assert_eq!(hit(start + 1., 30.), Some(RunHit { node: 3, offset: 0 }));

    assert_eq!(
      hit(start + width("world") - 1., 30.),
      Some(RunHit { node: 3, offset: 5 })
    );

    assert_eq!(hit(start - 10., 30.), None, "on the box");
    assert_eq!(hit(400., 30.), None, "beside the text");
    assert_eq!(hit(12., 50.), None, "below the text");

    assert_eq!(
      flow.nearest(content, Point::new(400., 30.)),
      Some(RunHit { node: 3, offset: 5 })
    );

    assert_eq!(
      flow.nearest(content, Point::new(0., 0.)),
      Some(RunHit { node: 1, offset: 0 })
    );

    let mut wrapped = TextFlow::new(engine(), style(), ParagraphStyle::default());

    wrapped.add_run(1_u8, &[], "alpha ", style());
    wrapped.add_run(2, &[], "beta", style());

    let narrow = Rect::new(Point::ZERO, Size::new(width("alpha") + 2., 48.));

    assert_eq!(
      wrapped.nearest(narrow, Point::new(narrow.size.width + 50., 12.)),
      Some(RunHit { node: 1, offset: 6 }),
      "past the end of a wrapped line, the run that line ends with"
    );

    assert_eq!(
      wrapped.nearest(narrow, Point::new(-5., 36.)),
      Some(RunHit { node: 2, offset: 0 }),
      "before the start of the next, the run it starts with"
    );

    assert_eq!(
      wrapped.run_at(narrow, Point::new(2., 36.)),
      Some(RunHit { node: 2, offset: 0 })
    );
  }
}
