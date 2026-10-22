//! What a drawn text entry reports of its text to assistive technology, read once above the
//! engine from its editor and the layout the editor shows.

use std::ops::Range;

use super::{Affinity, Laid as _, LineInfo, Position, Round, TextEngine, editing::Editor};
use crate::{AccessRun, AccessText, AccessTextPosition, Point, Rect, Size};

/// What a secure entry shows of each of its characters.
const MARK: &str = "\u{2022}";

impl AccessText {
  /// What `editor` reports: a run for each line its layout lays, in the layout's
  /// coordinates, and its selection; a secure entry's runs hold a mark for each of its
  /// characters, and a line's direction is its caret's.
  pub fn of<E: TextEngine>(editor: &Editor<E>) -> Self {
    let text = editor.text();
    let index = editor.index();
    let secure = editor.is_secure();
    let laid = editor.laid();
    let lines = laid.lines();
    let last = lines.len().saturating_sub(1);
    let mut runs = Vec::with_capacity(lines.len());
    let mut anchor = None;
    let mut focus = None;

    for (number, (at, line)) in (0..=u16::MAX).zip(lines.iter().enumerate()) {
      let range = line.range.start.min(text.len())..line.range.end.min(text.len());
      let first = index.grapheme_at_byte(range.start, Round::Down);
      let end = index.grapheme_at_byte(range.end, Round::Up);
      let mut previous_blank = if secure {
        range.start == 0
      }
      else {
        text[..range.start]
          .chars()
          .next_back()
          .is_none_or(char::is_whitespace)
      };
      let right_to_left = laid.caret(Position::new(range.start)).primary.right_to_left;
      let mut shown = String::with_capacity(range.len());
      let mut lengths = Vec::with_capacity(end - first);
      let mut edges = Vec::with_capacity(end - first);
      let mut starts = Vec::with_capacity(end - first);
      let mut words = Vec::new();

      for cluster in first..end {
        let start = index.byte_of_grapheme(cluster);
        let stop = index.byte_of_grapheme(cluster + 1);
        let character = if secure { MARK } else { &text[start..stop] };
        let blank = character.chars().all(char::is_whitespace);
        let from = laid.caret(Position::new(start)).primary.x;
        let to = laid
          .caret(Position::with(stop, Affinity::Upstream))
          .primary
          .x;
        let (left, right) = (from.min(to), from.max(to));
        let trailing = if right_to_left { left } else { right };

        if previous_blank
          && !blank
          && let Ok(count) = u8::try_from(lengths.len())
        {
          words.push(count);
        }

        previous_blank = blank;
        starts.push(lengths.len());
        shown.push_str(character);

        for (piece, length) in Self::pieces(character).enumerate() {
          lengths.push(length);
          edges.push(if piece == 0 {
            (left, right)
          }
          else {
            (trailing, trailing)
          });
        }
      }

      let bounds = Self::bounds(line, &edges);
      let leading = |(left, right): (f32, f32)| {
        if right_to_left {
          bounds.origin.x + bounds.size.width - right
        }
        else {
          left - bounds.origin.x
        }
      };
      let locate = |position: Position| {
        Self::holds(line, &range, position, at == last).then(|| {
          let cluster = index.grapheme_at_byte(position.index, Round::Down) - first;

          AccessTextPosition::new(
            number,
            starts.get(cluster).copied().unwrap_or(lengths.len()),
          )
        })
      };

      anchor = anchor.or_else(|| locate(editor.anchor()));
      focus = focus.or_else(|| locate(editor.focus()));
      runs.push(AccessRun {
        text: shown,
        character_lengths: lengths,
        character_positions: edges.iter().copied().map(leading).collect(),
        character_widths: edges.iter().map(|(left, right)| right - left).collect(),
        word_starts: words,
        bounds,
        right_to_left,
      });
    }

    let end = AccessTextPosition::new(
      u16::try_from(runs.len().saturating_sub(1)).unwrap_or(u16::MAX),
      runs.last().map_or(0, |run| run.character_lengths.len()),
    );

    Self {
      runs,
      anchor: anchor.unwrap_or(end),
      focus: focus.unwrap_or(end),
    }
  }

  /// The byte lengths of the characters `cluster` is reported as: one, or where it is longer
  /// than a character's 255 bytes, as many as its code points fill in turn.
  fn pieces(cluster: &str) -> impl Iterator<Item = u8> {
    let mut chars = cluster.chars().peekable();

    std::iter::from_fn(move || {
      let mut length = 0_u8;

      while let Some(longer) = chars.peek().and_then(|next| {
        u8::try_from(next.len_utf8())
          .ok()
          .and_then(|width| length.checked_add(width))
      }) {
        length = longer;
        chars.next();
      }

      (length > 0).then_some(length)
    })
  }

  /// The frame of `line`, whose characters span `edges`: from its content's start, or the
  /// first character's left where one stands before it, to the farther of its content's
  /// end and the last character's right.
  fn bounds(line: &LineInfo, edges: &[(f32, f32)]) -> Rect<f32> {
    let left = edges
      .iter()
      .fold(line.left, |left, (edge, _)| left.min(*edge));
    let right = edges
      .iter()
      .fold(line.left + line.width, |right, (_, edge)| right.max(*edge));

    Rect::new(
      Point::new(left, line.top),
      Size::new(right - left, line.height),
    )
  }

  /// Whether `position` stands on `line`, whose bytes are `range`: inside it, or at its
  /// end where it is the `last` line or the position leans back onto a line no newline
  /// ends.
  fn holds(line: &LineInfo, range: &Range<usize>, position: Position, last: bool) -> bool {
    range.contains(&position.index)
      || position.index == range.end
        && (last || position.affinity == Affinity::Upstream && !line.hard_break)
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    AccessRun,
    AccessText,
    AccessTextPosition,
    Edit,
    Point,
    Rect,
    Size,
    text::{
      ParagraphStyle,
      TextStyle,
      editing::Editor,
      grid::{ADVANCE, Grid, LINE},
    },
  };

  fn editor(text: &str, width: Option<f32>) -> Editor<Grid> {
    let mut editor = Editor::new(Grid, TextStyle::new(13.), ParagraphStyle::default());

    editor.set_multiline(true);
    editor.set_width(width);
    editor.write(&Edit::new(text));

    editor
  }

  fn texts(access: &AccessText) -> Vec<&str> {
    access.runs.iter().map(|run| run.text.as_str()).collect()
  }

  #[test]
  fn a_run_is_a_line_with_each_characters_length_place_and_width() {
    let access = AccessText::of(&editor("ab cd\ne\u{301}f", None));

    assert_eq!(texts(&access), ["ab cd\n", "e\u{301}f"]);

    let [first, second] = access.runs.as_slice()
    else {
      panic!("two runs: {access:?}");
    };

    assert_eq!(first.character_lengths, [1, 1, 1, 1, 1, 1]);
    assert_eq!(
      first.character_positions[..5],
      [0., ADVANCE, 2. * ADVANCE, 3. * ADVANCE, 4. * ADVANCE]
    );
    assert_eq!(first.character_widths[..5], [ADVANCE; 5]);
    assert_eq!(first.word_starts, [0, 3]);
    assert!(!first.right_to_left);
    assert_eq!(first.bounds.origin, Point::new(0., 0.));
    assert_eq!(first.bounds.size.height, LINE);
    assert!(first.bounds.size.width >= 5. * ADVANCE);

    assert_eq!(
      second.character_lengths,
      [3, 1],
      "a cluster is one character"
    );
    assert_eq!(second.character_positions, [0., ADVANCE]);
    assert_eq!(second.character_widths, [ADVANCE, ADVANCE]);
    assert_eq!(
      second.word_starts,
      [0],
      "a line after a newline starts a word"
    );
    assert_eq!(
      second.bounds,
      Rect::new(Point::new(0., LINE), Size::new(2. * ADVANCE, LINE))
    );
  }

  #[test]
  fn a_wrapped_line_starts_a_word_only_after_a_blank() {
    let access = AccessText::of(&editor("ab cd", Some(3. * ADVANCE)));

    assert_eq!(texts(&access), ["ab ", "cd"]);
    assert_eq!(access.runs[1].word_starts, [0]);

    let access = AccessText::of(&editor("abcdef", Some(3. * ADVANCE)));

    assert_eq!(texts(&access), ["abc", "def"]);
    assert_eq!(
      access.runs[1].word_starts, [0; 0],
      "a word broken across lines starts on the first"
    );
  }

  #[test]
  fn the_selection_is_a_line_and_a_character_on_it() {
    let mut entry = editor("ab cd\nef", None);

    entry.select_bytes(1, 7);

    let access = AccessText::of(&entry);

    assert_eq!(access.anchor, AccessTextPosition::new(0, 1));
    assert_eq!(access.focus, AccessTextPosition::new(1, 1));

    entry.select_bytes(8, 8);

    let access = AccessText::of(&entry);

    assert_eq!(
      access.focus,
      AccessTextPosition::new(1, 2),
      "the text's end is the last run's"
    );
    assert_eq!(access.anchor, access.focus);
  }

  #[test]
  fn the_caret_at_a_wrapped_lines_end_stands_where_its_affinity_leans() {
    let mut entry = editor("ab cd", Some(3. * ADVANCE));

    entry.select_bytes(3, 3);

    assert_eq!(
      AccessText::of(&entry).focus,
      AccessTextPosition::new(1, 0),
      "a caret leaning forward starts the next line"
    );
  }

  #[test]
  fn an_empty_entry_reports_one_empty_run_holding_the_caret() {
    let access = AccessText::of(&editor("", None));

    assert_eq!(
      access.runs,
      [AccessRun {
        bounds: Rect::new(Point::new(0., 0.), Size::new(0., LINE)),
        ..AccessRun::default()
      }]
    );
    assert_eq!(access.focus, AccessTextPosition::new(0, 0));
  }

  #[test]
  fn a_cluster_longer_than_a_character_is_reported_as_several() {
    let long = format!("e{}", "\u{301}".repeat(200));
    let mut entry = editor(&format!("{long}x"), None);

    entry.select_bytes(long.len(), long.len());

    let access = AccessText::of(&entry);
    let run = &access.runs[0];

    assert_eq!(run.character_lengths, [255, 146, 1]);
    assert_eq!(
      run
        .character_lengths
        .iter()
        .map(|length| usize::from(*length))
        .sum::<usize>(),
      run.text.len(),
      "the lengths add up to the text"
    );
    assert_eq!(run.character_positions, [0., ADVANCE, ADVANCE]);
    assert_eq!(run.character_widths, [ADVANCE, 0., ADVANCE]);
    assert_eq!(
      access.focus,
      AccessTextPosition::new(0, 2),
      "a place after the cluster is after all its characters"
    );
  }

  #[test]
  fn a_secure_entry_reports_its_marks() {
    let mut entry = editor("p a\u{301}ss", None);

    entry.set_secure(true);
    entry.select_bytes(0, 5);

    let access = AccessText::of(&entry);

    assert_eq!(texts(&access), ["\u{2022}".repeat(5)]);
    assert_eq!(access.runs[0].character_lengths, [3; 5]);
    assert_eq!(
      access.runs[0].word_starts,
      [0],
      "its word is the whole text"
    );
    assert_eq!(access.focus, AccessTextPosition::new(0, 3));
  }
}
