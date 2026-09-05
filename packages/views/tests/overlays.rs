//! Overlays: the bound signal opens and closes without rebuilding, a dismiss is a
//! request the app answers, an anchored popup names its anchor to the driver, and a
//! focus handle moves focus to the view it is bound to.

mod common;

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_core::{Color, EventKind, FocusOrder, HasChildren, Placement, View};
use skene_headless::{
  Headless,
  children,
  handle,
  is_focused,
  is_live,
  is_open,
  listened,
  overlay_anchor,
  overlay_props,
  reset,
  resolve_color,
  trigger_dismiss,
};
use skene_reactive::{Get, Owner, Set, Signal};
use skene_views::{Activate, Anchor};

use crate::common::{COLOR, blue, only_child, red};

type Container = skene_views::Container<Headless>;
type Column = skene_views::Column<Headless>;
type Text = skene_views::Text<Headless>;
type Overlay<C = skene_views::NoChild> = skene_views::Overlay<Headless, C>;
type RawButton = skene_views::RawButton<Headless>;
type FocusHandle = skene_views::FocusHandle<Headless>;

/// A modal follows its bound signal through the driver and paints a backdrop; opening
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

/// A popup anchored to a bound box passes the anchor's sink to the driver each time it
/// opens, carries no backdrop, and a derived placement re-places it.
#[test]
fn a_popup_anchored_to_a_bound_box_passes_the_anchors_sink() {
  reset();

  let owner = Owner::new();
  let open = Signal::new(false);
  let placement = Signal::opaque(Placement::Bottom);
  let anchor = Anchor::new();
  let column = owner.enter({
    let anchor = anchor.clone();

    move || {
      Column::new()
        .set_children((
          Container::new().bind_anchor(&anchor),
          Overlay::popup()
            .set_anchor(&anchor)
            .derive_placement(move || placement.get())
            .bind_open(open)
            .set_child(Text::new("popup")),
        ))
        .build()
    }
  });
  let face = children(column)[1];
  let popup = children(column)[2];
  let props = overlay_props(popup).expect("an overlay");

  assert!(!props.modal);
  assert_eq!(props.backdrop, None);
  assert_eq!(props.placement, Placement::Bottom);
  assert_eq!(anchor.node(), Some(face));
  assert_eq!(overlay_anchor(popup), handle(face));

  open.set(true);

  assert!(is_open(popup));
  assert_eq!(overlay_anchor(popup), handle(face));

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

/// A focus handle bound to a control or to a box with a focus order moves focus to it
/// and takes it off again; unbound it does nothing.
#[test]
fn a_focus_handle_moves_focus_to_its_bound_view() {
  reset();

  let owner = Owner::new();
  let button_focus = FocusHandle::new();
  let box_focus = FocusHandle::new();
  let unbound = FocusHandle::new();
  let column = owner.enter({
    let button_focus = button_focus.clone();
    let box_focus = box_focus.clone();

    move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .bind_focus(&button_focus)
            .set_label("Save"),
          Container::new()
            .set_focus_order(FocusOrder::Auto)
            .bind_focus(&box_focus),
        ))
        .build()
    }
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
