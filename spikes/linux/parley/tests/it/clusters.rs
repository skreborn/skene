//! Clusters: the caret steps over a ZWJ family, a flag, a modified emoji and a combining
//! sequence whole, and the selection the app sees counts them as one each.

use skene_foundation::{
  seam::content::{TextSelection, Unit},
  text::{
    Direction, Laid as _, ParagraphStyle, TextBoundary, VisualMove,
    editing::{Command, Movement},
  },
};

use crate::editor;

#[test]
fn the_caret_steps_over_whole_clusters() {
  let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
  let text = format!("a{family}\u{1F1F0}\u{1F1F7}e\u{301}\u{1F44D}\u{1F3FD}b");
  let mut editor = editor(&text, ParagraphStyle::default());
  let mut stops = vec![editor.focus().index];

  editor.set_selection(TextSelection { start: 0, end: 0 });

  while editor
    .perform(Command::Move {
      movement: Movement::Visual(VisualMove::Right),
      extend: false,
    })
    .handled
  {
    stops.push(editor.focus().index);
  }

  assert_eq!(stops[1..], [1, 19, 27, 30, 38, 39]);

  // Selecting the second cluster by keys selects the family whole.
  editor.set_selection(TextSelection { start: 1, end: 1 });
  let _ = editor.perform(Command::Move {
    movement: Movement::Visual(VisualMove::Right),
    extend: true,
  });

  assert_eq!(editor.selection(), TextSelection { start: 1, end: 2 });
  assert_eq!(editor.selection_in(Unit::Utf16), (1, 9));

  let _ = editor.perform(Command::Delete(TextBoundary::Grapheme, Direction::Forward));

  assert_eq!(
    editor.text(),
    "a\u{1F1F0}\u{1F1F7}e\u{301}\u{1F44D}\u{1F3FD}b"
  );
}

#[test]
fn a_hit_inside_a_cluster_lands_on_its_boundary() {
  let text = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
  let editor = editor(text, ParagraphStyle::default());
  let width = editor.laid().size().width;

  for x in [0.5, width / 2. - 1., width / 2. + 1., width - 0.5] {
    let index = editor.index_at(x, 8., Unit::Utf8);

    assert!(index == 0 || index == text.len(), "{x}: {index}");
  }
}
