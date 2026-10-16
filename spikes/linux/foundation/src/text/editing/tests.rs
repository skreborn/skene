//! The editing core over the canned engine: keys, selection by grapheme cluster, undo, the
//! controlled contract, composition and the input-client surface. Bidirectional movement is
//! held by `skene-parley`'s tests, over a real shaper.

use std::{cell::RefCell, rc::Rc};

use crate::{
  Signal,
  seam::{
    content::{Edit, TextSelection, Unit},
    report::{KeyEvent, Modifiers},
  },
  text::{
    ParagraphStyle, RunStyle,
    editing::{Binding, Clause, ClauseStyle, Command, Controlled, Editor, Notice, Origin},
    grid::Grid,
  },
};

fn editor(text: &str) -> Editor<Grid> {
  let mut editor = Editor::new(Grid, RunStyle::new(13.), ParagraphStyle::default());

  editor.write(&Edit::text(text));

  editor
}

/// A key press named as the old terminal tests name it: `"Ctrl+Shift+z"`.
fn key<E: crate::text::TextEngine>(editor: &mut Editor<E>, name: &str) -> bool {
  let mut modifiers = Modifiers::default();
  let mut parts: Vec<&str> = name.split('+').collect();
  let key = parts
    .pop()
    .map_or("+", |key| if key.is_empty() { "+" } else { key });

  for part in parts {
    match part {
      "Ctrl" => modifiers.control = true,
      "Shift" => modifiers.shift = true,
      "Alt" => modifiers.alt = true,
      _ => panic!("unknown modifier {part}"),
    }
  }

  let event = KeyEvent {
    key: key.to_owned(),
    code: String::new(),
    down: true,
    repeat: false,
    modifiers,
  };

  match Binding::for_key(&event, false) {
    Some(Binding::Command(command)) => editor.perform(command).handled,
    Some(Binding::Paste) => editor.perform(Command::Paste("pasted".into())).handled,
    None => false,
  }
}

fn keys(editor: &mut Editor<Grid>, names: &[&str]) {
  for name in names {
    key(editor, name);
  }
}

const fn selection(start: usize, end: usize) -> TextSelection {
  TextSelection { start, end }
}

#[test]
fn caret_edits_insert_and_delete() {
  let mut editor = editor("");

  keys(&mut editor, &["a", "b", "c", "ArrowLeft", "X"]);
  assert_eq!(editor.text(), "abXc");

  keys(&mut editor, &["Home", "Delete"]);
  assert_eq!(editor.text(), "bXc");

  keys(&mut editor, &["End", "Backspace"]);
  assert_eq!(editor.text(), "bX");
}

#[test]
fn caret_jumps_and_deletes_by_word() {
  let mut editor = editor("alpha beta gamma");

  keys(&mut editor, &["End", "Ctrl+ArrowLeft", "Ctrl+Backspace"]);
  assert_eq!(editor.text(), "alpha gamma");

  keys(
    &mut editor,
    &["Ctrl+ArrowLeft", "Ctrl+ArrowRight", "Ctrl+Delete"],
  );
  assert_eq!(editor.text(), "alpha");
}

#[test]
fn word_deletion_follows_unicode_word_boundaries() {
  let mut editor = editor("say don't foo.bar_baz(");

  key(&mut editor, "End");

  for expected in ["say don't foo.bar_baz", "say don't ", "say ", ""] {
    key(&mut editor, "Ctrl+Backspace");
    assert_eq!(editor.text(), expected);
  }
}

#[test]
fn undo_and_redo_step_through_edits() {
  let mut editor = editor("x");

  key(&mut editor, "End");
  key(&mut editor, "Ctrl+z");
  assert_eq!(editor.text(), "x", "the initial value is no step");

  keys(&mut editor, &["Backspace", "a", "b", "c", "ArrowLeft", "d"]);
  assert_eq!(editor.text(), "abdc");

  key(&mut editor, "Ctrl+Z");
  assert_eq!(
    editor.text(),
    "abc",
    "the character after the move undoes alone"
  );

  key(&mut editor, "Ctrl+z");
  assert_eq!(editor.text(), "", "the typed run undoes as one");

  key(&mut editor, "Ctrl+z");
  assert_eq!(editor.text(), "x", "the removal undoes");

  key(&mut editor, "Ctrl+z");
  assert_eq!(editor.text(), "x", "nothing further to undo");

  keys(&mut editor, &["Ctrl+Shift+z", "Ctrl+Shift+Z"]);
  assert_eq!(editor.text(), "abc");

  key(&mut editor, "Ctrl+y");
  assert_eq!(editor.text(), "abdc");

  keys(&mut editor, &["End", "Backspace", "Backspace"]);
  assert_eq!(editor.text(), "ab");

  key(&mut editor, "Ctrl+z");
  assert_eq!(editor.text(), "abdc", "the removed run undoes as one");

  key(&mut editor, "x");
  key(&mut editor, "Ctrl+y");
  assert_eq!(editor.text(), "abdcx", "an edit drops the redo steps");

  editor.write(&Edit::text("set"));
  key(&mut editor, "Ctrl+z");
  assert_eq!(
    editor.text(),
    "abdcx",
    "a value from outside is a step of its own"
  );

  key(&mut editor, "Ctrl+y");
  assert_eq!(editor.text(), "set");
}

#[test]
fn selection_takes_whole_clusters_over_emoji_and_combining_marks() {
  // A family (five code points), e with a combining acute, a flag (two regional
  // indicators), a thumbs up with a skin tone.
  let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
  let text = format!("a{family}e\u{301}\u{1F1F0}\u{1F1F7}\u{1F44D}\u{1F3FD}");
  let mut editor = editor(&text);

  key(&mut editor, "Home");
  keys(&mut editor, &["ArrowRight", "Shift+ArrowRight"]);
  assert_eq!(editor.selection(), selection(1, 2));
  assert_eq!(editor.text_in(1..9, Unit::Utf16), family);
  assert_eq!(editor.selection_in(Unit::Utf16), (1, 9));
  assert_eq!(editor.selection_in(Unit::CodePoint), (1, 6));

  keys(&mut editor, &["Shift+ArrowRight"]);
  assert_eq!(
    editor.selection(),
    selection(1, 3),
    "e and its accent are one cluster"
  );

  key(&mut editor, "Backspace");
  assert_eq!(editor.text(), "a\u{1F1F0}\u{1F1F7}\u{1F44D}\u{1F3FD}");

  key(&mut editor, "Delete");
  assert_eq!(editor.text(), "a\u{1F44D}\u{1F3FD}", "the flag goes whole");

  // An adapter selecting inside a cluster is widened to it outside a composition.
  editor.select_in(2, 3, Unit::Utf16);
  assert_eq!(editor.selection(), selection(1, 1));
  assert_eq!(editor.selection_in(Unit::Utf16), (1, 1));

  key(&mut editor, "End");
  assert_eq!(editor.selection(), selection(2, 2));
  assert_eq!(editor.count_in(Unit::Utf16), 5);
}

#[test]
fn the_pointer_selects_by_cluster_word_and_line() {
  let mut editor = editor("alpha beta gamma");

  // Ten units a cluster: x 34 is nearest the boundary after "alp".
  editor.press(34., 5., 1, false);
  assert_eq!(editor.selection(), selection(3, 3));

  editor.press(72., 5., 1, true);
  assert_eq!(editor.selection(), selection(3, 7));

  editor.press(72., 5., 2, false);
  assert_eq!(editor.selection(), selection(6, 10));

  editor.drag(150., 5.);
  assert_eq!(
    editor.selection(),
    selection(6, 16),
    "a drag extends by words"
  );

  editor.drag(5., 5.);
  assert_eq!(
    editor.selection(),
    selection(10, 0),
    "backward from the word's end"
  );

  editor.press(72., 5., 3, false);
  assert_eq!(editor.selection(), selection(0, 16));
}

#[test]
fn the_clipboard_copies_cuts_and_pastes_through_commands() {
  let mut editor = editor("alpha beta");

  let _ = editor.perform(Command::SelectAll);

  let copied = editor.perform(Command::Copy);

  assert_eq!(copied.clipboard.as_deref(), Some("alpha beta"));

  editor.press(52., 5., 2, false);

  let cut = editor.perform(Command::Cut);

  assert_eq!(cut.clipboard.as_deref(), Some(" "));
  assert_eq!(editor.text(), "alphabeta");

  let _ = editor.perform(Command::Paste("\r\nline\nbreaks\n".into()));
  assert_eq!(
    editor.text(),
    "alphalinebreaksbeta",
    "a field drops line breaks"
  );

  editor.set_multiline(true);
  let _ = editor.perform(Command::Paste("\r\n".into()));
  assert_eq!(
    editor.text(),
    "alphalinebreaks\nbeta",
    "an editor keeps them as line feeds"
  );

  editor.set_read_only(true);
  assert!(!editor.perform(Command::Paste("x".into())).handled);
  assert!(
    editor.perform(Command::SelectAll).handled,
    "a read-only text still selects"
  );
  assert!(editor.perform(Command::Copy).handled);
}

#[test]
fn an_editor_moves_between_lines_keeping_its_column() {
  let mut editor = editor("abcdef\nab\nabcdef");

  editor.set_multiline(true);
  editor.set_selection(selection(5, 5));

  let down = Command::Move {
    movement: crate::text::editing::Movement::Lines(1),
    extend: false,
  };

  let _ = editor.perform(down.clone());
  assert_eq!(editor.selection(), selection(9, 9), "the short line's end");

  let _ = editor.perform(down.clone());
  assert_eq!(
    editor.selection(),
    selection(15, 15),
    "the column comes back"
  );

  let _ = editor.perform(down);
  assert_eq!(
    editor.selection(),
    selection(16, 16),
    "past the last line, the end"
  );
}

#[test]
fn geometry_answers_synchronously_after_an_edit() {
  let mut editor = editor("ab");

  key(&mut editor, "End");
  assert_eq!(editor.rect_for(2..2, Unit::Utf16).x, 20.);

  key(&mut editor, "c");
  // No frame ran between the edit and the query: the query lays the text out.
  assert_eq!(editor.rect_for(3..3, Unit::Utf16).x, 30.);
  assert_eq!(editor.rect_for(0..2, Unit::Utf16).width, 20.);
  assert_eq!(editor.index_at(19., 5., Unit::Utf16), 2);
}

/// An entry under the controlled contract: the truth a signal, the handler `handle`.
fn controlled(
  value: &str,
  handle: impl Fn(Signal<String>, Edit) + 'static,
) -> (Editor<Grid>, Signal<String>) {
  let truth = Signal::new(value.to_owned());
  let mut editor = editor(value);

  editor.set_arbiter(Controlled::new(
    move || Edit::text(truth.get_untracked()),
    move |edit| handle(truth, edit),
  ));

  (editor, truth)
}

#[test]
fn a_handler_that_keeps_nothing_vetoes_the_edit() {
  let (mut editor, truth) = controlled("abc", |_, _| {});

  key(&mut editor, "End");
  key(&mut editor, "x");
  assert_eq!(editor.text(), "abc");
  assert_eq!(truth.get_untracked(), "abc");

  key(&mut editor, "Ctrl+z");
  assert_eq!(editor.text(), "abc", "a vetoed edit leaves no step");
}

#[test]
fn a_handler_that_transforms_the_edit_is_shown_with_the_caret_mapped() {
  let (mut editor, truth) = controlled("", |truth, edit| truth.set(edit.text.to_uppercase()));

  keys(&mut editor, &["a", "b"]);
  assert_eq!(editor.text(), "AB");
  assert_eq!(truth.get_untracked(), "AB");
  assert_eq!(editor.selection(), selection(2, 2));

  key(&mut editor, "Home");
  key(&mut editor, "c");
  assert_eq!(editor.text(), "CAB");
  assert_eq!(
    editor.selection(),
    selection(1, 1),
    "the caret mapped through the change"
  );
}

#[test]
fn a_write_of_what_is_shown_changes_nothing() {
  let notices = Rc::new(RefCell::new(Vec::<Notice>::new()));
  let mut editor = editor("abc");
  let heard = Rc::clone(&notices);

  editor.set_notifier(move |notice| heard.borrow_mut().push(notice));
  assert!(!editor.write(&Edit::text("abc")));
  assert!(notices.borrow().is_empty());

  assert!(editor.write(&Edit::text("abcd")));
  assert_eq!(notices.borrow()[0].origin, Origin::App);
  assert!(notices.borrow()[0].restart, "an input method starts afresh");
}

#[test]
fn a_composition_is_held_and_settled_at_its_end() {
  // The handler accepts only digits.
  let (mut editor, truth) = controlled("1", |truth, edit| {
    if edit
      .text
      .chars()
      .all(|character| character.is_ascii_digit())
    {
      truth.set(edit.text.to_string());
    }
  });

  key(&mut editor, "End");
  editor.set_marked("k", Vec::new(), 1..1, None, Unit::Utf16);
  assert_eq!(
    editor.text(),
    "1k",
    "the input method's text shows while it composes"
  );
  assert_eq!(editor.marked_range_in(Unit::Utf16), Some(1..2));

  editor.set_marked(
    "\u{D55C}",
    vec![Clause {
      range: 0..3,
      style: ClauseStyle::Target,
    }],
    1..1,
    None,
    Unit::Utf16,
  );
  assert_eq!(editor.text(), "1\u{D55C}");
  assert_eq!(truth.get_untracked(), "1");

  // The commit ends the composition: the truth is settled.
  editor.replace(None, "\u{D55C}", Unit::Utf16);
  assert!(!editor.is_composing());
  assert_eq!(editor.text(), "1");

  // An accepted composition stays, as one undo step.
  editor.set_marked("2", Vec::new(), 1..1, None, Unit::Utf16);
  editor.set_marked("23", Vec::new(), 2..2, None, Unit::Utf16);
  editor.unmark();
  assert_eq!(editor.text(), "123");
  assert_eq!(truth.get_untracked(), "123");

  key(&mut editor, "Ctrl+z");
  assert_eq!(editor.text(), "1");
}

#[test]
fn a_write_during_a_composition_waits_for_its_end() {
  let mut editor = editor("ab");

  key(&mut editor, "End");
  editor.set_marked("c", Vec::new(), 1..1, None, Unit::Utf16);
  assert!(!editor.write(&Edit::text("written")));
  assert_eq!(editor.text(), "abc");

  editor.unmark();
  assert_eq!(editor.text(), "written");
}

#[test]
fn an_empty_composition_ends_it_with_nothing() {
  let mut editor = editor("ab");

  key(&mut editor, "End");
  editor.set_marked("xy", Vec::new(), 2..2, None, Unit::Utf16);
  editor.set_marked("", Vec::new(), 0..0, None, Unit::Utf16);
  assert!(!editor.is_composing());
  assert_eq!(editor.text(), "ab");
}

#[test]
fn transpose_swaps_the_clusters_around_the_caret() {
  let mut editor = editor("abc");

  editor.set_selection(selection(1, 1));
  let _ = editor.perform(Command::Transpose);
  assert_eq!(editor.text(), "bac");

  editor.set_selection(selection(3, 3));
  let _ = editor.perform(Command::Transpose);
  assert_eq!(editor.text(), "bca");
}

#[test]
fn a_masked_entry_lays_out_bullets_and_keeps_real_offsets() {
  use crate::text::{Laid as _, MaskState, Masked};

  let state = Rc::new(MaskState::new('•'));
  let mut editor = Editor::new(
    Masked::new(Grid, Rc::clone(&state)),
    RunStyle::new(13.),
    ParagraphStyle::default(),
  );

  state.set_masked(true);
  editor.write(&Edit::text("pä55"));

  let laid = editor.laid();

  assert_eq!(laid.size().width, 40.);
  assert_eq!(laid.hit_test(22., 0.).position.index, 3);
  assert_eq!(laid.caret(crate::text::Position::new(3)).primary.x, 20.);

  // A word move or a word deletion in a secure entry crosses the whole text.
  key(&mut editor, "End");
  key(&mut editor, "Ctrl+Backspace");
  assert_eq!(editor.text(), "");

  editor.write(&Edit::text("ab"));
  state.reveal(Some(1));
  editor.relayout();
  assert_eq!(editor.laid().size().width, 20.);
}

#[test]
fn an_input_methods_deletion_and_its_commit_undo_as_one_step() {
  let mut editor = editor("한글");

  editor.set_selection(TextSelection { start: 2, end: 2 });
  editor.replace(Some(3..6), "", Unit::Utf8);
  editor.replace(None, "契", Unit::Utf8);
  assert_eq!(editor.text(), "한契");
  key(&mut editor, "Ctrl+z");
  assert_eq!(editor.text(), "한글");
}
