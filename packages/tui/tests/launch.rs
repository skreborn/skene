//! The launch configuration: which clipboard operations the app offers, and whether
//! it takes the mouse.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use skene_tui::{self as tui, Activate, Label, TextEntry, prelude::*};

use crate::common::{rgb, white, with_launch};

/// The selection highlight the tests read back.
fn highlight() -> Color {
  Color::srgb(0.2, 0.35, 0.53)
}

/// Mounts a selectable run over a field bound to `value`, under a typography painting
/// selections in white on the highlight, and focuses the field by keyboard.
fn mount_document(value: Signal<String>) {
  let _ = tui::mount(move || {
    Theme::new(
      Typography::default()
        .set_selection_ink(white())
        .set_selection_surface(highlight()),
    )
    .set_child_builder(move || {
      Column::new()
        .set_children((
          Text::new("alpha beta").set_selectable(true),
          RawTextField::new(TextEntry::new().bind_value(value)).set_width(20),
        ))
        .into_any()
    })
    .into_any()
    .build()
  });
  let _ = tui::render_to_text(20, 4);

  tui::trigger_key("Tab");
}

/// Without a clipboard, nothing in the document is selectable, no menu opens, the
/// clipboard keys are ordinary keys, and the terminal's own paste still inserts.
#[test]
fn no_clipboard_offers_no_selection_menu_or_key() {
  with_launch(tui::Launch::new().set_clipboard(Clipboard::NONE), || {
    let value = Signal::new(String::from("value"));

    mount_document(value);

    let colors = tui::render_to_colors(20, 4);

    assert_eq!(
      colors[20],
      (Some(rgb(white())), Some(rgb(highlight()))),
      "keyboard focus still selects the field's value"
    );

    tui::trigger_key("Ctrl+c");

    assert_eq!(tui::take_copied(), None, "Ctrl+C copies nothing");

    tui::trigger_key("Ctrl+x");

    assert_eq!(value.get_untracked(), "value", "Ctrl+X cuts nothing");

    tui::trigger_key("Ctrl+v");

    assert!(!tui::take_paste_requested(), "Ctrl+V reads nothing");

    tui::trigger_mouse_right_click(2, 1);

    assert!(
      !tui::render_to_text(40, 6).contains("Paste"),
      "no edit menu opens"
    );

    tui::trigger_paste("x");

    assert_eq!(
      value.get_untracked(),
      "x",
      "the terminal's own paste inserts"
    );

    tui::trigger_mouse_move(2, 0);

    assert_eq!(tui::current_cursor(), None, "the run shows no text cursor");

    tui::trigger_mouse_press(0, 0);
    tui::trigger_mouse_drag(4, 0);
    tui::trigger_mouse_release(4, 0);

    let colors = tui::render_to_colors(20, 4);

    assert!(
      colors[..10]
        .iter()
        .all(|(_, background)| background.is_none()),
      "a drag over the run selects nothing: {:?}\n{}",
      &colors[..10],
      tui::render_to_text(20, 4)
    );

    tui::trigger_mouse_right_click(2, 0);

    assert!(
      !tui::render_to_text(40, 6).contains("Copy"),
      "no copy menu opens"
    );
  });
}

/// Copy alone: `Ctrl+C` and `Ctrl+X` work, the edit menu lists `Cut` and `Copy`
/// without `Paste`, and `Ctrl+V` reads nothing.
#[test]
fn copy_alone_offers_cut_and_copy() {
  with_launch(tui::Launch::new().set_clipboard(Clipboard::COPY), || {
    let value = Signal::new(String::from("value"));

    mount_document(value);
    tui::trigger_key("Ctrl+c");

    assert_eq!(tui::take_copied().as_deref(), Some("value"));

    tui::trigger_key("Ctrl+v");

    assert!(!tui::take_paste_requested(), "Ctrl+V reads nothing");

    tui::trigger_mouse_right_click(2, 1);

    let menu = tui::render_to_text(40, 6);

    assert!(
      menu.contains("Cut") && menu.contains("Copy") && !menu.contains("Paste"),
      "the edit menu lists Cut and Copy alone: {menu}"
    );

    tui::trigger_key("Escape");
    tui::trigger_key("Ctrl+x");

    assert_eq!(tui::take_copied().as_deref(), Some("value"));
    assert_eq!(value.get_untracked(), "", "Ctrl+X cut the value");
  });
}

/// Paste alone: `Ctrl+V` reads, the edit menu lists `Paste` alone, and `Ctrl+C` and
/// `Ctrl+X` do nothing.
#[test]
fn paste_alone_offers_paste() {
  with_launch(tui::Launch::new().set_clipboard(Clipboard::PASTE), || {
    let value = Signal::new(String::from("value"));

    mount_document(value);
    tui::trigger_key("Ctrl+c");

    assert_eq!(tui::take_copied(), None, "Ctrl+C copies nothing");

    tui::trigger_key("Ctrl+x");

    assert_eq!(value.get_untracked(), "value", "Ctrl+X cuts nothing");

    tui::trigger_mouse_right_click(2, 1);

    let menu = tui::render_to_text(40, 6);

    assert!(
      menu.contains("Paste") && !menu.contains("Cut") && !menu.contains("Copy"),
      "the edit menu lists Paste alone: {menu}"
    );

    tui::trigger_key("Escape");
    tui::trigger_key("Ctrl+v");

    assert!(tui::take_paste_requested(), "Ctrl+V staged a read");

    tui::trigger_clipboard_reply("pasted");

    assert_eq!(value.get_untracked(), "pasted");
  });
}

/// With the mouse off, no mouse event acts: a click clicks nothing and hover shows no
/// pointer shape, while the keyboard still operates the controls.
#[test]
fn mouse_off_ignores_every_mouse_event() {
  with_launch(tui::Launch::new().set_mouse(false), || {
    let clicks = Signal::new(0);
    let _ = tui::mount(move || {
      RawButton::new(Activate::new())
        .handle_click(move || clicks.set(clicks.get() + 1))
        .set_label(Label::new(Text::new("Press")))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 4);

    tui::trigger_mouse_move(1, 0);

    assert_eq!(tui::current_cursor(), None);

    tui::trigger_mouse_click(1, 0);

    assert_eq!(clicks.get_untracked(), 0, "a click clicks nothing");

    tui::trigger_key("Tab");
    tui::trigger_key("Enter");

    assert_eq!(clicks.get_untracked(), 1, "the keyboard still clicks");
  });
}
