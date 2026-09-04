//! Server rendering of the text field: the label over its native input, the void-tag
//! form, value and placeholder serialization, styling, and inert handlers.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene_gui::{
  Condition,
  Container,
  FocusOrder,
  HasChildren,
  Inset,
  RawTextField,
  Row,
  State,
  Text,
  TextEntry,
  When,
  render_to_string,
};

/// The class of the first classed element in `body`.
fn first_class(body: &str) -> &str {
  let start = body.find("class=\"").expect("a class") + 7;
  let end = body[start..].find('"').expect("a closing quote") + start;

  &body[start..end]
}

/// A text field is a label over its native input, and the input serializes as a void
/// element: no closing tag.
#[test]
fn input_is_a_void_element_inside_its_label() {
  let rendered = render_to_string(|| RawTextField::new(TextEntry::new()));
  let body = rendered.body();

  assert!(body.starts_with("<label "), "{body}");
  assert!(body.contains("<input "), "{body}");
  assert!(body.contains("type=\"text\""), "{body}");
  assert!(!body.contains("</input>"), "{body}");
  assert!(body.ends_with("</label>"), "{body}");
}

/// The placeholder and value serialize as attributes, escaped.
#[test]
fn placeholder_and_value_serialize_as_attributes() {
  let rendered = render_to_string(|| {
    RawTextField::new(
      TextEntry::new()
        .set_placeholder("Type \"here\"")
        .set_value("a & b"),
    )
  });

  assert!(
    rendered
      .body()
      .contains("placeholder=\"Type &quot;here&quot;\"")
  );

  assert!(rendered.body().contains("value=\"a &amp; b\""));
}

/// A text field's styling interns like a box's, its focus variant anchored on the
/// native input.
#[test]
fn input_styling_interns_with_variants() {
  let rendered = render_to_string(|| {
    RawTextField::new(TextEntry::new())
      .set_padding(Inset::all(8.))
      .set_corner_radius(0f32.when(Condition::from(State::Focused), 4.))
  });

  let body = rendered.body();
  let class = first_class(body);
  let styles = rendered.styles();
  let base = styles.find(&format!(".{class}{{")).expect("the base rule");
  let base = &styles[base..styles[base..].find('}').expect("rule end") + base];

  assert!(base.contains("padding:8px 8px 8px 8px;"), "{styles}");
  assert!(
    styles.contains(&format!(":where(:is(input,textarea):focus) ~ .{class}"))
      && styles.contains("{border-radius:4px;}"),
    "the focus variant anchors on the input: {styles}",
  );
}

/// The focus order serializes as `tabindex` on the native input; positions count from
/// 0, so the first position maps to `tabindex="1"`.
#[test]
fn focus_attributes_serialize() {
  let rendered = render_to_string(|| {
    RawTextField::new(TextEntry::new()).set_focus_order(FocusOrder::Position(0))
  });

  let body = rendered.body();
  let input = body.find("<input").expect("the input");

  assert!(body[input..].contains("tabindex=\"1\""), "{body}");
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
      .set_child(Row::new().set_children((
        Text::new("t"),
        RawTextField::new(TextEntry::new().handle_change(|_| {})).handle_focus(|| {}),
      )))
  });

  let plain = render_to_string(|| {
    Container::new()
      .set_child(Row::new().set_children((Text::new("t"), RawTextField::new(TextEntry::new()))))
  });

  assert_eq!(rendered.body(), plain.body());
}
