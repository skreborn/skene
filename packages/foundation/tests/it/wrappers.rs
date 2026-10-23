//! The wrappers: each annotates the content of a derived subtree, on every rebuild, and
//! no other node.

use skene_foundation::{
  Dimension,
  Flexibility,
  GridPlacement,
  HasChild,
  HasChildren,
  Inset,
  ItemAlignment,
  Line,
  NodeId,
  StaticValue,
  View,
};
use skene_headless::{Headless, box_props, children, outline, reset};
use skene_reactive::{Owner, Set, Track, signal};

use crate::common::only_child;

type Column = skene_foundation::Column<Headless>;
type Container = skene_foundation::Container<Headless>;
type Flexible = skene_foundation::Flexible<Headless>;
type Grid = skene_foundation::Grid<Headless>;
type Stack = skene_foundation::Stack<Headless>;
type Text = skene_foundation::Text<Headless>;
type Pinned = skene_foundation::Pinned<Headless>;
type GridCell = skene_foundation::GridCell<Headless>;

/// A pinned wrapper around a derived subtree lands its edges on the content box, on
/// every rebuild, and on no other node.
#[test]
fn pinned_annotates_the_content_of_a_derived_subtree() {
  reset();

  let owner = Owner::new();
  let (version, set_version) = signal(0);
  let edges = Inset {
    top: Dimension::Length(4.),
    end: Dimension::Auto,
    bottom: Dimension::Auto,
    start: Dimension::Length(8.),
  };
  let root = owner.enter(|| {
    Stack::new()
      .derive_child(move || {
        version.track();

        Pinned::new()
          .set_edges(edges)
          .set_child(Container::new().set_child(Text::new("x")))
      })
      .build()
  });
  let pinned = |node: NodeId| box_props(node).and_then(|props| props.style.pinned);

  assert_eq!(outline(root), "Box\n  Box\n    Run \"x\"\n");

  let content = children(root)[2];

  assert_eq!(pinned(content), Some(StaticValue::from(edges)));
  assert_eq!(pinned(root), None);
  assert!(box_props(only_child(content)).is_none());

  set_version.set(1);

  let rebuilt = children(root)[2];

  assert_ne!(rebuilt, content);
  assert_eq!(pinned(rebuilt), Some(StaticValue::from(edges)));
  assert_eq!(pinned(root), None);

  owner.dispose();
}

/// A grid cell around a derived subtree lands its placements and both alignment axes
/// on the content box, on every rebuild.
#[test]
fn grid_cell_annotates_the_content_of_a_derived_subtree() {
  reset();

  let owner = Owner::new();
  let (version, set_version) = signal(0);
  let root = owner.enter(|| {
    Grid::uniform(2)
      .derive_child(move || {
        version.track();

        GridCell::new()
          .set_row(Line::at(2))
          .set_column(Line::span(2))
          .set_alignment(skene_foundation::Alignment::new(
            ItemAlignment::End,
            ItemAlignment::Center,
          ))
          .set_child(Container::new())
      })
      .build()
  });
  let check = |node: NodeId| {
    let style = box_props(node).unwrap().style;

    assert_eq!(
      style.grid_row_placement,
      Some(StaticValue::from(Line::at(2)))
    );
    assert_eq!(
      style.grid_column_placement,
      Some(StaticValue::from(Line {
        start: GridPlacement::Auto,
        end: GridPlacement::Span(2),
      }))
    );
    assert_eq!(
      style.self_horizontal_alignment,
      Some(StaticValue::from(ItemAlignment::End))
    );
    assert_eq!(
      style.self_alignment,
      Some(StaticValue::from(ItemAlignment::Center))
    );
  };
  let content = children(root)[2];

  check(content);
  assert_eq!(box_props(root).unwrap().style.grid_row_placement, None);

  set_version.set(1);

  let rebuilt = children(root)[2];

  assert_ne!(rebuilt, content);
  check(rebuilt);

  owner.dispose();
}

/// A wrapper whose child creates no node leaves its properties on nothing: the next box
/// built takes none of them.
#[test]
fn a_wrapper_around_nothing_annotates_no_later_box() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Stack::new()
      .set_children((
        Pinned::new()
          .set_edges(Inset::all(Dimension::Length(4.)))
          .set_child(skene_foundation::empty::<Headless>()),
        Container::new(),
      ))
      .build()
  });
  let later = children(root)
    .into_iter()
    .find_map(box_props)
    .expect("the later box");

  assert_eq!(later.style.pinned, None);

  owner.dispose();
}

/// A wrapper without its child renders nothing, so it takes no room in its container,
/// and annotates no later box.
#[test]
fn a_wrapper_without_its_child_renders_nothing() {
  reset();

  let owner = Owner::new();
  let (stack, column, grid) = owner.enter(|| {
    let stack = Stack::new()
      .set_children((
        Pinned::new().set_edges(Inset::all(Dimension::Length(4.))),
        Container::new(),
      ))
      .build();
    let column = Column::new()
      .set_children((
        Flexible::new().set_flexibility(Flexibility::grow(1.)),
        Container::new(),
      ))
      .build();
    let grid = Grid::uniform(2)
      .set_children((GridCell::new().set_row(Line::at(2)), Container::new()))
      .build();

    (stack, column, grid)
  });

  assert_eq!(outline(stack), "Box\n  Box\n");
  assert_eq!(outline(column), "Box\n  Box\n");
  assert_eq!(outline(grid), "Box\n  Box\n");

  let later = |root: NodeId| {
    let boxes: Vec<_> = children(root).into_iter().filter_map(box_props).collect();

    assert_eq!(boxes.len(), 1);

    boxes.into_iter().next().unwrap()
  };

  assert_eq!(later(stack).style.pinned, None);
  assert_eq!(later(column).style.flex, None);
  assert_eq!(later(grid).style.grid_row_placement, None);

  owner.dispose();
}

/// A wrapper given its child takes its setters still.
#[test]
fn a_wrapper_given_its_child_takes_its_setters() {
  reset();

  let owner = Owner::new();
  let edges = Inset::all(Dimension::Length(4.));
  let root = owner.enter(|| {
    let wrapper = Pinned::new().set_child(Container::new());

    Stack::new()
      .set_children((wrapper.set_edges(edges),))
      .build()
  });
  let content = children(root)
    .into_iter()
    .find_map(box_props)
    .expect("the content box");

  assert_eq!(content.style.pinned, Some(StaticValue::from(edges)));

  owner.dispose();
}
