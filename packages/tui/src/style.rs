//! Cell styles to terminal layout and paint.
//!
//! Layout is taffy with `1.0 = 1 cell`. A style resolves in two steps: the conditional
//! variants overlay the base against the terminal environment (size, focus), then the
//! effective style maps to a taffy [`Style`] for layout and to paint attributes for
//! drawing.

use skene_core::{Color, Composable, Layout, Overflow};
use taffy::{
  prelude::{TaffyAuto, TaffyGridLine, TaffyGridSpan, fit_content, fr, length, minmax, percent},
  style::{Display, Style},
};

use crate::props::{BoxStyle, Condition, ControlStyle, RunStyle};

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
  base.aspect_ratio = over.aspect_ratio.or(base.aspect_ratio);
  base.margin = over.margin.or(base.margin);
  base.padding = over.padding.or(base.padding);
  base.offset = over.offset.or(base.offset);
  base.pinned = over.pinned.or(base.pinned);
  base.color = over.color.or(base.color);
  base.border = over.border.or(base.border);
  base.border_glyphs = over.border_glyphs.or(base.border_glyphs);
  base.spacing = over.spacing.or(base.spacing);
  base.cursor = over.cursor.or(base.cursor);
  base.overflow = over.overflow.or(base.overflow);
  base.shadow = over.shadow.or(base.shadow);
  base.wrap = over.wrap.or(base.wrap);
  base.flex = over.flex.or(base.flex);
  base.arrangement = over.arrangement.or(base.arrangement);
  base.item_alignment = over.item_alignment.or(base.item_alignment);
  base.line_arrangement = over.line_arrangement.or(base.line_arrangement);
  base.item_horizontal_alignment = over
    .item_horizontal_alignment
    .or(base.item_horizontal_alignment);
  base.self_alignment = over.self_alignment.or(base.self_alignment);
  base.self_horizontal_alignment = over
    .self_horizontal_alignment
    .or(base.self_horizontal_alignment);
  base.grid_flow = over.grid_flow.or(base.grid_flow);
  base.grid_row_placement = over.grid_row_placement.or(base.grid_row_placement);
  base.grid_column_placement = over.grid_column_placement.or(base.grid_column_placement);
  base.grid_columns = over.grid_columns.clone().or(base.grid_columns);
  base.grid_rows = over.grid_rows.clone().or(base.grid_rows);
  base.grid_implicit_columns = over
    .grid_implicit_columns
    .clone()
    .or(base.grid_implicit_columns);
  base.grid_implicit_rows = over.grid_implicit_rows.clone().or(base.grid_implicit_rows);

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
  let mut effective = base.clone();

  for (condition, style) in variants {
    if condition.holds(&|leaf| leaf_holds(leaf, environment, interaction)) {
      effective = overlay_box(effective, style);
    }
  }

  effective
}

/// The control style with every variant whose conditions hold overlaid, later entries
/// winning.
pub(crate) fn effective_control_style(
  base: &ControlStyle,
  variants: &[(Condition, ControlStyle)],
  environment: Environment,
  interaction: Interaction,
) -> ControlStyle {
  let mut effective = *base;

  for (condition, style) in variants {
    if condition.holds(&|leaf| leaf_holds(leaf, environment, interaction)) {
      effective.color = style.color.or(effective.color);
      effective.bold = style.bold.or(effective.bold);
      effective.cursor = style.cursor.or(effective.cursor);
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
      Layout::Flow => Display::Block,
      Layout::Flex { .. } => Display::Flex,
      Layout::Grid | Layout::Stack => Display::Grid,
    },
    ..Style::default()
  };

  if let Layout::Flex { direction } = layout {
    out.flex_direction = match direction {
      skene_core::FlexDirection::Row => taffy::style::FlexDirection::Row,
      skene_core::FlexDirection::Column => taffy::style::FlexDirection::Column,
      skene_core::FlexDirection::RowReverse => taffy::style::FlexDirection::RowReverse,
      skene_core::FlexDirection::ColumnReverse => taffy::style::FlexDirection::ColumnReverse,
    };
  }

  apply_box_sizing(&mut out, style, in_stack);

  out
}

/// A [`Dimension`](skene_core::Dimension) in cells as a taffy dimension.
fn dimension(value: skene_core::Dimension<u32>) -> taffy::style::Dimension {
  match value {
    skene_core::Dimension::Length(cells) => length(cell(cells)),
    skene_core::Dimension::Fraction(value) => percent(value),
    skene_core::Dimension::Auto => taffy::style::Dimension::AUTO,
  }
}

/// A [`Dimension`](skene_core::Dimension) in cells as a taffy auto-capable length.
fn dimension_lpa(value: skene_core::Dimension<u32>) -> taffy::style::LengthPercentageAuto {
  match value {
    skene_core::Dimension::Length(cells) => length(cell(cells)),
    skene_core::Dimension::Fraction(value) => percent(value),
    skene_core::Dimension::Auto => taffy::style::LengthPercentageAuto::AUTO,
  }
}

/// A dimension inset as a taffy rect.
fn dimension_rect(
  inset: skene_core::Inset<skene_core::Dimension<u32>>,
) -> taffy::geometry::Rect<taffy::style::LengthPercentageAuto> {
  taffy::geometry::Rect {
    left: dimension_lpa(inset.left),
    right: dimension_lpa(inset.right),
    top: dimension_lpa(inset.top),
    bottom: dimension_lpa(inset.bottom),
  }
}

/// One [`TrackLimit`](skene_core::TrackLimit) as a taffy min-track sizing function.
fn min_track(limit: skene_core::TrackLimit<u32>) -> taffy::style::MinTrackSizingFunction {
  match limit {
    skene_core::TrackLimit::Length(cells) => length(cell(cells)),
    skene_core::TrackLimit::Fraction(value) => percent(value),
    // A flexible share is meaningless as a minimum; taffy treats it as auto, as CSS
    // does.
    skene_core::TrackLimit::Flex(_) | skene_core::TrackLimit::Auto => {
      taffy::style::MinTrackSizingFunction::AUTO
    }
    skene_core::TrackLimit::MinContent => taffy::prelude::min_content(),
    skene_core::TrackLimit::MaxContent => taffy::prelude::max_content(),
  }
}

/// One [`TrackLimit`](skene_core::TrackLimit) as a taffy max-track sizing function.
fn max_track(limit: skene_core::TrackLimit<u32>) -> taffy::style::MaxTrackSizingFunction {
  match limit {
    skene_core::TrackLimit::Length(cells) => length(cell(cells)),
    skene_core::TrackLimit::Fraction(value) => percent(value),
    skene_core::TrackLimit::Flex(value) => fr(value),
    skene_core::TrackLimit::Auto => taffy::style::MaxTrackSizingFunction::AUTO,
    skene_core::TrackLimit::MinContent => taffy::prelude::min_content(),
    skene_core::TrackLimit::MaxContent => taffy::prelude::max_content(),
  }
}

/// One [`GridTrack`](skene_core::GridTrack) as a taffy track sizing function.
fn track(value: skene_core::GridTrack<u32>) -> taffy::style::TrackSizingFunction {
  match value {
    skene_core::GridTrack::Length(cells) => length(cell(cells)),
    skene_core::GridTrack::Fraction(value) => percent(value),
    skene_core::GridTrack::Flex(value) => fr(value),
    skene_core::GridTrack::Auto => taffy::style::TrackSizingFunction::AUTO,
    skene_core::GridTrack::MinContent => taffy::prelude::min_content(),
    skene_core::GridTrack::MaxContent => taffy::prelude::max_content(),
    skene_core::GridTrack::FitContent(cells) => fit_content(length(cell(cells))),
    skene_core::GridTrack::Minmax(min, max) => minmax(min_track(min), max_track(max)),
  }
}

/// A track list as a taffy template.
fn template(
  tracks: &[skene_core::GridTrack<u32>],
) -> Vec<taffy::style::GridTemplateComponent<String>> {
  tracks
    .iter()
    .map(|entry| taffy::style::GridTemplateComponent::Single(track(*entry)))
    .collect()
}

/// One [`GridPlacement`](skene_core::GridPlacement) as taffy's.
fn placement(value: skene_core::GridPlacement) -> taffy::style::GridPlacement<String> {
  match value {
    skene_core::GridPlacement::Auto => taffy::style::GridPlacement::Auto,
    skene_core::GridPlacement::Line(index) => taffy::style::GridPlacement::from_line_index(index),
    skene_core::GridPlacement::Span(count) => taffy::style::GridPlacement::from_span(count),
  }
}

/// An [`Arrangement`](skene_core::Arrangement) as taffy's content alignment.
fn arrangement(value: skene_core::Arrangement) -> taffy::style::AlignContent {
  match value {
    skene_core::Arrangement::Start => taffy::style::AlignContent::Start,
    skene_core::Arrangement::End => taffy::style::AlignContent::End,
    skene_core::Arrangement::Center => taffy::style::AlignContent::Center,
    skene_core::Arrangement::Stretch => taffy::style::AlignContent::Stretch,
    skene_core::Arrangement::SpaceBetween => taffy::style::AlignContent::SpaceBetween,
    skene_core::Arrangement::SpaceAround => taffy::style::AlignContent::SpaceAround,
    skene_core::Arrangement::SpaceEvenly => taffy::style::AlignContent::SpaceEvenly,
  }
}

/// An [`ItemAlignment`](skene_core::ItemAlignment) as taffy's item alignment.
fn item(value: skene_core::ItemAlignment) -> taffy::style::AlignItems {
  match value {
    skene_core::ItemAlignment::Start => taffy::style::AlignItems::Start,
    skene_core::ItemAlignment::End => taffy::style::AlignItems::End,
    skene_core::ItemAlignment::Center => taffy::style::AlignItems::Center,
    skene_core::ItemAlignment::Baseline => taffy::style::AlignItems::Baseline,
    skene_core::ItemAlignment::Stretch => taffy::style::AlignItems::Stretch,
  }
}

/// Folds the effective style's layout properties into `out`, shared by boxes and
/// inputs.
// One fold per property; length is the property count, not complexity.
#[expect(clippy::too_many_lines)]
pub(crate) fn apply_box_sizing(out: &mut Style, style: &BoxStyle, in_stack: bool) {
  out.size = taffy::geometry::Size {
    width: style.width.map_or(taffy::style::Dimension::AUTO, dimension),
    height: style
      .height
      .map_or(taffy::style::Dimension::AUTO, dimension),
  };

  out.min_size = taffy::geometry::Size {
    width: style
      .min_width
      .map_or(taffy::style::Dimension::AUTO, dimension),
    height: style
      .min_height
      .map_or(taffy::style::Dimension::AUTO, dimension),
  };

  out.max_size = taffy::geometry::Size {
    width: style
      .max_width
      .map_or(taffy::style::Dimension::AUTO, dimension),
    height: style
      .max_height
      .map_or(taffy::style::Dimension::AUTO, dimension),
  };

  out.aspect_ratio = style.aspect_ratio;

  if let Some(margin) = style.margin {
    out.margin = dimension_rect(margin);
  }

  if let Some(padding) = style.padding {
    out.padding = taffy::geometry::Rect {
      left: length(cell(padding.left)),
      right: length(cell(padding.right)),
      top: length(cell(padding.top)),
      bottom: length(cell(padding.bottom)),
    };
  }

  // A pinned box leaves the flow; an offset one shifts within it. Pinned wins when
  // both are set.
  if let Some(pinned) = style.pinned {
    out.position = taffy::style::Position::Absolute;
    out.inset = dimension_rect(pinned);
  }
  else if let Some(offset) = style.offset {
    out.position = taffy::style::Position::Relative;
    out.inset = dimension_rect(offset);
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
      width: length(cell(spacing.column)),
      height: length(cell(spacing.row)),
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

  if let Some(wrap) = style.wrap {
    out.flex_wrap = match wrap {
      skene_core::Wrap::None => taffy::style::FlexWrap::NoWrap,
      skene_core::Wrap::Forward => taffy::style::FlexWrap::Wrap,
      skene_core::Wrap::Reverse => taffy::style::FlexWrap::WrapReverse,
    };
  }

  if let Some(flex) = style.flex {
    out.flex_basis = dimension(flex.initial);
    out.flex_grow = flex.grow_factor;
    out.flex_shrink = flex.shrink_factor;
  }

  out.justify_content = style.arrangement.map(arrangement);
  out.align_items = style.item_alignment.map(item);
  out.align_content = style.line_arrangement.map(arrangement);
  out.justify_items = style.item_horizontal_alignment.map(item);
  out.align_self = style.self_alignment.map(item);
  out.justify_self = style.self_horizontal_alignment.map(item);

  if let Some(tracks) = &style.grid_columns {
    out.grid_template_columns = template(tracks);
  }

  if let Some(tracks) = &style.grid_rows {
    out.grid_template_rows = template(tracks);
  }

  if let Some(tracks) = &style.grid_implicit_columns {
    out.grid_auto_columns = tracks.iter().map(|entry| track(*entry)).collect();
  }

  if let Some(tracks) = &style.grid_implicit_rows {
    out.grid_auto_rows = tracks.iter().map(|entry| track(*entry)).collect();
  }

  if let Some(flow) = style.grid_flow {
    out.grid_auto_flow = match flow {
      skene_core::GridFlow::Rows => taffy::style::GridAutoFlow::Row,
      skene_core::GridFlow::Columns => taffy::style::GridAutoFlow::Column,
      skene_core::GridFlow::DenseRows => taffy::style::GridAutoFlow::RowDense,
      skene_core::GridFlow::DenseColumns => taffy::style::GridAutoFlow::ColumnDense,
    };
  }

  if let Some(row) = style.grid_row_placement {
    out.grid_row = taffy::geometry::Line {
      start: placement(row.start),
      end: placement(row.end),
    };
  }

  if let Some(column) = style.grid_column_placement {
    out.grid_column = taffy::geometry::Line {
      start: placement(column.start),
      end: placement(column.end),
    };
  }

  if in_stack {
    out.grid_row = taffy::geometry::Line {
      start: taffy::style::GridPlacement::from_line_index(1),
      end: taffy::style::GridPlacement::Auto,
    };

    out.grid_column = taffy::geometry::Line {
      start: taffy::style::GridPlacement::from_line_index(1),
      end: taffy::style::GridPlacement::Auto,
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
