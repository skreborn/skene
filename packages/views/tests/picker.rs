//! The picker: the face opens the list and focus lands on the chosen option, the
//! arrows clamp, `Enter` chooses, the value signal is the truth, and `Escape` or a
//! dismiss close with the value untouched.

mod common;

use std::{cell::RefCell, rc::Rc};

use skene_core::{
  AccessProp,
  ControlKind,
  KeyEvent,
  Modifiers,
  NodeId,
  Prevented,
  Role,
  Toggle as Checked,
  View,
};
use skene_headless::{
  Headless,
  accessible_name,
  box_props,
  children,
  control_props,
  control_state,
  is_focused,
  is_open,
  is_tab_stop,
  overlay_anchor,
  overlay_props,
  reset,
  text,
  trigger_click,
  trigger_dismiss,
  trigger_key_down,
};
use skene_reactive::{Get, Owner, Set, Signal};
use skene_views::ListItem;

type Text = skene_views::Text<Headless>;
type RawPicker<F = skene_views::NoChild> = skene_views::RawPicker<Headless, F>;
type RawOption<C = skene_views::NoChild> = skene_views::RawOption<Headless, C>;

const CONSUMED: Prevented = Prevented {
  platform: true,
  bubbling: true,
};
const KEPT: Prevented = Prevented {
  platform: false,
  bubbling: true,
};

fn key(key: &str) -> KeyEvent {
  KeyEvent {
    key: key.to_owned(),
    modifiers: Modifiers::default(),
  }
}

/// The nodes of a built picker: the face, its shown text, the overlay, the listbox and
/// the options in order.
struct Parts {
  face: NodeId,
  shown: NodeId,
  overlay: NodeId,
  listbox: NodeId,
  options: Vec<NodeId>,
}

impl Parts {
  fn of(face: NodeId) -> Self {
    let face_children = children(face);
    let overlay = *face_children.last().expect("the overlay");
    let listbox = children(overlay)[1];
    let list = children(listbox)[0];
    let options = children(list);

    Self {
      face,
      shown: face_children[0],
      overlay,
      listbox,
      options: options[1..options.len() - 1].to_vec(),
    }
  }

  fn expanded(&self) -> Option<bool> {
    box_props(self.face)?
      .access
      .iter()
      .find_map(|prop| match prop {
        AccessProp::Expanded(expanded) => Some(*expanded),
        _ => None,
      })
  }

  fn selected(&self, index: usize) -> Option<bool> {
    box_props(self.options[index])?
      .access
      .iter()
      .find_map(|prop| match prop {
        AccessProp::Selected(selected) => Some(*selected),
        _ => None,
      })
  }
}

fn picker(owner: Owner, value: Signal<String>) -> Parts {
  let face = owner.enter(move || {
    RawPicker::new()
      .bind_value(value)
      .set_label("Size")
      .set_options(
        ["a", "b", "c"].map(|value| RawOption::new(ListItem::new(value)).set_label(value)),
      )
      .set_face(|shown| Text::derive(move || shown.get()))
      .build()
  });

  Parts::of(face)
}

/// The face is a combobox reporting whether it is expanded; `ArrowDown` opens the
/// anchored popup and focus lands on the chosen option, which is the list's tab stop
/// and reports itself selected.
#[test]
fn arrow_down_opens_the_list_with_focus_on_the_chosen_option() {
  reset();

  let owner = Owner::new();
  let value = Signal::new("b".to_owned());
  let parts = picker(owner, value);

  assert_eq!(
    control_props(parts.face).map(|props| props.kind),
    Some(ControlKind::Combobox)
  );
  assert_eq!(
    box_props(parts.face).and_then(|props| props.role),
    Some(Role::Combobox)
  );
  assert_eq!(accessible_name(parts.face), Some("Size".to_owned()));
  assert_eq!(text(parts.shown), Some("b".to_owned()));
  assert_eq!(
    control_props(parts.listbox).map(|props| props.kind),
    Some(ControlKind::Listbox)
  );
  assert_eq!(
    control_props(parts.options[0]).map(|props| props.kind),
    Some(ControlKind::ListboxOption)
  );
  assert_eq!(parts.options.len(), 3);
  assert_eq!(parts.expanded(), Some(false));
  assert!(!is_open(parts.overlay));
  assert!(!overlay_props(parts.overlay).expect("an overlay").modal);
  assert!(!is_tab_stop(parts.options[0]));
  assert!(is_tab_stop(parts.options[1]));
  assert_eq!(parts.selected(0), Some(false));
  assert_eq!(parts.selected(1), Some(true));
  assert_eq!(
    control_state(parts.options[1]).map(|state| state.checked),
    Some(Checked::On)
  );

  assert_eq!(trigger_key_down(parts.face, &key("ArrowDown")), CONSUMED);

  assert!(is_open(parts.overlay));
  assert_eq!(
    overlay_anchor(parts.overlay),
    skene_headless::handle(parts.face)
  );
  assert_eq!(parts.expanded(), Some(true));
  assert_eq!(
    control_state(parts.face).map(|state| state.open),
    Some(true)
  );
  assert!(is_focused(parts.options[1]));

  value.set("c".to_owned());

  assert_eq!(text(parts.shown), Some("c".to_owned()));
  assert_eq!(parts.selected(1), Some(false));
  assert_eq!(parts.selected(2), Some(true));

  owner.dispose();
}

/// The arrows move focus over the options and clamp at both ends, `Home` and `End`
/// reach them, nothing is chosen on the way, and `Enter` chooses the focused option,
/// closes the list and returns focus to the face.
#[test]
fn arrows_clamp_over_the_options_and_enter_chooses() {
  reset();

  let owner = Owner::new();
  let value = Signal::new("b".to_owned());
  let parts = picker(owner, value);
  let [first, second, third] = [parts.options[0], parts.options[1], parts.options[2]];

  assert_eq!(trigger_key_down(parts.face, &key("ArrowDown")), CONSUMED);
  assert!(is_focused(second));
  assert_eq!(trigger_key_down(second, &key("ArrowDown")), CONSUMED);
  assert!(is_focused(third));
  assert_eq!(trigger_key_down(third, &key("ArrowDown")), CONSUMED);
  assert!(is_focused(third));
  assert_eq!(trigger_key_down(third, &key("Home")), CONSUMED);
  assert!(is_focused(first));
  assert_eq!(trigger_key_down(first, &key("ArrowUp")), CONSUMED);
  assert!(is_focused(first));
  assert_eq!(trigger_key_down(first, &key("End")), CONSUMED);
  assert!(is_focused(third));
  assert_eq!(value.get(), "b");
  assert!(is_open(parts.overlay));
  assert_eq!(trigger_key_down(third, &key("x")), Prevented::default());

  assert_eq!(trigger_key_down(third, &key("Enter")), CONSUMED);

  assert_eq!(value.get(), "c");
  assert!(!is_open(parts.overlay));
  assert!(is_focused(parts.face));
  assert_eq!(parts.selected(2), Some(true));
  assert!(is_tab_stop(third));
  assert!(!is_tab_stop(second));

  assert_eq!(trigger_click(parts.face), KEPT);
  assert!(is_open(parts.overlay));
  assert!(is_focused(third));
  assert_eq!(trigger_click(first), KEPT);
  assert_eq!(value.get(), "a");
  assert!(!is_open(parts.overlay));

  owner.dispose();
}

/// The value signal is the truth: a choice the app does not write back leaves the
/// selection, the face and the tab stop where they were.
#[test]
fn the_value_signal_is_the_truth() {
  reset();

  let owner = Owner::new();
  let asked = Rc::new(RefCell::new(Vec::new()));
  let face = owner.enter({
    let asked = Rc::clone(&asked);

    move || {
      RawPicker::new()
        .set_value("a")
        .handle_change(move |value| asked.borrow_mut().push(value))
        .set_options(["a", "b"].map(|value| RawOption::new(ListItem::new(value)).set_label(value)))
        .set_face(|shown| Text::derive(move || shown.get()))
        .build()
    }
  });
  let parts = Parts::of(face);

  assert_eq!(trigger_key_down(parts.face, &key("Enter")), CONSUMED);
  assert!(is_focused(parts.options[0]));
  assert_eq!(
    trigger_key_down(parts.options[0], &key("ArrowDown")),
    CONSUMED
  );
  assert_eq!(trigger_key_down(parts.options[1], &key(" ")), CONSUMED);
  assert_eq!(*asked.borrow(), ["b"]);
  assert_eq!(text(parts.shown), Some("a".to_owned()));
  assert_eq!(parts.selected(0), Some(true));
  assert_eq!(parts.selected(1), Some(false));
  assert!(is_tab_stop(parts.options[0]));
  assert!(!is_open(parts.overlay));

  owner.dispose();
}

/// `Escape` from an option and a dismiss of the popup close the list with the value
/// untouched; `Escape` returns focus to the face, and a click on the face toggles.
#[test]
fn escape_and_a_dismiss_close_without_changing_the_value() {
  reset();

  let owner = Owner::new();
  let value = Signal::new("b".to_owned());
  let parts = picker(owner, value);

  assert_eq!(
    trigger_key_down(parts.face, &key("Escape")),
    Prevented::default()
  );
  assert_eq!(trigger_key_down(parts.face, &key(" ")), CONSUMED);
  assert!(is_open(parts.overlay));
  assert_eq!(
    trigger_key_down(parts.options[1], &key("ArrowDown")),
    CONSUMED
  );
  assert_eq!(trigger_key_down(parts.options[2], &key("Escape")), CONSUMED);
  assert!(!is_open(parts.overlay));
  assert_eq!(value.get(), "b");
  assert!(is_focused(parts.face));
  assert_eq!(parts.expanded(), Some(false));

  assert_eq!(trigger_click(parts.face), KEPT);
  assert!(is_open(parts.overlay));

  trigger_dismiss(parts.overlay);

  assert!(!is_open(parts.overlay));
  assert_eq!(value.get(), "b");

  assert_eq!(trigger_click(parts.face), KEPT);
  assert!(is_open(parts.overlay));
  assert_eq!(trigger_click(parts.face), KEPT);
  assert!(!is_open(parts.overlay));

  owner.dispose();
}
