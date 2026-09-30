//! A user in the terminal: nodes found by role and text as a reader finds them, and
//! driven by clicks, keys and typed text at the cells they cover.

use std::{cell::Cell, rc::Rc};

use skene_tui::{self as tui, Activate, TextEntry, prelude::*, testing, testing::user};

use crate::common::with_platform;

/// A button found by its role and name takes a click on its cells: focus moves to it
/// and its handler runs once; a field found by its caption takes typed text.
#[test]
fn a_user_clicks_and_types_into_what_it_finds() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);
    let typed = Signal::new(String::new());

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_label("Save")
            .handle_click(move || counted.set(counted.get() + 1)),
          RawButton::new(Activate::new()).set_label("Cancel"),
          RawTextField::new(TextEntry::new().bind_value(typed))
            .set_label("Name")
            .set_width(12),
        ))
        .into_any()
        .build()
    });

    let _ = testing::render_to_text(20, 5);
    let save = testing::find_by_role(Role::Button, "Save").expect("the Save button");

    assert_eq!(testing::find_all_by_role(Role::Button).len(), 2);
    assert_eq!(testing::accessible_name(save).as_deref(), Some("Save"));
    assert!(testing::find_by_text("Cancel").is_some());

    user::click(save);

    assert_eq!(clicks.get(), 1);
    assert_eq!(testing::focused_node(), Some(save));

    let field = testing::find_by_role(Role::TextInput, "Name").expect("the Name field");
    let _ = testing::render_to_text(20, 5);

    user::type_text(field, "héllo");

    assert_eq!(typed.get(), "héllo");
    assert_eq!(testing::focused_node(), Some(field));
  });
}
