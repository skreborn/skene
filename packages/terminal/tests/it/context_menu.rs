//! Context menus in cells: a menu wrapping a view opens at the right-pressed cell in
//! place of the terminal's own, its first usable item focused, its keys walk the enabled
//! items, an item's shortcut runs it from the served view, the menu key opens it at the
//! focused node, a standard item follows the clipboard the launch offers, the deepest
//! menu along the path wins, `Escape` closes it before touching the selection, and the
//! standard menu keeps an entry's selection while it acts on it.

use std::{cell::RefCell, rc::Rc};

use skene_terminal::{
  behavior::{Activate, TextEntry},
  platform::terminal::mount,
  prelude::*,
  testing,
};

use crate::{common, common::with_platform};

/// The menu items a reader finds open, in tree order.
fn items() -> Vec<skene_terminal::NodeId> {
  testing::find_all_by_role(Role::MenuItem)
}

/// Whether a reader finds `node` disabled.
fn is_disabled(node: skene_terminal::NodeId) -> bool {
  common::read(node).is_disabled()
}

/// A right press on the served view opens the list with its top-start corner at the
/// pressed cell and its first item focused, a click on an item runs it and closes the
/// list, `Escape` closes it, and an item's shortcut runs it from the served view without
/// opening anything.
#[test]
fn a_context_menu_opens_at_the_press_and_runs_the_chosen_item() {
  with_platform(|| {
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
    let _ = mount({
      let log = Rc::clone(&log);

      move || {
        ContextMenu::new()
          .set_items([
            MenuEntry::new("Rename")
              .set_shortcut(Shortcut::new(Modifiers::NONE, "F2"))
              .handle_activate({
                let log = Rc::clone(&log);

                move || log.borrow_mut().push("rename")
              }),
            MenuEntry::separator(),
            MenuEntry::new("Delete").handle_activate(move || log.borrow_mut().push("delete")),
          ])
          .set_child(
            Container::new()
              .set_focus_order(FocusOrder::Auto)
              .set_width(30)
              .set_height(8)
              .set_child(Text::new("note")),
          )
          .into_any()
          .build()
      }
    });
    let _ = testing::render_to_text(40, 12);

    testing::trigger_mouse_right_click(2, 1);

    let near = testing::locate(&testing::render_to_text(40, 12), "Rename");

    assert_eq!(items().len(), 2, "a separator is no item");
    assert_eq!(
      testing::focused_node(),
      items().first().copied(),
      "the first item takes focus"
    );

    testing::trigger_key("Escape");

    assert!(items().is_empty(), "Escape closes it");

    testing::trigger_mouse_right_click(6, 3);

    let frame = testing::render_to_text(40, 12);
    let far = testing::locate(&frame, "Rename");

    assert_eq!(
      (far.0 - near.0, far.1 - near.1),
      (4, 2),
      "the list moves with the press: {frame}"
    );

    let (x, y) = testing::locate(&frame, "Delete");

    testing::trigger_mouse_click(x, y);

    assert_eq!(*log.borrow(), vec!["delete"]);
    assert!(items().is_empty(), "running an item closes the list");

    testing::trigger_key("Tab");
    testing::trigger_key("F2");

    assert_eq!(
      *log.borrow(),
      vec!["delete", "rename"],
      "the shortcut is the item's"
    );
    assert!(items().is_empty(), "a shortcut opens nothing");
  });
}

/// The arrows walk the enabled items with wrapping, skipping a disabled one, `Home` and
/// `End` reach the ends, and `Enter` runs the focused item and closes the list.
#[test]
fn the_keys_walk_the_enabled_items_with_wrapping() {
  with_platform(|| {
    let chosen = Signal::new(String::new());
    let _ = mount(move || {
      let item = move |label: &'static str| {
        MenuEntry::new(label).handle_activate(move || chosen.set(label.to_owned()))
      };

      ContextMenu::new()
        .set_items([item("one"), item("two").set_disabled(true), item("three")])
        .set_child(
          Container::new()
            .set_focus_order(FocusOrder::Auto)
            .set_child(Text::new("note")),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 8);

    testing::trigger_mouse_right_click(1, 0);

    let entries = items();
    let focused = |index: usize| testing::focused_node() == Some(entries[index]);

    assert_eq!(entries.len(), 3);
    assert!(is_disabled(entries[1]), "a reader finds the item disabled");
    assert!(focused(0));

    for (key, index, why) in [
      ("ArrowDown", 2, "the disabled item is skipped"),
      ("ArrowDown", 0, "the walk wraps"),
      ("ArrowUp", 2, "and wraps back"),
      ("Home", 0, "Home reaches the first"),
      ("End", 2, "End reaches the last"),
    ] {
      testing::trigger_key(key);

      assert!(focused(index), "{why}");
    }

    testing::trigger_key("Enter");

    assert_eq!(chosen.get_untracked(), "three");
    assert!(items().is_empty(), "running it closes the list");
  });
}

/// A standard item is listed where the launch offers its action, disabled where the
/// action does not apply, and runs the action through the terminal; a menu with nothing
/// usable opens nothing, and the terminal's own menu stays closed as well.
#[test]
fn a_standard_item_follows_the_launchs_clipboard() {
  let menu = |child: AnyView| {
    ContextMenu::new()
      .set_items([MenuEntry::copy(), MenuEntry::paste()])
      .set_child(child)
      .into_any()
      .build()
  };

  common::with_launch(
    skene_terminal::Launch::new().set_clipboard(Clipboard::COPY),
    || {
      let _ = mount(|| menu(Text::new("selectable text").set_selectable(true).into_any()));
      let _ = testing::render_to_text(30, 8);

      testing::trigger_mouse_press(0, 0);
      testing::trigger_mouse_drag(10, 0);
      testing::trigger_mouse_release(10, 0);
      testing::trigger_mouse_right_click(2, 0);

      let entries = items();

      assert_eq!(
        entries.len(),
        1,
        "an action the launch withholds is not listed"
      );
      assert_eq!(
        testing::accessible_name(entries[0]).as_deref(),
        Some("Copy")
      );

      let _ = testing::render_to_text(30, 8);

      testing::user::click(entries[0]);

      assert_eq!(testing::take_copied().as_deref(), Some("selectable"));
      assert!(items().is_empty(), "running it closes the list");
    },
  );

  with_platform(|| {
    let value = Signal::new("alpha".to_owned());
    let _ = mount(|| {
      menu(
        BasicTextField::new(TextEntry::new().bind_value(value))
          .set_width(20)
          .into_any(),
      )
    });
    let _ = testing::render_to_text(30, 8);

    testing::trigger_key("Tab");
    testing::trigger_key("End");
    testing::trigger_mouse_right_click(2, 0);

    let entries = items();

    assert_eq!(entries.len(), 2, "an offered action is listed");
    assert!(is_disabled(entries[0]), "copying applies to no selection");
    assert_eq!(
      testing::focused_node(),
      Some(entries[1]),
      "focus lands on the usable item"
    );
  });

  with_platform(|| {
    let value = Signal::new("alpha".to_owned());
    let _ = mount(|| {
      menu(
        BasicTextField::new(TextEntry::new().bind_value(value))
          .set_read_only(true)
          .set_width(20)
          .into_any(),
      )
    });
    let _ = testing::render_to_text(30, 8);

    testing::trigger_key("Tab");
    testing::trigger_key("End");
    testing::trigger_mouse_right_click(2, 0);

    let frame = testing::render_to_text(30, 8);

    assert!(items().is_empty(), "nothing usable, nothing opens: {frame}");
    assert!(
      !frame.contains("Copy") && !frame.contains("Paste"),
      "and the terminal's own menu stays closed: {frame}"
    );
  });
}

/// A menu serving an existing node hears a request from anywhere beneath it, and a
/// closer menu takes the request before it.
#[test]
fn the_deepest_menu_along_the_path_wins() {
  with_platform(|| {
    let _ = mount(|| {
      let root = Column::new()
        .set_children((
          Text::new("plain").into_any(),
          ContextMenu::new()
            .set_items([MenuEntry::new("inner")])
            .set_child(Container::new().set_child(Text::new("leaf")))
            .into_any(),
        ))
        .build();
      let _ = ContextMenu::new()
        .set_items([MenuEntry::new("outer")])
        .serve(root);

      root
    });
    let open = || {
      items()
        .into_iter()
        .filter_map(testing::accessible_name)
        .collect::<Vec<_>>()
    };
    let _ = testing::render_to_text(30, 8);

    testing::trigger_mouse_right_click(1, 0);

    assert_eq!(open(), ["outer"], "the root's menu serves the plain run");

    testing::trigger_key("Escape");
    testing::trigger_mouse_right_click(1, 1);

    assert_eq!(open(), ["inner"], "the closer menu takes the request");
  });
}

/// A menu around a reactive view hears a request from the view's content, in place of
/// the terminal's own.
#[test]
fn a_context_menu_around_a_reactive_view_hears_its_content() {
  with_platform(|| {
    let _ = mount(|| {
      ContextMenu::new()
        .set_items([MenuEntry::new("Rename")])
        .set_child(AnyView::new(|| {
          derived(|| {
            Container::new()
              .set_child(Text::new("note").set_selectable(true))
              .build()
          })
        }))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 8);

    testing::trigger_mouse_press(0, 0);
    testing::trigger_mouse_drag(3, 0);
    testing::trigger_mouse_release(3, 0);
    testing::trigger_mouse_right_click(1, 0);

    let frame = testing::render_to_text(30, 8);

    assert!(frame.contains("Rename"), "{frame}");
    assert!(
      !frame.contains("Copy"),
      "the terminal's menu stays closed: {frame}"
    );
  });
}

/// A separator is as thick as the menu says, in whole cells.
#[test]
fn a_context_menu_sizes_its_separators() {
  with_platform(|| {
    let _ = mount(|| {
      ContextMenu::new()
        .set_separator_thickness(Dimension::Length(3))
        .set_items([
          MenuEntry::new("A"),
          MenuEntry::separator(),
          MenuEntry::new("B"),
        ])
        .set_child(Text::new("note"))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 10);

    testing::trigger_mouse_right_click(1, 0);

    let frame = testing::render_to_text(30, 10);
    let (_, a) = testing::locate(&frame, "A");
    let (_, b) = testing::locate(&frame, "B");

    assert_eq!(
      b - a,
      4,
      "three rows of separator between the items: {frame}"
    );
  });
}

/// An item view draws each item from its content while the menu keeps the item's
/// name and behavior: the label and the shortcut composed as the view says, the
/// accessible name still the label.
#[test]
fn an_item_view_draws_each_item() {
  with_platform(|| {
    let _ = mount(|| {
      ContextMenu::new()
        .set_item_view(|content| {
          Text::new(format!(
            "{} - {}",
            content.label(),
            content
              .shortcut()
              .map_or_else(|| "none".to_owned(), |shortcut| shortcut.to_string())
          ))
        })
        .set_items([MenuEntry::new("Rename").set_shortcut(Shortcut::new(Modifiers::NONE, "F2"))])
        .set_child(Text::new("note"))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 8);

    testing::trigger_mouse_right_click(1, 0);

    let frame = testing::render_to_text(30, 8);

    assert!(frame.contains("Rename - F2"), "{frame}");
    assert_eq!(
      items()
        .into_iter()
        .map(testing::accessible_name)
        .collect::<Vec<_>>(),
      [Some("Rename".to_owned())]
    );
  });
}

/// A right press on the served view opens its menu at the press instead of the
/// terminal's own, an item lights under the pointer, a click on it runs it and closes
/// the menu, and a right press elsewhere still opens the terminal's copy menu.
#[test]
fn a_menu_on_a_view_takes_the_right_press_from_the_standard_one() {
  with_platform(|| {
    let chosen = Signal::new(String::new());
    let _ = mount(|| {
      Column::new()
        .set_children((
          ContextMenu::new()
            .set_items([
              MenuEntry::new("Rename").handle_activate(move || chosen.set("rename".to_owned())),
              MenuEntry::separator(),
              MenuEntry::new("Delete").handle_activate(move || chosen.set("delete".to_owned())),
            ])
            .set_border_width(Inset::all(1))
            .set_border_color(Color::srgb(0.5, 0.5, 0.5))
            .set_child(Text::new("note")),
          Text::new("selectable text").set_selectable(true),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 8);

    testing::trigger_mouse_right_click(1, 0);

    let frame = testing::render_to_text(30, 8);

    assert!(
      frame.contains("Rename") && frame.contains("Delete"),
      "{frame}"
    );
    assert!(
      !frame.contains("Copy"),
      "the view's menu replaces the terminal's: {frame}"
    );

    let (x, y) = common::locate(&frame, "Delete");

    testing::trigger_mouse_click(x, y);

    assert_eq!(chosen.get_untracked(), "delete");
    assert!(
      !testing::render_to_text(30, 8).contains("Rename"),
      "running an item closes the menu"
    );

    testing::trigger_mouse_press(0, 1);
    testing::trigger_mouse_drag(9, 1);
    testing::trigger_mouse_release(9, 1);
    testing::trigger_mouse_right_click(2, 1);

    let frame = testing::render_to_text(30, 8);

    assert!(
      frame.contains("Copy"),
      "the terminal's menu serves the rest: {frame}"
    );
    assert!(!frame.contains("Rename"), "{frame}");
  });
}

/// The menu key opens the focused view's menu below it, the arrows walk the enabled
/// items, `Enter` runs the focused one, and `Escape` closes the menu.
#[test]
fn the_menu_key_opens_at_the_focused_view_and_the_keys_walk_it() {
  with_platform(|| {
    let chosen = Signal::new(String::new());
    let _ = mount(|| {
      Column::new()
        .set_children((
          Text::new("heading"),
          ContextMenu::new()
            .set_items([
              MenuEntry::new("one").handle_activate(move || chosen.set("one".to_owned())),
              MenuEntry::new("two").set_disabled(true),
              MenuEntry::new("three").handle_activate(move || chosen.set("three".to_owned())),
            ])
            .set_child(BasicButton::new(Activate::new()).set_label("target")),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 8);

    testing::trigger_key("Tab");
    testing::trigger_key("Shift+F10");

    let frame = testing::render_to_text(30, 8);
    let (_, button_row) = common::locate(&frame, "target");
    let (_, first_row) = common::locate(&frame, "one");

    assert!(
      first_row > button_row,
      "the menu opens below the button: {frame}"
    );

    testing::trigger_key("ArrowDown");
    testing::trigger_key("Enter");

    assert_eq!(
      chosen.get_untracked(),
      "three",
      "the disabled item is skipped"
    );
    assert!(
      !testing::render_to_text(30, 8).contains("one"),
      "the menu closed"
    );

    testing::trigger_key("ContextMenu");

    assert!(
      testing::render_to_text(30, 8).contains("one"),
      "the menu key opens it too"
    );

    testing::trigger_key("Escape");

    assert!(
      !testing::render_to_text(30, 8).contains("one"),
      "Escape closes it"
    );
  });
}

/// The standard menu over a focused entry keeps the entry's selection while the
/// menu holds focus, so `Cut` removes what was selected and the entry's caret is
/// where the right press put it afterwards.
#[test]
fn the_standard_menu_acts_on_the_entrys_selection() {
  with_platform(|| {
    let value = Signal::new(String::from("alpha beta"));
    let _ = mount(|| {
      BasicTextField::new(TextEntry::new().bind_value(value))
        .set_width(20)
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 8);

    testing::trigger_key("Tab");
    testing::trigger_mouse_right_click(2, 0);

    let frame = testing::render_to_text(30, 8);

    assert!(frame.contains("Cut"), "{frame}");

    testing::trigger_key("Escape");
    testing::trigger_key("Tab");
    testing::trigger_mouse_right_click(2, 0);

    let frame = testing::render_to_text(30, 8);
    let (x, y) = common::locate(&frame, "Cut");

    testing::trigger_mouse_click(x, y);

    assert_eq!(testing::take_copied().as_deref(), Some("alpha beta"));
    assert_eq!(value.get_untracked(), "", "Cut removed the selected value");
  });
}
