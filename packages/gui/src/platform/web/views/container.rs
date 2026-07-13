//! Web realization of [`Container`].

use skene_core::{Axis, NodeId, View};

use super::super::{
  Declarations,
  Styler,
  create_element,
  handle_click,
  override_style,
  set_attribute,
};
use crate::{Border, Container, Layout, Role};

/// The static display declarations for a [`Layout`].
#[inline(always)]
fn layout_declarations(layout: Layout) -> Declarations {
  match layout {
    Layout::Block => Declarations::new().set("display", "flow-root"),
    Layout::Flex { direction } => Declarations::new().set("display", "flex").set(
      "flex-direction",
      match direction {
        Axis::Horizontal => "row",
        Axis::Vertical => "column",
      },
    ),
    Layout::Grid { columns } => Declarations::new()
      .set("display", "grid")
      .set("grid-template-columns", format!("repeat({columns}, 1fr)")),
    Layout::Stack => Declarations::new().set("display", "grid"),
  }
}

impl View for Container {
  fn build(self) -> NodeId {
    let node = create_element(self.role.map_or("div", |Role::Button| "button"));

    if matches!(self.role, Some(Role::Button)) {
      set_attribute(node, "type", "button");
    }

    Styler::new(node, layout_declarations(self.layout))
      .prop(self.width, |declarations, value| {
        declarations.set("width", format!("{value}px"))
      })
      .prop(self.height, |declarations, value| {
        declarations.set("height", format!("{value}px"))
      })
      .prop(self.min_width, |declarations, value| {
        declarations.set("min-width", format!("{value}px"))
      })
      .prop(self.min_height, |declarations, value| {
        declarations.set("min-height", format!("{value}px"))
      })
      .prop(self.max_width, |declarations, value| {
        declarations.set("max-width", format!("{value}px"))
      })
      .prop(self.max_height, |declarations, value| {
        declarations.set("max-height", format!("{value}px"))
      })
      .prop(self.padding, |declarations, value| {
        declarations.set(
          "padding",
          format!(
            "{}px {}px {}px {}px",
            value.top, value.right, value.bottom, value.left,
          ),
        )
      })
      .prop(self.color, |declarations, value| {
        declarations.set("background-color", Declarations::render_color(value))
      })
      .prop(self.border, |declarations, value: Border| {
        declarations
          .set("border-width", format!("{}px", value.width))
          .set("border-style", "solid")
          .set("border-color", Declarations::render_color(value.color))
      })
      .prop(self.border_radius, |declarations, value| {
        declarations.set("border-radius", format!("{value}px"))
      })
      .prop(self.cursor, |declarations, value| {
        declarations.set("cursor", Declarations::render_cursor(value))
      })
      .prop(self.spacing, |declarations, value| {
        declarations.set("gap", format!("{value}px"))
      })
      .prop(self.main_axis_alignment, |declarations, value| {
        declarations.set("justify-content", Declarations::render_main_axis(value))
      })
      .prop(self.cross_axis_alignment, |declarations, value| {
        declarations.set("align-items", Declarations::render_cross_axis(value))
      })
      .finish();

    if let Some(action) = self.click_handler {
      handle_click(node, action);
    }

    match self.layout {
      // Overlap the children in one grid cell.
      Layout::Stack => self.children.mount_with(node, |child| {
        override_style(child, &Declarations::new().set("grid-area", "1 / 1"));

        child
      }),
      _ => self.children.mount(node),
    }

    node
  }
}
