//! The rest of the terminal's surface: the title, a shadow, the glyph tokens, and the
//! Simple controls' cell shape.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use skene_tui::{self as tui, Scene, TokenValue, glyph, prelude::*};

use crate::common::{black, colors_at, locate, red, with_driver};

/// The terminal title follows `set_title`, and a derived title follows its signal.
#[test]
fn title_follows_state() {
  with_driver(|| {
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
  with_driver(|| {
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

/// The glyph tokens are a token set: a theme redefines a control's marks beneath it,
/// and a node under the theme resolves the redefinition.
#[test]
fn a_theme_redefines_the_glyph_tokens() {
  with_driver(|| {
    let node = tui::mount(|| {
      Theme::new(Glyphs::default().set_checkbox("X", "O"))
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
  with_driver(|| {
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
      (2, 4),
      "a mark, a gap, the label: {frame}"
    );
    assert!(
      colors_at(20, 8, 0, 4).1.is_some(),
      "the mark fills its cell"
    );
  });
}
