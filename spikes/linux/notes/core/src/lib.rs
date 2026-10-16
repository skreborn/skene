//! The notes screen's state, actions and colors, shared by every shell of the app and naming
//! no platform: what plain `#[test]`s check, with no display.

use skene_foundation::{Color, Definitions, Media, Signal, Token, TokenRef, TokenSet};

/// One note.
#[derive(Clone, PartialEq, Debug)]
pub struct Note {
  /// Unique among the notes.
  pub id: u32,
  /// The title.
  pub title: String,
  /// Whether the note is pinned to the top.
  pub pinned: bool,
}

/// The notes, the draft being typed and the selection.
#[derive(Clone, Copy)]
pub struct Notes {
  /// Every note, pinned first.
  pub items: Signal<Vec<Note>>,
  /// The title in the text field.
  pub draft: Signal<String>,
  /// The selected note's id.
  pub selected: Signal<Option<u32>>,
  next: Signal<u32>,
}

impl Notes {
  /// Two notes to start with, the pinned one first.
  pub fn new() -> Self {
    Self {
      items: Signal::new(vec![
        Note {
          id: 1,
          title: "Groceries".into(),
          pinned: true,
        },
        Note {
          id: 2,
          title: "Call Ada".into(),
          pinned: false,
        },
      ]),
      draft: Signal::new(String::new()),
      selected: Signal::new(None),
      next: Signal::new(3),
    }
  }

  /// Adds the draft as a note, selects it and clears the draft; an empty draft adds nothing.
  pub fn add(&self) {
    let title = self.draft.get_untracked().trim().to_owned();

    if title.is_empty() {
      return;
    }

    let id = self.next.get_untracked();

    self.next.set(id + 1);

    self.items.update(|items| {
      items.push(Note {
        id,
        title,
        pinned: false,
      });
    });

    self.draft.set(String::new());
    self.selected.set(Some(id));
  }

  /// Pins or unpins note `id`, keeping pinned notes first.
  pub fn set_pinned(&self, id: u32, pinned: bool) {
    self.items.update(|items| {
      if let Some(note) = items.iter_mut().find(|note| note.id == id) {
        note.pinned = pinned;
      }

      items.sort_by_key(|note| !note.pinned);
    });
  }

  /// Whether note `id` is pinned.
  pub fn is_pinned(&self, id: u32) -> bool {
    self
      .items
      .with(|items| items.iter().any(|note| note.id == id && note.pinned))
  }

  /// Selects note `id`.
  pub fn select(&self, id: u32) {
    self.selected.set(Some(id));
  }

  /// The selected note, if any.
  pub fn selected_note(&self) -> Option<Note> {
    let id = self.selected.get()?;

    self
      .items
      .with(|items| items.iter().find(|note| note.id == id).cloned())
  }
}

impl Default for Notes {
  fn default() -> Self {
    Self::new()
  }
}

/// The reader's settings.
#[derive(Clone, Copy)]
pub struct Settings {
  /// Whether the app is dark whatever the reader's scheme; the reader's scheme where not.
  pub dark: Signal<bool>,
}

impl Settings {
  /// Following the reader's scheme, until the reader says otherwise.
  pub fn new() -> Self {
    Self {
      dark: Signal::new(false),
    }
  }
}

impl Default for Settings {
  fn default() -> Self {
    Self::new()
  }
}

/// The app's colors, each light with its dark variant in its definition.
pub struct Palette {
  background: Token<Color>,
  card: Token<Color>,
  selected: Token<Color>,
  ink: Token<Color>,
  muted: Token<Color>,
  accent: Token<Color>,
}

impl Palette {
  /// The page behind everything.
  pub const BACKGROUND: TokenRef<Color> = TokenRef::new("notes-background");
  /// A note's fill.
  pub const CARD: TokenRef<Color> = TokenRef::new("notes-card");
  /// The selected note's fill.
  pub const SELECTED: TokenRef<Color> = TokenRef::new("notes-selected");
  /// The text.
  pub const INK: TokenRef<Color> = TokenRef::new("notes-ink");
  /// Secondary text.
  pub const MUTED: TokenRef<Color> = TokenRef::new("notes-muted");
  /// The accent.
  pub const ACCENT: TokenRef<Color> = TokenRef::new("notes-accent");
}

impl Default for Palette {
  fn default() -> Self {
    let pair = |reference, light: u32, dark: u32| {
      Token::new(reference, Color::rgb(light)).when(Media::Dark, Color::rgb(dark))
    };

    Self {
      background: pair(Self::BACKGROUND, 0xf5f5f7, 0x1c1c1e),
      card: pair(Self::CARD, 0xffffff, 0x2c2c2e),
      selected: pair(Self::SELECTED, 0xe5e5ff, 0x3a3a5c),
      ink: pair(Self::INK, 0x1d1d1f, 0xf5f5f7),
      muted: pair(Self::MUTED, 0x6e6e73, 0xa1a1a6),
      accent: pair(Self::ACCENT, 0x5e5ce6, 0x7d7aff),
    }
  }
}

impl TokenSet for Palette {
  fn define(&self, definitions: &mut Definitions) {
    definitions
      .add(&self.background)
      .add(&self.card)
      .add(&self.selected)
      .add(&self.ink)
      .add(&self.muted)
      .add(&self.accent);
  }
}

#[cfg(test)]
mod tests {
  use super::Notes;

  fn titles(notes: &Notes) -> Vec<String> {
    notes
      .items
      .get()
      .into_iter()
      .map(|note| note.title)
      .collect()
  }

  #[test]
  fn adding_selects_the_new_note_and_clears_the_draft() {
    let notes = Notes::new();

    notes.draft.set("  Milk ".into());
    notes.add();

    assert_eq!(titles(&notes), ["Groceries", "Call Ada", "Milk"]);
    assert_eq!(notes.selected.get(), Some(3));
    assert_eq!(notes.draft.get(), "");
    assert_eq!(
      notes.selected_note().map(|note| note.title),
      Some("Milk".into())
    );
  }

  #[test]
  fn an_empty_draft_adds_nothing() {
    let notes = Notes::new();

    notes.draft.set("   ".into());
    notes.add();

    assert_eq!(titles(&notes), ["Groceries", "Call Ada"]);
    assert_eq!(notes.selected.get(), None);
  }

  #[test]
  fn pinned_notes_come_first() {
    let notes = Notes::new();

    notes.set_pinned(2, true);
    notes.set_pinned(1, false);

    assert_eq!(titles(&notes), ["Call Ada", "Groceries"]);
    assert!(notes.is_pinned(2));
    assert!(!notes.is_pinned(1));
  }
}
