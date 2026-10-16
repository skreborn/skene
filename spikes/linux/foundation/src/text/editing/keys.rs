//! The keys bound to the editing commands where `Control` is the command modifier (Linux,
//! Windows and the terminal), named as a browser names them.

use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
  seam::report::KeyEvent,
  text::{
    Direction, TextBoundary, VisualMove,
    editing::{Command, Movement},
  },
};

/// What a key asks of a text entry.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Binding {
  /// A command.
  Command(Command),
  /// A paste: the clipboard's text, once read, as [`Command::Paste`].
  Paste,
}

impl Binding {
  /// The binding of `event` (a key press) in an entry of several lines where `multiline`:
  /// `None` for a key an entry leaves to its ancestors (`Tab`, `Enter` in one line).
  #[must_use]
  pub fn for_key(event: &KeyEvent, multiline: bool) -> Option<Self> {
    if !event.down {
      return None;
    }

    let modifiers = event.modifiers;
    let control = modifiers.control && !modifiers.alt && !modifiers.meta;
    let plain = !modifiers.control && !modifiers.alt && !modifiers.meta;
    let extend = modifiers.shift;
    let shortcut = event.key.to_lowercase();
    let command = |command| Some(Self::Command(command));
    let moving = |movement| Some(Self::Command(Command::Move { movement, extend }));

    match shortcut.as_str() {
      "a" if control && !extend => command(Command::SelectAll),
      "z" if control && !extend => command(Command::Undo),
      "z" if control => command(Command::Redo),
      "y" if control && !extend => command(Command::Redo),
      "c" if control && !extend => command(Command::Copy),
      "x" if control && !extend => command(Command::Cut),
      "v" if control && !extend => Some(Self::Paste),
      "insert" if control && !extend => command(Command::Copy),
      "insert" if plain && extend => Some(Self::Paste),
      "delete" if plain && extend => command(Command::Cut),
      "backspace" if control => command(Command::Delete(TextBoundary::Word, Direction::Backward)),
      "backspace" if plain => command(Command::Delete(TextBoundary::Grapheme, Direction::Backward)),
      "delete" if control => command(Command::Delete(TextBoundary::Word, Direction::Forward)),
      "delete" if plain => command(Command::Delete(TextBoundary::Grapheme, Direction::Forward)),
      "arrowleft" if control => moving(Movement::Visual(VisualMove::WordLeft)),
      "arrowright" if control => moving(Movement::Visual(VisualMove::WordRight)),
      "arrowleft" if plain => moving(Movement::Visual(VisualMove::Left)),
      "arrowright" if plain => moving(Movement::Visual(VisualMove::Right)),
      "arrowup" if plain => moving(Movement::Lines(-1)),
      "arrowdown" if plain => moving(Movement::Lines(1)),
      "home" if control => moving(Movement::Logical(TextBoundary::Text, Direction::Backward)),
      "end" if control => moving(Movement::Logical(TextBoundary::Text, Direction::Forward)),
      "home" if plain => moving(Movement::Visual(VisualMove::LineStart)),
      "end" if plain => moving(Movement::Visual(VisualMove::LineEnd)),
      "enter" if plain && multiline => command(Command::InsertNewline),
      _ if plain && event.key.graphemes(true).count() == 1 => {
        command(Command::Insert(event.key.clone()))
      }
      _ => None,
    }
  }
}
