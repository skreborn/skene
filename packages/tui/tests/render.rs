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
        Button::new()
          .set_style(BoxStyle::new().set_width(6).set_height(3))
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

/// Pointer events reach handlers along the hit path with terminal and view-relative
/// cell coordinates.
#[test]
fn pointer_events_carry_cell_coordinates() {
  let (down, moved) = with_driver(|| {
    let down = Rc::new(Cell::new(None));
    let moved = Rc::new(Cell::new(None));

    {
      let down = Rc::clone(&down);
      let moved = Rc::clone(&moved);

      tui::mount(|| {
        Container::row()
          .set_children((
            Container::new().set_width(4).set_height(3),
            Container::new()
              .set_width(6)
              .set_height(3)
              .handle_pointer_down(move |event| {
                down.set(Some((
                  event.column,
                  event.row,
                  event.local_column,
                  event.local_row,
                  event.button,
                )));
              })
              .handle_pointer_move(move |event| {
                moved.set(Some((event.local_column, event.local_row, event.button)));
              }),
          ))
          .into_any()
          .build()
      });
    }

    let _ = tui::render_to_text(12, 4);

    tui::trigger_mouse_click(6, 1);
    tui::trigger_mouse_move(7, 2);

    (down.get(), moved.get())
  });

  assert_eq!(down, Some((6, 1, 2, 1, Some(PointerButton::Primary))));
  assert_eq!(moved, Some((3, 2, None)));
}

/// The terminal title follows `set_title` and a derived title follows its signal.
#[test]
fn title_follows_state() {
  let (first, second) = with_driver(|| {
    let name = RwSignal::new("one".to_owned());

    tui::mount(|| {
      tui::derive_title(move || format!("app: {}", name.get()));

      Text::new("body").into_any().build()
    });

    let first = tui::current_title();

    name.set("two".to_owned());

    (first, tui::current_title())
  });

  assert_eq!(first.as_deref(), Some("app: one"));
  assert_eq!(second.as_deref(), Some("app: two"));
}

/// A focus handle moves keyboard focus programmatically, and autofocus takes it at
/// mount.
#[test]
fn focus_handle_and_autofocus() {
  with_driver(|| {
    let handle = FocusHandle::new();
    let target = {
      let handle = handle.clone();

      tui::mount(move || {
        Container::column()
          .set_children((
            Input::new().set_autofocus(),
            Button::new()
              .set_style(BoxStyle::new().set_height(1))
              .handle_click(|| {})
              .bind_focus(&handle),
          ))
          .into_any()
          .build()
      })
    };

    let _ = tui::render_to_text(8, 3);

    let autofocused = tui::focused_node().expect("the autofocused input");

    handle.focus();

    let focused = tui::focused_node().expect("the handle's target");

    assert_ne!(autofocused, focused);

    handle.blur();

    assert_eq!(tui::focused_node(), None);

    let _ = target;
  });
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
            Button::new()
              .set_style(BoxStyle::new().set_width(4).set_height(2))
              .handle_click(move || first.set(first.get() + 1)),
            Button::new()
              .set_style(BoxStyle::new().set_width(4).set_height(2))
              .handle_click(move || second.set(second.get() + 1)),
          ))
          .into_any()
          .build()
      });
    }

    let _ = tui::render_to_text(10, 3);

    tui::trigger_mouse_click(5, 0);
    tui::trigger_key("Enter");
    tui::trigger_key(" ");

    (first.get(), second.get())
  });

  // The click hit the second button; Enter and Space then re-click it through the focus
  // it gained.
  assert_eq!(first, 0);
  assert_eq!(second, 3);
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

/// A modal overlay paints over the app with its backdrop scrim, blocks clicks beneath
/// it, and Escape asks it to close.
#[test]
fn modal_overlay_covers_blocks_and_dismisses() {
  let (closed, open_frame, blocked, after_escape) = with_driver(|| {
    let open = RwSignal::new(false);
    let clicks = Rc::new(Cell::new(0u32));

    {
      let clicks = Rc::clone(&clicks);

      tui::mount(move || {
        Container::column()
          .set_children((
            Button::new()
              .set_style(BoxStyle::new().set_width(6).set_height(1))
              .set_child(Text::new("base"))
              .handle_click(move || clicks.set(clicks.get() + 1)),
            Overlay::modal()
              .bind_open(open)
              .set_backdrop(Color::srgb(0., 0., 0.))
              .set_style(BoxStyle::new().set_padding(Inset::all(1)))
              .set_child(Text::new("dialog")),
          ))
          .into_any()
          .build()
      });
    }

    let closed = tui::render_to_text(12, 5);

    open.set(true);

    let open_frame = tui::render_to_text(12, 5);

    // A click where the base button sits is blocked and asks the modal to close.
    tui::trigger_mouse_click(1, 0);

    let blocked = clicks.get();

    let _ = tui::render_to_text(12, 5);

    tui::trigger_key("Escape");

    (closed, open_frame, blocked, tui::render_to_text(12, 5))
  });

  assert!(closed.contains("base"));
  assert!(!closed.contains("dialog"));
  assert!(open_frame.contains("dialog"));
  assert!(!open_frame.contains("base"));
  assert_eq!(blocked, 0);
  assert!(after_escape.contains("base"));
  assert!(!after_escape.contains("dialog"));
}

/// An anchored popup opens below its anchor, and a press outside asks it to close
/// while the press still reaches the app.
#[test]
fn popup_anchors_below_and_light_dismisses() {
  let (frame, dismissed_frame) = with_driver(|| {
    let open = RwSignal::new(false);
    let anchor = Anchor::new();

    {
      let anchor_for_button = anchor.clone();

      tui::mount(move || {
        Container::column()
          .set_children((
            Button::new()
              .set_style(BoxStyle::new().set_width(8).set_height(1))
              .set_child(Text::new("trigger"))
              .bind_anchor(&anchor_for_button),
            Overlay::popup()
              .bind_open(open)
              .set_anchor(&anchor)
              .set_placement(Placement::Bottom)
              .set_child(Text::new("menu")),
          ))
          .into_any()
          .build()
      });
    }

    let _ = tui::render_to_text(12, 4);

    open.set(true);

    let frame = tui::render_to_text(12, 4);

    tui::trigger_mouse_click(10, 3);

    (frame, tui::render_to_text(12, 4))
  });

  let lines: Vec<&str> = frame.lines().collect();

  assert!(lines[0].contains("trigger"));
  assert!(lines[1].contains("menu"));
  assert!(!dismissed_frame.contains("menu"));
}

/// The color-scheme conditions follow the reported background: dark by default, and a
/// `LightMode` variant takes over when the terminal reports a light one.
#[test]
fn color_scheme_conditions_follow_the_background() {
  let (dark, light) = with_driver(|| {
    tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_when(
          Condition::DarkMode,
          BoxStyle::new().set_border(Color::srgb(1., 1., 1.)),
        )
        .into_any()
        .build()
    });

    let dark = tui::render_to_text(12, 4);

    tui::trigger_color_scheme(false);

    (dark, tui::render_to_text(12, 4))
  });

  assert!(dark.contains('┌'));
  assert!(!light.contains('┌'));
}
