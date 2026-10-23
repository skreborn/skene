//! The layout the headless platform computes through the shared layout: each box's
//! rectangle at the viewport, a run measured by the platform's metrics, and the sizes a
//! frame reports.

use skene_foundation::{
  Condition,
  ContainerBase,
  DecorationBase,
  Dimension,
  Easing,
  FlexBase,
  Flexibility,
  HasChild,
  HasChildren,
  Inset,
  ItemAlignment,
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
  State,
  TextEntry,
  Transition,
  View,
  When,
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
  style_resolutions,
  text_width,
  trigger_frame,
  trigger_pointer_enter,
  trigger_pointer_leave,
  trigger_resize,
  trigger_scroll,
  trigger_size,
  visible_rect,
};
use skene_reactive::{Get, Owner, Set, Signal};

type BasicTextEditor = skene_foundation::BasicTextEditor<Headless>;
type BasicTextField = skene_foundation::BasicTextField<Headless>;
type Column = skene_foundation::Column<Headless>;
type Container = skene_foundation::Container<Headless>;
type Flexible = skene_foundation::Flexible<Headless>;
type RichText = skene_foundation::RichText<Headless>;
type Row = skene_foundation::Row<Headless>;
type Span = skene_foundation::Span<Headless>;
type Stack = skene_foundation::Stack<Headless>;
type Text = skene_foundation::Text<Headless>;
type Theme<S> = skene_foundation::Theme<Headless, S>;

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

/// How far above the baseline the pinned face's capital letters reach at `size`.
fn cap(size: f32) -> f32 {
  1490. * size / 2048.
}

/// How far below the top of a line box `height` tall the pinned face's baseline lies at
/// `size`: the box's room beyond the face's ascent and descent split above and below.
fn baseline(height: f32, size: f32) -> f32 {
  let scale = size / 2048.;

  (height - 2478. * scale) / 2. + 1984. * scale
}

/// Whether `actual` is `expected`, each edge within a ten-thousandth of a pixel.
fn near(actual: Option<Rect<f32>>, expected: Rect<f32>) -> bool {
  actual.is_some_and(|actual| {
    [
      (actual.origin.x, expected.origin.x),
      (actual.origin.y, expected.origin.y),
      (actual.size.width, expected.size.width),
      (actual.size.height, expected.size.height),
    ]
    .iter()
    .all(|(actual, expected)| (actual - expected).abs() < 1e-4)
  })
}

/// Asserts that the rectangle `node` lays out in is near `expected`.
#[track_caller]
fn assert_rect(node: NodeId, expected: Rect<f32>) {
  let actual = layout_rect(node);

  assert!(near(actual, expected), "{actual:?} is not {expected:?}");
}

/// A column lays its children out down the viewport, the spacing between them.
#[test]
fn a_column_lays_its_children_out_in_turn() {
  reset();

  let owner = Owner::new();
  let column = owner.enter(|| {
    Column::new()
      .set_spacing(10)
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

/// A wrapper without its child takes no room in its column and adds no spacing.
#[test]
fn a_wrapper_without_its_child_takes_no_room() {
  reset();

  let owner = Owner::new();
  let column = owner.enter(|| {
    Column::new()
      .set_spacing(10)
      .set_children((
        sized(100., 20.),
        Flexible::new().set_flexibility(Flexibility::grow(1.)),
        sized(50., 30.),
      ))
      .build()
  });
  let [first, second] = content(column)[..]
  else {
    panic!("two children");
  };

  assert_eq!(layout_rect(first), Some(rect(0., 0., 100., 20.)));
  assert_eq!(layout_rect(second), Some(rect(0., 30., 50., 30.)));

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

  assert_rect(line, rect(0., 0., text_width("abcd", 16.), cap(16.)));
  assert!(
    layout_rect(wrapped).is_some_and(|rect| (rect.size.height - 24. - cap(16.)).abs() < 1e-4),
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
      .set_child_builder(|| {
        Column::new().set_children((Row::new().set_children((Text::new("ab"),)),))
      })
      .build()
  });
  let run = find_by_text("ab").expect("the run");

  assert_rect(run, rect(0., 0., text_width("ab", 20.), cap(20.)));

  owner.dispose();
}

/// A flexible child takes the room its row leaves, and a stack lays every child out in
/// its one cell.
#[test]
fn a_flexible_child_grows_and_a_stack_overlays() {
  reset();

  let owner = Owner::new();
  let page = owner.enter(|| {
    Column::new()
      .set_children((
        Container::new().set_width(300).set_child(
          Row::new().set_children((
            Flexible::new()
              .set_flexibility(Flexibility::grow(1.))
              .set_child(sized(0., 10.)),
            sized(100., 10.),
          )),
        ),
        Stack::new().set_children((sized(40., 40.), sized(20., 20.))),
      ))
      .build()
  });
  let [row, stack] = content(page)[..]
  else {
    panic!("two children");
  };
  let grown = content(content(row)[0])[0];
  let layers = content(stack);

  assert_eq!(layout_rect(grown).map(|rect| rect.size.width), Some(200.));
  assert_eq!(
    layers
      .iter()
      .map(|layer| layout_rect(*layer).map(|rect| rect.origin))
      .collect::<Vec<_>>(),
    [Some(Point::new(0., 10.)), Some(Point::new(0., 10.))]
  );

  owner.dispose();
}

/// A hidden box keeps its place in the layout, as the web's `visibility: hidden` keeps
/// it, though nothing at it takes a hit.
#[test]
fn a_hidden_box_keeps_its_place_but_takes_no_hit() {
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

  assert_eq!(layout_rect(hidden), Some(rect(0., 0., 10., 10.)));
  assert_eq!(layout_rect(shown), Some(rect(0., 10., 10., 10.)));
  assert_ne!(node_at(Point::new(5., 5.)), Some(hidden));
  assert_eq!(node_at(Point::new(5., 15.)), Some(shown));

  owner.dispose();
}

/// A frame reports a box's laid-out size to a handle measuring it, and again as the
/// viewport resizes it; a size reported by hand holds against the layout.
#[test]
fn a_frame_reports_the_laid_out_size() {
  reset();

  let owner = Owner::new();
  let handle = NodeHandle::new();
  let page = owner.enter(move || {
    Column::new()
      .set_children((Container::new()
        .set_width(Dimension::Fraction(0.5))
        .set_height(40)
        .bind_handle(handle),))
      .build()
  });
  let [node] = content(page)[..]
  else {
    panic!("one child");
  };

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
  let page = owner.enter(|| {
    Column::new()
      .set_children((Container::new()
        .derive_width(move || width.get())
        .set_height(10)
        .add_transition(Transition::new(Property::Width, 1.).set_easing(Easing::Linear)),))
      .build()
  });
  let [node] = content(page)[..]
  else {
    panic!("one child");
  };

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
/// of a stack above an earlier one, the first box anywhere in the viewport it fills, and
/// nothing beyond the viewport.
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
    Some(column),
    "the column, filling the viewport"
  );
  assert_eq!(node_at(Point::new(10., 800.)), None, "beyond the viewport");

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
          .set_border_width(Inset::all(5))
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
  let page = owner.enter(|| {
    Column::new()
      .set_children((sized(100., 50.)
        .set_overflow(Overflow::ScrollIfNeeded)
        .set_child(Column::new().set_children((sized(100., 50.), sized(100., 50.)))),))
      .build()
  });
  let [scrolling] = content(page)[..]
  else {
    panic!("one child");
  };
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
    Some(page),
    "the page beneath, the second row clipped"
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
  let page = owner.enter(|| {
    Column::new()
      .set_children((RichText::new().add_spans([
        Span::new("Hello "),
        Span::group([Span::new("wor"), Span::new("ld")]),
      ]),))
      .build()
  });
  let [paragraph] = content(page)[..]
  else {
    panic!("one child");
  };
  let [hello, group] = content(paragraph)[..]
  else {
    panic!("two spans");
  };
  let [wor, ld] = content(group)[..]
  else {
    panic!("two spans in the group");
  };
  let start = text_width("Hello ", 16.);
  let end = start + text_width("world", 16.);
  let middle = end - text_width("ld", 16.);

  let top = cap(16.) - baseline(24., 16.);

  assert_rect(paragraph, rect(0., 0., 1024., cap(16.)));
  assert_rect(hello, rect(0., top, start, 24.));
  assert_rect(group, rect(start, top, end - start, 24.));
  assert_rect(wor, rect(start, top, middle - start, 24.));
  assert_rect(ld, rect(middle, top, text_width("ld", 16.), 24.));
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
  let page = owner.enter(|| {
    Column::new()
      .set_children((Container::new()
        .set_width(Dimension::Length(width))
        .set_child(RichText::new().add_spans([Span::new("c "), Span::new("aaa bb")])),))
      .build()
  });
  let [narrow] = content(page)[..]
  else {
    panic!("one child");
  };
  let [paragraph] = content(narrow)[..]
  else {
    panic!("one paragraph");
  };
  let [before, broken] = content(paragraph)[..]
  else {
    panic!("two spans");
  };
  let start = text_width("c ", 16.);

  let top = cap(16.) - baseline(24., 16.);

  assert_rect(paragraph, rect(0., 0., width, 24. + cap(16.)));
  assert_rect(before, rect(0., top, start, 24.));
  assert_rect(broken, rect(0., top, width, 48.));
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
  let page = owner.enter(|| {
    Column::new()
      .set_children((RichText::new().add_spans([
        Span::new("big ").set_font_size(Length(32.)),
        Span::new("small"),
      ]),))
      .build()
  });
  let [paragraph] = content(page)[..]
  else {
    panic!("one child");
  };
  let [big, small] = content(paragraph)[..]
  else {
    panic!("two spans");
  };
  let start = text_width("big ", 32.);

  assert_rect(paragraph, rect(0., 0., 1024., cap(32.)));
  assert_rect(big, rect(0., cap(32.) - baseline(48., 32.), start, 48.));
  assert_rect(
    small,
    rect(
      start,
      cap(32.) - baseline(24., 16.),
      text_width("small", 16.),
      24.,
    ),
  );

  owner.dispose();
}

/// An inline box lays out at its content's size among its paragraph's text, its line as
/// tall as the box, and a point finds the view inside it.
#[test]
fn an_inline_box_flows_among_its_paragraphs_text() {
  reset();

  let owner = Owner::new();
  let page = owner.enter(|| {
    Column::new()
      .set_children((RichText::new().add_spans([
        Span::new("ab "),
        Span::view(sized(20., 30.)),
        Span::new(" cd"),
      ]),))
      .build()
  });
  let [paragraph] = content(page)[..]
  else {
    panic!("one child");
  };
  let [before, inline, after] = content(paragraph)[..]
  else {
    panic!("three spans");
  };
  let [view] = content(inline)[..]
  else {
    panic!("one view");
  };
  let start = text_width("ab ", 16.);

  let top = 30. - baseline(24., 16.);

  assert_rect(paragraph, rect(0., 0., 1024., 30.));
  assert_rect(before, rect(0., top, start, 24.));
  assert_rect(inline, rect(start, 0., 20., 30.));
  assert_rect(view, rect(start, 0., 20., 30.));
  assert_rect(after, rect(start + 20., top, text_width(" cd", 16.), 24.));
  assert_eq!(node_at(Point::new(start + 6., 10.)), Some(view));

  owner.dispose();
}

/// A line may end before an inline box with no blank between them.
#[test]
fn a_line_may_end_before_an_inline_box() {
  reset();

  let owner = Owner::new();
  let page = owner.enter(|| {
    Column::new()
      .set_children((Container::new()
        .set_width(30)
        .set_child(RichText::new().add_spans([Span::new("abc"), Span::view(sized(20., 30.))])),))
      .build()
  });
  let [narrow] = content(page)[..]
  else {
    panic!("one child");
  };
  let [paragraph] = content(narrow)[..]
  else {
    panic!("one paragraph");
  };
  let [_, inline] = content(paragraph)[..]
  else {
    panic!("two spans");
  };

  let second = 24. - baseline(24., 16.) + cap(16.);

  assert_rect(paragraph, rect(0., 0., 30., second + 30.));
  assert_rect(inline, rect(0., second, 20., 30.));

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
  let page = owner.enter(|| {
    Column::new()
      .set_children((Container::new()
        .set_width(56)
        .set_child(RichText::new().add_spans([Span::new("well-known")])),))
      .build()
  });
  let [narrow] = content(page)[..]
  else {
    panic!("one child");
  };
  let [paragraph] = content(narrow)[..]
  else {
    panic!("one paragraph");
  };
  let [span] = content(paragraph)[..]
  else {
    panic!("one span");
  };

  assert_rect(paragraph, rect(0., 0., 56., 24. + cap(16.)));
  assert_rect(
    span,
    rect(
      0.,
      cap(16.) - baseline(24., 16.),
      text_width("known", 16.),
      48.,
    ),
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
      .set_children((Text::new("abc").set_letter_spacing(2),))
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

/// A text entry lays out at the size its native element takes in Chromium: a field
/// nineteen average characters and the width of the face's bounds wide and a line
/// tall, an editor twenty average characters wide and its lines tall.
#[test]
fn a_text_entry_lays_out_at_its_native_size() {
  reset();

  let owner = Owner::new();
  let page = owner.enter(|| {
    Column::new()
      .set_item_alignment(ItemAlignment::Start)
      .set_children((
        BasicTextField::new(TextEntry::new()),
        BasicTextEditor::new(TextEntry::new().set_lines(3)),
      ))
      .build()
  });
  let [field, editor] = content(page)[..]
  else {
    panic!("two entries");
  };
  let average = 1311. * 16. / 2048.;
  let bounds = ((5290_f32 + 1513.) * 16. / 2048.).round();

  assert_eq!(
    layout_rect(field).map(|rect| rect.size),
    Some(Size::new((average * 19. + bounds).ceil(), 24.))
  );
  assert_eq!(
    layout_rect(editor).map(|rect| rect.size),
    Some(Size::new((average * 20_f32).ceil(), 72.))
  );

  owner.dispose();
}

/// A text entry stretches across its layout's cross axis as an `<input>` does: across a
/// column's width, and to a row's height.
#[test]
fn a_text_entry_stretches_across_its_layout() {
  reset();

  let owner = Owner::new();
  let page = owner.enter(|| {
    Column::new()
      .set_children((Container::new()
        .set_width(400)
        .set_child(Column::new().set_children((
          BasicTextField::new(TextEntry::new()),
          Row::new().set_children((sized(10., 100.), BasicTextField::new(TextEntry::new()))),
        ))),))
      .build()
  });
  let [across, row] = content(content(content(page)[0])[0])[..]
  else {
    panic!("an entry and a row");
  };
  let [_, tall] = content(row)[..]
  else {
    panic!("a box and an entry");
  };

  assert_eq!(layout_rect(across).map(|rect| rect.size.width), Some(400.));
  assert_eq!(layout_rect(tall).map(|rect| rect.size.height), Some(100.));

  owner.dispose();
}

/// A change to one box's style resolves that box alone at the next layout, which lays it
/// out afresh beside the boxes it kept; a change to a run's text resolves the paragraph
/// it flows in alone.
#[test]
fn a_change_resolves_only_what_it_touched() {
  reset();

  let owner = Owner::new();
  let width = Signal::new(10_f32);
  let words = Signal::new("one".to_owned());
  let page = owner.enter(|| {
    Column::new()
      .set_children((
        Container::new()
          .derive_width(move || width.get())
          .set_height(10),
        sized(10., 10.),
        RichText::new().add_spans([Span::derive(move || words.get())]),
        sized(10., 10.),
      ))
      .build()
  });
  let [changing, kept, paragraph, _] = content(page)[..]
  else {
    panic!("four children");
  };

  assert_eq!(layout_rect(changing).map(|rect| rect.size.width), Some(10.));

  let resolved = style_resolutions();

  width.set(30.);

  assert_eq!(layout_rect(changing).map(|rect| rect.size.width), Some(30.));
  assert_eq!(layout_rect(kept), Some(rect(0., 10., 10., 10.)));
  assert_eq!(style_resolutions() - resolved, 1);

  let resolved = style_resolutions();

  words.set("one two".to_owned());

  assert_eq!(
    layout_rect(content(paragraph)[0]).map(|rect| rect.size.width),
    Some(text_width("one two", 16.))
  );
  assert_eq!(style_resolutions() - resolved, 1);

  owner.dispose();
}

/// A box sized by its interaction lays out afresh as the interaction moves.
#[test]
fn a_box_sized_by_its_hover_lays_out_as_it_moves() {
  reset();

  let owner = Owner::new();
  let page = owner.enter(|| {
    Column::new()
      .set_children((
        Container::new()
          .set_width(10.when(Condition::from(State::Hovered), 30))
          .set_height(10),
        sized(10., 10.),
      ))
      .build()
  });
  let [hovered, _] = content(page)[..]
  else {
    panic!("two children");
  };

  assert_eq!(layout_rect(hovered).map(|rect| rect.size.width), Some(10.));

  trigger_pointer_enter(hovered);

  assert_eq!(layout_rect(hovered).map(|rect| rect.size.width), Some(30.));

  trigger_pointer_leave(hovered);

  assert_eq!(layout_rect(hovered).map(|rect| rect.size.width), Some(10.));

  owner.dispose();
}

/// Text keeps its blanks as CSS's `white-space: pre-wrap` keeps them: the blanks
/// starting or ending the text take their room.
#[test]
fn text_keeps_its_blanks() {
  reset();

  let owner = Owner::new();
  let column = owner.enter(|| {
    Column::new()
      .set_children((
        Row::new().set_children((Text::new("ab   "),)),
        Row::new().set_children((Text::new("   ab"),)),
      ))
      .build()
  });
  let width =
    |index: usize| layout_rect(content(content(column)[index])[0]).map(|rect| rect.size.width);

  assert_eq!(width(0), Some(text_width("ab   ", 16.)));
  assert_eq!(width(1), Some(text_width("   ab", 16.)));

  owner.dispose();
}

/// A newline breaks its line, a newline ending the text opens a line, and an empty text
/// lays out one.
#[test]
fn a_newline_breaks_its_line_and_a_final_one_opens_another() {
  reset();

  let owner = Owner::new();
  let column = owner.enter(|| {
    Column::new()
      .set_children((
        Row::new().set_children((Text::new("a"),)),
        Row::new().set_children((Text::new("a\n"),)),
        Row::new().set_children((Text::new("a\n\n"),)),
        Row::new().set_children((Text::new(""),)),
      ))
      .build()
  });
  let height =
    |index: usize| layout_rect(content(content(column)[index])[0]).map(|rect| rect.size.height);

  assert_eq!(height(0), Some(cap(16.)));
  assert_eq!(height(1), Some(24. + cap(16.)));
  assert_eq!(height(2), Some(48. + cap(16.)));
  assert_eq!(height(3), Some(cap(16.)));

  owner.dispose();
}

/// The blanks ending a line where it wraps take no room on it.
#[test]
fn blanks_hang_where_a_line_wraps() {
  reset();

  let owner = Owner::new();
  let column = owner.enter(|| {
    Column::new()
      .set_children((Container::new()
        .set_width(40)
        .set_child(Text::new("aaa     bbb")),))
      .build()
  });
  let hung = content(content(column)[0])[0];

  assert_eq!(
    layout_rect(hung).map(|rect| rect.size.height),
    Some(24. + cap(16.))
  );

  owner.dispose();
}

/// A word too wide for a line of its own breaks where the line fills, as the page's
/// column narrows to the viewport, though its min-content width, which a row's item keeps,
/// is the widest word's.
#[test]
fn a_word_too_wide_for_its_line_breaks_where_it_fills() {
  reset();
  trigger_resize(Size::new(40., 400.));

  let owner = Owner::new();
  let column = owner.enter(|| {
    Column::new()
      .set_children((
        Text::new("aaaaaaaaaaaaaaaaaaaa"),
        Row::new().set_children((Text::new("aa aaaaaaaaaa b"),)),
      ))
      .build()
  });
  let [broken, row] = content(column)[..]
  else {
    panic!("a run and a row");
  };

  assert!(
    layout_rect(broken).is_some_and(|rect| (rect.size.height - 4. * 24. - cap(16.)).abs() < 1e-4),
    "five lines of four letters each"
  );
  assert_eq!(
    layout_rect(content(row)[0]).map(|rect| rect.size.width),
    Some(text_width("aaaaaaaaaa", 16.))
  );

  owner.dispose();
}
