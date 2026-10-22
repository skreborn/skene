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
///
/// Trimmed, its first line is cut to the highest cap height on it and its last to its
/// baseline, as CSS's `text-box-trim: trim-both` with `text-box-edge: cap alphabetic` trims a
/// block's text.
#[must_use]
pub struct TextFlow<E: TextEngine, N = NodeId> {
  engine: E,
  base: TextStyle,
  style: ParagraphStyle,
  cap: Option<f32>,
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
      .field("cap", &self.cap)
      .field("text", &self.text)
      .field("items", &self.items)
      .finish_non_exhaustive()
  }
}

impl<E: TextEngine, N: PartialEq> PartialEq for TextFlow<E, N> {
  fn eq(&self, other: &Self) -> bool {
    self.base == other.base
      && self.style == other.style
      && self.cap.map(f32::to_bits) == other.cap.map(f32::to_bits)
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
      cap: None,
      text: String::new(),
      items: Vec::new(),
      leaf: OnceCell::new(),
    }
  }

  /// The flow trimmed: its first line cut to the highest cap height on it, each run's
  /// `cap` times its font size and each box's its height, the base style's where it holds
  /// neither, and its last line to its baseline.
  pub fn trimmed(self, cap: f32) -> Self {
    Self {
      cap: Some(cap),
      leaf: OnceCell::new(),
      ..self
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
  /// its trim taken off above the content box's top.
  pub fn layout_in(&self, content: Rect<f32>) -> (<E::Paragraph as Paragraph>::Laid, Point<f32>) {
    let laid = self.leaf().laid(Some(content.size.width));
    let lines = laid.lines();
    let shift = lines.first().map_or(0., |first| first.top) + self.trims(&laid, &lines).0;

    (laid, Point::new(content.origin.x, content.origin.y - shift))
  }

  /// The run whose text holds byte `at` of the flow's text.
  #[must_use]
  pub fn node_at(&self, at: usize) -> Option<&N> {
    self
      .runs()
      .find_map(|(run, _)| run.range.contains(&at).then_some(&run.node))
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

  /// How far `laid`'s first line is trimmed at its top and its last at its bottom, its
  /// lines `lines`.
  fn trims(&self, laid: &<E::Paragraph as Paragraph>::Laid, lines: &[LineInfo]) -> (f32, f32) {
    let (Some(cap), Some(first), Some(last)) = (self.cap, lines.first(), lines.last())
    else {
      return (0., 0.);
    };
    let highest = self
      .pieces(laid)
      .into_iter()
      .filter(|(_, rect)| Self::line_of(lines, *rect) == Some(0))
      .map(|(item, rect)| match &self.items[item] {
        Item::Run(run) => cap * run.style.size,
        Item::Box(_) => rect.size.height,
      })
      .reduce(f32::max)
      .unwrap_or(cap * self.base.size);

    (
      (first.baseline - first.top - highest).max(0.),
      last.top + last.height - last.baseline,
    )
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
    let leaf = self.leaf();
    let measured = leaf.measure(
      (known.width, known.height),
      (space(available.width), space(available.height)),
    );
    let height = match (known.height, self.cap) {
      (None, Some(_)) => {
        let within = match (known.width, available.width) {
          (Some(width), _) | (None, AvailableSpace::Definite(width)) => Some(width),
          (None, AvailableSpace::MinContent) => Some(leaf.intrinsic().min),
          (None, AvailableSpace::MaxContent) => None,
        };
        let laid = leaf.sized(within);
        let (top, bottom) = self.trims(&laid, &laid.lines());

        laid.size().height - top - bottom
      }
      _ => measured.height,
    };

    Size::new(measured.width, height)
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
      && self.cap.map(f32::to_bits) == next.cap.map(f32::to_bits)
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
  use skene_foundation::{
    NodeId,
    NodeKind,
    Point,
    Rect,
    Scene,
    Size,
    text::{Laid as _, ParagraphStyle, TextStyle},
  };
  use skene_headless::Headless;
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
    include_bytes!("../../headless/fonts/inter-regular.otf"),
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

  /// A node of the scene, to key the layout by.
  fn node() -> NodeId {
    Scene::<Headless>::with(|scene| scene.create(NodeKind::Box, None))
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
    assert_eq!(flow.node_at(7), Some(&3));

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
