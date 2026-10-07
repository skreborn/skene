//! The layout the headless platform computes through the shared layout: each box's
//! rectangle at the viewport, a run measured by the platform's metrics, and the sizes a
//! frame reports.

use skene_foundation::{
  ContainerBase,
  DecorationBase,
  Dimension,
  Easing,
  FlexBase,
  Flexibility,
  HasChild,
  HasChildren,
  MotionBase,
  NodeHandle,
  NodeId,
  Point,
  Property,
  Rect,
  Size,
  Transition,
  View,
  web::Typography,
};
use skene_headless::{
  Headless,
  box_props,
  children,
  find_by_text,
  layout_rect,
  node_at,
  reset,
  run_props,
  trigger_frame,
  trigger_resize,
  trigger_size,
};
use skene_reactive::{Get, Owner, Set, Signal};

type Column = skene_foundation::Column<Headless>;
type Container = skene_foundation::Container<Headless>;
type Flexible<C = skene_foundation::NoChild> = skene_foundation::Flexible<Headless, C>;
type Row = skene_foundation::Row<Headless>;
type Stack = skene_foundation::Stack<Headless>;
type Text = skene_foundation::Text<Headless>;
type Theme<S, C = skene_foundation::NoChild> = skene_foundation::Theme<Headless, S, C>;

/// The boxes and runs placed under `node`, its anchors left out.
fn content(node: NodeId) -> Vec<NodeId> {
  children(node)
    .into_iter()
    .filter(|child| box_props(*child).is_some() || run_props(*child).is_some())
    .collect()
}

/// A box `width` by `height` logical pixels.
fn sized(width: f32, height: f32) -> Container {
  Container::new()
    .set_width(Dimension::Length(width))
    .set_height(Dimension::Length(height))
}

/// The rectangle at (`x`, `y`) of `width` by `height`.
const fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect<f32> {
  Rect {
    origin: Point::new(x, y),
    size: Size::new(width, height),
  }
}

/// A column lays its children out down the viewport, the spacing between them.
#[test]
fn a_column_lays_its_children_out_in_turn() {
  reset();

  let owner = Owner::new();
  let column = owner.enter(|| {
    Column::new()
      .set_spacing(10.)
      .set_children((sized(100., 20.), sized(50., 30.)))
      .build()
  });
  let [first, second] = content(column)[..]
  else {
    panic!("two children");
  };

  assert_eq!(layout_rect(first), Some(rect(0., 0., 100., 20.)));
  assert_eq!(layout_rect(second), Some(rect(0., 30., 50., 30.)));
  assert_eq!(
    layout_rect(column).map(|rect| rect.size.width),
    Some(1024.),
    "the column stretches across the viewport"
  );

  owner.dispose();
}

/// A run measures each cluster half its font size wide and each line one and a half
/// times its font size tall, wrapping between words where its box is narrower.
#[test]
fn a_run_measures_by_the_platforms_metrics_and_wraps() {
  reset();

  let owner = Owner::new();
  let column = owner.enter(|| {
    Column::new()
      .set_children((
        Row::new().set_children((Text::new("abcd"),)),
        Container::new()
          .set_width(Dimension::Length(40.))
          .set_child(Text::new("aa bb cc")),
      ))
      .build()
  });
  let boxes = content(column);
  let line = content(boxes[0])[0];
  let wrapped = content(boxes[1])[0];

  assert_eq!(
    layout_rect(line).map(|rect| rect.size),
    Some(Size::new(32., 24.))
  );
  assert_eq!(
    layout_rect(wrapped).map(|rect| rect.size.height),
    Some(48.),
    "two lines: `aa bb` fits 40, `cc` wraps"
  );

  owner.dispose();
}

/// A run that sets no size of its own measures by the typography in force where it sits.
#[test]
fn a_run_measures_by_the_typography_in_force() {
  reset();

  let owner = Owner::new();
  let _theme = owner.enter(|| {
    Theme::new(Typography::default().set_size(20.).set_line_height(2.))
      .set_child_builder(|| Row::new().set_children((Text::new("ab"),)))
      .build()
  });
  let run = find_by_text("ab").expect("the run");

  assert_eq!(
    layout_rect(run).map(|rect| rect.size),
    Some(Size::new(20., 40.))
  );

  owner.dispose();
}

/// A flexible child takes the room its row leaves, and a stack lays every child out in
/// its one cell.
#[test]
fn a_flexible_child_grows_and_a_stack_overlays() {
  reset();

  let owner = Owner::new();
  let (row, stack) = owner.enter(|| {
    (
      Container::new()
        .set_width(Dimension::Length(300.))
        .set_child(
          Row::new().set_children((
            Flexible::new()
              .set_flexibility(Flexibility::grow(1.))
              .set_child(sized(0., 10.)),
            sized(100., 10.),
          )),
        )
        .build(),
      Stack::new()
        .set_children((sized(40., 40.), sized(20., 20.)))
        .build(),
    )
  });
  let grown = content(content(row)[0])[0];
  let layers = content(stack);

  assert_eq!(layout_rect(grown).map(|rect| rect.size.width), Some(200.));
  assert_eq!(
    layers
      .iter()
      .map(|layer| layout_rect(*layer).map(|rect| rect.origin))
      .collect::<Vec<_>>(),
    [Some(Point::new(0., 0.)), Some(Point::new(0., 0.))]
  );

  owner.dispose();
}

/// A hidden box takes no room and has no rectangle.
#[test]
fn a_hidden_box_takes_no_room() {
  reset();

  let owner = Owner::new();
  let column = owner.enter(|| {
    Column::new()
      .set_children((sized(10., 10.).set_hidden(true), sized(10., 10.)))
      .build()
  });
  let [hidden, shown] = content(column)[..]
  else {
    panic!("two children");
  };

  assert_eq!(layout_rect(hidden), None);
  assert_eq!(layout_rect(shown), Some(rect(0., 0., 10., 10.)));

  owner.dispose();
}

/// A frame reports a box's laid-out size to a handle measuring it, and again as the
/// viewport resizes it; a size reported by hand holds against the layout.
#[test]
fn a_frame_reports_the_laid_out_size() {
  reset();

  let owner = Owner::new();
  let handle = NodeHandle::new();
  let node = owner.enter(move || {
    Container::new()
      .set_width(Dimension::Fraction(0.5))
      .set_height(Dimension::Length(40.))
      .bind_handle(handle)
      .build()
  });

  assert_eq!(handle.size(), None, "nothing is measured before a frame");

  trigger_frame(std::time::Duration::ZERO);

  assert_eq!(handle.size(), Some(Size::new(512., 40.)));

  trigger_resize(Size::new(600., 400.));

  assert_eq!(handle.size(), Some(Size::new(300., 40.)));

  trigger_size(node, Size::new(5., 5.));
  trigger_frame(std::time::Duration::ZERO);

  assert_eq!(
    handle.size(),
    Some(Size::new(5., 5.)),
    "the hand's size holds"
  );

  owner.dispose();
}

/// A box whose width moves under a transition lays out at the width the transition
/// presents at the clock's reading.
#[test]
fn a_box_lays_out_at_the_width_its_motion_presents() {
  reset();

  let owner = Owner::new();
  let width = Signal::new(100f32);
  let node = owner.enter(|| {
    Container::new()
      .derive_width(move || width.get())
      .set_height(Dimension::Length(10.))
      .add_transition(Transition::new(Property::Width, 1.).set_easing(Easing::Linear))
      .build()
  });

  assert_eq!(layout_rect(node).map(|rect| rect.size.width), Some(100.));

  width.set(200.);
  trigger_frame(std::time::Duration::from_millis(500));

  assert_eq!(
    layout_rect(node).map(|rect| rect.size.width),
    Some(150.),
    "halfway through the tween"
  );

  trigger_frame(std::time::Duration::from_millis(500));

  assert_eq!(layout_rect(node).map(|rect| rect.size.width), Some(200.));

  owner.dispose();
}

/// A point hits the topmost node covering it: a child above its parent, a later layer
/// of a stack above an earlier one, and nothing beyond what the layout covers.
#[test]
fn a_point_hits_the_topmost_node() {
  reset();

  let owner = Owner::new();
  let column = owner.enter(|| {
    Column::new()
      .set_children((
        sized(100., 20.),
        Stack::new().set_children((sized(100., 20.), sized(50., 10.))),
      ))
      .build()
  });
  let [first, stack] = content(column)[..]
  else {
    panic!("two children");
  };
  let [under, over] = content(stack)[..]
  else {
    panic!("two layers");
  };

  assert_eq!(node_at(Point::new(10., 5.)), Some(first));
  assert_eq!(node_at(Point::new(10., 25.)), Some(over), "the later layer");
  assert_eq!(
    node_at(Point::new(70., 25.)),
    Some(under),
    "beside the later layer"
  );
  assert_eq!(
    node_at(Point::new(500., 5.)),
    Some(column),
    "the column itself"
  );
  assert_eq!(
    node_at(Point::new(10., 500.)),
    None,
    "below everything laid out"
  );

  owner.dispose();
}
