//! Decoration in pixels: where the terminal shows graphics, a box with a corner
//! radius, a border or a shadow paints them as an image beneath its cells, which stay
//! uncolored and unbordered so it shows through, while a plain box and every glyph
//! stay cells.

use skene_tui::{self as tui, HasChild, prelude::*, testing};

use crate::common::{colors_at, red, with_platform};

/// A red card with a rounded border and a shadow, holding a plain blue box and text.
fn card() -> AnyView {
  Container::new()
    .set_width(8)
    .set_height(4)
    .set_color(red())
    .set_border_width(Inset::all(1))
    .set_border_color(Color::srgb(0., 1., 0.))
    .set_corner_radius(1.)
    .set_shadow(Color::srgb(0., 0., 0.).with_alpha(0.5))
    .set_child(
      Column::new().set_children((
        Text::new("hi"),
        Container::new()
          .set_width(2)
          .set_height(1)
          .set_color(Color::srgb(0., 0., 1.)),
      )),
    )
    .into_any()
}

/// Without graphics the card is glyphs and cell colors; with them its cells carry no
/// color and no border glyph, one backdrop beneath every colored cell covers the box
/// and its shadow's band at the cell's pixels, the corner pixel is clear and the
/// middle is the box's color, while the plain box inside and the text stay cells.
#[test]
fn a_decorated_box_becomes_a_backdrop_where_graphics_show() {
  with_platform(|| {
    let _ = tui::mount(|| card().build());

    let frame = testing::render_to_text(12, 6);

    assert!(frame.starts_with("╭──────╮"), "{frame}");
    assert_eq!(colors_at(12, 6, 1, 1).1, Some((255, 0, 0)));
    assert!(testing::render_to_pictures(12, 6).is_empty());

    testing::trigger_graphics(true);

    let frame = testing::render_to_text(12, 6);
    let pictures = testing::render_to_pictures(12, 6);

    assert!(
      frame.starts_with("\n hi\n"),
      "no glyph border remains: {frame}"
    );
    assert_eq!(
      colors_at(12, 6, 1, 1).1,
      None,
      "the card's cells stay uncolored"
    );
    assert_eq!(
      colors_at(12, 6, 1, 2).1,
      Some((0, 0, 255)),
      "the plain box paints cells"
    );
    assert_eq!(pictures.len(), 1);

    let backdrop = &pictures[0];

    assert!(backdrop.layer < 0);
    assert_eq!(
      (
        backdrop.column,
        backdrop.row,
        backdrop.columns,
        backdrop.rows
      ),
      (0, 0, 9, 5),
      "the box and its shadow's band"
    );
    assert_eq!((backdrop.width, backdrop.height), (90, 100));

    let pixel = |x: usize, y: usize| &backdrop.pixels[(y * 90 + x) * 4..(y * 90 + x) * 4 + 4];

    assert_eq!(pixel(0, 0)[3], 0, "the corner is rounded away");
    assert_eq!(
      pixel(40, 40),
      &[255, 0, 0, 255],
      "the middle is the box's color"
    );
    assert_eq!(
      &pixel(40, 1)[..3],
      &[0, 255, 0],
      "the border line at the edge"
    );
    assert!(
      pixel(40, 90)[3] > 0 && pixel(40, 90)[3] < 255,
      "the shadow softens"
    );
  });
}
