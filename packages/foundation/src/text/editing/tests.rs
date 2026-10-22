//! The editing core over the canned engine: keys, selection by grapheme cluster, undo, the
//! controlled contract, composition and the input-client surface. Bidirectional movement is
//! held by `skene-parley`'s tests, over a real shaper.

use std::{cell::RefCell, rc::Rc, str::FromStr as _};

use skene_reactive::{Get as _, Set as _, Signal};

use crate::{
  Code,
  Edit,
  Key,
  KeyEvent,
  Modifiers,
  TextSelection,
  Unit,
  text::{
    ParagraphStyle,
    TextStyle,
    editing::{
      Binding,
      Clause,
      ClauseStyle,
      Command,
      ControlledText,
      Editor,
      Notice,
      Origin,
      SelectionEnd,
    },
    grid::Grid,
  },
};

fn editor(text: &str) -> Editor<Grid> {
  let mut editor = Editor::new(Grid, TextStyle::new(13.), ParagraphStyle::default());

  editor.write(&Edit::new(text));

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
      "Ctrl" => modifiers.ctrl = true,
      "Shift" => modifiers.shift = true,
      "Alt" => modifiers.alt = true,
      _ => panic!("unknown modifier {part}"),
    }
  }

  let event = KeyEvent {
    key: Key::from_str(key).expect("a key's name"),
    code: Code::Unidentified,
    modifiers,
    repeat: false,
    composing: false,
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
fn word_deletion_follows_chromiums_word_boundaries() {
  let mut editor = editor("say don't foo.bar_baz(");

  key(&mut editor, "End");

  for expected in [
    "say don't foo.bar_baz",
    "say don't foo.",
    "say don't foo",
    "say don't ",
    "say ",
    "",
  ] {
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

  editor.write(&Edit::new("set"));
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

/// A base typed before a combining mark joins it into one cluster, and the caret lands after
/// the cluster rather than inside it.
#[test]
fn a_base_typed_before_a_combining_mark_leaves_the_caret_after_the_cluster() {
  let mut editor = editor("\u{301}x");

  key(&mut editor, "Home");
  key(&mut editor, "a");

  assert_eq!(editor.text(), "a\u{301}x");
  assert_eq!(editor.selection(), selection(1, 1));
  assert_eq!(editor.focus().index, 3, "the caret is on the cluster's end");
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
fn a_touch_handle_moves_its_end_of_the_selection_and_never_past_the_other() {
  let mut editor = editor("alpha beta gamma");

  editor.press(72., 5., 2, false);
  assert_eq!(editor.selection(), selection(6, 10));

  assert!(editor.move_selection_end(SelectionEnd::End, 150., 5.));
  assert_eq!(
    editor.selection(),
    selection(6, 15),
    "the end moves, the start stays the anchor"
  );

  editor.move_selection_end(SelectionEnd::Start, 34., 5.);
  assert_eq!(
    editor.selection(),
    selection(15, 3),
    "the start moves and becomes the caret"
  );

  editor.move_selection_end(SelectionEnd::Start, 155., 5.);
  assert_eq!(
    editor.selection(),
    selection(15, 14),
    "the start stops a cluster short of the end"
  );

  editor.move_selection_end(SelectionEnd::End, 5., 5.);
  assert_eq!(
    editor.selection(),
    selection(14, 15),
    "the end stops a cluster past the start"
  );
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
  assert_eq!(
    editor.sanitized("a\rb\r\n\rc"),
    "a\nb\n\nc",
    "a lone return too"
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
  let mut editor = editor("");

  editor.set_multiline(true);
  editor.write(&Edit::new("abcdef\nab\nabcdef"));
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

  editor.set_selection(selection(4, 4));

  let _ = editor.perform(Command::Move {
    movement: crate::text::editing::Movement::Lines(-1),
    extend: true,
  });
  assert_eq!(
    editor.selection(),
    selection(4, 0),
    "past the first line, the start"
  );
}

#[test]
fn geometry_answers_synchronously_after_an_edit() {
  let mut editor = editor("ab");

  key(&mut editor, "End");
  assert_eq!(editor.rect_for(2..2, Unit::Utf16).origin.x, 20.);

  key(&mut editor, "c");
  // No frame ran between the edit and the query: the query lays the text out.
  assert_eq!(editor.rect_for(3..3, Unit::Utf16).origin.x, 30.);
  assert_eq!(editor.rect_for(0..2, Unit::Utf16).size.width, 20.);
  assert_eq!(editor.rect_for_bytes(1..1).origin.x, 10.);
  assert_eq!(editor.index_at(19., 5., Unit::Utf16), 2);
  assert_eq!(editor.byte_at(19., 5.), 2);
}

/// An entry under the controlled contract: the truth a signal, the handler `handle`.
fn controlled(
  value: &str,
  handle: impl Fn(Signal<String>, Edit) + 'static,
) -> (Editor<Grid>, Signal<String>) {
  let truth = Signal::new(value.to_owned());
  let mut editor = editor(value);

  editor.set_arbiter(ControlledText::new(
    move || Edit::new(truth.get_untracked()),
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
fn a_rejected_edit_leaves_no_undo_step() {
  // The handler accepts only digits.
  let (mut editor, truth) = controlled("", |truth, edit| {
    if edit
      .text
      .chars()
      .all(|character| character.is_ascii_digit())
    {
      truth.set(edit.text.to_string());
    }
  });

  keys(&mut editor, &["1", "2"]);
  key(&mut editor, "x");
  assert_eq!(editor.text(), "12");
  assert_eq!(editor.selection(), selection(2, 2));

  // The truth written back after the report is what the entry shows: no step either.
  assert!(!editor.write(&Edit::new(truth.get_untracked())));

  key(&mut editor, "Ctrl+z");
  assert_eq!(
    editor.text(),
    "",
    "the accepted run undoes as one, the rejected key replays nothing"
  );

  key(&mut editor, "Ctrl+y");
  assert_eq!(editor.text(), "12");

  key(&mut editor, "Ctrl+y");
  assert_eq!(editor.text(), "12", "nothing further to redo");
}

#[test]
fn a_write_of_what_is_shown_changes_nothing() {
  let notices = Rc::new(RefCell::new(Vec::<Notice>::new()));
  let mut editor = editor("abc");
  let heard = Rc::clone(&notices);

  editor.set_notifier(move |notice| heard.borrow_mut().push(notice));
  assert!(!editor.write(&Edit::new("abc")));
  assert!(notices.borrow().is_empty());

  assert!(editor.write(&Edit::new("abcd")));
  assert_eq!(notices.borrow()[0].origin, Origin::App);
  assert!(notices.borrow()[0].restart, "an input method starts afresh");
}

#[test]
fn a_truth_shows_as_the_entry_takes_it() {
  let mut field = editor("");

  assert!(field.write(&Edit::new("a\r\nb")));
  assert_eq!(
    field.text(),
    "ab",
    "a field drops a written truth's line breaks"
  );
  assert!(
    !field.write(&Edit::new("a\nb")),
    "a truth shown as the entry takes it is shown already"
  );

  let (mut controlled, _) = controlled("", |truth, edit| truth.set(format!("{}\n", edit.text)));

  key(&mut controlled, "a");
  assert_eq!(
    controlled.text(),
    "a",
    "a field drops a verdict's line breaks"
  );

  let mut editor = editor("");

  editor.set_multiline(true);
  assert!(editor.write(&Edit::new("a\r\nb\rc")));
  assert_eq!(
    editor.text(),
    "a\nb\nc",
    "an editor's line breaks are line feeds"
  );
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
  assert!(!editor.write(&Edit::new("written")));
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
    TextStyle::new(13.),
    ParagraphStyle::default(),
  );

  state.set_masked(true);
  editor.write(&Edit::new("pä55"));

  let laid = editor.laid();

  assert_eq!(laid.size().width, 40.);
  assert_eq!(laid.hit_test(22., 0.).position.index, 3);
  assert_eq!(laid.caret(crate::text::Position::new(3)).primary.x, 20.);

  // A word move or a word deletion in a secure entry crosses the whole text.
  key(&mut editor, "End");
  key(&mut editor, "Ctrl+Backspace");
  assert_eq!(editor.text(), "");

  editor.write(&Edit::new("ab"));
  state.reveal(Some(1));
  editor.relayout();
  assert_eq!(editor.laid().size().width, 20.);
}

#[test]
fn an_input_methods_deletion_and_its_commit_undo_as_one_step() {
  let mut editor = editor("한글");

  editor.set_selection(TextSelection { start: 2, end: 2 });
  editor.replace_bytes(Some(3..6), "");
  editor.replace_bytes(None, "契");
  assert_eq!(editor.text(), "한契");
  key(&mut editor, "Ctrl+z");
  assert_eq!(editor.text(), "한글");
}

#[test]
fn a_secure_text_offers_no_copy_nor_cut_and_its_word_is_the_whole_text() {
  let mut editor = editor("pass word");

  editor.set_secure(true);

  let _ = editor.perform(Command::SelectAll);

  assert_eq!(
    editor.perform(Command::Copy),
    crate::text::editing::Outcome::IGNORED
  );
  assert_eq!(
    editor.perform(Command::Cut),
    crate::text::editing::Outcome::IGNORED
  );
  assert_eq!(editor.text(), "pass word", "a refused cut removes nothing");

  // Each cluster of the canned engine is ten wide: a press at 25 is in `pass`.
  let _ = editor.press(25., 10., 2, false);
  assert_eq!(
    editor.selection(),
    selection(0, 9),
    "a double press selects it all"
  );

  editor.set_secure(false);

  let _ = editor.press(25., 10., 2, false);
  assert_eq!(
    editor.selection(),
    selection(0, 4),
    "and a plain text's word alone"
  );
  assert!(editor.perform(Command::Copy).clipboard.is_some());
}

#[test]
fn a_secure_text_keeps_no_undo_history() {
  let mut editor = editor("");

  keys(&mut editor, &["o", "l", "d", "Ctrl+a", "x"]);
  editor.set_secure(true);
  key(&mut editor, "Ctrl+z");
  assert_eq!(editor.text(), "x", "what it kept is dropped");

  keys(
    &mut editor,
    &["Ctrl+a", "a", "b", "c", "Ctrl+a", "z", "Ctrl+z"],
  );
  assert_eq!(editor.text(), "z", "and nothing is recorded");

  editor.set_secure(false);
  keys(&mut editor, &["y", "Ctrl+z"]);
  assert_eq!(editor.text(), "z", "a plain text records again");
}

#[test]
fn a_command_logged_masked_shows_its_text_as_marks() {
  assert_eq!(
    Command::Insert("e\u{301}x".into()).masked('•'),
    "Insert(\"••\")"
  );
  assert_eq!(Command::Paste("ab".into()).masked('*'), "Paste(\"**\")");
  assert_eq!(Command::SelectAll.masked('•'), "SelectAll");
}

#[test]
fn a_command_that_edits_during_a_composition_ends_it_first() {
  use crate::text::{Direction, TextBoundary};

  let commands = [
    Command::Undo,
    Command::Redo,
    Command::Insert("\u{E9}".into()),
    Command::InsertNewline,
    Command::Paste("\u{E9}".into()),
    Command::Cut,
    Command::Delete(TextBoundary::Grapheme, Direction::Backward),
    Command::Transpose,
  ];

  for command in commands {
    let mut editor = editor("");

    keys(&mut editor, &["a", "b", "c"]);
    editor.set_marked("\u{D55C}\u{AE00}", Vec::new(), 0..0, None, Unit::Utf16);

    let shown = format!("{command:?}");
    let _ = editor.perform(command);

    assert!(!editor.is_composing(), "{shown}");
    assert_eq!(editor.marked_range_in(Unit::Utf16), None, "{shown}");

    let _ = editor.laid();
  }

  let mut undone = editor("");

  keys(&mut undone, &["a", "b", "c"]);
  undone.set_marked("xyz", Vec::new(), 3..3, None, Unit::Utf16);
  let _ = undone.perform(Command::Undo);
  assert_eq!(undone.text(), "abc", "the composition is the step undone");

  let mut deleting = editor("");

  deleting.set_marked("\u{D55C}\u{AE00}", Vec::new(), 2..2, None, Unit::Utf16);
  let _ = deleting.perform(Command::Delete(TextBoundary::Grapheme, Direction::Backward));
  assert_eq!(deleting.text(), "\u{D55C}");
}

#[test]
fn a_composition_that_changes_nothing_is_no_undo_step() {
  let mut cancelled = editor("");

  keys(&mut cancelled, &["a", "b"]);
  cancelled.set_marked("xy", Vec::new(), 2..2, None, Unit::Utf16);
  cancelled.set_marked("", Vec::new(), 0..0, None, Unit::Utf16);
  key(&mut cancelled, "Ctrl+z");
  assert_eq!(cancelled.text(), "", "a cancelled composition");

  let mut committed = editor("");

  keys(&mut committed, &["a", "b"]);
  committed.set_marked("x", Vec::new(), 1..1, None, Unit::Utf16);
  committed.replace(None, "", Unit::Utf16);
  key(&mut committed, "Ctrl+z");
  assert_eq!(committed.text(), "", "a composition committed as nothing");
}

#[test]
fn backspace_removes_one_code_point_but_keeps_emoji_sequences_whole() {
  let mut combining = editor("ae\u{301}");

  key(&mut combining, "End");
  key(&mut combining, "Backspace");
  assert_eq!(combining.text(), "ae", "the combining acute alone");
  key(&mut combining, "Backspace");
  assert_eq!(combining.text(), "a");

  for sequence in [
    "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
    "\u{1F44B}\u{1F3FD}",
    "\u{2764}\u{FE0F}",
    "1\u{FE0F}\u{20E3}",
    "\u{1F1EB}\u{1F1EE}",
  ] {
    let mut sequenced = editor(&format!("a{sequence}"));

    key(&mut sequenced, "End");
    key(&mut sequenced, "Backspace");
    assert_eq!(sequenced.text(), "a", "{sequence:?}");
  }

  let mut forward = editor("ae\u{301}b");

  keys(&mut forward, &["End", "ArrowLeft", "Delete"]);
  assert_eq!(
    forward.text(),
    "ae\u{301}",
    "forward removes the whole cluster"
  );
  keys(&mut forward, &["ArrowLeft", "Delete"]);
  assert_eq!(forward.text(), "a");
}

#[test]
fn typing_over_a_selection_is_one_undo_step_with_what_follows() {
  let mut editor = editor("hello");

  keys(&mut editor, &["Ctrl+a", "x", "y", "z"]);
  assert_eq!(editor.text(), "xyz");

  key(&mut editor, "Ctrl+z");
  assert_eq!(editor.text(), "hello");
  assert_eq!(editor.selection(), selection(0, 5));
}
