//! Server rendering of the virtual list: an unmeasured viewport serializes only the
//! overscan margin, the rest as spacer extent.

#![cfg(skene_gui_platform = "server")]

use skene_gui::{LazyColumn, LazyRow, Text, render_to_string};

/// A server render carries the scroll box, the overscan slice, and a trailing spacer
/// covering every unbuilt item.
#[test]
fn virtual_list_serializes_the_overscan_slice() {
  let rendered = render_to_string(|| {
    LazyColumn::new()
      .set_item_height(24.)
      .set_overscan(3)
      .derive_items(
        || 0..1000,
        |index| *index,
        |index| Text::new(format!("row {index}")),
      )
  });

  let body = rendered.body();

  assert!(body.contains("row 0"));
  assert!(body.contains("row 2"));
  assert!(!body.contains("row 3\u{3c}"));
  assert!(!body.contains("row 500"));

  assert!(rendered.styles().contains("overflow:auto;"));
  assert!(rendered.styles().contains("height:24px;"));

  // The derived spacer heights serialize into their slots: 997 unbuilt items x 24px.
  assert!(body.contains("--skene-height:23928px;"), "{body}");
}

/// A lazy row serializes the same way along the other axis: the overscan slice, item
/// widths, and a trailing spacer width covering every unbuilt item.
#[test]
fn virtual_row_serializes_the_overscan_slice() {
  let rendered = render_to_string(|| {
    LazyRow::new()
      .set_item_width(24.)
      .set_overscan(3)
      .derive_items(
        || 0..1000,
        |index| *index,
        |index| Text::new(format!("cell {index}")),
      )
  });

  let body = rendered.body();

  assert!(body.contains("cell 2"));
  assert!(!body.contains("cell 3\u{3c}"));
  assert!(rendered.styles().contains("width:24px;"));
  assert!(body.contains("--skene-width:23928px;"), "{body}");
}
