//! Server rendering of the input control: markup, void-tag form, value and placeholder
//! serialization, styling, and inert handlers.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene::{
  Container,
  HasChildren,
  Input,
  Text,
  core::{BoxStyle, Condition},
  render_to_string,
};

/// An input serializes as a void element: no closing tag.
#[test]
fn input_is_a_void_element() {
  let rendered = render_to_string(Input::new);

  assert!(rendered.body().starts_with("<input "));
  assert!(!rendered.body().contains("</input>"));
}

/// The placeholder and value serialize as attributes, escaped.
#[test]
fn placeholder_and_value_serialize_as_attributes() {
  let rendered = render_to_string(|| {
    Input::new()
      .set_placeholder("Type \"here\"")
      .set_value("a & b")
  });

  assert!(
    rendered
      .body()
      .contains("placeholder=\"Type &quot;here&quot;\"")
  );
  assert!(rendered.body().contains("value=\"a &amp; b\""));
}

/// Input styling interns like a box's, including focus variants.
#[test]
fn input_styling_interns_with_variants() {
  let rendered = render_to_string(|| {
    Input::new()
      .set_style(BoxStyle::new().set_padding(skene::Inset::all(8.)))
      .set_when(Condition::Focused, BoxStyle::new().set_border_radius(4.))
  });

  let body = rendered.body();
  let start = body.find("class=\"").expect("a class") + 7;
  let end = body[start..].find('"').expect("a closing quote") + start;
  let class = &body[start..end];

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}{{padding:8px 8px 8px 8px;}}"))
  );
  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}:focus{{border-radius:4px;}}"))
  );
}

/// Handlers are inert on the server: registering them changes nothing in the output.
#[test]
fn handlers_are_inert_on_the_server() {
  let rendered = render_to_string(|| {
    Container::new()
      .handle_pointer_enter(|| {})
      .handle_key_down(|_| {})
      .set_children((
        Text::new("t"),
        Input::new().handle_input(|_| {}).handle_focus(|| {}),
      ))
  });
  let plain = render_to_string(|| Container::new().set_children((Text::new("t"), Input::new())));

  assert_eq!(rendered.body(), plain.body());
}
