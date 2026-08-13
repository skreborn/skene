//! Server rendering of suspense: the render settles through the boundary, so the
//! content serializes and the fallback never does.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene_gui::{DerivedView, Resource, Suspense, Text, View, render_to_string};
// The csr and ssr halves each use a different subset of the surface traits.
#[allow(unused_imports)]
use skene_gui::{HasFocus, HasStyle};

/// The server drives the resource to its value and serializes the settled content.
#[test]
fn suspense_serializes_settled_content() {
  let rendered = render_to_string(|| {
    Suspense::new(|| {
      let value = Resource::new(|| async { 7u32 });

      DerivedView::new(move || match value.get() {
        None => Text::new("hold").into_any(),
        Some(value) => Text::new(format!("ready {value}")).into_any(),
      })
    })
    .set_fallback(|| Text::new("FALLBACK"))
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
    Suspense::new(|| {
      let value = Resource::new(|| async { 3u32 });

      DerivedView::new(move || match value.get() {
        None => Text::new("hold").into_any(),
        Some(value) => Text::new(format!("streamed {value}")).into_any(),
      })
    })
    .set_fallback(|| Text::new("FALLBACK"))
  });

  let body: String = stream.into_chunks().concat();

  assert!(body.contains("streamed 3"));
  assert!(!body.contains("FALLBACK"));
}

/// An error boundary with nothing reported serializes its content alone.
#[test]
fn error_boundary_passes_content_through() {
  let rendered = render_to_string(|| {
    skene_gui::ErrorBoundary::new(|| Text::new("CONTENT"))
      .set_fallback(|errors| Text::new(errors.with(|errors| format!("failed: {}", errors[0]))))
  });

  let body = rendered.body();

  assert!(body.contains("CONTENT"));
  assert!(!body.contains("failed"));
}

/// An error reported while building serializes the fallback and keeps the content out.
#[test]
fn error_boundary_serializes_reported_fallback() {
  let rendered = render_to_string(|| {
    skene_gui::ErrorBoundary::new(|| {
      let errors = skene_gui::Errors::current().expect("a boundary handle");

      errors.report(std::io::Error::other("broken"));

      Text::new("CONTENT")
    })
    .set_fallback(|errors| Text::new(errors.with(|errors| format!("failed: {}", errors[0]))))
  });

  let body = rendered.body();

  assert!(body.contains("failed: broken"));
  assert!(!body.contains("CONTENT"));
}

/// A transitional rebuild counts toward the enclosing boundary, so a stream settles
/// through it to the resolved content.
#[test]
fn transition_settles_through_the_stream() {
  let stream = skene_gui::render_to_stream(|| {
    skene_gui::DerivedView::transitional(|| {
      let value = Resource::new(|| async { 9u32 });

      DerivedView::new(move || match value.get() {
        None => Text::new("hold").into_any(),
        Some(value) => Text::new(format!("landed {value}")).into_any(),
      })
    })
  });

  let body: String = stream.into_chunks().concat();

  assert!(body.contains("landed 9"));
  assert!(!body.contains(">hold<"));
}
