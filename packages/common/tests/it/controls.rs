//! Controls: the behavior's verdict is the whole verdict, the bound signal is the
//! truth, parts resolve against the control, and a control listens only to what it
//! needs.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_common::{
  Activate,
  Behavior,
  ControlHandle,
  Label,
  LabelPlacement,
  Listbox,
  ListboxOption,
  Popup,
  Radio,
  RadioGroup,
  Slider,
  TextEntry,
  Toggle,
};
use skene_core::{
  AccessProp,
  AnyView,
  Axes,
  CheckState,
  Condition,
  Dimension,
  DragAxes,
  EventKind,
  FocusOrder,
  HasChild,
  HasChildren,
  ItemAlignment,
  Key,
  KeyEvent,
  NamedKey,
  NativeElement,
  Part,
  PointerButton,
  PointerEvent,
  PopupKind,
  Prevent,
  Prevented,
  Property,
  PropertyKey,
  Role,
  Size,
  Spacing,
  State,
  TextSelection,
  View,
  WheelEvent,
  WheelUnit,
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
  resolve_length,
  selection,
  text,
  trigger_blur,
  trigger_change,
  trigger_click,
  trigger_focus,
  trigger_key_down,
  trigger_key_up,
  trigger_pointer_cancel,
  trigger_pointer_down,
  trigger_pointer_move,
  trigger_pointer_up,
  trigger_selection_change,
  trigger_size,
  trigger_toggle,
  trigger_wheel,
  value,
};
use skene_reactive::{Get, Owner, Set, Signal};

use crate::common::{COLOR, blue, green, only_child, red};

type Container = skene_common::Container<Headless>;
type Column = skene_common::Column<Headless>;
type Text = skene_common::Text<Headless>;
type Control<B, C = skene_common::NoChild> = skene_common::Control<Headless, B, C>;
type BasicButton = skene_common::BasicButton<Headless>;
type BasicCheckbox = skene_common::BasicCheckbox<Headless>;
type BasicSlider = skene_common::BasicSlider<Headless>;
type BasicRadioGroup = skene_common::BasicRadioGroup<Headless>;
type BasicRadio = skene_common::BasicRadio<Headless>;
type BasicListbox = skene_common::BasicListbox<Headless>;
type BasicListboxOption = skene_common::BasicListboxOption<Headless>;
type BasicTextField = skene_common::BasicTextField<Headless>;
type BasicTextEditor = skene_common::BasicTextEditor<Headless>;

const CONSUMED: Prevented = Prevented {
  platform: true,
  bubbling: true,
};
const KEPT: Prevented = Prevented {
  platform: false,
  bubbling: true,
};

fn key(key: &str) -> KeyEvent {
  KeyEvent::new(key)
}

fn press_at(local_x: f32) -> PointerEvent {
  PointerEvent::mouse(local_x, 5.).with_press(PointerButton::Primary)
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
          BasicButton::new(Activate::new())
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
    control_props(button).map(|props| props.native),
    Some(Some(NativeElement::Button))
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

/// `Space` activates a button once, as it is released after a press it saw: the press
/// and its repeats do nothing, and a press the button lost focus after never
/// activates it; `Enter` activates on every press, repeats included.
#[test]
fn space_activates_a_button_once_on_its_release() {
  reset();

  let owner = Owner::new();
  let clicks = Rc::new(Cell::new(0));
  let node = owner.enter(|| {
    let clicks = Rc::clone(&clicks);

    BasicButton::new(Activate::new())
      .handle_click(move || clicks.set(clicks.get() + 1))
      .build()
  });
  let repeat = |name| KeyEvent {
    repeat: true,
    ..key(name)
  };

  assert_eq!(trigger_key_down(node, &key(" ")), CONSUMED);
  assert_eq!(trigger_key_down(node, &repeat(" ")), CONSUMED);
  assert_eq!(clicks.get(), 0);
  assert_eq!(trigger_key_up(node, &key(" ")), CONSUMED);
  assert_eq!(clicks.get(), 1);
  assert_eq!(trigger_key_up(node, &key(" ")), Prevented::default());
  assert_eq!(clicks.get(), 1);

  trigger_focus(node);

  let _ = trigger_key_down(node, &key(" "));

  trigger_blur(node);

  assert_eq!(trigger_key_up(node, &key(" ")), Prevented::default());
  assert_eq!(clicks.get(), 1);
  assert_eq!(trigger_key_down(node, &key("Enter")), CONSUMED);
  assert_eq!(trigger_key_down(node, &repeat("Enter")), CONSUMED);
  assert_eq!(clicks.get(), 3);

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

/// `Space`, on its release, and a click each flip the signal a checkbox is bound to;
/// the control's state and what it reports follow the signal, and `Enter` means nothing
/// to it.
#[test]
fn space_and_a_click_flip_a_bound_checkbox() {
  reset();

  let owner = Owner::new();
  let bound = Signal::new(false);
  let node = owner.enter(|| BasicCheckbox::new(Toggle::new().bind_checked(bound)).build());

  assert_eq!(
    control_state(node).map(|state| state.checked),
    Some(CheckState::Off)
  );
  assert_eq!(trigger_click(node), KEPT);
  assert!(bound.get());
  assert_eq!(
    control_state(node).map(|state| state.checked),
    Some(CheckState::On)
  );
  assert_eq!(
    box_props(node).map(|props| props.access),
    Some(vec![AccessProp::Checked(CheckState::On)])
  );
  assert_eq!(trigger_key_down(node, &key(" ")), CONSUMED);
  assert!(bound.get());
  assert_eq!(trigger_key_up(node, &key(" ")), CONSUMED);
  assert!(!bound.get());
  assert_eq!(trigger_key_down(node, &key("Enter")), Prevented::default());
  assert!(!bound.get());

  bound.set(true);

  assert_eq!(
    control_state(node).map(|state| state.checked),
    Some(CheckState::On)
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
    owner.enter(|| BasicCheckbox::new(Toggle::new().derive_checked(move || bound.get())).build());

  assert_eq!(trigger_click(node), KEPT);
  assert_eq!(trigger_key_down(node, &key(" ")), CONSUMED);
  assert_eq!(trigger_key_up(node, &key(" ")), CONSUMED);
  assert!(!bound.get());
  assert_eq!(
    control_state(node).map(|state| state.checked),
    Some(CheckState::Off)
  );

  owner.dispose();
}

/// A press maps to the value whose thumb centers under it: the thumb's center travels
/// the extent less the thumb, so a press on the thumb at either end holds that end.
#[test]
fn a_press_centers_the_thumb_under_the_pointer() {
  reset();

  let owner = Owner::new();
  let bound = Signal::new(0.);
  let node =
    owner.enter(|| BasicSlider::new(Slider::new().set_thumb_extent(20.).bind_value(bound)).build());

  trigger_size(node, Size::new(200., 20.));

  let _ = trigger_pointer_down(node, &press_at(190.));

  assert_value(bound, 100.);

  let _ = trigger_pointer_up(node, &press_at(190.));
  let _ = trigger_pointer_down(node, &press_at(100.));

  assert_value(bound, 50.);

  let _ = trigger_pointer_up(node, &press_at(100.));
  let _ = trigger_pointer_down(node, &press_at(10.));

  assert_value(bound, 0.);

  owner.dispose();
}

/// A text entry is controlled: an edit its handler rejects leaves the platform showing
/// the bound value, one it accepts stands, and a disabled entry hears none.
#[test]
fn a_rejected_edit_leaves_the_platform_showing_the_bound_value() {
  reset();

  let owner = Owner::new();
  let bound = Signal::new("12".to_owned());
  let disabled = Signal::new(false);
  let node = owner.enter(|| {
    BasicTextField::new(
      TextEntry::new()
        .derive_value(move || bound.get())
        .handle_change(move |value| {
          if value.chars().all(|character| character.is_ascii_digit()) {
            bound.set(value.to_owned());
          }
        }),
    )
    .derive_disabled(move || disabled.get())
    .build()
  });

  trigger_change(node, "12a");

  assert_eq!(bound.get(), "12");
  assert_eq!(
    value(node).as_deref(),
    Some("12"),
    "the rejected edit is undone"
  );

  trigger_change(node, "123");

  assert_eq!(bound.get(), "123");
  assert_eq!(value(node).as_deref(), Some("123"));

  disabled.set(true);
  trigger_change(node, "1234");

  assert_eq!(bound.get(), "123", "a disabled entry hears no edit");
  assert_eq!(value(node).as_deref(), Some("123"));

  owner.dispose();
}

/// A checkbox is controlled: a platform toggle its handler rejects is undone on the
/// platform.
#[test]
fn a_rejected_toggle_is_undone_on_the_platform() {
  reset();

  let owner = Owner::new();
  let node = owner
    .enter(|| BasicCheckbox::new(Toggle::new().set_checked(false).handle_toggle(|_| {})).build());

  trigger_toggle(node, true);

  assert_eq!(skene_headless::checked(node), Some(false));

  owner.dispose();
}

/// A key an input method is composing leaves the behavior alone.
#[test]
fn a_composing_key_never_reaches_the_behavior() {
  reset();

  let owner = Owner::new();
  let bound = Signal::new(50.);
  let node = owner.enter(|| BasicSlider::new(Slider::new().bind_value(bound)).build());
  let composing = KeyEvent {
    composing: true,
    ..key("ArrowRight")
  };

  assert_eq!(trigger_key_down(node, &composing), Prevented::default());
  assert_value(bound, 50.);
  assert_eq!(trigger_key_down(node, &key("ArrowRight")), CONSUMED);
  assert_value(bound, 51.);

  owner.dispose();
}

/// The arrows, page keys, `Home` and `End` move a slider's bound value by the range's
/// rules, a press a quarter of the way along its width sets a quarter of the range, and
/// a press of another button than the primary slides nothing.
#[test]
fn slider_keys_and_a_press_move_the_bound_value_by_the_ranges_rules() {
  reset();

  let owner = Owner::new();
  let bound = Signal::new(50.);
  let node = owner.enter(|| BasicSlider::new(Slider::new().set_step(5.).bind_value(bound)).build());

  assert_eq!(
    box_props(node).and_then(|props| props.drag_axes),
    Some(DragAxes::Horizontal),
    "a press drags the slider along its axis"
  );

  assert_eq!(
    box_props(node).map(|props| props.access),
    Some(vec![
      AccessProp::Value(50.),
      AccessProp::ValueMin(0.),
      AccessProp::ValueMax(100.),
      AccessProp::ValueStep(5.),
      AccessProp::Orientation(skene_core::Axis::Horizontal),
    ])
  );
  assert_eq!(trigger_key_down(node, &key("ArrowRight")), CONSUMED);
  assert_value(bound, 55.);
  assert_eq!(trigger_key_down(node, &key("ArrowLeft")), CONSUMED);
  assert_value(bound, 50.);
  assert_eq!(trigger_key_down(node, &key("PageUp")), CONSUMED);
  assert_value(bound, 60.);
  assert_eq!(trigger_key_down(node, &key("ArrowUp")), CONSUMED);
  assert_value(bound, 65.);
  assert_eq!(trigger_key_down(node, &key("PageDown")), CONSUMED);
  assert_value(bound, 55.);
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
      AccessProp::ValueStep(5.),
      AccessProp::Orientation(skene_core::Axis::Horizontal),
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

  let secondary = PointerEvent::mouse(150., 5.).with_press(PointerButton::Secondary);

  assert_eq!(trigger_pointer_down(node, &secondary), Prevented::default());
  assert_value(bound, 50.);
  assert_eq!(
    trigger_pointer_move(node, &press_at(160.)),
    Prevented::default()
  );
  assert_value(bound, 50.);
  assert_eq!(trigger_pointer_down(node, &press_at(120.)), CONSUMED);
  assert_value(bound, 60.);
  assert_eq!(trigger_pointer_cancel(node, &press_at(120.)), KEPT);
  assert_eq!(
    trigger_pointer_move(node, &press_at(160.)),
    Prevented::default()
  );
  assert_value(bound, 60.);

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
  let items = Signal::transient(vec!["a", "b"]);
  let group = owner.enter(|| {
    BasicRadioGroup::new(RadioGroup::new().bind_chosen(chosen))
      .set_child(Column::new().derive_children(
        move || items.get(),
        |item| *item,
        |item| BasicRadio::new(Radio::new(item)).set_label(item),
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
    Some(CheckState::On)
  );
  assert_eq!(
    control_state(member(0)).map(|state| state.checked),
    Some(CheckState::Off)
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

/// A radio group's disabled members are passed over: the tab stop is the first enabled
/// member while nothing is chosen, the arrows skip the disabled ones, and a member
/// disabled later gives up the stop.
#[test]
fn a_radio_group_passes_over_its_disabled_members() {
  reset();

  let owner = Owner::new();
  let chosen: Signal<Option<String>> = Signal::new(None);
  let second_disabled = Signal::new(false);
  let group = owner.enter(|| {
    BasicRadioGroup::new(RadioGroup::new().bind_chosen(chosen))
      .set_child(
        Column::new().set_children((
          BasicRadio::new(Radio::new("a"))
            .set_label("a")
            .set_disabled(true),
          BasicRadio::new(Radio::new("b"))
            .set_label("b")
            .derive_disabled(move || second_disabled.get()),
          BasicRadio::new(Radio::new("c")).set_label("c"),
        )),
      )
      .build()
  });
  let column = children(group)[0];
  let member = |index: usize| children(column)[index + 1];

  assert!(!is_tab_stop(member(0)));
  assert!(is_tab_stop(member(1)));
  assert_eq!(trigger_key_down(member(1), &key("ArrowDown")), CONSUMED);
  assert_eq!(chosen.get().as_deref(), Some("c"));
  assert_eq!(trigger_key_down(member(2), &key("ArrowDown")), CONSUMED);
  assert_eq!(chosen.get().as_deref(), Some("b"));

  second_disabled.set(true);

  assert!(!is_tab_stop(member(1)));
  assert!(is_tab_stop(member(2)));

  owner.dispose();
}

/// A listbox's arrows skip its disabled options, and `Tab` from an option leaves the
/// list without choosing it.
#[test]
fn a_listbox_skips_disabled_options_and_tab_chooses_nothing() {
  reset();

  let owner = Owner::new();
  let chosen: Signal<Option<String>> = Signal::new(None);
  let list = owner.enter(|| {
    BasicListbox::new(Listbox::new().bind_chosen(chosen))
      .set_child(
        Column::new().set_children((
          BasicListboxOption::new(ListboxOption::new("a")).set_label("a"),
          BasicListboxOption::new(ListboxOption::new("b"))
            .set_label("b")
            .set_disabled(true),
          BasicListboxOption::new(ListboxOption::new("c")).set_label("c"),
        )),
      )
      .build()
  });
  let column = children(list)[0];
  let option = |index: usize| children(column)[index + 1];

  assert_eq!(trigger_key_down(option(0), &key("ArrowDown")), CONSUMED);
  assert!(is_focused(option(2)));
  assert_eq!(
    trigger_key_down(option(2), &key("Tab")),
    Prevented::default()
  );
  assert_eq!(chosen.get(), None);

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
    BasicCheckbox::new(Toggle::new().bind_checked(bound))
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

/// Focus reached after a pointer press does not show, a key makes it show, and the
/// next press hides it again, as `:focus-visible` does.
#[test]
fn focus_shows_after_a_key_and_not_after_a_pointer_press() {
  reset();

  let owner = Owner::new();
  let node = owner.enter(|| {
    BasicButton::new(Activate::new())
      .set_color(red().when(focus_visible(), blue()))
      .set_label("go")
      .build()
  });

  let _ = trigger_pointer_down(node, &press_at(0.));

  trigger_focus(node);

  assert!(is_focused(node));
  assert_eq!(
    resolve_color(node, COLOR),
    Some(red()),
    "focused after a press, not shown"
  );

  let _ = trigger_key_down(node, &key("x"));

  assert_eq!(resolve_color(node, COLOR), Some(blue()), "a key shows it");

  let _ = trigger_pointer_down(node, &press_at(0.));

  assert_eq!(
    resolve_color(node, COLOR),
    Some(red()),
    "a press hides it again"
  );

  owner.dispose();
}

/// Focus in a text entry shows even when a pointer press reached it, as
/// `:focus-visible` does in an `<input>`.
#[test]
fn focus_in_a_text_entry_shows_after_a_pointer_press() {
  reset();

  let owner = Owner::new();
  let node = owner.enter(|| {
    BasicTextField::new(TextEntry::new())
      .set_color(red().when(focus_visible(), blue()))
      .build()
  });

  let _ = trigger_pointer_down(node, &press_at(0.));

  trigger_focus(node);

  assert!(is_focused(node));
  assert_eq!(resolve_color(node, COLOR), Some(blue()));

  owner.dispose();
}

/// A disabled control reports it, takes no focus and ignores activation, letting keys
/// by; enabling it again restores all three.
#[test]
fn a_disabled_control_reports_it_and_ignores_activation() {
  reset();

  let owner = Owner::new();
  let disabled = Signal::new(true);
  let clicks = Rc::new(Cell::new(0));
  let node = owner.enter(|| {
    let clicks = Rc::clone(&clicks);

    BasicButton::new(Activate::new())
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
  assert_eq!(trigger_key_down(node, &key("Enter")), Prevented::default());
  assert_eq!(clicks.get(), 0);

  disabled.set(false);

  assert_eq!(box_props(node).map(|props| props.access), Some(vec![]));
  assert_eq!(control_state(node).map(|state| state.disabled), Some(false));
  assert!(is_tab_stop(node));
  assert_eq!(trigger_click(node), KEPT);
  assert_eq!(clicks.get(), 1);

  let fixed = owner.enter(|| BasicButton::new(Activate::new()).set_disabled(true).build());

  assert_eq!(focus_order(fixed), None);

  owner.dispose();
}

/// A label part supplies the accessible name: a string names the control at build,
/// and any view's text is read from the part.
#[test]
fn a_label_part_supplies_the_accessible_name() {
  reset();

  let owner = Owner::new();
  let named = owner.enter(|| BasicButton::new(Activate::new()).set_label("Save").build());
  let drawn = owner.enter(|| {
    BasicCheckbox::new(Toggle::new())
      .set_label(Label::new(Text::new("Remember me")))
      .build()
  });
  let unnamed = owner.enter(|| BasicButton::new(Activate::new()).build());
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

/// A text entry's selection is controlled beside its value: the bound signal places
/// it on the platform, and a move the platform reports writes the signal.
#[test]
fn a_text_entrys_selection_binds_both_ways() {
  reset();

  let owner = Owner::new();
  let bound = Signal::new(TextSelection::new(1, 3));
  let field = owner.enter(|| {
    BasicTextField::new(TextEntry::new().set_value("hello").bind_selection(bound)).build()
  });

  assert_eq!(selection(field), Some(TextSelection::new(1, 3)));
  assert!(listened(field).contains(&EventKind::SelectionChange));

  bound.set(TextSelection::collapsed(5));

  assert_eq!(selection(field), Some(TextSelection::collapsed(5)));

  trigger_selection_change(field, TextSelection::new(4, 2));

  assert_eq!(bound.get(), TextSelection::new(4, 2));
  assert!(bound.get().is_reversed());
  assert_eq!(bound.get().range(), 2..4);
  assert_eq!((bound.get().min(), bound.get().max()), (2, 4));
  assert_eq!(bound.get().range().len(), 2);

  bound.set(TextSelection::new(2, 9));

  assert_eq!(
    selection(field),
    Some(TextSelection::new(2, 5)),
    "an index past the value clamps to its end"
  );
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
      .set_child(BasicTextField::new(
        TextEntry::new().set_placeholder("Name").bind_value(bound),
      ))
      .build()
  });
  let field = only_child(outer);

  assert_eq!(value(field), Some(String::new()));
  assert_eq!(
    control_props(field).map(|props| props.native),
    Some(Some(NativeElement::TextField))
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

  let editor = owner.enter(|| BasicTextEditor::new(TextEntry::new().set_lines(4)).build());

  assert_eq!(
    control_props(editor).map(|props| props.native),
    Some(Some(NativeElement::TextEditor))
  );
  assert_eq!(control_props(editor).map(|props| props.lines), Some(4));

  owner.dispose();
}

/// A behavior places its label: a text entry's sits beside its box by default, a
/// button's inside, and a third party's text entry may keep its label inside instead.
#[test]
fn a_behavior_places_its_label() {
  /// A text entry keeping its label inside its box.
  struct InlineEntry;

  impl Behavior<Headless> for InlineEntry {
    fn native(&self) -> Option<NativeElement> {
      Some(NativeElement::TextField)
    }

    fn role(&self) -> Option<Role> {
      None
    }

    fn listens(&self) -> &'static [EventKind] {
      &[]
    }

    fn label_placement(&self) -> LabelPlacement {
      LabelPlacement::Trailing
    }
  }

  reset();

  let owner = Owner::new();
  let (field, button, inline) = owner.enter(|| {
    (
      BasicTextField::new(TextEntry::new())
        .set_label("Name")
        .build(),
      BasicButton::new(Activate::new()).set_label("Save").build(),
      Control::new(InlineEntry).set_label("Inline").build(),
    )
  });
  let beside = |node| control_props(node).map(|props| props.label_beside);

  assert_eq!(beside(field), Some(true));
  assert_eq!(beside(button), Some(false));
  assert_eq!(beside(inline), Some(false));

  owner.dispose();
}

/// A caption placed anywhere names a control by reference, after the caption its own
/// label builds.
#[test]
fn a_caption_placed_anywhere_names_a_control() {
  reset();

  let owner = Owner::new();
  let (caption, own, field) = owner.enter(|| {
    let caption = Text::new("Email").build();
    let own = Text::new("Work").build();
    let field = BasicTextField::new(TextEntry::new())
      .set_label(Label::new(AnyView::node(own)))
      .set_labelled_by(caption)
      .build();

    (caption, own, field)
  });

  assert!(
    box_props(field).is_some_and(|props| props
      .access
      .contains(&AccessProp::LabelledBy(vec![own, caption]))),
    "{:?}",
    box_props(field).map(|props| props.access)
  );
  assert_eq!(accessible_name(field).as_deref(), Some("Work Email"));

  owner.dispose();
}

/// An accessible name names a control no caption does and outranks a caption's, and
/// a derived one follows its signals.
#[test]
fn an_accessible_name_names_a_control_over_its_caption() {
  reset();

  let owner = Owner::new();
  let name = Signal::new("Close".to_owned());
  let (icon, derived, captioned) = owner.enter(|| {
    (
      BasicButton::new(Activate::new())
        .set_accessible_name("Close")
        .set_child(Container::new())
        .build(),
      BasicButton::new(Activate::new())
        .derive_accessible_name(move || name.get())
        .build(),
      BasicButton::new(Activate::new())
        .set_label("×")
        .set_accessible_name("Close")
        .build(),
    )
  });

  assert_eq!(accessible_name(icon).as_deref(), Some("Close"));
  assert!(
    box_props(icon)
      .is_some_and(|props| { props.access.contains(&AccessProp::Name("Close".to_owned())) })
  );
  assert_eq!(accessible_name(derived).as_deref(), Some("Close"));

  name.set("Dismiss".to_owned());

  assert_eq!(accessible_name(derived).as_deref(), Some("Dismiss"));
  assert_eq!(accessible_name(captioned).as_deref(), Some("Close"));

  owner.dispose();
}

/// A control's parts, body and label run across half an em apart unless the control
/// lays them out otherwise.
#[test]
fn a_control_lays_out_its_row() {
  reset();

  let owner = Owner::new();
  let (plain, spaced) = owner.enter(|| {
    (
      BasicButton::new(Activate::new()).set_label("a").build(),
      BasicButton::new(Activate::new())
        .set_axis(skene_core::Axis::Vertical)
        .set_spacing(Spacing::all(3.))
        .set_item_alignment(ItemAlignment::Start)
        .set_label("b")
        .build(),
    )
  });
  let spacing = PropertyKey::Shared(Property::Spacing);
  let direction = |node| {
    box_props(node).map(|props| match props.layout {
      skene_core::Layout::Flex { direction } => Some(direction),
      _ => None,
    })
  };

  assert_eq!(resolve_length(plain, spacing), Some(8.));
  assert_eq!(resolve_length(spaced, spacing), Some(3.));
  assert_eq!(direction(plain), Some(Some(skene_core::FlexDirection::Row)));
  assert_eq!(
    direction(spaced),
    Some(Some(skene_core::FlexDirection::Column))
  );

  owner.dispose();
}

/// A popup face opening a list is a combobox, and one opening a menu or a dialog a
/// button, each reporting the popup it opens.
#[test]
fn a_popup_face_is_what_its_popup_makes_it() {
  reset();

  let owner = Owner::new();
  let faces = [PopupKind::Listbox, PopupKind::Menu, PopupKind::Dialog].map(|kind| {
    (
      kind,
      owner.enter(|| Control::new(Popup::new().set_kind(kind)).build()),
    )
  });

  for (kind, face) in faces {
    let (native, role) = match kind {
      PopupKind::Listbox => (None, Role::ComboBox),
      PopupKind::Menu | PopupKind::Dialog => (Some(NativeElement::Button), Role::Button),
    };

    assert_eq!(control_props(face).map(|props| props.native), Some(native));
    assert_eq!(box_props(face).and_then(|props| props.role), Some(role));
    assert!(
      box_props(face).is_some_and(|props| props.access.contains(&AccessProp::HasPopup(kind))),
      "{kind:?}"
    );
  }

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
      BasicCheckbox::new(Toggle::new()).build(),
      BasicSlider::new(Slider::new()).build(),
      BasicTextField::new(TextEntry::new()).build(),
      BasicButton::new(Activate::new())
        .handle_focus(|| {})
        .handle_pointer_move(|_| {})
        .build(),
    )
  });

  assert_eq!(
    listened(checkbox),
    [
      EventKind::Click,
      EventKind::KeyDown,
      EventKind::KeyUp,
      EventKind::Blur,
      EventKind::Toggle,
    ]
  );
  assert_eq!(
    listened(slider),
    [
      EventKind::KeyDown,
      EventKind::PointerDown,
      EventKind::PointerUp,
      EventKind::PointerMove,
      EventKind::PointerCancel,
    ]
  );
  assert_eq!(listened(field), [EventKind::Change]);
  assert_eq!(
    listened(button),
    [
      EventKind::Click,
      EventKind::KeyDown,
      EventKind::KeyUp,
      EventKind::PointerMove,
      EventKind::Focus,
      EventKind::Blur,
    ]
  );

  owner.dispose();
}

/// A stepper: a third party's behavior, built through the public trait alone.
struct Stepper {
  count: Signal<i32>,
}

impl Behavior<Headless> for Stepper {
  fn native(&self) -> Option<NativeElement> {
    Some(NativeElement::Button)
  }

  fn role(&self) -> Option<Role> {
    Some(Role::Button)
  }

  fn listens(&self) -> &'static [EventKind] {
    &[EventKind::KeyDown, EventKind::Wheel]
  }

  fn on_key(&self, event: &KeyEvent, _: &ControlHandle<Headless>) -> Prevent {
    match event.key {
      Key::Named(NamedKey::ArrowUp) => self.count.set(self.count.get() + 1),
      Key::Named(NamedKey::ArrowDown) => self.count.set(self.count.get() - 1),
      _ => return Prevent::None,
    }

    Prevent::All
  }

  fn on_wheel(&self, event: &WheelEvent, _: &ControlHandle<Headless>) -> Prevent {
    let step = if event.delta.vertical < 0. { 1 } else { -1 };

    self.count.set(self.count.get() + step);

    Prevent::All
  }

  #[expect(
    clippy::cast_precision_loss,
    reason = "a test's count stays far below the integers f32 holds exactly"
  )]
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

  let turn = PointerEvent::mouse(0., 0.).wheel(Axes::new(0., -3.), WheelUnit::Lines);

  assert_eq!(
    trigger_wheel(node, &turn),
    CONSUMED,
    "the stepper takes the turn"
  );
  assert_eq!(count.get(), 3);

  trigger_focus(node);

  assert_eq!(resolve_color(node, COLOR), Some(blue()));

  owner.dispose();
}

/// A control's required, read-only and invalid flags report as accessibility, the
/// invalid flag reaches the control-scoped state, and a described-by names its node.
#[test]
fn a_controls_flags_report_as_accessibility_and_state() {
  reset();

  let owner = Owner::new();
  let invalid = Signal::new(false);
  let (hint, field) = owner.enter(|| {
    let hint = Text::new("Required").build();
    let field = BasicTextField::new(TextEntry::new())
      .set_required(true)
      .set_read_only(true)
      .derive_invalid(move || invalid.get())
      .set_described_by(hint)
      .build();

    (hint, field)
  });

  assert_eq!(
    box_props(field).map(|props| props.access),
    Some(vec![
      AccessProp::Required(true),
      AccessProp::ReadOnly(true),
      AccessProp::DescribedBy(vec![hint]),
    ])
  );
  assert_eq!(control_state(field).map(|state| state.invalid), Some(false));

  invalid.set(true);

  assert!(box_props(field).is_some_and(|props| props.access.contains(&AccessProp::Invalid(true))));
  assert_eq!(control_state(field).map(|state| state.invalid), Some(true));

  owner.dispose();
}

/// A vertical range reports its orientation and reads the pointer along its height,
/// bottom to top.
#[test]
fn a_vertical_range_reads_the_pointer_bottom_to_top() {
  use skene_core::Axis;

  reset();

  let owner = Owner::new();
  let bound = Signal::new(50.);
  let node = owner
    .enter(|| BasicSlider::new(Slider::new().set_axis(Axis::Vertical).bind_value(bound)).build());

  assert!(box_props(node).is_some_and(|props| {
    props
      .access
      .contains(&AccessProp::Orientation(Axis::Vertical))
  }));

  trigger_size(node, Size::new(10., 100.));

  let _ = trigger_pointer_down(
    node,
    &PointerEvent::mouse(5., 10.).with_press(PointerButton::Primary),
  );

  assert!(
    (bound.get() - 90.).abs() < 0.01,
    "near the top reads high: {}",
    bound.get()
  );

  owner.dispose();
}
