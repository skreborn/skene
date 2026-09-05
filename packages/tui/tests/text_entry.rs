//! Text entry in cells: the value bound both ways, the caret and its keys, the window
//! over a long value, and the terminal's own cursor.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use skene_tui::{self as tui, TextEntry, prelude::*};

use crate::common::with_driver;

/// Typing into a focused field flows through `bind_value` into the signal and back onto
/// the frame; the placeholder shows while the value is empty.
#[test]
fn input_binds_a_signal_both_ways() {
  with_driver(|| {
    let name = Signal::new(String::new());

    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().set_placeholder("name").bind_value(name))
        .into_any()
        .build()
    });

    assert!(tui::render_to_text(10, 1).starts_with("name"));

    tui::trigger_key("Tab");
    tui::trigger_key("h");
    tui::trigger_key("i");

    assert_eq!(name.get_untracked(), "hi");
    assert!(tui::render_to_text(10, 1).starts_with("hi"));

    name.set("set".to_owned());

    assert!(tui::render_to_text(10, 1).starts_with("set"));
  });
}

/// The caret moves with arrows, `Home`, and `End`; edits insert and delete around it.
#[test]
fn caret_edits_insert_and_delete() {
  with_driver(|| {
    let name = Signal::new(String::new());

    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().bind_value(name))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 1);

    tui::trigger_key("Tab");

    for key in ["a", "b", "c"] {
      tui::trigger_key(key);
    }

    tui::trigger_key("ArrowLeft");
    tui::trigger_key("X");

    assert_eq!(name.get_untracked(), "abXc");

    tui::trigger_key("Home");
    tui::trigger_key("Delete");

    assert_eq!(name.get_untracked(), "bXc");

    tui::trigger_key("End");
    tui::trigger_key("Backspace");

    assert_eq!(name.get_untracked(), "bX");

    // `Ctrl+H` is `0x08`, the legacy byte of `Ctrl+Backspace`: one character deletes.
    tui::trigger_key("Ctrl+h");

    assert_eq!(name.get_untracked(), "b");
  });
}

/// `Ctrl`-arrows jump the caret over whole words; `Ctrl+Backspace` and `Ctrl+Delete`
/// remove them.
#[test]
fn caret_jumps_and_deletes_by_word() {
  with_driver(|| {
    let name = Signal::new(String::from("alpha beta gamma"));

    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().bind_value(name))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 1);

    tui::trigger_key("Tab");
    tui::trigger_key("Ctrl+ArrowLeft");
    tui::trigger_key("Ctrl+Backspace");

    assert_eq!(name.get_untracked(), "alpha gamma");

    tui::trigger_key("Ctrl+ArrowLeft");
    tui::trigger_key("Ctrl+ArrowRight");
    tui::trigger_key("Ctrl+Delete");

    assert_eq!(name.get_untracked(), "alpha");

    // `Ctrl+W` deletes the word too, for terminals whose keyboard encoding cannot
    // express `Ctrl+Backspace`.
    tui::trigger_key("Ctrl+w");

    assert_eq!(name.get_untracked(), "");
  });
}

/// A word removal eats trailing whitespace, then one same-class run: punctuation runs
/// break like words of their own.
#[test]
fn word_deletion_breaks_on_character_class() {
  with_driver(|| {
    let name = Signal::new(String::from("foo.bar_baz("));

    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().bind_value(name))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 1);

    tui::trigger_key("Tab");
    tui::trigger_key("Ctrl+Backspace");

    assert_eq!(name.get_untracked(), "foo.bar_baz");

    tui::trigger_key("Ctrl+Backspace");

    assert_eq!(name.get_untracked(), "foo.");

    tui::trigger_key("Ctrl+Backspace");

    assert_eq!(name.get_untracked(), "foo");
  });
}

/// A long value slides its window to keep the caret visible; a click re-places the
/// caret at the clicked column.
#[test]
fn caret_windows_long_values_and_follows_clicks() {
  with_driver(|| {
    let name = Signal::new(String::new());

    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().bind_value(name))
        .set_width(5)
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(10, 1);

    tui::trigger_key("Tab");

    for key in ["a", "b", "c", "d", "e", "f", "g", "h"] {
      tui::trigger_key(key);
    }

    assert_eq!(tui::render_to_text(10, 1), "efgh\n");

    tui::trigger_key("Home");

    assert_eq!(tui::render_to_text(10, 1), "abcde\n");

    tui::trigger_mouse_click(2, 0);
    tui::trigger_key("X");

    assert_eq!(name.get_untracked(), "abXcdefgh");
  });
}

/// An editor breaks lines on `Enter`, moves between them with the vertical arrows, and
/// shows its lines at once.
#[test]
fn an_editor_breaks_lines_and_moves_between_them() {
  with_driver(|| {
    let bio = Signal::new(String::new());

    let _ = tui::mount(|| {
      RawTextEditor::new(TextEntry::new().set_lines(3).bind_value(bio))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 4);

    tui::trigger_key("Tab");

    for key in ["a", "b", "Enter", "c", "d"] {
      tui::trigger_key(key);
    }

    assert_eq!(bio.get_untracked(), "ab\ncd");
    assert_eq!(tui::render_to_text(20, 4), "ab\ncd\n\n\n");
    assert_eq!(tui::render_to_caret(20, 4), Some((2, 1)));

    tui::trigger_key("ArrowUp");
    tui::trigger_key("X");

    assert_eq!(bio.get_untracked(), "abX\ncd");
  });
}

/// The caret is the terminal's own bar cursor: reported at the focused entry's caret
/// cell, absent without focus, and hidden while a modal covers the entry.
#[test]
fn the_caret_reports_the_terminal_cursor_cell() {
  with_driver(|| {
    let open = Signal::new(false);

    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          RawTextField::new(TextEntry::new()).into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_child(Text::new("cover"))
            .into_any(),
        ))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_caret(20, 4), None);

    tui::trigger_key("Tab");

    assert_eq!(tui::render_to_caret(20, 4), Some((0, 0)));

    tui::trigger_key("a");
    tui::trigger_key("b");

    assert_eq!(
      tui::render_to_caret(20, 4),
      Some((2, 0)),
      "the caret follows the typed value"
    );

    open.set(true);

    assert_eq!(
      tui::render_to_caret(20, 4),
      None,
      "a covering modal hides the cursor"
    );
  });
}
