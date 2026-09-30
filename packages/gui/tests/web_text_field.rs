//! Server rendering of the text field: the input as the control's own element, the
//! void-tag form, value and placeholder serialization, styling, and inert handlers.

#![cfg(skene_leaf = "server")]

use skene_gui::{
  Condition,
  Container,
  FocusOrder,
  HasChild,
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

/// A text field is its native input itself, which serializes as a void element: no
/// closing tag and no wrapper.
#[test]
fn input_is_the_control_and_a_void_element() {
  let rendered = render_to_string(|| RawTextField::new(TextEntry::new()));
  let body = rendered.body();

  assert!(body.starts_with("<input "), "{body}");
  assert!(body.contains("type=\"text\""), "{body}");
  assert!(body.contains("data-skene-control"), "{body}");
  assert!(!body.contains("</input>"), "{body}");
  assert!(!body.contains("label"), "{body}");
  assert!(body.ends_with('>'), "{body}");
}

/// A caption is a `<label for>` serialized before the input among its siblings, the
/// input carrying the id it names, and the streamed render agrees byte for byte.
#[test]
fn a_caption_is_a_label_before_the_input() {
  fn app() -> impl skene_gui::IntoView {
    Container::new().set_child(RawTextField::new(TextEntry::new()).set_label("Name"))
  }

  let rendered = render_to_string(app);
  let body = rendered.body();
  let label = body.find("<label ").expect("a label");
  let input = body.find("<input ").expect("an input");
  let id = body[input..]
    .split("id=\"")
    .nth(1)
    .and_then(|rest| rest.split('"').next())
    .expect("the input's id");

  assert!(label < input, "the label precedes the input: {body}");
  assert!(
    body[label..input].contains(&format!(" for=\"{id}\"")),
    "the label names the input's id: {body}"
  );
  assert!(body.contains(">Name</span></label><input"), "{body}");
  assert!(!body.contains("</input>"), "{body}");

  let streamed = skene_gui::render_to_stream(app).chunks().concat();

  assert!(
    streamed.contains(body),
    "the streamed render places the label the same: {streamed}"
  );
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

/// A text field's styling interns like a box's, its focus variant observed on the
/// input itself.
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
    styles.contains(&format!(
      ".{class}:where(:is(:focus,:has(> input.skene-native:focus)))"
    )) && styles.contains("{border-radius:4px;}"),
    "the focus variant is the input's own: {styles}",
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
