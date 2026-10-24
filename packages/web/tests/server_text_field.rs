//! Server rendering of the text field: the input as the control's own element, the
//! void-tag form, value and placeholder serialization, the secure flag and the keyboard,
//! styling, and inert handlers.

#![cfg(all(skene_platform = "web", skene_server))]

use skene_web::{
  Autofill,
  BasicTextEditor,
  BasicTextField,
  Condition,
  Container,
  ContainerBase,
  ControlBase,
  DecorationBase,
  DecorationWithCornerRadius,
  FocusOrder,
  HasChild,
  HasChildren,
  Inset,
  Keyboard,
  Row,
  RowBase,
  State,
  SubmitLabel,
  Text,
  TextBase,
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
  let rendered = render_to_string(|| BasicTextField::new(TextEntry::new()));
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
  fn app() -> impl skene_web::View {
    Container::new().set_child(BasicTextField::new(TextEntry::new()).set_label("Name"))
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

  let streamed = skene_web::render_to_stream(app).chunks().concat();

  assert!(
    streamed.contains(body),
    "the streamed render places the label the same: {streamed}"
  );
}

/// The placeholder and value serialize as attributes, escaped.
#[test]
fn placeholder_and_value_serialize_as_attributes() {
  let rendered = render_to_string(|| {
    BasicTextField::new(
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

/// A secure field renders as a password input, its value kept, and an editor, which is
/// never secure, renders as a plain `<textarea>`.
#[test]
fn a_secure_field_renders_as_a_password_input() {
  let rendered = render_to_string(|| {
    Container::new().set_child(Row::new().set_children((
      BasicTextField::new(TextEntry::new().set_secure(true).set_value("hunter2")),
      BasicTextEditor::new(TextEntry::new().set_lines(3).set_secure(true)),
    )))
  });
  let body = rendered.body();

  assert!(body.contains("type=\"password\""), "{body}");
  assert!(!body.contains("type=\"text\""), "{body}");
  assert!(body.contains("value=\"hunter2\""), "{body}");
  assert!(body.contains("<textarea "), "{body}");
  assert_eq!(body.matches("type=").count(), 1, "{body}");
}

/// A keyboard renders as the attributes its parts name, a number's as `inputmode` over a
/// text input and a URL's as its own input type, and a part left to the platform renders
/// nothing.
#[test]
fn a_keyboard_renders_its_input_mode_and_autofill() {
  let number = render_to_string(|| {
    BasicTextField::new(
      TextEntry::new().set_keyboard(Keyboard::NUMBER.with_autofill(Autofill::OneTimeCode)),
    )
  });
  let body = number.body();

  assert!(body.contains("type=\"text\""), "{body}");
  assert!(body.contains("inputmode=\"numeric\""), "{body}");
  assert!(body.contains("autocomplete=\"one-time-code\""), "{body}");

  for absent in [
    "autocorrect",
    "spellcheck",
    "autocapitalize",
    "enterkeyhint",
  ] {
    assert!(!body.contains(absent), "{absent}: {body}");
  }

  let url = render_to_string(|| {
    BasicTextField::new(
      TextEntry::new()
        .set_keyboard(
          Keyboard::URL
            .with_autocorrect(false)
            .with_submit(SubmitLabel::Go),
        )
        .set_secure(false),
    )
  });
  let body = url.body();

  assert!(body.contains("type=\"url\""), "{body}");
  assert!(body.contains("inputmode=\"url\""), "{body}");
  assert!(body.contains("autocorrect=\"off\""), "{body}");
  assert!(body.contains("spellcheck=\"false\""), "{body}");
  assert!(body.contains("enterkeyhint=\"go\""), "{body}");
  assert!(!body.contains("autocomplete"), "{body}");

  let login = render_to_string(|| {
    BasicTextField::new(
      TextEntry::new()
        .set_secure(true)
        .set_keyboard(Keyboard::TEXT.with_autofill(Autofill::Password)),
    )
  });
  let body = login.body();

  assert!(
    body.contains("type=\"password\""),
    "the secure flag wins: {body}"
  );
  assert!(body.contains("autocomplete=\"current-password\""), "{body}");
  assert!(!body.contains("inputmode"), "{body}");
}

/// A text field's styling interns like a box's, its focus variant observed on the
/// input itself.
#[test]
fn input_styling_interns_with_variants() {
  let rendered = render_to_string(|| {
    BasicTextField::new(TextEntry::new())
      .set_padding(Inset::all(8))
      .set_corner_radius(0.when(Condition::from(State::Focused), 4))
  });

  let body = rendered.body();
  let class = first_class(body);
  let styles = rendered.styles();
  let base = styles.find(&format!(".{class}{{")).expect("the base rule");
  let base = &styles[base..styles[base..].find('}').expect("rule end") + base];

  assert!(
    base.contains("padding-block:8px 8px;padding-inline:8px 8px;"),
    "{styles}"
  );
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
    BasicTextField::new(TextEntry::new()).set_focus_order(FocusOrder::Position(0))
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
        BasicTextField::new(TextEntry::new().handle_change(|_| {})).handle_focus(|| {}),
      )))
  });

  let plain = render_to_string(|| {
    Container::new()
      .set_child(Row::new().set_children((Text::new("t"), BasicTextField::new(TextEntry::new()))))
  });

  assert_eq!(rendered.body(), plain.body());
}
