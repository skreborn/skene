//! A keymap: its shortcuts act while focus is inside the view it serves.

use std::{cell::Cell, rc::Rc};

use skene_core::{HasChild, KeyEvent, Modifiers, Prevented, Shortcut, View};
use skene_headless::{Headless, children, reset, trigger_key_down};
use skene_reactive::Owner;

type Container = skene_common::Container<Headless>;
type Keymap<C = skene_common::NoChild> = skene_common::Keymap<Headless, C>;

/// A shortcut pressed with focus inside the view served runs its action in place of
/// the platform's response and the ancestors', and any other key passes by.
#[test]
fn a_keymaps_shortcut_runs_its_action() {
  reset();

  let owner = Owner::new();
  let saves = Rc::new(Cell::new(0));
  let count = Rc::clone(&saves);
  let served = owner.enter(|| {
    Keymap::new()
      .handle_shortcut(Shortcut::new(Modifiers::CTRL, "s"), move || {
        count.set(count.get() + 1);
      })
      .set_child(Container::new().set_child(Container::new()))
      .build()
  });
  let inside = children(served)[0];
  let save = KeyEvent {
    modifiers: Modifiers::CTRL,
    ..KeyEvent::new("s")
  };

  assert_eq!(
    trigger_key_down(inside, &save),
    Prevented {
      platform: true,
      bubbling: true,
    }
  );
  assert_eq!(saves.get(), 1);
  assert_eq!(
    trigger_key_down(inside, &KeyEvent::new("s")),
    Prevented::default()
  );
  assert_eq!(saves.get(), 1);

  owner.dispose();
}
