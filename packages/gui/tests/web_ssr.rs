//! Server-side rendering: a view tree to HTML, against the real string sink.
//!
//! Run under a WASI runtime:
//! `cargo test -p skene-gui --target wasm32-wasip2` (with `wasmtime` on `PATH`).
#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use std::time::Duration;

use skene_gui::{
  Color,
  Column,
  Container,
  Deferred,
  HasChildren,
  Resource,
  RichText,
  Row,
  Span,
  Text,
  Theme,
  Typography,
  View,
  client_only,
  derived,
  render_page_stream,
  render_to_stream,
  render_to_string,
  web,
};

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

/// A color as the CSS the driver renders it to.
fn css(color: Color) -> String {
  let [lightness, chroma, hue, alpha] = color.to_oklch();

  format!("oklch({lightness} {chroma} {hue} / {alpha})")
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
/// runner blocks on the WASI reactor until the sleep resolves, so the loaded tree
/// serializes.
#[test]
fn an_async_load_settles_before_serialization() {
  let page = render_to_string(|| {
    let greeting = Resource::new(|| async {
      web::sleep(Duration::from_millis(5)).await;

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

/// A complete document assembles the styles into `<head>`, the tree into `<body>`, and
/// the state into the script `hydrate_page` reads back.
#[test]
fn page_assembles_a_full_document() {
  let page = render_to_string(|| {
    let _greeting = Resource::new(|| async { "Ada".to_owned() });

    Text::new("hi")
  });

  let document = page.page();

  assert!(
    document.starts_with("<!DOCTYPE html><html><head><style id=\"skene-styles\">"),
    "the styles open the head: {document}",
  );

  assert!(
    document.contains(&format!("</head><body>{}", page.body())),
    "the tree is the body: {document}",
  );

  assert!(
    document
      .contains("<script id=\"skene-state\" type=\"application/json\">[[\"\\\"Ada\\\"\"],[]]"),
    "the state is embedded in the script: {document}",
  );

  assert!(
    document.ends_with("</script></body></html>"),
    "the document closes cleanly: {document}",
  );
}

/// A `client_only` view yields nothing on the server; its build closure never runs
/// here.
#[test]
fn client_only_is_empty_on_the_server() {
  let page =
    render_to_string(|| client_only(|| panic!("client_only must not build on the server")));

  assert_eq!(
    page.body(),
    "<!--s:0--><!--e:0-->",
    "an empty segment, nothing between its anchors: {}",
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
    page.state(),
    r#"[["\"Ada\"","\"Bo\""],[]]"#,
    "both values are JSON-encoded in creation order",
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
    page.state(),
    "[[],[]]",
    "no serializable resources means empty state"
  );
}

/// An inline `Span` flows inline—its rule carries no `display:block`—unlike a block
/// `Text`, whose rule does.
#[test]
fn an_inline_span_flows_inline_unlike_a_block_text() {
  let inline = render_to_string(|| RichText::new().set_spans([Span::new("x")]));
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

/// A `RichText` flows nested groups and embedded views inline.
#[test]
fn rich_text_nests_groups_and_embeds_views() {
  let page = render_to_string(|| {
    RichText::new().set_spans([
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
    stream.document().starts_with("<!DOCTYPE html><html><head>"),
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
fn deferred_app() -> impl skene_gui::IntoView {
  Row::new().set_children([
    Text::new("shell").into_any(),
    Deferred::new(|| {
      let data = Resource::new(|| async {
        web::sleep(Duration::from_millis(5)).await;

        "late".to_owned()
      });

      derived(move || {
        data.with(|value| match value {
          None => Text::new("loading").build(),
          Some(text) => Container::new()
            .set_color(Color::srgb(0.1, 0.2, 0.3))
            .set_child(Text::new(text.clone()))
            .build(),
        })
      })
    })
    .into_any(),
  ])
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

  assert!(pieces[0].starts_with("<!DOCTYPE html><html><head>"));
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
    RichText::new().set_spans([Span::group([Span::new("a")]).set_color(red)])
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
  web::set_render_deadline(Duration::from_millis(20));

  let rendered = render_to_string(|| {
    let value = Resource::client(|| async {
      web::sleep(Duration::from_hours(1)).await;

      1u32
    });

    derived(move || match value.get() {
      None => Text::new("still waiting").build(),
      Some(value) => Text::new(format!("done {value}")).build(),
    })
  });

  web::set_render_deadline(Duration::from_secs(10));

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
  assert!(rendered.state().contains("answer"));
  assert!(rendered.state().contains("41"));
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
  use skene_gui::core::{AccessProp, Role};

  let rendered = render_to_string(|| {
    Container::new()
      .set_role(Role::Slider)
      .set_focus_order(skene_gui::FocusOrder::Auto)
      .set_accessibility([
        AccessProp::Value(40.),
        AccessProp::ValueMin(0.),
        AccessProp::ValueMax(100.),
        AccessProp::Label("Volume".to_owned()),
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
  use skene_gui::{core::ColorScheme, reactive::Get};

  let black = Color::srgb(0., 0., 0.);
  let white = Color::srgb(1., 1., 1.);

  web::set_render_hints(web::RenderHints::from_headers([(
    "Sec-CH-Prefers-Color-Scheme",
    "dark",
  )]));

  let page = render_to_string(move || {
    Container::new().derive_color(move || {
      if skene_gui::media::scheme().get() == ColorScheme::Dark {
        black
      }
      else {
        white
      }
    })
  });

  web::set_render_hints(web::RenderHints::DEFAULT);

  assert!(
    page
      .body()
      .contains(&format!("style=\"--skene-color:{};\"", css(black))),
    "the dark value is the slot's serialized value: {}",
    page.body(),
  );
}

/// A page whose reader may have chosen a scheme carries the script that applies that
/// choice before the first paint—inline and blocking in the head, since a deferred one
/// would run after the page had already been painted in the platform's scheme.
#[test]
fn a_chooseable_scheme_preempts_its_own_flash() {
  use skene_gui::{SimplePalette, core::ColorScheme};

  let rendered = render_to_string(|| {
    Theme::new(SimplePalette::default()).set_child_builder(|| {
      skene_gui::derive_scheme(|| Some(ColorScheme::Dark));

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

/// A required, read-only text field serializes the native attributes a form enforces
/// without script; its description gets the id the field names; a picker's face names
/// the listbox it controls.
#[test]
fn control_flags_serialize_natively_and_references_carry_ids() {
  use skene_gui::{ListItem, RawOption, RawPicker, RawTextField, TextEntry, reactive::Get};

  let rendered = render_to_string(|| {
    let hint = Text::new("Required").build();

    Column::new().set_children((
      skene_gui::core::AnyView::node(hint),
      RawTextField::new(TextEntry::new())
        .set_required(true)
        .set_read_only(true)
        .set_invalid(true)
        .set_described_by(hint),
      RawPicker::new()
        .set_options(["a", "b"].map(|value| RawOption::new(ListItem::new(value)).set_label(value)))
        .set_face(|shown| Text::derive(move || shown.get())),
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
  assert!(body.contains("aria-haspopup=\"listbox\""), "{body}");

  let controls = body
    .split("aria-controls=\"")
    .nth(1)
    .and_then(|rest| rest.split('"').next())
    .expect("a controls reference");

  assert!(body.contains(&format!("id=\"{controls}\"")), "{body}");
}
