//! Context menus in cells: a menu wrapping a view opens at the right-pressed cell in
//! place of the terminal's own, its keys walk the items, the menu key opens it at the
//! focused node, `Escape` closes it before touching the selection, and the standard
//! menu keeps an entry's selection while it acts on it.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use skene_tui::{self as tui, Activate, TextEntry, prelude::*};

use crate::common::with_platform;

/// A right press on the served view opens its menu at the press instead of the
/// terminal's own, an item lights under the pointer, a click on it runs it and closes
/// the menu, and a right press elsewhere still opens the terminal's copy menu.
#[test]
fn a_menu_on_a_view_takes_the_right_press_from_the_standard_one() {
  with_platform(|| {
    let chosen = Signal::new(String::new());
    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          ContextMenu::new()
            .set_items([
              MenuItem::new("Rename").handle_select(move || chosen.set("rename".to_owned())),
              MenuItem::separator(),
              MenuItem::new("Delete").handle_select(move || chosen.set("delete".to_owned())),
            ])
            .set_border(Border::new(1, Color::srgb(0.5, 0.5, 0.5)))
            .set_child(Text::new("note")),
          Text::new("selectable text").set_selectable(true),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(30, 8);

    tui::trigger_mouse_right_click(1, 0);

    let frame = tui::render_to_text(30, 8);

    assert!(
      frame.contains("Rename") && frame.contains("Delete"),
      "{frame}"
    );
    assert!(
      !frame.contains("Copy"),
      "the view's menu replaces the terminal's: {frame}"
    );

    let (x, y) = common::locate(&frame, "Delete");

    tui::trigger_mouse_click(x, y);

    assert_eq!(chosen.get_untracked(), "delete");
    assert!(
      !tui::render_to_text(30, 8).contains("Rename"),
      "running an item closes the menu"
    );

    tui::trigger_mouse_press(0, 1);
    tui::trigger_mouse_drag(9, 1);
    tui::trigger_mouse_release(9, 1);
    tui::trigger_mouse_right_click(2, 1);

    let frame = tui::render_to_text(30, 8);

    assert!(
      frame.contains("Copy"),
      "the terminal's menu serves the rest: {frame}"
    );
    assert!(!frame.contains("Rename"), "{frame}");
  });
}

/// The menu key opens the focused view's menu below it, the arrows walk the enabled
/// items, `Enter` runs the focused one, and `Escape` closes the menu.
#[test]
fn the_menu_key_opens_at_the_focused_view_and_the_keys_walk_it() {
  with_platform(|| {
    let chosen = Signal::new(String::new());
    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          Text::new("heading"),
          ContextMenu::new()
            .set_items([
              MenuItem::new("one").handle_select(move || chosen.set("one".to_owned())),
              MenuItem::new("two").set_disabled(true),
              MenuItem::new("three").handle_select(move || chosen.set("three".to_owned())),
            ])
            .set_child(RawButton::new(Activate::new()).set_label("target")),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(30, 8);

    tui::trigger_key("Tab");
    tui::trigger_key("Shift+F10");

    let frame = tui::render_to_text(30, 8);
    let (_, button_row) = common::locate(&frame, "target");
    let (_, first_row) = common::locate(&frame, "one");

    assert!(
      first_row > button_row,
      "the menu opens below the button: {frame}"
    );

    tui::trigger_key("ArrowDown");
    tui::trigger_key("Enter");

    assert_eq!(
      chosen.get_untracked(),
      "three",
      "the disabled item is skipped"
    );
    assert!(
      !tui::render_to_text(30, 8).contains("one"),
      "the menu closed"
    );

    tui::trigger_key("ContextMenu");

    assert!(
      tui::render_to_text(30, 8).contains("one"),
      "the menu key opens it too"
    );

    tui::trigger_key("Escape");

    assert!(
      !tui::render_to_text(30, 8).contains("one"),
      "Escape closes it"
    );
  });
}

/// The standard menu over a focused entry keeps the entry's selection while the
/// menu holds focus, so `Cut` removes what was selected and the entry's caret is
/// where the right press put it afterwards.
#[test]
fn the_standard_menu_acts_on_the_entrys_selection() {
  with_platform(|| {
    let value = Signal::new(String::from("alpha beta"));
    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().bind_value(value))
        .set_width(20)
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(30, 8);

    tui::trigger_key("Tab");
    tui::trigger_mouse_right_click(2, 0);

    let frame = tui::render_to_text(30, 8);

    assert!(frame.contains("Cut"), "{frame}");

    tui::trigger_key("Escape");
    tui::trigger_key("Tab");
    tui::trigger_mouse_right_click(2, 0);

    let frame = tui::render_to_text(30, 8);
    let (x, y) = common::locate(&frame, "Cut");

    tui::trigger_mouse_click(x, y);

    assert_eq!(tui::take_copied().as_deref(), Some("alpha beta"));
    assert_eq!(value.get_untracked(), "", "Cut removed the selected value");
  });
}
