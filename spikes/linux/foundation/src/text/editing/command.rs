//! What a reader asks of a text entry, by key, by menu or through an input method's
//! `perform`: the editing commands, independent of the keys bound to them.

use crate::text::{Direction, TextBoundary, VisualMove};

/// Where a caret moves.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Movement {
  /// On screen: by cluster or word left or right, or to a visual line's ends.
  Visual(VisualMove),
  /// In the text's order: to a unit's boundary in a direction.
  Logical(TextBoundary, Direction),
  /// By lines, down where positive, keeping the horizontal goal across consecutive moves;
  /// past the first or the last line, to the text's start or end.
  Lines(isize),
}

/// A command over a text entry.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Command {
  /// Moves the caret, extending the selection from its anchor where `extend`, collapsing it
  /// otherwise; a horizontal move without `extend` over a selection collapses it to the end
  /// it points at.
  Move {
    /// Where to.
    movement: Movement,
    /// Whether the selection extends.
    extend: bool,
  },
  /// Removes the selection, or the text from the caret to a unit's boundary in a direction.
  Delete(TextBoundary, Direction),
  /// Replaces the selection with text: typed text, one undo step with the text typed before
  /// it until the caret moves.
  Insert(String),
  /// Breaks the line, in an entry of several lines.
  InsertNewline,
  /// Swaps the clusters around the caret, the two before it at a line's end.
  Transpose,
  /// Selects the whole text.
  SelectAll,
  /// Puts the selection on the clipboard ([`Outcome::clipboard`]).
  Copy,
  /// Puts the selection on the clipboard and removes it.
  Cut,
  /// Replaces the selection with the clipboard's text.
  Paste(String),
  /// Returns to the state before the last undo step.
  Undo,
  /// Returns to the state the last undo left.
  Redo,
}

impl Command {
  /// The command as `Debug` writes it, the text it carries shown as `mark` per grapheme
  /// cluster: what a log of a secure entry shows.
  #[must_use]
  pub fn masked(&self, mark: char) -> String {
    let marks = |text: &str| -> String {
      std::iter::repeat_n(
        mark,
        unicode_segmentation::UnicodeSegmentation::graphemes(text, true).count(),
      )
      .collect()
    };

    match self {
      Self::Insert(text) => format!("{:?}", Self::Insert(marks(text))),
      Self::Paste(text) => format!("{:?}", Self::Paste(marks(text))),
      other => format!("{other:?}"),
    }
  }
}

/// What a command did.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
#[must_use]
pub struct Outcome {
  /// Whether the command applied: an entry handles it, so the event goes no further.
  pub handled: bool,
  /// The text the command puts on the clipboard.
  pub clipboard: Option<String>,
}

impl Outcome {
  /// A command that did nothing.
  pub const IGNORED: Self = Self {
    handled: false,
    clipboard: None,
  };
  /// A command that applied.
  pub const HANDLED: Self = Self {
    handled: true,
    clipboard: None,
  };

  /// A command that applied where `handled`.
  #[inline(always)]
  pub const fn handled(handled: bool) -> Self {
    Self {
      handled,
      clipboard: None,
    }
  }
}
