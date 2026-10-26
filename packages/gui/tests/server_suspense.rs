//! Server rendering of suspense: the render settles through the boundary, so the
//! content serializes and the fallback never does.

#![cfg(all(skene_platform = "web", skene_server))]

use skene_gui::{
  DeferredBase,
  ErrorBoundary,
  ErrorBoundaryBase,
  Errors,
  Resource,
  RowBase,
  Suspense,
  SuspenseBase,
  Text,
  TextBase,
  View,
  derived,
  render_to_string,
};

/// The server drives the resource to its value and serializes the settled content.
#[test]
fn suspense_serializes_settled_content() {
  let rendered = render_to_string(|| {
    Suspense::new()
      .set_fallback_builder(|| Text::new("FALLBACK"))
      .set_child_builder(|| {
        let value = Resource::new(|| async { 7u32 });

        derived(move || match value.get() {
          None => Text::new("hold").build(),
          Some(value) => Text::new(format!("ready {value}")).build(),
        })
      })
  });

  let body = rendered.body();

  assert!(body.contains("ready 7"));
  assert!(!body.contains("FALLBACK"));
  assert!(!body.contains("hold"));
}

/// A suspense inside a streamed render counts as one pending task of the enclosing
/// boundary, so the stream settles through it too.
#[test]
fn suspense_settles_through_the_stream() {
  let stream = skene_gui::render_to_stream(|| {
    Suspense::new()
      .set_fallback_builder(|| Text::new("FALLBACK"))
      .set_child_builder(|| {
        let value = Resource::new(|| async { 3u32 });

        derived(move || match value.get() {
          None => Text::new("hold").build(),
          Some(value) => Text::new(format!("streamed {value}")).build(),
        })
      })
  });

  let body: String = stream.into_chunks().concat();

  assert!(body.contains("streamed 3"));
  assert!(!body.contains("FALLBACK"));
}

/// A suspense inside a `Deferred` boundary holds that boundary's chunk, not the shell:
/// the shell flushes first and the chunk carries the settled content.
#[test]
fn suspense_inside_a_deferred_boundary_settles_its_chunk() {
  use std::time::Duration;

  use skene_gui::{Deferred, HasChildren, Row, platform::web};

  let stream = skene_gui::render_to_stream(|| {
    Row::new().set_children([
      Text::new("shell").into_any(),
      Deferred::new()
        .set_child_builder(|| {
          Suspense::new()
            .set_fallback_builder(|| Text::new("FALLBACK"))
            .set_child_builder(|| {
              let value = Resource::new(|| async {
                web::sleep(Duration::from_millis(5)).await;

                5u32
              });

              derived(move || match value.get() {
                None => Text::new("hold").build(),
                Some(value) => Text::new(format!("deferred {value}")).build(),
              })
            })
        })
        .into_any(),
    ])
  });

  assert!(stream.chunks().len() >= 2, "{:?}", stream.chunks());
  assert!(stream.chunks()[0].contains(">shell</span>"));
  assert!(!stream.chunks()[0].contains("deferred 5"));

  let body = stream.document();

  assert!(body.contains("deferred 5"));
  assert!(!body.contains("FALLBACK"));
  assert!(!body.contains(">hold<"));
}

/// A palette whose references carry its default: a card.
#[derive(Debug, Clone, PartialEq)]
struct Linked {
  /// The card's surface.
  card: skene_gui::Token<skene_gui::Color>,
}

impl Linked {
  const CARD: skene_gui::TokenRef<skene_gui::Color> =
    skene_gui::TokenRef::of::<Self, f32>("streamed-card");
}

impl Default for Linked {
  fn default() -> Self {
    Self {
      card: skene_gui::Token::new(Self::CARD, skene_gui::Color::srgb(0.9, 0.9, 0.8)),
    }
  }
}

impl<V: From<skene_gui::Definition<skene_gui::Color>> + 'static> skene_gui::TokenSet<V> for Linked {
  fn definitions(&self) -> Vec<(skene_gui::TokenName, V)> {
    vec![self.card.define()]
  }
}

/// A palette first referenced in a `Deferred` boundary's chunk defines its default at
/// the root in that chunk's style delta, the shell carrying none of it.
#[test]
fn a_palette_first_referenced_in_a_chunk_defines_its_default_there() {
  use std::time::Duration;

  use skene_gui::{
    Container,
    ContainerBase,
    DecorationBase,
    Deferred,
    HasChildren,
    Row,
    platform::web,
  };

  let stream = skene_gui::render_to_stream(|| {
    Row::new().set_children([
      Text::new("shell").into_any(),
      Deferred::new()
        .set_child_builder(|| {
          Suspense::new()
            .set_fallback_builder(|| Text::new("FALLBACK"))
            .set_child_builder(|| {
              let value = Resource::new(|| async {
                web::sleep(Duration::from_millis(5)).await;

                5u32
              });

              derived(move || match value.get() {
                None => Text::new("hold").build(),
                Some(_) => Container::new().set_background(Linked::CARD).build(),
              })
            })
        })
        .into_any(),
    ])
  });
  let chunks = stream.chunks();

  assert!(chunks.len() >= 2, "{chunks:?}");
  assert!(!chunks[0].contains("streamed-card"), "{}", chunks[0]);

  let later = chunks[1..].concat();

  assert!(
    later.contains("@property --streamed-card{"),
    "the registration: {later}"
  );
  assert!(
    later.contains(":root{--streamed-card:"),
    "the definition: {later}"
  );
  assert!(
    later.contains("var(--streamed-card)"),
    "the consumer: {later}"
  );
}

/// An error boundary with nothing reported serializes its content alone.
#[test]
fn error_boundary_passes_content_through() {
  let rendered = render_to_string(|| {
    ErrorBoundary::new()
      .set_fallback_builder(|errors| {
        Text::new(errors.with(|errors| format!("failed: {}", errors[0])))
      })
      .set_child_builder(|| Text::new("CONTENT"))
  });

  let body = rendered.body();

  assert!(body.contains("CONTENT"));
  assert!(!body.contains("failed"));
}

/// An error reported while building serializes the fallback and keeps the content out.
#[test]
fn error_boundary_serializes_reported_fallback() {
  let rendered = render_to_string(|| {
    ErrorBoundary::new()
      .set_fallback_builder(|errors| {
        Text::new(errors.with(|errors| format!("failed: {}", errors[0])))
      })
      .set_child_builder(|| {
        let errors = Errors::current().expect("a boundary handle");

        errors.report(std::io::Error::other("broken"));

        Text::new("CONTENT")
      })
  });

  let body = rendered.body();

  assert!(body.contains("failed: broken"));
  assert!(!body.contains("CONTENT"));
}
