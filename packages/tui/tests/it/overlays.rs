//! Overlays in cells: a modal's scrim, blocking and dismissal, a popup's anchoring and
//! light dismissal, and focus trapped inside a modal, taken by it on open and returned
//! on close.

use std::{cell::Cell, rc::Rc};

use skene_tui::{self as tui, Activate, HasChild, prelude::*, testing};

use crate::common::{black, red, with_platform};

/// A modal overlay paints over the app with its backdrop scrim, blocks clicks beneath
/// it, and `Escape` asks it to close.
#[test]
fn modal_overlay_covers_blocks_and_dismisses() {
  with_platform(|| {
    let open = Signal::new(false);
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_label("base")
            .handle_click(move || counted.set(counted.get() + 1))
            .into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_backdrop(Some(black()))
            .set_padding(Inset::all(1))
            .set_child(Text::new("dialog"))
            .into_any(),
        ))
        .into_any()
        .build()
    });

    let closed = testing::render_to_text(12, 5);

    assert!(closed.contains("base") && !closed.contains("dialog"));

    open.set(true);

    let opened = testing::render_to_text(12, 5);

    assert!(opened.contains("dialog") && opened.contains("base"));

    // A click where the base button sits is blocked and asks the modal to close.
    testing::trigger_mouse_click(1, 0);

    assert_eq!(clicks.get(), 0);
    assert!(!open.get());

    open.set(true);

    let _ = testing::render_to_text(12, 5);

    testing::trigger_key("Escape");

    assert!(!open.get());
    assert!(!testing::render_to_text(12, 5).contains("dialog"));
  });
}

/// An anchored popup opens below its anchor, and a press outside asks it to close
/// while the press still reaches the app.
#[test]
fn popup_anchors_below_and_light_dismisses() {
  with_platform(|| {
    let open = Signal::new(false);
    let anchor = NodeHandle::new();
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_label("trigger")
            .bind_handle(anchor)
            .into_any(),
          Overlay::popup()
            .bind_open(open)
            .set_anchor(anchor)
            .set_placement(Placement::Bottom)
            .set_child(Text::new("menu"))
            .into_any(),
          Container::new()
            .set_height(1)
            .handle_click(move || counted.set(counted.get() + 1))
            .set_child(Text::new("below"))
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 4);

    open.set(true);

    let frame = testing::render_to_text(12, 4);
    let lines: Vec<&str> = frame.lines().collect();

    assert!(lines[0].contains("trigger"), "{frame}");
    assert!(lines[1].contains("menu"), "{frame}");

    testing::trigger_mouse_click(10, 1);

    assert!(!open.get(), "the outside press dismissed");
    assert_eq!(clicks.get(), 1, "and still reached the app");
    assert!(!testing::render_to_text(12, 4).contains("menu"));
  });
}

/// A popup anchored inside another opens against where its anchor shows, the outer
/// popup's placement included.
#[test]
fn a_nested_popup_anchors_where_its_anchor_shows() {
  with_platform(|| {
    let outer = Signal::new(true);
    let inner = Signal::new(false);
    let trigger = NodeHandle::new();
    let item = NodeHandle::new();

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          Text::new("top").into_any(),
          Text::new("trigger").into_any(),
          Container::new()
            .bind_handle(trigger)
            .set_child(Text::new("anchor"))
            .into_any(),
          Overlay::popup()
            .bind_open(outer)
            .set_anchor(trigger)
            .set_placement(Placement::Bottom)
            .set_child(
              Column::new().set_children((
                Text::new("first").into_any(),
                Container::new()
                  .bind_handle(item)
                  .set_child(Text::new("second"))
                  .into_any(),
                Overlay::popup()
                  .bind_open(inner)
                  .set_anchor(item)
                  .set_placement(Placement::Bottom)
                  .set_child(Text::new("nested"))
                  .into_any(),
              )),
            )
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 8);

    inner.set(true);

    let frame = testing::render_to_text(12, 8);
    let lines: Vec<&str> = frame.lines().collect();

    assert!(lines[3].starts_with("first"), "{frame}");
    assert!(lines[4].starts_with("second"), "{frame}");
    assert!(lines[5].starts_with("nested"), "{frame}");
  });
}

/// A modal's default scrim dims the app beneath it, and a fully transparent backdrop
/// disables the scrim entirely.
#[test]
fn modal_scrim_defaults_on_and_disables() {
  let scrim = |backdrop: Option<Color>| {
    with_platform(move || {
      let open = Signal::new(false);

      let _ = tui::mount(move || {
        let modal = Overlay::modal().bind_open(open);
        let modal = match backdrop {
          Some(color) => modal.set_backdrop(Some(color)),
          None => modal,
        };

        Column::new()
          .set_children((
            Container::new().set_color(red()).set_height(1).into_any(),
            modal.set_child(Text::new("hi")).into_any(),
          ))
          .into_any()
          .build()
      });

      let before = testing::render_to_colors(6, 3);

      open.set(true);

      (before, testing::render_to_colors(6, 3))
    })
  };

  let (before, dimmed) = scrim(None);

  assert_eq!(before[0].1, Some((255, 0, 0)));
  assert_eq!(dimmed[0].1, Some((128, 0, 0)));

  let (bright_before, undimmed) = scrim(Some(black().with_alpha(0.)));

  assert_eq!(bright_before[0].1, undimmed[0].1);
}

/// An open modal holds the tab order: `Tab` cycles the controls inside it and never
/// reaches the ones beneath.
#[test]
fn a_modal_traps_focus() {
  with_platform(|| {
    let open = Signal::new(true);
    let outside = NodeHandle::new();
    let inside = NodeHandle::new();

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_label("outside")
            .bind_handle(outside)
            .into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_child(
              RawButton::new(Activate::new())
                .set_label("inside")
                .bind_handle(inside),
            )
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 4);

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), inside.node());

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), inside.node());

    open.set(false);

    let _ = testing::render_to_text(12, 4);

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), outside.node());
  });
}

/// Overlays stack in the order they opened, as a browser's top layer does: of two open
/// modals, the one opened last holds the tab order, whichever was built first.
#[test]
fn the_modal_opened_last_holds_the_tab_order() {
  with_platform(|| {
    let (first_open, second_open) = (Signal::new(false), Signal::new(false));
    let (first, second) = (NodeHandle::new(), NodeHandle::new());

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          Overlay::modal()
            .bind_open(first_open)
            .set_child(
              RawButton::new(Activate::new())
                .set_label("first")
                .bind_handle(first),
            )
            .into_any(),
          Overlay::modal()
            .bind_open(second_open)
            .set_child(
              RawButton::new(Activate::new())
                .set_label("second")
                .bind_handle(second),
            )
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 4);

    second_open.set(true);

    let _ = testing::render_to_text(12, 4);

    first_open.set(true);

    let _ = testing::render_to_text(12, 4);

    assert_eq!(testing::focused_node(), first.node());

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), first.node());
  });
}

/// A modal stays in charge while a popup above it is open: with a tooltip showing over
/// the dialog, `Tab` stays inside, and a press on the page reaches nothing, dismissing
/// the tooltip and then the modal, as a press outside both does.
#[test]
fn a_modal_holds_under_a_popup_above_it() {
  with_platform(|| {
    let open = Signal::new(false);
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);
    let inside = NodeHandle::new();

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_label("page")
            .handle_click(move || counted.set(counted.get() + 1))
            .into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_backdrop(None)
            .set_child(
              Tooltip::new("Confirms")
                .set_child(
                  RawButton::new(Activate::new())
                    .set_label("inside")
                    .bind_handle(inside),
                )
                .into_any(),
            )
            .into_any(),
        ))
        .into_any()
        .build()
    });

    open.set(true);

    let _ = testing::render_to_text(24, 6);

    testing::trigger_key("Tab");

    let frame = testing::render_to_text(24, 6);

    assert_eq!(testing::focused_node(), inside.node());
    assert!(frame.contains("Confirms"), "the tooltip shows: {frame}");

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), inside.node());

    testing::trigger_mouse_click(1, 0);

    assert_eq!(clicks.get(), 0);
    assert!(!open.get(), "the press outside dismisses the modal too");
  });
}

/// A modal takes focus to its first tab stop as it opens and returns it to the node
/// that held focus as it closes.
#[test]
fn a_modal_takes_focus_on_open_and_returns_it_on_close() {
  with_platform(|| {
    let open = Signal::new(false);
    let opener = NodeHandle::new();
    let inside = NodeHandle::new();

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_label("open")
            .bind_handle(opener)
            .handle_click(move || open.set(true))
            .into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_child(
              RawButton::new(Activate::new())
                .set_label("close")
                .bind_handle(inside)
                .handle_click(move || open.set(false)),
            )
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 4);

    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    assert!(open.get(), "the opener opened the modal");
    assert_eq!(
      testing::focused_node(),
      inside.node(),
      "focus landed inside"
    );

    testing::trigger_key("Enter");

    assert!(!open.get(), "the close button closed the modal");
    assert_eq!(
      testing::focused_node(),
      opener.node(),
      "focus returned to the opener"
    );
  });
}
