//! A keymap in the terminal: its shortcuts act while focus is inside the view it serves.

use std::{cell::RefCell, rc::Rc};

use skene_terminal::{self as terminal, Activate, MenuEntry, prelude::*, testing};

use crate::common::with_platform;

/// A shortcut pressed with focus inside the view served runs its action in place of the
/// response the key would have and the ancestors', and any other key passes by: a
/// keymap binding `Shift-F10` keeps the context menu around it from opening, which the
/// same key opens with focus outside the view.
#[test]
fn a_keymaps_shortcut_runs_its_action() {
  with_platform(|| {
    let runs = Rc::new(RefCell::new(0u32));
    let heard = Rc::new(RefCell::new(Vec::new()));
    let (count, log) = (Rc::clone(&runs), Rc::clone(&heard));

    let _ = terminal::mount(move || {
      ContextMenu::new()
        .set_items([MenuEntry::new("Rename")])
        .set_child(
          Container::new()
            .handle_key_down(move |event| log.borrow_mut().push(event.key.to_string()))
            .set_child(
              Column::new().set_children((
                Keymap::new()
                  .handle_shortcut(Shortcut::new(Modifiers::SHIFT, "F10"), move || {
                    *count.borrow_mut() += 1;
                  })
                  .set_child(
                    Container::new()
                      .set_child(BasicButton::new(Activate::new()).set_label("inside")),
                  ),
                BasicButton::new(Activate::new()).set_label("outside"),
              )),
            ),
        )
        .into_any()
        .build()
    });
    let menus = || testing::render_to_text(20, 10).matches("Rename").count();
    let _ = testing::render_to_text(20, 10);

    testing::trigger_key("Tab");
    testing::trigger_key("Shift+F10");

    assert_eq!(*runs.borrow(), 1);
    assert_eq!(menus(), 0, "the menu is kept from opening");
    assert!(heard.borrow().is_empty(), "the ancestors hear nothing");

    testing::trigger_key("s");

    assert_eq!(*runs.borrow(), 1);
    assert_eq!(*heard.borrow(), ["s"]);

    testing::trigger_key("Tab");
    testing::trigger_key("Shift+F10");

    assert_eq!(
      *runs.borrow(),
      1,
      "outside the view the shortcut is no keymap's"
    );
    assert_eq!(menus(), 1, "and the menu opens");
  });
}
