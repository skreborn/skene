//! Server-side rendering: a view tree to HTML, against the real string sink.
//!
//! Run under a WASI runtime:
//! `cargo test -p skene-gui --target wasm32-wasip2` (with `wasmtime` on `PATH`).
#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use std::time::Duration;

use skene::{
  Color,
  Container,
  Deferred,
  DynamicView,
  HasChildren,
  Resource,
  RichText,
  Span,
  Text,
  View,
  client_only,
  render_to_stream,
  render_to_string,
  web,
};

/// The class of the first classed element in `body`.
fn first_class(body: &str) -> &str {
  let start = body.find("class=\"").expect("a classed element") + "class=\"".len();
  let rest = &body[start..];

  &rest[..rest.find('"').expect("class end")]
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
  let page = render_to_string(|| Container::new().set_children([Text::new("a"), Text::new("b")]));
  let body = page.body();
  // The shared class is on the sibling spans, not the container that wraps them.
  let span = body.find("<span").expect("a span");
  let class = first_class(&body[span..]);

  assert_eq!(
    body.matches(class).count(),
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

/// The payoff: an immediately-ready resource settles during the render, so the *loaded* tree
/// serializes, not the placeholder—the SSR fixpoint at work.
#[test]
fn a_resolved_resource_serializes_its_loaded_tree() {
  let page = render_to_string(|| {
    let greeting = Resource::new(|| async { "Ada".to_owned() });

    DynamicView::new(move || {
      greeting.with(|value| match value {
        None => Text::new("Loading…").into_any(),
        Some(name) => Container::new()
          .set_child(Text::new(name.clone()))
          .into_any(),
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
/// runner blocks on the WASI reactor until the sleep resolves, so the loaded tree serializes.
#[test]
fn an_async_load_settles_before_serialization() {
  let page = render_to_string(|| {
    let greeting = Resource::new(|| async {
      web::sleep(Duration::from_millis(5)).await;

      "Ada".to_owned()
    });

    DynamicView::new(move || {
      greeting.with(|value| match value {
        None => Text::new("Loading…").into_any(),
        Some(name) => Container::new()
          .set_child(Text::new(name.clone()))
          .into_any(),
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

/// A complete document assembles the styles into `<head>`, the tree into `<body>`, and the state
/// into the script `hydrate_page` reads back.
#[test]
fn page_assembles_a_full_document() {
  let page = render_to_string(|| {
    let _greeting = Resource::new(|| async { "Ada".to_owned() });

    Text::new("hi")
  });

  let document = page.page();

  assert!(
    document.starts_with("<!DOCTYPE html><html><head><style>"),
    "the styles open the head: {document}",
  );

  assert!(
    document.contains(&format!("</head><body>{}", page.body())),
    "the tree is the body: {document}",
  );

  assert!(
    document.contains("<script id=\"skene-state\" type=\"application/json\">[\"\\\"Ada\\\"\"]"),
    "the state is embedded in the script: {document}",
  );

  assert!(
    document.ends_with("</script></body></html>"),
    "the document closes cleanly: {document}",
  );
}

/// A `client_only` view yields nothing on the server; its build closure never runs here.
#[test]
fn client_only_is_empty_on_the_server() {
  let page =
    render_to_string(|| client_only(|| panic!("client_only must not build on the server")));

  assert_eq!(
    page.body(),
    "<div data-h=\"1\"></div>",
    "an empty, marked grouping node: {}",
    page.body(),
  );
}

/// Each resource's settled value is serialized into the render's state, by creation order, for
/// the client to adopt on hydration.
#[test]
fn resources_serialize_their_state() {
  let page = render_to_string(|| {
    let _first = Resource::new(|| async { "Ada".to_owned() });
    let _second = Resource::new(|| async { "Bo".to_owned() });

    Text::new("x")
  });

  assert_eq!(
    page.state(),
    r#"["\"Ada\"","\"Bo\""]"#,
    "both values are JSON-encoded in creation order",
  );
}

/// A client-only resource takes no part in the render's state, nor does it run on the server.
#[test]
fn a_client_resource_is_not_serialized() {
  let page = render_to_string(|| {
    let _client: Resource<String> =
      Resource::client(|| async { panic!("a client resource must not run on the server") });

    Text::new("x")
  });

  assert_eq!(
    page.state(),
    "[]",
    "no serializable resources means empty state"
  );
}

/// An inline `Span` is a classless, inline run—unlike a block `Text`, which carries a
/// `display:block` class.
#[test]
fn an_inline_span_is_classless_unlike_a_block_text() {
  let page = render_to_string(|| RichText::new([Span::text("x")]));

  assert_eq!(
    page.body(),
    "<div data-h=\"1\"><span data-h=\"2\">x</span></div>",
    "an unstyled inline run has no class (no display:block): {}",
    page.body(),
  );
}

/// A `RichText` flows nested groups and embedded views inline.
#[test]
fn rich_text_nests_groups_and_embeds_views() {
  let page = render_to_string(|| {
    RichText::new([
      Span::group([Span::text("a"), Span::text("b")]),
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

/// A page with no `Deferred` streams as a single chunk: the walk never pauses, so the one chunk
/// is the whole document—the same markup `render_to_string` would assemble.
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
}

/// The payoff: the shell flushes before a `Deferred` boundary's slow content settles. The first
/// chunk carries the shell and stops *before* the boundary; the boundary's loaded content arrives
/// in a later chunk, in document order—no placeholder, and no JavaScript.
#[test]
fn deferred_content_streams_after_the_shell() {
  let stream = render_to_stream(|| {
    Container::new().set_children([
      Text::new("shell").into_any(),
      Deferred::new(|| {
        let data = Resource::new(|| async {
          web::sleep(Duration::from_millis(5)).await;

          "late".to_owned()
        });

        DynamicView::new(move || {
          data.with(|value| match value {
            None => Text::new("loading").into_any(),
            Some(text) => Text::new(text.clone()).into_any(),
          })
        })
      })
      .into_any(),
    ])
  });

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

/// A group's style cascades to its nested runs: a colorless child adopts the group's color, so
/// both the group and the child carry the shared class.
#[test]
fn a_groups_style_cascades_to_its_children() {
  let page = render_to_string(|| {
    RichText::new([Span::group([Span::text("a")]).set_color(Color::srgb(1.0, 0.0, 0.0))])
  });

  assert_eq!(
    page.body().matches("class=\"skene-").count(),
    2,
    "the group and its inheriting child share the color class: {}",
    page.body(),
  );
}
