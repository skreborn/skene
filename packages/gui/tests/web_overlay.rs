//! Server rendering of overlays: top-layer elements serialized closed.

#![cfg(skene_leaf = "server")]

use skene_gui::{
  Container,
  HasChild,
  Inset,
  Overlay,
  Text,
  Tooltip,
  core::{Color, Placement},
  render_to_string,
};

/// The class of the first classed element in `body`.
fn first_class(body: &str) -> &str {
  let start = body.find("class=\"").expect("a class") + 7;
  let end = body[start..].find('"').expect("a closing quote") + start;

  &body[start..end]
}

/// A modal serializes as a closed dialog carrying its placement margins and its
/// backdrop rule.
#[test]
fn modal_serializes_as_a_closed_dialog() {
  let rendered = render_to_string(|| {
    Overlay::modal()
      .set_backdrop(Some(Color::srgb(0., 0., 0.)))
      .set_padding(Inset::all(16.))
      .set_child(Text::new("dialog content"))
  });

  let body = rendered.body();

  assert!(body.starts_with("<dialog "));
  assert!(!body.contains("open"));
  assert!(body.contains("dialog content"));
  assert!(body.ends_with("</dialog>"));

  let class = first_class(body);

  assert!(
    rendered.styles().contains(&format!(
      ".{class}{{background:none;border:0 solid;color:inherit;margin:auto;overflow:visible;\
       padding:16px 16px 16px 16px;}}"
    )),
    "{}",
    rendered.styles()
  );
  assert!(
    rendered.styles().contains(&format!(
      ".{class}:where([open],:popover-open){{display:grid;grid-template-columns:1fr;\
       grid-template-rows:1fr;}}"
    )),
    "the layout applies only while open: {}",
    rendered.styles()
  );

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

/// A text tooltip serializes as the described element's `title`, the scriptless
/// tooltip, with the bubble a closed hint popover the element names as its
/// description.
#[test]
fn a_tooltip_serializes_its_title_and_a_closed_hint() {
  let rendered = render_to_string(|| {
    Tooltip::new("Saves your work").set_child(Container::new().set_child(Text::new("Save")))
  });
  let body = rendered.body();
  let described = body.find("title=\"Saves your work\"").expect("the title");
  let reference = body.find("aria-describedby=\"").expect("the description") + 18;
  let id = &body[reference..reference + body[reference..].find('"').expect("a closing quote")];

  assert!(id.starts_with("skene-"), "{body}");
  assert!(
    body.contains(&format!("id=\"{id}\"")),
    "the bubble carries the id: {body}"
  );
  assert!(body.contains("popover=\"hint\""), "{body}");
  assert!(!body.contains(" open"), "{body}");
  assert!(
    body.find("popover=\"hint\"").expect("the bubble") > described,
    "the bubble follows the element it describes: {body}"
  );
  assert!(
    body.contains("Saves your work</"),
    "the bubble holds the text: {body}"
  );
}
