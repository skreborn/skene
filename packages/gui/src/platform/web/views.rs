//! Web realization of the built-in views.

use skene_core::NodeId;

mod container;
mod rich_text;
mod span;
mod text;

use super::{Declarations, Styler, create_element, set_attribute};
use crate::{TextStyle, text::resolved_text_style};

/// Creates a text run's element — an `<a>` when linked, else a `<span>` — applies the resolved
/// text style over `initial`, and returns the node with that resolved style, which a group
/// cascades to its children.
fn open_run(link: Option<&str>, style: TextStyle, initial: Declarations) -> (NodeId, TextStyle) {
  let node = match link {
    Some(href) => {
      let node = create_element("a");

      set_attribute(node, "href", href);

      node
    }
    None => create_element("span"),
  };

  let resolved = resolved_text_style(style, link.is_some());

  Styler::new(node, initial)
    .prop(resolved.color.clone(), |declarations, value| {
      declarations.set("color", Declarations::render_color(value))
    })
    .prop(resolved.font_size.clone(), |declarations, value| {
      declarations.set("font-size", format!("{value}px"))
    })
    .prop(resolved.font_weight.clone(), |declarations, value| {
      declarations.set("font-weight", value.to_string())
    })
    .finish();

  (node, resolved)
}
