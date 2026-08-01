//! Server rendering of the basic controls: semantic markup, state serialization, and
//! the expanded role vocabulary.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene_gui::{
  Checkbox,
  Container,
  HasChildren,
  Radio,
  Role,
  Select,
  Slider,
  Text,
  Textarea,
  render_to_string,
};

/// A checkbox serializes as a checkbox input, with its state as the `checked` attribute.
#[test]
fn checkbox_serializes_with_its_state() {
  let rendered = render_to_string(|| Checkbox::new().set_checked(true));

  assert!(rendered.body().contains("type=\"checkbox\""));
  assert!(rendered.body().contains("checked"));

  let unchecked = render_to_string(|| Checkbox::new().set_checked(false));

  assert!(!unchecked.body().contains("checked"));
}

/// A radio serializes with its group, value, and state.
#[test]
fn radio_serializes_group_value_and_state() {
  let rendered = render_to_string(|| {
    Container::new().set_children((
      Radio::new("color", "red").set_checked(true),
      Radio::new("color", "green"),
    ))
  });

  let body = rendered.body();

  assert_eq!(body.matches("type=\"radio\"").count(), 2);
  assert_eq!(body.matches("name=\"color\"").count(), 2);
  assert!(body.contains("value=\"red\""));
  assert!(body.contains("value=\"green\""));
  assert_eq!(body.matches("checked").count(), 1);
}

/// A select serializes its options in order and marks the chosen one selected.
#[test]
fn select_serializes_options_and_selection() {
  let rendered = render_to_string(|| {
    Select::new()
      .set_options([("a", "First"), ("b", "Second")])
      .set_value("b")
  });

  let body = rendered.body();

  assert!(body.contains("<select"));
  assert!(body.contains(">First</option>"));
  assert!(body.contains(">Second</option>"));

  let second = body.find("value=\"b\"").expect("the second option");

  assert!(body[second..body[second..].find('>').unwrap() + second].contains("selected"));
  assert!(!body[..second].contains("selected"));
}

/// A textarea serializes its value as text content, not an attribute.
#[test]
fn textarea_serializes_value_as_content() {
  let rendered = render_to_string(|| {
    Textarea::new()
      .set_placeholder("Notes")
      .set_rows(4)
      .set_value("draft text")
  });

  let body = rendered.body();

  assert!(body.contains("placeholder=\"Notes\""));
  assert!(body.contains("rows=\"4\""));
  assert!(body.contains(">draft text</textarea>"));
  assert!(!body.contains("value=\"draft text\""));
}

/// A slider serializes as a range input with its bounds, step, and value.
#[test]
fn slider_serializes_range_and_value() {
  let rendered = render_to_string(|| Slider::new(0., 100.).set_step(5.).set_value(40.));

  let body = rendered.body();

  assert!(body.contains("type=\"range\""));
  assert!(body.contains("min=\"0\""));
  assert!(body.contains("max=\"100\""));
  assert!(body.contains("step=\"5\""));
  assert!(body.contains("value=\"40\""));
}

/// Roles realize as their semantic elements, headings by level.
#[test]
fn roles_realize_semantic_elements() {
  let rendered = render_to_string(|| {
    Container::new().set_role(Role::Navigation).set_children((
      Container::new()
        .set_role(Role::Heading(2))
        .set_child(Text::new("Title")),
      Container::new()
        .set_role(Role::List)
        .set_child(Container::new().set_role(Role::ListItem)),
    ))
  });

  let body = rendered.body();

  assert!(body.starts_with("<nav"));
  assert!(body.contains("<h2"));
  assert!(body.contains("<ul"));
  assert!(body.contains("<li"));
  assert!(body.ends_with("</nav>"));
}
