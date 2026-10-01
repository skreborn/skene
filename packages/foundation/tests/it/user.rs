//! A user on the host platform: nodes found by role and text as a reader finds them, and
//! driven by clicks, keys and typed text as a browser dispatches them.

use std::{cell::Cell, rc::Rc};

use skene_foundation::{
  Activate,
  ContainerBase,
  HasChild,
  HasChildren,
  KeyEvent,
  NodeHandle,
  Role,
  TextEntry,
  TextSelection,
  View,
};
use skene_headless::{
  Headless,
  find_all_by_role,
  find_by_role,
  find_by_text,
  is_focused,
  reset,
  selection,
  user,
};
use skene_reactive::{Get, Owner, Set, Signal};

type Column = skene_foundation::Column<Headless>;
type Container = skene_foundation::Container<Headless>;
type Text = skene_foundation::Text<Headless>;
type BasicButton = skene_foundation::BasicButton<Headless>;
type BasicTextField = skene_foundation::BasicTextField<Headless>;
type BasicTextEditor = skene_foundation::BasicTextEditor<Headless>;
type Overlay = skene_foundation::Overlay<Headless>;

/// A button found by its role and name takes a user's click: focus moves to it and its
/// handler runs once; a click on a box taking no focus takes focus off it.
#[test]
fn a_click_focuses_and_activates_what_it_finds() {
  reset();

  let owner = Owner::new();
  let clicks = Rc::new(Cell::new(0));
  let counted = Rc::clone(&clicks);
  let _page = owner.enter(move || {
    Column::new()
      .set_children((
        BasicButton::new(Activate::new())
          .set_label("Save")
          .handle_click(move || counted.set(counted.get() + 1)),
        BasicButton::new(Activate::new()).set_label("Cancel"),
        Container::new().set_child(Text::new("Nothing to press")),
      ))
      .build()
  });
  let save = find_by_role(Role::Button, "Save").expect("the Save button");

  assert_eq!(find_all_by_role(Role::Button).len(), 2);
  assert_eq!(find_by_role(Role::Button, "Delete"), None);

  user::click(save);

  assert_eq!(clicks.get(), 1);
  assert!(is_focused(save));

  user::click(find_by_text("Nothing to press").expect("the text"));

  assert!(!is_focused(save));

  owner.dispose();
}

/// Typed text reaches a bound field one cluster at a time, the caret following it;
/// `Backspace` removes the cluster before the caret, and a multi-line entry takes a
/// newline where a single-line one does not.
#[test]
fn typed_text_edits_a_bound_entry() {
  reset();

  let owner = Owner::new();
  let line = Signal::new(String::new());
  let lines = Signal::new(String::new());
  let (field, editor) = owner.enter(move || {
    (
      BasicTextField::new(TextEntry::new().bind_value(line)).build(),
      BasicTextEditor::new(TextEntry::new().set_lines(3).bind_value(lines)).build(),
    )
  });

  user::type_text(field, "héllo\n");

  assert_eq!(line.get(), "héllo");
  assert_eq!(selection(field), Some(TextSelection::collapsed(5)));

  user::press_key(&KeyEvent::new("Backspace"));

  assert_eq!(line.get(), "héll");

  user::type_text(editor, "a\nb");

  assert_eq!(lines.get(), "a\nb");
  assert!(is_focused(editor));

  owner.dispose();
}

/// `Tab` pressed as a user presses it moves focus to the next tab stop.
#[test]
fn a_pressed_tab_moves_focus() {
  reset();

  let owner = Owner::new();
  let _page = owner.enter(|| {
    Column::new()
      .set_children((
        BasicButton::new(Activate::new()).set_label("First"),
        BasicButton::new(Activate::new()).set_label("Second"),
      ))
      .build()
  });
  let [first, second] =
    ["First", "Second"].map(|name| find_by_role(Role::Button, name).expect("a button"));

  user::click(first);
  user::press_key(&KeyEvent::new("Tab"));

  assert!(is_focused(second));

  owner.dispose();
}

/// A closed overlay's content is out of the tree a reader searches; open, it is found.
#[test]
fn a_closed_overlay_hides_its_content_from_a_search() {
  reset();

  let owner = Owner::new();
  let open = Signal::new(false);
  let anchor = NodeHandle::new();
  let _page = owner.enter(move || {
    Column::new()
      .set_children((
        Container::new().bind_handle(anchor),
        Overlay::popup()
          .set_anchor(anchor)
          .bind_open(open)
          .set_child(Text::new("Inside")),
      ))
      .build()
  });

  assert_eq!(find_by_text("Inside"), None);

  open.set(true);

  assert!(find_by_text("Inside").is_some());

  owner.dispose();
}
