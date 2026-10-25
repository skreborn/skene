//! Accessible names in the terminal, as a reader finds them: every named node, by the
//! accessible-name computation's order.

use skene_terminal::{
  Label,
  Painted,
  behavior::{Activate, TextEntry, Toggle},
  prelude::*,
  testing,
};

use crate::common::{mount_nodes, read, with_platform};

/// A region and a picture are named by their label or by the node they are labelled
/// by; a box of no role reads nothing from its content.
#[test]
fn a_box_is_named_by_its_label_or_its_heading() {
  with_platform(|| {
    let [heading, labelled, headed, picture, plain] = mount_nodes(|| {
      let heading = Text::new("Results").build();

      [
        heading,
        Container::new()
          .set_role(Role::Region)
          .set_label("Filters")
          .build(),
        Container::new()
          .set_role(Role::Region)
          .set_labelled_by(heading)
          .set_child(Text::new("ignored"))
          .build(),
        Painted::new(|_, _| {})
          .set_height(1)
          .set_description("Sales chart")
          .build(),
        Container::new().set_child(Text::new("content")).build(),
      ]
    });

    assert_eq!(
      testing::accessible_name(labelled).as_deref(),
      Some("Filters")
    );
    assert_eq!(testing::accessible_name(headed).as_deref(), Some("Results"));
    assert_eq!(read(headed).labelled_by(), [heading.access_id()]);
    assert_eq!(
      testing::accessible_name(picture).as_deref(),
      Some("Sales chart")
    );
    assert_eq!(testing::accessible_name(plain), None);
  });
}

/// A button, a heading and a link take their names from their content: each box's
/// text set apart, hidden content left out.
#[test]
fn a_button_a_heading_and_a_link_are_named_by_their_content() {
  with_platform(|| {
    let [button, heading, link] = mount_nodes(|| {
      [
        BasicButton::new(Activate::new())
          .set_child(
            Row::new().set_children((
              Text::new("Save"),
              Text::new("all"),
              Container::new()
                .set_hidden(true)
                .set_child(Text::new("drafts")),
            )),
          )
          .build(),
        Container::new()
          .set_role(Role::Heading)
          .set_child(Text::new("  Getting   started "))
          .build(),
        Text::new("Documentation").set_link("/docs").build(),
      ]
    });

    assert_eq!(
      testing::accessible_name(button).as_deref(),
      Some("Save all")
    );
    assert_eq!(
      testing::find_by_role(Role::Button, "Save all"),
      Some(button)
    );
    assert_eq!(
      testing::accessible_name(heading).as_deref(),
      Some("Getting started")
    );
    assert_eq!(
      testing::accessible_name(link).as_deref(),
      Some("Documentation")
    );
  });
}

/// A text entry within the content a name is taken from contributes its value, as an
/// embedded control does.
#[test]
fn an_embedded_entry_contributes_its_value() {
  with_platform(|| {
    let [phrase, named] = mount_nodes(|| {
      let phrase = Row::new()
        .set_children((
          Text::new("Deliver in"),
          BasicTextField::new(TextEntry::new().set_value("3")).set_width(2),
          Text::new("days"),
        ))
        .build();

      [
        phrase,
        Container::new()
          .set_role(Role::Group)
          .set_labelled_by(phrase)
          .build(),
      ]
    });

    assert_eq!(
      testing::accessible_name(named).as_deref(),
      Some("Deliver in 3 days")
    );
    assert_eq!(read(named).labelled_by(), [phrase.access_id()]);
  });
}

/// A label part supplies the accessible name: a string names the control at build and
/// draws as its caption, and any view's text is read from the part.
#[test]
fn a_label_part_supplies_the_accessible_name() {
  with_platform(|| {
    let [named, drawn, unnamed] = mount_nodes(|| {
      [
        BasicButton::new(Activate::new()).set_label("Save").build(),
        BasicCheckbox::new(Toggle::new())
          .set_label(Label::new(Text::new("Remember me")))
          .build(),
        BasicButton::new(Activate::new()).build(),
      ]
    });

    assert!(
      testing::render_to_text(20, 3).starts_with("Save"),
      "the label draws as the caption"
    );
    assert_eq!(testing::accessible_name(named).as_deref(), Some("Save"));
    assert_eq!(
      testing::accessible_name(drawn).as_deref(),
      Some("Remember me")
    );
    assert_eq!(testing::accessible_name(unnamed), None);
  });
}

/// A caption placed anywhere names a control by reference, after the caption its own
/// label builds.
#[test]
fn a_caption_placed_anywhere_names_a_control() {
  with_platform(|| {
    let [caption, own, field] = mount_nodes(|| {
      let caption = Text::new("Email").build();
      let own = Text::new("Work").build();
      let field = BasicTextField::new(TextEntry::new())
        .set_label(Label::new(AnyView::node(own)))
        .set_labelled_by(caption)
        .build();

      [caption, own, field]
    });

    assert_eq!(
      read(field).labelled_by(),
      [own.access_id(), caption.access_id()]
    );
    assert_eq!(
      testing::accessible_name(field).as_deref(),
      Some("Work Email")
    );
  });
}

/// An accessible name names a control no caption does and outranks a caption's, and a
/// derived one follows its signals.
#[test]
fn an_accessible_name_names_a_control_over_its_caption() {
  with_platform(|| {
    let name = Signal::new("Close".to_owned());
    let [icon, derived, captioned] = mount_nodes(|| {
      [
        BasicButton::new(Activate::new())
          .set_accessible_name("Close")
          .set_child(Container::new().set_width(1).set_height(1))
          .build(),
        BasicButton::new(Activate::new())
          .derive_accessible_name(move || name.get())
          .build(),
        BasicButton::new(Activate::new())
          .set_label("×")
          .set_accessible_name("Close")
          .build(),
      ]
    });

    assert_eq!(testing::accessible_name(icon).as_deref(), Some("Close"));
    assert_eq!(read(icon).label(), Some("Close"));
    assert_eq!(testing::accessible_name(derived).as_deref(), Some("Close"));

    name.set("Dismiss".to_owned());

    assert_eq!(
      testing::accessible_name(derived).as_deref(),
      Some("Dismiss")
    );
    assert_eq!(
      testing::accessible_name(captioned).as_deref(),
      Some("Close")
    );
  });
}
