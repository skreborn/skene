//! Accessible names on the host platform: every named node, by the accessible-name
//! computation's order.

use skene_common::{Activate, Painted, TextEntry};
use skene_core::{HasChild, HasChildren, Role, View, Visibility};
use skene_headless::{Headless, accessible_name, reset};
use skene_reactive::Owner;

type Container = skene_common::Container<Headless>;
type Row = skene_common::Row<Headless>;
type Text = skene_common::Text<Headless>;
type RawButton = skene_common::RawButton<Headless>;
type RawTextField = skene_common::RawTextField<Headless>;

/// A region, a dialog-like box and a picture are named by their label or by the node
/// they are labelled by; a box of no role reads nothing from its content.
#[test]
fn a_box_is_named_by_its_label_or_its_heading() {
  reset();

  let owner = Owner::new();
  let (labelled, headed, picture, plain) = owner.enter(|| {
    let heading = Text::new("Results").build();

    (
      Container::new()
        .set_role(Role::Region)
        .set_label("Filters")
        .build(),
      Container::new()
        .set_role(Role::Region)
        .set_labelled_by(heading)
        .set_child(Text::new("ignored"))
        .build(),
      Painted::<Headless>::new(|_, _| {})
        .set_description("Sales chart")
        .build(),
      Container::new().set_child(Text::new("content")).build(),
    )
  });

  assert_eq!(accessible_name(labelled).as_deref(), Some("Filters"));
  assert_eq!(accessible_name(headed).as_deref(), Some("Results"));
  assert_eq!(accessible_name(picture).as_deref(), Some("Sales chart"));
  assert_eq!(accessible_name(plain), None);

  owner.dispose();
}

/// A button, a heading and a link take their names from their content: each box's text
/// set apart, hidden content left out.
#[test]
fn a_button_a_heading_and_a_link_are_named_by_their_content() {
  reset();

  let owner = Owner::new();
  let (button, heading, link) = owner.enter(|| {
    (
      RawButton::new(Activate::new())
        .set_child(
          Row::new().set_children((
            Text::new("Save"),
            Text::new("all"),
            Container::new()
              .set_visibility(Visibility::Hidden)
              .set_child(Text::new("drafts")),
          )),
        )
        .build(),
      Container::new()
        .set_role(Role::Heading)
        .set_child(Text::new("  Getting   started "))
        .build(),
      Text::new("Documentation").set_link("/docs").build(),
    )
  });

  assert_eq!(accessible_name(button).as_deref(), Some("Save all"));
  assert_eq!(accessible_name(heading).as_deref(), Some("Getting started"));
  assert_eq!(accessible_name(link).as_deref(), Some("Documentation"));

  owner.dispose();
}

/// A text entry within the content a name is taken from contributes its value, as an
/// embedded control does.
#[test]
fn an_embedded_entry_contributes_its_value() {
  reset();

  let owner = Owner::new();
  let named = owner.enter(|| {
    let phrase = Row::new()
      .set_children((
        Text::new("Deliver in"),
        RawTextField::new(TextEntry::new().set_value("3")),
        Text::new("days"),
      ))
      .build();

    Container::new()
      .set_role(Role::Group)
      .set_labelled_by(phrase)
      .build()
  });

  assert_eq!(accessible_name(named).as_deref(), Some("Deliver in 3 days"));

  owner.dispose();
}
