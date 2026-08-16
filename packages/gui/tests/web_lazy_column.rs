//! Server rendering of the virtual list: an unmeasured viewport serializes only the
//! overscan margin, the rest as spacer extent.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

// The csr and ssr halves each use a different subset of the surface traits.
#[allow(unused_imports)]
use skene_gui::{HasDecoration, HasFocus};
use skene_gui::{LazyColumn, Text, render_to_string};

/// A server render carries the scroll box, the overscan slice, and a trailing spacer
/// covering every unbuilt item.
#[test]
fn virtual_list_serializes_the_overscan_slice() {
  let rendered = render_to_string(|| {
    LazyColumn::new(24.0, || 1000, |index| Text::new(format!("row {index}"))).set_overscan(3)
  });

  let body = rendered.body();

  assert!(body.contains("row 0"));
  assert!(body.contains("row 2"));
  assert!(!body.contains("row 3\u{3c}"));
  assert!(!body.contains("row 500"));

  assert!(rendered.styles().contains("overflow:auto;"));
  assert!(rendered.styles().contains("height:24px;"));

  // The derived spacer heights serialize inline: 997 unbuilt items x 24px.
  assert!(body.contains("height:23928px;"));
}
