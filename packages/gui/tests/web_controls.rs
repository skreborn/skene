//! Server rendering of the basic controls: semantic markup, state serialization, and
//! the expanded role vocabulary.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene_gui::{
  Container,
  FocusOrder,
  HasChildren,
  RawButton,
  RawCheckbox,
  RawPicker,
  RawRadio,
  RawSlider,
  RawTextEditor,
  RawTextField,
  Role,
  Text,
  TextStyle,
  TextTheme,
  core::{BoxStyle, Color, Condition},
  render_to_string,
};

/// A button serializes as a real button element carrying its reset class, its children
/// as content, and its tab-order attributes.
#[test]
fn button_serializes_as_a_button_element() {
  let rendered = render_to_string(|| {
    RawButton::new()
      .set_focus_order(FocusOrder::Auto)
      .set_child(Text::new("Go"))
      .handle_click(|| {})
  });

  let body = rendered.body();

  assert!(body.starts_with("<button "));
  assert!(body.contains("type=\"button\""));
  assert!(body.contains("tabindex=\"0\""));
  assert!(body.contains(">Go</span>"));
  assert!(body.ends_with("</button>"));
}

/// A checkbox serializes as a checkbox input, with its state as the `checked` attribute.
#[test]
fn checkbox_serializes_with_its_state() {
  let rendered = render_to_string(|| RawCheckbox::new().set_checked(true));

  assert!(rendered.body().contains("type=\"checkbox\""));
  assert!(rendered.body().contains("checked"));

  let unchecked = render_to_string(|| RawCheckbox::new().set_checked(false));

  assert!(!unchecked.body().contains("checked"));
}

/// A radio serializes with its group, value, and state.
#[test]
fn radio_serializes_group_value_and_state() {
  let rendered = render_to_string(|| {
    Container::new().set_children((
      RawRadio::new("color", "red").set_checked(true),
      RawRadio::new("color", "green"),
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
    RawPicker::new()
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
    RawTextEditor::new()
      .set_placeholder("Notes")
      .set_lines(4)
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
  let rendered = render_to_string(|| RawSlider::new(0., 100.).set_step(5.).set_value(40.));

  let body = rendered.body();

  assert!(body.contains("type=\"range\""));
  assert!(body.contains("min=\"0\""));
  assert!(body.contains("max=\"100\""));
  assert!(body.contains("step=\"5\""));
  assert!(body.contains("value=\"40\""));
}

/// Every control carries its own reset class: no native chrome, author styles from
/// zero. A toggle's reset keeps the border its drawn chrome needs, and the select opts
/// into the customizable-select rendering where supported.
#[test]
fn controls_carry_their_reset() {
  let rendered = render_to_string(|| RawButton::new().set_child(Text::new("Go")));

  assert!(rendered.styles().contains(
    "{appearance:none;background:none;border:none;margin:0;padding:0;\
     font:inherit;color:inherit;}"
  ));

  let rendered = render_to_string(RawCheckbox::new);
  let body = rendered.body();
  let start = body.find("class=\"").expect("a class") + 7;
  let end = body[start..].find('"').expect("a closing quote") + start;
  let class = body[start..end].to_owned();

  assert!(rendered.styles().contains(&format!(
    ".{class}{{appearance:none;background:none;border:1px solid currentColor;margin:0;\
     padding:0;font:inherit;color:inherit;}}"
  )));

  let rendered = render_to_string(|| RawPicker::new().set_options([("a", "A")]));

  assert!(
    rendered
      .styles()
      .contains("appearance:none;appearance:base-select;")
  );
}

/// Controls carry the built-in hover and focus feedback—a slight ink wash over the
/// background under a stronger ink mix into the border—each state emitted only while
/// no variant conditions on it; the slider draws its own chrome and carries none.
#[test]
fn controls_carry_interaction_feedback() {
  let rendered = render_to_string(|| RawButton::new().set_child(Text::new("Go")));
  let styles = rendered.styles();

  assert!(styles.contains(
    ":hover{background-image:linear-gradient(color-mix(in srgb, currentColor 6.25%, transparent)"
  ));
  assert!(styles.contains(
    ":focus-visible{background-image:linear-gradient(color-mix(in srgb, currentColor 9.375%, \
     transparent)"
  ));
  assert!(styles.contains(
    ":focus-visible:hover{background-image:linear-gradient(color-mix(in srgb, currentColor \
     12.5%, transparent)"
  ));
  assert!(styles.contains("border-color:color-mix(in srgb, currentColor 25%, transparent);"));

  let authored = render_to_string(|| {
    RawButton::new()
      .set_when(
        Condition::Hovered,
        BoxStyle::new().set_color(Color::srgb(1., 0., 0.)),
      )
      .set_child(Text::new("Go"))
  });
  let styles = authored.styles();

  assert!(!styles.contains(":hover{background-image:"));
  assert!(styles.contains(":focus-visible{background-image:"));

  let slider = render_to_string(|| RawSlider::new(0., 100.));

  assert!(!slider.styles().contains("background-image:"));
}

/// A labeled toggle wraps in a label box: the caption is part of the control, so
/// activating the caption activates the input.
#[test]
fn labeled_toggle_wraps_in_a_label() {
  let rendered = render_to_string(|| RawCheckbox::new().set_label("subscribed"));
  let body = rendered.body();

  assert!(body.starts_with("<label"));
  assert!(body.contains("type=\"checkbox\""));
  assert!(body.contains(">subscribed</span>"));
  assert!(body.ends_with("</label>"));
}

/// A control inherits the surrounding text theme through the framework's own
/// inheritance: the resolved ink and selection pair land in its interned class.
#[test]
fn controls_inherit_the_text_theme() {
  let rendered = render_to_string(|| {
    TextTheme::new(RawTextField::new()).set_base(
      TextStyle::new()
        .set_color(Color::srgb(1., 0., 0.))
        .set_selection_background(Color::srgb(0., 0., 1.)),
    )
  });
  let styles = rendered.styles();

  assert!(styles.contains("color:inherit;"));
  assert!(styles.contains("color:oklch("));
  assert!(styles.contains("::selection{background-color:"));
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
