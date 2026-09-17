//! Overlays in cells: a modal's scrim, blocking and dismissal, a popup's anchoring and
//! light dismissal, and focus trapped inside a modal, taken by it on open and returned
//! on close.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use std::{cell::Cell, rc::Rc};

use skene_tui::{self as tui, Activate, prelude::*};

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

    let closed = tui::render_to_text(12, 5);

    assert!(closed.contains("base") && !closed.contains("dialog"));

    open.set(true);

    let opened = tui::render_to_text(12, 5);

    assert!(opened.contains("dialog") && opened.contains("base"));

    // A click where the base button sits is blocked and asks the modal to close.
    tui::trigger_mouse_click(1, 0);

    assert_eq!(clicks.get(), 0);
    assert!(!open.get());

    open.set(true);

    let _ = tui::render_to_text(12, 5);

    tui::trigger_key("Escape");

    assert!(!open.get());
    assert!(!tui::render_to_text(12, 5).contains("dialog"));
  });
}

/// An anchored popup opens below its anchor, and a press outside asks it to close
/// while the press still reaches the app.
#[test]
fn popup_anchors_below_and_light_dismisses() {
  with_platform(|| {
    let open = Signal::new(false);
    let anchor = Anchor::new();
    let bound = anchor.clone();
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_label("trigger")
            .bind_anchor(&bound)
            .into_any(),
          Overlay::popup()
            .bind_open(open)
            .set_anchor(&anchor)
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
    let _ = tui::render_to_text(12, 4);

    open.set(true);

    let frame = tui::render_to_text(12, 4);
    let lines: Vec<&str> = frame.lines().collect();

    assert!(lines[0].contains("trigger"), "{frame}");
    assert!(lines[1].contains("menu"), "{frame}");

    tui::trigger_mouse_click(10, 1);

    assert!(!open.get(), "the outside press dismissed");
    assert_eq!(clicks.get(), 1, "and still reached the app");
    assert!(!tui::render_to_text(12, 4).contains("menu"));
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

      let before = tui::render_to_colors(6, 3);

      open.set(true);

      (before, tui::render_to_colors(6, 3))
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
    let outside = FocusHandle::new();
    let inside = FocusHandle::new();
    let (bound_outside, bound_inside) = (outside.clone(), inside.clone());

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_label("outside")
            .bind_focus(&bound_outside)
            .into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_child(
              RawButton::new(Activate::new())
                .set_label("inside")
                .bind_focus(&bound_inside),
            )
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(12, 4);

    tui::trigger_key("Tab");

    assert_eq!(tui::focused_node(), inside.node());

    tui::trigger_key("Tab");

    assert_eq!(tui::focused_node(), inside.node());

    open.set(false);

    let _ = tui::render_to_text(12, 4);

    tui::trigger_key("Tab");

    assert_eq!(tui::focused_node(), outside.node());
  });
}

/// A modal takes focus to its first tab stop as it opens and returns it to the node
/// that held focus as it closes.
#[test]
fn a_modal_takes_focus_on_open_and_returns_it_on_close() {
  with_platform(|| {
    let open = Signal::new(false);
    let opener = FocusHandle::new();
    let inside = FocusHandle::new();
    let (bound_opener, bound_inside) = (opener.clone(), inside.clone());

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_label("open")
            .bind_focus(&bound_opener)
            .handle_click(move || open.set(true))
            .into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_child(
              RawButton::new(Activate::new())
                .set_label("close")
                .bind_focus(&bound_inside)
                .handle_click(move || open.set(false)),
            )
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(12, 4);

    tui::trigger_key("Tab");
    tui::trigger_key("Enter");

    assert!(open.get(), "the opener opened the modal");
    assert_eq!(tui::focused_node(), inside.node(), "focus landed inside");

    tui::trigger_key("Enter");

    assert!(!open.get(), "the close button closed the modal");
    assert_eq!(
      tui::focused_node(),
      opener.node(),
      "focus returned to the opener"
    );
  });
}
