//! Text entry in cells: the value bound both ways, the caret and its keys, the window
//! over a long value or an editor's rows, the wheel over an editor, the terminal's own
//! cursor, and a handler that moves focus or changes the tree while an edit is decided.

use std::{cell::RefCell, rc::Rc};

use skene_terminal::{self as terminal, TextEntry, prelude::*, testing};

use crate::common::{colors_at, is_live, red, rgb, rows, white, with_platform};

/// Typing into a focused field flows through `bind_value` into the signal and back onto
/// the frame; the placeholder shows while the value is empty.
#[test]
fn input_binds_a_signal_both_ways() {
  with_platform(|| {
    let name = Signal::new(String::new());

    let _ = terminal::mount(|| {
      BasicTextField::new(TextEntry::new().set_placeholder("name").bind_value(name))
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

/// A derived placeholder shows while the value is empty and names the field, following
/// its signal.
#[test]
fn a_derived_placeholder_shows_and_names_the_field() {
  with_platform(|| {
    let hint = Signal::new("name".to_owned());

    let _ = terminal::mount(move || {
      BasicTextField::new(TextEntry::new().derive_placeholder(move || hint.get()))
        .into_any()
        .build()
    });

    assert!(testing::render_to_text(10, 1).starts_with("name"));
    assert!(testing::find_by_role(Role::TextInput, "name").is_some());

    hint.set("email".to_owned());

    assert!(testing::render_to_text(10, 1).starts_with("email"));
    assert!(testing::find_by_role(Role::TextInput, "email").is_some());
    assert!(testing::find_by_role(Role::TextInput, "name").is_none());
  });
}

/// The caret moves with arrows, `Home`, and `End`; edits insert and delete around it.
#[test]
fn caret_edits_insert_and_delete() {
  with_platform(|| {
    let name = Signal::new(String::new());

    let _ = terminal::mount(|| {
      BasicTextField::new(TextEntry::new().bind_value(name))
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

    let _ = terminal::mount(|| {
      BasicTextField::new(TextEntry::new().bind_value(name))
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

/// A word removal eats trailing whitespace, then one segment as Chromium's word
/// boundaries divide the text: a punctuation mark alone, a full stop between letters
/// included, or a whole word, an apostrophe between its letters and an underscore included.
#[test]
fn word_deletion_follows_chromiums_word_boundaries() {
  with_platform(|| {
    let name = Signal::new(String::from("say don't foo.bar_baz("));

    let _ = terminal::mount(|| {
      BasicTextField::new(TextEntry::new().bind_value(name))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("End");

    for expected in [
      "say don't foo.bar_baz",
      "say don't foo.",
      "say don't foo",
      "say don't ",
      "say ",
      "",
    ] {
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

    let _ = terminal::mount(|| {
      BasicTextField::new(TextEntry::new().bind_value(name))
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

    let _ = terminal::mount(|| {
      BasicTextEditor::new(TextEntry::new().set_lines(3).bind_value(bio))
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

    let _ = terminal::mount(|| {
      Column::new()
        .add_child(BasicTextEditor::new(
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

    let _ = terminal::mount(|| {
      Column::new()
        .add_child(BasicTextEditor::new(
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

    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          BasicTextField::new(TextEntry::new()).into_any(),
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

    let _ = terminal::mount(|| {
      Column::new()
        .add_child(
          BasicTextEditor::new(TextEntry::new().set_lines(3).bind_value(bio))
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

    let _ = terminal::mount(|| {
      Column::new()
        .add_child(BasicTextField::new(TextEntry::new().bind_value(name)).set_width(8))
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

    let _ = terminal::mount(|| {
      BasicTextField::new(TextEntry::new().bind_value(name))
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

    let _ = terminal::mount(|| {
      Row::new()
        .set_children((
          BasicTextEditor::new(TextEntry::new().set_lines(2).bind_value(bio)).set_height(3),
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

    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          BasicTextEditor::new(TextEntry::new().set_lines(4).bind_value(bio)),
          BasicTextField::new(TextEntry::new().bind_value(name)),
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

    let _ = terminal::mount(|| {
      BasicTextEditor::new(TextEntry::new().set_lines(3).bind_value(bio))
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
    let _ = terminal::mount(|| {
      Theme::new(Typography::default().set_ink(red()).set_weight(700.))
        .set_child_builder(|| {
          Column::new()
            .set_children((
              BasicTextField::new(TextEntry::new().set_value("hi")),
              BasicTextField::new(TextEntry::new().set_placeholder("name")),
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

    let _ = terminal::mount(|| {
      BasicTextField::new(TextEntry::new().bind_value(value))
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

/// A tab in an entry's value is one blank cell, in a field and an editor alike, and the
/// caret after it lands where the value shows.
#[test]
fn a_tab_in_an_entry_is_one_blank_cell() {
  with_platform(|| {
    let name = Signal::new("a\tb".to_owned());
    let bio = Signal::new("a\tb\nc".to_owned());

    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          BasicTextField::new(TextEntry::new().bind_value(name))
            .set_width(20)
            .into_any(),
          BasicTextEditor::new(TextEntry::new().set_lines(2).bind_value(bio)).into_any(),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(20, 3), "a b\na b\nc\n");

    testing::trigger_key("Tab");
    testing::trigger_key("End");

    assert_eq!(testing::render_to_caret(20, 3), Some((3, 0)));
  });
}

/// An edit its handler rejects shows the truth and leaves no undo step behind, after a
/// move has ended the run of typing as before it: the next undo returns to the state
/// before the run the handler accepted.
#[test]
fn a_rejected_edit_is_not_an_undo_step() {
  with_platform(|| {
    let name = Signal::new(String::new());

    let _ = terminal::mount(move || {
      BasicTextField::new(
        TextEntry::new()
          .derive_value(move || name.get())
          .handle_change(move |text| {
            if !text.contains('x') {
              name.set(text.to_owned());
            }
          }),
      )
      .into_any()
      .build()
    });
    let _ = testing::render_to_text(10, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("a");
    testing::trigger_key("b");
    testing::trigger_key("ArrowLeft");
    testing::trigger_key("ArrowRight");
    testing::trigger_key("x");

    assert_eq!(name.get_untracked(), "ab");
    assert_eq!(testing::render_to_text(10, 1), "ab\n");
    assert_eq!(testing::render_to_caret(10, 1), Some((2, 0)));

    testing::trigger_key("Ctrl+z");

    assert_eq!(
      name.get_untracked(),
      "",
      "the typed run undoes as one, the rejected edit no step of its own"
    );
    assert_eq!(testing::render_to_text(10, 1), "\n");

    testing::trigger_key("Ctrl+y");

    assert_eq!(name.get_untracked(), "ab");
  });
}

/// An edit its handler transforms shows the truth, the caret mapped through the change
/// from what was typed to what the truth holds.
#[test]
fn a_transformed_edit_shows_the_truth_with_the_caret_mapped() {
  with_platform(|| {
    let name = Signal::new(String::from("ab"));

    let _ = terminal::mount(move || {
      BasicTextField::new(
        TextEntry::new()
          .derive_value(move || name.get())
          .handle_change(move |text| name.set(text.replace(' ', ""))),
      )
      .into_any()
      .build()
    });
    let _ = testing::render_to_text(10, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("End");
    testing::trigger_key("ArrowLeft");
    testing::trigger_key(" ");

    assert_eq!(name.get_untracked(), "ab");
    assert_eq!(testing::render_to_text(10, 1), "ab\n");
    assert_eq!(
      testing::render_to_caret(10, 1),
      Some((1, 0)),
      "the caret typed past the removed blank maps back before it"
    );

    testing::trigger_key("x");

    assert_eq!(name.get_untracked(), "axb");
  });
}

/// A change handler that moves focus to another node runs while the entry's edit is
/// decided: an undo returning to the whole value selected moves focus on, the entry it
/// leaves collapses that selection once the edit has ended, nothing is held twice, and the
/// keys reach the entry focus moved to.
#[test]
fn a_handler_that_focuses_another_node_does_not_panic() {
  with_platform(|| {
    let name = Signal::new(String::from("abc"));
    let selection = Signal::new(skene_foundation::TextSelection::collapsed(0));
    let other = Signal::new(String::from("other"));
    let next = NodeHandle::new();

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicTextField::new(
            TextEntry::new()
              .bind_value(name)
              .bind_selection(selection)
              .handle_change(move |text| {
                name.set(text.to_owned());

                if text == "abc" {
                  next.focus();
                }
              }),
          )
          .into_any(),
          BasicTextField::new(TextEntry::new().bind_value(other))
            .bind_handle(next)
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 2);

    testing::trigger_key("Tab");
    testing::trigger_key("x");

    assert_eq!(name.get_untracked(), "x");

    testing::trigger_key("Ctrl+z");

    assert_eq!(name.get_untracked(), "abc");
    assert_eq!(testing::focused_node(), next.node(), "focus moved on");
    assert_eq!(
      selection.get_untracked(),
      skene_foundation::TextSelection::collapsed(3),
      "the entry focus left collapsed its selection once the edit ended"
    );

    testing::trigger_key("b");

    assert_eq!(
      other.get_untracked(),
      "bother",
      "the keys reach the entry focused from the script, its caret where it was"
    );
    assert_eq!(name.get_untracked(), "abc");
  });
}

/// A change handler that destroys the entry it runs for and relocates another, then
/// focuses the one it moved, runs while the edit is decided: the focused entry goes
/// with its focus, the moved one keeps its place in the order the list now has, focus
/// lands on it, and the keys reach it.
#[test]
fn a_handler_that_destroys_and_relocates_nodes_does_not_panic() {
  with_platform(|| {
    let order = Signal::new(vec!["a", "b", "c"]);
    let (first, last) = (NodeHandle::new(), NodeHandle::new());
    let log = Rc::new(RefCell::new(Vec::new()));

    let _ = terminal::mount({
      let log = Rc::clone(&log);

      move || {
        Column::new()
          .derive_children(
            move || order.get(),
            |key| *key,
            move |key| {
              let value = Signal::new(key.to_owned());
              let (focused, blurred) = (Rc::clone(&log), Rc::clone(&log));
              let field = BasicTextField::new(TextEntry::new().bind_value(value).handle_change(
                move |text| {
                  value.set(text.to_owned());

                  if key == "a" {
                    order.set(vec!["c", "b"]);
                    last.focus();
                  }
                },
              ))
              .handle_focus(move || focused.borrow_mut().push(format!("{key} focus")))
              .handle_blur(move || blurred.borrow_mut().push(format!("{key} blur")));

              match key {
                "a" => field.bind_handle(first),
                "c" => field.bind_handle(last),
                _ => field,
              }
            },
          )
          .into_any()
          .build()
      }
    });

    assert_eq!(rows(&testing::render_to_text(10, 3)), "a\nb\nc\n");

    let destroyed = first.node().expect("the first entry");

    testing::trigger_key("Tab");
    testing::trigger_key("x");

    assert_eq!(rows(&testing::render_to_text(10, 3)), "c\nb\n\n");
    assert!(!is_live(destroyed), "the edited entry went");
    assert_eq!(testing::focused_node(), last.node(), "focus moved on");
    assert_eq!(*log.borrow(), ["a focus", "a blur", "c focus"]);

    testing::trigger_key("y");

    assert_eq!(
      rows(&testing::render_to_text(10, 3)),
      "yc\nb\n\n",
      "the keys reach the moved entry"
    );
  });
}

/// An editor's caret at the end of a full row stands at the area's edge, past its last
/// cell, as it does at the end of a row it wrapped.
#[test]
fn a_caret_at_a_full_rows_end_stays_in_view() {
  with_platform(|| {
    let bio = Signal::new(String::new());

    let _ = terminal::mount(move || {
      Column::new()
        .add_child(BasicTextEditor::new(TextEntry::new().set_lines(3).bind_value(bio)).set_width(5))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 4);

    testing::trigger_key("Tab");

    for key in ["a", "b", "c", "d", "e"] {
      testing::trigger_key(key);
    }

    assert_eq!(testing::render_to_caret(20, 4), Some((5, 0)));

    testing::trigger_key("f");

    assert_eq!(rows(&testing::render_to_text(20, 4)), "abcde\nf\n\n\n");
    assert_eq!(testing::render_to_caret(20, 4), Some((1, 1)));

    testing::trigger_key("ArrowUp");
    testing::trigger_key("End");

    assert_eq!(testing::render_to_caret(20, 4), Some((5, 0)));
  });
}

/// `ArrowUp` and `ArrowDown` in a field of one line are no editing keys: they leave the
/// caret where it is and reach the field's ancestors.
#[test]
fn arrow_up_in_a_field_is_left_to_its_ancestors() {
  with_platform(|| {
    let name = Signal::new(String::from("abc"));
    let heard = Signal::new(Vec::<String>::new());

    let _ = terminal::mount(move || {
      Container::new()
        .handle_key_down(move |event| heard.update(|keys| keys.push(event.key.to_string())))
        .set_child(BasicTextField::new(TextEntry::new().bind_value(name)))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("End");
    testing::trigger_key("ArrowLeft");
    testing::trigger_key("ArrowUp");
    testing::trigger_key("ArrowDown");
    testing::trigger_key("x");

    assert_eq!(
      name.get_untracked(),
      "abxc",
      "the caret stayed where it was"
    );
    assert!(
      heard.get_untracked().ends_with(&[
        "ArrowUp".to_owned(),
        "ArrowDown".to_owned(),
        "x".to_owned()
      ]),
      "the ancestor heard the arrows: {:?}",
      heard.get_untracked()
    );
  });
}

/// An entry reports its text to assistive technology as the editing core lays it: a text
/// run a line, holding the value between them, and the selection as a place in a run at
/// each end; the outline reads the value alone.
#[test]
fn an_editor_reports_its_lines_as_text_runs_and_its_selection_in_them() {
  with_platform(|| {
    let value = Signal::new(String::new());

    let _ = terminal::mount(move || {
      BasicTextEditor::new(TextEntry::new().set_lines(3).bind_value(value))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 3);

    testing::trigger_key("Tab");

    for key in ["a", "b", "Enter", "c", "d", "e"] {
      testing::trigger_key(key);
    }

    testing::trigger_key("Shift+ArrowLeft");
    testing::trigger_key("Shift+ArrowLeft");

    let tree = testing::access_tree();
    let node = |id| {
      tree
        .nodes
        .iter()
        .find(|(held, _)| *held == id)
        .map(|(_, node)| node)
        .expect("the tree holds the node")
    };
    let (_, entry) = tree
      .nodes
      .iter()
      .find(|(_, node)| node.role() == Role::MultilineTextInput)
      .expect("the editor is in the tree");
    let runs: Vec<_> = entry
      .children()
      .iter()
      .copied()
      .filter(|id| node(*id).role() == Role::TextRun)
      .collect();
    let texts: Vec<_> = runs.iter().map(|id| node(*id).value().unwrap()).collect();

    assert_eq!(value.get_untracked(), "ab\ncde");
    assert_eq!(texts, ["ab\n", "cde"], "a run a line");
    assert_eq!(texts.concat(), value.get_untracked());
    assert_eq!(node(runs[1]).character_lengths(), [1, 1, 1]);
    assert_eq!(node(runs[1]).character_positions(), Some(&[0., 1., 2.][..]));
    assert_eq!(node(runs[1]).word_starts(), [0]);

    let selection = entry
      .text_selection()
      .expect("the editor reports its selection");

    assert_eq!(
      (selection.anchor.node, selection.anchor.character_index),
      (runs[1], 3)
    );
    assert_eq!(
      (selection.focus.node, selection.focus.character_index),
      (runs[1], 1)
    );

    testing::trigger_key("Shift+ArrowUp");

    let tree = testing::access_tree();
    let (_, entry) = tree
      .nodes
      .iter()
      .find(|(_, node)| node.role() == Role::MultilineTextInput)
      .expect("the editor is in the tree");
    let selection = entry
      .text_selection()
      .expect("the editor reports its selection");

    assert_eq!(
      (selection.focus.node, selection.focus.character_index),
      (runs[0], 1),
      "the caret moved up a line"
    );
    assert!(
      !testing::access_outline().contains("TextRun"),
      "{}",
      testing::access_outline()
    );
  });
}

/// A text entry is controlled: an edit its handler rejects leaves the frame showing the
/// bound value, one it accepts stands, and a disabled entry hears none.
#[test]
fn a_rejected_edit_leaves_the_platform_showing_the_bound_value() {
  with_platform(|| {
    let bound = Signal::new("12".to_owned());
    let disabled = Signal::new(false);

    let _ = terminal::mount(move || {
      BasicTextField::new(
        TextEntry::new()
          .derive_value(move || bound.get())
          .handle_change(move |value| {
            if value.chars().all(|character| character.is_ascii_digit()) {
              bound.set(value.to_owned());
            }
          }),
      )
      .derive_disabled(move || disabled.get())
      .into_any()
      .build()
    });
    let _ = testing::render_to_text(10, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("End");
    testing::trigger_key("a");

    assert_eq!(bound.get_untracked(), "12");
    assert_eq!(
      testing::render_to_text(10, 1),
      "12\n",
      "the rejected edit is undone"
    );

    testing::trigger_key("3");

    assert_eq!(bound.get_untracked(), "123");
    assert_eq!(testing::render_to_text(10, 1), "123\n");

    disabled.set(true);
    testing::trigger_key("4");

    assert_eq!(
      bound.get_untracked(),
      "123",
      "a disabled entry hears no edit"
    );
    assert_eq!(testing::render_to_text(10, 1), "123\n");
  });
}

/// A text entry's selection is controlled beside its value: the bound signal places it
/// where a reader finds it, clamped into the value, and a move the user makes writes
/// the signal, backward where it was made backward.
#[test]
fn a_text_entrys_selection_binds_both_ways() {
  with_platform(|| {
    let bound = Signal::new(TextSelection::new(1, 3));

    let _ = terminal::mount(move || {
      BasicTextField::new(TextEntry::new().set_value("hello").bind_selection(bound))
        .set_width(10)
        .into_any()
        .build()
    });
    let reported = || {
      let _ = testing::render_to_text(10, 1);
      let tree = testing::access_tree();
      let (_, entry) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::TextInput)
        .expect("the field is in the tree");
      let selection = entry
        .text_selection()
        .expect("the field reports its selection");

      (
        selection.anchor.character_index,
        selection.focus.character_index,
      )
    };

    assert_eq!(reported(), (1, 3));

    bound.set(TextSelection::collapsed(5));

    assert_eq!(reported(), (5, 5));

    testing::trigger_key("Tab");
    testing::trigger_key("End");
    testing::trigger_key("Shift+ArrowLeft");
    testing::trigger_key("Shift+ArrowLeft");

    assert_eq!(bound.get_untracked(), TextSelection::new(5, 3));
    assert_eq!(reported(), (5, 3));

    bound.set(TextSelection::new(2, 9));

    assert_eq!(
      reported(),
      (2, 5),
      "an index past the value clamps to its end"
    );
  });
}

/// A value written from its signal moves the caret it leaves free, and the selection's
/// handler hears the move as it hears the user's.
#[test]
fn a_written_value_reports_the_caret_it_moves() {
  with_platform(|| {
    let value = Signal::new("hello".to_owned());
    let heard = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&heard);

    let _ = terminal::mount(move || {
      BasicTextField::new(
        TextEntry::new()
          .bind_value(value)
          .handle_selection_change(move |selection| log.borrow_mut().push(selection)),
      )
      .set_width(10)
      .into_any()
      .build()
    });
    let _ = testing::render_to_text(10, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("End");
    heard.borrow_mut().clear();
    value.set("hi".to_owned());
    let _ = testing::render_to_text(10, 1);

    assert_eq!(
      heard.borrow().last(),
      Some(&TextSelection::collapsed(2)),
      "the caret clamped into the written value is reported"
    );
  });
}

/// A secure field draws one mark a cluster, a wide one and a combining sequence each
/// one mark of one cell, while its value stays the real text; derived, it draws its text
/// again once it is not secure.
#[test]
fn a_secure_field_draws_a_mark_a_cluster_and_keeps_its_value() {
  with_platform(|| {
    let value = Signal::new("日e\u{301}x".to_owned());
    let secure = Signal::new(true);

    let _ = terminal::mount(move || {
      BasicTextField::new(
        TextEntry::new()
          .derive_secure(move || secure.get())
          .bind_value(value),
      )
      .set_width(8)
      .into_any()
      .build()
    });

    assert_eq!(testing::render_to_styled(8, 1), "•••\n\n\n\n");

    testing::trigger_key("Tab");

    assert_eq!(
      testing::render_to_styled(8, 1),
      "•••\n\naaa\n\na reverse\n",
      "focus selects the value, a mark a cluster"
    );

    testing::trigger_key("End");
    testing::trigger_key("y");

    assert_eq!(
      value.get_untracked(),
      "日e\u{301}xy",
      "the value is the real text"
    );
    assert!(
      testing::render_to_text(8, 1) == "\u{2022}\u{2022}\u{2022}\u{2022}\n",
      "{}",
      testing::render_to_text(8, 1)
    );

    secure.set(false);

    assert!(
      testing::render_to_text(8, 1).starts_with("日e\u{301}xy"),
      "{}",
      testing::render_to_text(8, 1)
    );
  });
}

/// A secure field keeps its text from the clipboard as Chromium's password field does:
/// its context menu lists copying and cutting disabled and pasting live, and the copy key
/// copies nothing.
#[test]
fn a_secure_fields_menu_disables_copying_and_cutting() {
  with_platform(|| {
    let value = Signal::new("hunter2".to_owned());

    let _ = terminal::mount(move || {
      BasicTextField::new(TextEntry::new().set_secure(true).bind_value(value))
        .set_width(20)
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 8);

    testing::trigger_key("Tab");
    testing::trigger_key("Ctrl+c");

    assert_eq!(testing::take_copied(), None, "the copy key copies nothing");

    testing::trigger_mouse_right_click(2, 0);

    let items = testing::find_all_by_role(Role::MenuItem);
    let names: Vec<_> = items
      .iter()
      .map(|item| testing::accessible_name(*item).unwrap_or_default())
      .collect();
    let disabled: Vec<_> = items
      .iter()
      .map(|item| crate::common::read(*item).is_disabled())
      .collect();

    assert_eq!(names, ["Cut", "Copy", "Paste"]);
    assert_eq!(
      disabled,
      [true, true, false],
      "the selection is all the value"
    );
  });
}

/// A secure field reports itself as a password input, its value and its text runs a mark
/// a cluster.
#[test]
fn a_secure_fields_runs_read_marks() {
  with_platform(|| {
    let _ = terminal::mount(move || {
      BasicTextField::new(TextEntry::new().set_secure(true).set_value("pä55"))
        .set_width(10)
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 1);

    let tree = testing::access_tree();
    let (_, entry) = tree
      .nodes
      .iter()
      .find(|(_, node)| node.role() == Role::PasswordInput)
      .expect("the field is a password input");
    let runs: Vec<_> = entry
      .children()
      .iter()
      .filter_map(|id| {
        tree
          .nodes
          .iter()
          .find(|(held, node)| held == id && node.role() == Role::TextRun)
          .and_then(|(_, node)| node.value().map(str::to_owned))
      })
      .collect();

    assert_eq!(runs, ["\u{2022}\u{2022}\u{2022}\u{2022}"]);
    assert_eq!(entry.value(), Some("\u{2022}\u{2022}\u{2022}\u{2022}"));
  });
}
