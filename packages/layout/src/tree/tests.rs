//! [`Tree`]'s engines over small trees of boxes and runs, each a container or a member of
//! one as the scene creates it.

use std::{cell::Cell, collections::HashMap, rc::Rc};

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

/// The rectangle a layout gives a node in the root: its left, top, width and height.
type Bounds = (f32, f32, f32, f32);

/// A tree of boxes and runs, with each node's parent.
struct Scene {
  tree: Tree<Block>,
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
      Some(Block {
        width,
        height,
        measures: Rc::clone(&measures),
      }),
    );

    (run, measures)
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
        ..Placed::default()
      }),
      "decorated: {decorated}"
    );
  }
}
