//! Overlays in cells: a modal following its signal without rebuilding, its scrim,
//! blocking and dismissal, a dismiss as a request the app answers, a popup's anchoring,
//! re-placing and light dismissal, focus trapped inside a modal, taken by it on open and
//! returned on close, a closed overlay's tab stops passed over, and a focus handle moving
//! focus to the view it is bound to.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_simple_terminal::prelude::SimplePalette;
use skene_terminal::{self as terminal, Activate, HasChild, prelude::*, testing};

use crate::common::{black, blue, colors_at, red, rgb, with_platform};

/// A modal follows its bound signal, centered over the app; closing and reopening keep
/// its content, so a signal created while building the content keeps its value across a
/// close and the content is built once.
#[test]
fn a_modal_opens_and_closes_through_its_signal_without_rebuilding() {
  with_platform(|| {
    let open = Signal::new(false);
    let content = NodeHandle::new();
    let built = Rc::new(Cell::new(0u32));
    let inner: Rc<Cell<Option<Signal<i32>>>> = Rc::new(Cell::new(None));

    let _ = terminal::mount({
      let (built, inner) = (Rc::clone(&built), Rc::clone(&inner));

      move || {
        Column::new()
          .set_children((
            Text::new("page").into_any(),
            Overlay::modal()
              .bind_open(open)
              .set_child(AnyView::new(move || {
                let count = Signal::new(1);

                built.set(built.get() + 1);
                inner.set(Some(count));

                Container::new()
                  .bind_handle(content)
                  .derive_background(move || if count.get() > 1 { red() } else { blue() })
                  .set_child(Text::new("dialog"))
                  .build()
              }))
              .into_any(),
          ))
          .into_any()
          .build()
      }
    });

    assert!(!testing::render_to_text(20, 5).contains("dialog"));

    open.set(true);

    let frame = testing::render_to_text(20, 5);
    let (x, y) = testing::locate(&frame, "dialog");

    assert_eq!((x, y), (7, 2), "the modal sits centered: {frame}");
    assert_eq!(colors_at(20, 5, 7, 2).1, Some(rgb(blue())));

    let count = inner.get().expect("the content's signal");
    let node = content.node();

    count.set(2);

    assert_eq!(colors_at(20, 5, 7, 2).1, Some(rgb(red())));

    open.set(false);

    assert!(!testing::render_to_text(20, 5).contains("dialog"));
    assert_eq!(count.get(), 2, "the content's signal outlives the close");

    open.set(true);

    assert!(testing::render_to_text(20, 5).contains("dialog"));
    assert_eq!(colors_at(20, 5, 7, 2).1, Some(rgb(red())));
    assert_eq!(content.node(), node, "the same content node");
    assert_eq!(built.get(), 1, "the content was built once");
  });
}

/// A dismiss runs the handler and nothing else: a popup whose signal only derives its
/// openness stays open until the app writes the signal, and a bound popup closes, its
/// binding writing the signal.
#[test]
fn a_dismiss_requests_and_never_closes_by_itself() {
  with_platform(|| {
    let requested = Signal::new(true);
    let bound = Signal::new(false);
    let log = Rc::new(RefCell::new(Vec::new()));

    let _ = terminal::mount({
      let log = Rc::clone(&log);

      move || {
        Column::new()
          .set_children((
            Overlay::popup()
              .derive_open(move || requested.get())
              .handle_dismiss(move || log.borrow_mut().push("dismiss"))
              .set_child(Text::new("requested"))
              .into_any(),
            Overlay::popup()
              .bind_open(bound)
              .set_child(Text::new("bound"))
              .into_any(),
          ))
          .into_any()
          .build()
      }
    });

    assert!(testing::render_to_text(20, 5).contains("requested"));

    testing::trigger_key("Escape");

    assert_eq!(*log.borrow(), ["dismiss"]);
    assert!(requested.get());
    assert!(
      testing::render_to_text(20, 5).contains("requested"),
      "a dismiss alone closes nothing"
    );

    requested.set(false);
    bound.set(true);

    let frame = testing::render_to_text(20, 5);

    assert!(
      frame.contains("bound") && !frame.contains("requested"),
      "{frame}"
    );

    testing::trigger_key("Escape");

    assert!(!bound.get(), "the binding wrote the signal");
    assert!(!testing::render_to_text(20, 5).contains("bound"));
  });
}

/// A popup's derived placement re-places it while it stays open: below its anchor,
/// then above it.
#[test]
fn a_derived_placement_re_places_an_open_popup() {
  with_platform(|| {
    let placement = Signal::new(Placement::Bottom);
    let anchor = NodeHandle::new();

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          Text::new("top").into_any(),
          Text::new("").into_any(),
          Container::new()
            .bind_handle(anchor)
            .set_child(Text::new("anchor"))
            .into_any(),
          Overlay::popup()
            .set_anchor(anchor)
            .derive_placement(move || placement.get())
            .set_open(true)
            .set_child(Text::new("popup"))
            .into_any(),
        ))
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(12, 6);

    assert_eq!(testing::locate(&frame, "anchor").1, 2, "{frame}");
    assert_eq!(testing::locate(&frame, "popup"), (0, 3), "below: {frame}");

    placement.set(Placement::Top);

    let frame = testing::render_to_text(12, 6);

    assert_eq!(testing::locate(&frame, "popup"), (0, 1), "above: {frame}");
  });
}

/// A popup standing directly in a theme's scope, which the terminal makes no node for,
/// shows and takes presses where it is placed alone: never also where the scope stands
/// in the flow, so a press on the page there dismisses it.
#[test]
fn a_popup_in_a_themes_scope_shows_where_it_is_placed_alone() {
  with_platform(|| {
    let open = Signal::new(true);
    let anchor = NodeHandle::new();

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          Text::new("top").into_any(),
          Container::new()
            .bind_handle(anchor)
            .set_child(Text::new("anchor"))
            .into_any(),
          Theme::new(SimplePalette::default())
            .set_child_builder(move || {
              Overlay::popup()
                .set_anchor(anchor)
                .set_placement(Placement::Bottom)
                .bind_open(open)
                .set_child(Text::new("popup"))
            })
            .into_any(),
        ))
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(12, 4);

    assert_eq!(frame.matches("popup").count(), 1, "shown once: {frame}");
    assert_eq!(testing::locate(&frame, "popup"), (0, 2), "below: {frame}");
    assert_eq!(testing::locate(&frame, "top"), (0, 0), "{frame}");

    testing::trigger_mouse_click(1, 0);

    assert!(!open.get(), "a press on the page dismisses it");
  });
}

/// A press inside an open popup or on the anchor it opened from leaves it open, and a
/// press anywhere else dismisses it, as a browser's light dismiss does.
#[test]
fn a_press_inside_a_popup_or_on_its_anchor_leaves_it_open() {
  with_platform(|| {
    let open = Signal::new(false);
    let anchor = NodeHandle::new();

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          Container::new()
            .bind_handle(anchor)
            .set_child(Text::new("anchor"))
            .into_any(),
          Overlay::popup()
            .set_anchor(anchor)
            .set_placement(Placement::Bottom)
            .bind_open(open)
            .set_child(Text::new("inside"))
            .into_any(),
          Text::new("").into_any(),
          Text::new("page").into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 4);

    open.set(true);

    let frame = testing::render_to_text(12, 4);
    let (inside, anchored, page) = (
      testing::locate(&frame, "inside"),
      testing::locate(&frame, "anchor"),
      testing::locate(&frame, "page"),
    );

    testing::trigger_mouse_click(inside.0 + 1, inside.1);
    testing::trigger_mouse_click(anchored.0 + 1, anchored.1);

    assert!(open.get(), "a press inside or on the anchor leaves it open");

    testing::trigger_mouse_click(page.0 + 1, page.1);

    assert!(!open.get(), "a press outside dismisses it");
  });
}

/// A modal overlay paints over the app with its backdrop scrim, blocks clicks beneath
/// it, and `Escape` asks it to close.
#[test]
fn modal_overlay_covers_blocks_and_dismisses() {
  with_platform(|| {
    let open = Signal::new(false);
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("base")
            .handle_activate(move || counted.set(counted.get() + 1))
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

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
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

    let _ = terminal::mount(move || {
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

      let _ = terminal::mount(move || {
        let modal = Overlay::modal().bind_open(open);
        let modal = match backdrop {
          Some(color) => modal.set_backdrop(Some(color)),
          None => modal,
        };

        Column::new()
          .set_children((
            Container::new()
              .set_background(red())
              .set_height(1)
              .into_any(),
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

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("outside")
            .bind_handle(outside)
            .into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_child(
              BasicButton::new(Activate::new())
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

/// A closed overlay's tab stops are passed over, and an open one's take their place in
/// the order where the overlay stands.
#[test]
fn a_closed_overlays_tab_stops_are_passed_over() {
  with_platform(|| {
    let open = Signal::new(false);
    let (before, inside, after) = (NodeHandle::new(), NodeHandle::new(), NodeHandle::new());

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("before")
            .bind_handle(before)
            .into_any(),
          Overlay::popup()
            .bind_open(open)
            .set_child(
              BasicButton::new(Activate::new())
                .set_label("inside")
                .bind_handle(inside),
            )
            .into_any(),
          BasicButton::new(Activate::new())
            .set_label("after")
            .bind_handle(after)
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 4);

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), before.node());

    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      after.node(),
      "the closed overlay's stop is passed over"
    );

    open.set(true);

    let _ = testing::render_to_text(12, 4);

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), before.node(), "the order wraps");

    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      inside.node(),
      "the open overlay's stop stands where it does"
    );
  });
}

/// Overlays stack in the order they opened, as a browser's top layer does: of two open
/// modals, the one opened last holds the tab order, whichever was built first.
#[test]
fn the_modal_opened_last_holds_the_tab_order() {
  with_platform(|| {
    let (first_open, second_open) = (Signal::new(false), Signal::new(false));
    let (first, second) = (NodeHandle::new(), NodeHandle::new());

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          Overlay::modal()
            .bind_open(first_open)
            .set_child(
              BasicButton::new(Activate::new())
                .set_label("first")
                .bind_handle(first),
            )
            .into_any(),
          Overlay::modal()
            .bind_open(second_open)
            .set_child(
              BasicButton::new(Activate::new())
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

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("page")
            .handle_activate(move || counted.set(counted.get() + 1))
            .into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_backdrop(None)
            .set_child(
              Tooltip::new("Confirms")
                .set_child(
                  BasicButton::new(Activate::new())
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

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("open")
            .bind_handle(opener)
            .handle_activate(move || open.set(true))
            .into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_child(
              BasicButton::new(Activate::new())
                .set_label("close")
                .bind_handle(inside)
                .handle_activate(move || open.set(false)),
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

/// A focus handle bound to a control or to a box with a focus order moves focus to it
/// and takes it off again; unbound, it does nothing.
#[test]
fn a_focus_handle_moves_focus_to_its_bound_view() {
  with_platform(|| {
    let button = NodeHandle::new();
    let plain = NodeHandle::new();
    let unbound = NodeHandle::new();

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .bind_handle(button)
            .set_label("Save")
            .into_any(),
          Container::new()
            .set_focus_order(FocusOrder::Auto)
            .bind_handle(plain)
            .set_child(Text::new("box"))
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 4);

    assert_eq!(testing::focused_node(), None);

    button.focus();

    assert_eq!(testing::focused_node(), button.node());

    plain.focus();

    assert_eq!(testing::focused_node(), plain.node());

    unbound.focus();

    assert_eq!(
      testing::focused_node(),
      plain.node(),
      "unbound, nothing moves"
    );

    plain.blur();

    assert_eq!(testing::focused_node(), None);
  });
}
