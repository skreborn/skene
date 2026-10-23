//! Scrolling in cells: clipping, the wheel and its latch, scrollbars, focus revealing a
//! control, the page keys, and a lazy column's window.

use std::{cell::RefCell, rc::Rc};

use skene_terminal::{self as terminal, Activate, HasChild, prelude::*, testing};

use crate::common::with_platform;

/// A scroll box clips its content, the wheel moves it, a click resolves against the
/// scrolled position, and a thumb marks the offset on the right edge.
#[test]
fn scroll_box_clips_wheels_and_hit_tests() {
  with_platform(|| {
    let clicked = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&clicked);

    let _ = terminal::mount(|| {
      Container::new()
        .set_child(
          Container::new()
            .set_height(3)
            .set_overflow(Overflow::ScrollIfNeeded)
            .set_child(Column::new().derive_children(
              || 0..6,
              |index| *index,
              move |index| {
                let log = Rc::clone(&log);

                Container::new()
                  .handle_click(move || log.borrow_mut().push(index))
                  .set_child(Text::new(format!("item {index}")))
              },
            )),
        )
        .into_any()
        .build()
    });

    let first = testing::render_to_text(12, 4);

    assert!(first.contains("item 0") && first.contains("item 2"));
    assert!(!first.contains("item 3"));
    assert!(first.contains('█'));

    testing::trigger_mouse_scroll(2, 1, 0, 2);

    let scrolled = testing::render_to_text(12, 4);

    assert!(!scrolled.contains("item 1"));
    assert!(scrolled.contains("item 2") && scrolled.contains("item 4"));
    assert!(!scrolled.contains("item 5"));

    testing::trigger_mouse_click(2, 1);

    assert_eq!(*clicked.borrow(), vec![3]);
  });
}

/// A wheel's turn reaches the node under the pointer, in rows, before the box around it
/// scrolls by it, and a behavior taking the turn keeps the scroll from happening.
#[test]
fn a_wheel_reaches_the_node_under_it_before_it_scrolls() {
  /// Takes every turn for itself.
  struct Dial;

  impl terminal::Behavior<terminal::Terminal> for Dial {
    fn native(&self) -> Option<terminal::NativeElement> {
      Some(terminal::NativeElement::Button)
    }

    fn role(&self) -> Option<terminal::Role> {
      None
    }

    fn listens(&self) -> &'static [skene_foundation::EventKind] {
      &[skene_foundation::EventKind::Wheel]
    }

    fn on_wheel(
      &self,
      _: &skene_foundation::WheelEvent,
      _: &terminal::ControlHandle,
    ) -> terminal::Prevent {
      terminal::Prevent::All
    }
  }

  with_platform(|| {
    let turns = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&turns);

    let _ = terminal::mount(move || {
      Container::new()
        .set_height(3)
        .set_overflow(Overflow::ScrollIfNeeded)
        .set_child(
          Column::new().set_children((
            terminal::Control::new(Dial).set_height(1).set_label("dial"),
            Container::new()
              .handle_wheel(move |event| log.borrow_mut().push((event.delta, event.unit)))
              .set_child(
                Column::new().set_children(
                  (0..6)
                    .map(|index| Text::new(format!("item {index}")).into_any())
                    .collect::<Vec<_>>(),
                ),
              ),
          )),
        )
        .into_any()
        .build()
    });
    let first = testing::render_to_text(12, 3);

    assert!(first.contains("dial"));

    testing::trigger_mouse_scroll(1, 0, 0, 2);

    assert_eq!(
      testing::render_to_text(12, 3),
      first,
      "the dial takes the turn"
    );

    testing::trigger_mouse_scroll(1, 1, 0, 2);

    assert_eq!(
      *turns.borrow(),
      [(Axes::new(0., 2.), skene_foundation::WheelUnit::Lines)]
    );
    assert!(
      !testing::render_to_text(12, 3).contains("dial"),
      "the box scrolled"
    );
  });
}

/// A box scrolling on one axis alone keeps the other clipped and still under the wheel.
#[test]
fn overflow_scrolls_on_its_own_axis_alone() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_child(
          Container::new()
            .set_width(8)
            .set_height(3)
            .set_overflow(Axes::new(Overflow::Clip, Overflow::ScrollIfNeeded))
            .set_child(
              Container::new().set_width(20).set_child(
                Column::new().set_children(
                  (0..6)
                    .map(|index| Text::new(format!("row {index} runs long")).into_any())
                    .collect::<Vec<_>>(),
                ),
              ),
            ),
        )
        .into_any()
        .build()
    });

    let first = testing::render_to_text(12, 4);

    assert!(first.contains("row 0") && !first.contains("row 3"));

    testing::trigger_mouse_scroll(2, 1, 4, 0);

    assert_eq!(testing::render_to_text(12, 4), first);

    testing::trigger_mouse_scroll(2, 1, 0, 2);

    let scrolled = testing::render_to_text(12, 4);

    assert!(scrolled.contains("row 2") && !scrolled.contains("row 1"));
  });
}

/// Focusing a control below the fold scrolls it into view.
#[test]
fn focus_scrolls_into_view() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_child(
          Container::new()
            .set_height(3)
            .set_overflow(Overflow::Scroll)
            .set_child(
              Column::new().set_children(
                (0..6)
                  .map(|index| {
                    BasicButton::new(Activate::new())
                      .set_label(format!("button {index}"))
                      .into_any()
                  })
                  .collect::<Vec<_>>(),
              ),
            ),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 4);

    for _ in 0..5 {
      testing::trigger_key("Tab");
    }

    let frame = testing::render_to_text(12, 4);

    assert!(frame.contains("button 4"), "{frame}");
    assert!(!frame.contains("button 0"), "{frame}");
  });
}

/// A lazy column builds only the visible slice plus overscan, and scrolling shifts
/// that window while the content stays in place.
#[test]
fn lazy_column_windows_its_items() {
  with_platform(|| {
    let built = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&built);

    let _ = terminal::mount(|| {
      Container::new()
        .set_height(4)
        .set_child(
          LazyColumn::new()
            .set_item_height(1)
            .set_overscan(2)
            .derive_items(
              || 0..1000,
              |index| *index,
              move |index| {
                log.borrow_mut().push(index);

                Text::new(format!("row {index}"))
              },
            ),
        )
        .into_any()
        .build()
    });

    let first = testing::render_to_text(12, 6);

    assert!(
      first.contains("row 0") && first.contains("row 3"),
      "{first}"
    );
    assert!(!first.contains("row 7"), "{first}");

    testing::trigger_mouse_scroll(2, 1, 0, 500);

    let scrolled = testing::render_to_text(12, 6);

    assert!(
      scrolled.contains("row 500") && scrolled.contains("row 503"),
      "{scrolled}"
    );

    let log = built.borrow();
    let max_built = log.iter().copied().max().unwrap_or(0);

    assert!(
      log.len() < 30,
      "built {} items instead of a window",
      log.len()
    );
    assert!(max_built < 520);
    assert!(log.iter().any(|index| *index >= 490));
  });
}

/// A scrollbar responds to the mouse: a press on the track jumps the thumb there,
/// dragging follows the pointer, and a mid-scroll thumb paints in eighth-cell blocks.
#[test]
fn scrollbar_jumps_and_drags() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(4)
        .set_overflow(Overflow::ScrollIfNeeded)
        .set_child(
          Column::new().set_children(
            (0..12)
              .map(|index| Text::new(format!("r{index}")))
              .collect::<Vec<_>>(),
          ),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(6, 4);

    testing::trigger_mouse_press(5, 3);

    let jumped = testing::render_to_text(6, 4);

    assert!(
      jumped.contains("r8") && !jumped.contains("r0\n"),
      "the track press scrolled to the end: {jumped}"
    );

    testing::trigger_mouse_drag(5, 1);

    let dragged = testing::render_to_text(6, 4);
    let colors = testing::render_to_colors(6, 4);

    testing::trigger_mouse_release(5, 1);

    assert!(
      dragged.contains("r3") && !dragged.contains("r8"),
      "the drag scrolled back up, the thumb's start on the pointer's row: {dragged}"
    );
    assert!(
      dragged
        .chars()
        .any(|character| ('▁'..='▇').contains(&character)),
      "a mid-scroll thumb paints in eighth blocks: {dragged}"
    );

    let track: Vec<_> = (0..4).map(|row| colors[row * 6 + 5]).collect();
    let track_color = track
      .iter()
      .find_map(|(_, background)| *background)
      .expect("a track background");

    assert!(
      track.iter().all(|(foreground, background)| {
        *background == Some(track_color) || *foreground == Some(track_color)
      }),
      "the track color shows in every bar cell: {track:?}"
    );
  });
}

/// A thumb drag reads the pointer within its cell: on a track short against its
/// content, a pointer a quarter cell earlier or later lands the offset rows apart.
#[test]
fn a_scrollbar_drag_reads_the_pointer_within_its_cell() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(4)
        .set_overflow(Overflow::ScrollIfNeeded)
        .set_child(
          Column::new().set_children(
            (0..44)
              .map(|index| Text::new(format!("r{index}")))
              .collect::<Vec<_>>(),
          ),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(6, 4);

    testing::trigger_mouse_press(5, 0);

    let first_row = |frame: String| {
      frame
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
        .map(str::to_owned)
    };

    testing::trigger_mouse_drag_within(5, 1, (0.5, 0.25));

    let early = first_row(testing::render_to_text(6, 4));

    testing::trigger_mouse_drag(5, 1);

    let center = first_row(testing::render_to_text(6, 4));

    testing::trigger_mouse_drag_within(5, 1, (0.5, 0.75));

    let late = first_row(testing::render_to_text(6, 4));

    testing::trigger_mouse_release(5, 1);

    assert_eq!(early.as_deref(), Some("r10"), "a quarter cell in");
    assert_eq!(center.as_deref(), Some("r13"), "the cell's center");
    assert_eq!(late.as_deref(), Some("r17"), "three quarters in");
  });
}

/// The wheel's target latches until the pointer moves: an inner scrollable that merely
/// slides under the unmoved pointer never steals the scroll, and a real move
/// re-targets.
#[test]
fn wheel_target_latches_until_the_pointer_moves() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_child(
          Container::new()
            .set_height(4)
            .set_overflow(Overflow::ScrollIfNeeded)
            .set_child(
              Column::new().set_children((
                Text::new("top0"),
                Text::new("top1"),
                Text::new("top2"),
                Text::new("top3"),
                Container::new()
                  .set_height(4)
                  .set_min_height(4)
                  .set_width(10)
                  .set_overflow(Overflow::ScrollIfNeeded)
                  .set_child(
                    Column::new().set_children(
                      (0..8)
                        .map(|index| Text::new(format!("inner{index}")))
                        .collect::<Vec<_>>(),
                    ),
                  ),
                Text::new("bottom0"),
                Text::new("bottom1"),
                Text::new("bottom2"),
                Text::new("bottom3"),
              )),
            ),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(14, 5);

    testing::trigger_mouse_scroll(2, 1, 0, 4);
    testing::trigger_mouse_scroll(2, 1, 0, 2);

    let latched = testing::render_to_text(14, 5);

    assert!(
      latched.contains("bottom0") && !latched.contains("inner4"),
      "the second wheel kept the page as its target: {latched}"
    );

    testing::trigger_mouse_move(8, 3);
    testing::trigger_mouse_move(2, 1);
    testing::trigger_mouse_scroll(2, 1, 0, 1);

    let retargeted = testing::render_to_text(14, 5);

    assert!(
      retargeted.contains("inner4"),
      "after the move, the wheel scrolled the inner box: {retargeted}"
    );
  });
}

/// A wheel gesture holds its target at the target's limits—reaching the inner box's
/// end never chains mid-gesture—and a paused, fresh gesture chains past the at-limit
/// box to the page, as on the web.
#[test]
fn wheel_gesture_holds_at_the_target_limit_then_chains() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_child(
          Container::new()
            .set_height(4)
            .set_overflow(Overflow::ScrollIfNeeded)
            .set_child(
              Column::new().set_children((
                Container::new()
                  .set_height(2)
                  .set_min_height(2)
                  .set_width(10)
                  .set_overflow(Overflow::ScrollIfNeeded)
                  .set_child(
                    Column::new().set_children(
                      (0..4)
                        .map(|index| Text::new(format!("inner{index}")))
                        .collect::<Vec<_>>(),
                    ),
                  ),
                Text::new("bottom0"),
                Text::new("bottom1"),
                Text::new("bottom2"),
                Text::new("bottom3"),
                Text::new("bottom4"),
                Text::new("bottom5"),
              )),
            ),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(14, 5);

    testing::trigger_mouse_scroll(1, 0, 0, 2);
    testing::trigger_mouse_scroll(1, 0, 0, 2);

    let held = testing::render_to_text(14, 5);

    assert!(
      held.contains("inner2") && held.contains("bottom0"),
      "the at-limit inner box consumed the wheel: {held}"
    );

    testing::trigger_frame(std::time::Duration::from_millis(510));
    testing::trigger_mouse_scroll(1, 0, 0, 1);

    let chained = testing::render_to_text(14, 5);

    assert!(
      !chained.contains("inner2") && chained.contains("bottom2"),
      "the fresh gesture chained to the page: {chained}"
    );
  });
}

/// The page keys scroll the nearest scrollable box around the focus, or the first, by
/// the rows it shows less one.
#[test]
fn page_keys_scroll_by_a_viewport_less_a_row() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_height(4)
        .set_overflow(Overflow::ScrollIfNeeded)
        .set_child(
          Column::new().set_children(
            (0..9)
              .map(|index| Text::new(format!("r{index}")))
              .collect::<Vec<_>>(),
          ),
        )
        .into_any()
        .build()
    });

    assert!(testing::render_to_text(6, 4).starts_with("r0"));

    testing::trigger_key("PageDown");

    assert!(testing::render_to_text(6, 4).starts_with("r3"));

    testing::trigger_key("PageUp");

    assert!(testing::render_to_text(6, 4).starts_with("r0"));
  });
}

/// Content overflowing a box takes a click where it is drawn, as in a browser, unless
/// the box clips its overflow.
#[test]
fn overflowing_content_takes_a_click_where_it_is_drawn() {
  for (overflow, expected) in [(Overflow::Visible, vec![1]), (Overflow::Clip, vec![])] {
    with_platform(|| {
      let clicked = Rc::new(RefCell::new(Vec::new()));
      let log = Rc::clone(&clicked);

      let _ = terminal::mount(move || {
        Container::new()
          .set_child(
            Container::new()
              .set_height(1)
              .set_overflow(overflow)
              .set_child(Column::new().derive_children(
                || 0..2,
                |index| *index,
                move |index| {
                  let log = Rc::clone(&log);

                  Container::new()
                    .handle_click(move || log.borrow_mut().push(index))
                    .set_child(Text::new(format!("item {index}")))
                },
              )),
          )
          .into_any()
          .build()
      });

      let frame = testing::render_to_text(12, 3);

      assert_eq!(frame.contains("item 1"), overflow == Overflow::Visible);

      testing::trigger_mouse_click(2, 1);

      assert_eq!(*clicked.borrow(), expected, "{overflow:?}");
    });
  }
}
