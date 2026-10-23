//! The container: its handlers fire through dispatch, its decoration resolves through
//! the theme, and its focus and accessibility follow their signals.

use std::{cell::RefCell, rc::Rc};

use skene_foundation::{
  AccessProp,
  Axes,
  Bubbling,
  ContainerBase,
  DecorationBase,
  DragData,
  EventKind,
  FocusOrder,
  HasChild,
  HasChildren,
  Inset,
  KeyEvent,
  Modifiers,
  MotionBase,
  NamedKey,
  Overflow,
  PointerButton,
  PointerEvent,
  Prevented,
  Property,
  PropertyKey,
  ScrollEvent,
  State,
  StyleProperty,
  Transition,
  Value,
  View,
  WheelUnit,
};
use skene_headless::{
  Headless,
  box_props,
  children,
  is_focused,
  listened,
  reset,
  resolve_color,
  resolve_inset,
  trigger_blur,
  trigger_double_click,
  trigger_drag_over,
  trigger_drag_start,
  trigger_drop,
  trigger_focus,
  trigger_key_down,
  trigger_key_up,
  trigger_pointer_cancel,
  trigger_pointer_down,
  trigger_pointer_enter,
  trigger_pointer_leave,
  trigger_pointer_move,
  trigger_pointer_up,
  trigger_scroll,
  trigger_wheel,
};
use skene_reactive::{Get, Owner, Set, Signal};

use crate::common::{COLOR, TestPalette, blue, only_child, red};

type Container = skene_foundation::Container<Headless>;
type Theme<S, C = skene_foundation::NoChild> = skene_foundation::Theme<Headless, S, C>;

fn press() -> PointerEvent {
  PointerEvent::mouse(1., 2.).with_press(PointerButton::Primary)
}

fn key(key: &str) -> KeyEvent {
  KeyEvent::new(key)
}

/// Every handler registers its event on the node, in the fixed order of kinds, and
/// fires through dispatch: a preventable event climbs to the outer container until a
/// `Bubbling::Stop`, the pointer entering the inner container enters the outer one
/// first, and every other notification stays on its node.
#[expect(
  clippy::too_many_lines,
  reason = "one handler per event kind; length is the kind count, not complexity"
)]
#[test]
fn handlers_fire_through_dispatch_and_a_stop_ends_the_climb() {
  reset();

  let owner = Owner::new();
  let log = Rc::new(RefCell::new(Vec::new()));
  let note = |name: &'static str| {
    let log = Rc::clone(&log);

    move || log.borrow_mut().push(name)
  };
  let outer = owner.enter(|| {
    let outer_down = note("outer down");
    let outer_key = note("outer key");
    let outer_up = note("outer up");
    let outer_enter = note("outer enter");
    let inner_double = note("inner double click");
    let inner_down = note("inner down");
    let inner_key = note("inner key");
    let inner_key_up = note("inner key up");
    let inner_up = note("inner up");
    let inner_move = note("inner move");
    let inner_cancel = note("inner cancel");
    let inner_enter = note("inner enter");
    let inner_leave = note("inner leave");
    let inner_focus = note("inner focus");
    let inner_blur = note("inner blur");
    let inner_wheel = note("inner wheel");
    let inner_scroll = note("inner scroll");

    Container::new()
      .handle_pointer_down(move |_| outer_down())
      .handle_key_down(move |_| outer_key())
      .handle_pointer_up(move |_| outer_up())
      .handle_pointer_enter(outer_enter)
      .set_child(
        Container::new()
          .set_overflow(Overflow::Scroll)
          .handle_double_click(inner_double)
          .handle_pointer_down(move |event| {
            assert_eq!(event.button, Some(PointerButton::Primary));
            inner_down();

            Bubbling::Stop
          })
          .handle_key_down(move |event| {
            assert!(event.is(NamedKey::Enter));
            inner_key();
          })
          .handle_key_up(move |event| {
            assert!(event.is(NamedKey::Enter));
            inner_key_up();

            Bubbling::Stop
          })
          .handle_pointer_up(move |_| inner_up())
          .handle_pointer_move(move |_| inner_move())
          .handle_pointer_cancel(move |_| inner_cancel())
          .handle_pointer_enter(inner_enter)
          .handle_pointer_leave(inner_leave)
          .handle_focus(inner_focus)
          .handle_blur(inner_blur)
          .handle_wheel(move |event| {
            assert_eq!(event.delta, Axes::new(0., 3.));
            inner_wheel();
          })
          .handle_scroll(move |event| {
            assert_eq!(*event, ScrollEvent { x: 0., y: 10. });
            inner_scroll();
          }),
      )
      .build()
  });
  let inner = only_child(outer);

  assert_eq!(
    listened(inner),
    [
      EventKind::DoubleClick,
      EventKind::KeyDown,
      EventKind::KeyUp,
      EventKind::PointerDown,
      EventKind::PointerUp,
      EventKind::PointerMove,
      EventKind::PointerCancel,
      EventKind::PointerEnter,
      EventKind::PointerLeave,
      EventKind::Focus,
      EventKind::Blur,
      EventKind::Wheel,
      EventKind::Scroll,
    ]
  );
  assert_eq!(
    trigger_pointer_down(inner, &press()),
    Prevented {
      platform: false,
      bubbling: true,
    }
  );
  assert_eq!(trigger_key_down(inner, &key("Enter")), Prevented::default());
  assert_eq!(
    trigger_key_up(inner, &key("Enter")),
    Prevented {
      platform: false,
      bubbling: true,
    }
  );
  assert_eq!(trigger_pointer_up(inner, &press()), Prevented::default());
  assert_eq!(trigger_pointer_move(inner, &press()), Prevented::default());
  assert_eq!(
    trigger_pointer_cancel(inner, &press()),
    Prevented::default()
  );

  assert_eq!(trigger_double_click(inner), Prevented::default());

  trigger_pointer_enter(inner);
  trigger_pointer_leave(inner);
  trigger_focus(inner);
  trigger_blur(inner);
  assert_eq!(
    trigger_wheel(inner, &press().wheel(Axes::new(0., 3.), WheelUnit::Lines)),
    Prevented::default()
  );
  trigger_scroll(inner, &ScrollEvent { x: 0., y: 10. });

  assert_eq!(
    *log.borrow(),
    [
      "inner down",
      "inner key",
      "outer key",
      "inner key up",
      "inner up",
      "outer up",
      "inner move",
      "inner cancel",
      "inner double click",
      "outer enter",
      "inner enter",
      "inner leave",
      "inner focus",
      "inner blur",
      "inner wheel",
      "inner scroll",
    ]
  );

  owner.dispose();
}

/// A border whose color is a token resolves through the theme, its width per edge, and a
/// transition lands in the box's dynamics.
#[test]
fn border_with_a_token_color_resolves_through_the_theme() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Theme::new(TestPalette::with_card(red()))
      .set_child_builder(|| {
        Container::new()
          .set_border_width(Inset {
            top: 1.,
            end: 2.,
            bottom: 3.,
            start: 4.,
          })
          .set_border_color(TestPalette::CARD)
          .add_transition(Transition::new(Property::Background, 0.2))
      })
      .build()
  });
  let card = skene_headless::children(root)[0];
  let props = box_props(card).unwrap();

  assert_eq!(
    resolve_inset(card, PropertyKey::Shared(Property::BorderWidth)),
    Some(Inset {
      top: 1.,
      end: 2.,
      bottom: 3.,
      start: 4.,
    })
  );
  assert_eq!(
    resolve_color(card, PropertyKey::Shared(Property::BorderColor)),
    Some(red())
  );
  assert_eq!(props.dynamics.transitions.len(), 1);
  assert_eq!(
    props.dynamics.transitions[0].property,
    StyleProperty::Shared(Property::Background)
  );

  owner.dispose();
}

/// A fixed focus order and accessibility reach the props at creation; derived ones are
/// rewritten on the same node as their signals change.
#[test]
fn focus_order_and_accessibility_follow_their_signals() {
  reset();

  let owner = Owner::new();
  let position = Signal::new(FocusOrder::Position(1));
  let expanded = Signal::new(false);
  let (fixed, derived) = owner.enter(|| {
    (
      Container::new()
        .set_focus_order(FocusOrder::Auto)
        .set_accessibility([AccessProp::Name("fixed".to_owned())])
        .build(),
      Container::new()
        .derive_focus_order(move || position.get())
        .derive_accessibility(move || vec![AccessProp::Expanded(expanded.get())])
        .build(),
    )
  });
  let fixed = box_props(fixed).unwrap();

  assert_eq!(fixed.focus_order, Some(FocusOrder::Auto));
  assert_eq!(fixed.access, [AccessProp::Name("fixed".to_owned())]);

  let props = box_props(derived).unwrap();

  assert_eq!(props.focus_order, Some(FocusOrder::Position(1)));
  assert_eq!(props.access, [AccessProp::Expanded(false)]);

  position.set(FocusOrder::Skip);
  expanded.set(true);

  let props = box_props(derived).unwrap();

  assert_eq!(props.focus_order, Some(FocusOrder::Skip));
  assert_eq!(props.access, [AccessProp::Expanded(true)]);

  owner.dispose();
}

/// The pointer over a node holds `Hovered` on its ancestors too, as `:hover` does, and
/// leaving the node for the box around it keeps that box hovered.
#[test]
fn hover_holds_on_the_ancestors_of_the_hovered_node() {
  reset();

  let owner = Owner::new();
  let outer = owner.enter(|| {
    Container::new()
      .set_background(Value::from(red()).when(State::Hovered, blue()))
      .set_child(Container::new().set_background(Value::from(blue()).when(State::Hovered, red())))
      .build()
  });
  let inner = only_child(outer);
  let colors = || (resolve_color(outer, COLOR), resolve_color(inner, COLOR));

  trigger_pointer_enter(inner);

  assert_eq!(colors(), (Some(blue()), Some(red())));

  trigger_pointer_leave(inner);

  assert_eq!(colors(), (Some(blue()), Some(blue())));

  trigger_pointer_leave(outer);

  assert_eq!(colors(), (Some(red()), Some(blue())));

  owner.dispose();
}

/// `Tab` moves focus through the tab stops in order and wraps, `Shift-Tab` goes back,
/// and a hidden box's stops are skipped, as the terminal traverses.
#[test]
fn tab_traverses_the_scenes_tab_order() {
  reset();

  let owner = Owner::new();
  let column = owner.enter(|| {
    skene_foundation::Column::<Headless>::new()
      .set_children((
        Container::new().set_focus_order(FocusOrder::Auto),
        Container::new()
          .set_hidden(true)
          .set_child(Container::new().set_focus_order(FocusOrder::Auto)),
        Container::new().set_focus_order(FocusOrder::Auto),
      ))
      .build()
  });
  let boxes = children(column);
  let (first, last) = (boxes[1], boxes[3]);
  let back = KeyEvent {
    modifiers: Modifiers {
      shift: true,
      ..Modifiers::default()
    },
    ..key("Tab")
  };

  trigger_focus(first);

  let _ = trigger_key_down(first, &key("Tab"));

  assert!(is_focused(last), "the hidden box's stop is skipped");

  let _ = trigger_key_down(last, &key("Tab"));

  assert!(is_focused(first), "the order wraps");

  let _ = trigger_key_down(first, &back);

  assert!(is_focused(last), "Shift-Tab goes back");

  owner.dispose();
}

/// A drag from a box offering data reaches a box taking what it carries: the source
/// holds `Dragged` and the target `Targeted` while the drag lasts, a box taking another
/// type is no target, and the drop runs the target's action with the value.
#[test]
fn a_drag_drops_on_a_box_taking_what_it_carries() {
  reset();

  let owner = Owner::new();
  let dropped = Rc::new(RefCell::new(Vec::new()));
  let log = Rc::clone(&dropped);
  let column = owner.enter(|| {
    skene_foundation::Column::<Headless>::new()
      .set_children((
        Container::new()
          .set_background(Value::from(red()).when(State::Dragged, blue()))
          .set_drag_data(|| DragData::new(7u32).with_text("seven"))
          .set_child(Container::new()),
        Container::new()
          .set_background(Value::from(red()).when(State::Targeted, blue()))
          .handle_drop(move |value: &u32| log.borrow_mut().push(*value)),
        Container::new().handle_drop(|_: &String| {}),
      ))
      .build()
  });
  let boxes = children(column);
  let (source, target, other) = (boxes[1], boxes[2], boxes[3]);
  let inside = only_child(source);

  assert!(
    trigger_drag_start(inside),
    "the drag starts at the box offering data"
  );
  assert_eq!(resolve_color(source, COLOR), Some(blue()));
  assert!(
    !trigger_drag_over(other),
    "a box taking another type is no target"
  );
  assert!(trigger_drag_over(target));
  assert_eq!(resolve_color(target, COLOR), Some(blue()));
  assert!(trigger_drop());
  assert_eq!(*dropped.borrow(), [7]);
  assert_eq!(resolve_color(source, COLOR), Some(red()));
  assert_eq!(resolve_color(target, COLOR), Some(red()));

  owner.dispose();
}

/// A box view is small on the stack however many properties a style holds, so building
/// a deep tree of views in one expression fits a wasm stack.
#[test]
fn a_box_view_is_small_on_the_stack() {
  assert!(std::mem::size_of::<Container>() <= 1024);
  assert!(std::mem::size_of::<skene_foundation::Column<Headless>>() <= 1024);
}
