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
  assert!(open_frame.contains("base"));
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

/// A scroll box clips its content, the wheel moves it, a click resolves against the
/// scrolled position, and a thumb marks the offset on the right edge.
#[test]
fn scroll_box_clips_wheels_and_hit_tests() {
  let clicked = Rc::new(std::cell::RefCell::new(Vec::new()));
  let clicks = Rc::clone(&clicked);
  let (first, scrolled, log) = with_driver(move || {
    tui::mount(|| {
      Container::new()
        .set_child(
          Container::column()
            .set_height(3)
            .set_overflow(Overflow::Auto)
            .derive_children(
              || 0..6,
              |index| *index,
              move |index| {
                let clicked = Rc::clone(&clicked);

                Container::new()
                  .handle_click(move || clicked.borrow_mut().push(index))
                  .set_child(Text::new(format!("item {index}")))
              },
            ),
        )
        .into_any()
        .build()
    });

    let first = tui::render_to_text(12, 4);

    tui::trigger_mouse_scroll(2, 1, 0, 2);

    let scrolled = tui::render_to_text(12, 4);

    tui::trigger_mouse_click(2, 1);

    (first, scrolled, clicks.borrow().clone())
  });

  assert!(first.contains("item 0"));
  assert!(first.contains("item 2"));
  assert!(!first.contains("item 3"));
  assert!(first.contains('█'));

  assert!(!scrolled.contains("item 1"));
  assert!(scrolled.contains("item 2"));
  assert!(scrolled.contains("item 4"));
  assert!(!scrolled.contains("item 5"));

  assert_eq!(log, vec![3]);
}

/// Focusing a control below the fold scrolls it into view.
#[test]
fn focus_scrolls_into_view() {
  let frame = with_driver(|| {
    tui::mount(|| {
      Container::column()
        .set_height(3)
        .set_overflow(Overflow::Scroll)
        .set_children(
          (0..6)
            .map(|index| {
              Button::new()
                .set_child(Text::new(format!("button {index}")))
                .into_any()
            })
            .collect::<Vec<_>>(),
        )
        .into_any()
        .build()
    });

    let _ = tui::render_to_text(12, 4);

    for _ in 0..5 {
      tui::trigger_key("Tab");
    }

    tui::render_to_text(12, 4)
  });

  assert!(frame.contains("button 4"));
  assert!(!frame.contains("button 0"));
}

/// A virtual list builds only the visible slice plus overscan, and scrolling shifts
/// that window while the content stays in place.
#[test]
fn virtual_list_windows_its_items() {
  let built = Rc::new(std::cell::RefCell::new(Vec::new()));
  let log = Rc::clone(&built);
  let (first, second, scrolled, log) = with_driver(move || {
    tui::mount(|| {
      Container::new()
        .set_child(
          VirtualList::new(
            1,
            || 1000,
            move |index| {
              log.borrow_mut().push(index);

              Text::new(format!("row {index}"))
            },
          )
          .set_overscan(2)
          .set_style(BoxStyle::new().set_height(4)),
        )
        .into_any()
        .build()
    });

    // The first render measures the viewport; the second builds the settled window.
    let first = tui::render_to_text(12, 6);
    let second = tui::render_to_text(12, 6);

    tui::trigger_mouse_scroll(2, 1, 0, 500);

    let scrolled = tui::render_to_text(12, 6);

    (first, second, scrolled, built.borrow().clone())
  });

  assert!(first.contains("row 0"));

  assert!(second.contains("row 0"));
  assert!(second.contains("row 3"));
  assert!(!second.contains("row 7"));

  assert!(scrolled.contains("row 500"));
  assert!(scrolled.contains("row 503"));

  let max_built = log.iter().copied().max().unwrap_or(0);
  let min_built_after_500 = log.iter().copied().filter(|index| *index >= 490).count();

  assert!(
    log.len() < 30,
    "built {} items instead of a window",
    log.len()
  );
  assert!(max_built < 520);
  assert!(min_built_after_500 > 0);
}

/// A suspense view shows its fallback until the resource inside settles, then swaps to
/// the content; refetching does not re-suspend (the swap follows the boundary's tasks).
#[test]
fn suspense_swaps_fallback_for_loaded_content() {
  let (loading, loaded) = with_driver(|| {
    tui::mount(|| {
      Suspense::new(|| {
        let value = Resource::new(|| async { 42u32 });

        DynamicView::new(move || match value.get() {
          None => Text::new("hold").into_any(),
          Some(value) => Text::new(format!("value {value}")).into_any(),
        })
      })
      .set_fallback(|| Text::new("LOADING"))
      .into_any()
      .build()
    });

    let loading = tui::render_to_text(12, 2);

    let _ = tui::pump_tasks();

    (loading, tui::render_to_text(12, 2))
  });

  assert!(loading.contains("LOADING"));
  assert!(!loading.contains("value"));

  assert!(loaded.contains("value 42"));
  assert!(!loaded.contains("LOADING"));
}

/// A spawned task is scope-bound: content torn down before its task lands never runs
/// the continuation.
#[test]
fn tasks_end_with_their_scope() {
  let landed = Rc::new(Cell::new(false));
  let seen = Rc::clone(&landed);
  with_driver(move || {
    let show = RwSignal::new(true);

    tui::mount(|| {
      DynamicView::new(move || {
        if show.get() {
          let landed = Rc::clone(&landed);

          tui::spawn(async move { landed.set(true) });

          Text::new("on").into_any()
        }
        else {
          Text::new("off").into_any()
        }
      })
      .into_any()
      .build()
    });

    show.set(false);
    let _ = tui::pump_tasks();
  });

  assert!(!seen.get());
}

/// An error boundary swaps to its fallback when a captured handle reports, and clearing
/// restores the content as it was.
#[test]
fn error_boundary_presents_and_clears_reports() {
  let handle = Rc::new(Cell::new(None::<Errors>));
  let grabbed = Rc::clone(&handle);
  let (before, failed, restored) = with_driver(move || {
    tui::mount(|| {
      ErrorBoundary::new(move || {
        grabbed.set(Errors::current());

        Text::new("CONTENT")
      })
      .set_fallback(|errors| Text::new(errors.with(|errors| format!("failed: {}", errors[0]))))
      .into_any()
      .build()
    });

    let before = tui::render_to_text(30, 2);
    let errors = handle.get().expect("a boundary handle");

    errors.report(std::io::Error::other("boom"));

    let failed = tui::render_to_text(30, 2);

    errors.clear();

    (before, failed, tui::render_to_text(30, 2))
  });

  assert!(before.contains("CONTENT"));

  assert!(failed.contains("failed: boom"));
  assert!(!failed.contains("CONTENT"));

  assert!(restored.contains("CONTENT"));
  assert!(!restored.contains("failed"));
}

/// A transitional subtree holds the previous content while the next rebuild's resource
/// is pending, and swaps only once it settles.
#[test]
fn transition_holds_previous_content_until_settled() {
  let (first, held, swapped) = with_driver(|| {
    let route = RwSignal::new(1u32);

    tui::mount(|| {
      DynamicView::transitional(move || {
        let page = route.get();
        let value = Resource::new(move || async move { page * 10 });

        DynamicView::new(move || match value.get() {
          None => Text::new("hold").into_any(),
          Some(value) => Text::new(format!("page {value}")).into_any(),
        })
      })
      .into_any()
      .build()
    });

    let _ = tui::pump_tasks();

    let first = tui::render_to_text(12, 2);

    route.set(2);

    let held = tui::render_to_text(12, 2);

    let _ = tui::pump_tasks();

    (first, held, tui::render_to_text(12, 2))
  });

  assert!(first.contains("page 10"));

  assert!(held.contains("page 10"));
  assert!(!held.contains("page 20"));
  assert!(!held.contains("hold"));

  assert!(swapped.contains("page 20"));
  assert!(!swapped.contains("page 10"));
}

/// A checkbox toggles by `Space` and by mouse click, both directions of the binding.
#[test]
fn checkbox_toggles_by_key_and_click() {
  let (unchecked, checked, value) = with_driver(|| {
    let on = RwSignal::new(false);

    tui::mount(|| Checkbox::new().bind_checked(on).into_any().build());

    let unchecked = tui::render_to_text(6, 1);

    tui::trigger_key("Tab");
    tui::trigger_key(" ");

    let checked = tui::render_to_text(6, 1);

    tui::trigger_mouse_click(0, 0);

    (unchecked, checked, on.get_untracked())
  });

  assert!(unchecked.contains('\u{2610}'));
  assert!(checked.contains('\u{2611}'));
  assert!(!value);
}

/// Radios in a group are exclusive, arrow keys move the check, and the bound signal
/// follows the chosen value.
#[test]
fn radio_group_moves_the_check() {
  let (first, moved, value) = with_driver(|| {
    let choice = RwSignal::new("a".to_owned());

    tui::mount(|| {
      Container::column()
        .set_children((
          Radio::new("letters", "a").bind_value(choice),
          Radio::new("letters", "b").bind_value(choice),
          Radio::new("letters", "c").bind_value(choice),
        ))
        .into_any()
        .build()
    });

    let first = tui::render_to_text(6, 3);

    tui::trigger_key("Tab");
    tui::trigger_key("Enter");
    tui::trigger_key("ArrowDown");
    tui::trigger_key("ArrowDown");

    let moved = tui::render_to_text(6, 3);

    (first, moved, choice.get_untracked())
  });

  let checked = |frame: &str| {
    frame
      .lines()
      .enumerate()
      .filter(|(_, line)| line.contains('\u{25c9}'))
      .map(|(row, _)| row)
      .collect::<Vec<_>>()
  };

  assert_eq!(checked(&first), vec![0]);
  assert_eq!(checked(&moved), vec![2]);
  assert_eq!(value, "c");
}

/// A select cycles through its options with `Enter` and arrows, wrapping.
#[test]
fn select_cycles_its_options() {
  let (initial, cycled, back, value) = with_driver(|| {
    let unit = RwSignal::new("px".to_owned());

    tui::mount(|| {
      Select::new()
        .set_options([("px", "pixels"), ("cell", "cells")])
        .bind_value(unit)
        .into_any()
        .build()
    });

    let initial = tui::render_to_text(10, 1);

    tui::trigger_key("Tab");
    tui::trigger_key("Enter");

    let cycled = tui::render_to_text(10, 1);

    tui::trigger_key("ArrowUp");

    (
      initial,
      cycled,
      tui::render_to_text(10, 1),
      unit.get_untracked(),
    )
  });

  assert!(initial.contains("pixels"));
  assert!(cycled.contains("cells"));
  assert!(back.contains("pixels"));
  assert_eq!(value, "px");
}

/// A slider nudges by its step on arrows and jumps to a clicked cell.
#[test]
fn slider_nudges_and_jumps() {
  let (track, after_keys, after_click) = with_driver(|| {
    let volume = RwSignal::new(0.0f32);

    tui::mount(|| {
      Slider::new(0.0, 10.0)
        .set_step(1.0)
        .bind_value(volume)
        .into_any()
        .build()
    });

    let track = tui::render_to_text(14, 1);

    tui::trigger_key("Tab");
    tui::trigger_key("ArrowRight");
    tui::trigger_key("ArrowRight");

    let after_keys = volume.get_untracked();

    tui::trigger_mouse_click(11, 0);

    (track, after_keys, volume.get_untracked())
  });

  assert!(track.starts_with('●'));
  assert!((after_keys - 2.0).abs() < f32::EPSILON);
  assert!((after_click - 10.0).abs() < f32::EPSILON);
}

/// A shadow mixes its color into both colors of the band below and to the right.
#[test]
fn shadow_mixes_into_the_cells_beneath() {
  let colors = with_driver(|| {
    tui::mount(|| {
      Container::new()
        .set_color(Color::srgb(1., 0., 0.))
        .set_child(
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_shadow(Color::srgb(0., 0., 0.).with_alpha(0.5)),
        )
        .into_any()
        .build()
    });

    tui::render_to_colors(4, 3)
  });

  let background = |x: usize, y: usize| colors[y * 4 + x].1.expect("a painted background");
  let clean = background(0, 0);

  // The one-cell band below and to the right dims; everything else keeps the backdrop.
  assert_eq!(clean, (255, 0, 0));
  assert_eq!(background(1, 1), background(2, 1));
  assert!(background(2, 1).0 < clean.0);
  assert_eq!(background(3, 1), clean);
  assert_eq!(background(1, 2), clean);
}

/// A control theme restyles the glyphs of the subtree, and a control's own setter wins
/// over it.
#[test]
fn control_theme_restyles_glyphs() {
  let frame = with_driver(|| {
    tui::mount(|| {
      ControlTheme::new(Container::column().set_children((
        Checkbox::new().set_checked(true),
        Checkbox::new().set_checked(true).set_marks('#', '.'),
        Radio::new("g", "a").set_checked(true),
      )))
      .set_checkbox('X', 'O')
      .set_radio('>', '<')
      .into_any()
      .build()
    });

    tui::render_to_text(6, 3)
  });

  let lines: Vec<&str> = frame.lines().collect();

  assert_eq!(lines[0].trim_end(), "X");
  assert_eq!(lines[1].trim_end(), "#");
  assert_eq!(lines[2].trim_end(), ">");
}

/// A modal's default scrim dims the app beneath it, and a fully transparent backdrop
/// disables the scrim entirely.
#[test]
fn modal_scrim_defaults_on_and_disables() {
  let scrim = |backdrop: Option<Color>| {
    with_driver(move || {
      let open = RwSignal::new(false);

      tui::mount(move || {
        let modal = Overlay::modal().bind_open(open).set_child(Text::new("hi"));
        let modal = match backdrop {
          Some(color) => modal.set_backdrop(color),
          None => modal,
        };

        Container::column()
          .set_children((
            Container::new()
              .set_color(Color::srgb(1., 0., 0.))
              .set_height(1),
            modal,
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

  let (bright_before, undimmed) = scrim(Some(Color::srgb(0., 0., 0.).with_alpha(0.)));

  assert_eq!(bright_before[0].1, undimmed[0].1);
}

/// A small control styles through `ControlStyle`—the subset it honors; the
/// container-only properties (shadow, spacing, alignment) are not on the type.
#[test]
fn control_style_styles_the_honored_subset() {
  let frame = with_driver(|| {
    tui::mount(|| {
      Checkbox::new()
        .set_style(ControlStyle::new().set_border(Color::srgb(1., 1., 1.)))
        .into_any()
        .build()
    });

    tui::render_to_text(5, 3)
  });

  assert!(frame.contains('┌'));
  assert!(frame.contains('\u{2610}'));
}
