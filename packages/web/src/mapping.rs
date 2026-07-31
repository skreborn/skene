//! Typed properties to CSS declarations: the web realization of the driver vocabulary.
//!
//! One property maps to the same declaration text whether it is folded into a box's
//! interned class or applied alone as an inline override, so a value renders identically
//! on both paths and on both leaves.

use skene_core::{Axis, BoxProp, BoxProps, Layout, RunDisplay, RunProp, RunProps};

use crate::Declarations;

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

/// Folds one box property into `declarations`.
fn apply_box_prop(declarations: Declarations, prop: &BoxProp) -> Declarations {
  match prop {
    BoxProp::Width(value) => declarations.set("width", format!("{value}px")),
    BoxProp::Height(value) => declarations.set("height", format!("{value}px")),
    BoxProp::MinWidth(value) => declarations.set("min-width", format!("{value}px")),
    BoxProp::MinHeight(value) => declarations.set("min-height", format!("{value}px")),
    BoxProp::MaxWidth(value) => declarations.set("max-width", format!("{value}px")),
    BoxProp::MaxHeight(value) => declarations.set("max-height", format!("{value}px")),
    BoxProp::Padding(value) => declarations.set(
      "padding",
      format!(
        "{}px {}px {}px {}px",
        value.top, value.right, value.bottom, value.left,
      ),
    ),
    BoxProp::Color(value) => {
      declarations.set("background-color", Declarations::render_color(*value))
    }
    BoxProp::Border(value) => declarations
      .set("border-width", format!("{}px", value.width))
      .set("border-style", "solid")
      .set("border-color", Declarations::render_color(value.color)),
    BoxProp::BorderRadius(value) => declarations.set("border-radius", format!("{value}px")),
    BoxProp::Cursor(value) => declarations.set("cursor", Declarations::render_cursor(*value)),
    BoxProp::Spacing(value) => declarations.set("gap", format!("{value}px")),
    BoxProp::MainAxisAlignment(value) => {
      declarations.set("justify-content", Declarations::render_main_axis(*value))
    }
    BoxProp::CrossAxisAlignment(value) => {
      declarations.set("align-items", Declarations::render_cross_axis(*value))
    }
  }
}

/// The full static declaration set for a box: its layout plus every fixed property.
pub(crate) fn box_declarations(props: &BoxProps) -> Declarations {
  let mut declarations = layout_declarations(props.layout);
  let style = &props.style;

  if let Some(value) = style.width {
    declarations = apply_box_prop(declarations, &BoxProp::Width(value));
  }

  if let Some(value) = style.height {
    declarations = apply_box_prop(declarations, &BoxProp::Height(value));
  }

  if let Some(value) = style.min_width {
    declarations = apply_box_prop(declarations, &BoxProp::MinWidth(value));
  }

  if let Some(value) = style.min_height {
    declarations = apply_box_prop(declarations, &BoxProp::MinHeight(value));
  }

  if let Some(value) = style.max_width {
    declarations = apply_box_prop(declarations, &BoxProp::MaxWidth(value));
  }

  if let Some(value) = style.max_height {
    declarations = apply_box_prop(declarations, &BoxProp::MaxHeight(value));
  }

  if let Some(value) = style.padding {
    declarations = apply_box_prop(declarations, &BoxProp::Padding(value));
  }

  if let Some(value) = style.color {
    declarations = apply_box_prop(declarations, &BoxProp::Color(value));
  }

  if let Some(value) = style.border {
    declarations = apply_box_prop(declarations, &BoxProp::Border(value));
  }

  if let Some(value) = style.border_radius {
    declarations = apply_box_prop(declarations, &BoxProp::BorderRadius(value));
  }

  if let Some(value) = style.cursor {
    declarations = apply_box_prop(declarations, &BoxProp::Cursor(value));
  }

  if let Some(value) = style.spacing {
    declarations = apply_box_prop(declarations, &BoxProp::Spacing(value));
  }

  if let Some(value) = style.main_axis_alignment {
    declarations = apply_box_prop(declarations, &BoxProp::MainAxisAlignment(value));
  }

  if let Some(value) = style.cross_axis_alignment {
    declarations = apply_box_prop(declarations, &BoxProp::CrossAxisAlignment(value));
  }

  declarations
}

/// The declarations for one box property alone, for an inline override.
#[inline(always)]
pub(crate) fn box_prop_declarations(prop: &BoxProp) -> Declarations {
  apply_box_prop(Declarations::new(), prop)
}

/// Folds one text-run property into `declarations`.
fn apply_run_prop(declarations: Declarations, prop: &RunProp) -> Declarations {
  match prop {
    RunProp::Color(value) => declarations.set("color", Declarations::render_color(*value)),
    RunProp::FontSize(value) => declarations.set("font-size", format!("{value}px")),
    RunProp::FontWeight(value) => declarations.set("font-weight", value.to_string()),
  }
}

/// The full static declaration set for a text run: its display plus every fixed property.
pub(crate) fn run_declarations(props: &RunProps) -> Declarations {
  let mut declarations = match props.display {
    RunDisplay::Block => Declarations::new().set("display", "block"),
    RunDisplay::Inline => Declarations::new(),
    RunDisplay::InlineBox => Declarations::new().set("display", "inline-block"),
  };
  let style = &props.style;

  if let Some(value) = style.color {
    declarations = apply_run_prop(declarations, &RunProp::Color(value));
  }

  if let Some(value) = style.font_size {
    declarations = apply_run_prop(declarations, &RunProp::FontSize(value));
  }

  if let Some(value) = style.font_weight {
    declarations = apply_run_prop(declarations, &RunProp::FontWeight(value));
  }

  declarations
}

/// The declarations for one text-run property alone, for an inline override.
#[inline(always)]
pub(crate) fn run_prop_declarations(prop: &RunProp) -> Declarations {
  apply_run_prop(Declarations::new(), prop)
}
