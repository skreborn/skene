//! Controls: the behavior's verdict is the whole verdict, the bound signal is the
//! truth, parts resolve against the control, and a control listens only to what it
//! needs.

mod common;

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_core::{
  AccessProp,
  Condition,
  ControlKind,
  Dimension,
  EventKind,
  FocusOrder,
  HasChildren,
  KeyEvent,
  Modifiers,
  Part,
  PointerButton,
  PointerEvent,
  Prevent,
  Prevented,
  Role,
  Size,
  State,
  Toggle as Checked,
  View,
  When,
};
use skene_headless::{
  Headless,
  accessible_name,
  box_props,
  children,
  control_props,
  control_state,
  focus_order,
  is_focused,
  is_tab_stop,
  listened,
  part,
  reset,
  resolve_color,
  text,
  trigger_change,
  trigger_click,
  trigger_focus,
  trigger_key_down,
  trigger_pointer_down,
  trigger_pointer_move,
  trigger_pointer_up,
  trigger_size,
  value,
};
use skene_reactive::{Get, Owner, Set, Signal};
use skene_views::{
  Activate,
  Behavior,
  ControlHandle,
  Label,
  RadioMember,
  Range,
  Roving,
  TextEntry,
  Toggle,
};

use crate::common::{COLOR, blue, green, only_child, red};

type Container = skene_views::Container<Headless>;
type Column = skene_views::Column<Headless>;
type Text = skene_views::Text<Headless>;
type Control<B, C = skene_views::NoChild> = skene_views::Control<Headless, B, C>;
type RawButton = skene_views::RawButton<Headless>;
type RawCheckbox = skene_views::RawCheckbox<Headless>;
type RawSlider = skene_views::RawSlider<Headless>;
type RawRadioGroup = skene_views::RawRadioGroup<Headless>;
type RawRadio = skene_views::RawRadio<Headless>;
type RawTextField = skene_views::RawTextField<Headless>;
type RawTextEditor = skene_views::RawTextEditor<Headless>;

const CONSUMED: Prevented = Prevented {
  platform: true,
  bubbling: true,
};
const KEPT: Prevented = Prevented {
  platform: false,
  bubbling: true,
};

fn key(key: &str) -> KeyEvent {
  KeyEvent {
    key: key.to_owned(),
    modifiers: Modifiers::default(),
  }
}

fn press_at(local_x: f32) -> PointerEvent {
  PointerEvent {
    x: local_x,
    y: 5.,
    local_x,
    local_y: 5.,
    button: Some(PointerButton::Primary),
    modifiers: Modifiers::default(),
  }
}

fn note(log: &Rc<RefCell<Vec<&'static str>>>, name: &'static str) -> impl Fn() + 'static {
  let log = Rc::clone(log);

  move || log.borrow_mut().push(name)
}

fn assert_value(signal: Signal<f32>, expected: f32) {
  let held = signal.get();

  assert!(
    (held - expected).abs() < 1e-4,
    "expected {expected}, held {held}"
  );
}

fn focus_visible() -> Condition<f32> {
  Condition::from(State::FocusVisible)
}

fn checked() -> Condition<f32> {
  Condition::from(State::Checked)
}

/// A button's click is the button's alone and never reaches the container it sits
/// in, while a plain box's click on the same path still climbs; `Enter` on the button
/// is a click.
#[test]
fn a_button_click_stays_on_the_button_while_a_box_click_climbs() {
  reset();

  let owner = Owner::new();
  let log = Rc::new(RefCell::new(Vec::new()));
  let outer = owner.enter(|| {
    Container::new()
      .handle_click(note(&log, "outer"))
      .handle_key_down(note(&log, "outer key").into_key())
      .set_child(
        Column::new().set_children((
          RawButton::new(Activate::new())
            .handle_click(note(&log, "button"))
            .set_label("Save"),
          Container::new().handle_click(note(&log, "box")),
        )),
      )
      .build()
  });
  let column = only_child(outer);
  let button = children(column)[1];
  let plain = children(column)[2];

  assert_eq!(
    control_props(button).map(|props| props.kind),
    Some(ControlKind::Button)
  );
  assert_eq!(
    box_props(button).and_then(|props| props.role),
    Some(Role::Button)
  );
  assert_eq!(trigger_click(button), KEPT);
  assert_eq!(*log.borrow(), ["button"]);
  assert_eq!(trigger_click(plain), Prevented::default());
  assert_eq!(*log.borrow(), ["button", "box", "outer"]);
  assert_eq!(trigger_key_down(button, &key("Enter")), CONSUMED);
  assert_eq!(*log.borrow(), ["button", "box", "outer", "button"]);
  assert_eq!(trigger_key_down(button, &key("a")), Prevented::default());
  assert_eq!(
    *log.borrow(),
    ["button", "box", "outer", "button", "outer key"]
  );

  owner.dispose();
}

trait IntoKey {
  fn into_key(self) -> impl Fn(&KeyEvent) + 'static;
}

impl<F: Fn() + 'static> IntoKey for F {
  fn into_key(self) -> impl Fn(&KeyEvent) + 'static {
    move |_| self()
  }
}

/// `Space` and a click each flip the signal a checkbox is bound to; the control's
/// state and what it reports follow the signal, and `Enter` means nothing to it.
#[test]
fn space_and_a_click_flip_a_bound_checkbox() {
  reset();

  let owner = Owner::new();
  let bound = Signal::new(false);
  let node = owner.enter(|| RawCheckbox::new(Toggle::new().bind_checked(bound)).build());

  assert_eq!(
    control_state(node).map(|state| state.checked),
    Some(Checked::Off)
  );
  assert_eq!(trigger_click(node), KEPT);
  assert!(bound.get());
  assert_eq!(
    control_state(node).map(|state| state.checked),
    Some(Checked::On)
  );
  assert_eq!(
    box_props(node).map(|props| props.access),
    Some(vec![AccessProp::Checked(Checked::On)])
  );
  assert_eq!(trigger_key_down(node, &key(" ")), CONSUMED);
  assert!(!bound.get());
  assert_eq!(trigger_key_down(node, &key("Enter")), Prevented::default());
  assert!(!bound.get());

  bound.set(true);

  assert_eq!(
    control_state(node).map(|state| state.checked),
    Some(Checked::On)
  );

  owner.dispose();
}

/// A checkbox is controlled: with a derived state and no handler, a click changes
/// nothing.
#[test]
fn a_controlled_checkbox_keeps_its_state_without_a_handler() {
  reset();

  let owner = Owner::new();
  let bound = Signal::new(false);
  let node =
    owner.enter(|| RawCheckbox::new(Toggle::new().derive_checked(move || bound.get())).build());

  assert_eq!(trigger_click(node), KEPT);
  assert_eq!(trigger_key_down(node, &key(" ")), CONSUMED);
  assert!(!bound.get());
  assert_eq!(
    control_state(node).map(|state| state.checked),
    Some(Checked::Off)
  );

  owner.dispose();
}

/// The arrows, page keys, `Home` and `End` move a slider's bound value by the range's
/// rules, and a press a quarter of the way along its width sets a quarter of the range.
#[test]
fn slider_keys_and_a_press_move_the_bound_value_by_the_ranges_rules() {
  reset();

  let owner = Owner::new();
  let bound = Signal::new(50.);
  let node = owner.enter(|| RawSlider::new(Range::new().set_step(5.).bind_value(bound)).build());

  assert_eq!(
    box_props(node).map(|props| props.access),
    Some(vec![
      AccessProp::Value(50.),
      AccessProp::ValueMin(0.),
      AccessProp::ValueMax(100.),
    ])
  );
  assert_eq!(trigger_key_down(node, &key("ArrowRight")), CONSUMED);
  assert_value(bound, 55.);
  assert_eq!(trigger_key_down(node, &key("ArrowLeft")), CONSUMED);
  assert_value(bound, 50.);
  assert_eq!(trigger_key_down(node, &key("PageUp")), CONSUMED);
  assert_value(bound, 100.);
  assert_eq!(trigger_key_down(node, &key("ArrowUp")), CONSUMED);
  assert_value(bound, 100.);
  assert_eq!(trigger_key_down(node, &key("PageDown")), CONSUMED);
  assert_value(bound, 50.);
  assert_eq!(trigger_key_down(node, &key("Home")), CONSUMED);
  assert_value(bound, 0.);
  assert_eq!(trigger_key_down(node, &key("ArrowDown")), CONSUMED);
  assert_value(bound, 0.);
  assert_eq!(trigger_key_down(node, &key("End")), CONSUMED);
  assert_value(bound, 100.);
  assert_eq!(trigger_key_down(node, &key("Tab")), Prevented::default());
  assert_eq!(
    box_props(node).map(|props| props.access),
    Some(vec![
      AccessProp::Value(100.),
      AccessProp::ValueMin(0.),
      AccessProp::ValueMax(100.),
    ])
  );

  assert_eq!(trigger_pointer_down(node, &press_at(50.)), CONSUMED);
  assert_value(bound, 100.);

  trigger_size(node, Size::new(200., 20.));

  assert_eq!(trigger_pointer_down(node, &press_at(50.)), CONSUMED);
  assert_value(bound, 25.);
  assert_eq!(trigger_pointer_move(node, &press_at(100.)), KEPT);
  assert_value(bound, 50.);
  assert_eq!(trigger_pointer_up(node, &press_at(100.)), KEPT);
  assert_eq!(
    trigger_pointer_move(node, &press_at(150.)),
    Prevented::default()
  );
  assert_value(bound, 50.);
  assert_eq!(
    trigger_pointer_up(node, &press_at(150.)),
    Prevented::default()
  );

  owner.dispose();
}

/// A radio group with nothing chosen has its first member as the one tab stop,
/// choosing moves the stop and the checked state, the arrows step with wrap and focus,
/// and a member joining ahead of the others re-evaluates the stop.
#[test]
fn a_radio_groups_tab_stop_follows_the_choice_and_the_membership() {
  reset();

  let owner = Owner::new();
  let chosen: Signal<Option<String>> = Signal::new(None);
  let items = Signal::new(vec!["a", "b"]);
  let group = owner.enter(|| {
    RawRadioGroup::new(Roving::new().bind_chosen(chosen))
      .set_child(Column::new().derive_children(
        move || items.get(),
        |item| *item,
        |item| RawRadio::new(RadioMember::new(item)).set_label(item),
      ))
      .build()
  });
  let column = children(group)[0];
  let member = |index: usize| children(column)[index + 1];

  assert_eq!(focus_order(group), None);
  assert_eq!(listened(group), []);
  assert!(is_tab_stop(member(0)));
  assert!(!is_tab_stop(member(1)));
  assert_eq!(focus_order(member(1)), Some(FocusOrder::Skip));
  assert_eq!(trigger_click(member(1)), KEPT);
  assert_eq!(chosen.get().as_deref(), Some("b"));
  assert!(!is_tab_stop(member(0)));
  assert!(is_tab_stop(member(1)));
  assert_eq!(
    control_state(member(1)).map(|state| state.checked),
    Some(Checked::On)
  );
  assert_eq!(
    control_state(member(0)).map(|state| state.checked),
    Some(Checked::Off)
  );
  assert_eq!(trigger_key_down(member(1), &key("ArrowRight")), CONSUMED);
  assert_eq!(chosen.get().as_deref(), Some("a"));
  assert!(is_focused(member(0)));
  assert_eq!(trigger_key_down(member(0), &key("ArrowUp")), CONSUMED);
  assert_eq!(chosen.get().as_deref(), Some("b"));
  assert!(is_focused(member(1)));

  chosen.set(None);

  assert!(is_tab_stop(member(0)));

  items.set(vec!["z", "a", "b"]);

  assert_eq!(accessible_name(member(0)), Some("z".to_owned()));
  assert!(is_tab_stop(member(0)));
  assert!(!is_tab_stop(member(1)));
  assert!(!is_tab_stop(member(2)));
  assert_eq!(trigger_key_down(member(0), &key("ArrowLeft")), CONSUMED);
  assert_eq!(chosen.get().as_deref(), Some("b"));
  assert!(is_focused(member(2)));

  owner.dispose();
}

/// A part's focus variant resolves when the control is focused, and its checked
/// variant follows the control's state.
#[test]
fn a_parts_focus_and_checked_variants_follow_the_control() {
  reset();

  let owner = Owner::new();
  let bound = Signal::new(false);
  let node = owner.enter(|| {
    RawCheckbox::new(Toggle::new().bind_checked(bound))
      .set_part(
        Part::Face,
        Container::new().set_color(red().when(focus_visible(), blue())),
      )
      .set_part(
        Part::Mark,
        Container::new().set_color(red().when(checked(), green())),
      )
      .build()
  });
  let face = children(node)[0];
  let mark = children(node)[1];

  assert_eq!(part(node), Some(Part::Root));
  assert_eq!(part(face), Some(Part::Face));
  assert_eq!(part(mark), Some(Part::Mark));
  assert_eq!(resolve_color(face, COLOR), Some(red()));
  assert_eq!(resolve_color(mark, COLOR), Some(red()));

  trigger_focus(node);

  assert!(is_focused(node));
  assert!(!is_focused(face));
  assert_eq!(resolve_color(face, COLOR), Some(blue()));

  assert_eq!(trigger_click(node), KEPT);
  assert_eq!(resolve_color(mark, COLOR), Some(green()));

  owner.dispose();
}

/// A disabled control reports it, takes no focus and ignores activation; enabling it
/// again restores all three.
#[test]
fn a_disabled_control_reports_it_and_ignores_activation() {
  reset();

  let owner = Owner::new();
  let disabled = Signal::new(true);
  let clicks = Rc::new(Cell::new(0));
  let node = owner.enter(|| {
    let clicks = Rc::clone(&clicks);

    RawButton::new(Activate::new())
      .derive_disabled(move || disabled.get())
      .handle_click(move || clicks.set(clicks.get() + 1))
      .build()
  });

  assert_eq!(
    box_props(node).map(|props| props.access),
    Some(vec![AccessProp::Disabled(true)])
  );
  assert_eq!(control_state(node).map(|state| state.disabled), Some(true));
  assert_eq!(focus_order(node), None);
  assert!(!is_tab_stop(node));
  assert_eq!(trigger_click(node), CONSUMED);
  assert_eq!(trigger_key_down(node, &key("Enter")), CONSUMED);
  assert_eq!(clicks.get(), 0);

  disabled.set(false);

  assert_eq!(box_props(node).map(|props| props.access), Some(vec![]));
  assert_eq!(control_state(node).map(|state| state.disabled), Some(false));
  assert!(is_tab_stop(node));
  assert_eq!(trigger_click(node), KEPT);
  assert_eq!(clicks.get(), 1);

  let fixed = owner.enter(|| RawButton::new(Activate::new()).set_disabled(true).build());

  assert_eq!(focus_order(fixed), None);

  owner.dispose();
}

/// A label part supplies the accessible name: a string names the control at build,
/// and any view's text is read from the part.
#[test]
fn a_label_part_supplies_the_accessible_name() {
  reset();

  let owner = Owner::new();
  let named = owner.enter(|| RawButton::new(Activate::new()).set_label("Save").build());
  let drawn = owner.enter(|| {
    RawCheckbox::new(Toggle::new())
      .set_label(Label::new(Text::new("Remember me")))
      .build()
  });
  let unnamed = owner.enter(|| RawButton::new(Activate::new()).build());
  let label = children(named)[0];

  assert_eq!(part(label), Some(Part::Label));
  assert_eq!(text(label), Some("Save".to_owned()));
  assert_eq!(
    control_props(named).and_then(|props| props.label),
    Some("Save".to_owned())
  );
  assert_eq!(accessible_name(named), Some("Save".to_owned()));
  assert_eq!(control_props(drawn).and_then(|props| props.label), None);
  assert_eq!(accessible_name(drawn), Some("Remember me".to_owned()));
  assert_eq!(accessible_name(unnamed), None);

  owner.dispose();
}

/// A text field's change writes the bound signal and stays on its node, the signal
/// writes the field's value, and an editor is the same control with lines.
#[test]
fn a_text_fields_change_writes_the_signal_and_stays_on_its_node() {
  reset();

  let owner = Owner::new();
  let bound = Signal::new(String::new());
  let log = Rc::new(RefCell::new(Vec::new()));
  let outer = owner.enter(|| {
    Container::new()
      .handle_click(note(&log, "outer"))
      .set_child(RawTextField::new(
        TextEntry::new().set_placeholder("Name").bind_value(bound),
      ))
      .build()
  });
  let field = only_child(outer);

  assert_eq!(value(field), Some(String::new()));
  assert_eq!(
    control_props(field).map(|props| props.kind),
    Some(ControlKind::TextField)
  );
  assert_eq!(
    control_props(field).and_then(|props| props.placeholder),
    Some("Name".to_owned())
  );
  assert_eq!(box_props(field).and_then(|props| props.role), None);

  trigger_change(field, "typed");

  assert_eq!(bound.get(), "typed");
  assert!(log.borrow().is_empty());

  bound.set("fresh".to_owned());

  assert_eq!(value(field), Some("fresh".to_owned()));

  let editor = owner.enter(|| RawTextEditor::new(TextEntry::new().set_lines(4)).build());

  assert_eq!(
    control_props(editor).map(|props| props.kind),
    Some(ControlKind::TextEditor)
  );
  assert_eq!(control_props(editor).map(|props| props.lines), Some(4));

  owner.dispose();
}

/// A control registers only the events its behavior acts on and the ones its own
/// handlers observe.
#[test]
fn only_the_events_a_behavior_or_a_handler_needs_are_listened_to() {
  reset();

  let owner = Owner::new();
  let (checkbox, slider, field, button) = owner.enter(|| {
    (
      RawCheckbox::new(Toggle::new()).build(),
      RawSlider::new(Range::new()).build(),
      RawTextField::new(TextEntry::new()).build(),
      RawButton::new(Activate::new())
        .handle_focus(|| {})
        .handle_pointer_move(|_| {})
        .build(),
    )
  });

  assert_eq!(
    listened(checkbox),
    [EventKind::Click, EventKind::KeyDown, EventKind::Toggle]
  );
  assert_eq!(
    listened(slider),
    [
      EventKind::KeyDown,
      EventKind::PointerDown,
      EventKind::PointerUp,
      EventKind::PointerMove,
    ]
  );
  assert_eq!(listened(field), [EventKind::Change]);
  assert_eq!(
    listened(button),
    [
      EventKind::Click,
      EventKind::KeyDown,
      EventKind::PointerMove,
      EventKind::Focus,
    ]
  );

  owner.dispose();
}

/// A stepper: a third party's behavior, built through the public trait alone.
struct Stepper {
  count: Signal<i32>,
}

impl Behavior<Headless> for Stepper {
  fn kind(&self) -> ControlKind {
    ControlKind::Button
  }

  fn role(&self) -> Option<Role> {
    Some(Role::Button)
  }

  fn listens(&self) -> &'static [EventKind] {
    &[EventKind::KeyDown]
  }

  fn on_key(&self, event: &KeyEvent, _: &ControlHandle<Headless>) -> Prevent {
    match event.key.as_str() {
      "ArrowUp" => self.count.set(self.count.get() + 1),
      "ArrowDown" => self.count.set(self.count.get() - 1),
      _ => return Prevent::None,
    }

    Prevent::All
  }

  #[expect(clippy::cast_precision_loss)]
  fn access(&self) -> Vec<AccessProp> {
    vec![AccessProp::Value(self.count.get() as f32)]
  }

  fn focus_order(&self) -> Option<FocusOrder> {
    Some(FocusOrder::Position(2))
  }
}

/// A third-party behavior gets focus, decoration, parts, observers and reactive
/// reports from the same builder as the shipped ones.
#[test]
fn a_third_party_behavior_builds_through_public_types_alone() {
  reset();

  let owner = Owner::new();
  let count = Signal::new(0);
  let seen = Rc::new(Cell::new(0));
  let node = owner.enter(|| {
    let seen = Rc::clone(&seen);

    Control::new(Stepper { count })
      .set_width(Dimension::Length(40.))
      .set_color(red().when(focus_visible(), blue()))
      .set_part(Part::Face, Text::new("0"))
      .handle_key_down(move |_| seen.set(seen.get() + 1))
      .set_child(Container::new())
      .build()
  });

  assert_eq!(focus_order(node), Some(FocusOrder::Position(2)));
  assert_eq!(part(children(node)[0]), Some(Part::Face));
  assert_eq!(trigger_key_down(node, &key("ArrowUp")), CONSUMED);
  assert_eq!(trigger_key_down(node, &key("ArrowUp")), CONSUMED);
  assert_eq!(trigger_key_down(node, &key("x")), Prevented::default());
  assert_eq!(count.get(), 2);
  assert_eq!(seen.get(), 3);
  assert_eq!(
    box_props(node).map(|props| props.access),
    Some(vec![AccessProp::Value(2.)])
  );

  trigger_focus(node);

  assert_eq!(resolve_color(node, COLOR), Some(blue()));

  owner.dispose();
}
