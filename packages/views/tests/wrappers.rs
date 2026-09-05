//! The wrappers: each annotates the content of a derived subtree, on every rebuild, and
//! no other node.

mod common;

use skene_core::{
  Dimension,
  GridPlacement,
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

type Container = skene_views::Container<Headless>;
type Grid = skene_views::Grid<Headless>;
type Stack = skene_views::Stack<Headless>;
type Text = skene_views::Text<Headless>;
type Pinned<C = skene_views::NoChild> = skene_views::Pinned<Headless, C>;
type GridCell<C = skene_views::NoChild> = skene_views::GridCell<Headless, C>;

/// A pinned wrapper around a derived subtree lands its edges on the content box, on
/// every rebuild, and on no other node.
#[test]
fn pinned_annotates_the_content_of_a_derived_subtree() {
  reset();

  let owner = Owner::new();
  let (version, set_version) = signal(0);
  let edges = Inset {
    top: Dimension::Length(4.),
    right: Dimension::Auto,
    bottom: Dimension::Auto,
    left: Dimension::Length(8.),
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
          .set_alignment(skene_core::Alignment::new(
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
