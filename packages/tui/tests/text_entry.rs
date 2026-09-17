//! Text entry in cells: the value bound both ways, the caret and its keys, the window
//! over a long value or an editor's rows, the wheel over an editor, and the terminal's
//! own cursor.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use skene_tui::{self as tui, TextEntry, prelude::*};

use crate::common::{rows, white, with_platform};

/// Typing into a focused field flows through `bind_value` into the signal and back onto
/// the frame; the placeholder shows while the value is empty.
#[test]
fn input_binds_a_signal_both_ways() {
  with_platform(|| {
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
  with_platform(|| {
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
  with_platform(|| {
    let name = Signal::new(String::from("alpha beta gamma"));

    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().bind_value(name))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 1);

    tui::trigger_key("Tab");
    tui::trigger_key("End");
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
  with_platform(|| {
    let name = Signal::new(String::from("foo.bar_baz("));

    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().bind_value(name))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 1);

    tui::trigger_key("Tab");
    tui::trigger_key("End");
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
  with_platform(|| {
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
  with_platform(|| {
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

/// An editor's window follows the caret through its wrapped rows: typing past the
/// last row scrolls the window down, and moving the caret above it scrolls it back.
#[test]
fn an_editor_windows_its_rows_around_the_caret() {
  with_platform(|| {
    let bio = Signal::new(String::new());

    let _ = tui::mount(|| {
      RawTextEditor::new(TextEntry::new().set_lines(3).bind_value(bio))
        .into_any()
        .build()
    });
    let _ = rows(&tui::render_to_text(20, 4));

    tui::trigger_key("Tab");

    for key in ["a", "Enter", "b", "Enter", "c", "Enter", "d", "Enter", "e"] {
      tui::trigger_key(key);
    }

    assert_eq!(rows(&tui::render_to_text(20, 4)), "c\nd\ne\n\n");
    assert_eq!(tui::render_to_caret(20, 4), Some((1, 2)));

    for _ in 0..3 {
      tui::trigger_key("ArrowUp");
    }

    assert_eq!(rows(&tui::render_to_text(20, 4)), "b\nc\nd\n\n");
    assert_eq!(tui::render_to_caret(20, 4), Some((1, 0)));
  });
}

/// The wheel scrolls an editor's window over its rows without moving the caret,
/// consuming at its ends; a caret scrolled out of view hides until focus or an edit
/// brings its row back.
#[test]
fn the_wheel_scrolls_an_editor_and_leaves_the_caret() {
  with_platform(|| {
    let bio = Signal::new("a\nb\nc\nd\ne".to_owned());

    let _ = tui::mount(|| {
      Column::new()
        .set_child(RawTextEditor::new(
          TextEntry::new().set_lines(3).bind_value(bio),
        ))
        .into_any()
        .build()
    });

    assert_eq!(rows(&tui::render_to_text(20, 4)), "a\nb\nc\n\n");

    tui::trigger_mouse_scroll(1, 1, 0, 1);

    assert_eq!(rows(&tui::render_to_text(20, 4)), "b\nc\nd\n\n");

    tui::trigger_mouse_scroll(1, 1, 0, 5);

    assert_eq!(
      rows(&tui::render_to_text(20, 4)),
      "c\nd\ne\n\n",
      "clamped at the end"
    );

    tui::trigger_mouse_scroll(1, 1, 0, -5);

    assert_eq!(
      rows(&tui::render_to_text(20, 4)),
      "a\nb\nc\n\n",
      "clamped at the start"
    );

    tui::trigger_key("Tab");

    assert_eq!(
      rows(&tui::render_to_text(20, 4)),
      "c\nd\ne\n\n",
      "focus reveals the caret at the end"
    );
    assert_eq!(tui::render_to_caret(20, 4), Some((1, 2)));

    tui::trigger_mouse_scroll(1, 1, 0, -5);

    assert_eq!(rows(&tui::render_to_text(20, 4)), "a\nb\nc\n\n");
    assert_eq!(
      tui::render_to_caret(20, 4),
      None,
      "the caret hides while scrolled out"
    );

    tui::trigger_key("x");

    assert_eq!(bio.get_untracked(), "a\nb\nc\nd\nex");
    assert_eq!(
      rows(&tui::render_to_text(20, 4)),
      "c\nd\nex\n\n",
      "an edit reveals the caret"
    );
    assert_eq!(tui::render_to_caret(20, 4), Some((2, 2)));
  });
}

/// The caret is the terminal's own bar cursor: reported at the focused entry's caret
/// cell, absent without focus, and hidden while a modal covers the entry.
#[test]
fn the_caret_reports_the_terminal_cursor_cell() {
  with_platform(|| {
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

/// An editor whose rows overflow its box shows a scrollbar down its last column, and
/// dragging the thumb scrolls the window.
#[test]
fn an_overflowing_editor_shows_a_scrollbar_that_drags() {
  with_platform(|| {
    let bio = Signal::new("a\nb\nc\nd\ne\nf".to_owned());

    let _ = tui::mount(|| {
      Column::new()
        .set_child(
          RawTextEditor::new(TextEntry::new().set_lines(3).bind_value(bio))
            .set_width(10)
            .set_border(Border::new(1, white())),
        )
        .into_any()
        .build()
    });

    let frame = tui::render_to_text(12, 5);
    let bar: Vec<char> = frame
      .lines()
      .skip(1)
      .take(3)
      .map(|line| line.chars().nth(9).unwrap_or(' '))
      .collect();

    assert_eq!(
      bar,
      ['█', '▄', ' '],
      "the thumb covers the first row and a half: {frame}"
    );
    assert!(frame.contains("│a"), "the first row shows: {frame}");

    tui::trigger_mouse_press(9, 1);
    tui::trigger_mouse_drag(9, 3);
    tui::trigger_mouse_release(9, 3);

    let scrolled = tui::render_to_text(12, 5);

    assert!(
      scrolled.contains("│d") && !scrolled.contains("│a"),
      "the thumb drag scrolled to the end: {scrolled}"
    );

    bio.set("a\nb".to_owned());

    assert!(
      !tui::render_to_text(12, 5).contains('█'),
      "a fitting value shows no scrollbar"
    );
  });
}

/// The horizontal wheel scrolls a field's window along a long value without moving
/// the caret, consuming at its ends; the vertical wheel passes through it.
#[test]
fn the_horizontal_wheel_scrolls_a_field() {
  with_platform(|| {
    let name = Signal::new("abcdefghijklmnop".to_owned());

    let _ = tui::mount(|| {
      Column::new()
        .set_child(RawTextField::new(TextEntry::new().bind_value(name)).set_width(8))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(8, 1), "abcdefgh\n");

    tui::trigger_mouse_scroll(2, 0, 3, 0);

    assert_eq!(tui::render_to_text(8, 1), "defghijk\n");

    tui::trigger_mouse_scroll(2, 0, 30, 0);

    assert_eq!(tui::render_to_text(8, 1), "jklmnop\n", "clamped at the end");

    tui::trigger_mouse_scroll(2, 0, 0, 3);

    assert_eq!(
      tui::render_to_text(8, 1),
      "jklmnop\n",
      "the vertical wheel passes"
    );

    tui::trigger_mouse_scroll(2, 0, -30, 0);

    assert_eq!(
      tui::render_to_text(8, 1),
      "abcdefgh\n",
      "clamped at the start"
    );
  });
}

/// `Ctrl+Z` undoes an edit and `Ctrl+Shift+Z` or `Ctrl+Y` redoes it, Caps Lock
/// changing nothing: a run of typed characters is one step, as is a run of removed
/// ones, a caret move ends a run, a value written from outside is a step of its own,
/// and the initial value is not.
#[test]
fn undo_and_redo_step_through_edits() {
  with_platform(|| {
    let name = Signal::new("x".to_owned());

    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().bind_value(name))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 1);

    tui::trigger_key("Tab");
    tui::trigger_key("Ctrl+z");

    assert_eq!(name.get_untracked(), "x", "the initial value is no step");

    tui::trigger_key("Backspace");

    for key in ["a", "b", "c"] {
      tui::trigger_key(key);
    }

    tui::trigger_key("ArrowLeft");
    tui::trigger_key("d");

    assert_eq!(name.get_untracked(), "abdc");

    tui::trigger_key("Ctrl+Z");

    assert_eq!(
      name.get_untracked(),
      "abc",
      "the character after the move undoes alone, Caps Lock or not"
    );

    tui::trigger_key("Ctrl+y");
    tui::trigger_key("Ctrl+z");

    assert_eq!(
      name.get_untracked(),
      "abc",
      "the character after the move undoes alone"
    );

    tui::trigger_key("Ctrl+z");

    assert_eq!(name.get_untracked(), "", "the typed run undoes as one");

    tui::trigger_key("Ctrl+z");

    assert_eq!(name.get_untracked(), "x", "the removal undoes");

    tui::trigger_key("Ctrl+z");

    assert_eq!(name.get_untracked(), "x", "nothing further to undo");

    tui::trigger_key("Ctrl+Shift+z");
    tui::trigger_key("Ctrl+Shift+Z");

    assert_eq!(name.get_untracked(), "abc");

    tui::trigger_key("Ctrl+y");

    assert_eq!(name.get_untracked(), "abdc");
    assert!(tui::render_to_text(20, 1).starts_with("abdc"));

    tui::trigger_key("End");
    tui::trigger_key("Backspace");
    tui::trigger_key("Backspace");

    assert_eq!(name.get_untracked(), "ab");

    tui::trigger_key("Ctrl+z");

    assert_eq!(
      name.get_untracked(),
      "abdc",
      "the removed run undoes as one"
    );

    tui::trigger_key("x");

    assert_eq!(name.get_untracked(), "abdcx");

    tui::trigger_key("Ctrl+y");

    assert_eq!(
      name.get_untracked(),
      "abdcx",
      "an edit drops the redo steps"
    );

    name.set("set".to_owned());

    assert!(tui::render_to_text(20, 1).starts_with("set"));

    tui::trigger_key("Ctrl+z");

    assert_eq!(
      name.get_untracked(),
      "abdcx",
      "a value from outside is a step of its own"
    );

    tui::trigger_key("Ctrl+y");

    assert_eq!(name.get_untracked(), "set");
  });
}
