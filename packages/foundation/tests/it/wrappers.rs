//! The wrappers: each annotates the content of a derived subtree, on every rebuild, and
//! no other node; and every view holding one child renders nothing without it.

use skene_foundation::{
  AnyView,
  Dimension,
  Flexibility,
  GridPlacement,
  HasChild,
  HasChildren,
  Inset,
  ItemAlignment,
  Line,
  MenuEntry,
  Modifiers,
  NodeId,
  Shortcut,
  StaticValue,
  View,
  routing::Route,
  web::Typography,
};
use skene_headless::{Headless, box_props, children, full_outline, outline, reset};
use skene_reactive::{Owner, Set, Signal, Track, signal};

use crate::common::only_child;

type Column = skene_foundation::Column<Headless>;
type Container = skene_foundation::Container<Headless>;
type Flexible = skene_foundation::Flexible<Headless>;
type Grid = skene_foundation::Grid<Headless>;
type Stack = skene_foundation::Stack<Headless>;
type Text = skene_foundation::Text<Headless>;
type Pinned = skene_foundation::Pinned<Headless>;
type GridCell = skene_foundation::GridCell<Headless>;
type Overlay = skene_foundation::Overlay<Headless>;
type Tooltip = skene_foundation::Tooltip<Headless>;
type ContextMenu = skene_foundation::ContextMenu<Headless>;
type Visibility = skene_foundation::Visibility<Headless>;
type Keymap = skene_foundation::Keymap<Headless>;
type Link = skene_foundation::Link<Headless, Page>;
type Enhance = skene_foundation::Enhance<Headless>;
type Suspense = skene_foundation::Suspense<Headless>;
type ErrorBoundary = skene_foundation::ErrorBoundary<Headless>;
type Theme = skene_foundation::Theme<Headless, Typography>;
type Deferred = skene_foundation::web::Deferred<Headless>;
type AsyncView = skene_foundation::web::AsyncView<Headless>;

/// The one route a link in these tests names.
#[derive(Clone, PartialEq)]
struct Page;

impl Route for Page {
  fn parse(path: &str) -> Option<Self> {
    (path == "/").then_some(Self)
  }

  fn to_path(&self) -> String {
    "/".to_owned()
  }
}

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

/// A view that wraps one child renders nothing without it: no box, no overlay and no
/// scope, its fallback included, only the anchors of an empty segment.
#[test]
fn a_view_without_its_child_renders_nothing() {
  reset();

  let owner = Owner::new();
  let live = Signal::new(true);
  let root = owner.enter(|| {
    let childless: Vec<AnyView<Headless>> = vec![
      Overlay::modal()
        .set_open(true)
        .set_label("Settings")
        .into_any(),
      Tooltip::new("A tip").into_any(),
      ContextMenu::new()
        .set_items([MenuEntry::new("Inspect")])
        .into_any(),
      Visibility::new().into_any(),
      Keymap::new()
        .handle_shortcut(Shortcut::new(Modifiers::CTRL, "s"), || {})
        .into_any(),
      Link::new(Page).into_any(),
      Enhance::new(live.read_only())
        .set_fallback_builder(|| Text::new("fallback"))
        .into_any(),
      Suspense::new()
        .set_fallback_builder(|| Text::new("loading"))
        .into_any(),
      ErrorBoundary::new()
        .set_fallback_builder(|_| Text::new("failed"))
        .into_any(),
      Theme::new(Typography::default()).into_any(),
      Deferred::new().into_any(),
      AsyncView::new().into_any(),
    ];

    Column::new()
      .set_children(childless)
      .add_child(Text::new("x"))
      .build()
  });

  assert_eq!(outline(root), "Box\n  Run \"x\"\n");

  let full = full_outline(root);

  assert!(
    !full.contains("Scope"),
    "a theme without its child realizes no scope: {full}"
  );

  owner.dispose();
}

/// An enhancement without its child keeps its fallback while the signal is false and
/// empties once it turns.
#[test]
fn an_enhancement_without_its_child_empties_once_live() {
  reset();

  let owner = Owner::new();
  let live = Signal::new(false);
  let root = owner.enter(|| {
    Column::new()
      .add_child(Enhance::new(live.read_only()).set_fallback_builder(|| Text::new("fallback")))
      .build()
  });

  assert_eq!(outline(root), "Box\n  Run \"fallback\"\n");

  live.set(true);

  assert_eq!(outline(root), "Box\n");

  owner.dispose();
}

/// A view given its child takes its setters still: an overlay opened after its content
/// is set shows it.
#[test]
fn a_view_given_its_child_takes_its_setters() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    let overlay = Overlay::popup().set_child(Text::new("content"));
    let tooltip = Tooltip::new("A tip").set_child(Text::new("described"));

    Column::new()
      .set_children(vec![
        overlay.set_open(true).into_any(),
        tooltip.set_delay(std::time::Duration::ZERO).into_any(),
      ])
      .build()
  });

  assert_eq!(
    outline(root),
    "Box\n  Overlay popup\n    Run \"content\"\n  Run \"described\"\n  Overlay popup\n    Run \"A tip\"\n"
  );

  owner.dispose();
}
