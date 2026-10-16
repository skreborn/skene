//! Typing, word and line movement, undo and redo, windows and rows.

use skene_foundation::seam::content::Edit;

use skene_terminal::field::KeyOutcome;

use crate::{editor, field, key, keys, value};

#[test]
fn caret_edits_insert_and_delete() {
  let mut field = field("", 20);

  keys(&mut field, &["a", "b", "c", "ArrowLeft", "X"]);
  assert_eq!(value(&field), "abXc");

  keys(&mut field, &["Home", "Delete"]);
  assert_eq!(value(&field), "bXc");

  keys(&mut field, &["End", "Backspace"]);
  assert_eq!(value(&field), "bX");

  // `Ctrl+H` is `0x08`, the legacy byte of `Ctrl+Backspace`: one character deletes.
  key(&mut field, "Ctrl+h");
  assert_eq!(value(&field), "b");
}

#[test]
fn caret_jumps_and_deletes_by_word() {
  let mut field = field("alpha beta gamma", 20);

  keys(&mut field, &["End", "Ctrl+ArrowLeft", "Ctrl+Backspace"]);
  assert_eq!(value(&field), "alpha gamma");

  keys(
    &mut field,
    &["Ctrl+ArrowLeft", "Ctrl+ArrowRight", "Ctrl+Delete"],
  );
  assert_eq!(value(&field), "alpha");

  // `Ctrl+W` deletes the word too, for terminals whose keyboard encoding cannot express
  // `Ctrl+Backspace`.
  key(&mut field, "Ctrl+w");
  assert_eq!(value(&field), "");
}

#[test]
fn word_deletion_follows_unicode_word_boundaries() {
  let mut field = field("say don't foo.bar_baz(", 30);

  key(&mut field, "End");

  for expected in ["say don't foo.bar_baz", "say don't ", "say ", ""] {
    key(&mut field, "Ctrl+Backspace");
    assert_eq!(value(&field), expected);
  }
}

#[test]
fn caret_windows_long_values_and_follows_clicks() {
  let mut field = field("", 5);

  keys(&mut field, &["a", "b", "c", "d", "e", "f", "g", "h"]);
  assert_eq!(field.render().text(), "efgh\n");

  key(&mut field, "Home");
  assert_eq!(field.render().text(), "abcde\n");

  field.press(2, 0, 1, false);
  key(&mut field, "X");
  assert_eq!(value(&field), "abXcdefgh");
}

#[test]
fn an_editor_breaks_lines_and_moves_between_them() {
  let mut field = editor("", 20, 4);

  keys(&mut field, &["a", "b", "Enter", "c", "d"]);
  assert_eq!(value(&field), "ab\ncd");

  let frame = field.render();

  assert_eq!(frame.text(), "ab\ncd\n\n\n");
  assert_eq!(frame.cursor, Some((2, 1)));

  keys(&mut field, &["ArrowUp", "X"]);
  assert_eq!(value(&field), "abX\ncd");
}

#[test]
fn an_editor_windows_its_rows_around_the_caret() {
  let mut field = editor("", 10, 2);

  for key_name in ["1", "Enter", "2", "Enter", "3", "Enter", "4"] {
    key(&mut field, key_name);
  }

  assert_eq!(field.render().text(), "3\n4\n");

  keys(&mut field, &["ArrowUp", "ArrowUp", "ArrowUp"]);
  assert_eq!(field.render().text(), "1\n2\n");
}

#[test]
fn an_editor_wraps_at_its_width_and_keeps_its_column() {
  let mut field = editor("abcdefgh ijklmnop", 10, 3);

  assert_eq!(field.render().text(), "abcdefgh\nijklmnop\n\n");

  keys(
    &mut field,
    &[
      "Ctrl+Home",
      "ArrowRight",
      "ArrowRight",
      "ArrowRight",
      "ArrowDown",
    ],
  );
  assert_eq!(field.render().cursor, Some((3, 1)));

  // `End` goes to the end of the wrapped row, `Ctrl+End` to the value's.
  keys(&mut field, &["Ctrl+Home", "End"]);
  assert_eq!(field.render().cursor, Some((9, 0)));

  key(&mut field, "Ctrl+End");
  assert_eq!(field.render().cursor, Some((8, 1)));
}

#[test]
fn undo_and_redo_step_through_edits() {
  let mut field = field("x", 20);

  key(&mut field, "End");
  key(&mut field, "Ctrl+z");
  assert_eq!(value(&field), "x", "the initial value is no step");

  keys(&mut field, &["Backspace", "a", "b", "c", "ArrowLeft", "d"]);
  assert_eq!(value(&field), "abdc");

  key(&mut field, "Ctrl+Z");
  assert_eq!(
    value(&field),
    "abc",
    "the character after the move undoes alone, Caps Lock or not"
  );

  keys(&mut field, &["Ctrl+y", "Ctrl+z"]);
  assert_eq!(value(&field), "abc");

  key(&mut field, "Ctrl+z");
  assert_eq!(value(&field), "", "the typed run undoes as one");

  key(&mut field, "Ctrl+z");
  assert_eq!(value(&field), "x", "the removal undoes");

  keys(&mut field, &["Ctrl+Shift+z", "Ctrl+Shift+Z"]);
  assert_eq!(value(&field), "abc");

  key(&mut field, "Ctrl+y");
  assert_eq!(value(&field), "abdc");
  assert!(field.render().text().starts_with("abdc"));

  keys(&mut field, &["End", "Backspace", "Backspace"]);
  assert_eq!(value(&field), "ab");

  key(&mut field, "Ctrl+z");
  assert_eq!(value(&field), "abdc", "the removed run undoes as one");

  keys(&mut field, &["x", "Ctrl+y"]);
  assert_eq!(value(&field), "abdcx", "an edit drops the redo steps");

  field.editor_mut().write(&Edit::text("set"));
  assert!(field.render().text().starts_with("set"));

  key(&mut field, "Ctrl+z");
  assert_eq!(
    value(&field),
    "abdcx",
    "a value from outside is a step of its own"
  );

  key(&mut field, "Ctrl+y");
  assert_eq!(value(&field), "set");
}

#[test]
fn line_breaks_arrive_as_line_feeds_in_an_editor_and_drop_in_a_field() {
  let mut bio = editor("", 20, 4);

  bio.paste("a\r\nb\rc");
  assert_eq!(value(&bio), "a\nb\nc");
  assert_eq!(bio.render().text(), "a\nb\nc\n\n");

  let mut name = field("", 20);

  name.paste("4\r\n5");
  assert_eq!(value(&name), "45");
}

#[test]
fn a_read_only_entry_takes_no_edits() {
  let mut field = field("fixed", 20);

  field.editor_mut().set_read_only(true);
  keys(&mut field, &["End", "x", "Backspace", "Ctrl+x"]);
  assert_eq!(value(&field), "fixed");

  keys(&mut field, &["Ctrl+a", "Ctrl+c"]);
  assert_eq!(
    field.take_copied().as_deref(),
    Some("fixed"),
    "it still copies"
  );
}

#[test]
fn a_tab_in_an_entry_is_one_blank_cell() {
  let mut name = field("a\tb", 20);

  assert_eq!(name.render().text(), "a b\n");

  key(&mut name, "End");
  assert_eq!(name.render().cursor, Some((3, 0)));

  let mut bio = editor("a\tb\nc", 20, 2);

  assert_eq!(bio.render().text(), "a b\nc\n");
}

#[test]
fn a_placeholder_shows_while_the_value_is_empty() {
  let mut field = field("", 12);

  field.set_placeholder("Your name");

  let frame = field.render();

  assert_eq!(frame.text(), "Your name\n");
  assert!(frame.cell(0, 0).style.placeholder);

  key(&mut field, "J");
  assert_eq!(field.render().text(), "J\n");
}

#[test]
fn the_entry_behavior_decides_enter_read_only_and_secure() {
  let mut field = field("ab", 20);

  assert_eq!(key(&mut field, "Enter"), KeyOutcome::Activate);
  assert_eq!(value(&field), "ab", "Enter submits a field of one row");

  let mut editor = editor("ab", 20, 3);

  assert_eq!(key(&mut editor, "Enter"), KeyOutcome::Handled);

  field.set_read_only(true);
  keys(&mut field, &["End", "x", "Backspace", "Ctrl+w"]);
  assert_eq!(value(&field), "ab", "a read-only field keeps its text");

  keys(&mut field, &["Ctrl+a", "Ctrl+c"]);
  assert_eq!(field.take_copied().as_deref(), Some("ab"), "and its copy");

  field.set_read_only(false);
  field.set_secure(true);
  keys(&mut field, &["Ctrl+a", "Ctrl+c"]);
  assert_eq!(field.take_copied(), None, "a secure field offers no copy");
  assert_eq!(key(&mut field, "Ctrl+x"), KeyOutcome::Handled);
  assert_eq!(value(&field), "ab", "nor cut");
}
