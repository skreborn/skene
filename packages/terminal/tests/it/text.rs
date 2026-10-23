//! The runs' inks in cells: a run inherits through the typography tokens, a group
//! redefines them for its own spans, a group's media variant reaches them, and a
//! derived span rewrites its run beside a view it embeds.

use skene_terminal::{self as terminal, Color, prelude::*, testing};

use crate::common::{blue, colors_at, green, red, rgb, white, with_platform};

/// The text color of the cell (`x`, `y`) in a `width` x `height` frame.
fn ink_at(width: u16, height: u16, x: usize, y: usize) -> Option<(u8, u8, u8)> {
  colors_at(width, height, x, y).0
}

/// A text with no color of its own paints in the typography's ink, a link in its link
/// ink: a theme's under the theme, and the defaults under none—the terminal's own
/// foreground, and the blue that answers a dark background.
#[test]
fn a_text_without_a_color_paints_the_typography_ink() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Theme::new(Typography::default().set_ink(red()).set_link_ink(green())).set_child_builder(
            || {
              Column::new().set_children((
                Text::new("themed"),
                Text::new("link").set_link("https://example.com"),
              ))
            },
          ),
          Text::new("bare"),
          Text::new("link").set_link("https://example.com"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(8, 4), "themed\nlink\nbare\nlink\n");
    assert_eq!(ink_at(8, 4, 0, 0), Some(rgb(red())), "the theme's ink");
    assert_eq!(
      ink_at(8, 4, 0, 1),
      Some(rgb(green())),
      "the theme's link ink"
    );
    assert_eq!(ink_at(8, 4, 0, 2), None, "the terminal's own foreground");
    assert_eq!(
      ink_at(8, 4, 0, 3),
      Some(rgb(Color::srgb(0.4, 0.6, 1.))),
      "the default link ink"
    );
  });
}

/// A group's color redefines the ink for the spans beneath it and no other: a sibling
/// keeps the theme's ink, a nested group redefines again, a link inside keeps the link
/// ink, and a derived group color follows its signal.
#[test]
fn a_groups_color_redefines_the_ink_for_its_spans_alone() {
  with_platform(|| {
    let yellow = Color::srgb(1., 1., 0.);
    let accent = Signal::new(red());
    let _ = terminal::mount(move || {
      Theme::new(Typography::default().set_ink(white()).set_link_ink(blue()))
        .set_child_builder(move || {
          RichText::new().add_spans([
            Span::new("a"),
            Span::group([
              Span::new("b"),
              Span::group([Span::new("c")]).set_color(green()),
              Span::new("d").set_link("/d"),
            ])
            .derive_color(move || accent.get()),
            Span::new("e"),
          ])
        })
        .into_any()
        .build()
    });
    let inks = || (0..5).map(|x| ink_at(6, 1, x, 0)).collect::<Vec<_>>();

    assert_eq!(testing::render_to_text(6, 1), "abcde\n");
    assert_eq!(
      inks(),
      [white(), red(), green(), blue(), white()].map(|ink| Some(rgb(ink)))
    );

    accent.set(yellow);

    assert_eq!(
      inks(),
      [white(), yellow, green(), blue(), white()].map(|ink| Some(rgb(ink)))
    );
  });
}

/// A group's compound media variant reaches the spans beneath it, as a single media
/// variant does: the light scheme's color only once the terminal reports a light
/// background.
#[test]
fn a_groups_compound_media_variant_reaches_its_spans() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      RichText::new()
        .add_spans([Span::group([Span::new("x")]).set_color(red().when(
          Condition::and([Media::min_width(1).into(), Media::Light.into()]),
          green(),
        ))])
        .into_any()
        .build()
    });

    assert_eq!(ink_at(2, 1, 0, 0), Some(rgb(red())), "dark by default");

    testing::trigger_color_scheme(ColorScheme::Light);

    assert_eq!(ink_at(2, 1, 0, 0), Some(rgb(green())));
  });
}

/// A derived span rewrites its own run's text in place, beside a view the paragraph
/// embeds, which keeps its cells after it.
#[test]
fn a_derived_span_rewrites_its_run_beside_an_embedded_view() {
  with_platform(|| {
    let label = Signal::new("one".to_owned());
    let _ = terminal::mount(move || {
      RichText::new()
        .add_spans([
          Span::derive(move || label.get()),
          Span::view(
            Container::new()
              .set_background(red())
              .set_child(Text::new("boxed")),
          ),
        ])
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(10, 1), "oneboxed\n");
    assert_eq!(colors_at(10, 1, 3, 0).1, Some(rgb(red())), "the view's box");

    label.set("three".to_owned());

    assert_eq!(testing::render_to_text(10, 1), "threeboxed\n");
    assert_eq!(colors_at(10, 1, 4, 0).1, None, "the run, unboxed");
    assert_eq!(
      colors_at(10, 1, 5, 0).1,
      Some(rgb(red())),
      "the view, after it"
    );
  });
}
