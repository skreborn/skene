//! Painting on the real platform: what a later box draws covers a text drawn before it, the
//! platform's own scroll bar included, and a box's border lies under the children it holds,
//! which a box that clips cuts inside its border, as CSS paints them.

use skene_gui::{
  prelude::*,
  testing::{self, Role},
};

/// Thirty lines of red capitals, each wider than the scroll they scroll in both ways, 300
/// pixels wide and 200 tall.
fn scrolled() -> impl View {
  let mut lines = Column::new();

  for _ in 0..30 {
    lines = lines.add_child(
      Text::new("MMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMM")
        .set_font_size(20)
        .set_color(Color::rgb(0xff0000)),
    );
  }

  Column::new().set_padding(16).add_child(
    Container::new()
      .set_label("Scroller")
      .set_width(300)
      .set_child(
        Scroll::new(ScrollAxes::Both)
          .set_width(300)
          .set_height(200)
          .set_child(lines),
      ),
  )
}

#[skene_gui::test]
fn the_scroll_bar_draws_over_the_text_it_scrolls() {
  let screen = testing::render(scrolled);
  let bounds = screen.find_by_role(Role::Region, "Scroller").read().bounds;
  let shot = screen.render_to_image();
  // The bar runs down the scroll's trailing edge, inset from it, at least 24 pixels long.
  let column = (bounds.x + bounds.width - 5.).trunc();
  let (top, bottom) = ((bounds.y + 4.) as usize, (bounds.y + 22.) as usize);
  let bar: Vec<[u8; 3]> = (top..bottom)
    .map(|row| {
      let [red, green, blue, _] = shot.rgba_at(column, row as f32);

      [red, green, blue]
    })
    .collect();
  let pure_red = bar
    .iter()
    .filter(|[red, green, blue]| *red > 0xf0 && *green < 0x10 && *blue < 0x10)
    .count();

  assert_eq!(
    pure_red,
    0,
    "no glyph shows through the bar: {pure_red} of {} pixels are the text's own red: {bar:?}",
    bar.len()
  );
  assert!(
    bar
      .iter()
      .any(|[red, green, _]| (0x90..=0xe8).contains(red) && *green < 0x30),
    "the text lies beneath the bar, darkened by it: {bar:?}"
  );
}

/// A frame 100 by 40 with a 6-pixel red border holding a blue child 160 wide, clipping or not.
fn bordered(clip: bool) -> impl View {
  Column::new().set_padding(16).add_child(
    Row::new().add_child(
      Container::new()
        .set_label("Frame")
        .set_width(100)
        .set_height(40)
        .set_border(Border::new(6))
        .set_border_color(Color::rgb(0xff0000))
        .set_clip(clip)
        .set_child(
          Container::new()
            .set_width(160)
            .set_height(28)
            .set_background(Color::rgb(0x0000ff)),
        ),
    ),
  )
}

#[skene_gui::test]
fn a_child_that_overflows_its_box_draws_over_the_boxs_border() {
  let screen = testing::render(|| bordered(false));
  let frame = screen.find_by_role(Role::Region, "Frame").read().bounds;
  let y = frame.y + frame.height / 2.;

  assert_eq!(
    screen.pixel(frame.x + frame.width - 3., y),
    Color::rgb(0x0000ff),
    "the child is drawn after the border it overflows, as CSS draws it"
  );
  assert_eq!(
    screen.pixel(frame.x + frame.width + 20., y),
    Color::rgb(0x0000ff)
  );
}

#[skene_gui::test]
fn a_box_that_clips_cuts_its_child_inside_its_border() {
  let screen = testing::render(|| bordered(true));
  let frame = screen.find_by_role(Role::Region, "Frame").read().bounds;
  let y = frame.y + frame.height / 2.;

  assert_eq!(
    screen.pixel(frame.x + frame.width - 3., y),
    Color::rgb(0xff0000),
    "the border shows: the child is cut at the box inside it"
  );
  assert_eq!(
    screen.pixel(frame.x + frame.width - 9., y),
    Color::rgb(0x0000ff),
    "the child shows inside the border"
  );
  assert_ne!(
    screen.pixel(frame.x + frame.width + 20., y),
    Color::rgb(0x0000ff),
    "nothing of the child shows past the box"
  );
}
