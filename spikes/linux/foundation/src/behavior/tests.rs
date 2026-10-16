use std::time::{Duration, Instant};

use crate::{
  behavior::{
    Activate, Behavior, Collection, EntryAct, Orientation, Popup, PopupAct, Response, Slider,
    TextEntry, Toggle, ToggleKind, TypeAhead,
  },
  seam::{
    access::{Action, Checked},
    content::Range,
    report::{KeyEvent, Modifiers},
  },
  text::editing::{Binding, Command},
};

fn key(name: &str) -> KeyEvent {
  KeyEvent {
    key: name.into(),
    code: String::new(),
    down: true,
    repeat: false,
    modifiers: Modifiers::default(),
  }
}

fn up(name: &str) -> KeyEvent {
  KeyEvent {
    down: false,
    ..key(name)
  }
}

fn with(mut event: KeyEvent, modifiers: Modifiers) -> KeyEvent {
  event.modifiers = modifiers;

  event
}

const ALT: Modifiers = Modifiers {
  shift: false,
  control: false,
  alt: true,
  meta: false,
};

#[test]
fn a_command_activates_on_enter_and_on_space_release() {
  let idle = Activate::new(false);
  let held = Activate::new(true);

  assert_eq!(idle.key(&key("Enter")), Response::act(()));
  assert_eq!(
    idle.key(&key(" ")),
    Response::<()>::CONSUMED.with_pressed(true)
  );
  assert_eq!(idle.key(&up(" ")), Response::IGNORED);
  assert_eq!(held.key(&up(" ")), Response::act(()).with_pressed(false));
  assert_eq!(
    held.key(&key("Escape")),
    Response::<()>::CONSUMED.with_pressed(false)
  );
  assert_eq!(idle.key(&key("a")), Response::IGNORED);
  assert_eq!(idle.click(), Some(()));
  assert_eq!(idle.action(Action::Activate), Some(()));
}

#[test]
fn a_toggle_flips_on_space_release_and_leaves_enter() {
  let off = Toggle::new(ToggleKind::Checkbox, false, false, true);
  let mixed = Toggle::new(ToggleKind::Checkbox, false, true, true);
  let on = Toggle::new(ToggleKind::Switch, true, false, true);

  assert_eq!(off.key(&up(" ")).act, Some(true));
  assert_eq!(mixed.key(&up(" ")).act, Some(true));
  assert_eq!(on.key(&up(" ")).act, Some(false));
  assert_eq!(off.key(&key("Enter")), Response::IGNORED);
  assert_eq!(on.click(), Some(false));
  assert_eq!(on.facts().checked, Some(Checked::On));
  assert_eq!(mixed.facts().checked, Some(Checked::Mixed));
}

#[test]
fn a_radio_only_turns_on() {
  let off = Toggle::new(ToggleKind::Radio, false, false, false);
  let on = Toggle::new(ToggleKind::Radio, true, false, false);

  assert_eq!(off.click(), Some(true));
  assert_eq!(on.click(), None);
  assert_eq!(on.action(Action::Activate), None);
}

#[test]
fn a_slider_steps_pages_and_jumps() {
  let slider = Slider::new(Range {
    value: 0.5,
    minimum: 0.,
    maximum: 1.,
    step: 0.01,
  });

  assert_eq!(slider.key(&key("ArrowRight")).act, Some(0.51));
  assert_eq!(slider.key(&key("ArrowDown")).act, Some(0.49));
  assert!((slider.key(&key("PageUp")).act.unwrap() - 0.6).abs() < 1e-9);
  assert_eq!(slider.key(&key("Home")).act, Some(0.));
  assert_eq!(slider.key(&key("End")).act, Some(1.));
  assert_eq!(slider.key(&key("Tab")), Response::IGNORED);
  assert_eq!(slider.at(0.25), Some(0.25));
  assert_eq!(slider.at(0.5), None);
  assert_eq!(slider.action(Action::Increment), Some(0.51));

  let top = Slider::new(Range {
    value: 1.,
    ..slider.range
  });

  assert_eq!(top.key(&key("ArrowUp")), Response::CONSUMED);
}

#[test]
fn a_collection_skips_disabled_members_and_wraps_where_it_wraps() {
  let enabled = |index: usize| index != 2;
  let group = Collection::new(4, Some(1), Orientation::Both)
    .with_wrap(true)
    .with_enabled(&enabled);

  assert_eq!(group.key(&key("ArrowDown")).act, Some(3));
  assert_eq!(group.key(&key("ArrowRight")).act, Some(3));
  assert_eq!(group.key(&key("ArrowUp")).act, Some(0));

  let last = Collection {
    current: Some(3),
    ..group
  };

  assert_eq!(last.key(&key("ArrowDown")).act, Some(0));

  let list = Collection::new(4, Some(3), Orientation::Vertical).with_page(2);

  assert_eq!(list.key(&key("ArrowDown")), Response::CONSUMED);
  assert_eq!(list.key(&key("ArrowLeft")), Response::IGNORED);
  assert_eq!(list.key(&key("Home")).act, Some(0));
  assert_eq!(list.key(&key("PageUp")).act, Some(1));
}

#[test]
fn a_closed_popup_opens_or_chooses() {
  let closed = Popup::new(3, Some(0));

  assert_eq!(closed.key(&key(" ")).act, Some(PopupAct::Open));
  assert_eq!(
    closed.key(&with(key("ArrowDown"), ALT)).act,
    Some(PopupAct::Open)
  );
  assert_eq!(closed.key(&key("F4")).act, Some(PopupAct::Open));
  assert_eq!(closed.key(&key("ArrowDown")).act, Some(PopupAct::Choose(1)));
  assert_eq!(closed.key(&key("End")).act, Some(PopupAct::Choose(2)));
  assert_eq!(closed.key(&key("Enter")), Response::IGNORED);
  assert_eq!(closed.click(), Some(PopupAct::Open));
  assert_eq!(closed.facts().expanded, Some(false));
}

#[test]
fn an_open_popup_highlights_chooses_and_closes() {
  let open = Popup::new(3, Some(0)).with_open(true, Some(0));

  assert_eq!(
    open.key(&key("ArrowDown")).act,
    Some(PopupAct::Highlight(1))
  );
  assert_eq!(
    open.key(&key("Enter")).act,
    Some(PopupAct::ChooseAndClose(0))
  );
  assert_eq!(open.key(&key("Escape")).act, Some(PopupAct::Close));

  let tab = open.key(&key("Tab"));

  assert!(!tab.consumed);
  assert_eq!(tab.act, Some(PopupAct::ChooseAndClose(0)));
  assert_eq!(open.click(), Some(PopupAct::Close));
  assert_eq!(open.action(Action::Collapse), Some(PopupAct::Close));
}

#[test]
fn type_ahead_cycles_a_repeated_letter_and_matches_a_prefix() {
  let names = ["Apple", "Banana", "Blueberry", "Cherry"];
  let now = Instant::now();
  let mut search = TypeAhead::new();

  assert_eq!(search.type_text("b", now, &names, Some(0)), Some(1));
  assert_eq!(search.type_text("b", now, &names, Some(1)), Some(2));
  assert_eq!(search.type_text("b", now, &names, Some(2)), Some(1));

  let later = now + Duration::from_secs(2);

  assert_eq!(search.type_text("c", later, &names, Some(1)), Some(3));
  assert_eq!(search.type_text("h", later, &names, Some(3)), Some(3));
  assert_eq!(search.buffer(), "ch");
  assert!(search.is_active(later));
}

#[test]
fn a_text_entry_binds_the_editing_keys_and_submits_on_enter() {
  let field = TextEntry::new(false);
  let editor = TextEntry::new(true);
  let secure = TextEntry::new(false).with_secure(true);
  let control = Modifiers {
    control: true,
    ..Modifiers::default()
  };

  assert_eq!(field.key(&key("Enter")).act, Some(EntryAct::Activate));
  assert_eq!(
    editor.key(&key("Enter")).act,
    Some(EntryAct::Edit(Binding::Command(Command::InsertNewline)))
  );
  assert_eq!(
    field.key(&key("a")).act,
    Some(EntryAct::Edit(Binding::Command(Command::Insert(
      "a".into()
    ))))
  );
  assert_eq!(
    field.key(&with(key("c"), control)).act,
    Some(EntryAct::Edit(Binding::Command(Command::Copy)))
  );
  assert_eq!(secure.key(&with(key("c"), control)), Response::CONSUMED);
  assert_eq!(secure.key(&with(key("x"), control)), Response::CONSUMED);
  assert_eq!(field.key(&key("Tab")), Response::IGNORED);
}
