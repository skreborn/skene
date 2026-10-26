//! Decoration in cells: a shadow mixing into the cells beneath and moving by its offset,
//! a box's scrollbars in its own glyphs and colors, a paint mixing against the
//! terminal's own colors, and a run's decoration lines.

use skene_terminal::{CellDecoration, platform::terminal::mount, prelude::*, testing};

use crate::common::{black, blue, colors_at, red, rgb, with_platform};

/// A shadow mixes its color into both colors of the band below and to the right.
#[test]
fn shadow_mixes_into_the_cells_beneath() {
  with_platform(|| {
    let _ = mount(|| {
      Container::new()
        .set_background(red())
        .set_child(
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_shadow(black().with_alpha(0.5)),
        )
        .into_any()
        .build()
    });

    let colors = testing::render_to_colors(4, 3);
    let background = |x: usize, y: usize| colors[y * 4 + x].1.expect("a painted background");
    let clean = background(0, 0);

    assert_eq!(clean, (255, 0, 0));
    assert_eq!(background(1, 1), background(2, 1));
    assert!(background(2, 1).0 < clean.0);
    assert_eq!(background(3, 1), clean);
    assert_eq!(background(1, 2), clean);
  });
}

/// A shadow's offset moves it down and right by the cells it reaches, rounded up.
#[test]
fn a_shadows_offset_moves_it() {
  with_platform(|| {
    let _ = mount(|| {
      Container::new()
        .set_background(red())
        .set_child(
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_shadow(black().with_alpha(0.5))
            .set_shadow_offset(1.5),
        )
        .into_any()
        .build()
    });

    let colors = testing::render_to_colors(5, 4);
    let background = |x: usize, y: usize| colors[y * 5 + x].1.expect("a painted background");
    let clean = background(0, 0);

    assert_eq!(background(1, 1), clean, "the shadow starts two cells in");
    assert!(background(2, 2).0 < clean.0);
    assert!(background(3, 2).0 < clean.0);
    assert_eq!(background(3, 1), clean);
    assert_eq!(background(4, 2), clean);
    assert_eq!(background(2, 3), clean);
  });
}

/// A box's scrollbars draw with its own glyphs and colors: a line thumb moving by cells
/// over a line track, each in the color set for it.
#[test]
fn a_boxs_scrollbars_draw_with_its_glyphs_and_colors() {
  with_platform(|| {
    let _ = mount(|| {
      Container::new()
        .set_height(4)
        .set_overflow(Overflow::ScrollIfNeeded)
        .set_scrollbar_glyphs(ScrollbarGlyphs::LINE)
        .set_scrollbar_thumb(red())
        .set_scrollbar_track(blue())
        .set_child(
          Column::new().set_children(
            (0..8)
              .map(|index| Text::new(format!("{index}")).into_any())
              .collect::<Vec<_>>(),
          ),
        )
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(6, 4);
    let column: Vec<char> = frame
      .lines()
      .map(|line| line.chars().nth(5).unwrap_or(' '))
      .collect();

    assert_eq!(column, ['┃', '┃', '│', '│'], "{frame}");
    assert_eq!(colors_at(6, 4, 5, 0).0, Some(rgb(red())));
    assert_eq!(colors_at(6, 4, 5, 3).0, Some(rgb(blue())));
  });
}

/// A shadow over unpainted cells mixes against the terminal's own background once
/// the terminal reported it, and against black before; a scrollbar's track takes the
/// terminal's own foreground the same way.
#[test]
fn a_paint_mixes_against_the_terminals_own_colors() {
  with_platform(|| {
    let _ = mount(|| {
      Container::new()
        .set_width(2)
        .set_height(1)
        .set_shadow(black().with_alpha(0.5))
        .into_any()
        .build()
    });

    let shadow = |x: usize, y: usize| colors_at(4, 3, x, y).1.expect("a shadowed cell");

    assert_eq!(shadow(2, 1), (0, 0, 0), "black over the assumed black");

    testing::trigger_terminal_colors((220, 220, 220), (40, 42, 54));

    assert_eq!(
      shadow(2, 1),
      (20, 21, 27),
      "black over the reported background"
    );
    assert_eq!(colors_at(4, 3, 2, 1).0, Some((110, 110, 110)));
  });
}

/// A run's decoration reaches its cells: an underline keeps its style and color and
/// runs beneath the blanks between its words, an overline and a line through are their
/// own lines, a link is underlined unless its decoration says otherwise, and plain text
/// carries none.
#[test]
fn a_runs_decoration_paints_its_lines_into_the_cells() {
  with_platform(|| {
    let _ = mount(|| {
      Column::new()
        .set_children((
          Text::new("wavy")
            .set_decoration(TextDecoration::Underline)
            .set_decoration_style(TextDecorationStyle::Wavy)
            .set_decoration_color(red()),
          Text::new("over").set_decoration(TextDecoration::Overline),
          Text::new("gone").set_decoration(TextDecoration::LineThrough),
          RichText::new().add_spans([
            Span::new("a "),
            Span::new("link").set_link("https://example.com"),
            Span::new(" bare")
              .set_link("https://example.com")
              .set_decoration(TextDecoration::None),
          ]),
          Text::new("ab cd").set_decoration(TextDecoration::Underline),
        ))
        .into_any()
        .build()
    });

    let decorations = testing::render_to_decorations(12, 5);
    let at = |column: usize, row: usize| decorations[row * 12 + column];

    assert_eq!(
      at(0, 0),
      CellDecoration {
        underline: Some(TextDecorationStyle::Wavy),
        underline_color: Some((255, 0, 0)),
        ..CellDecoration::NONE
      }
    );
    assert_eq!(
      at(4, 0),
      CellDecoration::NONE,
      "the line ends with the text"
    );
    assert_eq!(
      at(0, 1),
      CellDecoration {
        overline: true,
        ..CellDecoration::NONE
      }
    );
    assert_eq!(
      at(0, 2),
      CellDecoration {
        line_through: true,
        ..CellDecoration::NONE
      }
    );
    assert_eq!(at(0, 3), CellDecoration::NONE, "plain text carries no line");
    assert_eq!(
      at(2, 3),
      CellDecoration {
        underline: Some(TextDecorationStyle::Solid),
        ..CellDecoration::NONE
      },
      "a link underlines by default"
    );
    assert_eq!(
      at(7, 3),
      CellDecoration::NONE,
      "a link's own decoration wins"
    );
    assert_eq!(
      at(2, 4),
      CellDecoration {
        underline: Some(TextDecorationStyle::Solid),
        ..CellDecoration::NONE
      },
      "the blank between the words"
    );
    assert_eq!(at(1, 3), CellDecoration::NONE, "a plain span's blank");
  });
}
