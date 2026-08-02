//! Server rendering of overlays: top-layer elements serialized closed.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene_gui::{
  HasChildren,
  Overlay,
  Text,
  core::{BoxStyle, Color, Placement},
  render_to_string,
};

/// A modal serializes as a closed dialog carrying its placement margins and its
/// backdrop rule.
#[test]
fn modal_serializes_as_a_closed_dialog() {
  let rendered = render_to_string(|| {
    Overlay::modal()
      .set_backdrop(Color::srgb(0., 0., 0.))
      .set_style(BoxStyle::new().set_padding(skene_gui::Inset::all(16.)))
      .set_child(Text::new("dialog content"))
  });

  let body = rendered.body();

  assert!(body.starts_with("<dialog "));
  assert!(!body.contains("open"));
  assert!(body.contains("dialog content"));
  assert!(body.ends_with("</dialog>"));

  let start = body.find("class=\"").expect("a class") + 7;
  let end = body[start..].find('"').expect("a closing quote") + start;
  let class = &body[start..end];

  assert!(rendered.styles().contains(&format!(
    ".{class}{{border:none;padding:0;background:none;color:inherit;overflow:visible;\
     margin:auto;padding:16px 16px 16px 16px;}}"
  )));

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}::backdrop{{background-color:"))
  );
}

/// A popup serializes as a manual popover, its placement in the margins.
#[test]
fn popup_serializes_as_a_manual_popover() {
  let rendered = render_to_string(|| {
    Overlay::popup()
      .set_placement(Placement::BottomEnd)
      .set_child(Text::new("menu"))
  });

  let body = rendered.body();

  assert!(body.starts_with("<div "));
  assert!(body.contains("popover=\"manual\""));
  assert!(body.contains("menu"));

  assert!(rendered.styles().contains("margin:auto 0 0 auto;"));
  assert!(!rendered.styles().contains("::backdrop"));
}

/// A modal ships a half-transparent black backdrop without being asked.
#[test]
fn modal_carries_a_default_backdrop() {
  let rendered = render_to_string(|| Overlay::modal().set_child(Text::new("dialog content")));

  assert!(rendered.styles().contains("::backdrop{background-color:"));
}
