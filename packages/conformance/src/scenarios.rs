//! The suite's scenarios, each generic over the driver acting it out.

use std::time::Duration;

use skene_foundation::{
  Activate,
  BasicButton,
  BasicCheckbox,
  BasicListboxOption,
  BasicPicker,
  BasicRadio,
  BasicRadioGroup,
  BasicSlider,
  BasicTextField,
  Column,
  Container,
  ContainerBase,
  ContextMenu,
  DecorationBase,
  FocusOrder,
  HasChild,
  HasChildren,
  Key,
  KeyEvent,
  ListboxOption,
  Media,
  MenuEntry,
  NodeId,
  Overlay,
  Platform,
  Radio,
  RadioGroup,
  Role,
  Scalar,
  Size,
  Slider,
  TOOLTIP_DELAY,
  Text,
  TextEntry,
  TextSelection,
  Toggle,
  Tooltip,
  View,
  Visibility,
  When,
};
use skene_reactive::{Get, Set, Signal, Update};

use crate::{Driver, Scenario};

/// Every scenario, in the order the suite runs them.
pub(crate) fn all<D: Driver>() -> Vec<Scenario<D>> {
  vec![
    Scenario {
      name: "a button activates on a click, on Enter and on Space",
      act: a_button_activates,
    },
    Scenario {
      name: "a checkbox toggles on a click and on Space",
      act: a_checkbox_toggles,
    },
    Scenario {
      name: "Tab moves through the controls in order, Shift-Tab back",
      act: tab_moves_through_the_controls,
    },
    Scenario {
      name: "a disabled button takes no click and no tab stop",
      act: a_disabled_button_is_skipped,
    },
    Scenario {
      name: "typed text edits a bound field, Backspace removing a cluster",
      act: typed_text_edits_a_field,
    },
    Scenario {
      name: "a read-only field takes focus and no typed text",
      act: a_read_only_field_takes_no_text,
    },
    Scenario {
      name: "the arrows choose within a radio group, focus following",
      act: the_arrows_choose_within_a_radio_group,
    },
    Scenario {
      name: "a slider steps by the arrows and reaches its ends by Home and End",
      act: a_slider_steps_by_its_keys,
    },
    Scenario {
      name: "a picker opens on a click, chooses on Enter and returns focus to its face",
      act: a_picker_chooses_and_returns_focus,
    },
    Scenario {
      name: "a tooltip shows on focus and hides on Escape",
      act: a_tooltip_shows_on_focus,
    },
    Scenario {
      name: "a context menu opens on Shift-F10 and closes on Escape",
      act: a_context_menu_opens_on_shift_f10,
    },
    Scenario {
      name: "a tooltip shows once the pointer rests on its view for the delay",
      act: a_tooltip_shows_after_its_delay,
    },
    Scenario {
      name: "a condition on the viewport's width follows a resize",
      act: a_viewport_condition_follows_a_resize,
    },
    Scenario {
      name: "a hidden view takes the focus inside it along and gives it back on its return",
      act: a_hidden_view_keeps_its_focus,
    },
    Scenario {
      name: "focus moving to a node outside an open popup closes it",
      act: focus_leaving_a_popup_closes_it,
    },
    Scenario {
      name: "a modal takes focus to its first stop as it opens and returns it as it closes",
      act: a_modal_takes_focus_as_it_opens,
    },
    Scenario {
      name: "a form reads as its roles, names, values and states",
      act: a_form_reads_alike,
    },
    Scenario {
      name: "the arrows wrap around a radio group, which Tab passes as one stop",
      act: the_arrows_wrap_around_a_radio_group,
    },
    Scenario {
      name: "a click on a checkbox's label toggles it, and Enter does not",
      act: a_checkbox_label_toggles_it,
    },
    Scenario {
      name: "Escape closes a picker's list without choosing and returns focus to its face",
      act: escape_closes_a_picker_unchosen,
    },
    Scenario {
      name: "a slider pages by PageUp and PageDown",
      act: a_slider_pages,
    },
    Scenario {
      name: "a click on text outside any control takes focus from the field holding it",
      act: a_click_outside_blurs,
    },
    Scenario {
      name: "a combining mark typed after its base joins it, the caret after the cluster",
      act: a_combining_mark_joins_its_base,
    },
  ]
}

/// The press of the key its W3C value `name` names.
fn key(name: &str) -> KeyEvent {
  KeyEvent::new(name)
}

/// The press of the key typing `text`.
fn typed(text: &str) -> KeyEvent {
  KeyEvent {
    key: Key::Character(text.to_owned()),
    ..KeyEvent::new(text)
  }
}

/// The press of the key `name` names with `Shift` held.
fn shifted(name: &str) -> KeyEvent {
  let mut event = KeyEvent::new(name);

  event.modifiers.shift = true;

  event
}

/// The node of role `role` named `name`, which the scenario acts on.
fn found<D: Driver>(driver: &mut D, role: Role, name: &str) -> NodeId {
  driver
    .find(role, name)
    .unwrap_or_else(|| panic!("no {role:?} named {name:?}"))
}

/// A button named `name`.
fn button<D: Driver>(name: &str) -> BasicButton<D::Platform> {
  BasicButton::new(Activate::new()).set_label(name)
}

fn a_button_activates<D: Driver>(driver: &mut D) {
  let clicks = Signal::new(0u32);

  driver.mount(move || {
    button::<D>("Save")
      .handle_click(move || clicks.update(|count| *count += 1))
      .into_any()
  });

  let save = found(driver, Role::Button, "Save");

  driver.click(save);

  assert_eq!(clicks.get(), 1, "a click activates it once");
  assert_eq!(driver.focused(), Some(save), "a click focuses it");

  driver.press(&key("Enter"));

  assert_eq!(clicks.get(), 2, "Enter activates it");

  driver.press(&key(" "));

  assert_eq!(clicks.get(), 3, "Space activates it");
}

fn a_checkbox_toggles<D: Driver>(driver: &mut D) {
  let checked = Signal::new(false);

  driver.mount(move || {
    BasicCheckbox::<D::Platform>::new(Toggle::new().bind_checked(checked))
      .set_label("Remember me")
      .into_any()
  });

  let checkbox = found(driver, Role::CheckBox, "Remember me");

  driver.click(checkbox);

  assert!(checked.get(), "a click checks it");

  driver.press(&key(" "));

  assert!(!checked.get(), "Space unchecks it");
}

fn tab_moves_through_the_controls<D: Driver>(driver: &mut D) {
  driver.mount(|| {
    Column::<D::Platform>::new()
      .set_children((button::<D>("One"), button::<D>("Two"), button::<D>("Three")))
      .into_any()
  });

  let [one, two, three] = ["One", "Two", "Three"].map(|name| found(driver, Role::Button, name));

  driver.press(&key("Tab"));

  assert_eq!(
    driver.focused(),
    Some(one),
    "the first Tab reaches the first"
  );

  driver.press(&key("Tab"));

  assert_eq!(driver.focused(), Some(two));

  driver.press(&key("Tab"));

  assert_eq!(driver.focused(), Some(three));

  driver.press(&shifted("Tab"));

  assert_eq!(driver.focused(), Some(two), "Shift-Tab goes back");
}

fn a_disabled_button_is_skipped<D: Driver>(driver: &mut D) {
  let clicks = Signal::new(0u32);

  driver.mount(move || {
    Column::<D::Platform>::new()
      .set_children((
        button::<D>("One"),
        button::<D>("Two")
          .set_disabled(true)
          .handle_click(move || clicks.update(|count| *count += 1)),
        button::<D>("Three"),
      ))
      .into_any()
  });

  let [one, two, three] = ["One", "Two", "Three"].map(|name| found(driver, Role::Button, name));

  driver.click(two);

  assert_eq!(clicks.get(), 0, "a click does nothing");
  assert_ne!(driver.focused(), Some(two), "a click does not focus it");

  driver.focus(one);
  driver.press(&key("Tab"));

  assert_eq!(driver.focused(), Some(three), "Tab passes over it");
}

fn typed_text_edits_a_field<D: Driver>(driver: &mut D) {
  let value = Signal::new(String::new());

  driver.mount(move || {
    BasicTextField::<D::Platform>::new(TextEntry::new().bind_value(value))
      .set_label("Name")
      .into_any()
  });

  let field = found(driver, Role::TextInput, "Name");

  driver.type_text(field, "héllo");

  assert_eq!(value.get(), "héllo", "each cluster lands in order");
  assert_eq!(driver.focused(), Some(field), "typing starts with a click");

  driver.press(&key("Backspace"));

  assert_eq!(
    value.get(),
    "héll",
    "Backspace removes the cluster before the caret"
  );
}

fn a_read_only_field_takes_no_text<D: Driver>(driver: &mut D) {
  let value = Signal::new("fixed".to_owned());

  driver.mount(move || {
    BasicTextField::<D::Platform>::new(TextEntry::new().bind_value(value))
      .set_label("Name")
      .set_read_only(true)
      .into_any()
  });

  let field = found(driver, Role::TextInput, "Name");

  driver.type_text(field, "x");

  assert_eq!(value.get(), "fixed", "typed text changes nothing");
  assert_eq!(driver.focused(), Some(field), "a click still focuses it");

  driver.press(&key("Backspace"));

  assert_eq!(value.get(), "fixed", "Backspace removes nothing");
}

fn the_arrows_choose_within_a_radio_group<D: Driver>(driver: &mut D) {
  let chosen: Signal<Option<String>> = Signal::new(None);

  driver.mount(move || {
    BasicRadioGroup::<D::Platform>::new(RadioGroup::new().bind_chosen(chosen))
      .set_accessible_name("Size")
      .set_child(Column::new().set_children((
        BasicRadio::new(Radio::new("s")).set_label("Small"),
        BasicRadio::new(Radio::new("m")).set_label("Medium"),
        BasicRadio::new(Radio::new("l")).set_label("Large"),
      )))
      .into_any()
  });

  let [small, medium] = ["Small", "Medium"].map(|name| found(driver, Role::RadioButton, name));

  driver.click(small);

  assert_eq!(chosen.get().as_deref(), Some("s"), "a click chooses");
  assert_eq!(driver.focused(), Some(small));

  driver.press(&key("ArrowDown"));

  assert_eq!(
    chosen.get().as_deref(),
    Some("m"),
    "ArrowDown chooses the next"
  );
  assert_eq!(driver.focused(), Some(medium), "focus follows the choice");

  driver.press(&key("ArrowUp"));

  assert_eq!(
    chosen.get().as_deref(),
    Some("s"),
    "ArrowUp chooses the previous"
  );
}

fn a_slider_steps_by_its_keys<D: Driver>(driver: &mut D) {
  let value = Signal::new(50f32);

  driver.mount(move || {
    BasicSlider::<D::Platform>::new(Slider::new().bind_value(value))
      .set_accessible_name("Volume")
      .into_any()
  });

  let slider = found(driver, Role::Slider, "Volume");

  driver.focus(slider);
  driver.press(&key("ArrowRight"));

  assert!(
    (value.get() - 51.).abs() < f32::EPSILON,
    "ArrowRight steps up"
  );

  driver.press(&key("ArrowLeft"));

  assert!(
    (value.get() - 50.).abs() < f32::EPSILON,
    "ArrowLeft steps down"
  );

  driver.press(&key("End"));

  assert!(
    (value.get() - 100.).abs() < f32::EPSILON,
    "End reaches the maximum"
  );

  driver.press(&key("Home"));

  assert!(value.get().abs() < f32::EPSILON, "Home reaches the minimum");
}

fn a_picker_chooses_and_returns_focus<D: Driver>(driver: &mut D) {
  let value = Signal::new("b".to_owned());

  driver.mount(move || {
    BasicPicker::<D::Platform>::new()
      .bind_value(value)
      .set_label("Letter")
      .set_options(
        ["a", "b", "c"]
          .map(|letter| BasicListboxOption::new(ListboxOption::new(letter)).set_label(letter)),
      )
      .set_face(|shown| Text::derive(move || shown.get()))
      .into_any()
  });

  let face = found(driver, Role::ComboBox, "Letter");

  assert!(
    driver.find(Role::ListBoxOption, "c").is_none(),
    "the list starts closed"
  );

  driver.click(face);

  let chosen = found(driver, Role::ListBoxOption, "b");

  assert_eq!(
    driver.focused(),
    Some(chosen),
    "focus lands on the chosen option"
  );

  driver.press(&key("ArrowDown"));
  driver.press(&key("Enter"));

  assert_eq!(value.get(), "c", "Enter chooses the focused option");
  assert!(
    driver.find(Role::ListBoxOption, "c").is_none(),
    "choosing closes the list"
  );
  assert_eq!(driver.focused(), Some(face), "focus returns to the face");
}

fn a_tooltip_shows_on_focus<D: Driver>(driver: &mut D) {
  driver.mount(|| {
    Tooltip::<D::Platform>::new("Saves your work")
      .set_child(button::<D>("Save"))
      .into_any()
  });

  assert!(
    driver.find(Role::Tooltip, "Saves your work").is_none(),
    "the tooltip starts hidden"
  );

  driver.press(&key("Tab"));

  assert!(
    driver.find(Role::Tooltip, "Saves your work").is_some(),
    "focus shows it at once"
  );

  driver.press(&key("Escape"));

  assert!(
    driver.find(Role::Tooltip, "Saves your work").is_none(),
    "Escape hides it"
  );
}

fn a_context_menu_opens_on_shift_f10<D: Driver>(driver: &mut D) {
  let renamed = Signal::new(false);

  driver.mount(move || {
    ContextMenu::<D::Platform>::new()
      .set_items([MenuEntry::new("Rename").handle_select(move || renamed.set(true))])
      .set_child(
        Container::new()
          .set_role(Role::Group)
          .set_label("Note")
          .set_focus_order(FocusOrder::Auto)
          .set_child(Text::new("A note")),
      )
      .into_any()
  });

  let note = found(driver, Role::Group, "Note");

  driver.focus(note);
  driver.press(&shifted("F10"));

  let rename = found(driver, Role::MenuItem, "Rename");

  assert_eq!(driver.focused(), Some(rename), "the first item takes focus");

  driver.press(&key("Escape"));

  assert!(
    driver.find(Role::MenuItem, "Rename").is_none(),
    "Escape closes the menu"
  );
  assert_eq!(driver.focused(), Some(note), "focus returns to the note");
  assert!(!renamed.get(), "closing runs nothing");
}

fn a_tooltip_shows_after_its_delay<D: Driver>(driver: &mut D) {
  driver.mount(|| {
    Tooltip::<D::Platform>::new("Saves your work")
      .set_child(button::<D>("Save"))
      .into_any()
  });

  let save = found(driver, Role::Button, "Save");

  driver.hover(save);

  assert!(
    driver.find(Role::Tooltip, "Saves your work").is_none(),
    "a pointer arriving shows nothing at once"
  );

  driver.advance(TOOLTIP_DELAY);

  assert!(
    driver.find(Role::Tooltip, "Saves your work").is_some(),
    "the pointer resting for the delay shows it"
  );
}

fn a_hidden_view_keeps_its_focus<D: Driver>(driver: &mut D) {
  const RETURN: Duration = Duration::from_millis(100);

  let shown = Signal::new(true);

  driver.mount(move || {
    Column::<D::Platform>::new()
      .set_children((
        Visibility::<D::Platform>::new()
          .derive_shown(move || shown.get())
          .set_child(button::<D>("Dismiss").handle_click(move || {
            shown.set(false);

            let _timer = D::Platform::schedule(RETURN, Box::new(move || shown.set(true)));
          })),
        button::<D>("Elsewhere"),
      ))
      .into_any()
  });

  let dismiss = found(driver, Role::Button, "Dismiss");

  driver.focus(dismiss);
  driver.press(&key("Enter"));

  assert_eq!(driver.focused(), None, "focus leaves with the view");
  assert_eq!(
    driver.find(Role::Button, "Dismiss"),
    None,
    "a reader finds nothing hidden"
  );

  driver.advance(RETURN);

  assert_eq!(
    driver.focused(),
    Some(dismiss),
    "focus returns with the view"
  );

  driver.press(&key("Enter"));

  let elsewhere = found(driver, Role::Button, "Elsewhere");

  driver.focus(elsewhere);
  driver.advance(RETURN);

  assert_eq!(
    driver.focused(),
    Some(elsewhere),
    "focus moved on meanwhile stays where it went"
  );
}

fn focus_leaving_a_popup_closes_it<D: Driver>(driver: &mut D) {
  let value = Signal::new("b".to_owned());

  driver.mount(move || {
    Column::<D::Platform>::new()
      .set_children((
        BasicPicker::<D::Platform>::new()
          .bind_value(value)
          .set_label("Letter")
          .set_options(
            ["a", "b", "c"]
              .map(|letter| BasicListboxOption::new(ListboxOption::new(letter)).set_label(letter)),
          )
          .set_face(|shown| Text::derive(move || shown.get())),
        button::<D>("Elsewhere"),
      ))
      .into_any()
  });

  let face = found(driver, Role::ComboBox, "Letter");

  driver.click(face);

  assert!(
    driver.find(Role::ListBoxOption, "c").is_some(),
    "a click opens the list"
  );

  let elsewhere = found(driver, Role::Button, "Elsewhere");

  driver.focus(elsewhere);

  assert!(
    driver.find(Role::ListBoxOption, "c").is_none(),
    "focus leaving closes the list"
  );
  assert_eq!(driver.focused(), Some(elsewhere));
}

fn a_modal_takes_focus_as_it_opens<D: Driver>(driver: &mut D) {
  let open = Signal::new(false);

  driver.mount(move || {
    Column::<D::Platform>::new()
      .set_children((
        button::<D>("Open").handle_click(move || open.set(true)),
        Overlay::<D::Platform>::modal()
          .bind_open(open)
          .set_child(button::<D>("Inside")),
      ))
      .into_any()
  });

  let opener = found(driver, Role::Button, "Open");

  driver.click(opener);

  let inside = found(driver, Role::Button, "Inside");

  assert_eq!(
    driver.focused(),
    Some(inside),
    "the modal's first stop takes focus"
  );

  driver.press(&key("Escape"));

  assert!(!open.get(), "Escape dismisses it");
  assert_eq!(
    driver.focused(),
    Some(opener),
    "focus returns to its opener"
  );
}

fn a_viewport_condition_follows_a_resize<D: Driver>(driver: &mut D) {
  let length = <<D::Platform as Platform>::Scalar as Scalar>::from_f32;

  driver.mount(move || {
    skene_foundation::Container::<D::Platform>::new()
      .set_hidden(true.when(Media::MinWidth(length(100.)), false))
      .set_child(Text::new("Wide"))
      .into_any()
  });

  driver.resize(Size::new(length(200.), length(40.)));

  assert!(
    driver.find_text("Wide").is_some(),
    "a wide viewport shows it"
  );

  driver.resize(Size::new(length(50.), length(40.)));

  assert!(driver.find_text("Wide").is_none(), "a narrow one hides it");
}

fn a_form_reads_alike<D: Driver>(driver: &mut D) {
  driver.mount(|| {
    Column::<D::Platform>::new()
      .set_children((
        skene_foundation::Container::new()
          .set_role(Role::Heading)
          .set_child(Text::new("Sign in")),
        BasicTextField::new(TextEntry::new().set_value("ada")).set_label("Name"),
        BasicCheckbox::new(Toggle::new().set_checked(true)).set_label("Remember me"),
        button::<D>("Continue").set_disabled(true),
      ))
      .into_any()
  });

  let checkbox = found(driver, Role::CheckBox, "Remember me");

  driver.click(checkbox);

  assert_eq!(
    driver.access_outline(),
    [
      "Window",
      "  Heading \"Sign in\"",
      "    Label = \"Sign in\"",
      "  TextInput \"Name\" = \"ada\"",
      "    Label = \"Name\"",
      "  CheckBox \"Remember me\" [focused, checked]",
      "    Label = \"Remember me\"",
      "  Button \"Continue\" [disabled]",
      "    Label = \"Continue\"",
      "",
    ]
    .join("\n")
  );
}

fn the_arrows_wrap_around_a_radio_group<D: Driver>(driver: &mut D) {
  let chosen: Signal<Option<String>> = Signal::new(None);

  driver.mount(move || {
    Column::<D::Platform>::new()
      .set_children((
        BasicRadioGroup::<D::Platform>::new(RadioGroup::new().bind_chosen(chosen))
          .set_accessible_name("Size")
          .set_child(Column::new().set_children((
            BasicRadio::new(Radio::new("s")).set_label("Small"),
            BasicRadio::new(Radio::new("m")).set_label("Medium"),
            BasicRadio::new(Radio::new("l")).set_label("Large"),
          ))),
        button::<D>("Next"),
      ))
      .into_any()
  });

  let [small, large] = ["Small", "Large"].map(|name| found(driver, Role::RadioButton, name));
  let next = found(driver, Role::Button, "Next");

  driver.click(large);
  driver.press(&key("ArrowDown"));

  assert_eq!(
    chosen.get().as_deref(),
    Some("s"),
    "ArrowDown on the last chooses the first"
  );
  assert_eq!(driver.focused(), Some(small), "focus follows the choice");

  driver.press(&key("ArrowUp"));

  assert_eq!(
    chosen.get().as_deref(),
    Some("l"),
    "ArrowUp on the first chooses the last"
  );

  driver.press(&key("Tab"));

  assert_eq!(driver.focused(), Some(next), "Tab leaves the group");

  driver.press(&shifted("Tab"));

  assert_eq!(
    driver.focused(),
    Some(large),
    "Shift-Tab returns to the chosen radio"
  );
}

fn a_checkbox_label_toggles_it<D: Driver>(driver: &mut D) {
  let checked = Signal::new(false);

  driver.mount(move || {
    BasicCheckbox::<D::Platform>::new(Toggle::new().bind_checked(checked))
      .set_label("Remember me")
      .into_any()
  });

  let label = driver
    .find_text("Remember me")
    .expect("the label's text is drawn");

  driver.click(label);

  assert!(checked.get(), "a click on the label checks it");

  driver.press(&key("Enter"));

  assert!(checked.get(), "Enter leaves it as it was");
}

fn escape_closes_a_picker_unchosen<D: Driver>(driver: &mut D) {
  let value = Signal::new("b".to_owned());

  driver.mount(move || {
    BasicPicker::<D::Platform>::new()
      .bind_value(value)
      .set_label("Letter")
      .set_options(
        ["a", "b", "c"]
          .map(|letter| BasicListboxOption::new(ListboxOption::new(letter)).set_label(letter)),
      )
      .set_face(|shown| Text::derive(move || shown.get()))
      .into_any()
  });

  let face = found(driver, Role::ComboBox, "Letter");

  driver.click(face);
  driver.press(&key("ArrowDown"));
  driver.press(&key("Escape"));

  assert_eq!(value.get(), "b", "Escape chooses nothing");
  assert!(
    driver.find(Role::ListBoxOption, "c").is_none(),
    "Escape closes the list"
  );
  assert_eq!(driver.focused(), Some(face), "focus returns to the face");
}

fn a_slider_pages<D: Driver>(driver: &mut D) {
  let value = Signal::new(50f32);

  driver.mount(move || {
    BasicSlider::<D::Platform>::new(Slider::new().bind_value(value))
      .set_accessible_name("Volume")
      .into_any()
  });

  let slider = found(driver, Role::Slider, "Volume");

  driver.focus(slider);
  driver.press(&key("PageUp"));

  assert!(
    (value.get() - 60.).abs() < f32::EPSILON,
    "PageUp raises it by a tenth of its range, not {}",
    value.get()
  );

  driver.press(&key("PageDown"));
  driver.press(&key("PageDown"));

  assert!(
    (value.get() - 40.).abs() < f32::EPSILON,
    "PageDown lowers it by a tenth of its range, not {}",
    value.get()
  );
}

fn a_click_outside_blurs<D: Driver>(driver: &mut D) {
  driver.mount(|| {
    Column::<D::Platform>::new()
      .set_children((
        BasicTextField::<D::Platform>::new(TextEntry::new()).set_label("Name"),
        Text::new("Plain words"),
      ))
      .into_any()
  });

  let field = found(driver, Role::TextInput, "Name");

  driver.focus(field);

  assert_eq!(driver.focused(), Some(field));

  let words = driver.find_text("Plain words").expect("the text is drawn");

  driver.click(words);

  assert_eq!(driver.focused(), None, "the field loses focus");
}

fn a_combining_mark_joins_its_base<D: Driver>(driver: &mut D) {
  let value = Signal::new(String::new());
  let selection = Signal::new(TextSelection::collapsed(0));

  driver.mount(move || {
    BasicTextField::<D::Platform>::new(TextEntry::new().bind_value(value).bind_selection(selection))
      .set_label("Name")
      .into_any()
  });

  let field = found(driver, Role::TextInput, "Name");

  driver.type_text(field, "e");
  driver.press(&typed("\u{301}"));

  assert_eq!(value.get(), "e\u{301}", "the mark lands after its base");
  assert_eq!(
    selection.get(),
    TextSelection::collapsed(1),
    "the caret sits after the one cluster they make"
  );
}
