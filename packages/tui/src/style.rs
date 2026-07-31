//! Typed properties to terminal layout and paint: the TUI realization of the driver
//! vocabulary.
//!
//! Layout is taffy with `1.0 = 1 cell`. A style resolves in two steps: the conditional
//! variants overlay the base against the terminal environment (size, focus), then the
//! effective style maps to a taffy [`Style`] for layout and to paint attributes for
//! drawing.

use skene_core::{
  Axis,
  BoxStyle,
  Color,
  Condition,
  CrossAxisAlignment,
  Layout,
  MainAxisAlignment,
  RunStyle,
};
use taffy::{
  prelude::{TaffyAuto, TaffyGridLine, fr, length, repeat},
  style::{AlignItems, Dimension, Display, FlexDirection, GridPlacement, JustifyContent, Style},
};

/// The terminal environment conditions resolve against.
#[derive(Clone, Copy)]
pub(crate) struct Environment {
  /// The terminal width, in cells.
  pub(crate) columns: f32,
  /// The terminal height, in cells.
  pub(crate) rows: f32,
}

/// Whether `condition` holds in `environment` for a node with the given focus state.
///
/// Pointer states never hold (the terminal has no pointer yet); a dark color scheme is
/// assumed, matching the common terminal.
fn condition_holds(condition: Condition, environment: Environment, focused: bool) -> bool {
  match condition {
    Condition::MinWidth(value) => environment.columns >= value,
    Condition::MaxWidth(value) => environment.columns <= value,
    Condition::MinHeight(value) => environment.rows >= value,
    Condition::MaxHeight(value) => environment.rows <= value,
    Condition::DarkMode => true,
    Condition::Focused | Condition::FocusVisible => focused,
    Condition::LightMode
    | Condition::ReducedMotion
    | Condition::Hovered
    | Condition::Pressed
    | Condition::Disabled => false,
  }
}

/// Overlays each set property of `over` onto `base`, `over` winning.
fn overlay_box(mut base: BoxStyle, over: &BoxStyle) -> BoxStyle {
  base.width = over.width.or(base.width);
  base.height = over.height.or(base.height);
  base.min_width = over.min_width.or(base.min_width);
  base.min_height = over.min_height.or(base.min_height);
  base.max_width = over.max_width.or(base.max_width);
  base.max_height = over.max_height.or(base.max_height);
  base.padding = over.padding.or(base.padding);
  base.color = over.color.or(base.color);
  base.border = over.border.or(base.border);
  base.border_radius = over.border_radius.or(base.border_radius);
  base.cursor = over.cursor.or(base.cursor);
  base.spacing = over.spacing.or(base.spacing);
  base.main_axis_alignment = over.main_axis_alignment.or(base.main_axis_alignment);
  base.cross_axis_alignment = over.cross_axis_alignment.or(base.cross_axis_alignment);

  base
}

/// The box style with every variant whose conditions hold overlaid, later entries
/// winning.
pub(crate) fn effective_box_style(
  base: &BoxStyle,
  variants: &[(Vec<Condition>, BoxStyle)],
  environment: Environment,
  focused: bool,
) -> BoxStyle {
  let mut effective = *base;

  for (conditions, style) in variants {
    if conditions
      .iter()
      .all(|condition| condition_holds(*condition, environment, focused))
    {
      effective = overlay_box(effective, style);
    }
  }

  effective
}

/// The run style with every variant whose conditions hold overlaid, later entries
/// winning.
pub(crate) fn effective_run_style(
  base: &RunStyle,
  variants: &[(Vec<Condition>, RunStyle)],
  environment: Environment,
  focused: bool,
) -> RunStyle {
  let mut effective = *base;

  for (conditions, style) in variants {
    if conditions
      .iter()
      .all(|condition| condition_holds(*condition, environment, focused))
    {
      effective.color = style.color.or(effective.color);
      effective.font_size = style.font_size.or(effective.font_size);
      effective.font_weight = style.font_weight.or(effective.font_weight);
    }
  }

  effective
}

/// The taffy style for a box: its layout mode plus the effective style's sizing.
pub(crate) fn box_taffy_style(layout: Layout, style: &BoxStyle, in_stack: bool) -> Style {
  let mut out = Style {
    display: match layout {
      Layout::Block => Display::Block,
      Layout::Flex { .. } => Display::Flex,
      Layout::Grid { .. } | Layout::Stack => Display::Grid,
    },
    ..Style::default()
  };

  if let Layout::Flex { direction } = layout {
    out.flex_direction = match direction {
      Axis::Horizontal => FlexDirection::Row,
      Axis::Vertical => FlexDirection::Column,
    };
  }

  if let Layout::Grid { columns } = layout {
    out.grid_template_columns = vec![repeat(columns, vec![fr(1.0)])];
  }

  apply_box_sizing(&mut out, style, in_stack);

  out
}

/// Folds the effective style's sizing into `out`, shared by boxes and inputs.
pub(crate) fn apply_box_sizing(out: &mut Style, style: &BoxStyle, in_stack: bool) {
  out.size = taffy::geometry::Size {
    width: style.width.map_or(Dimension::AUTO, length),
    height: style.height.map_or(Dimension::AUTO, length),
  };

  out.min_size = taffy::geometry::Size {
    width: style.min_width.map_or(Dimension::AUTO, length),
    height: style.min_height.map_or(Dimension::AUTO, length),
  };

  out.max_size = taffy::geometry::Size {
    width: style.max_width.map_or(Dimension::AUTO, length),
    height: style.max_height.map_or(Dimension::AUTO, length),
  };

  if let Some(padding) = style.padding {
    out.padding = taffy::geometry::Rect {
      left: length(padding.left),
      right: length(padding.right),
      top: length(padding.top),
      bottom: length(padding.bottom),
    };
  }

  if style.border.is_some() {
    out.border = taffy::geometry::Rect {
      left: length(1.0),
      right: length(1.0),
      top: length(1.0),
      bottom: length(1.0),
    };
  }

  if let Some(spacing) = style.spacing {
    out.gap = taffy::geometry::Size {
      width: length(spacing),
      height: length(spacing),
    };
  }

  out.justify_content = style.main_axis_alignment.map(|alignment| match alignment {
    MainAxisAlignment::Start => JustifyContent::FlexStart,
    MainAxisAlignment::Center => JustifyContent::Center,
    MainAxisAlignment::End => JustifyContent::FlexEnd,
    MainAxisAlignment::SpaceBetween => JustifyContent::SpaceBetween,
    MainAxisAlignment::SpaceAround => JustifyContent::SpaceAround,
    MainAxisAlignment::SpaceEvenly => JustifyContent::SpaceEvenly,
  });

  out.align_items = style.cross_axis_alignment.map(|alignment| match alignment {
    CrossAxisAlignment::Start => AlignItems::FlexStart,
    CrossAxisAlignment::End => AlignItems::FlexEnd,
    CrossAxisAlignment::Center => AlignItems::Center,
    CrossAxisAlignment::Stretch => AlignItems::Stretch,
    CrossAxisAlignment::Baseline => AlignItems::Baseline,
  });

  if in_stack {
    out.grid_row = taffy::geometry::Line {
      start: GridPlacement::from_line_index(1),
      end: GridPlacement::Auto,
    };

    out.grid_column = taffy::geometry::Line {
      start: GridPlacement::from_line_index(1),
      end: GridPlacement::Auto,
    };
  }
}

/// A [`Color`]'s terminal RGB, or `None` when fully transparent.
pub(crate) fn terminal_rgb(color: Color) -> Option<(u8, u8, u8)> {
  let [red, green, blue, alpha] = color.to_rgba();

  if alpha <= 0.0 {
    return None;
  }

  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  Some((
    (red.clamp(0.0, 1.0) * 255.0) as u8,
    (green.clamp(0.0, 1.0) * 255.0) as u8,
    (blue.clamp(0.0, 1.0) * 255.0) as u8,
  ))
}

/// Whether a font weight paints bold.
pub(crate) fn is_bold(weight: Option<f32>) -> bool {
  weight.is_some_and(|weight| weight >= 600.0)
}
