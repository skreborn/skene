//! The container in cells: its handlers hear real input through dispatch, its focus
//! order and accessibility follow their signals, `Tab` walks the scene's tab stops, a
//! clickable one is an activatable row, and its border inks through the theme.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_simple_tui::prelude::*;
use skene_tui::{
  PointerButton,
  behavior::Activate,
  platform::terminal::mount,
  prelude::*,
  testing,
};

use crate::common::{colors_at, locate, mount_nodes, read, red, rgb, with_platform};

/// Every handler hears its event through dispatch: a press, a key and a release climb
/// to the outer box until a `Bubbling::Stop`, the pointer entering the inner box enters
/// the outer one first, and every other notification stays on its node.
#[expect(
  clippy::too_many_lines,
  reason = "one handler per event kind; length is the kind count, not complexity"
)]
#[test]
fn handlers_fire_through_dispatch_and_a_stop_ends_the_climb() {
  with_platform(|| {
    let log = Rc::new(RefCell::new(Vec::<&'static str>::new()));
    let note = |name: &'static str| {
      let log = Rc::clone(&log);

      move || log.borrow_mut().push(name)
    };
    let outer_down = note("outer down");
    let outer_key = note("outer key");
    let outer_key_up = note("outer key up");
    let outer_up = note("outer up");
    let outer_enter = note("outer enter");
    let inner_double = note("inner double click");
    let inner_down = note("inner down");
    let inner_key = note("inner key");
    let inner_key_up = note("inner key up");
    let inner_up = note("inner up");
    let inner_move = note("inner move");
    let inner_enter = note("inner enter");
    let inner_leave = note("inner leave");
    let inner_focus = note("inner focus");
    let inner_blur = note("inner blur");
    let inner_wheel = note("inner wheel");
    let inner_scroll = note("inner scroll");

    let _ = mount(move || {
      Column::new()
        .set_children((
          Container::new()
            .handle_pointer_down(move |_| outer_down())
            .handle_key_down(move |_| outer_key())
            .handle_key_up(move |_| outer_key_up())
            .handle_pointer_up(move |_| outer_up())
            .handle_pointer_enter(outer_enter)
            .set_child(
              Container::new()
                .set_width(6)
                .set_height(2)
                .set_overflow(Overflow::Scroll)
                .set_focus_order(FocusOrder::Auto)
                .handle_double_click(inner_double)
                .handle_pointer_down(move |event| {
                  assert_eq!(event.button, Some(PointerButton::Primary));
                  inner_down();

                  Bubbling::Stop
                })
                .handle_key_down(move |_| inner_key())
                .handle_key_up(move |event| {
                  assert!(event.is(NamedKey::Enter));
                  inner_key_up();

                  Bubbling::Stop
                })
                .handle_pointer_up(move |_| inner_up())
                .handle_pointer_move(move |_| inner_move())
                .handle_pointer_enter(inner_enter)
                .handle_pointer_leave(inner_leave)
                .handle_focus(inner_focus)
                .handle_blur(inner_blur)
                .handle_wheel(move |_| inner_wheel())
                .handle_scroll(move |event| {
                  assert!(event.y > 0., "scrolled down");
                  inner_scroll();
                })
                .set_child(
                  Column::new().set_children(
                    (0..6)
                      .map(|index| Text::new(format!("item {index}")).into_any())
                      .collect::<Vec<_>>(),
                  ),
                ),
            )
            .into_any(),
          Container::new()
            .set_height(1)
            .set_focus_order(FocusOrder::Auto)
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let taken = || {
      let _ = testing::render_to_text(10, 4);

      std::mem::take(&mut *log.borrow_mut())
    };

    let _ = taken();

    testing::trigger_mouse_move(1, 0);

    assert_eq!(taken(), ["outer enter", "inner enter", "inner move"]);

    testing::trigger_mouse_press(1, 0);

    assert_eq!(
      taken(),
      ["inner down", "inner focus"],
      "the stop ends the climb"
    );

    testing::trigger_mouse_release(1, 0);

    assert_eq!(taken(), ["inner up", "outer up"]);

    testing::trigger_mouse_click(1, 0);

    assert_eq!(
      taken(),
      ["inner down", "inner up", "outer up", "inner double click"]
    );

    testing::trigger_key_releases(true);
    testing::trigger_key("Enter");

    assert_eq!(taken(), ["inner key", "outer key"]);

    testing::trigger_key_up("Enter");

    assert_eq!(taken(), ["inner key up"], "the stop ends the climb");

    testing::trigger_mouse_scroll(1, 0, 0, 1);

    assert_eq!(taken(), ["inner wheel", "inner scroll"]);

    testing::trigger_mouse_move(8, 0);

    assert_eq!(
      taken(),
      ["inner leave"],
      "the outer box still holds the pointer"
    );

    testing::trigger_key("Tab");

    assert_eq!(
      taken(),
      ["inner key", "outer key", "inner blur"],
      "the key reaches the focus it moves"
    );
  });
}

/// A fixed focus order and accessibility hold from creation; derived ones are rewritten
/// on the same node as their signals change, so `Tab` and a reader follow them.
#[test]
fn focus_order_and_accessibility_follow_their_signals() {
  with_platform(|| {
    let order = Signal::new(FocusOrder::Skip);
    let expanded = Signal::new(false);
    let [fixed, derived] = mount_nodes(move || {
      [
        Container::new()
          .set_height(1)
          .set_focus_order(FocusOrder::Auto)
          .set_accessibility([AccessProp::Name("fixed".to_owned())])
          .build(),
        Container::new()
          .set_height(1)
          .derive_focus_order(move || order.get())
          .derive_accessibility(move || vec![AccessProp::Expanded(expanded.get())])
          .build(),
      ]
    });

    assert_eq!(read(fixed).label(), Some("fixed"));
    assert_eq!(read(derived).is_expanded(), Some(false));

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), Some(fixed));

    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      Some(fixed),
      "the skipped box is passed over"
    );

    order.set(FocusOrder::Auto);
    expanded.set(true);

    let _ = testing::render_to_text(40, 12);

    assert_eq!(read(derived).is_expanded(), Some(true));

    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      Some(derived),
      "the box takes its place in the child order"
    );
  });
}

/// `Tab` moves focus through the tab stops in the scene's order and wraps, `Shift-Tab`
/// goes back, and a hidden box's stops are passed over.
#[test]
fn tab_traverses_the_scenes_tab_order() {
  with_platform(|| {
    let [first, _, last] = mount_nodes(|| {
      [
        Container::new()
          .set_height(1)
          .set_focus_order(FocusOrder::Auto)
          .build(),
        Container::new()
          .set_hidden(true)
          .set_child(
            Container::new()
              .set_height(1)
              .set_focus_order(FocusOrder::Auto),
          )
          .build(),
        Container::new()
          .set_height(1)
          .set_focus_order(FocusOrder::Auto)
          .build(),
      ]
    });

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), Some(first));

    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      Some(last),
      "the hidden box's stop is passed over"
    );

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), Some(first), "the order wraps");

    testing::trigger_key("Shift+Tab");

    assert_eq!(testing::focused_node(), Some(last), "Shift-Tab goes back");
  });
}

/// A counter and the action adding one to it.
fn counter() -> (Rc<Cell<u32>>, impl Fn() + Clone + 'static) {
  let count = Rc::new(Cell::new(0));
  let counted = Rc::clone(&count);

  (count, move || counted.set(counted.get() + 1))
}

/// A clickable container is a tab stop activated by `Enter` on every press, repeats
/// included, and by `Space` once on its release where it kept focus since the press,
/// each activation a click its handler hears.
#[test]
fn an_activatable_row_is_activated_by_enter_and_space() {
  with_platform(|| {
    let (clicks, click) = counter();
    let [row, other] = mount_nodes(move || {
      [
        Container::new()
          .set_height(1)
          .handle_click(click)
          .set_child(Text::new("Row"))
          .build(),
        Container::new()
          .set_height(1)
          .set_focus_order(FocusOrder::Auto)
          .build(),
      ]
    });

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), Some(row), "the row is a tab stop");

    testing::trigger_key("Enter");
    testing::trigger_key_event(&KeyEvent {
      repeat: true,
      ..KeyEvent::new("Enter")
    });

    assert_eq!(clicks.get(), 2, "every Enter activates, repeats included");

    testing::trigger_key(" ");

    assert_eq!(clicks.get(), 3, "a Space activates as it is released");

    testing::trigger_key_releases(true);
    testing::trigger_key(" ");
    testing::trigger_key_event(&KeyEvent {
      repeat: true,
      ..KeyEvent::new(" ")
    });

    assert_eq!(clicks.get(), 3, "a held Space waits for its release");

    testing::trigger_key_up(" ");
    testing::trigger_key_up(" ");

    assert_eq!(clicks.get(), 4, "the release activates once");

    testing::trigger_key(" ");
    testing::trigger_key("Tab");
    testing::trigger_key_up("Tab");

    assert_eq!(testing::focused_node(), Some(other));

    testing::trigger_key_up(" ");

    assert_eq!(clicks.get(), 4, "focus left before the release");

    testing::trigger_key("Shift+Tab");
    testing::trigger_key_up("Tab");
    testing::trigger_key(" ");
    testing::trigger_key("Tab");
    testing::trigger_key_up("Tab");
    testing::trigger_key("Shift+Tab");
    testing::trigger_key_up("Tab");

    assert_eq!(testing::focused_node(), Some(row));

    testing::trigger_key_up(" ");

    assert_eq!(
      clicks.get(),
      4,
      "focus left and came back before the release"
    );
  });
}

/// A control inside an activatable row owns its presses and its keys: its click, its
/// `Enter` and its `Space` activate it and never the row, while a press on the row's
/// own text clicks the row.
#[test]
fn the_controls_inside_a_row_own_their_presses() {
  with_platform(|| {
    let (rows, row_click) = counter();
    let (buttons, button_click) = counter();

    let _ = mount(move || {
      Container::new()
        .handle_click(row_click)
        .set_child(
          Row::new().set_spacing(1).set_children((
            Text::new("Row"),
            BasicButton::new(Activate::new())
              .set_label("Inside")
              .handle_activate(button_click),
          )),
        )
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(20, 2);
    let (x, y) = locate(&frame, "Inside");

    testing::trigger_mouse_click(x, y);
    testing::trigger_key("Enter");
    testing::trigger_key(" ");

    assert_eq!(buttons.get(), 3, "the button's press and keys are its own");
    assert_eq!(rows.get(), 0, "none reaches the row");

    let (x, y) = locate(&frame, "Row");

    testing::trigger_mouse_click(x, y);
    testing::trigger_key("Enter");

    assert_eq!(rows.get(), 2, "the row's own press and key activate it");
    assert_eq!(buttons.get(), 3);
  });
}

/// An activatable row takes its place in the tab order by the scene's child order, the
/// controls it holds after it.
#[test]
fn an_activatable_row_is_a_tab_stop_in_child_order() {
  with_platform(|| {
    let inside = NodeHandle::new();
    let [before, row, after] = mount_nodes(move || {
      [
        BasicButton::new(Activate::new())
          .set_label("Before")
          .build(),
        Container::new()
          .handle_click(|| {})
          .set_child(
            BasicButton::new(Activate::new())
              .set_label("Inside")
              .bind_handle(inside),
          )
          .build(),
        BasicButton::new(Activate::new()).set_label("After").build(),
      ]
    });
    let mut order = Vec::new();

    for _ in 0..4 {
      testing::trigger_key("Tab");
      order.extend(testing::focused_node());
    }

    assert_eq!(
      order,
      [before, row, inside.node().expect("the button built"), after]
    );
  });
}

/// A border whose color is a token inks through the theme, and its width per edge
/// draws those edges alone.
#[test]
fn a_border_with_a_token_color_inks_through_the_theme() {
  with_platform(|| {
    let _ = mount(|| {
      Theme::new(SimplePalette::default().set_card(red()))
        .set_child_builder(|| {
          Container::new()
            .set_width(4)
            .set_height(3)
            .set_border_width(Inset::new(1, 1, 0, 1))
            .set_border_color(SimplePalette::CARD)
        })
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(6, 3), "┌───\n│\n└───\n");

    for (x, y) in [(0, 0), (3, 0), (0, 1), (0, 2), (3, 2)] {
      assert_eq!(colors_at(6, 3, x, y).0, Some(rgb(red())), "({x}, {y})");
    }
  });
}
