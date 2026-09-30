//! The context menu example rendered headlessly: every section paints, a right click
//! on a note opens its menu in place of the standard one, running an item records it,
//! and a right click on the plain text opens the standard menu.

#![cfg(not(target_arch = "wasm32"))]

use skene_tui::{self as tui, prelude::*, testing};

/// Installs a fresh platform, runs `body`, and uninstalls afterwards.
fn with_platform<R>(body: impl FnOnce() -> R) -> R {
  tui::install();

  let result = body();

  tui::uninstall();

  result
}

/// The column and row at which `needle` first appears in a rendered frame.
fn locate(frame: &str, needle: &str) -> (u16, u16) {
  for (row, line) in frame.lines().enumerate() {
    if let Some(column) = line.find(needle) {
      let column = line[..column].chars().count();

      return (
        u16::try_from(column).expect("a narrow frame"),
        u16::try_from(row).expect("a short frame"),
      );
    }
  }

  panic!("{needle:?} did not paint in:\n{frame}");
}

#[test]
fn the_menus_open_where_a_view_opts_in() {
  with_platform(|| {
    let _ = tui::mount(|| tui_context_menu::app().into_view().build());

    let frame = testing::render_to_text(60, 40);

    for title in ["CONTEXT MENU", "Notes", "Groceries", "Draft", "nothing yet"] {
      assert!(
        frame.contains(title),
        "section {title} did not paint:\n{frame}"
      );
    }

    // The draft card's caption names the standard items; a menu adds a line of its own.
    let copies = |frame: &str| frame.matches("Copy").count();
    let plain_copies = copies(&frame);
    let (x, y) = locate(&frame, "Travel plans");

    testing::trigger_mouse_right_click(x, y);

    let menu = testing::render_to_text(60, 40);

    assert!(
      menu.contains("Rename") && menu.contains("Delete"),
      "the note's menu:\n{menu}"
    );
    assert_eq!(
      copies(&menu),
      plain_copies,
      "in place of the standard one:\n{menu}"
    );

    let (x, y) = locate(&menu, "Delete");

    testing::trigger_mouse_click(x, y);

    let frame = testing::render_to_text(60, 40);

    assert!(
      !frame
        .lines()
        .any(|line| line.trim_matches([' ', '│']) == "Travel plans"),
      "the note is gone:\n{frame}"
    );
    assert!(
      frame.contains("Deleted Travel plans"),
      "and recorded:\n{frame}"
    );

    let (x, y) = locate(&frame, "Right-click around");

    testing::trigger_mouse_press(x, y);
    testing::trigger_mouse_drag(x + 5, y);
    testing::trigger_mouse_release(x + 5, y);
    testing::trigger_mouse_right_click(x + 2, y);

    let menu = testing::render_to_text(60, 40);

    assert_eq!(
      copies(&menu),
      plain_copies + 1,
      "the standard menu serves the rest:\n{menu}"
    );
    assert!(!menu.contains("Rename"), "{menu}");
  });
}
