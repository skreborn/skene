//! Server rendering of the input control: markup, void-tag form, value and placeholder
//! serialization, styling, and inert handlers.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene_gui::{
  Container,
  FocusOrder,
  HasChildren,
  RawTextField,
  Row,
  Text,
  core::{BoxStyle, Condition},
  render_to_string,
};
// The csr and ssr halves each use a different subset of the surface traits.
#[allow(unused_imports)]
use skene_gui::{HasEvents, HasFocus, HasStyle};

/// An input serializes as a void element: no closing tag.
#[test]
fn input_is_a_void_element() {
  let rendered = render_to_string(RawTextField::new);

  assert!(rendered.body().starts_with("<input "));
  assert!(!rendered.body().contains("</input>"));
}

/// The placeholder and value serialize as attributes, escaped.
#[test]
fn placeholder_and_value_serialize_as_attributes() {
  let rendered = render_to_string(|| {
    RawTextField::new()
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

/// `RawTextField` styling interns like a box's, including focus variants.
#[test]
fn input_styling_interns_with_variants() {
  let rendered = render_to_string(|| {
    RawTextField::new()
      .set_style(BoxStyle::new().set_padding(skene_gui::Inset::all(8.)))
      .set_when(Condition::Focused, BoxStyle::new().set_border_radius(4.))
  });

  let body = rendered.body();
  let start = body.find("class=\"").expect("a class") + 7;
  let end = body[start..].find('"').expect("a closing quote") + start;
  let class = &body[start..end];

  assert!(rendered.styles().contains(&format!(
    ".{class}{{appearance:none;background:none;border:none;margin:0;padding:0;\
     font:inherit;color:inherit;padding:8px 8px 8px 8px;}}"
  )));

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}:focus{{border-radius:4px;}}"))
  );
}

/// The focus order and autofocus serialize as attributes; positions count from 0, so
/// the first position maps to `tabindex="1"`.
#[test]
fn focus_attributes_serialize() {
  let rendered = render_to_string(|| {
    RawTextField::new()
      .set_focus_order(FocusOrder::Position(0))
      .set_autofocus()
  });

  assert!(rendered.body().contains("tabindex=\"1\""));
  assert!(rendered.body().contains("autofocus"));
}

/// Handlers are inert on the server: registering them changes nothing in the output.
#[test]
fn handlers_are_inert_on_the_server() {
  let rendered = render_to_string(|| {
    Container::new()
      .handle_pointer_enter(|| {})
      .handle_key_down(|_| {})
      .handle_pointer_down(|_| {})
      .handle_pointer_up(|_| {})
      .handle_pointer_move(|_| {})
      .handle_scroll(|_| {})
      .set_child(
        Row::new().set_children((
          Text::new("t"),
          RawTextField::new()
            .handle_change(|_| {})
            .handle_focus(|| {}),
        )),
      )
  });

  let plain = render_to_string(|| {
    Container::new().set_child(Row::new().set_children((Text::new("t"), RawTextField::new())))
  });

  assert_eq!(rendered.body(), plain.body());
}
