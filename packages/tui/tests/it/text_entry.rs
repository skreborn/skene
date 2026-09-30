//! Text entry in cells: the value bound both ways, the caret and its keys, the window
//! over a long value or an editor's rows, the wheel over an editor, and the terminal's
//! own cursor.

use skene_tui::{self as tui, TextEntry, prelude::*, testing};

use crate::common::{colors_at, red, rgb, rows, white, with_platform};

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

    assert!(testing::render_to_text(10, 1).starts_with("name"));

    testing::trigger_key("Tab");
    testing::trigger_key("h");
    testing::trigger_key("i");

    assert_eq!(name.get_untracked(), "hi");
    assert!(testing::render_to_text(10, 1).starts_with("hi"));

    name.set("set".to_owned());

    assert!(testing::render_to_text(10, 1).starts_with("set"));
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
    let _ = testing::render_to_text(20, 1);

    testing::trigger_key("Tab");

    for key in ["a", "b", "c"] {
      testing::trigger_key(key);
    }

    testing::trigger_key("ArrowLeft");
    testing::trigger_key("X");

    assert_eq!(name.get_untracked(), "abXc");

    testing::trigger_key("Home");
    testing::trigger_key("Delete");

    assert_eq!(name.get_untracked(), "bXc");

    testing::trigger_key("End");
    testing::trigger_key("Backspace");

    assert_eq!(name.get_untracked(), "bX");

    // `Ctrl+H` is `0x08`, the legacy byte of `Ctrl+Backspace`: one character deletes.
    testing::trigger_key("Ctrl+h");

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
    let _ = testing::render_to_text(20, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("End");
    testing::trigger_key("Ctrl+ArrowLeft");
    testing::trigger_key("Ctrl+Backspace");

    assert_eq!(name.get_untracked(), "alpha gamma");

    testing::trigger_key("Ctrl+ArrowLeft");
    testing::trigger_key("Ctrl+ArrowRight");
    testing::trigger_key("Ctrl+Delete");

    assert_eq!(name.get_untracked(), "alpha");

    // `Ctrl+W` deletes the word too, for terminals whose keyboard encoding cannot
    // express `Ctrl+Backspace`.
    testing::trigger_key("Ctrl+w");

    assert_eq!(name.get_untracked(), "");
  });
}

/// A word removal eats trailing whitespace, then one segment as Unicode word
/// boundaries divide the text: a punctuation mark alone, or a whole word, an apostrophe
/// or a dot between its letters and an underscore included.
#[test]
fn word_deletion_follows_unicode_word_boundaries() {
  with_platform(|| {
    let name = Signal::new(String::from("say don't foo.bar_baz("));

    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().bind_value(name))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("End");

    for expected in ["say don't foo.bar_baz", "say don't ", "say ", ""] {
      testing::trigger_key("Ctrl+Backspace");

      assert_eq!(name.get_untracked(), expected);
    }
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
    let _ = testing::render_to_text(10, 1);

    testing::trigger_key("Tab");

    for key in ["a", "b", "c", "d", "e", "f", "g", "h"] {
      testing::trigger_key(key);
    }

    assert_eq!(testing::render_to_text(10, 1), "efgh\n");

    testing::trigger_key("Home");

    assert_eq!(testing::render_to_text(10, 1), "abcde\n");

    testing::trigger_mouse_click(2, 0);
    testing::trigger_key("X");

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
    let _ = testing::render_to_text(20, 4);

    testing::trigger_key("Tab");

    for key in ["a", "b", "Enter", "c", "d"] {
      testing::trigger_key(key);
    }

    assert_eq!(bio.get_untracked(), "ab\ncd");
    assert_eq!(testing::render_to_text(20, 4), "ab\ncd\n\n\n");
    assert_eq!(testing::render_to_caret(20, 4), Some((2, 1)));

    testing::trigger_key("ArrowUp");
    testing::trigger_key("X");

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
      Column::new()
        .add_child(RawTextEditor::new(
          TextEntry::new().set_lines(3).bind_value(bio),
        ))
        .into_any()
        .build()
    });
    let _ = rows(&testing::render_to_text(20, 4));

    testing::trigger_key("Tab");

    for key in ["a", "Enter", "b", "Enter", "c", "Enter", "d", "Enter", "e"] {
      testing::trigger_key(key);
    }

    assert_eq!(rows(&testing::render_to_text(20, 4)), "c\nd\ne\n\n");
    assert_eq!(testing::render_to_caret(20, 4), Some((1, 2)));

    for _ in 0..3 {
      testing::trigger_key("ArrowUp");
    }

    assert_eq!(rows(&testing::render_to_text(20, 4)), "b\nc\nd\n\n");
    assert_eq!(testing::render_to_caret(20, 4), Some((1, 0)));
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
        .add_child(RawTextEditor::new(
          TextEntry::new().set_lines(3).bind_value(bio),
        ))
        .into_any()
        .build()
    });

    assert_eq!(rows(&testing::render_to_text(20, 4)), "a\nb\nc\n\n");

    testing::trigger_mouse_scroll(1, 1, 0, 1);

    assert_eq!(rows(&testing::render_to_text(20, 4)), "b\nc\nd\n\n");

    testing::trigger_mouse_scroll(1, 1, 0, 5);

    assert_eq!(
      rows(&testing::render_to_text(20, 4)),
      "c\nd\ne\n\n",
      "clamped at the end"
    );

    testing::trigger_mouse_scroll(1, 1, 0, -5);

    assert_eq!(
      rows(&testing::render_to_text(20, 4)),
      "a\nb\nc\n\n",
      "clamped at the start"
    );

    testing::trigger_key("Tab");

    assert_eq!(
      rows(&testing::render_to_text(20, 4)),
      "c\nd\ne\n\n",
      "focus reveals the caret at the end"
    );
    assert_eq!(testing::render_to_caret(20, 4), Some((1, 2)));

    testing::trigger_mouse_scroll(1, 1, 0, -5);

    assert_eq!(rows(&testing::render_to_text(20, 4)), "a\nb\nc\n\n");
    assert_eq!(
      testing::render_to_caret(20, 4),
      None,
      "the caret hides while scrolled out"
    );

    testing::trigger_key("x");

    assert_eq!(bio.get_untracked(), "a\nb\nc\nd\nex");
    assert_eq!(
      rows(&testing::render_to_text(20, 4)),
      "c\nd\nex\n\n",
      "an edit reveals the caret"
    );
    assert_eq!(testing::render_to_caret(20, 4), Some((2, 2)));
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

    assert_eq!(testing::render_to_caret(20, 4), None);

    testing::trigger_key("Tab");

    assert_eq!(testing::render_to_caret(20, 4), Some((0, 0)));

    testing::trigger_key("a");
    testing::trigger_key("b");

    assert_eq!(
      testing::render_to_caret(20, 4),
      Some((2, 0)),
      "the caret follows the typed value"
    );

    open.set(true);

    assert_eq!(
      testing::render_to_caret(20, 4),
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
        .add_child(
          RawTextEditor::new(TextEntry::new().set_lines(3).bind_value(bio))
            .set_width(10)
            .set_border_width(Inset::all(1))
            .set_border_color(white()),
        )
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(12, 5);
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

    testing::trigger_mouse_press(9, 1);
    testing::trigger_mouse_drag(9, 3);
    testing::trigger_mouse_release(9, 3);

    let scrolled = testing::render_to_text(12, 5);

    assert!(
      scrolled.contains("│d") && !scrolled.contains("│a"),
      "the thumb drag scrolled to the end: {scrolled}"
    );

    bio.set("a\nb".to_owned());

    assert!(
      !testing::render_to_text(12, 5).contains('█'),
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
        .add_child(RawTextField::new(TextEntry::new().bind_value(name)).set_width(8))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(8, 1), "abcdefgh\n");

    testing::trigger_mouse_scroll(2, 0, 3, 0);

    assert_eq!(testing::render_to_text(8, 1), "defghijk\n");

    testing::trigger_mouse_scroll(2, 0, 30, 0);

    assert_eq!(
      testing::render_to_text(8, 1),
      "jklmnop\n",
      "clamped at the end"
    );

    testing::trigger_mouse_scroll(2, 0, 0, 3);

    assert_eq!(
      testing::render_to_text(8, 1),
      "jklmnop\n",
      "the vertical wheel passes"
    );

    testing::trigger_mouse_scroll(2, 0, -30, 0);

    assert_eq!(
      testing::render_to_text(8, 1),
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
    let _ = testing::render_to_text(20, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("Ctrl+z");

    assert_eq!(name.get_untracked(), "x", "the initial value is no step");

    testing::trigger_key("Backspace");

    for key in ["a", "b", "c"] {
      testing::trigger_key(key);
    }

    testing::trigger_key("ArrowLeft");
    testing::trigger_key("d");

    assert_eq!(name.get_untracked(), "abdc");

    testing::trigger_key("Ctrl+Z");

    assert_eq!(
      name.get_untracked(),
      "abc",
      "the character after the move undoes alone, Caps Lock or not"
    );

    testing::trigger_key("Ctrl+y");
    testing::trigger_key("Ctrl+z");

    assert_eq!(
      name.get_untracked(),
      "abc",
      "the character after the move undoes alone"
    );

    testing::trigger_key("Ctrl+z");

    assert_eq!(name.get_untracked(), "", "the typed run undoes as one");

    testing::trigger_key("Ctrl+z");

    assert_eq!(name.get_untracked(), "x", "the removal undoes");

    testing::trigger_key("Ctrl+z");

    assert_eq!(name.get_untracked(), "x", "nothing further to undo");

    testing::trigger_key("Ctrl+Shift+z");
    testing::trigger_key("Ctrl+Shift+Z");

    assert_eq!(name.get_untracked(), "abc");

    testing::trigger_key("Ctrl+y");

    assert_eq!(name.get_untracked(), "abdc");
    assert!(testing::render_to_text(20, 1).starts_with("abdc"));

    testing::trigger_key("End");
    testing::trigger_key("Backspace");
    testing::trigger_key("Backspace");

    assert_eq!(name.get_untracked(), "ab");

    testing::trigger_key("Ctrl+z");

    assert_eq!(
      name.get_untracked(),
      "abdc",
      "the removed run undoes as one"
    );

    testing::trigger_key("x");

    assert_eq!(name.get_untracked(), "abdcx");

    testing::trigger_key("Ctrl+y");

    assert_eq!(
      name.get_untracked(),
      "abdcx",
      "an edit drops the redo steps"
    );

    name.set("set".to_owned());

    assert!(testing::render_to_text(20, 1).starts_with("set"));

    testing::trigger_key("Ctrl+z");

    assert_eq!(
      name.get_untracked(),
      "abdcx",
      "a value from outside is a step of its own"
    );

    testing::trigger_key("Ctrl+y");

    assert_eq!(name.get_untracked(), "set");
  });
}

/// An editor shows as many rows as its laid-out height holds, whatever lines it asks
/// for, and is as wide as its longest line and the caret's cell past it.
#[test]
fn an_editor_shows_its_laid_out_rows_and_sizes_by_its_longest_line() {
  with_platform(|| {
    let bio = Signal::new("alpha\nbe\ngamma delta".to_owned());

    let _ = tui::mount(|| {
      Row::new()
        .set_children((
          RawTextEditor::new(TextEntry::new().set_lines(2).bind_value(bio)).set_height(3),
          Text::new("|"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(
      rows(&testing::render_to_text(20, 3)),
      "alpha       |\nbe\ngamma delta\n"
    );
  });
}

/// An editor holds its line breaks as line feeds, however they arrive, and a field
/// drops them.
#[test]
fn line_breaks_arrive_as_line_feeds_in_an_editor_and_drop_in_a_field() {
  with_platform(|| {
    let bio = Signal::new("a\r\nb\rc".to_owned());
    let name = Signal::new("x\ny".to_owned());

    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          RawTextEditor::new(TextEntry::new().set_lines(4).bind_value(bio)),
          RawTextField::new(TextEntry::new().bind_value(name)),
        ))
        .into_any()
        .build()
    });

    assert_eq!(rows(&testing::render_to_text(20, 5)), "a\nb\nc\n\nxy\n");

    testing::trigger_key("Tab");
    testing::trigger_paste("1\r\n2\r3");

    assert_eq!(bio.get_untracked(), "a\nb\nc1\n2\n3");

    testing::trigger_key("Tab");
    testing::trigger_paste("4\r\n5");

    assert_eq!(name.get_untracked(), "45");
  });
}

/// In an editor, `Home` and `End` go to the ends of the wrapped row and, under `Ctrl`,
/// of the value; the vertical arrows aim for the column the first of them left, across
/// shorter rows and wide clusters, and go to the value's ends past its first and last
/// rows.
#[test]
fn an_editors_keys_follow_its_rows_and_keep_their_column() {
  with_platform(|| {
    let bio = Signal::new("abcdef ghij".to_owned());

    let _ = tui::mount(|| {
      RawTextEditor::new(TextEntry::new().set_lines(3).bind_value(bio))
        .set_width(8)
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(8, 3);

    testing::trigger_key("Tab");
    testing::trigger_key("Home");
    testing::trigger_key("X");

    assert_eq!(bio.get_untracked(), "abcdef Xghij", "the row's start");

    testing::trigger_key("ArrowUp");
    testing::trigger_key("End");
    testing::trigger_key("Y");

    assert_eq!(bio.get_untracked(), "abcdefY Xghij", "the row's end");

    testing::trigger_key("Ctrl+End");
    testing::trigger_key("Z");
    testing::trigger_key("Ctrl+Home");
    testing::trigger_key("W");

    assert_eq!(bio.get_untracked(), "WabcdefY XghijZ", "the value's ends");

    bio.set("abcdef\nab\nabcdef".to_owned());
    testing::trigger_key("Ctrl+End");
    testing::trigger_key("ArrowUp");
    testing::trigger_key("ArrowUp");
    testing::trigger_key("V");

    assert_eq!(
      bio.get_untracked(),
      "abcdefV\nab\nabcdef",
      "the column held across a shorter row"
    );

    bio.set("\u{65e5}\u{672c}\u{8a9e}\nabcdef".to_owned());
    testing::trigger_key("Ctrl+End");

    for _ in 0..3 {
      testing::trigger_key("ArrowLeft");
    }

    testing::trigger_key("ArrowUp");
    testing::trigger_key("U");

    assert_eq!(
      bio.get_untracked(),
      "\u{65e5}\u{672c}U\u{8a9e}\nabcdef",
      "a column inside a wide cluster goes past it"
    );

    testing::trigger_key("ArrowUp");
    testing::trigger_key("T");
    testing::trigger_key("ArrowDown");
    testing::trigger_key("ArrowDown");
    testing::trigger_key("S");

    assert_eq!(
      bio.get_untracked(),
      "T\u{65e5}\u{672c}U\u{8a9e}\nabcdefS",
      "the value's ends past the first and last rows"
    );
  });
}

/// An entry's value is inked and weighted by the typography in force, and its
/// placeholder shows at half the ink's strength.
#[test]
fn an_entry_takes_the_typography_ink_and_weight() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Theme::new(Typography::default().set_ink(red()).set_weight(700.))
        .set_child_builder(|| {
          Column::new()
            .set_children((
              RawTextField::new(TextEntry::new().set_value("hi")),
              RawTextField::new(TextEntry::new().set_placeholder("name")),
            ))
            .into_any()
        })
        .into_any()
        .build()
    });

    assert_eq!(colors_at(10, 2, 0, 0).0, Some(rgb(red())));
    assert!(testing::render_to_bold(10, 2)[0]);
    assert_eq!(colors_at(10, 2, 0, 1).0, Some((128, 0, 0)));
  });
}

/// A read-only entry moves its caret and selects, and takes no edit: no typing, removal,
/// undo, cut or paste.
#[test]
fn a_read_only_entry_takes_no_edits() {
  with_platform(|| {
    let value = Signal::new("abc".to_owned());

    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().bind_value(value))
        .set_read_only(true)
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 1);

    testing::trigger_key("Tab");

    for key in ["x", "Backspace", "Delete", "Ctrl+x", "Ctrl+z"] {
      testing::trigger_key(key);
    }

    testing::trigger_paste("zz");

    assert_eq!(value.get_untracked(), "abc");

    testing::trigger_key("ArrowLeft");
    testing::trigger_key("Shift+End");
    testing::trigger_key("x");

    assert_eq!(value.get_untracked(), "abc");
    assert_eq!(testing::render_to_caret(10, 1), Some((3, 0)));
  });
}

/// A tab in an entry's value advances to its stop, in a field and an editor alike, and
/// the caret after it lands where the value shows.
#[test]
fn a_tab_in_an_entry_reaches_its_stop() {
  with_platform(|| {
    let name = Signal::new("a\tb".to_owned());
    let bio = Signal::new("a\tb\nc".to_owned());

    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          RawTextField::new(TextEntry::new().bind_value(name))
            .set_width(20)
            .into_any(),
          RawTextEditor::new(TextEntry::new().set_lines(2).bind_value(bio)).into_any(),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(20, 3), "a       b\na       b\nc\n");

    testing::trigger_key("Tab");
    testing::trigger_key("End");

    assert_eq!(testing::render_to_caret(20, 3), Some((9, 0)));
  });
}
