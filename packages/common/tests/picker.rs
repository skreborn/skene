//! The picker: the face opens the list and focus lands on the chosen option, the
//! arrows clamp, `Enter` chooses, the value signal is the truth, `Escape` or a
//! dismiss close with the value untouched, and typing picks by name on the closed face
//! and in the open list.

mod common;

use std::{cell::RefCell, rc::Rc};

use skene_common::{Label, ListItem, TYPE_AHEAD_RESET};
use skene_core::{
  AccessProp,
  KeyEvent,
  NativeElement,
  NodeId,
  PickerOption,
  Placement,
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
  options,
  overlay_anchor,
  overlay_props,
  reset,
  text,
  trigger_change,
  trigger_click,
  trigger_dismiss,
  trigger_frame,
  trigger_hydrated,
  trigger_key_down,
  value as written_value,
};
use skene_reactive::{Get, Owner, Set, Signal};

type Text = skene_common::Text<Headless>;
type RawPicker<F = skene_common::NoChild> = skene_common::RawPicker<Headless, F>;
type RawOption<C = skene_common::NoChild> = skene_common::RawOption<Headless, C>;

const CONSUMED: Prevented = Prevented {
  platform: true,
  bubbling: true,
};
const KEPT: Prevented = Prevented {
  platform: false,
  bubbling: true,
};

fn key(key: &str) -> KeyEvent {
  KeyEvent::new(key)
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
  let start = owner.enter(move || {
    RawPicker::new()
      .bind_value(value)
      .set_label("Size")
      .set_options(
        ["a", "b", "c"].map(|value| RawOption::new(ListItem::new(value)).set_label(value)),
      )
      .set_face(|shown| Text::derive(move || shown.get()))
      .build()
  });

  Parts::of(form(start))
}

/// The one form the picker's segment `start` holds.
fn form(start: NodeId) -> NodeId {
  let nodes = skene_core::Scene::<Headless>::with(|scene| scene.segment_nodes(start));

  assert_eq!(nodes.len(), 1, "one form at a time: {nodes:?}");

  nodes[0]
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
    box_props(parts.face).and_then(|props| props.role),
    Some(Role::ComboBox)
  );
  assert_eq!(
    box_props(parts.face).and_then(|props| props.role),
    Some(Role::ComboBox)
  );
  assert_eq!(accessible_name(parts.face), Some("Size".to_owned()));
  assert_eq!(text(parts.shown), Some("b".to_owned()));
  assert_eq!(
    box_props(parts.listbox).and_then(|props| props.role),
    Some(Role::ListBox)
  );
  assert_eq!(
    box_props(parts.options[0]).and_then(|props| props.role),
    Some(Role::ListBoxOption)
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
  assert_eq!(overlay_anchor(parts.overlay), Some(parts.face));
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
  assert_eq!(trigger_key_down(third, &key("F5")), Prevented::default());

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
  let parts = Parts::of(form(face));

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

/// The face reports what it opens: a listbox popup, and the listbox node it controls
/// once the list is built.
#[test]
fn the_face_reports_its_popup_and_the_list_it_controls() {
  use skene_core::PopupKind;

  reset();

  let owner = Owner::new();
  let value = Signal::new("a".to_owned());
  let parts = picker(owner, value);
  let access = box_props(parts.face)
    .map(|props| props.access)
    .unwrap_or_default();

  assert!(
    access.contains(&AccessProp::HasPopup(PopupKind::Listbox)),
    "{access:?}"
  );
  assert!(
    access.contains(&AccessProp::Controls(vec![parts.listbox])),
    "{access:?}"
  );

  owner.dispose();
}

/// `Tab` on a focused option commits it and lets focus travel on: the value follows,
/// the list closes, and the key is left to the platform.
#[test]
fn tab_on_an_option_commits_it_and_closes_the_list() {
  reset();

  let owner = Owner::new();
  let value = Signal::new("a".to_owned());
  let parts = picker(owner, value);

  let _ = trigger_key_down(parts.face, &key("ArrowDown"));

  assert!(is_open(parts.overlay));

  let _ = trigger_key_down(parts.options[0], &key("ArrowDown"));
  let verdict = trigger_key_down(parts.options[1], &key("Tab"));

  assert!(!verdict.platform, "Tab travels on");
  assert_eq!(value.get(), "b");
  assert!(!is_open(parts.overlay));

  owner.dispose();
}

/// A derived option name keeps type-ahead current: the characters match the name the
/// option goes by now.
#[test]
fn type_ahead_matches_a_derived_name_as_it_reads_now() {
  use skene_common::Label;

  reset();

  let owner = Owner::new();
  let value = Signal::new("ba".to_owned());
  let renamed = Signal::new("Apple".to_owned());
  let face = owner.enter(move || {
    RawPicker::new()
      .bind_value(value)
      .set_options([
        RawOption::new(ListItem::new("ap")).set_label(
          Label::new(Text::derive(move || renamed.get())).derive_name(move || renamed.get()),
        ),
        RawOption::new(ListItem::new("ba"))
          .set_label(Label::new(Text::new("Banana")).set_name("Banana")),
      ])
      .set_face(|shown| Text::derive(move || shown.get()))
      .build()
  });
  let parts = Parts::of(form(face));

  renamed.set("Cherry".to_owned());

  assert_eq!(trigger_key_down(parts.face, &key("c")), CONSUMED);
  assert_eq!(value.get(), "ap", "the name as it reads now");

  owner.dispose();
}

/// Options named apart from their values for type-ahead: `Label::set_name` supplies
/// the name the characters match.
fn named_picker(owner: Owner, value: Signal<String>) -> Parts {
  use skene_common::Label;

  let face = owner.enter(move || {
    RawPicker::new()
      .bind_value(value)
      .set_label("Fruit")
      .set_options(
        [("ap", "Apple"), ("av", "Avocado"), ("ba", "Banana")].map(|(value, name)| {
          RawOption::new(ListItem::new(value)).set_label(Label::new(Text::new(name)).set_name(name))
        }),
      )
      .set_face(|shown| Text::derive(move || shown.get()))
      .build()
  });

  Parts::of(form(face))
}

/// Typing over the closed face chooses by name without opening: two characters in
/// quick succession match a prefix, a character repeated cycles through the options
/// starting with it, and once the buffer's reset has elapsed the next character
/// starts afresh. `Space` extends a buffer already begun and toggles the list
/// otherwise.
#[test]
fn typing_over_the_closed_face_chooses_by_name() {
  reset();

  let owner = Owner::new();
  let value = Signal::new("ba".to_owned());
  let parts = named_picker(owner, value);

  assert_eq!(trigger_key_down(parts.face, &key("a")), CONSUMED);
  assert_eq!(value.get(), "ap");
  assert!(!is_open(parts.overlay), "type-ahead keeps the list closed");

  assert_eq!(trigger_key_down(parts.face, &key("v")), CONSUMED);
  assert_eq!(value.get(), "av", "the buffer matches a prefix");

  trigger_frame(TYPE_AHEAD_RESET);

  let _ = trigger_key_down(parts.face, &key("a"));

  assert_eq!(
    value.get(),
    "ap",
    "a fresh character cycles past the chosen"
  );

  let _ = trigger_key_down(parts.face, &key("a"));

  assert_eq!(value.get(), "av", "a repeated character cycles on");

  let _ = trigger_key_down(parts.face, &key("z"));

  assert_eq!(value.get(), "av", "no match leaves the value alone");
  assert_eq!(trigger_key_down(parts.face, &key(" ")), CONSUMED);
  assert!(!is_open(parts.overlay), "space extends the buffer begun");

  trigger_frame(TYPE_AHEAD_RESET);

  assert_eq!(trigger_key_down(parts.face, &key(" ")), CONSUMED);
  assert!(is_open(parts.overlay), "space opens the list without one");

  owner.dispose();
}

/// Typing over the open list moves focus to the option whose name the characters
/// begin, without choosing; `Space` extends a buffer already begun.
#[test]
fn typing_over_the_open_list_moves_focus_by_name() {
  reset();

  let owner = Owner::new();
  let value = Signal::new("ap".to_owned());
  let parts = named_picker(owner, value);

  let _ = trigger_key_down(parts.face, &key("ArrowDown"));

  assert!(is_focused(parts.options[0]));
  assert_eq!(trigger_key_down(parts.options[0], &key("b")), CONSUMED);
  assert!(is_focused(parts.options[2]), "focus reaches the match");
  assert_eq!(value.get(), "ap", "typing chooses nothing");
  assert!(is_open(parts.overlay));

  trigger_frame(TYPE_AHEAD_RESET);

  let _ = trigger_key_down(parts.options[2], &key("a"));

  assert!(is_focused(parts.options[0]), "a character wraps around");

  let _ = trigger_key_down(parts.options[0], &key("v"));

  assert!(is_focused(parts.options[1]), "the buffer matches a prefix");
  assert_eq!(trigger_key_down(parts.options[1], &key(" ")), CONSUMED);
  assert!(
    is_open(parts.overlay),
    "space extends the buffer rather than choosing"
  );
  assert_eq!(value.get(), "ap");

  trigger_frame(TYPE_AHEAD_RESET);

  let _ = trigger_key_down(parts.options[1], &key(" "));

  assert_eq!(value.get(), "av", "a fresh space chooses");
  assert!(!is_open(parts.overlay));

  owner.dispose();
}

/// Until the render is hydrated the picker is the platform's own choice control,
/// offering the options by value and name with the bound value written; a platform
/// choice reaches the signal, and once hydrated the drawn face takes its place
/// showing the value chosen.
#[test]
fn until_hydrated_the_picker_is_the_platforms_own() {
  reset();
  trigger_hydrated(false);

  let owner = Owner::new();
  let value = Signal::new("b".to_owned());
  let start = owner.enter(move || {
    RawPicker::new()
      .bind_value(value)
      .set_label("Size")
      .set_options(["a", "b", "c"].map(|value| {
        let name = value.to_uppercase();

        RawOption::new(ListItem::new(value))
          .set_disabled(value == "c")
          .set_label(Label::new(Text::new(&name)).set_name(name))
      }))
      .set_face(|shown| Text::derive(move || shown.get()))
      .build()
  });
  let native = form(start);

  assert_eq!(
    control_props(native).map(|props| props.native),
    Some(Some(NativeElement::Picker))
  );
  assert_eq!(
    options(native),
    Some(
      [("a", "A", false), ("b", "B", false), ("c", "C", true)]
        .map(|(value, name, disabled)| PickerOption {
          value: value.to_owned(),
          name: name.to_owned(),
          disabled,
        })
        .to_vec()
    ),
    "the options are read from the option views, the disabled one marked"
  );
  assert_eq!(written_value(native).as_deref(), Some("b"));
  assert_eq!(accessible_name(native).as_deref(), Some("Size"));

  trigger_change(native, "c");

  assert_eq!(value.get(), "c", "the platform's choice reaches the signal");
  assert_eq!(written_value(native).as_deref(), Some("c"));

  trigger_hydrated(true);

  let face = form(start);

  assert_eq!(
    box_props(face).and_then(|props| props.role),
    Some(Role::ComboBox),
    "the drawn face took the platform's place"
  );
  assert_eq!(text(Parts::of(face).shown).as_deref(), Some("c"));
}

/// The drawn face that takes the platform's place carries the same caption, the name
/// it supplies and the disabled setting.
#[test]
fn the_drawn_face_keeps_the_caption_name_and_disabled_setting() {
  reset();
  trigger_hydrated(false);

  let owner = Owner::new();
  let start = owner.enter(move || {
    RawPicker::new()
      .set_label("Size")
      .set_disabled(true)
      .set_options(["a"].map(|value| RawOption::new(ListItem::new(value)).set_label(value)))
      .set_face(|shown| Text::derive(move || shown.get()))
      .build()
  });
  let caption = |form| {
    skene_core::Scene::<Headless>::with(|scene| {
      scene
        .children(form)
        .find(|child| scene.part(*child) == Some(skene_core::Part::Label))
    })
  };
  let native = form(start);
  let native_caption = caption(native);

  assert_eq!(accessible_name(native).as_deref(), Some("Size"));
  assert!(control_state(native).is_some_and(|state| state.disabled));

  trigger_hydrated(true);

  let face = form(start);

  assert_eq!(
    box_props(face).and_then(|props| props.role),
    Some(Role::ComboBox)
  );
  assert_eq!(accessible_name(face).as_deref(), Some("Size"));
  assert!(control_state(face).is_some_and(|state| state.disabled));
  assert!(native_caption.is_some());
  assert_eq!(
    caption(face),
    native_caption,
    "the caption moved, not rebuilt"
  );

  owner.dispose();
}

/// The list opens below the face unless the picker places it elsewhere.
#[test]
fn a_picker_places_its_list() {
  reset();

  let owner = Owner::new();
  let below = picker(owner, Signal::new("a".to_owned()));

  assert_eq!(
    overlay_props(below.overlay).map(|props| props.placement),
    Some(Placement::Bottom)
  );

  let above = Parts::of(form(owner.enter(move || {
    RawPicker::new()
      .set_placement(Placement::Top)
      .set_options([RawOption::new(ListItem::new("a")).set_label("a")])
      .set_face(|shown| Text::derive(move || shown.get()))
      .build()
  })));

  assert_eq!(
    overlay_props(above.overlay).map(|props| props.placement),
    Some(Placement::Top)
  );

  owner.dispose();
}
