//! A user in the terminal: nodes found by role and text as a reader finds them, and
//! driven by clicks, keys and typed text at the cells that show of them.

use std::{cell::Cell, rc::Rc};

use skene_tui::{
  behavior::{Activate, TextEntry},
  platform::terminal::mount,
  prelude::*,
  testing,
  testing::user,
};

use crate::common::{mount_nodes, with_platform};

/// A button found by its role and name takes a click on its cells: focus moves to it
/// and its handler runs once; a field found by its caption takes typed text.
#[test]
fn a_user_clicks_and_types_into_what_it_finds() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);
    let typed = Signal::new(String::new());

    let _ = mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("Save")
            .handle_activate(move || counted.set(counted.get() + 1)),
          BasicButton::new(Activate::new()).set_label("Cancel"),
          BasicTextField::new(TextEntry::new().bind_value(typed))
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

/// A button found by its role and name takes a user's click: focus moves to it and its
/// handler runs once; a search for a name nothing carries finds nothing, and a click on
/// a box taking no focus takes focus off the button.
#[test]
fn a_click_focuses_and_activates_what_it_finds() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("Save")
            .handle_activate(move || counted.set(counted.get() + 1)),
          BasicButton::new(Activate::new()).set_label("Cancel"),
          Container::new().set_child(Text::new("Nothing to press")),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 3);
    let save = testing::find_by_role(Role::Button, "Save").expect("the Save button");

    assert_eq!(testing::find_all_by_role(Role::Button).len(), 2);
    assert_eq!(testing::find_by_role(Role::Button, "Delete"), None);

    user::click(save);

    assert_eq!(clicks.get(), 1);
    assert_eq!(testing::focused_node(), Some(save));

    user::click(testing::find_by_text("Nothing to press").expect("the text"));

    assert_eq!(testing::focused_node(), None);
  });
}

/// Typed text reaches a bound field one cluster at a time, the caret following it;
/// `Backspace` removes the cluster before the caret, and a multi-line entry takes a
/// newline where a single-line one does not.
#[test]
fn typed_text_edits_a_bound_entry() {
  with_platform(|| {
    let line = Signal::new(String::new());
    let lines = Signal::new(String::new());
    let [field, editor] = mount_nodes(|| {
      [
        BasicTextField::new(TextEntry::new().bind_value(line))
          .set_width(10)
          .build(),
        BasicTextEditor::new(TextEntry::new().set_lines(3).bind_value(lines)).build(),
      ]
    });

    user::type_text(field, "héllo\n");

    assert_eq!(line.get(), "héllo");
    assert_eq!(testing::render_to_caret(40, 12), Some((5, 0)));

    user::press_key(&KeyEvent::new("Backspace"));

    assert_eq!(line.get(), "héll");
    assert_eq!(testing::render_to_caret(40, 12), Some((4, 0)));

    user::type_text(editor, "a\nb");

    assert_eq!(lines.get(), "a\nb");
    assert_eq!(testing::focused_node(), Some(editor));
  });
}

/// `Tab` pressed as a user presses it moves focus to the next tab stop.
#[test]
fn a_pressed_tab_moves_focus() {
  with_platform(|| {
    let _ = mount(|| {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new()).set_label("First"),
          BasicButton::new(Activate::new()).set_label("Second"),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 2);
    let [first, second] =
      ["First", "Second"].map(|name| testing::find_by_role(Role::Button, name).expect("a button"));

    user::click(first);
    user::press_key(&KeyEvent::new("Tab"));

    assert_eq!(testing::focused_node(), Some(second));
  });
}

/// A closed overlay's content is out of the tree a reader searches; open, it is found.
#[test]
fn a_closed_overlay_hides_its_content_from_a_search() {
  with_platform(|| {
    let open = Signal::new(false);
    let anchor = NodeHandle::new();

    let _ = mount(move || {
      Column::new()
        .set_children((
          Container::new().set_height(1).bind_handle(anchor),
          Overlay::popup()
            .set_anchor(anchor)
            .bind_open(open)
            .set_child(Text::new("Inside")),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 4);

    assert_eq!(testing::find_by_text("Inside"), None);

    open.set(true);

    let _ = testing::render_to_text(10, 4);

    assert!(testing::find_by_text("Inside").is_some());
  });
}

/// A span flowing in a paragraph takes a click on the cells its paragraph draws it in,
/// and the span beside it takes none.
#[test]
fn a_user_clicks_a_span_where_it_flows() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new((0u32, 0u32)));
    let (beside, counted) = (Rc::clone(&clicks), Rc::clone(&clicks));

    let _ = mount(move || {
      RichText::new()
        .add_spans([
          Span::new("plain ")
            .handle_click(move || beside.set((beside.get().0 + 1, beside.get().1))),
          Span::new("click")
            .handle_click(move || counted.set((counted.get().0, counted.get().1 + 1))),
        ])
        .into_any()
        .build()
    });

    let _ = testing::render_to_text(14, 1);

    user::click(testing::find_by_text("click").expect("the span"));

    assert_eq!(clicks.get(), (0, 1));
  });
}

/// A box with a shadow takes a click on what shows of its own cells, never on the band
/// its shadow paints beyond them.
#[test]
fn a_user_clicks_a_shadowed_row_on_its_own_cells() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);
    let row = NodeHandle::new();

    let _ = mount(move || {
      Column::new()
        .set_children((
          Container::new()
            .set_width(6)
            .set_height(1)
            .set_shadow(Color::srgb(0., 0., 0.).with_alpha(0.5))
            .handle_click(move || counted.set(counted.get() + 1))
            .bind_handle(row)
            .set_child(Text::new("Row")),
          Text::new("Below"),
        ))
        .into_any()
        .build()
    });

    let _ = testing::render_to_text(10, 3);

    user::click(row.node().expect("the row built"));

    assert_eq!(clicks.get(), 1);
  });
}

/// A box half scrolled out of its scroll box takes a click on what shows of it, inside
/// the scroll box's clip, never on what lies above the scroll box where the rest of it
/// is scrolled.
#[test]
fn a_user_clicks_a_scrolled_box_where_it_shows() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new((0u32, 0u32)));
    let (above, counted) = (Rc::clone(&clicks), Rc::clone(&clicks));
    let tall = NodeHandle::new();

    let _ = mount(move || {
      Column::new()
        .set_children((
          Container::new()
            .set_height(2)
            .handle_click(move || above.set((above.get().0 + 1, above.get().1)))
            .set_child(Text::new("Above")),
          Container::new()
            .set_height(3)
            .set_overflow(Overflow::ScrollIfNeeded)
            .set_child(
              Column::new().set_children((
                Container::new()
                  .set_height(4)
                  .handle_click(move || counted.set((counted.get().0, counted.get().1 + 1)))
                  .bind_handle(tall)
                  .set_child(Text::new("tall")),
                Container::new().set_height(2).set_child(Text::new("short")),
              )),
            ),
          Text::new("Below"),
        ))
        .into_any()
        .build()
    });

    let _ = testing::render_to_text(12, 6);

    testing::trigger_mouse_scroll(2, 3, 0, 3);

    let _ = testing::render_to_text(12, 6);

    user::click(tall.node().expect("the tall box built"));

    assert_eq!(clicks.get(), (0, 1));
  });
}

/// A span whose first line is scrolled out of its scroll box takes a click on the first
/// of its lines that shows, never on what lies above the scroll box where its first line
/// is scrolled.
#[test]
fn a_user_clicks_a_wrapped_span_on_a_line_that_shows() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new((0u32, 0u32)));
    let (above, counted) = (Rc::clone(&clicks), Rc::clone(&clicks));

    let _ = mount(move || {
      Column::new()
        .set_children((
          Container::new()
            .set_height(1)
            .handle_click(move || above.set((above.get().0 + 1, above.get().1)))
            .set_child(Text::new("Above")),
          Container::new()
            .set_width(5)
            .set_height(1)
            .set_overflow(Overflow::ScrollIfNeeded)
            .set_child(
              RichText::new().add_spans([
                Span::new("ab "),
                Span::new("cd ef")
                  .handle_click(move || counted.set((counted.get().0, counted.get().1 + 1))),
              ]),
            ),
        ))
        .into_any()
        .build()
    });

    let _ = testing::render_to_text(8, 3);

    testing::trigger_mouse_scroll(0, 1, 0, 1);

    let shown = testing::render_to_text(8, 3);

    assert!(
      shown
        .lines()
        .nth(1)
        .is_some_and(|line| line.starts_with("ef")),
      "the second line shows: {shown:?}"
    );

    user::click(testing::find_by_text("cd ef").expect("the span"));

    assert_eq!(clicks.get(), (0, 1));
  });
}

/// A box laid out wholly below the screen shows no cell, so a user cannot click it.
#[test]
#[should_panic(expected = "shows no cell on screen")]
fn a_user_cannot_click_a_box_below_the_screen() {
  with_platform(|| {
    let below = NodeHandle::new();

    let _ = mount(move || {
      Column::new()
        .set_children((
          Container::new()
            .set_min_height(3)
            .set_child(Text::new("Top")),
          Container::new()
            .set_min_height(2)
            .handle_click(|| {})
            .bind_handle(below)
            .set_child(Text::new("Below")),
        ))
        .into_any()
        .build()
    });

    let _ = testing::render_to_text(6, 3);

    user::click(below.node().expect("the box below built"));
  });
}
