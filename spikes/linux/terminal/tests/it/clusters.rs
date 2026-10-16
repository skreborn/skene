//! Text measured by clusters in cells: a wide cluster takes two, a combining sequence one,
//! the caret and the selection index clusters.

use skene_foundation::seam::content::TextSelection;

use crate::{field, key, keys, value};

#[test]
fn a_field_edits_and_places_its_caret_by_cluster() {
  let mut field = field("日本x", 12);

  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 0, end: 3 },
    "keyboard focus selects the value"
  );

  key(&mut field, "End");
  assert_eq!(field.caret_cell(), Some((5, 0)));

  keys(&mut field, &["ArrowLeft", "ArrowLeft"]);
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 1, end: 1 }
  );
  assert_eq!(field.caret_cell(), Some((2, 0)));

  key(&mut field, "Backspace");
  assert_eq!(value(&field), "本x");

  keys(&mut field, &["End", "e", "\u{301}"]);
  assert_eq!(value(&field), "本xe\u{301}");
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 3, end: 3 }
  );
  assert_eq!(field.caret_cell(), Some((4, 0)));

  key(&mut field, "Backspace");
  assert_eq!(value(&field), "本x", "a removal takes the whole cluster");

  field.press(1, 0, 1, false);
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 1, end: 1 },
    "a wide cluster's second cell places the caret after it"
  );

  key(&mut field, "Shift+ArrowLeft");
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 1, end: 0 }
  );

  field
    .editor_mut()
    .set_selection(TextSelection { start: 0, end: 1 });
  assert_eq!(field.caret_cell(), Some((2, 0)));

  key(&mut field, "y");
  assert_eq!(value(&field), "yx", "typing replaces the placed selection");

  field
    .editor_mut()
    .set_selection(TextSelection { start: 50, end: 50 });
  assert_eq!(
    field.caret_cell(),
    Some((2, 0)),
    "an offset past the end is the end"
  );
}

#[test]
fn a_fields_window_scrolls_by_cells() {
  let mut field = field("日本語", 4);

  key(&mut field, "End");
  assert!(field.render().text().starts_with("語"));
  assert_eq!(field.caret_cell(), Some((2, 0)));

  key(&mut field, "Home");
  assert!(field.render().text().starts_with("日本"));
  assert_eq!(field.caret_cell(), Some((0, 0)));
}

/// Where the terminal has not confirmed grapheme clustering (mode 2027) a cluster is as wide
/// as its code points drawn one by one: the thumbs up with its skin tone takes four cells.
#[test]
fn the_styled_snapshot_shows_wide_cells_and_the_selection() {
  let mut field = field("a日👍🏽b", 8);

  keys(
    &mut field,
    &["Home", "ArrowRight", "Shift+ArrowRight", "Shift+ArrowRight"],
  );

  assert_eq!(
    field.render().to_styled(),
    "|a日👍🏽b|\n|.s>s>>>.|\ncursor 7,0\n"
  );
}
