//! The container: its handlers fire through dispatch, its decoration resolves through
//! the theme, and its focus and accessibility follow their signals.

mod common;

use std::{cell::RefCell, rc::Rc};

use skene_core::{
  AccessProp,
  Border,
  Bubbling,
  EventKind,
  FocusOrder,
  HasChildren,
  KeyEvent,
  Modifiers,
  Overflow,
  PointerButton,
  PointerEvent,
  Prevented,
  Property,
  ScrollEvent,
  StyleProperty,
  Transition,
  View,
};
use skene_headless::{
  Headless,
  box_props,
  listened,
  reset,
  resolve_border,
  trigger_blur,
  trigger_focus,
  trigger_key_down,
  trigger_pointer_down,
  trigger_pointer_enter,
  trigger_pointer_leave,
  trigger_pointer_move,
  trigger_pointer_up,
  trigger_scroll,
};
use skene_reactive::{Get, Owner, Set, Signal};

use crate::common::{TestPalette, only_child, red};

type Container = skene_views::Container<Headless>;
type Theme<P, C = skene_views::NoChild> = skene_views::Theme<Headless, P, C>;

fn press() -> PointerEvent {
  PointerEvent {
    x: 1.,
    y: 2.,
    local_x: 1.,
    local_y: 2.,
    button: Some(PointerButton::Primary),
    modifiers: Modifiers::default(),
  }
}

fn key(key: &str) -> KeyEvent {
  KeyEvent {
    key: key.to_owned(),
    modifiers: Modifiers::default(),
  }
}

/// Every handler registers its event on the node, in the fixed order of kinds, and
/// fires through dispatch: a preventable event climbs to the outer container until a
/// `Bubbling::Stop`, and a notification stays on its node.
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
    let inner_down = note("inner down");
    let inner_key = note("inner key");
    let inner_up = note("inner up");
    let inner_move = note("inner move");
    let inner_enter = note("inner enter");
    let inner_leave = note("inner leave");
    let inner_focus = note("inner focus");
    let inner_blur = note("inner blur");
    let inner_scroll = note("inner scroll");

    Container::new()
      .handle_pointer_down(move |_| outer_down())
      .handle_key_down(move |_| outer_key())
      .handle_pointer_up(move |_| outer_up())
      .handle_pointer_enter(outer_enter)
      .set_child(
        Container::new()
          .set_overflow(Overflow::Scroll)
          .handle_pointer_down(move |event| {
            assert_eq!(event.button, Some(PointerButton::Primary));
            inner_down();

            Bubbling::Stop
          })
          .handle_key_down(move |event| {
            assert_eq!(event.key, "Enter");
            inner_key();
          })
          .handle_pointer_up(move |_| inner_up())
          .handle_pointer_move(move |_| inner_move())
          .handle_pointer_enter(inner_enter)
          .handle_pointer_leave(inner_leave)
          .handle_focus(inner_focus)
          .handle_blur(inner_blur)
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
      EventKind::KeyDown,
      EventKind::PointerDown,
      EventKind::PointerUp,
      EventKind::PointerMove,
      EventKind::PointerEnter,
      EventKind::PointerLeave,
      EventKind::Focus,
      EventKind::Blur,
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
  assert_eq!(trigger_pointer_up(inner, &press()), Prevented::default());
  assert_eq!(trigger_pointer_move(inner, &press()), Prevented::default());

  trigger_pointer_enter(inner);
  trigger_pointer_leave(inner);
  trigger_focus(inner);
  trigger_blur(inner);
  trigger_scroll(inner, &ScrollEvent { x: 0., y: 10. });

  assert_eq!(
    *log.borrow(),
    [
      "inner down",
      "inner key",
      "outer key",
      "inner up",
      "outer up",
      "inner move",
      "inner enter",
      "inner leave",
      "inner focus",
      "inner blur",
      "inner scroll",
    ]
  );

  owner.dispose();
}

/// A border whose color is a token resolves through the theme, and a transition lands
/// in the box's dynamics.
#[test]
fn border_with_a_token_color_resolves_through_the_theme() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Theme::new(TestPalette::with_card(red()))
      .set_child_builder(|| {
        Container::new()
          .set_border(Border::new(2., TestPalette::CARD))
          .set_transition(Transition::new(Property::Color, 0.2))
      })
      .build()
  });
  let card = skene_headless::children(root)[0];
  let props = box_props(card).unwrap();

  assert_eq!(resolve_border(card), Some(Border::new(2., red())));
  assert_eq!(props.dynamics.transitions.len(), 1);
  assert_eq!(
    props.dynamics.transitions[0].property,
    StyleProperty::Shared(Property::Color)
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
        .set_accessibility([AccessProp::Label("fixed".to_owned())])
        .build(),
      Container::new()
        .derive_focus_order(move || position.get())
        .derive_accessibility(move || vec![AccessProp::Expanded(expanded.get())])
        .build(),
    )
  });
  let fixed = box_props(fixed).unwrap();

  assert_eq!(fixed.focus_order, Some(FocusOrder::Auto));
  assert_eq!(fixed.access, [AccessProp::Label("fixed".to_owned())]);

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
