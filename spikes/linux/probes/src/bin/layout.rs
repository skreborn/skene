//! Layout on real GTK: every container is a SkeneBox whose layout manager runs the engine over
//! GTK's own measure; rows, columns, wrapping rows, grids, stacks, a box's alignment, weights,
//! fractions, bounds, the toolkit's minimum as a floor, right to left, and a scroll as a
//! measured leaf.

use probes::{result, run, show, widget, xdotool};
use skene_gui::{
  platform::linux::{
    Button as GtkButton,
    gtk::{self, prelude::*},
  },
  prelude::*,
};

type Rect = (f32, f32, f32, f32);

fn sized(width: i32, height: i32, handle: &NodeHandle) -> Container {
  Container::new()
    .set_width(width)
    .set_height(height)
    .set_background(Color::rgb(0x3366cc))
    .set_handle(handle)
}

fn handles<const N: usize>() -> [NodeHandle; N] {
  std::array::from_fn(|_| NodeHandle::new())
}

fn body() {
  // A row: its padding, its spacing, its children at their sizes, aligned to the start.
  let [a, b] = handles();
  let shown = show(400, -1, || {
    Row::new()
      .set_padding(8)
      .set_spacing(10)
      .set_item_alignment(AxisAlignment::Start)
      .add_child(sized(50, 20, &a))
      .add_child(sized(30, 40, &b))
  });
  let rects: Vec<Rect> = [&a, &b].iter().map(|handle| shown.rect(handle)).collect();

  result(
    "row: padding 8, spacing 10, start",
    rects == [(8., 8., 50., 20.), (68., 8., 30., 40.)],
    format!("{rects:?}"),
  );

  shown.close();

  // A column centers its items across.
  let [a, b] = handles();
  let shown = show(300, -1, || {
    Column::new()
      .set_spacing(6)
      .set_item_alignment(AxisAlignment::Center)
      .add_child(sized(100, 20, &a))
      .add_child(sized(40, 20, &b))
  });
  let rects: Vec<Rect> = [&a, &b].iter().map(|handle| shown.rect(handle)).collect();

  result(
    "column: spacing 6, centered",
    rects == [(100., 0., 100., 20.), (130., 26., 40., 20.)],
    format!("{rects:?}"),
  );

  shown.close();

  // A row aligns texts of two sizes on their first baseline, as measured by GTK.
  let [small, large] = handles();
  let shown = show(400, -1, || {
    Row::new()
      .set_spacing(8)
      .set_item_alignment(AxisAlignment::Baseline)
      .add_child(Text::new("Small").set_font_size(12).set_handle(&small))
      .add_child(Text::new("Large").set_font_size(32).set_handle(&large))
  });
  let baseline = |handle: &NodeHandle| {
    let rect = shown.rect(handle);
    let measured = widget(handle).measure(gtk::Orientation::Vertical, rect.2 as i32);

    rect.1 + measured.3 as f32
  };
  let (small_line, large_line) = (baseline(&small), baseline(&large));

  result(
    "baseline alignment of 12 and 32 pixel texts",
    (small_line - large_line).abs() <= 1. && shown.rect(&small).1 > 5.,
    format!("baselines at {small_line} and {large_line}"),
  );

  shown.close();

  // Flexible weights share what is left: 1 and 3 of 400.
  let [a, b] = handles();
  let shown = show(400, -1, || {
    Row::new()
      .add_child(
        Flexible::new()
          .set_flexibility(1)
          .set_child(Container::new().set_height(10).set_handle(&a)),
      )
      .add_child(
        Flexible::new()
          .set_flexibility(3)
          .set_child(Container::new().set_height(10).set_handle(&b)),
      )
  });
  let widths = (shown.rect(&a).2, shown.rect(&b).2);

  result(
    "flexible weights 1:3",
    widths == (100., 300.),
    format!("{widths:?}"),
  );

  shown.close();

  // A fraction of the parent, and bounds.
  let [half, capped, tall] = handles();
  let shown = show(400, -1, || {
    Column::new()
      .add_child(
        Container::new()
          .set_width(Dimension::Fraction(0.5))
          .set_height(10)
          .set_handle(&half),
      )
      .add_child(
        Container::new()
          .set_max_width(120)
          .set_height(10)
          .set_handle(&capped),
      )
      .add_child(Container::new().set_min_height(50).set_handle(&tall))
  });
  let (half, capped, tall) = (shown.rect(&half), shown.rect(&capped), shown.rect(&tall));

  result("fraction 0.5 of 400", half.2 == 200., format!("{half:?}"));
  result(
    "max width 120 in a stretching column",
    capped.2 == 120.,
    format!("{capped:?}"),
  );
  result("min height 50", tall.3 == 50., format!("{tall:?}"));
  shown.close();

  // The toolkit's minimum is a floor: a GTK button in a row too narrow for it keeps the width
  // GTK says it needs, where the engine alone would shrink it.
  let [button, other] = handles();
  let shown = show(400, -1, || {
    Container::new().set_width(120).set_child(
      Row::new()
        .add_child(GtkButton::new("A rather long caption").set_handle(&button))
        .add_child(GtkButton::new("Another long one").set_handle(&other)),
    )
  });
  let minimum = widget(&button).measure(gtk::Orientation::Horizontal, -1).0 as f32;
  let width = shown.rect(&button).2;

  result(
    "the GTK minimum is a floor",
    width >= minimum && minimum > 60.,
    format!("allocated {width}, GTK minimum {minimum}, row 120"),
  );

  shown.close();

  // A wrapping row: 50 wide children, spacing 10, in 200: three a line.
  let items: [NodeHandle; 5] = handles();
  let shown = show(200, -1, || {
    Row::new()
      .set_wrap(true)
      .set_spacing(10)
      .add_children(items.iter().map(|item| sized(50, 20, item)))
  });
  let rects: Vec<Rect> = items.iter().map(|item| shown.rect(item)).collect();

  result(
    "wrapping row: three a line, the fourth on the next",
    rects[2].0 == 120. && rects[3] == (0., 30., 50., 20.) && rects[4].0 == 60.,
    format!("{rects:?}"),
  );

  shown.close();

  // A grid: a fixed track and a flexible one.
  let [first, second, third] = handles();
  let shown = show(300, -1, || {
    Grid::new()
      .set_columns(Tracks::from([GridTrack::Length(100.), GridTrack::Flex(1.)]))
      .set_spacing(4)
      .add_child(Container::new().set_height(20).set_handle(&first))
      .add_child(Container::new().set_height(20).set_handle(&second))
      .add_child(Container::new().set_height(20).set_handle(&third))
  });
  let rects: Vec<Rect> = [&first, &second, &third]
    .iter()
    .map(|handle| shown.rect(handle))
    .collect();

  result(
    "grid: 100 and 1fr columns, spacing 4",
    rects
      == [
        (0., 0., 100., 20.),
        (104., 0., 196., 20.),
        (0., 24., 100., 20.),
      ],
    format!("{rects:?}"),
  );

  shown.close();

  // A grid flowing down its columns into implicit ones, and a cell spanning two columns.
  let cells: [NodeHandle; 5] = handles();
  let span = NodeHandle::new();
  let shown = show(400, -1, || {
    Grid::new()
      .set_columns(Tracks::from([
        GridTrack::Length(60.),
        GridTrack::Length(60.),
      ]))
      .set_rows(Tracks::from([
        GridTrack::Length(20.),
        GridTrack::Length(20.),
      ]))
      .set_implicit_columns(Tracks::from([GridTrack::Length(30.)]))
      .set_implicit_rows(Tracks::from([GridTrack::Length(20.)]))
      .set_flow(Flow::Columns)
      .set_item_alignment(Alignment::STRETCH)
      .add_children(cells.iter().map(|cell| Container::new().set_handle(cell)))
      .add_child(
        GridCell::new()
          .set_cell(GridPlacement::at(3, 1).spanning(1, 2))
          .set_child(Container::new().set_handle(&span)),
      )
  });
  let rects: Vec<Rect> = cells.iter().map(|cell| shown.rect(cell)).collect();
  let spanned = shown.rect(&span);

  result(
    "grid flow by columns into an implicit 30 column",
    rects[1] == (0., 20., 60., 20.) && rects[2].0 == 60. && rects[4] == (120., 0., 30., 20.),
    format!("{rects:?}"),
  );
  result(
    "grid cell at row 3 spanning two columns",
    spanned == (0., 40., 120., 20.),
    format!("{spanned:?}"),
  );

  shown.close();

  // A stack: the first child fills it, a pinned one keeps 4 from the bottom trailing corner.
  let [back, badge] = handles();
  let shown = show(200, -1, || {
    Stack::new()
      .add_child(Container::new().set_height(100).set_handle(&back))
      .add_child(
        Pinned::new()
          .set_edges(Edges {
            bottom: Some(4.),
            trailing: Some(4.),
            ..Edges::default()
          })
          .set_child(sized(20, 20, &badge)),
      )
  });
  let (back, badge) = (shown.rect(&back), shown.rect(&badge));

  result(
    "stack: a pinned badge 4 from the bottom trailing corner",
    back == (0., 0., 200., 100.) && badge == (176., 76., 20., 20.),
    format!("back {back:?} badge {badge:?}"),
  );

  shown.close();

  // A box centers its child.
  let child = NodeHandle::new();
  let shown = show(200, -1, || {
    Container::new()
      .set_height(100)
      .set_alignment(Alignment::CENTER)
      .set_child(sized(50, 20, &child))
  });
  let rect = shown.rect(&child);

  result(
    "a box centers its child",
    rect == (75., 40., 50., 20.),
    format!("{rect:?}"),
  );

  shown.close();

  // Right to left mirrors a row: the leading child sits at the right.
  gtk::Widget::set_default_direction(gtk::TextDirection::Rtl);

  let [a, b] = handles();
  let shown = show(400, -1, || {
    Row::new()
      .set_padding(Inset::new(0., 0., 0., 8.))
      .set_spacing(10)
      .add_child(sized(50, 20, &a))
      .add_child(sized(30, 20, &b))
  });
  let (a_rect, b_rect) = (shown.rect(&a), shown.rect(&b));

  result(
    "right to left mirrors a row, the leading padding at the right",
    a_rect.0 == 342. && b_rect.0 == 302.,
    format!("{a_rect:?} {b_rect:?}"),
  );

  shown.close();
  gtk::Widget::set_default_direction(gtk::TextDirection::Ltr);

  // A scroll is a measured leaf: as tall as its content where it may be, bounded where its
  // sizing says, and it scrolls.
  let [scroll, natural, last] = handles();
  let shown = show(300, 400, || {
    Column::new()
      .add_child(
        Scroll::new(ScrollAxes::Vertical)
          .set_handle(&natural)
          .set_child(Column::new().add_child(sized(50, 80, &NodeHandle::new()))),
      )
      .add_child(
        Scroll::new(ScrollAxes::Vertical)
          .set_height(100)
          .set_handle(&scroll)
          .set_child(
            Column::new()
              .set_spacing(4)
              .add_children((0..19).map(|row| Text::new(format!("Row {row}"))))
              .add_child(Text::new("The last row").set_handle(&last)),
          ),
      )
  });
  let (natural_rect, scroll_rect) = (shown.rect(&natural), shown.rect(&scroll));
  let scrolled = widget(&scroll)
    .downcast::<gtk::ScrolledWindow>()
    .expect("a scroll is a GtkScrolledWindow");
  let adjustment = scrolled.vadjustment();
  let before = adjustment.value();

  shown.hover(scrolled.upcast_ref());
  xdotool(&["click", "--repeat", "5", "5"]);

  let after = adjustment.value();

  result(
    "a scroll takes its content's height where unbounded",
    natural_rect.3 == 80.,
    format!("{natural_rect:?}"),
  );
  result(
    "a scroll of height 100 holds a taller column",
    scroll_rect == (0., 80., 300., 100.) && adjustment.upper() > 300.,
    format!("{scroll_rect:?}, content {}", adjustment.upper()),
  );
  result(
    "the wheel scrolls it (GTK's own scrolling)",
    before == 0. && after > 0.,
    format!("offset {before} then {after} after five wheel steps"),
  );

  shown.close();

  // A scroll with a weight takes the column's height left.
  let [header, flexible] = handles();
  let shown = show(300, 300, || {
    Column::new().add_child(sized(10, 40, &header)).add_child(
      Flexible::new().set_flexibility(1).set_child(
        Scroll::new(ScrollAxes::Vertical)
          .set_handle(&flexible)
          .set_child(Column::new().add_children((0..40).map(|row| Text::new(format!("{row}"))))),
      ),
    )
  });
  let rect = shown.rect(&flexible);

  result(
    "a flexible scroll fills the column's height left",
    rect == (0., 40., 300., 260.),
    format!("{rect:?}"),
  );

  shown.close();
}

fn main() -> std::process::ExitCode {
  run("layout", body)
}
