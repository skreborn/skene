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
  Inset,
  Length,
  MotionBase,
  NodeHandle,
  NodeId,
  Overflow,
  Point,
  Property,
  Rect,
  RunWithTypeMetrics,
  ScrollEvent,
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
  text_width,
  trigger_frame,
  trigger_resize,
  trigger_scroll,
  trigger_size,
  visible_rect,
};
use skene_reactive::{Get, Owner, Set, Signal};

type Column = skene_foundation::Column<Headless>;
type Container = skene_foundation::Container<Headless>;
type Flexible<C = skene_foundation::NoChild> = skene_foundation::Flexible<Headless, C>;
type RichText = skene_foundation::RichText<Headless>;
type Row = skene_foundation::Row<Headless>;
type Span = skene_foundation::Span<Headless>;
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

/// A run measures by the platform's metrics, the pinned face at the font size, and
/// wraps between words at its box's width.
#[test]
fn a_run_measures_by_the_platforms_metrics_and_wraps() {
  reset();

  let owner = Owner::new();
  let column = owner.enter(|| {
    Column::new()
      .set_children((
        Row::new().set_children((Text::new("abcd"),)),
        Container::new()
          .set_width(Dimension::Length(text_width("aa bb", 16.)))
          .set_child(Text::new("aa bb cc")),
      ))
      .build()
  });
  let boxes = content(column);
  let line = content(boxes[0])[0];
  let wrapped = content(boxes[1])[0];

  assert_eq!(
    layout_rect(line).map(|rect| rect.size),
    Some(Size::new(text_width("abcd", 16.), 24.))
  );
  assert_eq!(
    layout_rect(wrapped).map(|rect| rect.size.height),
    Some(48.),
    "two lines: `aa bb` fits, `cc` wraps"
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
    Some(Size::new(text_width("ab", 20.), 40.))
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

/// A box clipping its overflow hides what lies beneath it beyond its padding box from a
/// hit, where the box itself still takes it.
#[test]
fn a_clipping_box_hides_its_overflow_from_a_hit() {
  reset();

  let owner = Owner::new();
  let outer = owner.enter(|| {
    sized(200., 300.)
      .set_child(
        sized(100., 50.)
          .set_border_width(Inset::all(5.))
          .set_overflow(Overflow::Clip)
          .set_child(sized(90., 200.)),
      )
      .build()
  });
  let [clipping] = content(outer)[..]
  else {
    panic!("one child");
  };
  let [overflowing] = content(clipping)[..]
  else {
    panic!("one child");
  };

  assert_eq!(layout_rect(overflowing), Some(rect(5., 5., 90., 200.)));
  assert_eq!(visible_rect(overflowing), Some(rect(5., 5., 90., 40.)));
  assert_eq!(node_at(Point::new(20., 25.)), Some(overflowing));
  assert_eq!(
    node_at(Point::new(20., 47.)),
    Some(clipping),
    "the clipping box's border"
  );
  assert_eq!(
    node_at(Point::new(20., 100.)),
    Some(outer),
    "beyond the clipping box"
  );

  owner.dispose();
}

/// A scrolled box shifts what lies beneath it back by its offsets, and a hit finds what
/// the scroll brought into view.
#[test]
fn a_scrolled_box_shifts_its_content() {
  reset();

  let owner = Owner::new();
  let scrolling = owner.enter(|| {
    sized(100., 50.)
      .set_overflow(Overflow::ScrollIfNeeded)
      .set_child(Column::new().set_children((sized(100., 50.), sized(100., 50.))))
      .build()
  });
  let [column] = content(scrolling)[..]
  else {
    panic!("one child");
  };
  let [first, second] = content(column)[..]
  else {
    panic!("two rows");
  };

  assert_eq!(node_at(Point::new(10., 25.)), Some(first));
  assert_eq!(
    node_at(Point::new(10., 75.)),
    None,
    "the second row, clipped"
  );

  trigger_scroll(scrolling, &ScrollEvent { x: 0., y: 50. });

  assert_eq!(layout_rect(second), Some(rect(0., 0., 100., 50.)));
  assert_eq!(visible_rect(first), Some(rect(0., 0., 100., 0.)));
  assert_eq!(node_at(Point::new(10., 25.)), Some(second));

  owner.dispose();
}

/// A paragraph's spans flow on one line, each where its text is, a group spanning its
/// own, and a point finds the span drawn there.
#[test]
fn a_paragraphs_spans_flow_on_one_line() {
  reset();

  let owner = Owner::new();
  let paragraph = owner.enter(|| {
    RichText::new()
      .add_spans([
        Span::new("Hello "),
        Span::group([Span::new("wor"), Span::new("ld")]),
      ])
      .build()
  });
  let [hello, group] = content(paragraph)[..]
  else {
    panic!("two spans");
  };
  let [wor, ld] = content(group)[..]
  else {
    panic!("two spans in the group");
  };
  let start = text_width("Hello ", 16.);
  let middle = start + text_width("wor", 16.);

  assert_eq!(layout_rect(paragraph), Some(rect(0., 0., 1024., 24.)));
  assert_eq!(layout_rect(hello), Some(rect(0., 0., start, 24.)));
  assert_eq!(
    layout_rect(group),
    Some(rect(start, 0., text_width("world", 16.), 24.))
  );
  assert_eq!(
    layout_rect(wor),
    Some(rect(start, 0., text_width("wor", 16.), 24.))
  );
  assert_eq!(
    layout_rect(ld),
    Some(rect(middle, 0., text_width("ld", 16.), 24.))
  );
  assert_eq!(node_at(Point::new(start + 2., 10.)), Some(wor));
  assert_eq!(
    node_at(Point::new(200., 10.)),
    Some(paragraph),
    "beyond the text"
  );

  owner.dispose();
}

/// A paragraph wraps across its spans, and a span broken across lines covers each of
/// its pieces, spanning them, but takes a press only where a piece is.
#[test]
fn a_paragraph_wraps_across_its_spans() {
  reset();

  let width = text_width("c aaa", 16.);
  let owner = Owner::new();
  let narrow = owner.enter(|| {
    Container::new()
      .set_width(Dimension::Length(width))
      .set_child(RichText::new().add_spans([Span::new("c "), Span::new("aaa bb")]))
      .build()
  });
  let [paragraph] = content(narrow)[..]
  else {
    panic!("one paragraph");
  };
  let [before, broken] = content(paragraph)[..]
  else {
    panic!("two spans");
  };
  let start = text_width("c ", 16.);

  assert_eq!(layout_rect(paragraph), Some(rect(0., 0., width, 48.)));
  assert_eq!(layout_rect(before), Some(rect(0., 0., start, 24.)));
  assert_eq!(layout_rect(broken), Some(rect(0., 0., width, 48.)));
  assert_eq!(
    node_at(Point::new(start + 2., 10.)),
    Some(broken),
    "its first piece"
  );
  assert_eq!(
    node_at(Point::new(2., 30.)),
    Some(broken),
    "its second piece"
  );
  assert_eq!(
    node_at(Point::new(text_width("bb", 16.) + 4., 30.)),
    Some(paragraph),
    "beside its second piece"
  );

  owner.dispose();
}

/// A line is as tall as its tallest span, a smaller span sitting on its bottom.
#[test]
fn a_line_is_as_tall_as_its_tallest_span() {
  reset();

  let owner = Owner::new();
  let paragraph = owner.enter(|| {
    RichText::new()
      .add_spans([
        Span::new("big ").set_font_size(Length(32.)),
        Span::new("small"),
      ])
      .build()
  });
  let [big, small] = content(paragraph)[..]
  else {
    panic!("two spans");
  };
  let start = text_width("big ", 32.);

  assert_eq!(layout_rect(paragraph), Some(rect(0., 0., 1024., 48.)));
  assert_eq!(layout_rect(big), Some(rect(0., 0., start, 48.)));
  assert_eq!(
    layout_rect(small),
    Some(rect(start, 24., text_width("small", 16.), 24.))
  );

  owner.dispose();
}

/// An inline box lays out at its content's size among its paragraph's text, its line as
/// tall as the box, and a point finds the view inside it.
#[test]
fn an_inline_box_flows_among_its_paragraphs_text() {
  reset();

  let owner = Owner::new();
  let paragraph = owner.enter(|| {
    RichText::new()
      .add_spans([
        Span::new("ab "),
        Span::view(sized(20., 30.)),
        Span::new(" cd"),
      ])
      .build()
  });
  let [before, inline, after] = content(paragraph)[..]
  else {
    panic!("three spans");
  };
  let [view] = content(inline)[..]
  else {
    panic!("one view");
  };
  let start = text_width("ab ", 16.);

  assert_eq!(layout_rect(paragraph), Some(rect(0., 0., 1024., 30.)));
  assert_eq!(layout_rect(before), Some(rect(0., 6., start, 24.)));
  assert_eq!(layout_rect(inline), Some(rect(start, 0., 20., 30.)));
  assert_eq!(layout_rect(view), Some(rect(start, 0., 20., 30.)));
  assert_eq!(
    layout_rect(after),
    Some(rect(start + 20., 6., text_width(" cd", 16.), 24.))
  );
  assert_eq!(node_at(Point::new(start + 6., 10.)), Some(view));

  owner.dispose();
}

/// A line may end before an inline box with no blank between them.
#[test]
fn a_line_may_end_before_an_inline_box() {
  reset();

  let owner = Owner::new();
  let narrow = owner.enter(|| {
    Container::new()
      .set_width(Dimension::Length(30.))
      .set_child(RichText::new().add_spans([Span::new("abc"), Span::view(sized(20., 30.))]))
      .build()
  });
  let [paragraph] = content(narrow)[..]
  else {
    panic!("one paragraph");
  };
  let [_, inline] = content(paragraph)[..]
  else {
    panic!("two spans");
  };

  assert_eq!(layout_rect(paragraph), Some(rect(0., 0., 30., 54.)));
  assert_eq!(layout_rect(inline), Some(rect(0., 24., 20., 30.)));

  owner.dispose();
}

/// The layout follows the scene from frame to frame: rows reordered, removed and added,
/// and a row's width moved by a signal.
#[test]
fn the_layout_follows_the_scene_across_frames() {
  reset();

  let owner = Owner::new();
  let (rows, width) = owner.enter(|| (Signal::new(vec![1_u8, 2, 3]), Signal::new(40_f32)));
  let column = owner.enter(|| {
    Column::new()
      .derive_children(
        move || rows.get(),
        |row| *row,
        move |row| {
          Container::new()
            .derive_width(move || if row == 2 { width.get() } else { 10. })
            .set_height(Dimension::Length(10. * f32::from(row)))
        },
      )
      .build()
  });
  let [one, two, three] = content(column)[..]
  else {
    panic!("three rows");
  };

  assert_eq!(layout_rect(two), Some(rect(0., 10., 40., 20.)));
  assert_eq!(layout_rect(three), Some(rect(0., 30., 10., 30.)));

  rows.set(vec![3, 2, 4]);
  width.set(60.);

  let [moved, kept, added] = content(column)[..]
  else {
    panic!("three rows");
  };

  assert_eq!((moved, kept), (three, two));
  assert_eq!(layout_rect(three), Some(rect(0., 0., 10., 30.)));
  assert_eq!(layout_rect(two), Some(rect(0., 30., 60., 20.)));
  assert_eq!(layout_rect(added), Some(rect(0., 50., 10., 40.)));
  assert_eq!(layout_rect(one), None, "the removed row");

  owner.dispose();
}

/// A paragraph breaks where the Unicode line breaking algorithm allows, after a hyphen
/// as after a blank.
#[test]
fn a_paragraph_breaks_after_a_hyphen() {
  reset();

  let owner = Owner::new();
  let narrow = owner.enter(|| {
    Container::new()
      .set_width(Dimension::Length(56.))
      .set_child(RichText::new().add_spans([Span::new("well-known")]))
      .build()
  });
  let [paragraph] = content(narrow)[..]
  else {
    panic!("one paragraph");
  };
  let [span] = content(paragraph)[..]
  else {
    panic!("one span");
  };

  assert_eq!(layout_rect(paragraph), Some(rect(0., 0., 56., 48.)));
  assert_eq!(
    layout_rect(span),
    Some(rect(0., 0., text_width("known", 16.), 48.))
  );

  owner.dispose();
}

/// A run's letter spacing widens each of its clusters by it.
#[test]
fn letter_spacing_widens_each_cluster() {
  reset();

  let owner = Owner::new();
  let row = owner.enter(|| {
    Row::new()
      .set_children((Text::new("abc").set_letter_spacing(Length(2.)),))
      .build()
  });
  let [run] = content(row)[..]
  else {
    panic!("one run");
  };

  assert_eq!(
    layout_rect(run).map(|rect| rect.size.width),
    Some(text_width("abc", 16.) + 6.)
  );

  owner.dispose();
}
