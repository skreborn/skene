//! The layout views: their properties reach the driver's props, tokens and conditions
//! included, and each exposes exactly its own vocabulary.

mod common;

use skene_core::{
  Alignment,
  Arrangement,
  BoxValue,
  Condition,
  DriverLeaf,
  Expr,
  FlexDirection,
  GridTrack,
  HasChildren,
  Inset,
  ItemAlignment,
  Layout,
  Length,
  Media,
  Size,
  Slot,
  Spacing,
  StaticLeaf,
  StaticValue,
  View,
  When,
};
use skene_headless::{
  Headless,
  SlotValue,
  box_props,
  children,
  reset,
  resolve_length,
  set_viewport,
  slot_leaf,
};
use skene_reactive::{Get, Owner, Set, Signal};

use crate::common::{ASPECT_RATIO, PADDING, SPACING, TestPalette};

type Container = skene_views::Container<Headless>;
type Row = skene_views::Row<Headless>;
type Column = skene_views::Column<Headless>;
type Grid = skene_views::Grid<Headless>;
type Stack = skene_views::Stack<Headless>;
type Theme<P, C = skene_views::NoChild> = skene_views::Theme<Headless, P, C>;

/// A layout's spacing reaches the driver as declared, and a token-valued spacing
/// resolves through the theme to the palette's length.
#[test]
fn layout_props_reach_box_props_and_a_token_spacing_resolves() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Theme::new(TestPalette::default())
      .set_child_builder(|| {
        Column::new().set_children((
          Row::new()
            .set_spacing(TestPalette::GUTTER_SPACING)
            .set_arrangement(Arrangement::SpaceBetween)
            .set_item_alignment(ItemAlignment::Center),
          Row::reversed().set_spacing(Expr::<Length>::from(TestPalette::GUTTER_LENGTH) * 2.),
          Row::new().set_spacing(Spacing {
            row: 1.,
            column: 2.,
          }),
        ))
      })
      .build()
  });
  let column = children(root)[0];
  let rows = &children(column)[1..4];
  let tokened = box_props(rows[0]).unwrap();

  assert_eq!(
    tokened.layout,
    Layout::Flex {
      direction: FlexDirection::Row,
    }
  );
  assert_eq!(
    tokened.style.spacing,
    Some(StaticValue::from(StaticLeaf::Token(
      TestPalette::GUTTER_SPACING
    )))
  );
  assert_eq!(
    tokened.style.arrangement,
    Some(StaticValue::from(Arrangement::SpaceBetween))
  );
  assert_eq!(
    tokened.style.item_alignment,
    Some(StaticValue::from(ItemAlignment::Center))
  );
  assert_eq!(resolve_length(rows[0], SPACING), Some(8.));
  assert_eq!(
    box_props(rows[1]).unwrap().layout,
    Layout::Flex {
      direction: FlexDirection::RowReverse,
    }
  );
  assert_eq!(resolve_length(rows[1], SPACING), Some(16.));
  assert_eq!(
    box_props(rows[2]).unwrap().style.spacing,
    Some(StaticValue::from(Spacing {
      row: 1.,
      column: 2.
    }))
  );
  assert_eq!(resolve_length(rows[2], SPACING), None);

  owner.dispose();
}

/// A media condition on a layout's padding switches with the viewport, whether spelled
/// on the inset, on a plain spacing, or on a plain number.
#[test]
fn media_condition_on_padding_switches_with_the_viewport() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Column::new()
      .set_spacing(Spacing::from(8.).when(Media::MinWidth(900.), 16.))
      .set_padding(Inset::all(4.).when(Media::MinWidth(900.), Inset::all(12.)))
      .set_child(Container::new().set_aspect_ratio(1f32.when(Media::MinWidth(900.), 2.)))
      .build()
  });
  let child = children(root)[1];

  assert_eq!(resolve_length(root, SPACING), Some(16.));
  assert_eq!(resolve_length(root, PADDING), Some(12.));
  assert_eq!(resolve_length(child, ASPECT_RATIO), Some(2.));

  set_viewport(Size::new(800., 600.));

  assert_eq!(resolve_length(root, SPACING), Some(8.));
  assert_eq!(resolve_length(root, PADDING), Some(4.));
  assert_eq!(resolve_length(child, ASPECT_RATIO), Some(1.));
  assert_eq!(
    box_props(root).unwrap().style.padding.unwrap().variants(),
    &[(
      Condition::Media(Media::MinWidth(900.)),
      DriverLeaf::Static(StaticLeaf::Fixed(Inset::all(12.))),
    )]
  );

  owner.dispose();
}

/// A uniform grid declares one flexible track per column, and its two-axis alignment
/// lands on both item alignment properties.
#[test]
fn grid_uniform_declares_equal_tracks_and_splits_its_alignment() {
  reset();

  let owner = Owner::new();
  let grid = owner.enter(|| {
    Grid::uniform(3)
      .set_item_alignment(Alignment::new(ItemAlignment::End, ItemAlignment::Start))
      .set_row_arrangement(Arrangement::Center)
      .build()
  });
  let props = box_props(grid).unwrap();

  assert_eq!(props.layout, Layout::Grid);
  assert_eq!(
    props.style.grid_columns,
    Some(StaticValue::from(vec![GridTrack::Flex(1.); 3]))
  );
  assert_eq!(
    props.style.item_horizontal_alignment,
    Some(StaticValue::from(ItemAlignment::End))
  );
  assert_eq!(
    props.style.item_alignment,
    Some(StaticValue::from(ItemAlignment::Start))
  );
  assert_eq!(
    props.style.line_arrangement,
    Some(StaticValue::from(Arrangement::Center))
  );

  owner.dispose();
}

/// A stack's derived alignment writes both axes as slots that follow the signal.
#[test]
fn stack_derived_alignment_drives_both_axes() {
  reset();

  let owner = Owner::new();
  let alignment = Signal::opaque(Alignment::CENTER);
  let stack = owner.enter(|| {
    Stack::new()
      .derive_alignment(move || alignment.get())
      .build()
  });
  let props = box_props(stack).unwrap();

  assert_eq!(props.layout, Layout::Stack);
  assert_eq!(
    props.style.item_alignment,
    Some(StaticValue::from(DriverLeaf::Slot(Slot(0))))
  );
  assert_eq!(
    props.style.item_horizontal_alignment,
    Some(StaticValue::from(DriverLeaf::Slot(Slot(1))))
  );
  assert_eq!(
    slot_leaf(stack, Slot(1)),
    Some(StaticLeaf::Fixed(SlotValue::Box(BoxValue::ItemAlignment(
      ItemAlignment::Center
    ))))
  );

  alignment.set(Alignment::new(ItemAlignment::Start, ItemAlignment::End));

  assert_eq!(
    slot_leaf(stack, Slot(0)),
    Some(StaticLeaf::Fixed(SlotValue::Box(BoxValue::ItemAlignment(
      ItemAlignment::End
    ))))
  );
  assert_eq!(
    slot_leaf(stack, Slot(1)),
    Some(StaticLeaf::Fixed(SlotValue::Box(BoxValue::ItemAlignment(
      ItemAlignment::Start
    ))))
  );

  owner.dispose();
}
