//! Text measured in clusters: a combining sequence takes one cell and a wide
//! character two—in layout, paint, wrapping, a field's caret and window, and the
//! selection an app binds—and the terminal's grapheme clustering (mode 2027) sets
//! an emoji sequence's width.

use skene_terminal::{
  self as terminal,
  CellDecoration,
  HasChild,
  TextSelection,
  behavior::TextEntry,
  prelude::*,
  testing,
};

use crate::common::with_platform;

/// A family emoji: three emoji joined by zero width joiners, one cluster.
const FAMILY: &str = "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}";

/// `text` followed by an underlined pipe, so the cell after the text can be found.
fn marked(text: &str) -> Row {
  Row::new().set_children((
    Text::new(text),
    Text::new("|").set_decoration(TextDecoration::Underline),
  ))
}

/// The first underlined column of `row` in a frame `width` cells wide.
fn underlined(decorations: &[CellDecoration], width: usize, row: usize) -> Option<usize> {
  (0..width).find(|column| decorations[row * width + column].underline.is_some())
}

/// A combining sequence is one cell, a wide character two, and an emoji sequence as
/// many as its parts until the terminal confirms clustering; the cell after each is
/// laid out and painted accordingly, and a wide cluster wraps as one.
#[test]
fn text_is_measured_by_cells_not_characters() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          marked("e\u{301}x"),
          marked("日本"),
          marked(FAMILY),
          Container::new()
            .set_width(4)
            .set_child(Text::new("日本 本語")),
        ))
        .into_any()
        .build()
    });

    let decorations = testing::render_to_decorations(12, 5);

    assert_eq!(underlined(&decorations, 12, 0), Some(2));
    assert_eq!(underlined(&decorations, 12, 1), Some(4));
    assert_eq!(underlined(&decorations, 12, 2), Some(6));
    assert!(
      testing::render_to_text(12, 5).starts_with("e\u{301}x|\n日本|\n"),
      "{}",
      testing::render_to_text(12, 5)
    );
    assert!(
      testing::render_to_text(12, 5).ends_with("日本\n本語\n"),
      "{:?}",
      testing::render_to_text(12, 5)
    );

    testing::trigger_grapheme_clustering(true);

    let decorations = testing::render_to_decorations(12, 5);

    assert_eq!(underlined(&decorations, 12, 2), Some(2));

    testing::trigger_grapheme_clustering(false);

    let decorations = testing::render_to_decorations(12, 5);

    assert_eq!(underlined(&decorations, 12, 2), Some(6));
  });
}

/// A field's caret, selection and edits index clusters: the caret sits after a wide
/// cluster's cells, the arrows step over one, `Backspace` removes a wide one whole and a
/// combining mark alone, a mark typed after its base joins it, a click on a wide cluster's second
/// cell lands after it, and the selection an app binds reads and writes cluster indices, clamped
/// into the value.
#[test]
fn a_field_edits_and_places_its_caret_by_cluster() {
  with_platform(|| {
    let value = Signal::new("日本x".to_owned());
    let selection = Signal::new(TextSelection::default());
    let _ = terminal::mount(move || {
      BasicTextField::new(TextEntry::new().bind_value(value).bind_selection(selection))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 1);

    testing::trigger_key("Tab");

    assert_eq!(
      selection.get_untracked(),
      TextSelection::new(0, 3),
      "keyboard focus selects the value"
    );

    testing::trigger_key("End");

    assert_eq!(testing::render_to_caret(12, 1), Some((5, 0)));

    testing::trigger_key("ArrowLeft");
    testing::trigger_key("ArrowLeft");

    assert_eq!(selection.get_untracked(), TextSelection::collapsed(1));
    assert_eq!(testing::render_to_caret(12, 1), Some((2, 0)));

    testing::trigger_key("Backspace");

    assert_eq!(value.get_untracked(), "本x");

    testing::trigger_key("End");
    testing::trigger_key("e");
    testing::trigger_key("\u{301}");

    assert_eq!(value.get_untracked(), "本xe\u{301}");
    assert_eq!(selection.get_untracked(), TextSelection::collapsed(3));
    assert_eq!(testing::render_to_caret(12, 1), Some((4, 0)));

    testing::trigger_key("Backspace");

    assert_eq!(
      value.get_untracked(),
      "本xe",
      "a removal backward takes the mark alone, as Chromium's does"
    );

    testing::trigger_key("Backspace");

    assert_eq!(value.get_untracked(), "本x");

    testing::trigger_mouse_click(1, 0);

    assert_eq!(
      selection.get_untracked(),
      TextSelection::collapsed(1),
      "a wide cluster's second cell places the caret after it"
    );

    testing::trigger_key("Shift+ArrowLeft");

    assert_eq!(selection.get_untracked(), TextSelection::new(1, 0));

    selection.set(TextSelection::new(0, 1));

    assert_eq!(testing::render_to_caret(12, 1), Some((2, 0)));

    testing::trigger_key("y");

    assert_eq!(
      value.get_untracked(),
      "yx",
      "typing replaces the placed selection"
    );
    assert_eq!(selection.get_untracked(), TextSelection::collapsed(1));

    selection.set(TextSelection::collapsed(50));

    assert_eq!(
      testing::render_to_caret(12, 1),
      Some((2, 0)),
      "an offset past the end is the end"
    );
  });
}

/// A field's window scrolls by cells: at four cells over three wide clusters, the
/// caret at the end shows the last cluster alone, and `Home` brings the first back.
#[test]
fn a_fields_window_scrolls_by_cells() {
  with_platform(|| {
    let value = Signal::new("日本語".to_owned());
    let _ = terminal::mount(move || {
      BasicTextField::new(TextEntry::new().bind_value(value))
        .set_width(4)
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(8, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("End");

    assert!(testing::render_to_text(8, 1).starts_with("語"));
    assert_eq!(testing::render_to_caret(8, 1), Some((2, 0)));

    testing::trigger_key("Home");

    assert!(testing::render_to_text(8, 1).starts_with("日本"));
    assert_eq!(testing::render_to_caret(8, 1), Some((0, 0)));
  });
}

/// The document selection indexes clusters: a drag over wide characters copies whole
/// ones, from the cluster under the press to the one under the release.
#[test]
fn the_document_selection_takes_whole_clusters() {
  with_platform(|| {
    let _ = terminal::mount(|| Text::new("日本語").set_selectable(true).build());
    let _ = testing::render_to_text(12, 4);

    testing::trigger_mouse_press(0, 0);
    testing::trigger_mouse_drag(5, 0);
    testing::trigger_mouse_release(5, 0);
    testing::trigger_mouse_right_click(1, 0);

    let _ = testing::render_to_text(12, 4);

    testing::trigger_mouse_click(3, 1);

    assert_eq!(testing::take_copied().as_deref(), Some("日本語"));

    testing::trigger_mouse_press(2, 0);
    testing::trigger_mouse_drag(3, 0);
    testing::trigger_mouse_release(3, 0);
    testing::trigger_mouse_right_click(2, 0);

    let _ = testing::render_to_text(12, 4);

    testing::trigger_mouse_click(4, 1);

    assert_eq!(testing::take_copied().as_deref(), Some("本"));
  });
}

/// Text carrying control characters paints each as one visible cell, so none reaches
/// the terminal and the layout agrees with the paint, with clustering or without.
#[test]
fn a_control_character_paints_as_one_visible_cell() {
  with_platform(|| {
    let _ = terminal::mount(|| marked("a\u{1b}]2;x\u{7}b\u{7f}c").build());

    for clustering in [false, true] {
      testing::trigger_grapheme_clustering(clustering);

      let text = testing::render_to_text(16, 1);

      assert_eq!(
        text, "a\u{fffd}]2;x\u{fffd}b\u{fffd}c|\n",
        "clustering {clustering}"
      );
      assert_eq!(
        underlined(&testing::render_to_decorations(16, 1), 16, 0),
        Some(10)
      );
    }
  });
}

/// A tab in text is one blank cell.
#[test]
fn a_tab_is_one_blank_cell() {
  with_platform(|| {
    let _ = terminal::mount(|| marked("ab\tc\td").build());

    let text = testing::render_to_text(24, 1);

    assert_eq!(text, "ab c d|\n");
    assert_eq!(
      underlined(&testing::render_to_decorations(24, 1), 24, 0),
      Some(6)
    );
  });
}
