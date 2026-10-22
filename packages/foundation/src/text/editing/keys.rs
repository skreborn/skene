//! The keys bound to the editing commands where `Control` is the command modifier (Linux,
//! Windows and the terminal), as a browser binds them in a text field.

use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
  Key,
  KeyEvent,
  NamedKey,
  text::{
    Direction,
    TextBoundary,
    VisualMove,
    editing::{Command, Movement},
  },
};

/// What a key asks of a text entry.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub enum Binding {
  /// A command.
  Command(Command),
  /// A paste: the clipboard's text, once read, as [`Command::Paste`].
  Paste,
}

impl Binding {
  /// The binding of `event`, a key press, in an entry of several lines where `multiline`:
  /// `None` for a key an entry leaves to its ancestors (`Tab`, `Enter` in one line, an arrow
  /// up or down in one line) or to an input method (a key it composes with).
  #[must_use]
  pub fn for_key(event: &KeyEvent, multiline: bool) -> Option<Self> {
    if event.composing {
      return None;
    }

    let modifiers = event.modifiers;
    let control = modifiers.ctrl && !modifiers.alt && !modifiers.meta;
    let plain = !modifiers.ctrl && !modifiers.alt && !modifiers.meta;
    let extend = modifiers.shift;
    let command = |command| Some(Self::Command(command));
    let moving = |movement| Some(Self::Command(Command::Move { movement, extend }));

    match &event.key {
      Key::Named(named) => match named {
        NamedKey::Insert if control && !extend => command(Command::Copy),
        NamedKey::Insert if plain && extend => Some(Self::Paste),
        NamedKey::Delete if plain && extend => command(Command::Cut),
        NamedKey::Backspace if control => {
          command(Command::Delete(TextBoundary::Word, Direction::Backward))
        }
        NamedKey::Backspace if plain => {
          command(Command::Delete(TextBoundary::Grapheme, Direction::Backward))
        }
        NamedKey::Delete if control => {
          command(Command::Delete(TextBoundary::Word, Direction::Forward))
        }
        NamedKey::Delete if plain => {
          command(Command::Delete(TextBoundary::Grapheme, Direction::Forward))
        }
        NamedKey::ArrowLeft if control => moving(Movement::Visual(VisualMove::WordLeft)),
        NamedKey::ArrowRight if control => moving(Movement::Visual(VisualMove::WordRight)),
        NamedKey::ArrowLeft if plain => moving(Movement::Visual(VisualMove::Left)),
        NamedKey::ArrowRight if plain => moving(Movement::Visual(VisualMove::Right)),
        NamedKey::ArrowUp if plain && multiline => moving(Movement::Lines(-1)),
        NamedKey::ArrowDown if plain && multiline => moving(Movement::Lines(1)),
        NamedKey::Home if control => {
          moving(Movement::Logical(TextBoundary::Text, Direction::Backward))
        }
        NamedKey::End if control => {
          moving(Movement::Logical(TextBoundary::Text, Direction::Forward))
        }
        NamedKey::Home if plain => moving(Movement::Visual(VisualMove::LineStart)),
        NamedKey::End if plain => moving(Movement::Visual(VisualMove::LineEnd)),
        NamedKey::Enter if plain && multiline => command(Command::InsertNewline),
        _ => None,
      },
      Key::Character(_) if control => match event.shortcut_key().as_ref() {
        "a" if !extend => command(Command::SelectAll),
        "z" if !extend => command(Command::Undo),
        "z" => command(Command::Redo),
        "y" if !extend => command(Command::Redo),
        "c" if !extend => command(Command::Copy),
        "x" if !extend => command(Command::Cut),
        "v" => Some(Self::Paste),
        _ => None,
      },
      Key::Character(text) if plain && text.graphemes(true).count() == 1 => {
        command(Command::Insert(text.clone()))
      }
      Key::Character(_) => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::Binding;
  use crate::{
    Code,
    Key,
    KeyEvent,
    Modifiers,
    NamedKey,
    text::{
      Direction,
      TextBoundary,
      VisualMove,
      editing::{Command, Movement},
    },
  };

  fn press(key: Key, modifiers: Modifiers) -> KeyEvent {
    KeyEvent {
      key,
      code: Code::Unidentified,
      modifiers,
      repeat: false,
      composing: false,
    }
  }

  const SHIFT: Modifiers = Modifiers {
    ctrl: false,
    shift: true,
    alt: false,
    meta: false,
  };
  const CTRL: Modifiers = Modifiers {
    ctrl: true,
    shift: false,
    alt: false,
    meta: false,
  };
  const CTRL_SHIFT: Modifiers = Modifiers {
    ctrl: true,
    shift: true,
    alt: false,
    meta: false,
  };

  fn named(key: NamedKey, modifiers: Modifiers, multiline: bool) -> Option<Binding> {
    Binding::for_key(&press(Key::Named(key), modifiers), multiline)
  }

  #[test]
  fn an_arrow_up_or_down_binds_only_in_an_entry_of_several_lines() {
    for key in [NamedKey::ArrowUp, NamedKey::ArrowDown] {
      assert_eq!(named(key, Modifiers::default(), false), None);
      assert_eq!(named(key, SHIFT, false), None);
    }

    assert_eq!(
      named(NamedKey::ArrowUp, Modifiers::default(), true),
      Some(Binding::Command(Command::Move {
        movement: Movement::Lines(-1),
        extend: false,
      }))
    );
    assert_eq!(
      named(NamedKey::ArrowDown, SHIFT, true),
      Some(Binding::Command(Command::Move {
        movement: Movement::Lines(1),
        extend: true,
      }))
    );
  }

  #[test]
  fn ctrl_shift_a_binds_nothing_and_ctrl_shift_v_pastes() {
    let event = press(Key::Character("A".into()), CTRL_SHIFT);

    assert_eq!(Binding::for_key(&event, false), None);
    assert_eq!(
      Binding::for_key(&press(Key::Character("a".into()), CTRL), false),
      Some(Binding::Command(Command::SelectAll))
    );
    assert_eq!(
      Binding::for_key(&press(Key::Character("V".into()), CTRL_SHIFT), false),
      Some(Binding::Paste),
      "Ctrl+Shift+V pastes as plain text, which a text entry's paste is"
    );
  }

  #[test]
  fn the_insert_and_delete_keys_reach_the_clipboard() {
    assert_eq!(
      named(NamedKey::Delete, SHIFT, false),
      Some(Binding::Command(Command::Cut)),
      "Shift+Delete cuts"
    );
    assert_eq!(
      named(NamedKey::Insert, SHIFT, false),
      Some(Binding::Paste),
      "Shift+Insert pastes"
    );
    assert_eq!(
      named(NamedKey::Insert, CTRL, false),
      Some(Binding::Command(Command::Copy)),
      "Ctrl+Insert copies"
    );
    assert_eq!(named(NamedKey::Insert, Modifiers::default(), false), None);
    assert_eq!(
      named(NamedKey::Delete, Modifiers::default(), false),
      Some(Binding::Command(Command::Delete(
        TextBoundary::Grapheme,
        Direction::Forward
      )))
    );
  }

  #[test]
  fn a_plain_key_of_one_cluster_inserts_it_and_a_composing_key_binds_nothing() {
    let typed = press(Key::Character("e\u{301}".into()), SHIFT);

    assert_eq!(
      Binding::for_key(&typed, false),
      Some(Binding::Command(Command::Insert("e\u{301}".into())))
    );
    assert_eq!(
      Binding::for_key(
        &press(Key::Character("ab".into()), Modifiers::default()),
        false
      ),
      None,
      "two clusters are no key's text"
    );

    let composing = KeyEvent {
      composing: true,
      ..press(Key::Named(NamedKey::ArrowLeft), Modifiers::default())
    };

    assert_eq!(Binding::for_key(&composing, false), None);
    assert_eq!(
      named(NamedKey::ArrowLeft, CTRL_SHIFT, false),
      Some(Binding::Command(Command::Move {
        movement: Movement::Visual(VisualMove::WordLeft),
        extend: true,
      }))
    );
  }

  #[test]
  fn a_shortcut_on_a_non_latin_layout_is_its_physical_key() {
    let event = KeyEvent {
      code: Code::KeyZ,
      ..press(Key::Character("\u{44f}".into()), CTRL)
    };

    assert_eq!(
      Binding::for_key(&event, false),
      Some(Binding::Command(Command::Undo))
    );
  }
}
