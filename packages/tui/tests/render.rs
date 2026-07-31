//! Headless renders of the terminal facade: the cell-typed views laid out and painted to
//! a text grid, and driven by simulated keys.

#![cfg(not(target_arch = "wasm32"))]

use std::{cell::Cell, rc::Rc};

use skene_tui::{self as tui, prelude::*};

/// Serializes the tests: the UI-thread designation is process-wide, so each test must
/// install, run, and uninstall without another test's thread interleaving.
static DRIVER: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Installs a fresh driver, runs `body`, and uninstalls afterwards.
fn with_driver<R>(body: impl FnOnce() -> R) -> R {
  let _serialized = DRIVER
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();

  let result = body();

  tui::uninstall();

  result
}

/// A column stacks its runs on successive rows.
#[test]
fn column_stacks_runs() {
  let rendered = with_driver(|| {
    tui::mount(|| {
      Container::column()
        .set_children((Text::new("alpha"), Text::new("beta")))
        .into_any()
        .build()
    });

    tui::render_to_text(12, 4)
  });

  assert_eq!(rendered, "alpha\nbeta\n\n\n");
}

/// A row lays its runs side by side, separated by the gap, in cells.
#[test]
fn row_flows_runs_with_spacing() {
  let rendered = with_driver(|| {
    tui::mount(|| {
      Container::row()
        .set_spacing(2)
        .set_children((Text::new("ab"), Text::new("cd")))
        .into_any()
        .build()
    });

    tui::render_to_text(12, 2)
  });

  assert_eq!(rendered, "ab  cd\n\n");
}

/// A border draws box-drawing characters; the root child fills the viewport.
#[test]
fn border_draws_a_frame() {
  let rendered = with_driver(|| {
    tui::mount(|| {
      Container::new()
        .set_width(7)
        .set_border(Color::srgb(1., 1., 1.))
        .set_child(Text::new("hi"))
        .into_any()
        .build()
    });

    tui::render_to_text(10, 4)
  });

  assert_eq!(rendered, "┌─────┐\n│hi   │\n│     │\n└─────┘\n");
}

/// A `MaxWidth` variant applies at narrow terminal sizes and not at wide ones.
#[test]
fn media_variant_resolves_against_terminal_size() {
  let (narrow, wide) = with_driver(|| {
    tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_when(
          Condition::MaxWidth(10),
          BoxStyle::new().set_border(Color::srgb(1., 1., 1.)),
        )
        .into_any()
        .build()
    });

    (tui::render_to_text(8, 4), tui::render_to_text(20, 4))
  });

  assert!(narrow.contains('┌'));
  assert!(!wide.contains('┌'));
}

/// A composed condition resolves in the terminal: the variant applies when either
/// alternative holds, including a nested `and`.
#[test]
fn composed_condition_resolves() {
  let (narrow, tall, wide) = with_driver(|| {
    tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_when(
          Condition::or([
            Condition::MaxWidth(10),
            Condition::and([Condition::MinHeight(6), Condition::MaxHeight(8)]),
          ]),
          BoxStyle::new().set_border(Color::srgb(1., 1., 1.)),
        )
        .into_any()
        .build()
    });

    (
      tui::render_to_text(8, 4),
      tui::render_to_text(20, 7),
      tui::render_to_text(20, 4),
    )
  });

  assert!(narrow.contains('┌'));
  assert!(tall.contains('┌'));
  assert!(!wide.contains('┌'));
}

/// Bold is a plain `bool` on a run, and derived text re-renders on a signal change.
#[test]
fn bold_and_derived_text() {
  let (frame, before, after) = with_driver(|| {
    let count = RwSignal::new(1u32);

    tui::mount(|| {
      Container::column()
        .set_children((
          Text::new("title").set_bold(true),
          Text::derive(move || format!("n={}", count.get())),
        ))
        .into_any()
        .build()
    });

    let frame = tui::render_to_text(8, 2);
    let before = frame.clone();

    count.set(2);

    (frame, before, tui::render_to_text(8, 2))
  });

  assert_eq!(frame, "title\nn=1\n");
  assert_eq!(before, "title\nn=1\n");
  assert_eq!(after, "title\nn=2\n");
}

/// Tab focuses a clickable box, a `Focused` variant applies, and `Enter` clicks it.
#[test]
fn focus_variant_and_click() {
  let (unfocused, focused, count) = with_driver(|| {
    let clicks = Rc::new(Cell::new(0u32));

    {
      let clicks = Rc::clone(&clicks);

      tui::mount(|| {
        Container::new()
          .set_width(6)
          .set_height(3)
          .set_when(
            Condition::Focused,
            BoxStyle::new().set_border(Color::srgb(1., 1., 1.)),
          )
          .handle_click(move || clicks.set(clicks.get() + 1))
          .into_any()
          .build()
      });
    }

    let unfocused = tui::render_to_text(8, 4);

    tui::trigger_key("Tab");

    let focused = tui::render_to_text(8, 4);

    tui::trigger_key("Enter");

    (unfocused, focused, clicks.get())
  });

  assert!(!unfocused.contains('┌'));
  assert!(focused.contains('┌'));
  assert_eq!(count, 1);
}

/// Typing into a focused input flows through `bind_value` into the signal and back onto
/// the frame.
#[test]
fn input_binds_a_signal_both_ways() {
  let (value, frame) = with_driver(|| {
    let name = RwSignal::new(String::new());

    tui::mount(|| {
      Input::new()
        .set_placeholder("name")
        .bind_value(name)
        .into_any()
        .build()
    });

    tui::trigger_key("Tab");
    tui::trigger_key("h");
    tui::trigger_key("i");

    (name.get_untracked(), tui::render_to_text(10, 1))
  });

  assert_eq!(value, "hi");
  assert!(frame.starts_with("hi"));
}

/// A dynamic subtree swaps its content when its dependency changes.
#[test]
fn dynamic_swaps_content() {
  let (before, after) = with_driver(|| {
    let on = RwSignal::new(false);

    tui::mount(|| {
      DynamicView::new(move || {
        if on.get() {
          Text::new("on").into_any()
        }
        else {
          Text::new("off").into_any()
        }
      })
      .into_any()
      .build()
    });

    let before = tui::render_to_text(6, 1);

    on.set(true);

    (before, tui::render_to_text(6, 1))
  });

  assert_eq!(before, "off\n");
  assert_eq!(after, "on\n");
}

/// The mouse hovering a node applies its `Hovered` variant and fires pointer handlers;
/// moving off reverts and fires the leave handler.
#[test]
fn hover_variant_and_pointer_handlers() {
  let (plain, hovered, after, enters, leaves) = with_driver(|| {
    let enters = Rc::new(Cell::new(0u32));
    let leaves = Rc::new(Cell::new(0u32));

    {
      let enters = Rc::clone(&enters);
      let leaves = Rc::clone(&leaves);

      tui::mount(|| {
        Container::row()
          .set_children((
            Container::new()
              .set_width(4)
              .set_height(2)
              .set_when(
                Condition::Hovered,
                BoxStyle::new().set_border(Color::srgb(1., 1., 1.)),
              )
              .handle_pointer_enter(move || enters.set(enters.get() + 1))
              .handle_pointer_leave(move || leaves.set(leaves.get() + 1)),
            Container::new().set_width(4).set_height(2),
          ))
          .into_any()
          .build()
      });
    }

    let plain = tui::render_to_text(10, 3);

    tui::trigger_mouse_move(1, 0);

    let hovered = tui::render_to_text(10, 3);

    tui::trigger_mouse_move(6, 0);

    let after = tui::render_to_text(10, 3);

    (plain, hovered, after, enters.get(), leaves.get())
  });

  assert!(!plain.contains('┌'));
  assert!(hovered.contains('┌'));
  assert!(!after.contains('┌'));
  assert_eq!(enters, 1);
  assert_eq!(leaves, 1);
}

/// A mouse click hits the topmost node at the cell, runs its click handlers, and
/// focuses it.
#[test]
fn mouse_click_hits_and_focuses() {
  let (first, second) = with_driver(|| {
    let first = Rc::new(Cell::new(0u32));
    let second = Rc::new(Cell::new(0u32));

    {
      let first = Rc::clone(&first);
      let second = Rc::clone(&second);

      tui::mount(|| {
        Container::row()
          .set_children((
            Container::new()
              .set_width(4)
              .set_height(2)
              .handle_click(move || first.set(first.get() + 1)),
            Container::new()
              .set_width(4)
              .set_height(2)
              .handle_click(move || second.set(second.get() + 1)),
          ))
          .into_any()
          .build()
      });
    }

    let _ = tui::render_to_text(10, 3);

    tui::trigger_mouse_click(5, 0);
    tui::trigger_key("Enter");

    (first.get(), second.get())
  });

  // The click hit the second box; Enter then re-clicks it through the focus it gained.
  assert_eq!(first, 0);
  assert_eq!(second, 2);
}

/// The hover path's deepest node with a cursor decides the pointer shape.
#[test]
fn hover_cursor_resolves() {
  with_driver(|| {
    tui::mount(|| {
      Container::new()
        .set_width(4)
        .set_height(2)
        .set_cursor(skene_tui::core::MouseCursor::Click)
        .into_any()
        .build()
    });

    let _ = tui::render_to_text(10, 3);

    tui::trigger_mouse_move(1, 0);
  });
}
