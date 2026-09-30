//! Pictures in cells: a painted box stands its fallback until the terminal proves its
//! graphics, then renders at its cells times the cell's pixels and is placed over
//! them, cropped to the cells in view and left out under an open overlay.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use skene_tui::{self as tui, Activate, kurbo::Rect, prelude::*, testing};

use crate::common::with_platform;

/// A box filled red, `width` by `height` cells.
fn red_box(width: u32, height: u32) -> Painted {
  Painted::new(|canvas, size| {
    canvas.fill(
      &Rect::new(0., 0., f64::from(size.width), f64::from(size.height)),
      Color::srgb(1., 0., 0.),
    );
  })
  .set_width(width)
  .set_height(height)
}

/// Without graphics the fallback stands and no picture is placed; with them the
/// painted box takes its place, its image the box's cells at ten by twenty pixels
/// each, placed over the box; without them again nothing is placed.
#[test]
fn a_painted_box_falls_back_until_graphics_are_proven() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          red_box(4, 2).set_fallback(|| Text::new("a red box")),
          Text::new("after"),
        ))
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(20, 6);

    assert!(frame.starts_with("a red box\nafter\n"), "{frame}");
    assert!(testing::render_to_pictures(20, 6).is_empty());

    testing::trigger_graphics(true);

    let frame = testing::render_to_text(20, 6);
    let pictures = testing::render_to_pictures(20, 6);

    assert!(frame.starts_with("\n\nafter\n"), "{frame}");
    assert_eq!(pictures.len(), 1);

    let picture = &pictures[0];

    assert_eq!(
      (picture.column, picture.row, picture.columns, picture.rows),
      (0, 0, 4, 2)
    );
    assert_eq!((picture.width, picture.height), (40, 40));
    assert_eq!(picture.crop, (0, 0, 40, 40));
    assert_eq!(picture.pixels.len(), 40 * 40 * 4);
    assert_eq!(&picture.pixels[..4], &[255, 0, 0, 255]);

    testing::trigger_graphics(false);

    assert!(testing::render_to_pictures(20, 6).is_empty());
  });
}

/// A picture scrolled half out of a clipping box is placed over the cells in view,
/// showing the matching part of its image; one under an open popup is left out.
#[test]
fn a_picture_is_cropped_to_the_cells_in_view_and_hidden_under_an_overlay() {
  with_platform(|| {
    testing::trigger_graphics(true);

    let open = Signal::new(false);
    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          Container::new()
            .set_height(2)
            .set_overflow(Overflow::Auto)
            .set_child(Column::new().set_children((Text::new("top"), red_box(4, 3)))),
          RawButton::new(Activate::new())
            .set_label("open")
            .handle_click(move || open.set(true)),
          Overlay::popup()
            .set_position(skene_tui::core::Point::new(0., 0.))
            .derive_open(move || open.get())
            .set_child(Text::new("popup")),
        ))
        .into_any()
        .build()
    });

    let pictures = testing::render_to_pictures(20, 8);

    assert_eq!(pictures.len(), 1);
    assert_eq!(
      (pictures[0].row, pictures[0].rows, pictures[0].crop),
      (1, 1, (0, 0, 40, 20)),
      "the top row of the picture shows below the text"
    );

    testing::trigger_mouse_scroll(1, 1, 0, 2);

    let pictures = testing::render_to_pictures(20, 8);

    assert_eq!(pictures.len(), 1);
    assert_eq!(
      (pictures[0].row, pictures[0].rows, pictures[0].crop),
      (0, 2, (0, 20, 40, 40)),
      "scrolled, the lower rows of the picture show"
    );

    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    let frame = testing::render_to_text(20, 8);

    assert!(frame.contains("popup"), "{frame}");
    assert!(
      testing::render_to_pictures(20, 8).is_empty(),
      "the popup covers the picture's cells"
    );
  });
}
