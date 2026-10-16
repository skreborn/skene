//! An input method's composition through the input-client surface: shown underlined by
//! clause, committed as one undo step, its caret where the input method puts it.

use skene_foundation::{
  seam::content::Unit,
  text::editing::{Clause, ClauseStyle},
};

use crate::{field, key, value};

#[test]
fn a_composition_shows_its_clauses_and_commits_as_one_step() {
  let mut field = field("", 12);

  // A Japanese input method: raw kana, then converted with the first clause as the target.
  field
    .editor_mut()
    .set_marked("にほんご", Vec::new(), 12..12, None, Unit::Utf8);
  assert_eq!(
    field.render().to_styled(),
    "|にほんご    |\n|~>~>~>~>....|\ncursor 8,0\n"
  );

  field.editor_mut().set_marked(
    "日本語",
    vec![
      Clause {
        range: 0..6,
        style: ClauseStyle::Target,
      },
      Clause {
        range: 6..9,
        style: ClauseStyle::Converted,
      },
    ],
    0..0,
    None,
    Unit::Utf8,
  );

  let frame = field.render();

  assert_eq!(
    frame.to_styled(),
    "|日本語      |\n|=>=>_>......|\ncursor 0,0\n"
  );
  assert_eq!(field.editor().marked_range_in(Unit::Utf16), Some(0..3));
  assert_eq!(
    field.editor().rect_for(1..2, Unit::Utf16).x,
    2.,
    "where candidates go"
  );

  field.editor_mut().replace(None, "日本語", Unit::Utf8);
  assert!(!field.editor().is_composing());
  assert_eq!(
    field.render().to_styled(),
    "|日本語      |\n|.>.>.>......|\ncursor 6,0\n"
  );

  key(&mut field, "Ctrl+z");
  assert_eq!(value(&field), "", "the composition undoes as one step");
}

#[test]
fn hangul_composes_jamo_by_jamo_in_place() {
  let mut field = field("", 12);

  for marked in ["ㅎ", "하", "한"] {
    field.editor_mut().set_marked(
      marked,
      Vec::new(),
      marked.len()..marked.len(),
      None,
      Unit::Utf8,
    );
  }

  assert_eq!(value(&field), "한");
  field.editor_mut().unmark();
  field
    .editor_mut()
    .set_marked("ㄱ", Vec::new(), 3..3, None, Unit::Utf8);
  assert_eq!(value(&field), "한ㄱ");
  assert_eq!(field.editor().marked_range_in(Unit::CodePoint), Some(1..2));
  field.editor_mut().replace(None, "글", Unit::Utf8);
  assert_eq!(value(&field), "한글");
}
