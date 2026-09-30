//! Selection inside a text entry: the value selected on keyboard focus, `Shift` and
//! the mouse extending it, a drag held past the edge autoscrolling, edits replacing
//! it, the clipboard keys, the edit menu, and the terminal's paste.

use std::time::Duration;

use skene_tui::{self as tui, TextEntry, prelude::*, testing};

use crate::common::{rgb, rows, white, with_platform};

/// The selection highlight the tests read back.
fn highlight() -> Color {
  Color::srgb(0.2, 0.35, 0.53)
}

/// Mounts a single-line field bound to `value` under a typography painting selections
/// in white on the highlight, at `width` cells, and focuses it by keyboard.
fn mount_field(value: Signal<String>, width: u32) {
  let _ = tui::mount(move || {
    Theme::new(
      Typography::default()
        .set_selection_ink(white())
        .set_selection_surface(highlight()),
    )
    .set_child_builder(move || {
      BasicTextField::new(TextEntry::new().bind_value(value))
        .set_width(width)
        .into_any()
    })
    .into_any()
    .build()
  });
  let _ = testing::render_to_text(20, 4);

  testing::trigger_key("Tab");
}

/// Keyboard focus selects a field's whole value, so typing replaces it; `ArrowRight`
/// collapses the selection to its end and `ArrowLeft` to its start.
#[test]
fn keyboard_focus_selects_the_value_and_typing_replaces_it() {
  with_platform(|| {
    let value = Signal::new(String::from("alpha"));

    mount_field(value, 10);

    let colors = testing::render_to_colors(20, 4);

    assert_eq!(colors[0], (Some(rgb(white())), Some(rgb(highlight()))));
    assert_eq!(colors[4].1, Some(rgb(highlight())));
    assert_eq!(colors[5].1, None, "the highlight ends with the value");

    testing::trigger_key("ArrowRight");
    testing::trigger_key("x");

    assert_eq!(value.get_untracked(), "alphax", "collapsed to the end");

    testing::trigger_key("Ctrl+a");
    testing::trigger_key("ArrowLeft");
    testing::trigger_key("y");

    assert_eq!(value.get_untracked(), "yalphax", "collapsed to the start");

    testing::trigger_key("Ctrl+a");
    testing::trigger_key("z");

    assert_eq!(value.get_untracked(), "z", "typing replaces the selection");
  });
}

/// `Shift` with the caret keys extends the selection—by character, by word, to the
/// ends—and `Backspace` removes what is selected.
#[test]
fn shift_extends_the_selection_and_backspace_removes_it() {
  with_platform(|| {
    let value = Signal::new(String::from("alpha beta gamma"));

    mount_field(value, 20);
    testing::trigger_key("End");
    testing::trigger_key("Shift+ArrowLeft");
    testing::trigger_key("Shift+ArrowLeft");
    testing::trigger_key("Backspace");

    assert_eq!(value.get_untracked(), "alpha beta gam");

    testing::trigger_key("Ctrl+Shift+ArrowLeft");
    testing::trigger_key("Delete");

    assert_eq!(value.get_untracked(), "alpha beta ");

    testing::trigger_key("Home");
    testing::trigger_key("Shift+ArrowRight");
    testing::trigger_key("Shift+End");
    testing::trigger_key("Shift+ArrowLeft");
    testing::trigger_key("Ctrl+Backspace");

    assert_eq!(
      value.get_untracked(),
      " ",
      "a word removal takes the selection alone"
    );

    testing::trigger_key("a");
    testing::trigger_key("b");
    testing::trigger_key("Shift+Home");
    testing::trigger_key("Shift+ArrowRight");
    testing::trigger_key("x");

    assert_eq!(
      value.get_untracked(),
      "ax ",
      "shrinking the selection keeps extending from its anchor"
    );
  });
}

/// A drag across a field selects the cells it covers, a double click takes the word
/// under the pointer, a triple click the whole value.
#[test]
fn the_mouse_selects_by_drag_word_and_value() {
  with_platform(|| {
    let value = Signal::new(String::from("alpha beta"));

    mount_field(value, 12);
    testing::trigger_mouse_press(1, 0);
    testing::trigger_mouse_drag(4, 0);
    testing::trigger_mouse_release(4, 0);

    let colors = testing::render_to_colors(20, 4);

    assert_eq!(colors[0].1, None);
    assert_eq!(colors[1].1, Some(rgb(highlight())));
    assert_eq!(colors[3].1, Some(rgb(highlight())));
    assert_eq!(colors[4].1, None, "the drag end is exclusive");

    testing::trigger_key("x");

    assert_eq!(value.get_untracked(), "axa beta");

    testing::trigger_mouse_click(4, 0);
    testing::trigger_mouse_click(4, 0);
    testing::trigger_key("y");

    assert_eq!(
      value.get_untracked(),
      "axa y",
      "a double click takes the word"
    );

    for _ in 0..3 {
      testing::trigger_mouse_click(1, 0);
    }

    testing::trigger_key("z");

    assert_eq!(value.get_untracked(), "z", "a triple click takes the value");
  });
}

/// The clipboard keys inside an entry: `Ctrl+C` stages the selection, `Ctrl+X` stages
/// and removes it, `Ctrl+V` asks the loop to read the clipboard, and the terminal's
/// reply lands in the entry that asked; a reply nobody asked for lands nowhere.
#[test]
fn the_clipboard_keys_copy_cut_and_paste() {
  with_platform(|| {
    let value = Signal::new(String::from("alpha beta"));

    mount_field(value, 20);
    testing::trigger_key("End");
    testing::trigger_key("Ctrl+Shift+ArrowLeft");
    testing::trigger_key("Ctrl+c");

    assert_eq!(testing::take_copied().as_deref(), Some("beta"));
    assert_eq!(
      value.get_untracked(),
      "alpha beta",
      "a copy leaves the value"
    );

    testing::trigger_key("Ctrl+x");

    assert_eq!(testing::take_copied().as_deref(), Some("beta"));
    assert_eq!(value.get_untracked(), "alpha ");

    testing::trigger_clipboard_reply("stray");

    assert_eq!(
      value.get_untracked(),
      "alpha ",
      "a reply nobody asked for lands nowhere"
    );

    testing::trigger_key("Ctrl+v");

    assert!(testing::take_paste_requested());

    testing::trigger_clipboard_reply("gamma\ndelta");

    assert_eq!(
      value.get_untracked(),
      "alpha gammadelta",
      "a field drops the pasted line breaks"
    );

    testing::trigger_key("Ctrl+Shift+ArrowLeft");
    testing::trigger_paste("beta");

    assert_eq!(
      value.get_untracked(),
      "alpha beta",
      "a paste replaces the selection"
    );
  });
}

/// An editor keeps the pasted line breaks, and `Enter` over a selection replaces it
/// with the break.
#[test]
fn an_editor_pastes_line_breaks_and_breaks_over_a_selection() {
  with_platform(|| {
    let value = Signal::new(String::from("ab"));

    let _ = tui::mount(move || {
      BasicTextEditor::new(TextEntry::new().set_lines(3).bind_value(value))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 4);

    testing::trigger_key("Tab");
    testing::trigger_paste("cd\nef");

    assert_eq!(value.get_untracked(), "abcd\nef");

    testing::trigger_key("Shift+ArrowUp");
    testing::trigger_key("Enter");

    assert_eq!(
      value.get_untracked(),
      "ab\n",
      "the selection reached the column above the caret"
    );
  });
}

/// Right-clicking a focused entry opens the edit menu with `Cut`, `Copy` and `Paste`;
/// `Cut` removes the selection into the clipboard and `Paste` stages a read.
#[test]
fn the_edit_menu_cuts_copies_and_pastes() {
  with_platform(|| {
    let value = Signal::new(String::from("alpha beta"));

    mount_field(value, 20);
    testing::trigger_mouse_right_click(2, 0);

    let menu = testing::render_to_text(40, 6);

    assert!(
      menu.contains("Cut") && menu.contains("Copy") && menu.contains("Paste"),
      "the edit menu opened: {menu}"
    );
    assert_eq!(
      value.get_untracked(),
      "alpha beta",
      "a right press inside the selection keeps it"
    );

    testing::trigger_mouse_click(4, 1);

    assert_eq!(testing::take_copied().as_deref(), Some("alpha beta"));
    assert_eq!(value.get_untracked(), "", "Cut removed the value");

    testing::trigger_paste("x");
    testing::trigger_mouse_right_click(0, 0);
    testing::trigger_mouse_click(4, 3);

    assert!(testing::take_paste_requested(), "Paste staged a read");
    assert!(
      !testing::render_to_text(40, 6).contains("Cut"),
      "the menu closed"
    );

    testing::trigger_clipboard_reply("y");

    assert_eq!(
      value.get_untracked(),
      "yx",
      "the reply landed in the entry that asked, at the caret the right press placed"
    );
  });
}

/// Focus leaving an entry collapses its selection, and `Escape` collapses it in place;
/// a value written from outside collapses it too.
#[test]
fn blur_escape_and_writes_collapse_the_selection() {
  with_platform(|| {
    let value = Signal::new(String::from("alpha"));

    mount_field(value, 10);
    testing::trigger_key("Escape");

    assert_eq!(
      testing::render_to_colors(20, 4)[0].1,
      None,
      "Escape collapsed it"
    );

    testing::trigger_key("x");

    assert_eq!(
      value.get_untracked(),
      "alphax",
      "the caret stayed at the end"
    );

    testing::trigger_key("Ctrl+a");
    testing::trigger_mouse_click(15, 3);

    assert_eq!(
      testing::render_to_colors(20, 4)[0].1,
      None,
      "the blur collapsed it"
    );

    testing::trigger_key("Tab");
    value.set(String::from("beta"));
    testing::trigger_key("y");

    assert_eq!(
      value.get_untracked(),
      "betay",
      "a write from outside collapses at the end"
    );
  });
}

/// Mounts `child` under the selection typography, at 20 x 4 cells, and focuses its
/// entry by keyboard.
fn mount_under_typography(child: impl FnOnce() -> AnyView + 'static) {
  let _ = tui::mount(move || {
    Theme::new(
      Typography::default()
        .set_selection_ink(white())
        .set_selection_surface(highlight()),
    )
    .set_child_builder(child)
    .into_any()
    .build()
  });
  let _ = testing::render_to_text(20, 4);

  testing::trigger_key("Tab");
}

/// A drag held past a field's end scrolls its window on the frame clock: the caret
/// follows to the edge, the selection grows, the pointer's distance past the edge sets
/// the pace, and the scroll stops at the value's end.
#[test]
fn a_drag_held_past_the_end_autoscrolls_the_field() {
  with_platform(|| {
    let value = Signal::new(String::from("abcdefghij"));

    mount_field(value, 5);
    testing::trigger_key("Home");

    assert_eq!(testing::render_to_text(20, 4), "abcde\n\n\n\n");

    testing::trigger_mouse_press(1, 0);
    testing::trigger_mouse_drag(5, 0);

    assert_eq!(
      testing::render_to_text(20, 4),
      "abcde\n\n\n\n",
      "the crossing itself moves nothing"
    );
    assert_eq!(
      testing::render_to_caret(20, 4),
      Some((4, 0)),
      "the caret sits at the edge"
    );

    testing::trigger_frame(Duration::from_millis(50));

    assert_eq!(
      testing::render_to_text(20, 4),
      "bcdef\n\n\n\n",
      "one interval scrolls by the overshoot"
    );

    let colors = testing::render_to_colors(20, 4);

    assert_eq!(colors[0].1, Some(rgb(highlight())));
    assert_eq!(colors[3].1, Some(rgb(highlight())));
    assert_eq!(
      colors[4].1, None,
      "the caret at the edge is the selection's end"
    );

    testing::trigger_mouse_drag(7, 0);
    testing::trigger_frame(Duration::from_millis(100));

    assert_eq!(
      testing::render_to_text(20, 4),
      "ghij\n\n\n\n",
      "farther past the edge scrolls faster, and stops at the end"
    );

    testing::trigger_mouse_release(7, 0);
    testing::trigger_key("x");

    assert_eq!(value.get_untracked(), "ax");
  });
}

/// A drag held before a field's start scrolls its window back to the value's start.
#[test]
fn a_drag_held_before_the_start_autoscrolls_the_field_back() {
  with_platform(|| {
    let value = Signal::new(String::from("abcdefghij"));

    mount_under_typography(move || {
      Row::new()
        .set_children((
          Text::new("> "),
          BasicTextField::new(TextEntry::new().bind_value(value)).set_width(5),
        ))
        .into_any()
    });

    assert_eq!(
      testing::render_to_text(20, 4),
      "> ghij\n\n\n\n",
      "focus shows the value's end"
    );

    testing::trigger_mouse_press(6, 0);
    testing::trigger_mouse_drag(0, 0);

    let _ = testing::render_to_text(20, 4);

    testing::trigger_frame(Duration::from_millis(50));

    assert_eq!(testing::render_to_text(20, 4), "> efghi\n\n\n\n");
    assert_eq!(testing::render_to_caret(20, 4), Some((2, 0)));

    testing::trigger_frame(Duration::from_millis(150));

    assert_eq!(
      testing::render_to_text(20, 4),
      "> abcde\n\n\n\n",
      "the scroll stops at the start"
    );

    testing::trigger_mouse_release(0, 0);
    testing::trigger_key("x");

    assert_eq!(
      value.get_untracked(),
      "x",
      "the selection reached the start"
    );
  });
}

/// A drag held above an editor scrolls its rows up on the frame clock, the caret
/// following on the top row.
#[test]
fn a_drag_held_above_autoscrolls_the_editor() {
  with_platform(|| {
    let value = Signal::new(String::from("a\nb\nc\nd\ne\nf"));

    mount_under_typography(move || {
      Column::new()
        .set_children((
          Text::new("head"),
          BasicTextEditor::new(TextEntry::new().set_lines(3).bind_value(value)),
        ))
        .into_any()
    });

    assert_eq!(
      rows(&testing::render_to_text(20, 4)),
      "head\nd\ne\nf\n",
      "focus shows the last rows"
    );

    testing::trigger_mouse_press(1, 3);
    testing::trigger_mouse_drag(0, 0);

    let _ = rows(&testing::render_to_text(20, 4));

    testing::trigger_frame(Duration::from_millis(50));

    assert_eq!(rows(&testing::render_to_text(20, 4)), "head\nc\nd\ne\n");
    assert_eq!(testing::render_to_caret(20, 4), Some((0, 1)));

    testing::trigger_frame(Duration::from_millis(100));

    assert_eq!(
      rows(&testing::render_to_text(20, 4)),
      "head\na\nb\nc\n",
      "the scroll stops at the first row"
    );

    testing::trigger_mouse_release(0, 0);
    testing::trigger_key("x");

    assert_eq!(
      value.get_untracked(),
      "x",
      "the selection reached the start"
    );
  });
}
