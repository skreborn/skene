//! Selection inside a text entry: the value selected on keyboard focus, `Shift` and
//! the mouse extending it, edits replacing it, the clipboard keys, the edit menu, and
//! the terminal's paste.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use skene_tui::{self as tui, TextEntry, prelude::*};

use crate::common::{rgb, white, with_driver};

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
      RawTextField::new(TextEntry::new().bind_value(value))
        .set_width(width)
        .into_any()
    })
    .into_any()
    .build()
  });
  let _ = tui::render_to_text(20, 4);

  tui::trigger_key("Tab");
}

/// Keyboard focus selects a field's whole value, so typing replaces it; `ArrowRight`
/// collapses the selection to its end and `ArrowLeft` to its start.
#[test]
fn keyboard_focus_selects_the_value_and_typing_replaces_it() {
  with_driver(|| {
    let value = Signal::new(String::from("alpha"));

    mount_field(value, 10);

    let colors = tui::render_to_colors(20, 4);

    assert_eq!(colors[0], (Some(rgb(white())), Some(rgb(highlight()))));
    assert_eq!(colors[4].1, Some(rgb(highlight())));
    assert_eq!(colors[5].1, None, "the highlight ends with the value");

    tui::trigger_key("ArrowRight");
    tui::trigger_key("x");

    assert_eq!(value.get_untracked(), "alphax", "collapsed to the end");

    tui::trigger_key("Ctrl+a");
    tui::trigger_key("ArrowLeft");
    tui::trigger_key("y");

    assert_eq!(value.get_untracked(), "yalphax", "collapsed to the start");

    tui::trigger_key("Ctrl+a");
    tui::trigger_key("z");

    assert_eq!(value.get_untracked(), "z", "typing replaces the selection");
  });
}

/// `Shift` with the caret keys extends the selection—by character, by word, to the
/// ends—and `Backspace` removes what is selected.
#[test]
fn shift_extends_the_selection_and_backspace_removes_it() {
  with_driver(|| {
    let value = Signal::new(String::from("alpha beta gamma"));

    mount_field(value, 20);
    tui::trigger_key("End");
    tui::trigger_key("Shift+ArrowLeft");
    tui::trigger_key("Shift+ArrowLeft");
    tui::trigger_key("Backspace");

    assert_eq!(value.get_untracked(), "alpha beta gam");

    tui::trigger_key("Ctrl+Shift+ArrowLeft");
    tui::trigger_key("Delete");

    assert_eq!(value.get_untracked(), "alpha beta ");

    tui::trigger_key("Home");
    tui::trigger_key("Shift+ArrowRight");
    tui::trigger_key("Shift+End");
    tui::trigger_key("Shift+ArrowLeft");
    tui::trigger_key("Ctrl+Backspace");

    assert_eq!(
      value.get_untracked(),
      " ",
      "a word removal takes the selection alone"
    );

    tui::trigger_key("a");
    tui::trigger_key("b");
    tui::trigger_key("Shift+Home");
    tui::trigger_key("Shift+ArrowRight");
    tui::trigger_key("x");

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
  with_driver(|| {
    let value = Signal::new(String::from("alpha beta"));

    mount_field(value, 12);
    tui::trigger_mouse_press(1, 0);
    tui::trigger_mouse_drag(4, 0);
    tui::trigger_mouse_release(4, 0);

    let colors = tui::render_to_colors(20, 4);

    assert_eq!(colors[0].1, None);
    assert_eq!(colors[1].1, Some(rgb(highlight())));
    assert_eq!(colors[3].1, Some(rgb(highlight())));
    assert_eq!(colors[4].1, None, "the drag end is exclusive");

    tui::trigger_key("x");

    assert_eq!(value.get_untracked(), "axa beta");

    tui::trigger_mouse_click(4, 0);
    tui::trigger_mouse_click(4, 0);
    tui::trigger_key("y");

    assert_eq!(
      value.get_untracked(),
      "axa y",
      "a double click takes the word"
    );

    for _ in 0..3 {
      tui::trigger_mouse_click(1, 0);
    }

    tui::trigger_key("z");

    assert_eq!(value.get_untracked(), "z", "a triple click takes the value");
  });
}

/// The clipboard keys inside an entry: `Ctrl+C` stages the selection, `Ctrl+X` stages
/// and removes it, `Ctrl+V` asks the loop to read the clipboard where the terminal
/// answers one, and the terminal's paste lands in place of the selection.
#[test]
fn the_clipboard_keys_copy_cut_and_paste() {
  with_driver(|| {
    let value = Signal::new(String::from("alpha beta"));

    mount_field(value, 20);
    tui::trigger_key("End");
    tui::trigger_key("Ctrl+Shift+ArrowLeft");
    tui::trigger_key("Ctrl+c");

    assert_eq!(tui::take_copied().as_deref(), Some("beta"));
    assert_eq!(
      value.get_untracked(),
      "alpha beta",
      "a copy leaves the value"
    );

    tui::trigger_key("Ctrl+x");

    assert_eq!(tui::take_copied().as_deref(), Some("beta"));
    assert_eq!(value.get_untracked(), "alpha ");

    tui::trigger_key("Ctrl+v");

    assert!(
      !tui::take_paste_requested(),
      "no read is staged without terminal support"
    );

    tui::trigger_clipboard_read_support(true);
    tui::trigger_key("Ctrl+v");

    assert!(tui::take_paste_requested());

    tui::trigger_paste("gamma\ndelta");

    assert_eq!(
      value.get_untracked(),
      "alpha gammadelta",
      "a field drops the pasted line breaks"
    );

    tui::trigger_key("Ctrl+Shift+ArrowLeft");
    tui::trigger_paste("beta");

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
  with_driver(|| {
    let value = Signal::new(String::from("ab"));

    let _ = tui::mount(move || {
      RawTextEditor::new(TextEntry::new().set_lines(3).bind_value(value))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 4);

    tui::trigger_key("Tab");
    tui::trigger_paste("cd\nef");

    assert_eq!(value.get_untracked(), "abcd\nef");

    tui::trigger_key("Shift+ArrowUp");
    tui::trigger_key("Enter");

    assert_eq!(
      value.get_untracked(),
      "ab\n",
      "the selection reached the column above the caret"
    );
  });
}

/// Right-clicking a focused entry opens the edit menu with `Cut`, `Copy` and `Paste`;
/// `Cut` removes the selection into the clipboard, `Paste` stages a read, and the
/// items the terminal cannot serve present disabled with a hint.
#[test]
fn the_edit_menu_cuts_copies_and_pastes() {
  with_driver(|| {
    let value = Signal::new(String::from("alpha beta"));

    mount_field(value, 20);
    tui::trigger_clipboard_read_support(true);
    tui::trigger_mouse_right_click(2, 0);

    let menu = tui::render_to_text(40, 6);

    assert!(
      menu.contains("Cut") && menu.contains("Copy") && menu.contains("Paste"),
      "the edit menu opened: {menu}"
    );
    assert_eq!(
      value.get_untracked(),
      "alpha beta",
      "a right press inside the selection keeps it"
    );

    tui::trigger_mouse_click(4, 1);

    assert_eq!(tui::take_copied().as_deref(), Some("alpha beta"));
    assert_eq!(value.get_untracked(), "", "Cut removed the value");

    tui::trigger_paste("x");
    tui::trigger_mouse_right_click(0, 0);
    tui::trigger_mouse_click(4, 3);

    assert!(tui::take_paste_requested(), "Paste staged a read");
    assert!(
      !tui::render_to_text(40, 6).contains("Cut"),
      "the menu closed"
    );

    tui::trigger_clipboard_read_support(false);
    tui::trigger_mouse_right_click(0, 0);

    let disabled = tui::render_to_text(40, 6);

    assert!(
      disabled.contains("paste with the terminal's key"),
      "the hint presents: {disabled}"
    );

    tui::trigger_mouse_click(4, 3);

    assert!(
      !tui::take_paste_requested(),
      "a disabled Paste stages nothing"
    );
    assert!(
      tui::render_to_text(40, 6).contains("Paste"),
      "a disabled item holds the menu open"
    );

    tui::trigger_key("Escape");
    tui::trigger_mouse_click(30, 3);
    tui::trigger_mouse_right_click(2, 0);
    tui::trigger_key("q");

    assert_eq!(
      value.get_untracked(),
      "xq",
      "a right press focusing the entry places its caret"
    );
  });
}

/// Focus leaving an entry collapses its selection, and `Escape` collapses it in place;
/// a value written from outside collapses it too.
#[test]
fn blur_escape_and_writes_collapse_the_selection() {
  with_driver(|| {
    let value = Signal::new(String::from("alpha"));

    mount_field(value, 10);
    tui::trigger_key("Escape");

    assert_eq!(
      tui::render_to_colors(20, 4)[0].1,
      None,
      "Escape collapsed it"
    );

    tui::trigger_key("x");

    assert_eq!(
      value.get_untracked(),
      "alphax",
      "the caret stayed at the end"
    );

    tui::trigger_key("Ctrl+a");
    tui::trigger_mouse_click(15, 3);

    assert_eq!(
      tui::render_to_colors(20, 4)[0].1,
      None,
      "the blur collapsed it"
    );

    tui::trigger_key("Tab");
    value.set(String::from("beta"));
    tui::trigger_key("y");

    assert_eq!(
      value.get_untracked(),
      "betay",
      "a write from outside collapses at the end"
    );
  });
}
