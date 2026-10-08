//! The scene drawn as pixels: a box's background, border and corners, what a clipping
//! box clips, text in its ink and decoration, and a painted box's picture.

use skene_foundation::{
  Color,
  DecorationBase,
  DecorationWithCornerRadius,
  Dimension,
  HasChild,
  HasChildren,
  Inset,
  Length,
  Overflow,
  Painted,
  PaintedBase,
  RunBase,
  TextDecoration,
  View,
  kurbo,
};
use skene_headless::{
  Headless,
  Image,
  find_by_text,
  layout_rect,
  render_to_image,
  reset,
  text_width,
};
use skene_reactive::Owner;

type Container = skene_foundation::Container<Headless>;
type RichText = skene_foundation::RichText<Headless>;
type Row = skene_foundation::Row<Headless>;
type Span = skene_foundation::Span<Headless>;
type Text = skene_foundation::Text<Headless>;

/// Whether the pixel at (`x`, `y`) of `image` is `color`'s red, green and blue, each
/// within two steps.
fn shows(image: &Image, x: u16, y: u16, color: [u8; 3]) -> bool {
  image.pixel(x, y).is_some_and(|pixel| {
    pixel[..3]
      .iter()
      .zip(color)
      .all(|(drawn, wanted)| drawn.abs_diff(wanted) <= 2)
  })
}

/// Whether any pixel of `image` in the columns `columns` and the rows `rows` is darker
/// than mid-gray.
fn inked(image: &Image, columns: std::ops::Range<u16>, rows: std::ops::Range<u16>) -> bool {
  rows.into_iter().any(|y| {
    columns.clone().any(|x| {
      image
        .pixel(x, y)
        .is_some_and(|pixel| pixel[..3].iter().all(|channel| *channel < 128))
    })
  })
}

/// A box draws its background inside its border, the border around it, and leaves the
/// page showing past its rounded corners.
#[test]
fn a_box_draws_its_background_border_and_corners() {
  reset();

  let owner = Owner::new();
  let _row = owner.enter(|| {
    Row::new()
      .set_children((Container::new()
        .set_width(Dimension::Length(100.))
        .set_height(Dimension::Length(60.))
        .set_background(Color::srgb(1., 0., 0.))
        .set_border_width(Inset::all(4.))
        .set_border_color(Color::srgb(0., 0., 1.))
        .set_corner_radius(Length(12.)),))
      .build()
  });
  let image = render_to_image();

  assert!(shows(&image, 50, 30, [255, 0, 0]), "the background");
  assert!(shows(&image, 1, 30, [0, 0, 255]), "the border");
  assert!(shows(&image, 0, 0, [255, 255, 255]), "past the corner");
  assert!(shows(&image, 150, 30, [255, 255, 255]), "beside the box");

  owner.dispose();
}

/// A box clipping its overflow draws nothing of what it holds past its padding box.
#[test]
fn a_clipping_box_clips_what_it_holds() {
  reset();

  let owner = Owner::new();
  let _row = owner.enter(|| {
    Row::new()
      .set_children((Container::new()
        .set_width(Dimension::Length(50.))
        .set_height(Dimension::Length(50.))
        .set_overflow(Overflow::Clip)
        .set_child(
          Container::new()
            .set_width(Dimension::Length(100.))
            .set_height(Dimension::Length(100.))
            .set_background(Color::srgb(0., 1., 0.)),
        ),))
      .build()
  });
  let image = render_to_image();

  assert!(shows(&image, 25, 25, [0, 255, 0]), "inside the clip");
  assert!(shows(&image, 75, 25, [255, 255, 255]), "past the clip");

  owner.dispose();
}

/// Text draws its glyphs in its ink inside the rectangle it lays out in, and nothing
/// beside it.
#[test]
fn text_draws_its_glyphs_where_it_lays_out() {
  reset();

  let owner = Owner::new();
  let _row = owner.enter(|| Row::new().set_children((Text::new("Hello"),)).build());
  let run = find_by_text("Hello").expect("the run");
  let rect = layout_rect(run).expect("the run's rectangle");
  let image = render_to_image();
  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a test run's rectangle is a small, non-negative number of pixels"
  )]
  let (width, height) = (
    rect.size.width.ceil() as u16,
    rect.size.height.ceil() as u16,
  );

  assert!(inked(&image, 0..width, 0..height), "its glyphs");
  assert!(
    !inked(&image, width + 4..width + 40, 0..height),
    "beside it"
  );

  owner.dispose();
}

/// An underlined span draws its line beneath the blanks between its words too.
#[test]
fn an_underline_runs_beneath_the_blanks() {
  reset();

  let owner = Owner::new();
  let _paragraphs = owner.enter(|| {
    Row::new()
      .set_children((
        RichText::new().add_spans([Span::new("ab cd").set_decoration(TextDecoration::Underline)]),
        RichText::new().add_spans([Span::new("ab cd")]),
      ))
      .build()
  });
  let image = render_to_image();
  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a test span's blank sits a small, non-negative number of pixels in"
  )]
  let (blank, plain) = (
    text_width("ab", 16.).ceil() as u16 + 1,
    (text_width("ab cd", 16.) + text_width("ab", 16.)).ceil() as u16 + 2,
  );

  assert!(
    inked(&image, blank..blank + 2, 0..24),
    "under the underlined blank"
  );
  assert!(
    !inked(&image, plain..plain + 2, 0..24),
    "under the plain blank"
  );

  owner.dispose();
}

/// A painted box draws the picture its painter paints, at its place.
#[test]
fn a_painted_box_draws_its_picture() {
  reset();

  let owner = Owner::new();
  let _row = owner.enter(|| {
    Row::new()
      .set_children((
        Container::new()
          .set_width(Dimension::Length(30.))
          .set_height(Dimension::Length(10.)),
        Painted::<Headless>::new(|canvas, size| {
          canvas.fill(
            &kurbo::Rect::new(0., 0., f64::from(size.width), f64::from(size.height)),
            Color::srgb(0., 1., 0.),
          );
        })
        .set_width(Dimension::Length(40.))
        .set_height(Dimension::Length(40.)),
      ))
      .build()
  });
  let image = render_to_image();

  assert!(shows(&image, 50, 20, [0, 255, 0]), "the picture");
  assert!(shows(&image, 15, 20, [255, 255, 255]), "before the box");

  owner.dispose();
}
