//! Type-ahead: the characters typed in quick succession over a grouping control,
//! matched against its members' names.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  time::Duration,
};

use crate::{Key, KeyEvent, Realization, Timer};

/// How long after the last character a type-ahead buffer is kept.
pub const TYPE_AHEAD_RESET: Duration = Duration::from_millis(500);

/// The characters typed in quick succession over a grouping control, cleared once the
/// platform's clock has run [`TYPE_AHEAD_RESET`] past the last.
#[derive(Debug, Default)]
#[must_use]
pub struct TypeAhead {
  buffer: RefCell<String>,
  reset: Cell<Option<Timer>>,
}

impl TypeAhead {
  /// An empty buffer.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Feeds `event` into the buffer: the buffer so far for a printable key typed
  /// without `Ctrl`, `Alt` or `Meta`, a space only extending a buffer already begun;
  /// `None` for any other key. Each character schedules the reset afresh.
  pub fn typed<P: Realization>(self: &Rc<Self>, event: &KeyEvent) -> Option<String> {
    let modifiers = event.modifiers;

    if modifiers.ctrl || modifiers.alt || modifiers.meta {
      return None;
    }

    let Key::Character(text) = &event.key
    else {
      return None;
    };
    let mut characters = text.chars();
    let character = characters.next().filter(|_| characters.next().is_none())?;

    if character.is_control() || (character == ' ' && self.buffer.borrow().is_empty()) {
      return None;
    }

    if let Some(timer) = self.reset.take() {
      P::cancel(timer);
    }

    self.buffer.borrow_mut().push(character);

    let cleared = Rc::clone(self);

    self.reset.set(Some(P::schedule(
      TYPE_AHEAD_RESET,
      Box::new(move || {
        cleared.buffer.borrow_mut().clear();
        cleared.reset.set(None);
      }),
    )));

    Some(self.buffer.borrow().clone())
  }

  /// The index of the member `buffer` picks among `names`, given the member `current`
  /// holds focus or is chosen: for one character, repeated or not, the next name
  /// starting with it after `current`; for more, the first name starting with the
  /// whole buffer from `current` on. Both wrap, neither minds case, and a member with
  /// no name is passed over.
  #[must_use]
  pub fn matching(buffer: &str, names: &[Option<String>], current: Option<usize>) -> Option<usize> {
    if names.is_empty() {
      return None;
    }

    let lower = buffer.to_lowercase();
    let mut characters = lower.chars();
    let first = characters.next()?;
    let repeated = characters.all(|character| character == first);
    let needle = if repeated { first.to_string() } else { lower };
    let from = match current {
      Some(current) if repeated => current + 1,
      Some(current) => current,
      None => 0,
    };

    (0..names.len())
      .map(|offset| (from + offset) % names.len())
      .find(|index| {
        names[*index]
          .as_ref()
          .is_some_and(|name| name.to_lowercase().starts_with(&needle))
      })
  }
}

#[cfg(test)]
mod tests {
  use super::TypeAhead;

  fn names() -> Vec<Option<String>> {
    ["Apple", "Avocado", "Banana", "blueberry"]
      .map(|name| Some(name.to_owned()))
      .to_vec()
  }

  /// One character moves past the current member to the next name starting with it,
  /// wrapping; more characters match the whole buffer from the current member on.
  #[test]
  fn one_character_cycles_and_more_match_a_prefix() {
    let names = names();

    assert_eq!(TypeAhead::matching("a", &names, None), Some(0));
    assert_eq!(TypeAhead::matching("a", &names, Some(0)), Some(1));
    assert_eq!(TypeAhead::matching("a", &names, Some(1)), Some(0));
    assert_eq!(TypeAhead::matching("aa", &names, Some(0)), Some(1));
    assert_eq!(TypeAhead::matching("B", &names, Some(0)), Some(2));
    assert_eq!(TypeAhead::matching("bl", &names, Some(2)), Some(3));
    assert_eq!(TypeAhead::matching("av", &names, Some(0)), Some(1));
    assert_eq!(TypeAhead::matching("z", &names, Some(0)), None);
    assert_eq!(TypeAhead::matching("a", &[], None), None);
  }

  /// A member with no name is passed over, never matched.
  #[test]
  fn an_unnamed_member_is_passed_over() {
    let names = [None, Some("Apple".to_owned()), None];

    assert_eq!(TypeAhead::matching("a", &names, None), Some(1));
    assert_eq!(TypeAhead::matching("a", &names, Some(1)), Some(1));
    assert_eq!(TypeAhead::matching("b", &names, None), None);
  }
}
