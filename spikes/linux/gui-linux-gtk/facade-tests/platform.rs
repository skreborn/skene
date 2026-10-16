//! The Linux platform's own controls, read and acted on as a reader and a user do
//! (`skene_gui::testing`): what the conformance suite, written with the shared views alone,
//! does not reach.

use skene_gui::{
  platform::linux::{CheckButton, DropDown},
  prelude::*,
  testing::{self, Role},
};

#[skene_gui::test]
fn a_check_button_is_a_checkbox_a_click_checks() {
  let done = Signal::new(false);
  let screen = testing::render(move || {
    Column::new()
      .set_padding(16)
      .add_child(CheckButton::new().set_caption("Remember me").bind_on(done))
  });
  let check = screen.find_by_role(Role::Checkbox, "Remember me");

  assert!(!check.is_on());
  check.click();
  assert!(check.is_on(), "a reader reads the check");
  assert!(done.get_untracked());
}

#[skene_gui::test]
fn a_mixed_check_button_reads_as_mixed() {
  let screen = testing::render(move || {
    Column::new()
      .set_padding(16)
      .add_child(CheckButton::new().set_caption("All").set_mixed(true))
  });

  assert_eq!(
    screen.find_by_role(Role::Checkbox, "All").read().checked,
    Some(testing::Checked::Mixed)
  );
}

#[skene_gui::test]
fn a_dropdown_is_chosen_by_its_names() {
  let chosen = Signal::new(Some(1_u32));
  let screen = testing::render(move || {
    Column::new().set_padding(16).add_child(
      DropDown::new()
        .set_options(["Small", "Large"])
        .bind_chosen(chosen)
        .set_accessible_name("Size"),
    )
  });

  assert_eq!(screen.find_by_role(Role::ComboBox, "Size").name(), "Size");
  chosen.set(Some(0));
  screen.settle();
  assert_eq!(chosen.get_untracked(), Some(0));
}
