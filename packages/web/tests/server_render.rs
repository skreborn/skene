//! Server-side rendering: a view tree to HTML, against the real string sink.
//!
//! Run with the server platform selected:
//! `RUSTFLAGS='--cfg skene_platform="server"' cargo test -p skene-web --test server_render`.
#![cfg(skene_web_platform = "server")]

use std::time::Duration;

use skene_foundation::{
  BasicPickerBase,
  ColumnBase,
  ContainerBase,
  ControlBase,
  DecorationBase,
  EnhanceBase,
  Flexibility,
  FlexibleBase,
  MotionBase,
  OverlayBase,
  PaintedBase,
  RichTextBase,
  RowBase,
  RunBase,
  SpanBase,
  TextBase,
  ThemeBase,
  TooltipBase,
  VisibilityBase,
};
use skene_simple_web::prelude::*;
use skene_web::{
  Activate,
  AsyncView,
  AsyncViewBase,
  BasicButton,
  BasicListbox,
  BasicListboxOption,
  BasicSlider,
  Color,
  Column,
  Container,
  Deferred,
  DeferredBase,
  Enhance,
  Flexible,
  HasChild,
  HasChildren,
  Label,
  LinkAnnotation,
  LinkPolicy,
  Listbox,
  ListboxOption,
  Overlay,
  Painted,
  Resource,
  RichText,
  Row,
  Slider,
  SliderRange,
  Span,
  Text,
  Theme,
  Tooltip,
  Typography,
  View,
  client_only,
  derived,
  platform,
  render_page_stream,
  render_to_stream,
  render_to_string,
};

/// The state a page's state script carries, its escaping undone.
fn state_of(page: &skene_web::Rendered) -> String {
  let script = page.state_script();
  let start = script.find('>').expect("the opening tag's end") + 1;
  let end = script.rfind("</script>").expect("the closing tag");

  script[start..end].replace("\\u003c", "<")
}

/// Every class named in `body`, in document order.
fn classes(body: &str) -> Vec<String> {
  body
    .match_indices("class=\"")
    .map(|(start, _)| {
      let rest = &body[start + 7..];

      rest[..rest.find('"').expect("class end")].to_owned()
    })
    .collect()
}

/// The class of the first classed element in `body`.
fn first_class(body: &str) -> String {
  classes(body).into_iter().next().expect("a classed element")
}

/// The base rule of `class` in `styles`: the declarations between its braces.
fn base_rule<'a>(styles: &'a str, class: &str) -> &'a str {
  let start = styles
    .find(&format!(".{class}{{"))
    .expect("the class's base rule")
    + class.len()
    + 2;
  let rest = &styles[start..];

  &rest[..rest.find('}').expect("rule end")]
}

/// A color as the CSS the platform renders it to.
fn css(color: Color) -> String {
  let [lightness, chroma, hue, alpha] = color.to_oklch();

  format!("oklch({lightness} {chroma} {hue} / {alpha})")
}

/// The server carries an enhancement's fallback as markup and never builds its
/// child; a painted box with a fallback renders the fallback in place of the canvas.
#[test]
fn an_enhancement_renders_its_fallback() {
  let page = render_to_string(|| {
    Enhance::new(skene_web::media::hydrated())
      .set_fallback_builder(|| Text::new("baseline"))
      .set_child_builder(|| Text::new("live"))
  });

  assert!(page.body().contains(">baseline</span>"), "{}", page.body());
  assert!(!page.body().contains("live"), "{}", page.body());

  let page =
    render_to_string(|| Painted::new(|_, _| {}).set_fallback_builder(|| Text::new("a picture")));

  assert!(page.body().contains(">a picture</span>"), "{}", page.body());
  assert!(!page.body().contains("<canvas"), "{}", page.body());
}

/// The role vocabulary is AccessKit's, open past the shipped controls: a switch is
/// spelled in ARIA on a generic element, and a heading is the element of the level it
/// reports.
#[test]
fn any_role_reaches_the_markup() {
  use skene_foundation::{AccessProp, Role};

  let page = render_to_string(|| {
    Column::new().set_children((
      Container::new()
        .set_role(Role::Switch)
        .set_child(Text::new("wifi")),
      Container::new()
        .set_role(Role::Heading)
        .set_accessibility([AccessProp::Level(3)])
        .set_child(Text::new("Details")),
    ))
  });
  let body = page.body();

  assert!(body.contains("role=\"switch\""), "{body}");
  assert!(
    body.contains("<h3 ") && body.contains("aria-level=\"3\""),
    "{body}"
  );
}

/// Names and roles reach the markup: a region named by text carries `aria-label`, a
/// control captioned by a view is named by reference to the caption's id, a modal
/// takes the name given it, a status box carries its live role, a painted box with a
/// description is a named image, a tooltip's bubble is a tooltip, and a scrolling box
/// takes a place in the focus order.
#[test]
fn names_and_roles_reach_the_markup() {
  use skene_foundation::{Dimension, Overflow, Role};

  let page = render_to_string(|| {
    Column::new().set_children((
      Container::new()
        .set_role(Role::Navigation)
        .set_label("Chapters")
        .set_child(Text::new("one")),
      BasicListbox::new(Listbox::new())
        .set_label(Label::new(Text::new("Fruit")))
        .set_child(BasicListboxOption::new(ListboxOption::new("apple")).set_label("Apple")),
      Container::new()
        .set_role(Role::Status)
        .set_child(Text::new("saved")),
      Painted::new(|_, _| {}).set_description("Eight blue stripes"),
      Container::new()
        .set_overflow(Overflow::ScrollIfNeeded)
        .set_height(Dimension::Length(40.))
        .set_child(Text::new("long")),
      Tooltip::new("Saves your work").set_child(Text::new("hover me")),
      Overlay::modal()
        .set_label("Settings")
        .set_child(Text::new("inside")),
    ))
  });
  let body = page.body();

  assert!(
    body.contains("<nav ") && body.contains("aria-label=\"Chapters\""),
    "{body}"
  );

  let labelled = body
    .split("aria-labelledby=\"")
    .nth(1)
    .map(|rest| rest[..rest.find('"').expect("id end")].to_owned())
    .expect("a labelled listbox");

  assert!(
    body.contains(&format!("id=\"{labelled}\"")),
    "the caption carries the id: {body}"
  );
  assert!(body.contains("role=\"status\""), "{body}");
  assert!(
    body.contains("role=\"img\"") && body.contains("aria-label=\"Eight blue stripes\""),
    "{body}"
  );
  assert!(body.contains("role=\"tooltip\""), "{body}");
  assert!(
    body.contains("<dialog") && body.contains("aria-label=\"Settings\""),
    "{body}"
  );
  assert!(
    body.matches("tabindex=\"0\"").count() >= 2,
    "the listbox and the scrolling box are tab stops: {body}"
  );
}

/// A control its label names rather than its text keeps the name in the markup.
#[test]
fn a_named_control_keeps_its_accessible_name() {
  let page = render_to_string(|| {
    BasicButton::new(Activate::new()).set_label(Label::new(Text::new("×")).set_name("Close"))
  });
  let body = page.body();

  assert!(body.contains("aria-label=\"Close\""), "{body}");
}

/// A button drawn by an icon alone carries the accessible name it is given.
#[test]
fn an_icon_button_carries_its_accessible_name() {
  let page = render_to_string(|| {
    BasicButton::new(Activate::new())
      .set_accessible_name("Close")
      .set_child(Container::new())
  });
  let body = page.body();

  assert!(body.contains("aria-label=\"Close\""), "{body}");
}

/// A pointer's precision and hover compile to the media features that test them.
#[test]
fn pointer_conditions_compile_to_their_media_features() {
  use skene_foundation::{Media, PointerPrecision, Value};

  let page = render_to_string(|| {
    Container::new()
      .set_background(
        Value::from(Color::srgb(1., 0., 0.))
          .when(
            Media::Pointer(PointerPrecision::Coarse),
            Color::srgb(0., 1., 0.),
          )
          .when(Media::Hover, Color::srgb(0., 0., 1.)),
      )
      .set_child(Text::new("x"))
  });
  let styles = page.styles();

  assert!(styles.contains("(pointer: coarse)"), "{styles}");
  assert!(styles.contains("(hover: hover)"), "{styles}");
}

/// A hidden box compiles to `visibility: hidden`, and a visible one inherits, so it
/// shows only where its ancestors do.
#[test]
fn hiding_compiles_to_hidden_or_inherited_visibility() {
  use skene_foundation::{State, When};

  let page = render_to_string(|| {
    Container::new()
      .set_hidden(true.when(State::Hovered, false))
      .set_child(Text::new("x"))
  });
  let styles = page.styles();

  assert!(styles.contains("visibility:hidden"), "{styles}");
  assert!(styles.contains("visibility:inherit"), "{styles}");
}

/// A slider's scriptless input holds the slider's range, step and value natively.
#[test]
fn a_sliders_input_holds_its_range_natively() {
  let page = render_to_string(|| {
    BasicSlider::new(
      Slider::new()
        .set_range(SliderRange::new(0., 200., 0.))
        .set_value(150.),
    )
  });
  let body = page.body();

  for attribute in ["min=\"0\"", "max=\"200\"", "step=\"any\"", "value=\"150\""] {
    assert!(body.contains(attribute), "{attribute}: {body}");
  }

  assert!(!body.contains("aria-valuenow"), "{body}");
}

/// A radio group's scriptless radios share one `name`, so the browser keeps one of them
/// chosen and submits them as one field; another group's radios share another.
#[test]
fn a_radio_groups_radios_share_one_name() {
  use skene_web::{BasicRadio, BasicRadioGroup, Radio, RadioGroup};

  let group = |values: [&'static str; 2]| {
    BasicRadioGroup::new(RadioGroup::new().set_chosen(values[0]))
      .set_child(Column::new().set_children(values.map(|value| BasicRadio::new(Radio::new(value)))))
  };
  let page = render_to_string(move || {
    Column::new().set_children([group(["s", "m"]), group(["red", "blue"])])
  });
  let body = page.body();
  let names: Vec<&str> = body
    .split(" name=\"")
    .skip(1)
    .map(|rest| &rest[..rest.find('"').expect("the attribute's end")])
    .collect();

  assert_eq!(names.len(), 4, "every radio is named: {body}");
  assert_eq!(names[0], names[1], "one group's radios share it: {body}");
  assert_eq!(names[2], names[3], "so do the other's: {body}");
  assert_ne!(names[0], names[2], "two groups' names differ: {body}");
}

/// A picker's scriptless form is a `<select>` listing its options by value and name
/// with the chosen one selected, and no drawn face.
#[test]
fn a_picker_renders_to_a_select() {
  let page = render_to_string(|| {
    SimplePicker::new()
      .set_options([("s", "Small"), ("m", "Medium")])
      .set_chosen("m")
      .set_label("Size")
  });
  let body = page.body();

  assert!(body.contains("<select"), "{body}");
  assert!(
    body.contains("<option value=\"s\">Small</option><option value=\"m\" selected>Medium</option>"),
    "{body}"
  );
  assert!(
    body.contains(" for=\"skene-"),
    "the caption is a label: {body}"
  );
  assert!(!body.contains("combobox"), "no drawn face: {body}");
}

/// A picker's scriptless `<select>` marks the options that cannot be chosen.
#[test]
fn a_pickers_select_marks_its_disabled_options() {
  use skene_web::{BasicPicker, reactive::Get};

  let page = render_to_string(|| {
    BasicPicker::new()
      .set_chosen("s")
      .set_options([("", "Choose one", true), ("s", "Small", false)].map(
        |(value, name, disabled)| {
          BasicListboxOption::new(ListboxOption::new(value))
            .set_disabled(disabled)
            .set_label(Label::new(Text::new(name)).set_name(name))
        },
      ))
      .set_face(|shown| Text::derive(move || shown.get()))
  });
  let body = page.body();

  assert!(
    body.contains(
      "<option value=\"\" disabled>Choose one</option><option value=\"s\" selected>Small</option>"
    ),
    "{body}"
  );
}

#[test]
fn a_text_renders_to_a_styled_span() {
  let page = render_to_string(|| Text::new("Hello"));

  assert!(
    page.body().starts_with("<span data-h=\"1\" class=\"skene-"),
    "a text is a marked, classed span: {}",
    page.body(),
  );

  assert!(page.body().ends_with(">Hello</span>"), "{}", page.body());
  assert!(
    page.styles().contains("display:block"),
    "the span's class rule is emitted: {}",
    page.styles(),
  );
}

#[test]
fn a_container_nests_its_child() {
  let page = render_to_string(|| Container::new().set_child(Text::new("Hi")));
  let body = page.body();

  assert!(
    body.starts_with("<div data-h="),
    "a container is a div: {body}"
  );

  assert!(
    body.contains("<span") && body.contains(">Hi</span>"),
    "the child span nests inside: {body}",
  );

  assert!(
    body.ends_with("</div>"),
    "the container closes after its child: {body}",
  );
}

#[test]
fn identical_styles_share_one_rule() {
  // Two siblings styled the same must share a class and emit its rule exactly once.
  let page = render_to_string(|| Row::new().set_children([Text::new("a"), Text::new("b")]));
  let body = page.body();
  // The shared class is on the sibling spans, not the container that wraps them.
  let span = body.find("<span").expect("a span");
  let class = first_class(&body[span..]);

  assert_eq!(
    body.matches(&class).count(),
    2,
    "both spans carry the shared class: {body}",
  );

  assert_eq!(
    page.styles().matches(&format!(".{class}{{")).count(),
    1,
    "the shared rule is emitted once: {}",
    page.styles(),
  );
}

#[test]
fn text_and_attributes_are_escaped() {
  let page = render_to_string(|| Text::new("a<b>&\"c"));

  assert!(
    page.body().contains("a&lt;b&gt;&amp;\"c"),
    "text content is HTML-escaped (quotes need no escape in text): {}",
    page.body(),
  );
}

/// The language an app sets is its page's root `lang`, in a render and in a stream.
#[test]
fn a_page_carries_its_language() {
  let app = || {
    skene_web::head::set_language("pt-BR");

    Text::new("olá")
  };

  assert!(
    render_to_string(app)
      .page()
      .starts_with("<!DOCTYPE html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\">")
  );
  assert!(
    render_to_stream(app)
      .document()
      .starts_with("<!DOCTYPE html><html lang=\"pt-BR\"><head>")
  );
}

/// A wrapper's per-child properties reach a text it wraps, compiled into the run's
/// class as into a box's.
#[test]
fn a_wrapper_lays_out_the_text_it_wraps() {
  let page = render_to_string(|| {
    Row::new().set_children((
      Flexible::new()
        .set_flexibility(Flexibility::grow(1.))
        .set_child(Text::new("title")),
      Text::new("x"),
    ))
  });
  let body = page.body();
  let title = body.find(">title<").expect("the title");
  let class = &body[..title];
  let class = &class[class.rfind("class=\"").expect("the title's class") + 7..];
  let class = &class[..class.find('"').expect("the class's end")];

  let styles = page.styles();
  let rule = &styles[styles
    .find(&format!(".{class}{{"))
    .expect("the title's rule")..];
  let rule = &rule[..rule.find('}').expect("the rule's end")];

  assert!(rule.contains("flex:1 1 auto;"), "{rule}");
}

/// A link whose destination would run script is written with no `href`.
#[test]
fn a_script_destination_is_never_followed() {
  let page = render_to_string(|| Text::new("site").set_link(" JavaScript:alert(1)"));

  assert!(!page.body().contains("href"), "{}", page.body());
  assert!(page.body().contains(">site<"), "{}", page.body());
}

/// A link opens as its policy says: the default keeps an external page from reaching
/// back to ours or learning where the reader came from, and a policy rewrites what it
/// changes, on a text and a span alike.
#[test]
fn a_link_opens_as_its_policy_says() {
  let page = render_to_string(|| Text::new("site").set_link("https://example.com"));
  let body = page.body();

  assert!(body.contains("target=\"_blank\""), "{body}");
  assert!(body.contains("rel=\"noopener noreferrer\""), "{body}");

  let page = render_to_string(|| {
    Text::new("site")
      .set_link("https://example.com")
      .set_link_policy(
        LinkPolicy::new()
          .set_new_page(false)
          .set_annotation(LinkAnnotation::Sponsored),
      )
  });
  let body = page.body();

  assert!(!body.contains("target="), "{body}");
  assert!(body.contains("rel=\"noreferrer sponsored\""), "{body}");

  let page = render_to_string(|| {
    RichText::new().add_spans([Span::new("docs")
      .set_link_policy(LinkPolicy::new().set_new_page(true))
      .set_link("/docs")])
  });
  let body = page.body();

  assert!(body.contains("href=\"/docs\" target=\"_blank\""), "{body}");
  assert!(body.contains("rel=\"noopener\""), "{body}");
}

/// A transition name is written as a CSS identifier, so no name ends the rule or the
/// `<style>` it sits in.
#[test]
fn a_transition_name_stays_one_identifier() {
  let page = render_to_string(|| {
    Container::new().set_transition_name("x}</style><img src=x onerror=alert(1)>")
  });
  let styles = page.styles();

  assert!(!styles.contains("</style><img"), "{styles}");
  assert!(
    styles.contains(r"view-transition-name:x\}\<\/style\>\<img\ src\=x\ onerror\=alert\(1\)\>"),
    "{styles}"
  );
}

/// The payoff: an immediately-ready resource settles during the render, so the *loaded*
/// tree serializes, not the placeholder—the SSR fixpoint at work.
#[test]
fn a_resolved_resource_serializes_its_loaded_tree() {
  let page = render_to_string(|| {
    let greeting = Resource::new(|| async { "Ada".to_owned() });

    derived(move || {
      greeting.with(|value| match value {
        None => Text::new("Loading…").build(),
        Some(name) => Container::new().set_child(Text::new(name.clone())).build(),
      })
    })
  });

  assert!(
    page.body().contains(">Ada</span>"),
    "the loaded value is serialized: {}",
    page.body(),
  );

  assert!(
    !page.body().contains("Loading"),
    "the placeholder was replaced before serialization: {}",
    page.body(),
  );
}

/// An *async* load—one that parks on a host operation—settles before serialization: the
/// render parks until the sleep resolves, so the loaded tree
/// serializes.
#[test]
fn an_async_load_settles_before_serialization() {
  let page = render_to_string(|| {
    let greeting = Resource::new(|| async {
      platform::sleep(Duration::from_millis(5)).await;

      "Ada".to_owned()
    });

    derived(move || {
      greeting.with(|value| match value {
        None => Text::new("Loading…").build(),
        Some(name) => Container::new().set_child(Text::new(name.clone())).build(),
      })
    })
  });

  assert!(
    page.body().contains(">Ada</span>"),
    "the async value settled and serialized: {}",
    page.body(),
  );

  assert!(
    !page.body().contains("Loading"),
    "the render blocked on the load, not the placeholder: {}",
    page.body(),
  );
}

/// A list reordered by work the render waits on serializes in its new order, each item
/// once.
#[test]
fn a_list_reordered_before_serialization_serializes_in_its_new_order() {
  let page = render_to_string(|| {
    let order = Resource::new(|| async {
      platform::sleep(Duration::from_millis(1)).await;

      vec![4, 3, 2, 1]
    });

    Column::new().derive_children(
      move || order.with(|order| order.cloned().unwrap_or_else(|| vec![1, 2, 3, 4])),
      |item: &i32| *item,
      |item| Text::new(format!("item {item}")),
    )
  });
  let body = page.body();
  let at = |item: i32| {
    body
      .find(&format!(">item {item}</span>"))
      .expect("every item serializes")
  };

  assert!(at(4) < at(3) && at(3) < at(2) && at(2) < at(1), "{body}");
  assert_eq!(body.matches(">item ").count(), 4, "{body}");
}

/// A complete document assembles the styles into `<head>`, the tree into `<body>`, and
/// the state into the script `launch` reads back.
#[test]
fn page_assembles_a_full_document() {
  let page = render_to_string(|| {
    let _greeting = Resource::new(|| async { "Ada".to_owned() });

    Text::new("hi")
  });

  let document = page.page();

  assert!(
    document.starts_with(
      "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
       content=\"width=device-width, initial-scale=1\"><style id=\"skene-styles\">"
    ),
    "the character set and the viewport open the head, then the styles: {document}",
  );

  assert!(
    document.contains(&format!("</head><body>{}", page.body())),
    "the tree is the body: {document}",
  );

  assert!(
    document
      .contains("<script id=\"skene-state\" type=\"application/json\">[[\"\\\"Ada\\\"\"],[],[]]"),
    "the state is embedded in the script: {document}",
  );

  assert!(
    document.ends_with("</script></body></html>"),
    "the document closes cleanly: {document}",
  );
}

/// A `client_only` view yields nothing on the server, its build closure never running
/// here: its place is the empty enhancement a hydrating client adopts.
#[test]
fn client_only_is_empty_on_the_server() {
  let page =
    render_to_string(|| client_only(|| panic!("client_only must not build on the server")));

  assert_eq!(
    page.body(),
    "<!--s:0--><!--s:1--><!--e:1--><!--e:0-->",
    "empty segments, nothing between their anchors: {}",
    page.body(),
  );
}

/// Each resource's settled value is serialized into the render's state, by creation
/// order, for the client to adopt on hydration.
#[test]
fn resources_serialize_their_state() {
  let page = render_to_string(|| {
    let _first = Resource::new(|| async { "Ada".to_owned() });
    let _second = Resource::new(|| async { "Bo".to_owned() });

    Text::new("x")
  });

  assert_eq!(
    state_of(&page),
    r#"[["\"Ada\"","\"Bo\""],[],[]]"#,
    "both values are JSON-encoded in creation order",
  );
}

/// A lazy segment's resources serialize under the segment's place—its build order among
/// the lazy segments of the segment holding it, under that segment's own place—apart
/// from the resources built beside it, so the client matches them however late the
/// segment's content arrives.
#[test]
fn a_lazy_segments_resources_serialize_under_its_place() {
  let page = render_to_string(|| {
    let _before = Resource::new(|| async { "before".to_owned() });
    let first = AsyncView::new().set_child_loader(|| async {
      let _inner = Resource::new(|| async { "inner".to_owned() });

      AsyncView::new().set_child_loader(|| async {
        let _nested = Resource::new(|| async { "nested".to_owned() });

        Text::new("nested")
      })
    });
    let second = AsyncView::new().set_child_loader(|| async {
      let _second = Resource::new(|| async { "second".to_owned() });

      Text::new("second")
    });
    let _after = Resource::new(|| async { "after".to_owned() });

    Column::new().set_children((first, second))
  });

  assert_eq!(
    state_of(&page),
    r#"[["\"before\"","\"after\""],[],[[[0],["\"inner\""]],[[1],["\"second\""]],[[0,0],["\"nested\""]]]]"#,
    "the resources outside any lazy segment in creation order, then each segment's own",
  );
}

/// A client-only resource takes no part in the render's state, nor does it run on the
/// server.
#[test]
fn a_client_resource_is_not_serialized() {
  let page = render_to_string(|| {
    let _client: Resource<String> =
      Resource::client(|| async { panic!("a client resource must not run on the server") });

    Text::new("x")
  });

  assert_eq!(
    state_of(&page),
    "[[],[],[]]",
    "no serializable resources means empty state"
  );
}

/// An inline `Span` flows inline—its rule carries no `display:block`—unlike a block
/// `Text`, whose rule does.
#[test]
fn an_inline_span_flows_inline_unlike_a_block_text() {
  let inline = render_to_string(|| RichText::new().add_spans([Span::new("x")]));
  let block = render_to_string(|| Text::new("x"));
  let span = classes(inline.body())[1].clone();

  assert!(
    inline.body().starts_with("<div data-h=\"1\"") && inline.body().contains("<span data-h=\"2\""),
    "a paragraph is a block holding its runs: {}",
    inline.body(),
  );
  assert!(
    !base_rule(inline.styles(), &span).contains("display:"),
    "an inline run sets no display: {}",
    inline.styles(),
  );
  assert!(
    base_rule(block.styles(), &first_class(block.body())).contains("display:block"),
    "a block run does: {}",
    block.styles(),
  );
}

/// A text on a line of its own lays out one line while empty, through a rule of no
/// more specificity than its class that precedes its own; an inline run takes none.
#[test]
fn an_empty_block_text_keeps_one_line() {
  let block = render_to_string(|| Text::new(""));
  let inline = render_to_string(|| RichText::new().add_spans([Span::new("")]));
  let class = first_class(block.body());
  let empty = format!(".{class}:where(:empty){{min-block-size:var(--skene-line);}}");

  assert!(
    block.body().ends_with("></span>"),
    "an empty text renders an empty span: {}",
    block.body()
  );
  assert!(block.styles().contains(&empty), "{}", block.styles());
  assert!(
    block.styles().find(&empty) < block.styles().find(&format!(".{class}{{")),
    "the empty rule precedes the class's own: {}",
    block.styles()
  );
  assert!(
    !inline.styles().contains(":where(:empty)"),
    "an inline run keeps no line of its own: {}",
    inline.styles()
  );
  assert!(
    block.styles().contains("--skene-line: 1lh;") && block.styles().contains("--skene-line: 1cap;"),
    "the reset defines the line, trimmed where line boxes are"
  );
}

/// A `RichText` flows nested groups and embedded views inline.
#[test]
fn rich_text_nests_groups_and_embeds_views() {
  let page = render_to_string(|| {
    RichText::new().add_spans([
      Span::group([Span::new("a"), Span::new("b")]),
      Span::view(Container::new()),
    ])
  });

  let body = page.body();

  assert!(
    body.contains(">a</span>") && body.contains(">b</span>"),
    "the group's nested runs are present: {body}",
  );

  assert_eq!(
    body.matches("<div").count(),
    2,
    "the block wrapper plus the embedded view's `<div>`: {body}",
  );

  assert!(
    page.styles().contains("inline-block"),
    "the embedded view's host span is inline-level: {}",
    page.styles(),
  );
}

/// A page with no `Deferred` streams as a single chunk: the walk never pauses, so the
/// one chunk is the whole document—the same markup `render_to_string` would assemble.
#[test]
fn a_page_without_deferred_streams_as_one_chunk() {
  let stream = render_to_stream(|| Text::new("hi"));

  assert_eq!(stream.chunks().len(), 1, "no boundary means no split");
  assert!(
    stream
      .document()
      .starts_with("<!DOCTYPE html><html><head><meta charset=\"utf-8\">"),
    "the one chunk opens the document: {}",
    stream.document(),
  );

  assert!(
    stream.document().contains(">hi</span>") && stream.document().ends_with("</html>"),
    "and carries the whole body: {}",
    stream.document(),
  );

  assert_eq!(
    stream.document(),
    render_to_string(|| Text::new("hi")).page(),
    "and is the page a whole render assembles",
  );
}

/// A page whose slow content sits behind a `Deferred` boundary.
fn deferred_app() -> impl skene_web::View {
  Row::new().set_children([
    Text::new("shell").into_any(),
    Deferred::new()
      .set_child_builder(|| {
        let data = Resource::new(|| async {
          platform::sleep(Duration::from_millis(5)).await;

          "late".to_owned()
        });

        derived(move || {
          data.with(|value| match value {
            None => Text::new("loading").build(),
            Some(text) => Container::new()
              .set_background(Color::srgb(0.1, 0.2, 0.3))
              .set_child(Text::new(text.clone()))
              .build(),
          })
        })
      })
      .into_any(),
  ])
}

/// A sibling shown before a paused boundary, by work the boundary waits on, moves the
/// walk's place and nothing it streams: each node streams once, in order.
#[test]
fn a_stream_resumes_where_it_stood_among_changed_siblings() {
  use skene_web::{
    Visibility,
    reactive::{Get, Set, Signal},
  };

  let stream = render_to_stream(|| {
    let grown = Signal::new(false);

    Row::new().set_children([
      Visibility::new()
        .derive_shown(move || grown.get())
        .set_child(Text::new("grown"))
        .into_any(),
      Deferred::new()
        .set_child_builder(move || {
          skene_web::spawn(async move {
            platform::sleep(Duration::from_millis(1)).await;
            grown.set(true);
          });

          let data = Resource::new(|| async {
            platform::sleep(Duration::from_millis(5)).await;

            "late".to_owned()
          });

          derived(move || data.with(|value| Text::new(value.cloned().unwrap_or_default()).build()))
        })
        .into_any(),
      Text::new("after").into_any(),
    ])
  });
  let document = stream.document();
  let comments: Vec<&str> = document
    .match_indices("<!--")
    .map(|(at, _)| &document[at..])
    .map(|rest| &rest[..rest.find("-->").expect("a comment end")])
    .collect();
  let mut unique = comments.clone();

  unique.sort_unstable();
  unique.dedup();

  assert!(
    !stream.chunks()[0].contains(">grown</span>"),
    "the sibling showed while the walk paused: {document}"
  );
  assert_eq!(
    comments.len(),
    unique.len(),
    "no anchor streams twice: {document}"
  );
  assert_eq!(document.matches(">after</span>").count(), 1, "{document}");
  assert!(
    document.find(">late</span>") < document.find(">after</span>"),
    "{document}"
  );
}

/// The warnings a render logs, collected.
struct Warnings(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

impl skene_web::tracing::Subscriber for Warnings {
  fn enabled(&self, metadata: &skene_web::tracing::Metadata<'_>) -> bool {
    *metadata.level() <= skene_web::tracing::Level::WARN
  }

  fn new_span(&self, _: &skene_web::tracing::span::Attributes<'_>) -> skene_web::tracing::span::Id {
    skene_web::tracing::span::Id::from_u64(1)
  }

  fn record(&self, _: &skene_web::tracing::span::Id, _: &skene_web::tracing::span::Record<'_>) {}

  fn record_follows_from(
    &self,
    _: &skene_web::tracing::span::Id,
    _: &skene_web::tracing::span::Id,
  ) {
  }

  fn event(&self, event: &skene_web::tracing::Event<'_>) {
    struct Message<'a>(&'a mut String);

    impl skene_web::tracing::field::Visit for Message<'_> {
      fn record_debug(
        &mut self,
        _: &skene_web::tracing::field::Field,
        value: &dyn std::fmt::Debug,
      ) {
        let _ = std::fmt::Write::write_fmt(self.0, format_args!("{value:?}"));
      }
    }

    let mut message = String::new();

    event.record(&mut Message(&mut message));
    self.0.lock().expect("the warnings").push(message);
  }

  fn enter(&self, _: &skene_web::tracing::span::Id) {}

  fn exit(&self, _: &skene_web::tracing::span::Id) {}
}

/// A title registered inside a deferred boundary, after the streamed shell flushed,
/// misses the streamed page and is warned about.
#[test]
fn a_title_registered_after_the_shell_is_warned_about() {
  use skene_web::window;

  let warnings = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
  let stream = skene_web::tracing::subscriber::with_default(
    Warnings(std::sync::Arc::clone(&warnings)),
    || {
      render_to_stream(|| {
        Row::new().add_child(Deferred::new().set_child_builder(|| {
          let data = Resource::new(|| async {
            platform::sleep(Duration::from_millis(1)).await;

            "late".to_owned()
          });

          derived(move || {
            if data.with(|value| value.is_some()) {
              window::set_title("Late");
            }

            Text::new("x").build()
          })
        }))
      })
    },
  );

  assert!(!stream.document().contains("<title>Late</title>"));
  assert!(
    warnings
      .lock()
      .expect("the warnings")
      .iter()
      .any(|warning| warning.contains("head registration")),
    "{:?}",
    warnings.lock().expect("the warnings")
  );
}

/// The piece-driven stream: the sink receives the opening shell, a chunk per deferred
/// boundary, and the closing tail, in order—and the pieces concatenate to the same
/// document the collected stream assembles.
#[test]
fn page_stream_pieces_concatenate_to_the_document() {
  let mut pieces: Vec<String> = Vec::new();

  render_page_stream(deferred_app, |piece| pieces.push(piece.to_owned()));

  assert!(
    pieces.len() >= 4,
    "shell, a chunk each side of the boundary, and the tail: {} pieces",
    pieces.len(),
  );

  assert!(pieces[0].starts_with("<!DOCTYPE html><html><head><meta charset=\"utf-8\">"));
  assert!(pieces[pieces.len() - 1].ends_with("</body></html>"));
  assert_eq!(
    pieces.concat(),
    render_to_stream(deferred_app).document(),
    "the pieces concatenate to the collected stream's document",
  );
}

/// The payoff: the shell flushes before a `Deferred` boundary's slow content settles.
/// The first chunk carries the shell and stops *before* the boundary; the boundary's
/// loaded content arrives in a later chunk, in document order—no placeholder, and no
/// JavaScript.
#[test]
fn deferred_content_streams_after_the_shell() {
  let stream = render_to_stream(deferred_app);

  assert!(
    stream.chunks().len() >= 2,
    "the shell flushes before the boundary settles: {:?}",
    stream.chunks(),
  );

  assert!(
    stream.chunks()[0].contains(">shell</span>"),
    "the first chunk carries the shell: {}",
    stream.chunks()[0],
  );

  assert!(
    !stream.chunks()[0].contains(">late</span>") && !stream.chunks()[0].contains(">loading</span>"),
    "and stops before the boundary—neither its loaded value nor its placeholder: {}",
    stream.chunks()[0],
  );

  let document = stream.document();

  assert!(
    document.contains(">late</span>") && !document.contains(">loading</span>"),
    "the boundary streams its settled content, never the placeholder: {document}",
  );

  assert!(
    document.find("shell") < document.find("late"),
    "and in document order, the shell before the deferred content: {document}",
  );
}

/// A boundary whose loaded content introduces a class the shell never used streams
/// that class's rule ahead of the content, in the same chunk, so a client with no
/// JavaScript paints it styled; the whole document holds each rule once.
#[test]
fn a_streamed_boundary_carries_the_styles_it_introduces() {
  let stream = render_to_stream(deferred_app);
  let document = stream.document();
  let late = document.find(">late</span>").expect("the late content");
  let before = classes(&document[..late]);
  // The loaded container's class: the one before the run's own.
  let late_class = before[before.len() - 2].clone();
  let rule = format!(".{late_class}{{");

  assert!(
    !stream.chunks()[0].contains(&rule),
    "the shell does not know the class yet: {}",
    stream.chunks()[0],
  );

  let chunk = stream
    .chunks()
    .iter()
    .find(|chunk| chunk.contains(">late</span>"))
    .expect("the chunk carrying the content");
  let delta = chunk
    .find("<style data-skene-style>")
    .expect("the chunk opens with a style delta");

  assert!(
    chunk[delta..].find(&rule) < chunk.find(">late</span>"),
    "the rule precedes the content it styles: {chunk}",
  );
  assert_eq!(
    document.matches(&rule).count(),
    1,
    "the rule is emitted once: {document}"
  );
  assert!(
    document.contains(&format!(
      "background-color:{};",
      css(Color::srgb(0.1, 0.2, 0.3))
    )),
    "{document}"
  );
}

/// A group's style cascades to its nested runs: the group redefines the ink token on
/// its own run, and a colorless child reads the token.
#[test]
fn a_groups_style_cascades_to_its_children() {
  let red = Color::srgb(1.0, 0.0, 0.0);
  let page = render_to_string(move || {
    RichText::new().add_spans([Span::group([Span::new("a")]).set_color(red)])
  });
  let classes = classes(page.body());

  assert_eq!(
    classes.len(),
    3,
    "the block, the group and the child: {}",
    page.body()
  );
  assert!(
    page
      .styles()
      .contains(&format!("{{--typography-ink:{};}}", css(red))),
    "the group defines the ink beneath it: {}",
    page.styles(),
  );
  assert!(
    base_rule(page.styles(), &classes[2]).contains("color:var(--typography-ink)"),
    "the child inherits it through the token: {}",
    page.styles(),
  );
}

/// A render that cannot settle within its deadline returns with placeholders instead
/// of hanging its caller.
#[test]
fn render_deadline_cuts_off_unsettled_work() {
  platform::set_render_deadline(Duration::from_millis(20));

  let rendered = render_to_string(|| {
    let value = Resource::new(|| async {
      platform::sleep(Duration::from_hours(1)).await;

      1u32
    });

    derived(move || match value.get() {
      None => Text::new("still waiting").build(),
      Some(value) => Text::new(format!("done {value}")).build(),
    })
  });

  platform::set_render_deadline(Duration::from_secs(10));

  assert!(rendered.body().contains("still waiting"));
  assert!(!rendered.body().contains("done"));
}

/// A tagged resource serializes its settled value under its tag, decoupled from
/// creation order.
#[test]
fn tagged_resource_serializes_under_its_tag() {
  let rendered = render_to_string(|| {
    let value = Resource::tagged("answer", || async { 41u32 });

    derived(move || match value.get() {
      None => Text::new("hold").build(),
      Some(value) => Text::new(format!("tagged {value}")).build(),
    })
  });

  assert!(rendered.body().contains("tagged 41"));
  assert!(state_of(&rendered).contains("answer"));
  assert!(state_of(&rendered).contains("41"));
}

/// A run's selection colors compile to a `::selection` rule inside its interned class,
/// and `set_selectable(true)` opts the run in through `user-select`—the reset keeps
/// everything else unselectable.
#[test]
fn selection_styling_compiles_into_the_class() {
  let page = render_to_string(|| {
    Column::new().set_children((
      Text::new("pick me")
        .set_selectable(true)
        .set_selection_color(Color::srgb(1., 1., 1.))
        .set_selection_background(Color::srgb(1., 0., 0.)),
      Text::new("plain"),
    ))
  });
  let styles = page.styles();

  assert!(
    styles.contains("::selection{background-color:oklch(") || styles.contains("::selection{color:"),
    "a ::selection rule renders: {styles}",
  );
  assert!(
    styles.contains("user-select:text;"),
    "the opt-in restores user-select: {styles}",
  );
  assert!(
    styles.contains("user-select: none"),
    "the reset defaults everything unselectable: {styles}",
  );
}

/// The typography's selection colors inherit down to its runs through the selection
/// tokens.
#[test]
fn typography_selection_colors_inherit() {
  let surface = Color::srgb(0., 1., 0.);
  let page = render_to_string(move || {
    Theme::new(
      Typography::default()
        .set_selection_ink(Color::srgb(0., 0., 0.))
        .set_selection_surface(surface),
    )
    .set_child_builder(|| Text::new("themed"))
  });
  let run = classes(page.body())[1].clone();

  assert!(
    page.styles().contains(&format!(
      ".{run}::selection{{background-color:var(--typography-selection-surface);"
    )),
    "the run selects in the token: {}",
    page.styles(),
  );
  assert!(
    page
      .styles()
      .contains(&format!("--typography-selection-surface:{};", css(surface))),
    "which the theme defines: {}",
    page.styles(),
  );
}

/// A widget role serializes as an ARIA role on a plain element, and its reported state
/// as the matching `aria-*` attributes—so a scriptless client still meets a real
/// control.
#[test]
fn a_widget_role_and_state_serialize_as_aria() {
  use skene_foundation::{AccessProp, Role};

  let rendered = render_to_string(|| {
    Container::new()
      .set_role(Role::Slider)
      .set_focus_order(skene_web::FocusOrder::Auto)
      .set_accessibility([
        AccessProp::Value(40.),
        AccessProp::ValueMin(0.),
        AccessProp::ValueMax(100.),
        AccessProp::Name("Volume".to_owned()),
      ])
  });

  let body = rendered.body();

  assert!(
    body.starts_with("<div"),
    "a widget role stays a plain element"
  );
  assert!(body.contains("role=\"slider\""));
  assert!(body.contains("aria-valuenow=\"40\""));
  assert!(body.contains("aria-valuemin=\"0\""));
  assert!(body.contains("aria-valuemax=\"100\""));
  assert!(body.contains("aria-label=\"Volume\""));
  assert!(
    body.contains("tabindex=\"0\""),
    "a drawn control joins the tab order the way a native one does",
  );
}

/// A render given a dark scheme hint answers a derived leaf with its dark value.
#[test]
fn a_dark_scheme_hint_serializes_the_dark_value_of_a_derived_leaf() {
  use skene_foundation::ColorScheme;
  use skene_web::reactive::Get;

  let black = Color::srgb(0., 0., 0.);
  let white = Color::srgb(1., 1., 1.);

  platform::set_render_hints(platform::RenderHints::from_headers([(
    "Sec-CH-Prefers-Color-Scheme",
    "dark",
  )]));

  let page = render_to_string(move || {
    Container::new().derive_background(move || {
      if skene_web::media::scheme().get() == ColorScheme::Dark {
        black
      }
      else {
        white
      }
    })
  });

  platform::set_render_hints(platform::RenderHints::DEFAULT);

  assert!(
    page
      .body()
      .contains(&format!("style=\"--skene-background:{};\"", css(black))),
    "the dark value is the slot's serialized value: {}",
    page.body(),
  );
}

/// A page whose reader may have chosen a scheme carries the script that applies that
/// choice before the first paint, inline and blocking in the head.
#[test]
fn a_chooseable_scheme_preempts_its_own_flash() {
  use skene_foundation::ColorScheme;

  let rendered = render_to_string(|| {
    Theme::new(SimplePalette::default()).set_child_builder(|| {
      skene_web::derive_scheme(|| Some(ColorScheme::Dark));

      Text::new("themed")
    })
  });
  let head = rendered.head();

  assert!(head.contains("<script>"), "the script is inline, not a src");
  assert!(!head.contains("defer"), "and blocking, which is the point");
  assert!(head.contains("localStorage.getItem('skene-scheme')"));
  assert!(head.contains("data-skene-scheme"));
}

/// A page that never offers a choice pays nothing for one.
#[test]
fn a_page_with_no_chooseable_scheme_carries_no_script() {
  let rendered = render_to_string(|| Text::new("plain"));

  assert!(!rendered.head().contains("<script>"));
}

/// A disabled checkbox keeps its native input out of the tab order with
/// `tabindex="-1"`, as the browser writes it.
#[test]
fn a_disabled_checkbox_keeps_its_input_out_of_the_tab_order() {
  use skene_web::{BasicCheckbox, Toggle};

  let rendered = render_to_string(|| BasicCheckbox::new(Toggle::new()).set_disabled(true));
  let body = rendered.body();
  let input = body
    .split("<input")
    .nth(1)
    .and_then(|rest| rest.split('>').next())
    .expect("the checkbox's input");

  assert!(input.contains(" disabled"), "{body}");
  assert!(input.contains("tabindex=\"-1\""), "{body}");
}

/// A low-level element's `style` attribute and its declarations serialize as one
/// attribute, the attribute replacing the declarations set before it, as the browser
/// holds them.
#[test]
fn a_markup_style_and_its_declarations_serialize_as_one_attribute() {
  use skene_foundation::{AnyView, NodeKind};
  use skene_web::{
    Active,
    platform::{Declarations, WebMarkup},
  };

  let rendered = render_to_string(|| {
    AnyView::node(skene_foundation::LowLevel::<Active>::realize(
      NodeKind::Box,
      |node| {
        Active::create_element(node, "div");
        Active::set_declarations(node, &Declarations::new().set("margin", "0"));
        Active::set_attribute(node, "style", "color: red");
        Active::set_declarations(node, &Declarations::new().set("padding", "0"));
      },
    ))
  });
  let body = rendered.body();

  assert_eq!(body.matches(" style=").count(), 1, "{body}");
  assert!(body.contains("style=\"color: red;padding:0;\""), "{body}");
}

/// A required, read-only text field serializes the native attributes a form enforces
/// without script, and its description gets the id the field names.
#[test]
fn control_flags_serialize_natively_and_references_carry_ids() {
  use skene_web::{BasicTextField, TextEntry};

  let rendered = render_to_string(|| {
    let hint = Text::new("Required").build();

    Column::new().set_children((
      skene_foundation::AnyView::node(hint),
      BasicTextField::new(TextEntry::new())
        .set_required(true)
        .set_read_only(true)
        .set_invalid(true)
        .set_described_by(hint),
    ))
  });
  let body = rendered.body();

  assert!(body.contains(" required"), "{body}");
  assert!(body.contains(" readonly"), "{body}");
  assert!(
    !body.contains("aria-required"),
    "a form element takes the native form: {body}"
  );
  assert!(body.contains("aria-invalid=\"true\""), "{body}");

  let described = body
    .split("aria-describedby=\"")
    .nth(1)
    .and_then(|rest| rest.split('"').next())
    .expect("a description reference");

  assert!(body.contains(&format!("id=\"{described}\"")), "{body}");
}

/// A control's own description and a tooltip's fold into one reference list, each
/// referenced node carrying the id the list names.
#[test]
fn descriptions_fold_into_one_reference_list() {
  use skene_web::{BasicTextField, TextEntry};

  let rendered = render_to_string(|| {
    let hint = Text::new("Required").build();

    Column::new().set_children((
      skene_foundation::AnyView::node(hint),
      Tooltip::new("Your full name")
        .set_child(BasicTextField::new(TextEntry::new()).set_described_by(hint)),
    ))
  });
  let body = rendered.body();
  let described: Vec<&str> = body
    .split("aria-describedby=\"")
    .nth(1)
    .and_then(|rest| rest.split('"').next())
    .expect("a description reference")
    .split(' ')
    .collect();

  assert_eq!(body.matches("aria-describedby=").count(), 1, "{body}");
  assert_eq!(described.len(), 2, "{body}");

  for id in described {
    assert!(body.contains(&format!("id=\"{id}\"")), "{body}");
  }
}

/// The state script escapes the markup its state holds, so it closes nothing early.
#[test]
fn the_state_script_closes_nothing_early() {
  let page = render_to_string(|| {
    let _tag = Resource::new(|| async { "</script><script>alert(1)</script>".to_owned() });

    Text::new("x")
  });
  let script = page.state_script();

  assert_eq!(script.matches("</script>").count(), 1, "{script}");
  assert!(state_of(&page).contains("</script><script>alert(1)"));
}

/// A render's inline script carries the nonce it was given, and none without one.
#[test]
fn an_inline_script_carries_the_renders_nonce() {
  let app = || {
    skene_web::media::derive_scheme(|| None);

    Text::new("x")
  };

  platform::set_script_nonce(Some("r4nd\"om"));

  let page = render_to_string(app).page();

  platform::set_script_nonce(None);

  assert!(page.contains("<script nonce=\"r4nd&quot;om\">"), "{page}");
  assert!(!render_to_string(app).page().contains("nonce="));
}

/// A page boots and preloads the client bundle it was given, and in the dev loop
/// carries the dev script, every inline script under the render's nonce.
#[test]
fn a_page_boots_its_bundle_under_the_nonce() {
  platform::set_client_bundle(Some("app"));
  platform::set_dev_loop(true);
  platform::set_script_nonce(Some("n0"));

  let page = render_to_string(|| Text::new("x")).page();

  platform::set_client_bundle(None);
  platform::set_dev_loop(false);
  platform::set_script_nonce(None);

  assert!(
    page.contains("<link rel=\"modulepreload\" href=\"/app.js\">"),
    "{page}"
  );
  assert!(
    page.contains("<meta name=\"skene-dev\"><script nonce=\"n0\">"),
    "{page}"
  );
  assert!(
    page.ends_with(
      "<script type=\"module\" nonce=\"n0\">import init from \"/app.js\";init();</script>\
       </body></html>"
    ),
    "{page}"
  );

  let bare = render_to_string(|| Text::new("x")).page();

  assert!(!bare.contains("<script type=\"module\""), "{bare}");
  assert!(!bare.contains("skene-dev"), "{bare}");
}

/// A head registration is an element a head holds, with real attribute names.
#[test]
#[should_panic(expected = "a head element is one of")]
fn a_head_registration_names_a_head_element() {
  let _ = render_to_string(|| {
    let _ = platform::add_head_element("img", &[("src", "x")]);

    Text::new("x")
  });
}

/// An attribute name that is not one is refused rather than written into the markup.
#[test]
#[should_panic(expected = "is not an attribute name")]
fn a_head_registration_refuses_an_attribute_that_is_not_one() {
  let _ = render_to_string(|| {
    let _ = platform::add_head_element("meta", &[("onload=alert(1) x", "y")]);

    Text::new("x")
  });
}
