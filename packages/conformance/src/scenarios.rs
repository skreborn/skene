//! The suite's scenarios, each generic over the driver acting it out.

use skene_foundation::{IntoScalar, Key, KeyEvent, Role, Size, TOOLTIP_DELAY};

use crate::{BREAKPOINT, Driver, RETURN, Scenario};

/// Every scenario, in the order the suite runs them.
#[expect(clippy::too_many_lines, reason = "one entry per scenario")]
pub(crate) fn all<D: Driver>() -> Vec<Scenario<D>> {
  vec![
    Scenario {
      name: "a button activates on a click, on Enter and on Space",
      page: "button",
      act: a_button_activates,
    },
    Scenario {
      name: "a checkbox toggles on a click and on Space",
      page: "checkbox",
      act: a_checkbox_toggles,
    },
    Scenario {
      name: "Tab moves through the controls in order, Shift-Tab back",
      page: "buttons",
      act: tab_moves_through_the_controls,
    },
    Scenario {
      name: "a disabled button takes no click and no tab stop",
      page: "disabled-button",
      act: a_disabled_button_is_skipped,
    },
    Scenario {
      name: "typed text edits a bound field, Backspace removing a cluster",
      page: "text-field",
      act: typed_text_edits_a_field,
    },
    Scenario {
      name: "a read-only field takes focus and no typed text",
      page: "read-only-field",
      act: a_read_only_field_takes_no_text,
    },
    Scenario {
      name: "the arrows choose within a radio group, focus following",
      page: "radio-group",
      act: the_arrows_choose_within_a_radio_group,
    },
    Scenario {
      name: "a slider steps by the arrows and reaches its ends by Home and End",
      page: "slider",
      act: a_slider_steps_by_its_keys,
    },
    Scenario {
      name: "a picker opens on a click, chooses on Enter and returns focus to its face",
      page: "picker",
      act: a_picker_chooses_and_returns_focus,
    },
    Scenario {
      name: "a tooltip shows on focus and hides on Escape",
      page: "tooltip",
      act: a_tooltip_shows_on_focus,
    },
    Scenario {
      name: "a context menu opens on Shift-F10 and closes on Escape",
      page: "context-menu",
      act: a_context_menu_opens_on_shift_f10,
    },
    Scenario {
      name: "a context menu's item runs on Enter, closing the menu",
      page: "context-menu",
      act: a_context_menu_item_runs_on_enter,
    },
    Scenario {
      name: "a tooltip shows once the pointer rests on its view for the delay",
      page: "tooltip",
      act: a_tooltip_shows_after_its_delay,
    },
    Scenario {
      name: "a condition on the viewport's width follows a resize",
      page: "viewport",
      act: a_viewport_condition_follows_a_resize,
    },
    Scenario {
      name: "a hidden view takes the focus inside it along and gives it back on its return",
      page: "visibility",
      act: a_hidden_view_keeps_its_focus,
    },
    Scenario {
      name: "focus moving to a node outside an open popup closes it",
      page: "picker",
      act: focus_leaving_a_popup_closes_it,
    },
    Scenario {
      name: "a modal takes focus to its first stop as it opens and returns it as it closes",
      page: "modal",
      act: a_modal_takes_focus_as_it_opens,
    },
    Scenario {
      name: "a form reads as its roles, names, values and states",
      page: "form",
      act: a_form_reads_alike,
    },
    Scenario {
      name: "the arrows wrap around a radio group, which Tab passes as one stop",
      page: "radio-group",
      act: the_arrows_wrap_around_a_radio_group,
    },
    Scenario {
      name: "a click on a checkbox's label toggles it, and Enter does not",
      page: "checkbox",
      act: a_checkbox_label_toggles_it,
    },
    Scenario {
      name: "Escape closes a picker's list without choosing and returns focus to its face",
      page: "picker",
      act: escape_closes_a_picker_unchosen,
    },
    Scenario {
      name: "a slider pages by PageUp and PageDown",
      page: "slider",
      act: a_slider_pages,
    },
    Scenario {
      name: "a click on text outside any control takes focus from the field holding it",
      page: "text-field",
      act: a_click_outside_blurs,
    },
    Scenario {
      name: "a combining mark typed after its base joins it, the caret after the cluster",
      page: "text-field",
      act: a_combining_mark_joins_its_base,
    },
    Scenario {
      name: "focus shows after a key and not after a press",
      page: "buttons",
      act: focus_shows_after_a_key,
    },
    Scenario {
      name: "a text entry shows its focus however it is reached",
      page: "text-field",
      act: a_text_entry_shows_its_focus,
    },
    Scenario {
      name: "a press holds pressed along its path until its release",
      page: "nested-boxes",
      act: a_press_holds_along_its_path,
    },
    Scenario {
      name: "a box hears focus entering and leaving its subtree, not moving within it",
      page: "focus-group",
      act: a_box_hears_focus_entering_and_leaving,
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
fn found<D: Driver>(driver: &mut D, role: Role, name: &str) -> D::Node {
  driver
    .find(role, name)
    .unwrap_or_else(|| panic!("no {role:?} named {name:?}"))
}

fn a_button_activates<D: Driver>(driver: &mut D) {
  let save = found(driver, Role::Button, "Save");

  driver.click(&save);

  assert_eq!(driver.probe("clicks"), "1", "a click activates it once");
  assert_eq!(driver.focused().as_ref(), Some(&save), "a click focuses it");

  driver.press(&key("Enter"));

  assert_eq!(driver.probe("clicks"), "2", "Enter activates it");

  driver.press(&key(" "));

  assert_eq!(driver.probe("clicks"), "3", "Space activates it");
}

fn a_checkbox_toggles<D: Driver>(driver: &mut D) {
  let checkbox = found(driver, Role::CheckBox, "Remember me");

  driver.click(&checkbox);

  assert_eq!(driver.probe("checked"), "true", "a click checks it");

  driver.press(&key(" "));

  assert_eq!(driver.probe("checked"), "false", "Space unchecks it");
}

fn tab_moves_through_the_controls<D: Driver>(driver: &mut D) {
  let [one, two, three] = ["One", "Two", "Three"].map(|name| found(driver, Role::Button, name));

  driver.press(&key("Tab"));

  assert_eq!(
    driver.focused().as_ref(),
    Some(&one),
    "the first Tab reaches the first"
  );

  driver.press(&key("Tab"));

  assert_eq!(driver.focused().as_ref(), Some(&two));

  driver.press(&key("Tab"));

  assert_eq!(driver.focused().as_ref(), Some(&three));

  driver.press(&shifted("Tab"));

  assert_eq!(driver.focused().as_ref(), Some(&two), "Shift-Tab goes back");
}

fn a_disabled_button_is_skipped<D: Driver>(driver: &mut D) {
  let [one, two, three] = ["One", "Two", "Three"].map(|name| found(driver, Role::Button, name));

  driver.click(&two);

  assert_eq!(driver.probe("clicks"), "0", "a click does nothing");
  assert_ne!(
    driver.focused().as_ref(),
    Some(&two),
    "a click does not focus it"
  );

  driver.focus(&one);
  driver.press(&key("Tab"));

  assert_eq!(
    driver.focused().as_ref(),
    Some(&three),
    "Tab passes over it"
  );
}

fn typed_text_edits_a_field<D: Driver>(driver: &mut D) {
  let field = found(driver, Role::TextInput, "Name");

  driver.type_text(&field, "héllo");

  assert_eq!(
    driver.probe("value"),
    "héllo",
    "each cluster lands in order"
  );
  assert_eq!(
    driver.focused().as_ref(),
    Some(&field),
    "typing starts with a click"
  );

  driver.press(&key("Backspace"));

  assert_eq!(
    driver.probe("value"),
    "héll",
    "Backspace removes the cluster before the caret"
  );
}

fn a_read_only_field_takes_no_text<D: Driver>(driver: &mut D) {
  let field = found(driver, Role::TextInput, "Name");

  driver.type_text(&field, "x");

  assert_eq!(driver.probe("value"), "fixed", "typed text changes nothing");
  assert_eq!(
    driver.focused().as_ref(),
    Some(&field),
    "a click still focuses it"
  );

  driver.press(&key("Backspace"));

  assert_eq!(driver.probe("value"), "fixed", "Backspace removes nothing");
}

fn the_arrows_choose_within_a_radio_group<D: Driver>(driver: &mut D) {
  let [small, medium] = ["Small", "Medium"].map(|name| found(driver, Role::RadioButton, name));

  driver.click(&small);

  assert_eq!(driver.probe("chosen"), "s", "a click chooses");
  assert_eq!(driver.focused().as_ref(), Some(&small));

  driver.press(&key("ArrowDown"));

  assert_eq!(driver.probe("chosen"), "m", "ArrowDown chooses the next");
  assert_eq!(
    driver.focused().as_ref(),
    Some(&medium),
    "focus follows the choice"
  );

  driver.press(&key("ArrowUp"));

  assert_eq!(driver.probe("chosen"), "s", "ArrowUp chooses the previous");
}

fn a_slider_steps_by_its_keys<D: Driver>(driver: &mut D) {
  let slider = found(driver, Role::Slider, "Volume");

  driver.focus(&slider);
  driver.press(&key("ArrowRight"));

  assert_eq!(driver.probe("value"), "51", "ArrowRight steps up");

  driver.press(&key("ArrowLeft"));

  assert_eq!(driver.probe("value"), "50", "ArrowLeft steps down");

  driver.press(&key("End"));

  assert_eq!(driver.probe("value"), "100", "End reaches the maximum");

  driver.press(&key("Home"));

  assert_eq!(driver.probe("value"), "0", "Home reaches the minimum");
}

fn a_picker_chooses_and_returns_focus<D: Driver>(driver: &mut D) {
  let face = found(driver, Role::ComboBox, "Letter");

  assert!(
    driver.find(Role::ListBoxOption, "c").is_none(),
    "the list starts closed"
  );

  driver.click(&face);

  let chosen = found(driver, Role::ListBoxOption, "b");

  assert_eq!(
    driver.focused().as_ref(),
    Some(&chosen),
    "focus lands on the chosen option"
  );

  driver.press(&key("ArrowDown"));
  driver.press(&key("Enter"));

  assert_eq!(
    driver.probe("chosen"),
    "c",
    "Enter chooses the focused option"
  );
  assert!(
    driver.find(Role::ListBoxOption, "c").is_none(),
    "choosing closes the list"
  );
  assert_eq!(
    driver.focused().as_ref(),
    Some(&face),
    "focus returns to the face"
  );
}

fn a_tooltip_shows_on_focus<D: Driver>(driver: &mut D) {
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
  let note = found(driver, Role::Group, "Note");

  driver.focus(&note);
  driver.press(&shifted("F10"));

  let rename = found(driver, Role::MenuItem, "Rename");

  assert_eq!(
    driver.focused().as_ref(),
    Some(&rename),
    "the first item takes focus"
  );

  driver.press(&key("Escape"));

  assert!(
    driver.find(Role::MenuItem, "Rename").is_none(),
    "Escape closes the menu"
  );
  assert_eq!(
    driver.focused().as_ref(),
    Some(&note),
    "focus returns to the note"
  );
  assert_eq!(driver.probe("renamed"), "false", "closing runs nothing");
}

fn a_context_menu_item_runs_on_enter<D: Driver>(driver: &mut D) {
  let note = found(driver, Role::Group, "Note");

  driver.focus(&note);
  driver.press(&shifted("F10"));

  let rename = found(driver, Role::MenuItem, "Rename");

  assert_eq!(driver.focused().as_ref(), Some(&rename));

  driver.press(&key("Enter"));

  assert_eq!(driver.probe("renamed"), "true", "Enter runs the item");
  assert!(
    driver.find(Role::MenuItem, "Rename").is_none(),
    "running it closes the menu"
  );
  assert_eq!(
    driver.focused().as_ref(),
    Some(&note),
    "focus returns to the note"
  );
}

fn a_tooltip_shows_after_its_delay<D: Driver>(driver: &mut D) {
  let save = found(driver, Role::Button, "Save");

  driver.hover(&save);

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
  let dismiss = found(driver, Role::Button, "Dismiss");

  driver.focus(&dismiss);
  driver.press(&key("Enter"));

  assert_eq!(driver.focused(), None, "focus leaves with the view");
  assert_eq!(
    driver.find(Role::Button, "Dismiss"),
    None,
    "a reader finds nothing hidden"
  );

  driver.advance(RETURN);

  assert_eq!(
    driver.focused().as_ref(),
    Some(&dismiss),
    "focus returns with the view"
  );

  driver.press(&key("Enter"));

  let elsewhere = found(driver, Role::Button, "Elsewhere");

  driver.focus(&elsewhere);
  driver.advance(RETURN);

  assert_eq!(
    driver.focused().as_ref(),
    Some(&elsewhere),
    "focus moved on meanwhile stays where it went"
  );
}

fn focus_leaving_a_popup_closes_it<D: Driver>(driver: &mut D) {
  let face = found(driver, Role::ComboBox, "Letter");

  driver.click(&face);

  assert!(
    driver.find(Role::ListBoxOption, "c").is_some(),
    "a click opens the list"
  );

  let elsewhere = found(driver, Role::Button, "Elsewhere");

  driver.focus(&elsewhere);

  assert!(
    driver.find(Role::ListBoxOption, "c").is_none(),
    "focus leaving closes the list"
  );
  assert_eq!(driver.focused().as_ref(), Some(&elsewhere));
}

fn a_modal_takes_focus_as_it_opens<D: Driver>(driver: &mut D) {
  let opener = found(driver, Role::Button, "Open");

  driver.click(&opener);

  let inside = found(driver, Role::Button, "Inside");

  assert_eq!(
    driver.focused().as_ref(),
    Some(&inside),
    "the modal's first stop takes focus"
  );

  driver.press(&key("Escape"));

  assert_eq!(driver.probe("open"), "false", "Escape dismisses it");
  assert_eq!(
    driver.focused().as_ref(),
    Some(&opener),
    "focus returns to its opener"
  );
}

fn a_viewport_condition_follows_a_resize<D: Driver>(driver: &mut D) {
  driver.resize(Size::new(
    (BREAKPOINT + 100).into_scalar(),
    100.into_scalar(),
  ));

  assert!(
    driver.find_text("Wide").is_some(),
    "a wide viewport shows it"
  );

  driver.resize(Size::new(
    (BREAKPOINT - 100).into_scalar(),
    100.into_scalar(),
  ));

  assert!(driver.find_text("Wide").is_none(), "a narrow one hides it");
}

fn a_form_reads_alike<D: Driver>(driver: &mut D) {
  let checkbox = found(driver, Role::CheckBox, "Remember me");

  driver.click(&checkbox);

  assert!(
    driver.find(Role::Heading, "Sign in").is_some(),
    "the heading is named by its text"
  );

  let name = found(driver, Role::TextInput, "Name");

  assert_eq!(
    driver.read(&name).value.as_deref(),
    Some("ada"),
    "the field holds its text"
  );
  assert!(
    driver.read(&checkbox).checked,
    "a click leaves a checkbox fixed checked as it was"
  );
  assert_eq!(
    driver.focused().as_ref(),
    Some(&checkbox),
    "a click focuses it"
  );

  let button = found(driver, Role::Button, "Continue");

  assert!(
    driver.read(&button).disabled,
    "the button reads as disabled"
  );
  assert!(
    !driver.read(&name).disabled && !driver.read(&checkbox).disabled,
    "the rest read as enabled"
  );
}

fn the_arrows_wrap_around_a_radio_group<D: Driver>(driver: &mut D) {
  let [small, large] = ["Small", "Large"].map(|name| found(driver, Role::RadioButton, name));
  let next = found(driver, Role::Button, "Next");

  driver.click(&large);
  driver.press(&key("ArrowDown"));

  assert_eq!(
    driver.probe("chosen"),
    "s",
    "ArrowDown on the last chooses the first"
  );
  assert_eq!(
    driver.focused().as_ref(),
    Some(&small),
    "focus follows the choice"
  );

  driver.press(&key("ArrowUp"));

  assert_eq!(
    driver.probe("chosen"),
    "l",
    "ArrowUp on the first chooses the last"
  );

  driver.press(&key("Tab"));

  assert_eq!(
    driver.focused().as_ref(),
    Some(&next),
    "Tab leaves the group"
  );

  driver.press(&shifted("Tab"));

  assert_eq!(
    driver.focused().as_ref(),
    Some(&large),
    "Shift-Tab returns to the chosen radio"
  );
}

fn a_checkbox_label_toggles_it<D: Driver>(driver: &mut D) {
  let label = driver
    .find_text("Remember me")
    .expect("the label's text is drawn");

  driver.click(&label);

  assert_eq!(
    driver.probe("checked"),
    "true",
    "a click on the label checks it"
  );

  driver.press(&key("Enter"));

  assert_eq!(driver.probe("checked"), "true", "Enter leaves it as it was");
}

fn escape_closes_a_picker_unchosen<D: Driver>(driver: &mut D) {
  let face = found(driver, Role::ComboBox, "Letter");

  driver.click(&face);
  driver.press(&key("ArrowDown"));
  driver.press(&key("Escape"));

  assert_eq!(driver.probe("chosen"), "b", "Escape chooses nothing");
  assert!(
    driver.find(Role::ListBoxOption, "c").is_none(),
    "Escape closes the list"
  );
  assert_eq!(
    driver.focused().as_ref(),
    Some(&face),
    "focus returns to the face"
  );
}

fn a_slider_pages<D: Driver>(driver: &mut D) {
  let slider = found(driver, Role::Slider, "Volume");

  driver.focus(&slider);
  driver.press(&key("PageUp"));

  assert_eq!(
    driver.probe("value"),
    "60",
    "PageUp raises it by a tenth of its range"
  );

  driver.press(&key("PageDown"));
  driver.press(&key("PageDown"));

  assert_eq!(
    driver.probe("value"),
    "40",
    "PageDown lowers it by a tenth of its range"
  );
}

fn a_click_outside_blurs<D: Driver>(driver: &mut D) {
  let field = found(driver, Role::TextInput, "Name");

  driver.focus(&field);

  assert_eq!(driver.focused().as_ref(), Some(&field));

  let words = driver.find_text("Plain words").expect("the text is drawn");

  driver.click(&words);

  assert_eq!(driver.focused(), None, "the field loses focus");
}

fn a_combining_mark_joins_its_base<D: Driver>(driver: &mut D) {
  let field = found(driver, Role::TextInput, "Name");

  driver.type_text(&field, "e");
  driver.press(&typed("\u{301}"));

  assert_eq!(
    driver.probe("value"),
    "e\u{301}",
    "the mark lands after its base"
  );
  assert_eq!(
    driver.probe("selection"),
    "1..1",
    "the caret sits after the one cluster they make"
  );
}

fn focus_shows_after_a_key<D: Driver>(driver: &mut D) {
  let [one, two] = ["One", "Two"].map(|name| found(driver, Role::Button, name));

  driver.click(&one);

  assert_eq!(driver.focused().as_ref(), Some(&one), "a click focuses it");
  assert!(!driver.focus_shows(), "focus a press reached does not show");

  driver.press(&key("Tab"));

  assert_eq!(driver.focused().as_ref(), Some(&two));
  assert!(driver.focus_shows(), "focus a key moved shows");

  driver.click(&one);

  assert!(!driver.focus_shows(), "the next press hides it again");
}

fn a_text_entry_shows_its_focus<D: Driver>(driver: &mut D) {
  let field = found(driver, Role::TextInput, "Name");

  driver.click(&field);

  assert_eq!(
    driver.focused().as_ref(),
    Some(&field),
    "a click focuses it"
  );
  assert!(
    driver.focus_shows(),
    "a text entry shows focus a press reached"
  );
}

fn a_press_holds_along_its_path<D: Driver>(driver: &mut D) {
  let [outer, inner] = ["Outer", "Inner"].map(|name| found(driver, Role::Group, name));

  assert!(
    !driver.is_pressed(&outer) && !driver.is_pressed(&inner),
    "nothing holds a press at first"
  );

  driver.hold(&inner);

  assert!(driver.is_pressed(&inner), "the node pressed holds it");
  assert!(driver.is_pressed(&outer), "so does the box around it");

  driver.release(&inner);

  assert!(
    !driver.is_pressed(&outer) && !driver.is_pressed(&inner),
    "the release ends it along the path"
  );
}

fn a_box_hears_focus_entering_and_leaving<D: Driver>(driver: &mut D) {
  let probes = |driver: &mut D| (driver.probe("entered"), driver.probe("left"));

  driver.press(&key("Tab"));

  assert_eq!(
    probes(driver),
    ("1".to_owned(), "0".to_owned()),
    "focus entering the box"
  );

  driver.press(&key("Tab"));

  assert_eq!(
    probes(driver),
    ("1".to_owned(), "0".to_owned()),
    "focus moving within it is nothing to the box"
  );

  driver.press(&key("Tab"));

  assert_eq!(
    probes(driver),
    ("1".to_owned(), "1".to_owned()),
    "focus leaving the box"
  );
}
