//! Tooltips in cells: the bubble paints beside its anchor once the pointer has rested
//! for the delay, at once on focus that shows, stays while the pointer rests on it, and leaves
//! with the pointer, the focus or `Escape`.

use std::time::Duration;

use skene_tui::{self as tui, Activate, TOOLTIP_DELAY, TOOLTIP_GRACE, prelude::*, testing};

use crate::common::with_platform;

/// A click focuses a described button without opening its bubble at once, since focus
/// a press gives does not show; the rest before the delay still opens it.
#[test]
fn a_click_leaves_the_opening_to_the_delay() {
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
    let _ = testing::render_to_text(24, 4);

    testing::trigger_mouse_click(1, 0);

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "focus from a click does not show"
    );

    testing::trigger_frame(TOOLTIP_DELAY);

    assert!(
      testing::render_to_text(24, 4).contains("Saves your work"),
      "the pointer rested"
    );
  });
}

/// A tooltip on a button opens below it after the delay while the pointer rests on
/// the button, closes the grace period after the pointer leaves, opens at once as the
/// button takes focus, and closes on `Escape`.
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
    let frame = testing::render_to_text(24, 4);

    assert!(
      frame
        .lines()
        .next()
        .is_some_and(|line| line.contains("save")),
      "{frame}"
    );
    assert!(!frame.contains("Saves"), "{frame}");

    testing::trigger_mouse_move(1, 0);

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "the pointer has not rested"
    );

    testing::trigger_frame(TOOLTIP_DELAY);

    let open = testing::render_to_text(24, 4);
    let lines: Vec<&str> = open.lines().collect();

    assert!(
      lines[1].contains("Saves your work"),
      "the bubble sits below: {open}"
    );

    testing::trigger_mouse_move(20, 3);

    assert!(
      testing::render_to_text(24, 4).contains("Saves your work"),
      "open through the grace period"
    );

    testing::trigger_frame(TOOLTIP_GRACE);

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "the pointer left"
    );

    testing::trigger_key("Tab");

    assert!(
      testing::render_to_text(24, 4).contains("Saves your work"),
      "focus opens at once"
    );

    testing::trigger_key("Escape");

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "escape closed it"
    );

    testing::trigger_mouse_move(1, 0);
    testing::trigger_frame(Duration::from_millis(100));
    testing::trigger_mouse_move(20, 3);
    testing::trigger_frame(TOOLTIP_DELAY);

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "a leave before the delay cancels"
    );
  });
}

/// The pointer can cross from the button onto the bubble, which stays open while the
/// pointer rests on it and closes the grace period after the pointer leaves both.
#[test]
fn a_tooltip_stays_open_while_the_pointer_rests_on_its_bubble() {
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
    let _ = testing::render_to_text(24, 4);

    testing::trigger_mouse_move(1, 0);
    testing::trigger_frame(TOOLTIP_DELAY);

    let open = testing::render_to_text(24, 4);
    let column = open
      .lines()
      .nth(1)
      .and_then(|line| line.split("Saves").next())
      .map(|before| before.chars().count())
      .expect("the bubble");

    testing::trigger_mouse_move(u16::try_from(column).expect("a column"), 1);
    testing::trigger_frame(TOOLTIP_GRACE * 2);

    assert!(
      testing::render_to_text(24, 4).contains("Saves your work"),
      "open while the pointer rests on the bubble"
    );

    testing::trigger_mouse_move(20, 3);
    testing::trigger_frame(TOOLTIP_GRACE);

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "closed once the pointer left both"
    );
  });
}
