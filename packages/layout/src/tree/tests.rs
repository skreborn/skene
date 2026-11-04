//! [`Tree`]'s engines over small trees of boxes and runs, each a container or a member of
//! one as the scene creates it.

use std::{
  cell::{Cell, RefCell},
  collections::HashMap,
  rc::Rc,
};

use skene_foundation::{
  Axis,
  BoxStyle,
  Dimension,
  Flexibility,
  GridTemplate,
  GridTrack,
  Inset,
  ItemAlignment,
  Layout,
  NodeId,
  Plain,
  Point,
  Rect,
  Size,
  Spacing,
};
use taffy::{
  prelude::AvailableSpace,
  style::{Display, Style},
};

use super::{Leaf, Placed, Tree};
use crate::{Borders, BoxLayout, Place};

/// A run of a fixed size, counting its measures.
#[derive(Clone)]
struct Block {
  width: f32,
  height: f32,
  measures: Rc<Cell<u32>>,
}

impl PartialEq for Block {
  fn eq(&self, other: &Self) -> bool {
    self.width.to_bits() == other.width.to_bits() && self.height.to_bits() == other.height.to_bits()
  }
}

impl Leaf for Block {
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    _available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    self.measures.set(self.measures.get() + 1);

    Size::new(
      known.width.unwrap_or(self.width),
      known.height.unwrap_or(self.height),
    )
  }
}

/// A paragraph of boxes, counting its measures and keeping the sizes each sizing gave its
/// boxes: it measures as its boxes side by side, as tall as the tallest, and places them
/// left to right along its content box's top.
#[derive(Clone)]
struct Boxes {
  children: Vec<(NodeId, Size<f32>)>,
  measures: Rc<Cell<u32>>,
  sizings: Rc<RefCell<Vec<Vec<Size<f32>>>>>,
}

impl PartialEq for Boxes {
  fn eq(&self, other: &Self) -> bool {
    self
      .children
      .iter()
      .map(|(node, _)| node)
      .eq(other.children.iter().map(|(node, _)| node))
  }
}

impl Leaf for Boxes {
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    _available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    self.measures.set(self.measures.get() + 1);

    Size::new(
      known
        .width
        .unwrap_or_else(|| self.children.iter().map(|(_, size)| size.width).sum()),
      known.height.unwrap_or_else(|| {
        self
          .children
          .iter()
          .map(|(_, size)| size.height)
          .fold(0., f32::max)
      }),
    )
  }

  fn place(&self, content: Rect<f32>) -> Vec<(NodeId, Rect<f32>)> {
    let mut left = content.origin.x;

    self
      .children
      .iter()
      .map(|(node, size)| {
        let rect = Rect::new(Point::new(left, content.origin.y), *size);

        left += size.width;

        (*node, rect)
      })
      .collect()
  }

  fn boxes(&self) -> impl Iterator<Item = NodeId> {
    self.children.iter().map(|(node, _)| *node)
  }

  fn size_boxes(&mut self, sizes: impl IntoIterator<Item = Size<f32>>) {
    let sizes: Vec<Size<f32>> = sizes.into_iter().collect();

    for ((_, held), size) in self.children.iter_mut().zip(&sizes) {
      *held = *size;
    }

    self.sizings.borrow_mut().push(sizes);
  }
}

/// What a leaf of the scene measures: a run, or a paragraph of boxes.
#[derive(Clone, PartialEq)]
enum Content {
  /// A run of a fixed size.
  Block(Block),
  /// A paragraph of boxes.
  Boxes(Boxes),
}

impl Leaf for Content {
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    match self {
      Self::Block(block) => block.measure(known, available),
      Self::Boxes(boxes) => boxes.measure(known, available),
    }
  }

  fn place(&self, content: Rect<f32>) -> Vec<(NodeId, Rect<f32>)> {
    match self {
      Self::Block(block) => block.place(content),
      Self::Boxes(boxes) => boxes.place(content),
    }
  }

  fn boxes(&self) -> impl Iterator<Item = NodeId> {
    match self {
      Self::Block(_) => None,
      Self::Boxes(boxes) => Some(Leaf::boxes(boxes)),
    }
    .into_iter()
    .flatten()
  }

  fn size_boxes(&mut self, sizes: impl IntoIterator<Item = Size<f32>>) {
    if let Self::Boxes(boxes) = self {
      boxes.size_boxes(sizes);
    }
  }
}

/// The rectangle a layout gives a node in the root: its left, top, width and height.
type Bounds = (f32, f32, f32, f32);

/// A tree of boxes and runs, with each node's parent.
struct Scene {
  tree: Tree<Content>,
  parents: HashMap<NodeId, NodeId>,
  next: u32,
}

impl Scene {
  /// An empty scene, rounding its layouts where `rounding` says so.
  fn new(rounding: bool) -> Self {
    Self {
      tree: Tree::new(rounding),
      parents: HashMap::new(),
      next: 0,
    }
  }

  /// A fresh node.
  fn fresh(&mut self) -> NodeId {
    self.next += 1;

    NodeId::new(self.next, 0)
  }

  /// Adds a box styled `style`, laid out as `layout`, in a stack's cell where `in_stack`
  /// says so, a container of its own where `decorated` says so, holding `children`.
  fn add(
    &mut self,
    decorated: bool,
    style: &BoxStyle<f32, Plain>,
    layout: Layout,
    in_stack: bool,
    children: &[NodeId],
  ) -> NodeId {
    let added = self.fresh();

    self.tree.insert(added, decorated);

    self.tree.set_style(
      added,
      style.taffy_style(
        layout,
        Place {
          in_stack,
          borders: Borders::Width,
        },
      ),
    );

    self.adopt(added, children);

    added
  }

  /// Gives `node` its children.
  fn adopt(&mut self, node: NodeId, children: &[NodeId]) {
    self.tree.set_children(node, children.iter().copied());

    for child in children {
      self.parents.insert(*child, node);
    }
  }

  /// Adds a run `width` by `height`, and the count of its measures.
  fn run(&mut self, width: f32, height: f32) -> (NodeId, Rc<Cell<u32>>) {
    let run = self.fresh();
    let measures = Rc::new(Cell::new(0));

    self.tree.insert(run, false);

    self.tree.set_context(
      run,
      Some(Content::Block(Block {
        width,
        height,
        measures: Rc::clone(&measures),
      })),
    );

    (run, measures)
  }

  /// Adds a paragraph styled `style` holding `boxes` as its children, each unsized, and
  /// the count of its measures.
  fn paragraph(
    &mut self,
    style: &BoxStyle<f32, Plain>,
    boxes: &[NodeId],
  ) -> (NodeId, Rc<Cell<u32>>) {
    let paragraph = self.add(false, style, Layout::Single, false, boxes);
    let measures = Rc::new(Cell::new(0));

    self
      .tree
      .set_context(paragraph, Some(Self::boxes(boxes, &measures)));

    (paragraph, measures)
  }

  /// A paragraph's context holding `boxes`, each unsized, counting its measures in
  /// `measures`.
  fn boxes(boxes: &[NodeId], measures: &Rc<Cell<u32>>) -> Content {
    Content::Boxes(Boxes {
      children: boxes.iter().map(|node| (*node, Size::ZERO)).collect(),
      measures: Rc::clone(measures),
      sizings: Rc::default(),
    })
  }

  /// `node`'s rectangle in its parent before any rounding, as the last layout left it.
  fn unrounded(&self, node: NodeId) -> Rect<f32> {
    self
      .tree
      .unrounded_layout(node)
      .expect("a node of the tree")
      .rect
  }

  /// Lays the tree under `root` out in `width` by `height`.
  fn lay_out(&mut self, root: NodeId, width: f32, height: f32) {
    self.tree.compute(
      root,
      taffy::geometry::Size {
        width: AvailableSpace::Definite(width),
        height: AvailableSpace::Definite(height),
      },
    );
  }

  /// Adds a box 5 by 5 positioned absolutely 1 from its container's top and left, laid
  /// out in a stack's cell.
  fn pinned(&mut self) -> NodeId {
    self.add(
      true,
      &BoxStyle {
        pinned: Some(Inset::new(
          Dimension::Length(1.),
          Dimension::Length(1.),
          Dimension::Auto,
          Dimension::Auto,
        )),
        ..sized(5., 5.)
      },
      Layout::Single,
      true,
      &[],
    )
  }

  /// The boxes positioned absolutely that `node`'s last layout placed inside it.
  fn hoisted(&self, node: NodeId) -> Vec<NodeId> {
    self
      .tree
      .nodes
      .map()
      .get(node)
      .expect("a node of the tree")
      .hoisted
      .iter()
      .map(|id| super::engine::node(*id))
      .collect()
  }

  /// `node`'s rectangle in its root as the last layout left it.
  fn at(&self, node: NodeId) -> Bounds {
    let rect = self.tree.layout(node).expect("a node of the tree").rect;
    let (mut x, mut y) = (rect.origin.x, rect.origin.y);
    let mut current = node;

    while let Some(parent) = self.parents.get(&current) {
      let origin = self
        .tree
        .layout(*parent)
        .expect("a node of the tree")
        .rect
        .origin;

      x += origin.x;
      y += origin.y;
      current = *parent;
    }

    (x, y, rect.size.width, rect.size.height)
  }

  /// Adds a box of the taffy style `style`, a container of its own where `decorated`
  /// says so, holding `children`.
  fn styled(&mut self, decorated: bool, style: Style, children: &[NodeId]) -> NodeId {
    let added = self.fresh();

    self.tree.insert(added, decorated);
    self.tree.set_style(added, style);
    self.adopt(added, children);

    added
  }

  /// Adds the boxes a paragraph and a row hold alike, each followed by the nodes inside
  /// it: a padded and bordered box of its own, one offset relative, one offset relative
  /// wholly before its parent's origin, a hidden one, one whose content overflows it, one
  /// that scrolls its content, and one holding a box positioned fixed.
  fn assorted(&mut self) -> Vec<NodeId> {
    let edges = |left: f32, right: f32, top: f32, bottom: f32| taffy::geometry::Rect {
      left: taffy::style::LengthPercentage::length(left),
      right: taffy::style::LengthPercentage::length(right),
      top: taffy::style::LengthPercentage::length(top),
      bottom: taffy::style::LengthPercentage::length(bottom),
    };
    let (run, _) = self.run(2.25, 1.5);
    let framed = Style {
      padding: edges(0.5, 1., 0.25, 0.75),
      border: edges(0.3, 0.3, 0.6, 0.6),
      ..Style::default()
    };
    let framed = self.styled(true, framed, &[run]);
    let offset = Style {
      inset: insets(Some(1.5), Some(4.), None, Some(0.5)),
      ..fixed(3., 2.)
    };
    let offset = self.styled(false, offset, &[]);
    let behind = Style {
      inset: insets(Some(-40.), None, Some(-30.), None),
      ..fixed(3., 2.)
    };
    let behind = self.styled(false, behind, &[]);
    let (unseen, _) = self.run(3., 3.);
    let hidden = Style {
      display: Display::None,
      ..Style::default()
    };
    let hidden = self.styled(false, hidden, &[unseen]);
    let wide = self.styled(false, fixed(8., 9.), &[]);
    let overflowing = self.styled(true, fixed(5., 5.), &[wide]);
    let (scrolled, _) = self.run(6., 7.);
    let scrolling = Style {
      overflow: taffy::geometry::Point {
        x: taffy::style::Overflow::Scroll,
        y: taffy::style::Overflow::Scroll,
      },
      scrollbar_width: 1.,
      ..fixed(4., 4.)
    };
    let scrolling = self.styled(false, scrolling, &[scrolled]);
    let fixed_box = Style {
      position: taffy::style::Position::Fixed,
      inset: insets(Some(1.), None, Some(2.), None),
      ..fixed(2., 2.)
    };
    let fixed_box = self.styled(false, fixed_box, &[]);
    let holding = self.styled(false, fixed(1.5, 1.5), &[fixed_box]);

    vec![
      framed,
      run,
      offset,
      behind,
      hidden,
      unseen,
      overflowing,
      wide,
      scrolling,
      scrolled,
      holding,
      fixed_box,
    ]
  }
}

/// A row's or a column's layout.
const fn flex(axis: Axis) -> Layout {
  Layout::Flex {
    axis,
    reversed: false,
  }
}

/// A box `width` by `height`.
fn sized(width: f32, height: f32) -> BoxStyle<f32, Plain> {
  BoxStyle {
    width: Some(Dimension::Length(width)),
    height: Some(Dimension::Length(height)),
    ..BoxStyle::default()
  }
}

/// A style padded 8 on every edge, `spacing` between its children, aligned at the start.
fn padded(spacing: f32) -> BoxStyle<f32, Plain> {
  BoxStyle {
    padding: Some(Inset::new(8., 8., 8., 8.)),
    spacing: Some(Spacing::from(spacing)),
    item_alignment: Some(ItemAlignment::Start),
    ..BoxStyle::default()
  }
}

/// A row places its children inside its padding with its spacing between them, each a
/// run of the row's engine or a box of its own, and is as large as they are at its
/// content's size.
#[test]
fn a_row_places_its_children_with_padding_and_spacing() {
  for decorated in [false, true] {
    let mut scene = Scene::new(false);
    let (first, _) = scene.run(50., 20.);
    let (inner, _) = scene.run(30., 40.);
    let second = scene.add(
      decorated,
      &BoxStyle::default(),
      Layout::Single,
      false,
      &[inner],
    );
    let row = scene.add(
      true,
      &padded(10.),
      flex(Axis::Horizontal),
      false,
      &[first, second],
    );

    scene.tree.compute(
      row,
      taffy::geometry::Size {
        width: AvailableSpace::MaxContent,
        height: AvailableSpace::MaxContent,
      },
    );

    assert_eq!(scene.at(row), (0., 0., 106., 56.), "decorated: {decorated}");
    assert_eq!(scene.at(first), (8., 8., 50., 20.));
    assert_eq!(scene.at(second), (68., 8., 30., 40.));
    assert_eq!(scene.at(inner), (68., 8., 30., 40.));
  }
}

/// A flexible child takes the room its row leaves, the run inside it stretched across it.
#[test]
fn a_flexible_child_takes_the_space_left() {
  let mut scene = Scene::new(false);
  let (fixed, _) = scene.run(50., 20.);
  let (inner, _) = scene.run(0., 20.);
  let grown = scene.add(
    true,
    &BoxStyle {
      flex: Some(Flexibility::grow(1.)),
      ..BoxStyle::default()
    },
    Layout::Single,
    false,
    &[inner],
  );
  let row = scene.add(
    false,
    &padded(10.),
    flex(Axis::Horizontal),
    false,
    &[fixed, grown],
  );
  let root = scene.add(true, &sized(200., 100.), Layout::Single, false, &[row]);

  scene.lay_out(root, 200., 100.);

  assert_eq!(scene.at(grown), (68., 8., 124., 20.));
  assert_eq!(scene.at(inner), (68., 8., 124., 20.));
}

/// A grid places its children in its flexible tracks, its rows stretched over its height,
/// and a stack lays every child out in its one cell, a pinned one placed from its edges.
#[test]
fn a_grid_places_children_in_flexible_tracks_and_a_stack_pins() {
  let mut scene = Scene::new(false);
  let cells: Vec<NodeId> = (0..4)
    .map(|index| {
      let (run, _) = scene.run(10., 10.);

      scene.add(
        index % 2 == 0,
        &BoxStyle::default(),
        Layout::Single,
        false,
        &[run],
      )
    })
    .collect();
  let grid = scene.add(
    false,
    &BoxStyle {
      grid_columns: Some(
        GridTemplate::new()
          .add_track(GridTrack::Flex(1.))
          .add_track(GridTrack::Flex(3.)),
      ),
      spacing: Some(Spacing::from(4.)),
      ..sized(204., 100.)
    },
    Layout::Grid,
    false,
    &cells,
  );
  let root = scene.add(true, &BoxStyle::default(), Layout::Single, false, &[grid]);

  scene.lay_out(root, 204., 100.);

  assert_eq!(scene.at(cells[1]), (54., 0., 150., 48.));
  assert_eq!(scene.at(cells[2]), (0., 52., 50., 48.));

  let back = scene.add(true, &BoxStyle::default(), Layout::Single, true, &[]);
  let (mark, _) = scene.run(10., 10.);
  let badge = scene.add(
    false,
    &BoxStyle {
      pinned: Some(Inset::new(
        Dimension::Auto,
        Dimension::Auto,
        Dimension::Length(4.),
        Dimension::Length(4.),
      )),
      ..BoxStyle::default()
    },
    Layout::Single,
    true,
    &[mark],
  );
  let stack = scene.add(
    true,
    &BoxStyle {
      padding: Some(Inset::new(8., 6., 0., 0.)),
      ..sized(100., 50.)
    },
    Layout::Stack,
    false,
    &[back, badge],
  );

  scene.lay_out(stack, 100., 50.);

  assert_eq!(scene.at(back), (8., 6., 92., 44.));
  assert_eq!(scene.at(badge), (86., 36., 10., 10.));
  assert_eq!(scene.at(mark), (86., 36., 10., 10.));
}

/// A column shrinks a child down to the least height it sets, the box beneath laid out
/// by its own engine at the height the column gives it.
#[test]
fn a_column_shrinks_to_its_childrens_minimums() {
  let mut scene = Scene::new(false);
  let (header, _) = scene.run(10., 40.);
  let (content, _) = scene.run(10., 600.);
  let tall = scene.add(
    true,
    &BoxStyle {
      flex: Some(Flexibility::grow(1.)),
      min_height: Some(Dimension::Length(46.)),
      ..BoxStyle::default()
    },
    Layout::Single,
    false,
    &[content],
  );
  let column = scene.add(
    true,
    &sized(100., 300.),
    flex(Axis::Vertical),
    false,
    &[header, tall],
  );

  scene.lay_out(column, 100., 300.);

  assert_eq!(scene.at(tall), (0., 40., 100., 260.));

  scene.tree.set_style(
    column,
    sized(100., 50.).taffy_style(
      flex(Axis::Vertical),
      Place {
        in_stack: false,
        borders: Borders::Width,
      },
    ),
  );
  scene.lay_out(column, 100., 50.);

  assert_eq!(
    scene.at(tall),
    (0., 40., 100., 46.),
    "no shorter than its least"
  );
}

/// A box is no shorter than its content, within its greatest height, where its column
/// shrinks, a box's least height being its content's as CSS's grid has it; a column
/// whose least height is nothing shrinks to its padding.
#[test]
fn a_box_is_no_shorter_than_its_content_where_a_column_shrinks_to_its_least() {
  let mut scene = Scene::new(false);
  let own = BoxStyle {
    padding: Some(Inset::new(2., 2., 2., 2.)),
    max_height: Some(Dimension::Length(300.)),
    ..BoxStyle::default()
  };
  let (inside, _) = scene.run(40., 500.);
  let boxed = scene.add(true, &own, Layout::Single, false, &[inside]);
  let (beneath, _) = scene.run(40., 500.);
  let shrinking = scene.add(
    true,
    &BoxStyle {
      min_height: Some(Dimension::Length(0.)),
      ..own
    },
    flex(Axis::Vertical),
    false,
    &[beneath],
  );
  let column = scene.add(
    true,
    &sized(100., 50.),
    flex(Axis::Vertical),
    false,
    &[boxed, shrinking],
  );

  scene.lay_out(column, 100., 50.);

  assert_eq!(scene.at(boxed), (0., 0., 100., 300.));
  assert_eq!(scene.at(shrinking), (0., 300., 100., 4.));
}

/// Boxes a third of a row wide each round from where they lie in the root, once over the
/// hierarchy: their widths sum to the row's, and each run inside one takes its box's
/// rounded rectangle, its fractional one kept beside it.
#[test]
fn a_nested_container_rounds_from_its_fractional_rectangle() {
  let mut scene = Scene::new(true);
  let thirds: Vec<(NodeId, NodeId)> = (0..3)
    .map(|_| {
      let (run, _) = scene.run(0., 10.);
      let third = scene.add(
        true,
        &BoxStyle {
          flex: Some(Flexibility::grow(1.)),
          ..BoxStyle::default()
        },
        Layout::Single,
        false,
        &[run],
      );

      (third, run)
    })
    .collect();
  let boxes: Vec<NodeId> = thirds.iter().map(|(third, _)| *third).collect();
  let row = scene.add(
    true,
    &sized(100., 10.),
    flex(Axis::Horizontal),
    false,
    &boxes,
  );

  scene.lay_out(row, 100., 10.);

  let placed: Vec<Bounds> = boxes.iter().map(|third| scene.at(*third)).collect();

  assert_eq!(
    placed,
    [(0., 0., 33., 10.), (33., 0., 34., 10.), (67., 0., 33., 10.)]
  );

  for (third, run) in &thirds {
    assert_eq!(
      scene.at(*run),
      scene.at(*third),
      "a run takes its box's rectangle"
    );
  }

  let fractional = scene
    .tree
    .unrounded_layout(boxes[1])
    .expect("a node of the tree")
    .rect;

  assert!((fractional.origin.x - 100. / 3.).abs() < 1e-4);
  assert!((fractional.size.width - 100. / 3.).abs() < 1e-4);
}

/// What a container measured is kept until a node beneath it is marked, its runs measured
/// again then and not before.
#[test]
fn what_a_container_measured_is_kept_until_it_is_invalidated() {
  let mut scene = Scene::new(true);
  let (run, measures) = scene.run(30., 20.);
  let inner = scene.add(true, &padded(10.), Layout::Single, false, &[run]);
  let row = scene.add(false, &padded(10.), flex(Axis::Horizontal), false, &[inner]);
  let root = scene.add(true, &BoxStyle::default(), Layout::Single, false, &[row]);

  scene.lay_out(root, 200., 100.);

  assert!(measures.get() > 0, "a first layout measures its runs");

  let first = scene.at(run);

  measures.set(0);
  scene.lay_out(root, 200., 100.);

  assert_eq!(
    measures.get(),
    0,
    "a layout of nothing marked measures nothing"
  );
  assert_eq!(scene.at(run), first);

  scene.tree.mark_dirty(run);
  scene.lay_out(root, 200., 100.);

  assert!(measures.get() > 0, "a marked run is measured again");
  assert_eq!(scene.at(run), first);
}

/// A margin collapses through an empty document laid out by an engine of its own, with
/// the margins of its siblings in the document around it, as one tree collapses it.
#[test]
fn a_margin_collapses_through_a_document_of_its_own() {
  let mut scene = Scene::new(false);
  let margin = |top: f32, bottom: f32| BoxStyle {
    margin: Some(Inset::new(
      Dimension::Length(0.),
      Dimension::Length(top),
      Dimension::Length(0.),
      Dimension::Length(bottom),
    )),
    ..BoxStyle::default()
  };
  let (first_run, _) = scene.run(10., 10.);
  let first = scene.add(
    false,
    &margin(0., 5.),
    Layout::Document,
    false,
    &[first_run],
  );
  let (gone, _) = scene.run(10., 10.);
  let empty = scene.add(true, &margin(7., 3.), Layout::Document, false, &[gone]);
  let (last_run, _) = scene.run(10., 10.);
  let last = scene.add(true, &margin(4., 0.), Layout::Document, false, &[last_run]);
  let document = scene.add(
    true,
    &BoxStyle::default(),
    Layout::Document,
    false,
    &[first, empty, last],
  );

  scene.tree.set_style(
    gone,
    Style {
      display: Display::None,
      ..Style::default()
    },
  );
  scene.lay_out(document, 100., 100.);

  assert_eq!(scene.at(empty).3, 0.);
  assert_eq!(
    scene.at(last).1,
    17.,
    "five, seven, three and four collapse to seven"
  );
}

/// A subtree hidden across an engine's edge lays every node out at nothing, a box
/// positioned absolutely inside it included, rounded or not; shown again, it lays out
/// where it was.
#[test]
fn a_hidden_subtree_across_engines_lays_out_at_nothing() {
  for rounding in [false, true] {
    let mut scene = Scene::new(rounding);
    let (run, _) = scene.run(10., 10.);
    let pinned = scene.add(
      true,
      &BoxStyle {
        pinned: Some(Inset::new(
          Dimension::Length(2.),
          Dimension::Length(1.),
          Dimension::Auto,
          Dimension::Auto,
        )),
        ..sized(5., 5.)
      },
      Layout::Single,
      true,
      &[],
    );
    let stack = scene.add(
      false,
      &BoxStyle::default(),
      Layout::Stack,
      false,
      &[run, pinned],
    );
    let panel = scene.add(true, &sized(40., 30.), Layout::Single, false, &[stack]);
    let shown = BoxStyle {
      pinned: Some(Inset::new(
        Dimension::Length(3.),
        Dimension::Length(3.),
        Dimension::Auto,
        Dimension::Auto,
      )),
      ..BoxStyle::default()
    };
    let overlay = scene.add(true, &shown, Layout::Single, false, &[panel]);
    let root = scene.add(true, &sized(100., 100.), Layout::Single, false, &[overlay]);

    scene.lay_out(root, 100., 100.);

    let open: Vec<Bounds> = [overlay, panel, stack, run, pinned]
      .map(|node| scene.at(node))
      .to_vec();

    assert_eq!(scene.at(pinned), (5., 4., 5., 5.), "rounding: {rounding}");

    let mut hidden = shown.taffy_style(
      Layout::Single,
      Place {
        in_stack: false,
        borders: Borders::Width,
      },
    );

    hidden.display = Display::None;
    scene.tree.set_style(overlay, hidden);
    scene.lay_out(root, 100., 100.);

    for node in [overlay, panel, stack, run, pinned] {
      let rect = scene.tree.layout(node).expect("a node of the tree").rect;

      assert_eq!(rect, Rect::ZERO, "{node:?} hidden, rounding: {rounding}");
    }

    scene.tree.set_style(
      overlay,
      shown.taffy_style(
        Layout::Single,
        Place {
          in_stack: false,
          borders: Borders::Width,
        },
      ),
    );
    scene.lay_out(root, 100., 100.);

    assert_eq!(
      [overlay, panel, stack, run, pinned]
        .map(|node| scene.at(node))
        .to_vec(),
      open
    );
  }
}

/// A box positioned absolutely inside a layout-only box its parent's engine lays out is
/// placed against that box.
#[test]
fn an_absolute_child_of_a_folded_box_is_placed_against_it() {
  let mut scene = Scene::new(true);
  let (above, _) = scene.run(10., 20.);
  let (flowing, _) = scene.run(10., 10.);
  let pinned = scene.add(
    true,
    &BoxStyle {
      pinned: Some(Inset::new(
        Dimension::Length(4.),
        Dimension::Length(3.),
        Dimension::Auto,
        Dimension::Auto,
      )),
      ..sized(5., 5.)
    },
    Layout::Single,
    false,
    &[],
  );
  let folded = scene.add(
    false,
    &BoxStyle {
      padding: Some(Inset::new(10., 10., 10., 10.)),
      ..BoxStyle::default()
    },
    flex(Axis::Vertical),
    false,
    &[flowing, pinned],
  );
  let root = scene.add(
    true,
    &BoxStyle::default(),
    flex(Axis::Vertical),
    false,
    &[above, folded],
  );

  scene.lay_out(root, 100., 100.);

  assert_eq!(scene.at(folded), (0., 20., 30., 30.));
  assert_eq!(scene.at(pinned), (4., 23., 5., 5.));
}

/// A node with no parent lays out as a root, a container or not, at its content's size,
/// and leaves the tree it is not part of as it was.
#[test]
fn an_orphan_lays_out_as_a_root() {
  let mut scene = Scene::new(true);
  let (page_run, _) = scene.run(10., 10.);
  let page = scene.add(true, &sized(100., 100.), Layout::Single, false, &[page_run]);

  scene.lay_out(page, 100., 100.);

  let laid = scene.at(page_run);

  for decorated in [true, false] {
    let (first, _) = scene.run(12., 3.);
    let (second, _) = scene.run(7., 5.);
    let orphan = scene.add(
      decorated,
      &padded(1.),
      flex(Axis::Horizontal),
      false,
      &[first, second],
    );

    scene.tree.compute(
      orphan,
      taffy::geometry::Size {
        width: AvailableSpace::MaxContent,
        height: AvailableSpace::MaxContent,
      },
    );

    assert_eq!(
      scene.at(orphan),
      (0., 0., 36., 21.),
      "decorated: {decorated}"
    );
    assert_eq!(scene.at(second), (21., 8., 7., 5.));
    assert_eq!(scene.at(page_run), laid);
  }
}

/// A box whose last child, positioned absolutely, is removed lays out as a leaf, its
/// child's place forgotten.
#[test]
fn a_box_whose_absolute_child_is_removed_lays_out_as_a_leaf() {
  let mut scene = Scene::new(true);
  let pinned = scene.pinned();
  let stack = scene.add(false, &sized(20., 20.), Layout::Stack, false, &[pinned]);
  let root = scene.add(true, &BoxStyle::default(), Layout::Single, false, &[stack]);

  scene.lay_out(root, 50., 50.);

  assert_eq!(scene.at(pinned), (1., 1., 5., 5.));
  assert_eq!(scene.hoisted(stack), [pinned]);

  scene.tree.remove(pinned);
  scene.parents.remove(&pinned);
  scene.lay_out(root, 50., 50.);

  assert_eq!(scene.at(stack), (0., 0., 20., 20.));
  assert_eq!(scene.hoisted(stack), [], "a leaf places nothing");
}

/// A box positioned absolutely inside a hidden subtree, the box holding it hidden by an
/// ancestor or itself, is placed by none while hidden, may be removed, and the subtree
/// shown again lays out without it.
#[test]
fn an_absolute_box_removed_while_hidden_leaves_its_container() {
  let styled = |style: &BoxStyle<f32, Plain>, layout: Layout, display: Display| {
    let mut style = style.taffy_style(
      layout,
      Place {
        in_stack: false,
        borders: Borders::Width,
      },
    );

    style.display = display;

    style
  };
  let overlay_box = BoxStyle::<f32, Plain> {
    pinned: Some(Inset::new(
      Dimension::Length(0.),
      Dimension::Length(0.),
      Dimension::Auto,
      Dimension::Auto,
    )),
    ..BoxStyle::default()
  };

  for hide_stack in [false, true] {
    let mut scene = Scene::new(true);
    let pinned = scene.pinned();
    let (run, _) = scene.run(10., 10.);
    let stack = scene.add(
      false,
      &BoxStyle::default(),
      Layout::Stack,
      false,
      &[run, pinned],
    );
    let overlay = scene.add(true, &overlay_box, Layout::Single, false, &[stack]);
    let root = scene.add(true, &sized(50., 50.), Layout::Single, false, &[overlay]);
    let (hidden, style, layout) = if hide_stack {
      (stack, BoxStyle::default(), Layout::Stack)
    }
    else {
      (overlay, overlay_box.clone(), Layout::Single)
    };
    let shown = styled(&style, layout, Display::Grid);

    scene.tree.set_style(hidden, shown.clone());
    scene.lay_out(root, 50., 50.);

    assert_eq!(scene.hoisted(stack), [pinned], "hide_stack: {hide_stack}");

    scene
      .tree
      .set_style(hidden, styled(&style, layout, Display::None));
    scene.lay_out(root, 50., 50.);

    assert_eq!(
      scene.hoisted(stack),
      [],
      "a hidden box places nothing, hide_stack: {hide_stack}"
    );

    scene.tree.remove(pinned);
    scene.parents.remove(&pinned);
    scene.lay_out(root, 50., 50.);
    scene.tree.set_style(hidden, shown);
    scene.lay_out(root, 50., 50.);

    assert_eq!(scene.at(run), (0., 0., 10., 10.));
    assert_eq!(scene.hoisted(stack), []);
  }
}

/// A box positioned fixed and laid out by its own container's engine is placed against
/// the root, with the nodes beneath it laid out by the engine that places it.
#[test]
fn a_fixed_box_and_its_members_are_laid_out_by_the_engine_placing_it() {
  for decorated in [false, true] {
    let mut scene = Scene::new(true);
    let (run, _) = scene.run(6., 4.);
    let fixed = scene.add(
      decorated,
      &BoxStyle::default(),
      Layout::Single,
      false,
      &[run],
    );
    let mut style = BoxStyle::<f32, Plain>::default().taffy_style(
      Layout::Single,
      Place {
        in_stack: false,
        borders: Borders::Width,
      },
    );

    style.position = taffy::style::Position::Fixed;
    style.inset = taffy::geometry::Rect {
      left: taffy::style::LengthPercentageAuto::length(3.),
      right: taffy::style::LengthPercentageAuto::auto(),
      top: taffy::style::LengthPercentageAuto::length(2.),
      bottom: taffy::style::LengthPercentageAuto::auto(),
    };
    scene.tree.set_style(fixed, style);

    let panel = scene.add(
      true,
      &BoxStyle {
        padding: Some(Inset::new(10., 10., 10., 10.)),
        ..BoxStyle::default()
      },
      Layout::Single,
      false,
      &[fixed],
    );
    let root = scene.add(true, &sized(100., 100.), Layout::Single, false, &[panel]);

    scene.lay_out(root, 100., 100.);

    let rect = scene.tree.layout(fixed).expect("a node of the tree").rect;

    assert_eq!(
      (
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height
      ),
      (3., 2., 6., 4.),
      "decorated: {decorated}"
    );
  }
}

/// A scrolling box's placement holds its rectangle, its border and padding, the rectangle
/// its content reaches from its padding box's corner, and how far it scrolls, its
/// scrollbars and the padding past its content counted in, as Chromium scrolls it.
#[test]
fn a_scrolling_box_is_placed_with_its_overflow_and_how_far_it_scrolls() {
  for decorated in [false, true] {
    let mut scene = Scene::new(true);
    let (run, _) = scene.run(100., 80.);
    let scroller = scene.add(
      decorated,
      &BoxStyle::default(),
      Layout::Single,
      false,
      &[run],
    );
    let edges = |left: f32, top: f32, right: f32, bottom: f32| taffy::geometry::Rect {
      left: taffy::style::LengthPercentage::length(left),
      right: taffy::style::LengthPercentage::length(right),
      top: taffy::style::LengthPercentage::length(top),
      bottom: taffy::style::LengthPercentage::length(bottom),
    };

    scene.tree.set_style(
      scroller,
      Style {
        display: Display::Flex,
        flex_direction: taffy::style::FlexDirection::Column,
        align_items: taffy::style::AlignItems::START,
        size: taffy::geometry::Size {
          width: taffy::style::Dimension::length(40.),
          height: taffy::style::Dimension::length(30.),
        },
        border: edges(1., 2., 3., 4.),
        padding: edges(5., 5., 5., 5.),
        overflow: taffy::geometry::Point {
          x: taffy::style::Overflow::Scroll,
          y: taffy::style::Overflow::Scroll,
        },
        scrollbar_width: 2.,
        ..Style::default()
      },
    );

    let root = scene.add(true, &sized(100., 100.), Layout::Single, false, &[scroller]);

    scene.lay_out(root, 100., 100.);

    assert_eq!(
      scene.tree.layout(scroller),
      Some(Placed {
        rect: Rect::new(Point::new(0., 0.), Size::new(40., 30.)),
        border: Inset::new(1., 2., 3., 4.),
        padding: Inset::new(5., 5., 5., 5.),
        overflow: Rect::new(Point::new(0., 0.), Size::new(110., 90.)),
        scroll_limit: Size::new(76., 68.),
      }),
      "decorated: {decorated}"
    );

    assert_eq!(
      scene.tree.layout(run),
      Some(Placed {
        rect: Rect::new(Point::new(6., 7.), Size::new(100., 80.)),
        overflow: Rect::new(Point::new(0., 0.), Size::new(100., 80.)),
        ..Placed::default()
      }),
      "decorated: {decorated}"
    );
  }
}

/// A box laid out in full to read its baseline while its parent is only sized leaves no
/// layout behind once the parent lays it out again, the boxes each a run of their
/// container's engine or a container of their own.
#[test]
fn a_baseline_read_while_sizing_leaves_no_layout_behind() {
  for decorated in [false, true] {
    let mut scene = Scene::new(false);
    let row_style = |height: taffy::style::Dimension, align: taffy::style::AlignItems| Style {
      display: Display::Flex,
      size: taffy::geometry::Size {
        width: taffy::style::Dimension::auto(),
        height,
      },
      align_items: align,
      ..Style::default()
    };
    let (content, _) = scene.run(1., 1.5);
    let stretched = scene.add(
      decorated,
      &BoxStyle::default(),
      Layout::Single,
      false,
      &[content],
    );
    let zero = scene.add(
      decorated,
      &BoxStyle::default(),
      Layout::Single,
      false,
      &[stretched],
    );
    let sibling = scene.add(decorated, &BoxStyle::default(), Layout::Single, false, &[]);
    let baseline = scene.add(
      decorated,
      &BoxStyle::default(),
      Layout::Single,
      false,
      &[zero, sibling],
    );
    let row = scene.add(
      true,
      &BoxStyle::default(),
      Layout::Single,
      false,
      &[baseline],
    );
    let auto = taffy::style::Dimension::auto();
    let normal = taffy::style::AlignItems::NORMAL;

    scene.tree.set_style(stretched, row_style(auto, normal));
    scene.tree.set_style(
      zero,
      row_style(taffy::style::Dimension::percent(0.), normal),
    );
    scene.tree.set_style(sibling, Style::default());
    scene.tree.set_style(
      baseline,
      row_style(auto, taffy::style::AlignItems::BASELINE),
    );
    scene.tree.set_style(row, row_style(auto, normal));

    for height in [AvailableSpace::MaxContent, AvailableSpace::Definite(1.5)] {
      scene.tree.compute(
        row,
        taffy::geometry::Size {
          width: AvailableSpace::MaxContent,
          height,
        },
      );
    }

    assert_eq!(
      scene
        .tree
        .layout(stretched)
        .expect("a node of the tree")
        .rect
        .size
        .height,
      0.,
      "decorated: {decorated}"
    );
  }
}

/// Two paragraphs, one inside a box of the other, under a row whose padding places the
/// outer one at a fractional origin.
struct Nested {
  root: NodeId,
  outer: NodeId,
  outer_measures: Rc<Cell<u32>>,
  holder: NodeId,
  beside: NodeId,
  inner: NodeId,
  inner_measures: Rc<Cell<u32>>,
  boxed: NodeId,
  content: NodeId,
}

impl Nested {
  /// The outer paragraph, padded 2, holds the box `holder` and the container `beside`, 3
  /// by 4; `holder`, laid out by the outer paragraph's engine, holds the inner paragraph,
  /// whose one box `boxed`, a container of its own, holds the container `content`, 5.5 by
  /// 6; the row's padding places the outer paragraph 0.4 from its top left.
  fn new(scene: &mut Scene) -> Self {
    let content = scene.add(true, &sized(5.5, 6.), Layout::Single, false, &[]);
    let boxed = scene.add(
      true,
      &BoxStyle::default(),
      Layout::Single,
      false,
      &[content],
    );
    let (inner, inner_measures) = scene.paragraph(&BoxStyle::default(), &[boxed]);
    let holder = scene.add(false, &BoxStyle::default(), Layout::Single, false, &[inner]);
    let beside = scene.add(true, &sized(3., 4.), Layout::Single, false, &[]);
    let (outer, outer_measures) = scene.paragraph(
      &BoxStyle {
        padding: Some(Inset::new(2., 2., 2., 2.)),
        ..BoxStyle::default()
      },
      &[holder, beside],
    );
    let root = scene.add(
      true,
      &BoxStyle {
        padding: Some(Inset::new(0.4, 0.4, 0., 0.)),
        item_alignment: Some(ItemAlignment::Start),
        ..BoxStyle::default()
      },
      flex(Axis::Horizontal),
      false,
      &[outer],
    );

    Self {
      root,
      outer,
      outer_measures,
      holder,
      beside,
      inner,
      inner_measures,
      boxed,
      content,
    }
  }
}

/// A paragraph's boxes lay out at their content's size within the paragraph's own layout,
/// a box inside a box inside a paragraph included: each placed where its paragraph places
/// it in its content box, the paragraph measured around them, and each rounded from where
/// it lies in the root.
#[test]
fn a_paragraphs_boxes_lay_out_within_its_layout() {
  let mut scene = Scene::new(true);
  let nested = Nested::new(&mut scene);

  scene.lay_out(nested.root, 100., 100.);

  assert_eq!(scene.unrounded(nested.content).size, Size::new(5.5, 6.));
  assert_eq!(
    scene.unrounded(nested.boxed),
    Rect::new(Point::ZERO, Size::new(5.5, 6.))
  );
  assert_eq!(
    scene.unrounded(nested.inner),
    Rect::new(Point::ZERO, Size::new(5.5, 6.))
  );
  assert_eq!(
    scene.unrounded(nested.holder),
    Rect::new(Point::new(2., 2.), Size::new(5.5, 6.)),
    "a box lies inside its paragraph's padding"
  );
  assert_eq!(
    scene.unrounded(nested.beside),
    Rect::new(Point::new(7.5, 2.), Size::new(3., 4.)),
    "the next box lies after it"
  );
  assert_eq!(
    scene.unrounded(nested.outer),
    Rect::new(Point::new(0.4, 0.4), Size::new(12.5, 10.)),
    "the paragraph measures around its boxes and its padding"
  );

  assert_eq!(scene.at(nested.outer), (0., 0., 13., 10.));
  assert_eq!(scene.at(nested.holder), (2., 2., 6., 6.));
  assert_eq!(scene.at(nested.beside), (8., 2., 3., 4.));
  assert_eq!(
    scene.at(nested.content),
    (2., 2., 6., 6.),
    "the innermost box rounds from where it lies in the root"
  );
}

/// A change to the content of a box inside a box marks both paragraphs, and one layout
/// measures and places them again.
#[test]
fn a_change_inside_a_box_lays_both_paragraphs_out_again() {
  let mut scene = Scene::new(false);
  let nested = Nested::new(&mut scene);

  scene.lay_out(nested.root, 100., 100.);
  nested.outer_measures.set(0);
  nested.inner_measures.set(0);

  scene.tree.set_style(
    nested.content,
    sized(7., 6.).taffy_style(
      Layout::Single,
      Place {
        in_stack: false,
        borders: Borders::Width,
      },
    ),
  );

  assert!(scene.tree.is_dirty(nested.inner));
  assert!(scene.tree.is_dirty(nested.outer));

  scene.lay_out(nested.root, 100., 100.);

  assert!(nested.inner_measures.get() > 0);
  assert!(nested.outer_measures.get() > 0);
  assert_eq!(
    scene.unrounded(nested.holder),
    Rect::new(Point::new(2., 2.), Size::new(7., 6.))
  );
  assert_eq!(scene.unrounded(nested.beside).origin, Point::new(9., 2.));
  assert_eq!(scene.unrounded(nested.outer).size, Size::new(14., 10.));
}

/// A paragraph's context made afresh with its boxes unsized is the one it holds, its boxes'
/// sizes no part of its equality, and marks nothing.
#[test]
fn a_paragraph_made_afresh_with_unsized_boxes_marks_nothing() {
  let mut scene = Scene::new(true);
  let nested = Nested::new(&mut scene);

  scene.lay_out(nested.root, 100., 100.);

  let measures = Rc::new(Cell::new(0));

  assert!(!scene.tree.set_context(
    nested.outer,
    Some(Scene::boxes(&[nested.holder, nested.beside], &measures))
  ));
  assert!(
    !scene
      .tree
      .set_context(nested.inner, Some(Scene::boxes(&[nested.boxed], &measures)))
  );
  assert!(!scene.tree.is_dirty(nested.inner));
  assert!(!scene.tree.is_dirty(nested.outer));
  assert!(!scene.tree.is_dirty(nested.root));
}

/// A paragraph's boxes are given their sizes together, in order, each its own laid-out
/// size, once in every pass that measures the paragraph, and again once one of them
/// changes.
#[test]
fn a_paragraphs_boxes_are_sized_in_order_once_per_pass() {
  let mut scene = Scene::new(false);
  let boxes = [(1., 2.), (3., 4.), (5., 6.)]
    .map(|(width, height)| scene.add(true, &sized(width, height), Layout::Single, false, &[]));
  let paragraph = scene.add(false, &BoxStyle::default(), Layout::Single, false, &boxes);
  let measures = Rc::new(Cell::new(0));
  let sizings = Rc::new(RefCell::new(Vec::new()));

  scene.tree.set_context(
    paragraph,
    Some(Content::Boxes(Boxes {
      children: boxes.iter().map(|node| (*node, Size::ZERO)).collect(),
      measures: Rc::clone(&measures),
      sizings: Rc::clone(&sizings),
    })),
  );

  let root = scene.add(
    true,
    &BoxStyle {
      item_alignment: Some(ItemAlignment::Start),
      ..BoxStyle::default()
    },
    flex(Axis::Horizontal),
    false,
    &[paragraph],
  );

  scene.lay_out(root, 100., 100.);

  assert!(measures.get() > 0);
  assert_eq!(sizings.borrow().len(), measures.get() as usize);
  assert!(
    sizings
      .borrow()
      .iter()
      .all(|sizes| *sizes == [Size::new(1., 2.), Size::new(3., 4.), Size::new(5., 6.)])
  );
  assert_eq!(scene.unrounded(boxes[2]).origin, Point::new(4., 0.));

  sizings.borrow_mut().clear();
  measures.set(0);

  scene.tree.set_style(
    boxes[1],
    sized(7., 4.).taffy_style(
      Layout::Single,
      Place {
        in_stack: false,
        borders: Borders::Width,
      },
    ),
  );

  scene.lay_out(root, 100., 100.);

  assert!(measures.get() > 0);
  assert_eq!(sizings.borrow().len(), measures.get() as usize);
  assert!(
    sizings
      .borrow()
      .iter()
      .all(|sizes| *sizes == [Size::new(1., 2.), Size::new(7., 4.), Size::new(5., 6.)])
  );
  assert_eq!(scene.unrounded(boxes[2]).origin, Point::new(8., 0.));
}

/// A hidden paragraph lays its boxes out at nothing, the boxes inside them too.
#[test]
fn a_hidden_paragraph_lays_its_boxes_out_at_nothing() {
  let mut scene = Scene::new(true);
  let nested = Nested::new(&mut scene);

  scene.lay_out(nested.root, 100., 100.);
  scene.tree.set_style(
    nested.outer,
    Style {
      display: Display::None,
      ..Style::default()
    },
  );
  scene.lay_out(nested.root, 100., 100.);

  for node in [
    nested.outer,
    nested.holder,
    nested.beside,
    nested.inner,
    nested.boxed,
    nested.content,
  ] {
    assert_eq!(scene.unrounded(node), Rect::ZERO);
    assert_eq!(scene.at(node), (0., 0., 0., 0.));
  }
}

/// Removing a box marks its paragraph, which lays out without it once its context names
/// it no more.
#[test]
fn removing_a_box_marks_its_paragraph() {
  let mut scene = Scene::new(false);
  let nested = Nested::new(&mut scene);

  scene.lay_out(nested.root, 100., 100.);
  scene.tree.remove(nested.beside);

  assert!(scene.tree.is_dirty(nested.outer));

  scene.tree.set_context(
    nested.outer,
    Some(Scene::boxes(&[nested.holder], &nested.outer_measures)),
  );
  scene.lay_out(nested.root, 100., 100.);

  assert_eq!(scene.unrounded(nested.outer).size, Size::new(9.5, 10.));
}

/// A taffy style `width` by `height`.
fn fixed(width: f32, height: f32) -> Style {
  Style {
    size: taffy::geometry::Size {
      width: taffy::style::Dimension::length(width),
      height: taffy::style::Dimension::length(height),
    },
    ..Style::default()
  }
}

/// Insets of `left`, `right`, `top` and `bottom`, each auto where `None`.
fn insets(
  left: Option<f32>,
  right: Option<f32>,
  top: Option<f32>,
  bottom: Option<f32>,
) -> taffy::geometry::Rect<taffy::style::LengthPercentageAuto> {
  let inset = |edge: Option<f32>| {
    edge.map_or_else(
      taffy::style::LengthPercentageAuto::auto,
      taffy::style::LengthPercentageAuto::length,
    )
  };

  taffy::geometry::Rect {
    left: inset(left),
    right: inset(right),
    top: inset(top),
    bottom: inset(bottom),
  }
}

/// A paragraph lays its boxes out as a row aligned at the start lays out the same boxes,
/// scrolling or not: every taffy layout inside it, rounded and not, equal field by field,
/// its own included, a box offset relative, a hidden one, one whose content overflows it,
/// one that scrolls, one holding a box positioned fixed and one wholly before the origin,
/// which a scrolling paragraph leaves out of its overflow, among them.
#[test]
fn a_paragraph_lays_its_boxes_out_as_a_row_does() {
  for scrolls in [false, true] {
    let overflow = if scrolls {
      taffy::style::Overflow::Scroll
    }
    else {
      taffy::style::Overflow::Visible
    };
    let container = Style {
      display: Display::Flex,
      align_items: taffy::style::AlignItems::START,
      padding: taffy::geometry::Rect::length(1.3),
      border: taffy::geometry::Rect::length(0.4),
      overflow: taffy::geometry::Point {
        x: overflow,
        y: overflow,
      },
      scrollbar_width: 1.5,
      ..Style::default()
    };
    let root = Style {
      display: Display::Flex,
      flex_direction: taffy::style::FlexDirection::Column,
      align_items: taffy::style::AlignItems::START,
      padding: taffy::geometry::Rect::length(0.45),
      ..Style::default()
    };
    let mut scene = Scene::new(true);
    let inside = scene.assorted();
    let boxes = [
      inside[0], inside[2], inside[3], inside[4], inside[6], inside[8], inside[10],
    ];
    let paragraph = scene.styled(false, container.clone(), &boxes);
    let measures = Rc::new(Cell::new(0));

    scene
      .tree
      .set_context(paragraph, Some(Scene::boxes(&boxes, &measures)));

    let first = scene.styled(true, root.clone(), &[paragraph]);
    let laid = scene.assorted();
    let items = [
      laid[0], laid[2], laid[3], laid[4], laid[6], laid[8], laid[10],
    ];
    let row = scene.styled(false, container, &items);
    let second = scene.styled(true, root, &[row]);

    scene.lay_out(first, 100., 100.);
    scene.lay_out(second, 100., 100.);

    for (held, laid) in inside
      .iter()
      .chain([&paragraph])
      .zip(laid.iter().chain([&row]))
    {
      assert_eq!(
        scene.tree.taffy_unrounded_layout(*held),
        scene.tree.taffy_unrounded_layout(*laid),
        "unrounded, scrolling: {scrolls}"
      );
      assert_eq!(
        scene.tree.taffy_layout(*held),
        scene.tree.taffy_layout(*laid),
        "rounded, scrolling: {scrolls}"
      );
    }
  }
}
