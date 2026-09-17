//! The rest of the terminal's surface: the title, a shadow, the glyph tokens, and the
//! Simple controls' cell shape.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use skene_simple_tui::{glyph, *};
use skene_tui::{self as tui, CellDecoration, Scene, TokenValue, prelude::*};

use crate::common::{black, colors_at, locate, red, with_platform};

/// The terminal title follows `set_title`, and a derived title follows its signal.
#[test]
fn title_follows_state() {
  with_platform(|| {
    let name = Signal::new("one".to_owned());

    let _ = tui::mount(|| {
      derive_title(move || format!("app: {}", name.get()));

      Text::new("body").into_any().build()
    });

    assert_eq!(tui::current_title().as_deref(), Some("app: one"));

    name.set("two".to_owned());

    assert_eq!(tui::current_title().as_deref(), Some("app: two"));

    set_title("fixed");

    assert_eq!(tui::current_title().as_deref(), Some("fixed"));
  });
}

/// A shadow mixes its color into both colors of the band below and to the right.
#[test]
fn shadow_mixes_into_the_cells_beneath() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_color(red())
        .set_child(
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_shadow(black().with_alpha(0.5)),
        )
        .into_any()
        .build()
    });

    let colors = tui::render_to_colors(4, 3);
    let background = |x: usize, y: usize| colors[y * 4 + x].1.expect("a painted background");
    let clean = background(0, 0);

    assert_eq!(clean, (255, 0, 0));
    assert_eq!(background(1, 1), background(2, 1));
    assert!(background(2, 1).0 < clean.0);
    assert_eq!(background(3, 1), clean);
    assert_eq!(background(1, 2), clean);
  });
}

/// A shadow over unpainted cells mixes against the terminal's own background once
/// the terminal reported it, and against black before; a scrollbar's track takes the
/// terminal's own foreground the same way.
#[test]
fn a_paint_mixes_against_the_terminals_own_colors() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(2)
        .set_height(1)
        .set_shadow(black().with_alpha(0.5))
        .into_any()
        .build()
    });

    let shadow = |x: usize, y: usize| colors_at(4, 3, x, y).1.expect("a shadowed cell");

    assert_eq!(shadow(2, 1), (0, 0, 0), "black over the assumed black");

    tui::trigger_terminal_colors((220, 220, 220), (40, 42, 54));

    assert_eq!(
      shadow(2, 1),
      (20, 21, 27),
      "black over the reported background"
    );
    assert_eq!(colors_at(4, 3, 2, 1).0, Some((110, 110, 110)));
  });
}

/// The glyph tokens are a token set: a theme redefines a control's marks beneath it,
/// and a node under the theme resolves the redefinition.
#[test]
fn a_theme_redefines_the_glyph_tokens() {
  with_platform(|| {
    let node = tui::mount(|| {
      Theme::new(SimpleGlyphs::default().set_checkbox("X", "O"))
        .set_child_builder(|| Text::new("under"))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(8, 1);

    let (checked, unchecked) = Scene::<tui::Tui>::with(|scene| {
      let text = scene.children(node).next().expect("the themed text");

      (
        scene.resolve_token(text, glyph::CHECKBOX_CHECKED.name()),
        scene.resolve_token(text, glyph::CHECKBOX_UNCHECKED.name()),
      )
    });

    assert_eq!(
      checked,
      Some(TokenValue::Text(Definition::from("X".to_owned())))
    );
    assert_eq!(
      unchecked,
      Some(TokenValue::Text(Definition::from("O".to_owned())))
    );
  });
}

/// The Simple controls take the cell shape: a button is one row with one-cell insets,
/// a field is framed, and a checkbox's mark is one glyph-sized cell beside its label.
#[test]
fn simple_controls_take_the_cell_shape() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Theme::new(SimplePalette::default())
        .set_child_builder(|| {
          Column::new().set_children((
            SimpleButton::new("Go").into_any(),
            SimpleTextField::new().set_placeholder("name").into_any(),
            SimpleCheckbox::new().set_label("agree").into_any(),
          ))
        })
        .into_any()
        .build()
    });

    let frame = tui::render_to_text(20, 8);
    let (go_column, go_row) = locate(&frame, "Go");
    let (_, name_row) = locate(&frame, "name");
    let (agree_column, agree_row) = locate(&frame, "agree");

    assert_eq!((go_column, go_row), (1, 0), "a one-row button: {frame}");
    assert!(
      colors_at(20, 8, 0, 0).1.is_some(),
      "the inset cell carries the surface"
    );
    assert_eq!(name_row, 2, "a framed field beneath: {frame}");
    assert!(
      frame
        .lines()
        .nth(1)
        .is_some_and(|line| line.starts_with('╭'))
    );
    assert_eq!(
      (agree_column, agree_row),
      (4, 4),
      "a glyph mark, a gap, the label: {frame}"
    );
    assert!(
      frame
        .lines()
        .nth(4)
        .is_some_and(|line| line.starts_with("[ ] agree")),
      "the mark is the unchecked glyph: {frame}"
    );
  });
}

/// A run's decoration reaches its cells: an underline keeps its style and color, an
/// overline and a line through are their own lines, a link is underlined unless its
/// decoration says otherwise, and plain text carries none.
#[test]
fn a_runs_decoration_paints_its_lines_into_the_cells() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          Text::new("wavy")
            .set_decoration(TextDecoration::Underline)
            .set_decoration_style(TextDecorationStyle::Wavy)
            .set_decoration_color(red()),
          Text::new("over").set_decoration(TextDecoration::Overline),
          Text::new("gone").set_decoration(TextDecoration::LineThrough),
          RichText::new().set_spans([
            Span::new("a "),
            Span::new("link").set_link("https://example.com"),
            Span::new(" bare")
              .set_link("https://example.com")
              .set_decoration(TextDecoration::None),
          ]),
        ))
        .into_any()
        .build()
    });

    let decorations = tui::render_to_decorations(12, 4);
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
  });
}
