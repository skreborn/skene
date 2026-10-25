//! The pointer in cells: coordinates, hover and its handlers, the cursor shapes, links,
//! clicks completing on release, a hover repainting only its rows, the node a press
//! lands on, and a box sized by its hover.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  time::Duration,
};

use skene_simple_terminal::prelude::*;
use skene_terminal::{
  self as terminal,
  Handler,
  PointerButton,
  PointerButtons,
  PointerEvent,
  PointerInfo,
  PointerType,
  Prevent,
  Scene,
  behavior::{Activate, TextEntry},
  platform::terminal::Terminal,
  prelude::*,
  testing,
};

use crate::common::{blue, colors_at, green, red, rgb, with_platform};

/// Pointer events reach handlers along the hit path with terminal and view-relative
/// positions, at the cell's center when the terminal reports cells alone.
#[test]
fn pointer_events_carry_cell_coordinates() {
  with_platform(|| {
    let down = Rc::new(Cell::new(None));
    let moved = Rc::new(Cell::new(None));
    let (pressed, hovered) = (Rc::clone(&down), Rc::clone(&moved));

    let _ = terminal::mount(|| {
      Row::new()
        .set_children((
          Container::new().set_width(4).set_height(3),
          Container::new()
            .set_width(6)
            .set_height(3)
            .handle_pointer_down(move |event| {
              pressed.set(Some((
                event.x,
                event.y,
                event.local_x,
                event.local_y,
                event.button,
              )));
            })
            .handle_pointer_move(move |event| {
              hovered.set(Some((event.local_x, event.local_y, event.button)));
            }),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 4);

    testing::trigger_mouse_click(6, 1);
    testing::trigger_mouse_move(7, 2);

    assert_eq!(
      down.get(),
      Some((6.5, 1.5, 2.5, 1.5, Some(PointerButton::Primary)))
    );
    assert_eq!(moved.get(), Some((3.5, 2.5, None)));
  });
}

/// The terminal's pointer is the primary mouse: a press holds its button at a mouse's
/// pressure, a drag's move changes no button and holds the pressed one, the release
/// holds none, and a context menu the keyboard opens names no pointer.
#[test]
fn the_terminals_pointer_is_the_primary_mouse() {
  with_platform(|| {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (down, moved, up) = (Rc::clone(&seen), Rc::clone(&seen), Rc::clone(&seen));
    let node = terminal::mount(move || {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_focus_order(FocusOrder::Auto)
        .handle_pointer_down(move |event| down.borrow_mut().push(*event))
        .handle_pointer_move(move |event| moved.borrow_mut().push(*event))
        .handle_pointer_up(move |event| up.borrow_mut().push(*event))
        .into_any()
        .build()
    });
    let requested = Rc::new(Cell::new(None));
    let menu = Rc::clone(&requested);

    Scene::<Terminal>::with(|scene| {
      scene.handle(
        node,
        Handler::ContextMenu(Rc::new(move |event: &PointerEvent| {
          menu.set(Some(event.pointer));

          Prevent::None
        })),
      );
    });

    let _ = testing::render_to_text(12, 4);

    testing::trigger_mouse_press(1, 1);
    testing::trigger_mouse_drag(2, 1);
    testing::trigger_mouse_release(2, 1);
    testing::trigger_key("Shift+F10");

    let mouse = PointerEvent::mouse(0., 0.).pointer;
    let seen = seen.borrow();
    let states: Vec<_> = seen
      .iter()
      .map(|event| (event.pointer, event.button, event.buttons, event.pressure))
      .collect();

    assert_eq!(
      states,
      [
        (
          mouse,
          Some(PointerButton::Primary),
          PointerButton::Primary.into(),
          0.5
        ),
        (mouse, None, PointerButton::Primary.into(), 0.5),
        (
          mouse,
          Some(PointerButton::Primary),
          PointerButtons::new(),
          0.
        ),
      ]
    );
    assert_eq!(
      requested.get(),
      Some(PointerInfo {
        pointer_id: None,
        persistent_device_id: None,
        pointer_type: PointerType::Unknown,
      })
    );
  });
}

/// A press on a box declaring drag axes keeps its moves and its release with the box
/// wherever the pointer goes; a press on a box without them leaves each to the node
/// under the pointer, as on the web.
#[test]
fn only_a_dragging_box_keeps_a_press() {
  with_platform(|| {
    let log = Rc::new(RefCell::new(Vec::new()));
    let note = |name: &'static str| {
      let log = Rc::clone(&log);

      move |_: &PointerEvent| log.borrow_mut().push(name)
    };
    let (dragging_move, dragging_up) = (note("dragging move"), note("dragging up"));
    let (plain_move, plain_up) = (note("plain move"), note("plain up"));

    let _ = terminal::mount(move || {
      Row::new()
        .set_children((
          Container::new()
            .set_width(4)
            .set_height(2)
            .set_drag_axes(DragAxes::Horizontal)
            .handle_pointer_move(dragging_move)
            .handle_pointer_up(dragging_up),
          Container::new()
            .set_width(4)
            .set_height(2)
            .handle_pointer_move(plain_move)
            .handle_pointer_up(plain_up),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 3);

    testing::trigger_mouse_press(1, 0);
    testing::trigger_mouse_drag(6, 0);
    testing::trigger_mouse_release(6, 0);

    assert_eq!(*log.borrow(), ["dragging move", "dragging up"]);

    log.borrow_mut().clear();
    testing::trigger_mouse_press(5, 0);
    testing::trigger_mouse_drag(1, 0);
    testing::trigger_mouse_release(1, 0);

    assert_eq!(*log.borrow(), ["dragging move", "dragging up"]);
  });
}

/// Two quick clicks at one cell are a double click: the second click reaches the
/// double-click handler after its own, and a click at another cell, or after the
/// multi-click window has passed on the platform's clock, starts afresh.
#[test]
fn two_quick_clicks_at_one_cell_are_a_double_click() {
  with_platform(|| {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (clicked, doubled) = (Rc::clone(&log), Rc::clone(&log));

    let _ = terminal::mount(move || {
      Container::new()
        .set_width(4)
        .set_height(2)
        .handle_click(move || clicked.borrow_mut().push("click"))
        .handle_double_click(move || doubled.borrow_mut().push("double click"))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 3);

    testing::trigger_mouse_click(1, 0);
    testing::trigger_mouse_click(1, 0);
    testing::trigger_mouse_click(2, 1);

    assert_eq!(*log.borrow(), ["click", "click", "double click", "click"]);

    log.borrow_mut().clear();
    testing::trigger_frame(Duration::from_millis(600));
    testing::trigger_mouse_click(2, 1);

    assert_eq!(
      *log.borrow(),
      ["click"],
      "a click past the multi-click window starts afresh"
    );
  });
}

/// A press on a box offering drag data, its content's cells included, starts a drag
/// once the pointer leaves the cell, cancelling the press: the source holds
/// `Dragged` while the drag lasts, the box under the pointer taking what it carries
/// holds `Targeted` and one taking another type does not, and the release drops the
/// drag there without a click.
#[test]
fn a_drag_moves_between_boxes_and_drops_on_release() {
  with_platform(|| {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (clicked, cancelled, dropped) = (Rc::clone(&log), Rc::clone(&log), Rc::clone(&log));

    let _ = terminal::mount(move || {
      Row::new()
        .set_children((
          Container::new()
            .set_width(4)
            .set_height(2)
            .set_background(green().when(State::Dragged, blue()))
            .set_drag_data(|| terminal::DragData::new(String::from("card")))
            .handle_click(move || clicked.borrow_mut().push(String::from("click")))
            .handle_pointer_cancel(move |_| cancelled.borrow_mut().push(String::from("cancel")))
            .set_child(Container::new().set_width(2).set_height(1)),
          Container::new()
            .set_width(4)
            .set_height(2)
            .set_background(red().when(State::Targeted, blue()))
            .handle_drop(move |card: &String| dropped.borrow_mut().push(card.clone())),
          Container::new()
            .set_width(4)
            .set_height(2)
            .set_background(red().when(State::Targeted, blue()))
            .handle_drop(|_: &u32| {}),
        ))
        .into_any()
        .build()
    });
    let source = || colors_at(14, 3, 3, 0).1;
    let target = || colors_at(14, 3, 5, 0).1;
    let other = || colors_at(14, 3, 9, 0).1;

    assert_eq!(target(), Some(rgb(red())));

    testing::trigger_mouse_press(1, 0);
    testing::trigger_mouse_drag(1, 0);

    assert_eq!(
      *log.borrow(),
      Vec::<String>::new(),
      "no drag within the cell"
    );
    assert_eq!(source(), Some(rgb(green())));

    testing::trigger_mouse_drag(9, 0);

    assert_eq!(source(), Some(rgb(blue())), "the source is dragged");
    assert_eq!(
      other(),
      Some(rgb(red())),
      "a box taking another type is no target"
    );

    testing::trigger_mouse_drag(5, 0);

    assert_eq!(target(), Some(rgb(blue())));
    assert_eq!(other(), Some(rgb(red())));

    testing::trigger_mouse_release(5, 0);

    assert_eq!(*log.borrow(), ["cancel", "card"]);
    assert_eq!(target(), Some(rgb(red())));
    assert_eq!(source(), Some(rgb(green())), "the drag has ended");

    testing::trigger_mouse_click(1, 0);

    assert_eq!(
      *log.borrow(),
      ["cancel", "card", "click"],
      "a press that stays clicks"
    );
  });
}

/// The mouse hovering a node applies its `Hovered` variant and fires the enter handler;
/// moving off reverts and fires the leave handler, once each.
#[test]
fn hover_variant_and_pointer_handlers() {
  with_platform(|| {
    let enters = Rc::new(Cell::new(0u32));
    let leaves = Rc::new(Cell::new(0u32));
    let (entered, left) = (Rc::clone(&enters), Rc::clone(&leaves));

    let _ = terminal::mount(|| {
      Row::new()
        .set_children((
          Container::new()
            .set_width(4)
            .set_height(2)
            .set_background(red().when(State::Hovered, blue()))
            .handle_pointer_enter(move || entered.set(entered.get() + 1))
            .handle_pointer_leave(move || left.set(left.get() + 1)),
          Container::new().set_width(4).set_height(2),
        ))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(10, 3, 0, 0).1, Some(rgb(red())));

    testing::trigger_mouse_move(1, 0);
    testing::trigger_mouse_move(2, 1);

    assert_eq!(colors_at(10, 3, 0, 0).1, Some(rgb(blue())));
    assert_eq!((enters.get(), leaves.get()), (1, 0));

    testing::trigger_mouse_move(6, 0);

    assert_eq!(colors_at(10, 3, 0, 0).1, Some(rgb(red())));
    assert_eq!((enters.get(), leaves.get()), (1, 1));
  });
}

/// The pointer over a node holds `Hovered` on its ancestors too, as `:hover` does, and
/// leaving the node for the box around it keeps that box hovered.
#[test]
fn hover_holds_on_the_ancestors_of_the_hovered_node() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(2)
        .set_background(red().when(State::Hovered, blue()))
        .set_child(
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(blue().when(State::Hovered, red())),
        )
        .into_any()
        .build()
    });
    let colors = || (colors_at(8, 3, 4, 1).1, colors_at(8, 3, 0, 0).1);

    assert_eq!(colors(), (Some(rgb(red())), Some(rgb(blue()))));

    testing::trigger_mouse_move(0, 0);

    assert_eq!(colors(), (Some(rgb(blue())), Some(rgb(red()))));

    testing::trigger_mouse_move(4, 1);

    assert_eq!(colors(), (Some(rgb(blue())), Some(rgb(blue()))));

    testing::trigger_mouse_move(7, 2);

    assert_eq!(colors(), (Some(rgb(red())), Some(rgb(blue()))));
  });
}

/// The pointer over an open overlay holds `Hovered` on the boxes around the overlay in
/// the tree too, as a browser's `:hover` holds on a popover's ancestors.
#[test]
fn hover_over_an_overlay_holds_on_its_ancestors() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(10)
        .set_height(3)
        .set_background(red().when(State::Hovered, blue()))
        .set_child(
          Overlay::popup()
            .set_placement(Placement::TopStart)
            .set_open(true)
            .set_child(Container::new().set_width(3).set_height(1)),
        )
        .into_any()
        .build()
    });

    assert_eq!(colors_at(10, 3, 8, 2).1, Some(rgb(red())));

    testing::trigger_mouse_move(1, 0);

    assert_eq!(colors_at(10, 3, 8, 2).1, Some(rgb(blue())));
  });
}

/// The hover cursor defaults follow the content: the pointer over buttons and toggling
/// controls, the I-beam over entries and selectable text, and none over anything
/// else—an explicit style still winning.
#[test]
fn hover_cursor_defaults_follow_the_content() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("press")
            .into_any(),
          SimpleCheckbox::new().set_label("check").into_any(),
          BasicTextField::new(TextEntry::new()).into_any(),
          Text::new("words").set_selectable(true).into_any(),
          Text::new("chrome").into_any(),
          BasicButton::new(Activate::new())
            .set_cursor(MouseCursor::Default)
            .set_label("styled")
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let frame = testing::render_to_text(20, 8);
    let cursors: Vec<_> = (0..6)
      .map(|row| {
        testing::trigger_mouse_move(1, row);

        testing::mouse_cursor()
      })
      .collect();

    assert_eq!(
      cursors,
      vec![
        Some(MouseCursor::Click),
        Some(MouseCursor::Click),
        Some(MouseCursor::Text),
        Some(MouseCursor::Text),
        None,
        Some(MouseCursor::Default),
      ],
      "{frame}"
    );
  });
}

/// A click on a linked span stages its destination for the system handler; a click
/// beside it stages nothing, and the link shows the pointing hand.
#[test]
fn link_click_opens_the_destination() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      RichText::new()
        .add_spans([
          Span::new("read the "),
          Span::new("docs").set_link("https://example.com/docs"),
        ])
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 2);

    testing::trigger_mouse_move(10, 0);

    assert_eq!(testing::mouse_cursor(), Some(MouseCursor::Click));

    testing::trigger_mouse_click(10, 0);

    assert_eq!(
      testing::take_opened().as_deref(),
      Some("https://example.com/docs")
    );

    testing::trigger_mouse_click(2, 0);

    assert_eq!(testing::take_opened(), None);
  });
}

/// A click stops at the view that took it: a box says so with its verdict, and an
/// ancestor never sees it.
#[test]
fn a_click_stops_at_the_view_that_took_it() {
  with_platform(|| {
    let inner_runs = Rc::new(Cell::new(0u32));
    let outer_runs = Rc::new(Cell::new(0u32));
    let (inner, outer) = (Rc::clone(&inner_runs), Rc::clone(&outer_runs));

    let _ = terminal::mount(|| {
      Container::new()
        .handle_click(move || outer.set(outer.get() + 1))
        .set_child(
          Container::new()
            .handle_click(move || {
              inner.set(inner.get() + 1);

              Bubbling::Stop
            })
            .set_child(Text::new("xx")),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(6, 1);

    testing::trigger_mouse_click(0, 0);

    assert_eq!(inner_runs.get(), 1);
    assert_eq!(outer_runs.get(), 0, "the box that handled it kept it");
  });
}

/// A click completes on release over the same target: pressing one button and
/// releasing over another—or over nothing—clicks neither.
#[test]
fn clicks_complete_on_release_over_the_same_target() {
  with_platform(|| {
    let first = Rc::new(Cell::new(0u32));
    let second = Rc::new(Cell::new(0u32));
    let (one, two) = (Rc::clone(&first), Rc::clone(&second));

    let _ = terminal::mount(|| {
      Row::new()
        .set_spacing(2)
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("one")
            .handle_activate(move || one.set(one.get() + 1)),
          BasicButton::new(Activate::new())
            .set_label("two")
            .handle_activate(move || two.set(two.get() + 1)),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 3);

    testing::trigger_mouse_press(1, 0);
    testing::trigger_mouse_release(6, 0);
    testing::trigger_mouse_press(1, 0);
    testing::trigger_mouse_release(15, 2);
    testing::trigger_mouse_press(1, 0);
    testing::trigger_mouse_release(1, 0);

    assert_eq!(first.get(), 1);
    assert_eq!(second.get(), 0);
  });
}

/// A release with no delivered press still clicks: VTE terminals swallow every rapid
/// press past the third yet forward the release, so the orphan release stands in.
#[test]
fn an_orphan_release_stands_in_for_a_swallowed_click() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = terminal::mount(|| {
      BasicButton::new(Activate::new())
        .set_label("go")
        .handle_activate(move || counted.set(counted.get() + 1))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 3);

    for _ in 0..3 {
      testing::trigger_mouse_click(1, 0);
    }

    for _ in 0..3 {
      testing::trigger_mouse_release(1, 0);
    }

    assert_eq!(clicks.get(), 6);
  });
}

/// A hover change over one box repaints only the rows that box covers; a full frame
/// repaints every row.
#[test]
fn a_hover_change_repaints_only_its_rows() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children(
          (0..3)
            .map(|index| {
              Container::new()
                .set_height(1)
                .set_background(red().when(State::Hovered, blue()))
                .set_child(Text::new(format!("row {index}")))
            })
            .collect::<Vec<_>>(),
        )
        .into_any()
        .build()
    });

    let _ = testing::render_to_text(10, 5);

    assert_eq!(testing::repainted_rows(), vec![0, 1, 2, 3, 4]);

    testing::trigger_mouse_move(1, 1);

    let frame = testing::render_to_text(10, 5);

    assert_eq!(frame, "row 0\nrow 1\nrow 2\n\n\n");
    assert_eq!(testing::repainted_rows(), vec![1]);
    assert_eq!(colors_at(10, 5, 0, 1).1, Some(rgb(blue())));

    testing::trigger_mouse_move(1, 2);

    let _ = testing::render_to_text(10, 5);

    assert_eq!(testing::repainted_rows(), vec![1, 2]);
    assert_eq!(colors_at(10, 5, 0, 1).1, Some(rgb(red())));
    assert_eq!(colors_at(10, 5, 0, 2).1, Some(rgb(blue())));

    testing::trigger_mouse_move(1, 4);

    let _ = testing::render_to_text(10, 5);

    assert_eq!(testing::repainted_rows(), vec![2]);
  });
}

/// A press lands on the topmost node covering its cell: a child above its parent, a
/// stack's later layer above an earlier one, the earlier beside it, and the page's box,
/// filling the terminal, wherever nothing above it lies.
#[test]
fn a_point_hits_the_topmost_node() {
  with_platform(|| {
    let hits = Rc::new(RefCell::new(Vec::new()));
    let named = |name: &'static str| {
      let hits = Rc::clone(&hits);

      move || {
        hits.borrow_mut().push(name);

        Bubbling::Stop
      }
    };

    let _ = terminal::mount(|| {
      Container::new()
        .handle_click(named("page"))
        .set_child(
          Column::new().set_children((
            Container::new()
              .set_width(10)
              .set_height(2)
              .handle_click(named("first")),
            Stack::new().set_children((
              Container::new()
                .set_width(10)
                .set_height(2)
                .handle_click(named("under")),
              Container::new()
                .set_width(5)
                .set_height(1)
                .handle_click(named("over")),
            )),
          )),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 8);

    for (x, y) in [(1, 0), (1, 2), (7, 2), (7, 3), (15, 0), (1, 7)] {
      testing::trigger_mouse_click(x, y);
    }

    assert_eq!(
      *hits.borrow(),
      ["first", "over", "under", "under", "page", "page"]
    );
  });
}

/// A box clipping its overflow hides what lies beneath it beyond its padding box from a
/// press, its own border still taking it and the box around it beyond.
#[test]
fn a_clipping_box_hides_its_overflow_from_a_hit() {
  with_platform(|| {
    let hits = Rc::new(RefCell::new(Vec::new()));
    let named = |name: &'static str| {
      let hits = Rc::clone(&hits);

      move || {
        hits.borrow_mut().push(name);

        Bubbling::Stop
      }
    };

    let _ = terminal::mount(|| {
      Container::new()
        .set_width(20)
        .set_height(8)
        .handle_click(named("outer"))
        .set_child(
          Container::new()
            .set_width(10)
            .set_height(3)
            .set_border_width(Inset::all(1))
            .set_overflow(Overflow::Clip)
            .handle_click(named("clipping"))
            .set_child(
              Container::new()
                .set_width(8)
                .set_height(6)
                .handle_click(named("overflowing")),
            ),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 8);

    for (x, y) in [(2, 1), (2, 2), (2, 4)] {
      testing::trigger_mouse_click(x, y);
    }

    assert_eq!(*hits.borrow(), ["overflowing", "clipping", "outer"]);
  });
}

/// A box sized by its hover lays out afresh as the pointer enters and leaves it, the
/// box after it moving with it.
#[test]
fn a_box_sized_by_its_hover_lays_out_as_it_moves() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Row::new()
        .set_children((
          Container::new()
            .set_width(2.when(State::Hovered, 6))
            .set_height(1)
            .set_background(red()),
          Text::new("|"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(10, 2), "  |\n\n");

    testing::trigger_mouse_move(0, 0);

    assert_eq!(testing::render_to_text(10, 2), "      |\n\n");
    assert_eq!(colors_at(10, 2, 5, 0).1, Some(rgb(red())));

    testing::trigger_mouse_move(0, 1);

    assert_eq!(testing::render_to_text(10, 2), "  |\n\n");
  });
}
