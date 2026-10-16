use std::rc::Rc;

use crate::{
  NodeId, Owner, Scene, Signal,
  behavior::{
    ActivateKeys, Behavior, Collection, CollectionKeys, ControlHandle, ControlOps, EntryAct,
    EntryKeys, ListboxPolicy, Orientation, Popup, PopupAct, PopupKeys, Response, Roster,
    SliderKeys, Toggle, ToggleKeys, ToggleKind, TypeAhead,
  },
  seam::{
    access::Checked,
    content::Range,
    control::Prevent,
    report::{KeyEvent, Modifiers},
    runtime::Cancel,
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

/// A handle over no realization: its node one of a canned scene's, its operations doing
/// nothing.
fn handle() -> ControlHandle {
  let node: NodeId = Scene::<()>::new().insert(());
  let ops = ControlOps {
    focus: |_| {},
    find_part: |_, _| None,
    frame: |_| None,
    schedule: |_, _| Cancel(None),
    bind: |_, _, _| {},
    control: |_, _, _, _| {},
  };

  ControlHandle::new(node, ops, || None, || false)
}

#[test]
fn a_command_activates_on_enter_and_on_space_release() {
  let idle = ActivateKeys::new(false);
  let held = ActivateKeys::new(true);

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
}

#[test]
fn a_toggle_flips_on_space_release_and_leaves_enter() {
  let off = ToggleKeys::new(ToggleKind::Checkbox, false, false, true);
  let mixed = ToggleKeys::new(ToggleKind::Checkbox, false, true, true);
  let on = ToggleKeys::new(ToggleKind::Switch, true, false, true);

  assert_eq!(off.key(&up(" ")).act, Some(true));
  assert_eq!(mixed.key(&up(" ")).act, Some(true));
  assert_eq!(on.key(&up(" ")).act, Some(false));
  assert_eq!(off.key(&key("Enter")), Response::IGNORED);
  assert_eq!(on.flipped(), Some(false));
  assert_eq!(on.checked(), Checked::On);
  assert_eq!(mixed.checked(), Checked::Mixed);
}

#[test]
fn a_toggle_asks_its_handler_and_shows_the_truth() {
  let owner = Owner::new();

  owner.run(|| {
    let on = Signal::new(false);
    let asked = Signal::new(0);
    let control = handle();
    let refusing = Toggle::checkbox()
      .derive_checked(move || on.get())
      .handle_change(move |_| asked.update(|asked| *asked += 1));

    assert_eq!(refusing.on_click(&control), Prevent::All);
    assert_eq!((on.get(), asked.get()), (false, 1));

    let bound = Toggle::switch().bind_checked(on);

    let _ = bound.on_click(&control);

    assert!(on.get());
    assert_eq!(bound.access().checked, Some(Checked::On));
  });

  owner.dispose();
}

#[test]
fn a_radio_only_turns_on() {
  let off = ToggleKeys::new(ToggleKind::Radio, false, false, false);
  let on = ToggleKeys::new(ToggleKind::Radio, true, false, false);

  assert_eq!(off.flipped(), Some(true));
  assert_eq!(on.flipped(), None);
}

#[test]
fn a_slider_steps_pages_and_jumps() {
  let slider = SliderKeys::new(Range {
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
  assert_eq!(slider.stepped(true), Some(0.51));

  let top = SliderKeys::new(Range {
    value: 1.,
    ..slider.range
  });

  assert_eq!(top.key(&key("ArrowUp")), Response::CONSUMED);
}

#[test]
fn a_collection_skips_disabled_members_and_wraps_where_it_wraps() {
  let enabled = |index: usize| index != 2;
  let group = CollectionKeys::new(4, Some(1), Orientation::Both)
    .with_wrap(true)
    .with_enabled(&enabled);

  assert_eq!(group.key(&key("ArrowDown")).act, Some(3));
  assert_eq!(group.key(&key("ArrowRight")).act, Some(3));
  assert_eq!(group.key(&key("ArrowUp")).act, Some(0));

  let last = CollectionKeys {
    current: Some(3),
    ..group
  };

  assert_eq!(last.key(&key("ArrowDown")).act, Some(0));

  let list = CollectionKeys::new(4, Some(3), Orientation::Vertical).with_page(2);

  assert_eq!(list.key(&key("ArrowDown")), Response::CONSUMED);
  assert_eq!(list.key(&key("ArrowLeft")), Response::IGNORED);
  assert_eq!(list.key(&key("Home")).act, Some(0));
  assert_eq!(list.key(&key("PageUp")).act, Some(1));
}

#[test]
fn a_closed_popup_opens_or_chooses() {
  let closed = PopupKeys::new(3, Some(0));

  assert_eq!(closed.key(&key(" ")).act, Some(PopupAct::Open));
  assert_eq!(
    closed.key(&with(key("ArrowDown"), ALT)).act,
    Some(PopupAct::Open)
  );
  assert_eq!(closed.key(&key("F4")).act, Some(PopupAct::Open));
  assert_eq!(closed.key(&key("ArrowDown")).act, Some(PopupAct::Choose(1)));
  assert_eq!(closed.key(&key("End")).act, Some(PopupAct::Choose(2)));
  assert_eq!(closed.key(&key("Enter")), Response::IGNORED);
}

#[test]
fn a_popup_closes_on_escape_and_tab_and_its_face_finds_by_typing() {
  let owner = Owner::new();

  owner.run(|| {
    let chosen = Signal::new(Some(0_usize));
    let list = Collection::<ListboxPolicy>::new().bind_chosen(chosen);
    let names = || -> Vec<Rc<str>> {
      ["Apple", "Banana", "Cherry"]
        .into_iter()
        .map(Rc::from)
        .collect()
    };
    let popup = Popup::new(list.roster(), names);
    let control = handle();

    assert_eq!(popup.on_click(&control), Prevent::All);
    assert!(popup.open().get());
    assert_eq!(popup.on_key(&key("Escape"), &control), Prevent::All);
    assert!(!popup.open().get());
    assert_eq!(chosen.get(), Some(0));

    let _ = popup.on_click(&control);

    assert_eq!(popup.on_key(&key("Tab"), &control), Prevent::None);
    assert!(!popup.open().get());
    assert_eq!(popup.on_key(&key("c"), &control), Prevent::All);
    assert_eq!(chosen.get(), Some(2));
    assert_eq!(popup.access().value_text.as_deref(), Some("Cherry"));
  });

  owner.dispose();
}

#[test]
fn type_ahead_cycles_a_repeated_letter_and_matches_a_prefix() {
  let names = ["Apple", "Banana", "Blueberry", "Cherry"];
  let mut search = TypeAhead::new();

  assert_eq!(search.type_text("b", &names, Some(0)), Some(1));
  assert_eq!(search.type_text("b", &names, Some(1)), Some(2));
  assert_eq!(search.type_text("b", &names, Some(2)), Some(1));

  search.reset();

  assert_eq!(search.type_text("c", &names, Some(1)), Some(3));
  assert_eq!(search.type_text("h", &names, Some(3)), Some(3));
  assert_eq!(search.buffer(), "ch");
  assert!(search.is_active());
}

#[test]
fn a_text_entry_binds_the_editing_keys_and_submits_on_enter() {
  let field = EntryKeys::new(false);
  let editor = EntryKeys::new(true);
  let secure = EntryKeys::new(false).with_secure(true);
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

#[test]
fn a_roster_is_one_tab_stop_on_its_chosen_member() {
  let owner = Owner::new();

  owner.run(|| {
    let roster = Roster::new(crate::behavior::Controlled::bind(Signal::new(Some(1))));

    for index in 0..3 {
      roster.register(index, handle());
    }

    assert_eq!(roster.count(), 3);
    assert_eq!(roster.stop(), Some(1));
  });

  owner.dispose();
}
