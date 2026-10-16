//! Selection by keys and pointer, the clipboard, and the controlled contract through a field.

use std::{cell::RefCell, rc::Rc};

use skene_foundation::seam::content::{Edit, TextSelection};

use crate::{editor, field, key, keys, value};
use skene_foundation::text::editing::Controlled;
use skene_terminal::field::KeyOutcome;

#[test]
fn keyboard_focus_selects_the_value_and_typing_replaces_it() {
  let mut field = field("old", 20);

  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 0, end: 3 }
  );
  assert_eq!(
    field.render().to_styled(),
    "|old                 |\n|sss.................|\ncursor 3,0\n"
  );

  key(&mut field, "n");
  assert_eq!(value(&field), "n");
}

#[test]
fn shift_extends_the_selection_and_backspace_removes_it() {
  let mut field = field("hello world", 20);

  keys(
    &mut field,
    &[
      "End",
      "Shift+ArrowLeft",
      "Shift+ArrowLeft",
      "Shift+Ctrl+ArrowLeft",
    ],
  );
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 11, end: 6 }
  );

  key(&mut field, "Backspace");
  assert_eq!(value(&field), "hello ");

  keys(&mut field, &["Home", "Shift+End"]);
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 0, end: 6 }
  );

  // A plain arrow over a selection collapses it to the end it points at.
  key(&mut field, "ArrowLeft");
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 0, end: 0 }
  );
}

#[test]
fn the_mouse_selects_by_drag_word_and_value() {
  let mut field = field("alpha beta gamma", 20);

  field.press(1, 0, 1, false);
  field.drag(4, 0);
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 1, end: 4 }
  );

  field.press(7, 0, 2, false);
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 6, end: 10 }
  );

  field.drag(13, 0);
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 6, end: 16 }
  );

  field.press(7, 0, 3, false);
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 0, end: 16 }
  );

  // A shifted press extends from the anchor.
  field.press(2, 0, 1, false);
  field.press(8, 0, 1, true);
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 2, end: 8 }
  );
}

#[test]
fn a_triple_press_in_an_editor_selects_the_line() {
  let mut field = editor("one\ntwo three\nfour", 20, 3);

  field.press(2, 1, 3, false);
  assert_eq!(
    field.editor().selection(),
    TextSelection { start: 4, end: 13 }
  );
}

#[test]
fn the_clipboard_keys_copy_cut_and_paste() {
  let mut field = field("alpha beta", 20);

  keys(&mut field, &["Ctrl+a", "Ctrl+c"]);
  assert_eq!(field.take_copied().as_deref(), Some("alpha beta"));

  keys(&mut field, &["End", "Ctrl+Shift+ArrowLeft", "Ctrl+x"]);
  assert_eq!(field.take_copied().as_deref(), Some("beta"));
  assert_eq!(value(&field), "alpha ");

  assert_eq!(key(&mut field, "Ctrl+v"), KeyOutcome::Paste);
  field.paste("gamma");
  assert_eq!(value(&field), "alpha gamma");

  keys(&mut field, &["Ctrl+a", "Shift+Delete"]);
  assert_eq!(
    field.take_copied().as_deref(),
    Some("alpha gamma"),
    "Shift+Delete cuts"
  );
  assert_eq!(key(&mut field, "Shift+Insert"), KeyOutcome::Paste);
}

#[test]
fn an_editor_pastes_line_breaks_and_breaks_over_a_selection() {
  let mut field = editor("ab", 20, 3);

  keys(&mut field, &["End", "Shift+ArrowLeft", "Enter"]);
  assert_eq!(value(&field), "a\n");

  field.paste("x\r\ny");
  assert_eq!(value(&field), "a\nx\ny");
}

#[test]
fn a_controlled_field_shows_the_truth() {
  let truth = Rc::new(RefCell::new(String::from("12")));
  let mut field = field("12", 20);
  let read = Rc::clone(&truth);
  let write = Rc::clone(&truth);

  // Digits only.
  field.editor_mut().set_arbiter(Controlled::new(
    move || Edit::text(read.borrow().clone()),
    move |edit: Edit| {
      if edit
        .text
        .chars()
        .all(|character| character.is_ascii_digit())
      {
        *write.borrow_mut() = edit.text.to_string();
      }
    },
  ));
  keys(&mut field, &["End", "3", "x", "4"]);
  assert_eq!(value(&field), "1234");
  assert_eq!(*truth.borrow(), "1234");
}
