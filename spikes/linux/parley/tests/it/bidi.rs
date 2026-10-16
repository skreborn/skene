//! Bidirectional text: visual movement crosses direction runs on screen, logical movement in
//! the text's order, carets split at direction boundaries, and hits land where they are drawn.

use skene_foundation::{
  seam::content::TextSelection,
  text::{
    BaseDirection, Direction, Laid as _, ParagraphStyle, Position, TextBoundary, VisualMove,
    editing::{Command, Movement},
  },
};

use crate::{editor, laid};

fn moving(movement: Movement) -> Command {
  Command::Move {
    movement,
    extend: false,
  }
}

/// Moves right (or left) until the caret stops, the caret's x after each move.
fn sweep(text: &str, style: ParagraphStyle, start: usize, right: bool) -> (Vec<f32>, Vec<usize>) {
  let mut editor = editor(text, style);
  let direction = if right {
    VisualMove::Right
  } else {
    VisualMove::Left
  };

  editor.set_selection(TextSelection { start, end: start });

  let mut xs = vec![editor.caret_rect().x];
  let mut indices = vec![editor.focus().index];

  while editor.perform(moving(Movement::Visual(direction))).handled {
    xs.push(editor.caret_rect().x);
    indices.push(editor.focus().index);
    assert!(xs.len() < 200, "the sweep ends");
  }

  (xs, indices)
}

#[test]
fn right_crosses_a_hebrew_run_on_screen_in_left_to_right_text() {
  let text = "abc \u{5E9}\u{5DC}\u{5D5}\u{5DD} def";
  let (xs, indices) = sweep(text, ParagraphStyle::default(), 0, true);

  assert!(
    xs.windows(2).all(|pair| pair[1] > pair[0] - 0.01),
    "the caret only moves right on screen: {xs:?}"
  );
  assert!(
    indices.windows(2).any(|pair| pair[1] < pair[0]),
    "inside the Hebrew run the logical index goes back: {indices:?}"
  );
  assert_eq!(*indices.last().unwrap(), text.len());

  // One stop for each of the thirteen cluster boundaries, the two ends of the Hebrew run
  // where it meets the Latin text each a single stop on screen.
  assert_eq!(indices.len(), 13, "{indices:?}");

  for boundary in [0, 1, 2, 3, 6, 8, 10, 13, 14, 15, 16] {
    assert!(
      indices.contains(&boundary),
      "{boundary} visited: {indices:?}"
    );
  }
}

#[test]
fn left_from_the_end_mirrors_the_sweep() {
  let text = "abc \u{5E9}\u{5DC}\u{5D5}\u{5DD} def";
  let (xs, indices) = sweep(text, ParagraphStyle::default(), 13, false);

  assert!(xs.windows(2).all(|pair| pair[1] < pair[0] + 0.01), "{xs:?}");
  assert_eq!(*indices.last().unwrap(), 0);
}

#[test]
fn in_a_right_to_left_paragraph_left_moves_forward_in_the_text() {
  let text =
    "\u{645}\u{631}\u{62D}\u{628}\u{627} \u{628}\u{627}\u{644}\u{639}\u{627}\u{644}\u{645}";
  let style = ParagraphStyle::default().with_direction(BaseDirection::RightToLeft);
  let mut editor = editor(text, style);

  editor.set_selection(TextSelection { start: 0, end: 0 });

  let start = editor.caret_rect().x;
  let _ = editor.perform(moving(Movement::Visual(VisualMove::Left)));

  assert_eq!(editor.selection(), TextSelection { start: 1, end: 1 });
  assert!(
    editor.caret_rect().x < start,
    "the caret moved left on screen"
  );

  // A word to the left is the next word in the text.
  let _ = editor.perform(moving(Movement::Visual(VisualMove::WordLeft)));
  let after_word = editor.selection().end;

  assert!(after_word >= 5, "{after_word}");

  // Home is the line's start: its right edge in a right-to-left paragraph.
  let _ = editor.perform(moving(Movement::Visual(VisualMove::LineStart)));

  assert_eq!(editor.selection().end, 0);
  assert!(editor.caret_rect().x > start - 1.);

  // Logical movement ignores the screen.
  let _ = editor.perform(moving(Movement::Logical(
    TextBoundary::Word,
    Direction::Forward,
  )));

  assert_eq!(editor.selection().end, 5);
}

#[test]
fn a_direction_boundary_has_two_carets() {
  let laid = laid("abc \u{5E9}\u{5DC}\u{5D5}\u{5DD} def", None);
  // Byte 12 ends the Hebrew run (logically) and precedes " def": drawn at the run's left
  // edge on one side and before " def" on the other.
  let carets = laid.caret(Position::new(12));
  let secondary = carets.secondary.expect("a split caret");

  assert!((secondary.x - carets.primary.x).abs() > 10.);
  assert!(carets.primary.right_to_left != secondary.right_to_left);

  assert!(
    laid.caret(Position::new(1)).secondary.is_none(),
    "inside a run, one caret"
  );
}

#[test]
fn a_selection_across_directions_covers_several_rectangles() {
  let laid = laid("abc \u{5E9}\u{5DC}\u{5D5}\u{5DD} def", None);
  // "c " and the first two Hebrew letters: visually discontiguous.
  let rects = laid.range_rects(2..8);

  assert!(rects.len() >= 2, "{rects:?}");
}

#[test]
fn a_click_lands_where_the_text_is_drawn() {
  let text = "\u{5E9}\u{5DC}\u{5D5}\u{5DD}";
  let laid = laid(text, None);
  let width = laid.size().width;

  // The first Hebrew letter is drawn at the right: a click at the far right is before it.
  assert_eq!(laid.hit_test(width - 0.5, 8.).position.index, 0);
  assert_eq!(laid.hit_test(0.5, 8.).position.index, text.len());
}

#[test]
fn backspace_takes_an_arabic_letter_with_its_mark() {
  // Beh with a kasra, then alef: two clusters.
  let mut editor = editor("\u{628}\u{650}\u{627}", ParagraphStyle::default());
  let _ = editor.perform(Command::Move {
    movement: Movement::Logical(TextBoundary::Text, Direction::Forward),
    extend: false,
  });
  let _ = editor.perform(Command::Delete(TextBoundary::Grapheme, Direction::Backward));

  assert_eq!(editor.text(), "\u{628}\u{650}");

  let _ = editor.perform(Command::Delete(TextBoundary::Grapheme, Direction::Backward));

  assert_eq!(editor.text(), "");
}
