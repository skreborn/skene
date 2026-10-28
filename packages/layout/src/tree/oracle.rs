//! [`Tree`] held to one `TaffyTree` receiving the same writes, over random trees shaped as
//! a scene's boxes, controls, overlays and paragraphs lay out, and random runs of writes.

use std::{
  cell::RefCell,
  collections::{BTreeMap, HashMap},
  rc::Rc,
};

use proptest::{
  prelude::*,
  test_runner::{Config as ProptestConfig, RngSeed},
};
use skene_foundation::{
  Arrangement,
  AspectRatio,
  Axes,
  Axis,
  BoxStyle,
  Dimension,
  Flexibility,
  GridFlow,
  GridPlacement,
  GridTemplate,
  GridTrack,
  Inset,
  ItemAlignment,
  Layout,
  Line,
  NodeId,
  Overflow,
  Plain,
  Spacing,
  TrackLimit,
  Wrap,
};
use taffy::{
  TaffyTree,
  prelude::AvailableSpace,
  style::{Display, Position, Style},
  tree::LayoutOutput,
};

use super::{Leaf, Tree};
use crate::{Borders, BoxLayout, Place};

/// How many times each leaf was measured since the tally was last cleared, by node.
type Tally = Rc<RefCell<BTreeMap<NodeId, u32>>>;

/// A leaf laid out as a wrapped text: words of fixed widths a space apart, broken
/// greedily at the width it is given, each line `line` high; every measure is tallied.
#[derive(Clone, Debug)]
struct Canned {
  node: NodeId,
  words: Vec<f32>,
  line: f32,
  paint: u8,
  tally: Tally,
}

impl PartialEq for Canned {
  fn eq(&self, other: &Self) -> bool {
    self.node == other.node
      && self.words == other.words
      && self.line.to_bits() == other.line.to_bits()
      && self.paint == other.paint
  }
}

impl Leaf for Canned {
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> skene_foundation::Size<f32> {
    *self.tally.borrow_mut().entry(self.node).or_default() += 1;

    let limit = known.width.unwrap_or(match available.width {
      AvailableSpace::Definite(width) => width,
      AvailableSpace::MinContent => 0.,
      AvailableSpace::MaxContent => f32::INFINITY,
    });
    let mut widest = 0_f32;
    let mut lines = 0_u16;
    let mut current: Option<f32> = None;

    for word in &self.words {
      current = match current {
        Some(width) if width + 1. + word <= limit => Some(width + 1. + word),
        Some(width) => {
          widest = widest.max(width);
          lines += 1;

          Some(*word)
        }
        None => Some(*word),
      };
    }

    if let Some(width) = current {
      widest = widest.max(width);
      lines += 1;
    }

    skene_foundation::Size::new(
      known.width.unwrap_or(widest),
      known.height.unwrap_or(f32::from(lines.max(1)) * self.line),
    )
  }

  fn repaint_as(&mut self, next: &Self) -> bool {
    if self.node != next.node
      || self.words != next.words
      || self.line.to_bits() != next.line.to_bits()
    {
      return false;
    }

    self.paint = next.paint;

    true
  }
}

/// One taffy tree receiving every write as [`Tree`]'s contract says it applies: a static
/// position made relative, children outside the tree left out, and a node marked only
/// where a value differs from the one it holds.
struct Oracle {
  taffy: TaffyTree<Canned>,
  nodes: HashMap<NodeId, taffy::NodeId>,
}

impl Oracle {
  fn new(rounding: bool) -> Self {
    let mut taffy = TaffyTree::new();

    if !rounding {
      taffy.disable_rounding();
    }

    Self {
      taffy,
      nodes: HashMap::new(),
    }
  }

  fn relative(mut style: Style) -> Style {
    if style.position == Position::Static {
      style.position = Position::Relative;
    }

    style
  }

  fn insert(&mut self, node: NodeId) {
    if self.nodes.contains_key(&node) {
      return;
    }

    let id = self
      .taffy
      .new_leaf(Self::relative(Style::default()))
      .expect("allocate an oracle node");

    self.nodes.insert(node, id);
  }

  fn remove(&mut self, node: NodeId) {
    if let Some(id) = self.nodes.remove(&node) {
      self.taffy.remove(id).expect("remove an oracle node");
    }
  }

  fn set_style(&mut self, node: NodeId, style: Style) -> bool {
    let Some(&id) = self.nodes.get(&node)
    else {
      return false;
    };
    let style = Self::relative(style);

    if *self.taffy.style(id).expect("an oracle node's style") == style {
      return false;
    }

    self
      .taffy
      .set_style(id, style)
      .expect("style an oracle node");

    true
  }

  fn set_context(&mut self, node: NodeId, context: Option<Canned>) -> bool {
    let Some(&id) = self.nodes.get(&node)
    else {
      return false;
    };

    if self.taffy.get_node_context(id) == context.as_ref() {
      return false;
    }

    if let (Some(held), Some(next)) = (self.taffy.get_node_context_mut(id), &context)
      && held.repaint_as(next)
    {
      return false;
    }

    self
      .taffy
      .set_node_context(id, context)
      .expect("give an oracle node its context");

    true
  }

  fn mark_dirty(&mut self, node: NodeId) {
    if let Some(&id) = self.nodes.get(&node) {
      self.taffy.mark_dirty(id).expect("mark an oracle node");
    }
  }

  fn set_children(&mut self, node: NodeId, children: &[NodeId]) -> bool {
    let children: Vec<taffy::NodeId> = children
      .iter()
      .filter_map(|child| self.nodes.get(child).copied())
      .collect();
    let Some(&id) = self.nodes.get(&node)
    else {
      return false;
    };

    if self.taffy.children(id).expect("an oracle node's children") == children {
      return false;
    }

    self
      .taffy
      .set_children(id, &children)
      .expect("give an oracle node its children");

    true
  }

  fn compute(&mut self, node: NodeId, available: taffy::geometry::Size<AvailableSpace>) {
    let Some(&id) = self.nodes.get(&node)
    else {
      return;
    };

    self
      .taffy
      .compute_layout_with_measure(id, available, |input, _, context, _| {
        let known = input.known_dimensions;
        let size = context.map_or(
          skene_foundation::Size::new(
            known.width.unwrap_or_default(),
            known.height.unwrap_or_default(),
          ),
          |context| context.measure(known, input.available_space),
        );

        LayoutOutput::from_outer_size(taffy::geometry::Size {
          width: size.width,
          height: size.height,
        })
      })
      .expect("lay the oracle out");
  }

  /// `node`'s parent, `None` for a node outside the tree or with none.
  fn parent(&self, node: NodeId) -> Option<NodeId> {
    let parent = self.taffy.parent(*self.nodes.get(&node)?)?;

    self
      .nodes
      .iter()
      .find_map(|(scene, id)| (*id == parent).then_some(*scene))
  }

  /// `node`'s children, in order; none for a node outside the tree.
  fn children(&self, node: NodeId) -> Vec<NodeId> {
    let Some(&id) = self.nodes.get(&node)
    else {
      return Vec::new();
    };
    let children = self.taffy.children(id).expect("an oracle node's children");

    children
      .into_iter()
      .filter_map(|child| {
        self
          .nodes
          .iter()
          .find_map(|(scene, id)| (*id == child).then_some(*scene))
      })
      .collect()
  }
}

/// One node's genes: each byte picks one of its properties, a low byte the plainest
/// choice.
type Gene = [u8; 30];

/// The residue `byte` leaves above the `chance` highest values, `None` below them.
const fn chosen(byte: u8, chance: u8) -> Option<u8> {
  byte.checked_sub(u8::MAX - chance)
}

/// A dimension: auto, a whole length, a fractional length, a fraction, a negative length
/// or a fraction past one.
fn dimension(residue: u8) -> Dimension<f32> {
  let step = f32::from(residue / 6);

  match residue % 6 {
    0 => Dimension::Auto,
    1 => Dimension::Length(step),
    2 => Dimension::Length(step / 3.),
    3 => Dimension::Fraction(f32::from(residue / 6 % 11) / 10.),
    4 => Dimension::Length(-step),
    _ => Dimension::Fraction(1. + step / 8.),
  }
}

/// A length a padding, border or spacing takes: whole, fractional or negative.
fn extent(residue: u8) -> f32 {
  match residue % 4 {
    0 => 0.,
    1 => f32::from(residue / 4 % 4),
    2 => f32::from(residue / 4 % 5) / 2.5,
    _ => -1.,
  }
}

/// One of the tracks a template or an implicit list takes.
fn track(pick: u8) -> GridTrack<f32> {
  match pick % 10 {
    0 => GridTrack::Auto,
    1 => GridTrack::Length(f32::from(pick / 10 % 7) + 0.5),
    2 => GridTrack::Fraction(0.25),
    3 => GridTrack::Flex(f32::from(pick / 10 % 3) + 1.),
    4 => GridTrack::MinContent,
    5 => GridTrack::MaxContent,
    6 => GridTrack::FitContent(4.),
    7 => GridTrack::Between(TrackLimit::Length(2.), TrackLimit::Flex(1.)),
    8 => GridTrack::Between(TrackLimit::MinContent, TrackLimit::Auto),
    _ => GridTrack::Between(TrackLimit::Fraction(0.1), TrackLimit::MaxContent),
  }
}

/// A template of one to four tracks.
fn template(residue: u8) -> GridTemplate<f32> {
  (0..=residue % 4).fold(GridTemplate::new(), |template, index| {
    template.add_track(track(residue / 4 + index * 3))
  })
}

/// One end of a placement.
const fn placement(pick: u8) -> GridPlacement {
  match pick % 4 {
    0 => GridPlacement::Auto,
    1 => GridPlacement::Line(pick as i16 / 4 % 3 + 1),
    2 => GridPlacement::Span(pick as u16 / 4 % 2 + 1),
    _ => GridPlacement::Line(-1),
  }
}

/// The alignment `pick` names.
const fn alignment(pick: u8) -> ItemAlignment {
  match pick % 5 {
    0 => ItemAlignment::Start,
    1 => ItemAlignment::End,
    2 => ItemAlignment::Center,
    3 => ItemAlignment::Baseline,
    _ => ItemAlignment::Stretch,
  }
}

/// The arrangement `pick` names.
const fn arrangement(pick: u8) -> Arrangement {
  match pick % 7 {
    0 => Arrangement::Start,
    1 => Arrangement::End,
    2 => Arrangement::Center,
    3 => Arrangement::Stretch,
    4 => Arrangement::SpaceBetween,
    5 => Arrangement::SpaceAround,
    _ => Arrangement::SpaceEvenly,
  }
}

/// The overflow `pick` names.
const fn overflow(pick: u8) -> Overflow {
  match pick % 4 {
    0 => Overflow::Visible,
    1 => Overflow::Clip,
    2 => Overflow::Scroll,
    _ => Overflow::ScrollIfNeeded,
  }
}

/// The layout `gene` lays its box out as.
const fn layout(gene: &Gene) -> Layout {
  match gene[1] % 10 {
    0 | 1 => Layout::Single,
    2 => Layout::Flex {
      axis: Axis::Horizontal,
      reversed: false,
    },
    3 => Layout::Flex {
      axis: Axis::Vertical,
      reversed: false,
    },
    4 => Layout::Flex {
      axis: Axis::Horizontal,
      reversed: true,
    },
    5 => Layout::Flex {
      axis: Axis::Vertical,
      reversed: true,
    },
    6 => Layout::Grid,
    7 => Layout::Stack,
    _ => Layout::Document,
  }
}

/// The box style `gene` reads as.
#[expect(
  clippy::too_many_lines,
  reason = "one decode per property; length is the property count, not complexity"
)]
fn box_style(gene: &Gene) -> BoxStyle<f32, Plain> {
  let mut style = BoxStyle::<f32, Plain> {
    width: chosen(gene[2], 150).map(dimension),
    height: chosen(gene[3], 150).map(dimension),
    min_width: chosen(gene[4], 50).map(dimension),
    min_height: chosen(gene[5], 50).map(dimension),
    max_width: chosen(gene[6], 50).map(dimension),
    max_height: chosen(gene[7], 50).map(dimension),
    aspect_ratio: chosen(gene[8], 15).map(|residue| AspectRatio(f32::from(residue % 3) + 0.5)),
    ..BoxStyle::default()
  };

  style.margin = chosen(gene[9], 100).map(|residue| Inset {
    top: Dimension::Length(f32::from(residue % 4) - 1.),
    end: if residue % 5 == 0 {
      Dimension::Auto
    }
    else {
      Dimension::Length(f32::from(residue / 5 % 3))
    },
    bottom: Dimension::Length(f32::from(residue / 4 % 4)),
    start: if residue % 7 == 0 {
      Dimension::Fraction(0.1)
    }
    else {
      Dimension::Length(f32::from(residue / 7 % 2))
    },
  });

  style.padding = chosen(gene[10], 100).map(|residue| Inset {
    top: extent(residue),
    end: extent(residue / 2),
    bottom: extent(residue / 3),
    start: extent(residue.rotate_left(3)),
  });

  match chosen(gene[11], 60).map(|residue| (residue % 3, residue / 3)) {
    Some((0, residue)) => {
      style.offset = Some(Inset {
        top: dimension(residue),
        end: Dimension::Auto,
        bottom: Dimension::Auto,
        start: dimension(residue / 2),
      });
    }
    Some((_, residue)) => {
      style.pinned = Some(Inset {
        top: dimension(residue),
        end: if residue % 2 == 0 {
          Dimension::Auto
        }
        else {
          Dimension::Length(1.)
        },
        bottom: Dimension::Auto,
        start: dimension(residue / 3),
      });
    }
    None => {}
  }

  style.border_width = chosen(gene[12], 80).map(|residue| Inset {
    top: f32::from(residue % 3),
    end: f32::from(residue / 3 % 3) / 2.,
    bottom: f32::from(residue / 9 % 3),
    start: f32::from(residue / 27 % 3),
  });

  style.spacing = chosen(gene[13], 100).map(|residue| Spacing {
    horizontal: extent(residue),
    vertical: extent(residue / 4),
  });

  style.wrap = chosen(gene[14], 80).map(|residue| match residue % 3 {
    0 => Wrap::None,
    1 => Wrap::Forward,
    _ => Wrap::Reverse,
  });

  style.flex = chosen(gene[15], 100).map(|residue| Flexibility {
    initial: dimension(residue / 9),
    grow_factor: f32::from(residue % 3),
    shrink_factor: f32::from(residue / 3 % 3),
  });

  if let Some(residue) = chosen(gene[16], 100) {
    style.arrangement = Some(arrangement(residue));
    style.line_arrangement = (residue % 2 == 0).then_some(arrangement(residue / 7));
  }

  if let Some(residue) = chosen(gene[17], 100) {
    style.item_alignment = Some(alignment(residue));
    style.item_horizontal_alignment = (residue % 2 == 0).then_some(alignment(residue / 5));
  }

  if let Some(residue) = chosen(gene[18], 60) {
    style.self_alignment = Some(alignment(residue));
    style.self_horizontal_alignment = (residue % 3 == 0).then_some(alignment(residue / 5));
  }

  style.grid_columns = chosen(gene[19], 120).map(template);
  style.grid_rows = chosen(gene[20], 80).map(template);

  if let Some(residue) = chosen(gene[21], 80) {
    style.grid_implicit_columns = Some(vec![track(residue)]);
    style.grid_implicit_rows =
      (residue % 2 == 0).then(|| vec![track(residue / 3), GridTrack::Auto]);
    style.grid_flow = Some(match residue % 4 {
      0 => GridFlow::Rows,
      1 => GridFlow::Columns,
      2 => GridFlow::DenseRows,
      _ => GridFlow::DenseColumns,
    });
  }

  if let Some(residue) = chosen(gene[22], 100) {
    style.grid_row_placement = Some(Line {
      start: placement(residue),
      end: placement(residue / 4),
    });
    style.grid_column_placement = (residue % 3 != 0).then_some(Line {
      start: placement(residue / 2),
      end: placement(residue / 8),
    });
  }

  style.overflow = chosen(gene[23], 60).map(|residue| Axes {
    horizontal: overflow(residue),
    vertical: overflow(residue / 4),
  });

  style
}

/// The leaf `gene` measures as, for `node`, its every measure tallied in `tally`.
fn canned(gene: &Gene, node: NodeId, tally: &Tally) -> Option<Canned> {
  let residue = chosen(gene[24], 140)?;

  Some(Canned {
    node,
    words: (0..=residue % 5)
      .map(|index| f32::from((residue / 5 + index * 5) % 7) * 0.75 + 1.)
      .collect(),
    line: if residue % 3 == 0 { 1.5 } else { 1. },
    paint: gene[25],
    tally: Rc::clone(tally),
  })
}

/// What a node is in the scene: a box in its parent's flow, an overlay above the root's
/// flow, or an inline box laid out on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
  Flow,
  Overlay,
  Inline,
}

/// A space a root lays out in.
fn viewport(pick: u8) -> taffy::geometry::Size<AvailableSpace> {
  taffy::geometry::Size {
    width: if pick.is_multiple_of(11) {
      AvailableSpace::MaxContent
    }
    else {
      AvailableSpace::Definite(12. + f32::from(pick % 8) * 7.5)
    },
    height: if pick.is_multiple_of(13) {
      AvailableSpace::MinContent
    }
    else {
      AvailableSpace::Definite(8. + f32::from(pick / 8 % 6) * 5.25)
    },
  }
}

/// The space an inline box lays out in on its own.
const MAX_CONTENT: taffy::geometry::Size<AvailableSpace> = taffy::geometry::Size {
  width: AvailableSpace::MaxContent,
  height: AvailableSpace::MaxContent,
};

/// A layout's every number as its bits, so two layouts compare bit for bit.
fn bits(layout: &taffy::tree::Layout) -> Vec<u32> {
  let rect = |rect: taffy::geometry::Rect<f32>| {
    [rect.left, rect.right, rect.top, rect.bottom].map(f32::to_bits)
  };

  [
    vec![
      layout.order,
      layout.location.x.to_bits(),
      layout.location.y.to_bits(),
      layout.size.width.to_bits(),
      layout.size.height.to_bits(),
      layout.scrollbar_size.width.to_bits(),
      layout.scrollbar_size.height.to_bits(),
    ],
    rect(layout.scrollable_overflow_rect).to_vec(),
    rect(layout.border).to_vec(),
    rect(layout.padding).to_vec(),
    rect(layout.margin).to_vec(),
  ]
  .concat()
}

/// The tree under test and the oracle, written alike, with what the harness knows of
/// each node to style it as the scene would.
struct Pair {
  tree: Tree<Canned>,
  oracle: Oracle,
  tree_tally: Tally,
  oracle_tally: Tally,
  borders: Borders,
  root: NodeId,
  roles: HashMap<NodeId, Role>,
  layouts: HashMap<NodeId, Layout>,
  ids: Vec<NodeId>,
  next: u32,
}

impl Pair {
  fn new(rounding: bool, borders: Borders) -> Self {
    Self {
      tree: Tree::new(rounding),
      oracle: Oracle::new(rounding),
      tree_tally: Tally::default(),
      oracle_tally: Tally::default(),
      borders,
      root: NodeId::new(0, 0),
      roles: HashMap::new(),
      layouts: HashMap::new(),
      ids: Vec::new(),
      next: 0,
    }
  }

  /// A fresh node id.
  fn fresh(&mut self) -> NodeId {
    let node = NodeId::new(self.next, 0);

    self.next += 1;
    self.ids.push(node);

    node
  }

  /// One of the ids the case has made, in the tree or not.
  fn pick(&self, pick: u8) -> NodeId {
    self.ids[usize::from(pick) % self.ids.len()]
  }

  /// Inserts `node` into both, created `decorated` or not.
  fn insert(&mut self, node: NodeId, decorated: bool) {
    let _ = decorated;

    self.tree.insert(node);
    self.oracle.insert(node);
  }

  fn remove(&mut self, node: NodeId) {
    self.tree.remove(node);
    self.oracle.remove(node);
  }

  /// Styles `node` from `gene` as the scene maps its role: placed in its parent's stack
  /// cell where its parent is a stack, an overlay detached and absolute, a hidden one not
  /// displayed.
  fn style(&mut self, node: NodeId, gene: &Gene, parent: Option<NodeId>) {
    let role = self.roles.get(&node).copied().unwrap_or(Role::Flow);
    let layout = layout(gene);
    let place = Place {
      in_stack: role == Role::Flow
        && parent.and_then(|parent| self.layouts.get(&parent)) == Some(&Layout::Stack),
      borders: self.borders,
    };
    let mut style = box_style(gene).taffy_style(layout, place);

    if role == Role::Overlay {
      style.position = Position::Absolute;
    }

    if gene[26] < 24 {
      style.display = Display::None;
    }

    self.layouts.insert(node, layout);

    let held = self.tree.set_style(node, style.clone());

    assert_eq!(
      held,
      self.oracle.set_style(node, style),
      "set_style's answer"
    );
  }

  /// Gives `node` the leaf `gene` reads as, or none.
  fn context(&mut self, node: NodeId, gene: &Gene) {
    let tree = canned(gene, node, &self.tree_tally);
    let oracle = canned(gene, node, &self.oracle_tally);
    let held = self.tree.set_context(node, tree);

    assert_eq!(
      held,
      self.oracle.set_context(node, oracle),
      "set_context's answer"
    );
  }

  fn set_children(&mut self, node: NodeId, children: &[NodeId]) {
    let held = self.tree.set_children(node, children.iter().copied());

    assert_eq!(
      held,
      self.oracle.set_children(node, children),
      "set_children's answer"
    );
  }

  /// Whether `node` is `ancestor` or beneath it.
  fn descends(&self, node: NodeId, ancestor: NodeId) -> bool {
    let mut current = Some(node);

    while let Some(at) = current {
      if at == ancestor {
        return true;
      }

      current = self.oracle.parent(at);
    }

    false
  }

  /// Builds the case's tree from `genes`, its first gene the root's: each other node in
  /// its parent's flow, above the root's flow as an overlay, or on its own as an inline
  /// box, as its genes pick.
  fn build(&mut self, genes: &[Gene]) {
    let mut parents: Vec<Option<usize>> = Vec::new();

    for (index, gene) in genes.iter().enumerate() {
      let node = self.fresh();
      let (role, parent) = if index == 0 {
        (Role::Flow, None)
      }
      else if gene[28] < 26 {
        (Role::Overlay, Some(0))
      }
      else if gene[0] > 240 {
        (Role::Inline, None)
      }
      else {
        (Role::Flow, Some(usize::from(gene[0]) % index))
      };

      self.roles.insert(node, role);
      parents.push(parent);

      self.insert(node, gene[29].is_multiple_of(2));
      self.style(node, gene, parent.map(|parent| self.ids[parent]));
      self.context(node, gene);
    }

    for index in 0..genes.len() {
      let node = self.ids[index];
      let mut children: Vec<NodeId> = (0..genes.len())
        .filter(|child| {
          parents[*child] == Some(index) && self.roles[&self.ids[*child]] == Role::Flow
        })
        .map(|child| self.ids[child])
        .collect();

      if index == 0 {
        children.extend(
          (0..genes.len())
            .map(|child| self.ids[child])
            .filter(|child| self.roles[child] == Role::Overlay),
        );
      }

      self.set_children(node, &children);
    }
  }

  /// Applies one write of a random run, as `action` picks it; `true` where it computed.
  fn act(&mut self, action: &(u8, u8, u8, Gene)) -> bool {
    let (kind, first, second, gene) = action;
    let node = self.pick(*first);

    match kind % 10 {
      0 => {
        let parent = self.oracle.parent(node);

        self.style(node, gene, parent);
      }
      1 => self.context(node, gene),
      2 => self.repaint(node, *second),
      3 => self.reorder(node, *second),
      4 => self.adopt(node, *second),
      5 => self.set_children(node, &[]),
      6 => {
        self.tree.mark_dirty(node);
        self.oracle.mark_dirty(node);
      }
      7 => {
        if node != self.root {
          self.remove(node);
        }
      }
      8 => self.add(node, *second, gene),
      _ => {
        self.recompute(*first, *second);

        return true;
      }
    }

    false
  }

  /// Gives `node`'s leaf, where it has one, another paint and nothing else.
  fn repaint(&mut self, node: NodeId, pick: u8) {
    let Some(held) = self
      .oracle
      .nodes
      .get(&node)
      .and_then(|id| self.oracle.taffy.get_node_context(*id))
      .cloned()
    else {
      return;
    };
    let repainted = |tally: &Tally| Canned {
      paint: held.paint.wrapping_add(1).wrapping_add(pick),
      tally: Rc::clone(tally),
      ..held.clone()
    };
    let tree = repainted(&self.tree_tally);
    let oracle = repainted(&self.oracle_tally);
    let changed = self.tree.set_context(node, Some(tree));

    assert_eq!(
      changed,
      self.oracle.set_context(node, Some(oracle)),
      "a repaint's answer"
    );
    assert!(!changed, "a paint-only change keeps the measure");
  }

  /// Gives `node` its children turned and maybe reversed, as `pick` says, with one of the
  /// case's ids not among them, in the tree or not, added now and then.
  fn reorder(&mut self, node: NodeId, pick: u8) {
    let mut children = self.oracle.children(node);

    if !children.is_empty() {
      let turn = usize::from(pick) % children.len();

      children.rotate_left(turn);

      if pick.is_multiple_of(3) {
        children.reverse();
      }
    }

    let added = self.pick(pick.wrapping_mul(7));

    if pick.is_multiple_of(5) && !children.contains(&added) {
      children.push(added);
    }

    children.retain(|child| *child != self.root && !self.descends(node, *child));
    self.set_children(node, &children);
  }

  /// Moves the id `pick` names under `node`, where that makes no cycle, and now and then
  /// gives its previous parent back the children it held, which takes it back.
  fn adopt(&mut self, node: NodeId, pick: u8) {
    let child = self.pick(pick);

    if child == self.root || !self.oracle.nodes.contains_key(&child) || self.descends(node, child) {
      return;
    }

    let previous = self
      .oracle
      .parent(child)
      .map(|parent| (parent, self.oracle.children(parent)));
    let mut children: Vec<NodeId> = self
      .oracle
      .children(node)
      .into_iter()
      .filter(|held| *held != child)
      .collect();
    let at = usize::from(pick) % (children.len() + 1);

    children.insert(at, child);
    self.set_children(node, &children);

    if pick.is_multiple_of(2)
      && let Some((parent, children)) = previous
      && !self.descends(parent, child)
    {
      self.set_children(parent, &children);
    }
  }

  /// Inserts a fresh node, or one removed earlier as `pick` says, styled from `gene`, as
  /// `node`'s last child where `node` is in the tree.
  fn add(&mut self, node: NodeId, pick: u8, gene: &Gene) {
    let added = if pick.is_multiple_of(4)
      && let Some(gone) = self
        .ids
        .iter()
        .copied()
        .find(|id| !self.oracle.nodes.contains_key(id))
    {
      gone
    }
    else {
      self.fresh()
    };
    let parent = self.oracle.nodes.contains_key(&node).then_some(node);

    self.roles.insert(added, Role::Flow);
    self.insert(added, gene[29].is_multiple_of(2));

    if let Some(node) = parent.filter(|parent| *parent != added) {
      let mut children = self.oracle.children(node);

      children.push(added);
      self.set_children(node, &children);
      self.style(added, gene, Some(node));
    }
    else {
      self.style(added, gene, None);
    }

    self.context(added, gene);
  }

  /// Lays the root out in the space `pick` names, after a parentless node `first` picks
  /// laid out on its own as an inline box is, where `pick` says so.
  fn recompute(&mut self, first: u8, pick: u8) {
    let inline: Vec<NodeId> = self
      .ids
      .iter()
      .copied()
      .filter(|id| {
        *id != self.root && self.oracle.nodes.contains_key(id) && self.oracle.parent(*id).is_none()
      })
      .collect();

    if !pick.is_multiple_of(2) && !inline.is_empty() {
      self.compute(inline[usize::from(first) % inline.len()], MAX_CONTENT);
    }

    self.compute(self.root, viewport(pick));
  }

  /// Lays both out from `node` in `available`, then holds every node's layouts, its mark
  /// and its leaf's measures to the oracle's.
  fn compute(&mut self, node: NodeId, available: taffy::geometry::Size<AvailableSpace>) {
    self.tree_tally.borrow_mut().clear();
    self.oracle_tally.borrow_mut().clear();
    self.tree.compute(node, available);
    self.oracle.compute(node, available);

    assert_eq!(
      *self.tree_tally.borrow(),
      *self.oracle_tally.borrow(),
      "the measures each leaf took"
    );

    self.check();
  }

  /// Holds every node's layouts and mark to the oracle's.
  fn check(&self) {
    for (node, id) in &self.oracle.nodes {
      assert!(self.tree.contains(*node), "{node:?} in the tree");

      assert_eq!(
        self.tree.layout(*node).map(bits),
        self.oracle.taffy.layout(*id).ok().map(bits),
        "{node:?}'s layout"
      );

      assert_eq!(
        self.tree.unrounded_layout(*node).map(bits),
        Some(bits(self.oracle.taffy.unrounded_layout(*id))),
        "{node:?}'s unrounded layout"
      );

      assert_eq!(
        self.tree.is_dirty(*node),
        self.oracle.taffy.dirty(*id).expect("an oracle node's mark"),
        "{node:?}'s mark"
      );
    }

    for node in &self.ids {
      assert_eq!(
        self.tree.contains(*node),
        self.oracle.nodes.contains_key(node),
        "{node:?} in the tree"
      );
    }
  }
}

/// A node's genes.
fn gene() -> impl Strategy<Value = Gene> {
  any::<Gene>()
}

/// One write of a random run: its kind, two picks and the genes it styles from.
fn action() -> impl Strategy<Value = (u8, u8, u8, Gene)> {
  (any::<u8>(), any::<u8>(), any::<u8>(), gene())
}

proptest! {
  // Every run tries the same cases, and a failure is kept by no file.
  #![proptest_config(ProptestConfig {
    cases: 256,
    rng_seed: RngSeed::Fixed(0x5ce4e),
    failure_persistence: None,
    ..ProptestConfig::default()
  })]

  /// Any tree a scene lays out, written by any run of writes and laid out from its root
  /// or from an inline box in any space, rounded or not, lays every node out as one taffy
  /// tree given the same writes does, bit for bit, with the same marks and every leaf
  /// measured as often.
  #[test]
  fn the_tree_lays_out_as_one_taffy_tree(
    rounding in any::<bool>(),
    unit in any::<bool>(),
    genes in proptest::collection::vec(gene(), 1..40),
    actions in proptest::collection::vec(action(), 0..12),
    first in any::<u8>(),
    last in any::<u8>(),
  ) {
    let mut pair = Pair::new(rounding, if unit { Borders::Unit } else { Borders::Width });

    pair.build(&genes);
    pair.compute(pair.root, viewport(first));

    for action in &actions {
      if !pair.act(action) {
        pair.check();
      }
    }

    pair.compute(pair.root, viewport(last));
  }
}
