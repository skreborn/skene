//! Cell styles to terminal layout and paint.
//!
//! Layout is taffy with `1.0 = 1 cell`. A style resolves in two steps: the conditional
//! variants overlay the base against the terminal environment (size, focus), then the
//! effective style maps to a taffy [`Style`] for layout and to paint attributes for
//! drawing.

use skene_core::{
  Axis,
  Color,
  Composable,
  CrossAxisAlignment,
  Layout,
  MainAxisAlignment,
  Overflow,
};
use taffy::{
  prelude::{TaffyAuto, TaffyGridLine, fr, length, repeat},
  style::{AlignItems, Dimension, Display, FlexDirection, GridPlacement, JustifyContent, Style},
};

use crate::props::{BoxStyle, Condition, RunStyle};

/// The terminal environment conditions resolve against.
#[derive(Clone, Copy)]
pub(crate) struct Environment {
  /// The terminal width, in cells.
  pub(crate) columns: u16,
  /// The terminal height, in cells.
  pub(crate) rows: u16,
  /// Whether the terminal background is dark.
  pub(crate) dark: bool,
}

/// A node's interaction state, resolved by the driver.
#[derive(Clone, Copy, Default)]
pub(crate) struct Interaction {
  /// Whether the node has keyboard focus.
  pub(crate) focused: bool,
  /// Whether the mouse hovers the node (itself or a descendant).
  pub(crate) hovered: bool,
}

/// Whether leaf `condition` holds in `environment` for a node in `interaction`.
fn leaf_holds(condition: &Condition, environment: Environment, interaction: Interaction) -> bool {
  match condition {
    Condition::MinWidth(value) => u32::from(environment.columns) >= *value,
    Condition::MaxWidth(value) => u32::from(environment.columns) <= *value,
    Condition::MinHeight(value) => u32::from(environment.rows) >= *value,
    Condition::MaxHeight(value) => u32::from(environment.rows) <= *value,
    Condition::Focused => interaction.focused,
    Condition::Hovered => interaction.hovered,
    Condition::DarkMode => environment.dark,
    Condition::LightMode => !environment.dark,
    Condition::And(_) | Condition::Or(_) => unreachable!("a composite is never a leaf"),
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
  base.spacing = over.spacing.or(base.spacing);
  base.cursor = over.cursor.or(base.cursor);
  base.overflow = over.overflow.or(base.overflow);
  base.main_axis_alignment = over.main_axis_alignment.or(base.main_axis_alignment);
  base.cross_axis_alignment = over.cross_axis_alignment.or(base.cross_axis_alignment);

  base
}

/// The box style with every variant whose conditions hold overlaid, later entries
/// winning.
pub(crate) fn effective_box_style(
  base: &BoxStyle,
  variants: &[(Condition, BoxStyle)],
  environment: Environment,
  interaction: Interaction,
) -> BoxStyle {
  let mut effective = *base;

  for (condition, style) in variants {
    if condition.holds(&|leaf| leaf_holds(leaf, environment, interaction)) {
      effective = overlay_box(effective, style);
    }
  }

  effective
}

/// The run style with every variant whose conditions hold overlaid, later entries
/// winning.
pub(crate) fn effective_run_style(
  base: &RunStyle,
  variants: &[(Condition, RunStyle)],
  environment: Environment,
  interaction: Interaction,
) -> RunStyle {
  let mut effective = *base;

  for (condition, style) in variants {
    if condition.holds(&|leaf| leaf_holds(leaf, environment, interaction)) {
      effective.color = style.color.or(effective.color);
      effective.bold = style.bold.or(effective.bold);
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
    width: style.width.map_or(Dimension::AUTO, cells),
    height: style.height.map_or(Dimension::AUTO, cells),
  };

  out.min_size = taffy::geometry::Size {
    width: style.min_width.map_or(Dimension::AUTO, cells),
    height: style.min_height.map_or(Dimension::AUTO, cells),
  };

  out.max_size = taffy::geometry::Size {
    width: style.max_width.map_or(Dimension::AUTO, cells),
    height: style.max_height.map_or(Dimension::AUTO, cells),
  };

  if let Some(padding) = style.padding {
    out.padding = taffy::geometry::Rect {
      left: length(cell(padding.left)),
      right: length(cell(padding.right)),
      top: length(cell(padding.top)),
      bottom: length(cell(padding.bottom)),
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
      width: length(cell(spacing)),
      height: length(cell(spacing)),
    };
  }

  if let Some(overflow) = style.overflow {
    let mapped = match overflow {
      Overflow::Visible => taffy::style::Overflow::Visible,
      Overflow::Hidden => taffy::style::Overflow::Hidden,
      Overflow::Scroll | Overflow::Auto => taffy::style::Overflow::Scroll,
    };

    out.overflow = taffy::geometry::Point {
      x: mapped,
      y: mapped,
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

/// A cell length as layout units.
#[inline(always)]
fn cell(value: u32) -> f32 {
  #[expect(clippy::cast_precision_loss)]
  {
    value as f32
  }
}

/// A cell length as a taffy dimension.
#[inline(always)]
fn cells(value: u32) -> Dimension {
  length(cell(value))
}

/// A [`Color`]'s terminal RGB, or `None` when fully transparent.
pub(crate) fn terminal_rgb(color: Color) -> Option<(u8, u8, u8)> {
  let [red, green, blue, alpha] = color.to_rgba();

  if alpha <= 0.0 {
    return None;
  }

  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  Some((
    (red.clamp(0.0, 1.0) * 255.0).round() as u8,
    (green.clamp(0.0, 1.0) * 255.0).round() as u8,
    (blue.clamp(0.0, 1.0) * 255.0).round() as u8,
  ))
}
