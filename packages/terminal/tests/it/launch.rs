//! The launch configuration: which clipboard operations the app offers, and whether
//! it takes the mouse.

use skene_terminal::{self as terminal, Activate, Label, TextEntry, prelude::*, testing};

use crate::common::{blue, colors_at, red, rgb, white, with_launch};

/// The selection highlight the tests read back.
fn highlight() -> Color {
  Color::srgb(0.2, 0.35, 0.53)
}

/// Mounts a selectable run over a field bound to `value`, under a typography painting
/// selections in white on the highlight, and focuses the field by keyboard.
fn mount_document(value: Signal<String>) {
  let _ = terminal::mount(move || {
    Theme::new(
      Typography::default()
        .set_selection_ink(white())
        .set_selection_surface(highlight()),
    )
    .set_child_builder(move || {
      Column::new()
        .set_children((
          Text::new("alpha beta").set_selectable(true),
          BasicTextField::new(TextEntry::new().bind_value(value)).set_width(20),
        ))
        .into_any()
    })
    .into_any()
    .build()
  });
  let _ = testing::render_to_text(20, 4);

  testing::trigger_key("Tab");
}

/// A terminal whose clipboard answers offers every operation whatever the launch
/// chose: the keys copy, cut and paste, and the document is selectable.
#[test]
fn an_answering_clipboard_offers_every_operation() {
  with_launch(
    terminal::Launch::new().set_clipboard(Clipboard::NONE),
    || {
      let value = Signal::new(String::from("value"));

      testing::trigger_answering_clipboard(true);
      mount_document(value);
      testing::trigger_key("Ctrl+c");

      assert_eq!(testing::take_copied().as_deref(), Some("value"));

      testing::trigger_key("Ctrl+v");

      assert!(
        testing::take_paste_requested(),
        "Ctrl+V reads the clipboard"
      );

      testing::trigger_key("Ctrl+x");

      assert_eq!(value.get_untracked(), "", "Ctrl+X cuts");

      testing::trigger_mouse_press(0, 0);
      testing::trigger_mouse_drag(4, 0);
      testing::trigger_mouse_release(4, 0);

      let colors = testing::render_to_colors(20, 4);

      assert!(
        colors[..4]
          .iter()
          .all(|(_, background)| *background == Some(rgb(highlight()))),
        "a drag over the run selects it"
      );
    },
  );
}

/// Without a clipboard, nothing in the document is selectable, no menu opens, the
/// clipboard keys are ordinary keys, and the terminal's own paste still inserts.
#[test]
fn no_clipboard_offers_no_selection_menu_or_key() {
  with_launch(
    terminal::Launch::new().set_clipboard(Clipboard::NONE),
    || {
      let value = Signal::new(String::from("value"));

      mount_document(value);

      let colors = testing::render_to_colors(20, 4);

      assert_eq!(
        colors[20],
        (Some(rgb(white())), Some(rgb(highlight()))),
        "keyboard focus still selects the field's value"
      );

      testing::trigger_key("Ctrl+c");

      assert_eq!(testing::take_copied(), None, "Ctrl+C copies nothing");

      testing::trigger_key("Ctrl+x");

      assert_eq!(value.get_untracked(), "value", "Ctrl+X cuts nothing");

      testing::trigger_key("Ctrl+v");

      assert!(!testing::take_paste_requested(), "Ctrl+V reads nothing");

      testing::trigger_mouse_right_click(2, 1);

      assert!(
        !testing::render_to_text(40, 6).contains("Paste"),
        "no edit menu opens"
      );

      testing::trigger_paste("x");

      assert_eq!(
        value.get_untracked(),
        "x",
        "the terminal's own paste inserts"
      );

      testing::trigger_mouse_move(2, 0);

      assert_eq!(
        testing::mouse_cursor(),
        None,
        "the run shows no text cursor"
      );

      testing::trigger_mouse_press(0, 0);
      testing::trigger_mouse_drag(4, 0);
      testing::trigger_mouse_release(4, 0);

      let colors = testing::render_to_colors(20, 4);

      assert!(
        colors[..10]
          .iter()
          .all(|(_, background)| background.is_none()),
        "a drag over the run selects nothing: {:?}\n{}",
        &colors[..10],
        testing::render_to_text(20, 4)
      );

      testing::trigger_mouse_right_click(2, 0);

      assert!(
        !testing::render_to_text(40, 6).contains("Copy"),
        "no copy menu opens"
      );
    },
  );
}

/// Copy alone: `Ctrl+C` and `Ctrl+X` work, the edit menu lists `Cut` and `Copy`
/// without `Paste`, and `Ctrl+V` reads nothing.
#[test]
fn copy_alone_offers_cut_and_copy() {
  with_launch(
    terminal::Launch::new().set_clipboard(Clipboard::COPY),
    || {
      let value = Signal::new(String::from("value"));

      mount_document(value);
      testing::trigger_key("Ctrl+c");

      assert_eq!(testing::take_copied().as_deref(), Some("value"));

      testing::trigger_key("Ctrl+v");

      assert!(!testing::take_paste_requested(), "Ctrl+V reads nothing");

      testing::trigger_mouse_right_click(2, 1);

      let menu = testing::render_to_text(40, 6);

      assert!(
        menu.contains("Cut") && menu.contains("Copy") && !menu.contains("Paste"),
        "the edit menu lists Cut and Copy alone: {menu}"
      );

      testing::trigger_key("Escape");
      testing::trigger_key("Ctrl+x");

      assert_eq!(testing::take_copied().as_deref(), Some("value"));
      assert_eq!(value.get_untracked(), "", "Ctrl+X cut the value");
    },
  );
}

/// Paste alone: `Ctrl+V` reads, the edit menu lists `Paste` alone, and `Ctrl+C` and
/// `Ctrl+X` do nothing.
#[test]
fn paste_alone_offers_paste() {
  with_launch(
    terminal::Launch::new().set_clipboard(Clipboard::PASTE),
    || {
      let value = Signal::new(String::from("value"));

      mount_document(value);
      testing::trigger_key("Ctrl+c");

      assert_eq!(testing::take_copied(), None, "Ctrl+C copies nothing");

      testing::trigger_key("Ctrl+x");

      assert_eq!(value.get_untracked(), "value", "Ctrl+X cuts nothing");

      testing::trigger_mouse_right_click(2, 1);

      let menu = testing::render_to_text(40, 6);

      assert!(
        menu.contains("Paste") && !menu.contains("Cut") && !menu.contains("Copy"),
        "the edit menu lists Paste alone: {menu}"
      );

      testing::trigger_key("Escape");
      testing::trigger_key("Ctrl+v");

      assert!(testing::take_paste_requested(), "Ctrl+V staged a read");

      testing::trigger_clipboard_reply("pasted");

      assert_eq!(value.get_untracked(), "pasted");
    },
  );
}

/// With the mouse off, no mouse event acts: a click clicks nothing and hover shows no
/// pointer shape, while the keyboard still operates the controls.
#[test]
fn mouse_off_ignores_every_mouse_event() {
  with_launch(terminal::Launch::new().set_mouse(false), || {
    let clicks = Signal::new(0);
    let _ = terminal::mount(move || {
      BasicButton::new(Activate::new())
        .handle_activate(move || clicks.set(clicks.get() + 1))
        .set_label(Label::new(Text::new("Press")))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 4);

    testing::trigger_mouse_move(1, 0);

    assert_eq!(testing::mouse_cursor(), None);

    testing::trigger_mouse_click(1, 0);

    assert_eq!(clicks.get_untracked(), 0, "a click clicks nothing");

    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    assert_eq!(clicks.get_untracked(), 1, "the keyboard still clicks");
  });
}

/// Mounts a button counting its clicks at the top of the frame.
fn mount_button() -> Signal<u32> {
  let clicks = Signal::new(0);

  let _ = terminal::mount(move || {
    BasicButton::new(Activate::new())
      .set_label(Label::new(Text::new("press")))
      .handle_activate(move || clicks.update(|count| *count += 1))
      .into_any()
      .build()
  });
  let _ = testing::render_to_text(20, 2);

  clicks
}

/// The mouse key hands the mouse to the terminal—the hover path empties and every
/// mouse event is ignored—and pressed again takes it back; so does any other key,
/// which then acts as usual.
#[test]
fn the_mouse_key_releases_the_mouse_and_any_key_takes_it_back() {
  with_launch(terminal::Launch::new(), || {
    let clicks = mount_button();

    testing::trigger_mouse_move(2, 0);

    assert_eq!(testing::mouse_cursor(), Some(MouseCursor::Click));

    testing::trigger_key("Alt+m");

    assert_eq!(testing::mouse_cursor(), None, "the hover path empties");

    testing::trigger_mouse_move(2, 0);
    testing::trigger_mouse_click(2, 0);

    assert_eq!(testing::mouse_cursor(), None);
    assert_eq!(clicks.get_untracked(), 0, "the click never reaches the app");

    testing::trigger_key("Alt+M");
    testing::trigger_mouse_click(2, 0);

    assert_eq!(
      clicks.get_untracked(),
      1,
      "the key, in either case, takes the mouse back"
    );
    assert_eq!(testing::mouse_cursor(), Some(MouseCursor::Click));

    testing::trigger_key("Alt+m");
    testing::trigger_key("Tab");
    testing::trigger_mouse_click(2, 0);

    assert_eq!(clicks.get_untracked(), 2, "any key takes the mouse back");
  });
}

/// The mouse key cancels a press in progress: the node the press reached hears the
/// cancel and holds `Pressed` no longer.
#[test]
fn the_mouse_key_cancels_a_press_in_progress() {
  with_launch(terminal::Launch::new(), || {
    let cancels = Signal::new(0u32);

    let _ = terminal::mount(move || {
      Container::new()
        .set_width(4)
        .set_height(2)
        .set_background(red().when(State::Pressed, blue()))
        .handle_pointer_cancel(move |_| cancels.update(|count| *count += 1))
        .into_any()
        .build()
    });
    let color = || colors_at(10, 3, 0, 0).1;

    assert_eq!(color(), Some(rgb(red())));

    testing::trigger_mouse_press(1, 0);

    assert_eq!(color(), Some(rgb(blue())));

    testing::trigger_key("Alt+m");

    assert_eq!(cancels.get_untracked(), 1);
    assert_eq!(color(), Some(rgb(red())), "the press is over");
  });
}

/// The mouse key is the launch's to choose; `None` leaves the mouse where the launch
/// put it, and a launch without the mouse has nothing to hand over.
#[test]
fn the_mouse_key_is_the_launches_choice() {
  with_launch(
    terminal::Launch::new().set_mouse_key(Some(terminal::Shortcut::new(
      terminal::Modifiers::NONE,
      "F9",
    ))),
    || {
      let clicks = mount_button();

      testing::trigger_key("Alt+m");
      testing::trigger_mouse_click(2, 0);

      assert_eq!(
        clicks.get_untracked(),
        1,
        "the default key is an ordinary key"
      );

      testing::trigger_key("F9");
      testing::trigger_mouse_click(2, 0);

      assert_eq!(
        clicks.get_untracked(),
        1,
        "the chosen key releases the mouse"
      );
    },
  );

  with_launch(terminal::Launch::new().set_mouse_key(None), || {
    let clicks = mount_button();

    testing::trigger_key("Alt+m");
    testing::trigger_mouse_click(2, 0);

    assert_eq!(clicks.get_untracked(), 1, "no key releases the mouse");
  });

  with_launch(terminal::Launch::new().set_mouse(false), || {
    let clicks = mount_button();

    testing::trigger_key("Alt+m");
    testing::trigger_key("Alt+m");
    testing::trigger_mouse_click(2, 0);

    assert_eq!(clicks.get_untracked(), 0, "the mouse stays the terminal's");
  });
}

/// A `Ctrl` shortcut acts by the letter on its physical key under a layout whose key
/// types another script, and `Ctrl-Q` quits whatever the case.
#[test]
fn shortcuts_act_by_the_physical_key_under_any_layout() {
  with_launch(terminal::Launch::new(), || {
    let value = Signal::new(String::from("value"));

    mount_document(value);

    let cyrillic = |key: &str, code: terminal::Code| terminal::KeyEvent {
      code,
      modifiers: terminal::Modifiers::CTRL,
      ..terminal::KeyEvent::new(key)
    };

    testing::trigger_key_event(&cyrillic("\u{441}", terminal::Code::KeyC));

    assert_eq!(testing::take_copied().as_deref(), Some("value"));
    assert!(!testing::is_quit_requested());

    testing::trigger_key("Ctrl+Q");

    assert!(testing::is_quit_requested());
  });
}
