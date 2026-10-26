//! Shortcuts in the terminal: a keymap's act while focus is inside the view it serves,
//! and a control's or an activatable row's from anywhere in the window.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_tui::{
  BasicListbox,
  MenuEntry,
  behavior::{Activate, Radio, TextEntry},
  platform::terminal::mount,
  prelude::*,
  testing,
};

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

    let _ = mount(move || {
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

/// `Ctrl-S`.
const CTRL_S: Shortcut = Shortcut::new(Modifiers::CTRL, "s");

/// A counter and the action adding one to it.
fn counter() -> (Rc<Cell<u32>>, impl Fn() + Clone + 'static) {
  let count = Rc::new(Cell::new(0));
  let counted = Rc::clone(&count);

  (count, move || counted.set(counted.get() + 1))
}

/// A button named `name`, running `action` on its activation.
fn button(name: &str, action: impl Fn() + 'static) -> BasicButton {
  BasicButton::new(Activate::new())
    .set_label(name)
    .handle_activate(action)
}

/// A control's shortcut activates it from anywhere in the window: with nothing focused,
/// with focus on another control and in a text entry, whose value it leaves alone, and
/// the access tree reports it in the W3C form.
#[test]
fn a_controls_shortcut_activates_it_from_anywhere() {
  with_platform(|| {
    let (saves, save) = counter();
    let value = Signal::new(String::new());

    let _ = mount(move || {
      Column::new()
        .set_children((
          button("Save", save.clone()).set_shortcut(CTRL_S),
          button("Other", || {}),
          BasicTextField::new(TextEntry::new().bind_value(value)).set_label("Note"),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 6);

    testing::trigger_key("Ctrl+s");

    assert_eq!(saves.get(), 1, "with nothing focused");
    assert_eq!(testing::focused_node(), None, "focus does not move");

    testing::trigger_key("Tab");
    testing::trigger_key("Tab");
    testing::trigger_key("Ctrl+s");

    assert_eq!(saves.get(), 2, "with focus on another control");

    testing::trigger_key("Tab");
    testing::trigger_key("x");
    testing::trigger_key("Ctrl+s");

    assert_eq!(saves.get(), 3, "with focus in a text entry");
    assert_eq!(value.get(), "x", "the entry's value is left alone");

    let outline = testing::access_outline();

    assert!(
      outline.contains("Button \"Save\" [shortcut Control+S]"),
      "{outline}"
    );
  });
}

/// A shortcut holding no modifier is the key of the text entry holding focus, typed
/// into it, and acts wherever no entry holds focus.
#[test]
fn a_shortcut_without_a_modifier_leaves_an_entry_its_key() {
  with_platform(|| {
    let (news, new) = counter();
    let value = Signal::new(String::new());

    let _ = mount(move || {
      Column::new()
        .set_children((
          button("New", new.clone()).set_shortcut(Shortcut::new(Modifiers::NONE, "n")),
          BasicTextField::new(TextEntry::new().bind_value(value)).set_label("Note"),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 4);

    testing::trigger_key("n");

    assert_eq!(news.get(), 1, "no entry holds focus");

    testing::trigger_key("Tab");
    testing::trigger_key("Tab");
    testing::trigger_key("n");

    assert_eq!(news.get(), 1, "the entry takes its key");
    assert_eq!(value.get(), "n");
  });
}

/// A collection's shortcut focuses the member holding its stop, choosing nothing, and
/// one whose members hold no stop is passed over, the next control the key is the
/// shortcut of acting.
#[test]
fn a_collections_shortcut_focuses_its_stop() {
  with_platform(|| {
    let (saves, save) = counter();
    let (chooses, choose) = counter();
    let chosen = Signal::new(Some("b"));

    let _ = mount(move || {
      Column::new()
        .set_children((
          BasicRadioGroup::new()
            .set_label("Size")
            .bind_chosen(chosen)
            .handle_choose(move |_| choose())
            .set_shortcut(Shortcut::new(Modifiers::CTRL, "g"))
            .set_child(Column::new().set_children((
              BasicRadio::new(Radio::new(Some("a"))).set_label("a"),
              BasicRadio::new(Radio::new(Some("b"))).set_label("b"),
            )))
            .into_any(),
          BasicListbox::<Option<&str>>::new()
            .set_accessible_name("Empty")
            .set_shortcut(Shortcut::new(Modifiers::CTRL, "s"))
            .into_any(),
          button("Save", save)
            .set_shortcut(Shortcut::new(Modifiers::CTRL, "s"))
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 8);

    testing::trigger_key("Ctrl+g");

    assert_eq!(
      testing::focused_node(),
      testing::find_by_role(Role::RadioButton, "b"),
      "the chosen radio takes focus"
    );
    assert_eq!(
      (chooses.get(), chosen.get()),
      (0, Some("b")),
      "nothing is chosen"
    );

    testing::trigger_key("Ctrl+s");

    assert_eq!(saves.get(), 1, "the listbox holding no stop is passed over");
  });
}

/// A shortcut passes over a disabled control to the next one taking it in the scene's
/// child order, and an open modal excludes every control outside it, its own taking the
/// key in their place.
#[test]
fn a_shortcut_passes_over_a_disabled_control_and_one_a_modal_excludes() {
  with_platform(|| {
    let (drafts, draft) = counter();
    let (saves, save) = counter();
    let (confirms, confirm) = counter();
    let open = Signal::new(false);
    let confirming = Signal::new(false);

    let _ = mount(move || {
      Column::new()
        .set_children((
          button("Draft", draft.clone())
            .set_disabled(true)
            .set_shortcut(CTRL_S)
            .into_any(),
          button("Save", save.clone()).set_shortcut(CTRL_S).into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_child(
              Column::new().set_children((
                button("Cancel", || {}),
                Container::new()
                  .derive_hidden(move || !confirming.get())
                  .set_child(button("Confirm", confirm.clone()).set_shortcut(CTRL_S)),
              )),
            )
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 6);

    testing::trigger_key("Ctrl+s");

    assert_eq!(
      (drafts.get(), saves.get()),
      (0, 1),
      "the disabled one passed over"
    );

    open.set(true);
    let _ = testing::render_to_text(20, 6);
    testing::trigger_key("Ctrl+s");

    assert_eq!(
      (saves.get(), confirms.get()),
      (1, 0),
      "the modal excludes the control beneath, and its own is hidden"
    );

    confirming.set(true);
    let _ = testing::render_to_text(20, 6);
    testing::trigger_key("Ctrl+s");

    assert_eq!(
      (saves.get(), confirms.get()),
      (1, 1),
      "the modal's own control takes it"
    );
  });
}

/// A control a `Visibility` has hidden takes no shortcut, and takes it again once shown.
#[test]
fn a_control_visibility_hides_takes_no_shortcut() {
  with_platform(|| {
    let (saves, save) = counter();
    let shown = Signal::new(true);

    let _ = mount(move || {
      Visibility::new()
        .derive_shown(move || shown.get())
        .set_child(button("Save", save.clone()).set_shortcut(CTRL_S))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 2);

    testing::trigger_key("Ctrl+s");
    shown.set(false);
    testing::trigger_key("Ctrl+s");

    assert_eq!(saves.get(), 1, "hidden, it takes nothing");

    shown.set(true);
    testing::trigger_key("Ctrl+s");

    assert_eq!(saves.get(), 2, "shown again");
  });
}

/// A keymap binding a key inside its view runs first, and the control taking the same
/// key acts wherever focus is outside the view.
#[test]
fn a_keymaps_binding_wins_over_a_controls_shortcut_inside_its_view() {
  with_platform(|| {
    let (bound, bind) = counter();
    let (saves, save) = counter();

    let _ = mount(move || {
      Column::new()
        .set_children((
          Keymap::new()
            .handle_shortcut(CTRL_S, bind.clone())
            .set_child(button("Inside", || {})),
          button("Save", save.clone()).set_shortcut(CTRL_S),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 4);

    testing::trigger_key("Tab");
    testing::trigger_key("Ctrl+s");

    assert_eq!(
      (bound.get(), saves.get()),
      (1, 0),
      "inside the keymap's view"
    );

    testing::trigger_key("Tab");
    testing::trigger_key("Ctrl+s");

    assert_eq!((bound.get(), saves.get()), (1, 1), "outside it");
  });
}

/// An activatable row's shortcut activates it as a click does and is reported, and a
/// box handling no click ignores its shortcut and reports none.
#[test]
fn a_rows_shortcut_activates_it_and_a_plain_boxs_does_nothing() {
  with_platform(|| {
    let (clicks, click) = counter();

    let _ = mount(move || {
      Column::new()
        .set_children((
          Container::new()
            .set_label("Row")
            .set_role(Role::Group)
            .set_shortcut(CTRL_S)
            .handle_click(click.clone())
            .set_child(Text::new("Row")),
          Container::new()
            .set_label("Plain")
            .set_role(Role::Group)
            .set_shortcut(Shortcut::new(Modifiers::CTRL, "p"))
            .set_child(Text::new("Plain")),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 4);

    testing::trigger_key("Ctrl+s");
    testing::trigger_key("Ctrl+p");

    assert_eq!(clicks.get(), 1);

    let outline = testing::access_outline();

    assert!(
      outline.contains("Group \"Row\" [shortcut Control+S]"),
      "{outline}"
    );
    assert!(outline.contains("Group \"Plain\"\n"), "{outline}");
  });
}
