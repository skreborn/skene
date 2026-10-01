//! Overlays: the bound signal opens and closes without rebuilding, a dismiss is a
//! request the app answers, an anchored popup names its anchor to the platform, and a
//! focus handle moves focus to the view it is bound to.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_foundation::{
  Activate,
  Color,
  EventKind,
  FocusOrder,
  HasChildren,
  Placement,
  PointerButton,
  PointerEvent,
  View,
};
use skene_headless::{
  Headless,
  children,
  is_focused,
  is_live,
  is_open,
  listened,
  overlay_anchor,
  overlay_props,
  reset,
  resolve_color,
  trigger_dismiss,
  trigger_pointer_down,
};
use skene_reactive::{Get, Owner, Set, Signal};

use crate::common::{COLOR, blue, only_child, red};

type Container = skene_foundation::Container<Headless>;
type Column = skene_foundation::Column<Headless>;
type Text = skene_foundation::Text<Headless>;
type Overlay<C = skene_foundation::NoChild> = skene_foundation::Overlay<Headless, C>;
type BasicButton = skene_foundation::BasicButton<Headless>;
type NodeHandle = skene_foundation::NodeHandle<Headless>;

/// A modal follows its bound signal through the platform and paints a backdrop; opening
/// and closing keep the same content node, so a signal created while building the
/// content keeps its value across a close.
#[test]
fn a_modal_opens_and_closes_through_its_signal_without_rebuilding() {
  reset();

  let owner = Owner::new();
  let open = Signal::new(false);
  let inner: Rc<Cell<Option<Signal<i32>>>> = Rc::new(Cell::new(None));
  let node = owner.enter({
    let inner = Rc::clone(&inner);

    move || {
      let count = Signal::new(1);

      inner.set(Some(count));

      Overlay::modal()
        .bind_open(open)
        .set_color(red())
        .set_child(Container::new().derive_color(
          move || {
            if count.get() > 1 { red() } else { blue() }
          },
        ))
        .build()
    }
  });
  let props = overlay_props(node).expect("an overlay");
  let content = only_child(node);
  let count = inner.get().expect("the content's signal");

  assert!(props.modal);
  assert!(props.backdrop.is_some());
  assert_eq!(props.placement, Placement::Center);
  assert!(!is_open(node));
  assert_eq!(resolve_color(node, COLOR), Some(red()));
  assert_eq!(resolve_color(content, COLOR), Some(blue()));

  open.set(true);

  assert!(is_open(node));

  count.set(2);

  assert_eq!(resolve_color(content, COLOR), Some(red()));

  open.set(false);

  assert!(!is_open(node));
  assert!(is_live(content));
  assert_eq!(count.get(), 2);

  open.set(true);

  assert!(is_open(node));
  assert_eq!(only_child(node), content);
  assert_eq!(resolve_color(content, COLOR), Some(red()));

  owner.dispose();
}

/// A dismiss runs the handler and nothing else: the overlay stays open until the app
/// writes the signal, which a bound signal does on its own.
#[test]
fn a_dismiss_requests_and_never_closes_by_itself() {
  reset();

  let owner = Owner::new();
  let open = Signal::new(true);
  let log = Rc::new(RefCell::new(Vec::new()));
  let (requested, bound) = owner.enter({
    let log = Rc::clone(&log);

    move || {
      let requested = Overlay::popup()
        .derive_open(move || open.get())
        .handle_dismiss(move || log.borrow_mut().push("dismiss"))
        .set_child(Text::new("requested"))
        .build();
      let bound = Overlay::popup()
        .bind_open(open)
        .set_child(Text::new("bound"))
        .build();

      (requested, bound)
    }
  });

  assert!(listened(requested).contains(&EventKind::Dismiss));
  assert!(is_open(requested));
  assert!(is_open(bound));

  trigger_dismiss(requested);

  assert_eq!(*log.borrow(), ["dismiss"]);
  assert!(open.get());
  assert!(is_open(requested));

  trigger_dismiss(bound);

  assert!(!open.get());
  assert!(!is_open(requested));
  assert!(!is_open(bound));

  owner.dispose();
}

/// A popup anchored to a bound box passes the anchor's sink to the platform each time it
/// opens, carries no backdrop, and a derived placement re-places it.
#[test]
fn a_popup_anchored_to_a_bound_box_passes_the_anchors_sink() {
  reset();

  let owner = Owner::new();
  let open = Signal::new(false);
  let placement = Signal::transient(Placement::Bottom);
  let anchor = NodeHandle::new();
  let column = owner.enter(move || {
    Column::new()
      .set_children((
        Container::new().bind_handle(anchor),
        Overlay::popup()
          .set_anchor(anchor)
          .derive_placement(move || placement.get())
          .bind_open(open)
          .set_child(Text::new("popup")),
      ))
      .build()
  });
  let face = children(column)[1];
  let popup = children(column)[2];
  let props = overlay_props(popup).expect("an overlay");

  assert!(!props.modal);
  assert_eq!(props.backdrop, None);
  assert_eq!(props.placement, Placement::Bottom);
  assert_eq!(anchor.node(), Some(face));
  assert_eq!(overlay_anchor(popup), Some(face));

  open.set(true);

  assert!(is_open(popup));
  assert_eq!(overlay_anchor(popup), Some(face));

  placement.set(Placement::Top);

  assert_eq!(
    overlay_props(popup).map(|props| props.placement),
    Some(Placement::Top)
  );
  assert!(is_open(popup));

  let modal = owner.enter(|| {
    Overlay::modal()
      .set_backdrop(Some(Color::TRANSPARENT))
      .set_child(Text::new("modal"))
      .build()
  });

  assert_eq!(
    overlay_props(modal).and_then(|props| props.backdrop),
    Some(Color::TRANSPARENT)
  );

  owner.dispose();
}

/// A press outside an open popup dismisses it, a press inside it or on the anchor it
/// opened from does not, as a browser's light dismiss does.
#[test]
fn a_press_outside_a_popup_dismisses_it() {
  reset();

  let owner = Owner::new();
  let open = Signal::new(false);
  let anchor = NodeHandle::new();
  let column = owner.enter(move || {
    Column::new()
      .set_children((
        Container::new().bind_handle(anchor),
        Container::new(),
        Overlay::popup()
          .set_anchor(anchor)
          .bind_open(open)
          .set_child(Container::new()),
      ))
      .build()
  });
  let [face, page, popup] = [1, 2, 3].map(|index| children(column)[index]);
  let inside = only_child(popup);
  let press = PointerEvent::mouse(0., 0.).with_press(PointerButton::Primary);

  open.set(true);

  let _ = trigger_pointer_down(inside, &press);
  let _ = trigger_pointer_down(face, &press);

  assert!(open.get(), "a press inside or on the anchor leaves it open");

  let _ = trigger_pointer_down(page, &press);

  assert!(!open.get(), "a press outside dismisses it");

  owner.dispose();
}

/// A focus handle bound to a control or to a box with a focus order moves focus to it
/// and takes it off again; unbound it does nothing.
#[test]
fn a_focus_handle_moves_focus_to_its_bound_view() {
  reset();

  let owner = Owner::new();
  let button_focus = NodeHandle::new();
  let box_focus = NodeHandle::new();
  let unbound = NodeHandle::new();
  let column = owner.enter(move || {
    Column::new()
      .set_children((
        BasicButton::new(Activate::new())
          .bind_handle(button_focus)
          .set_label("Save"),
        Container::new()
          .set_focus_order(FocusOrder::Auto)
          .bind_handle(box_focus),
      ))
      .build()
  });
  let button = children(column)[1];
  let plain = children(column)[2];

  assert_eq!(button_focus.node(), Some(button));
  assert!(!is_focused(button));

  button_focus.focus();

  assert!(is_focused(button));

  box_focus.focus();

  assert!(is_focused(plain));
  assert!(!is_focused(button));

  unbound.focus();

  assert!(is_focused(plain));

  box_focus.blur();

  assert!(!is_focused(plain));

  owner.dispose();
}
