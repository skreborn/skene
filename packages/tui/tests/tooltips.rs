//! Tooltips in cells: the bubble paints beside its anchor once the pointer has rested
//! for the delay, at once on focus, and leaves with the pointer, the focus or `Escape`.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use std::time::Duration;

use skene_tui::{self as tui, Activate, TOOLTIP_DELAY, prelude::*};

use crate::common::with_platform;

/// A tooltip on a button opens below it after the delay while the pointer rests on
/// the button, closes as the pointer leaves, opens at once as the button takes focus,
/// and closes on `Escape`.
#[test]
fn a_tooltip_paints_beside_its_anchor_on_hover_and_focus() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          Tooltip::new("Saves your work")
            .set_placement(Placement::Bottom)
            .set_child(RawButton::new(Activate::new()).set_label("save"))
            .into_any(),
          Text::new("below").into_any(),
        ))
        .into_any()
        .build()
    });
    let frame = tui::render_to_text(24, 4);

    assert!(
      frame
        .lines()
        .next()
        .is_some_and(|line| line.contains("save")),
      "{frame}"
    );
    assert!(!frame.contains("Saves"), "{frame}");

    tui::trigger_mouse_move(1, 0);

    assert!(
      !tui::render_to_text(24, 4).contains("Saves"),
      "the pointer has not rested"
    );

    tui::trigger_frame(TOOLTIP_DELAY);

    let open = tui::render_to_text(24, 4);
    let lines: Vec<&str> = open.lines().collect();

    assert!(
      lines[1].contains("Saves your work"),
      "the bubble sits below: {open}"
    );

    tui::trigger_mouse_move(20, 3);

    assert!(
      !tui::render_to_text(24, 4).contains("Saves"),
      "the pointer left"
    );

    tui::trigger_key("Tab");

    assert!(
      tui::render_to_text(24, 4).contains("Saves your work"),
      "focus opens at once"
    );

    tui::trigger_key("Escape");

    assert!(
      !tui::render_to_text(24, 4).contains("Saves"),
      "escape closed it"
    );

    tui::trigger_mouse_move(1, 0);
    tui::trigger_frame(Duration::from_millis(100));
    tui::trigger_mouse_move(20, 3);
    tui::trigger_frame(TOOLTIP_DELAY);

    assert!(
      !tui::render_to_text(24, 4).contains("Saves"),
      "a leave before the delay cancels"
    );
  });
}
