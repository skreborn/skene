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
  base.position = over.position.or(base.position);
  base.inset = over.inset.or(base.inset);
  base.color = over.color.or(base.color);
  base.border = over.border.or(base.border);
  base.row_spacing = over.row_spacing.or(base.row_spacing);
  base.column_spacing = over.column_spacing.or(base.column_spacing);
  base.cursor = over.cursor.or(base.cursor);
  base.overflow = over.overflow.or(base.overflow);
  base.shadow = over.shadow.or(base.shadow);
  base.flex_wrap = over.flex_wrap.or(base.flex_wrap);
  base.flex_grow = over.flex_grow.or(base.flex_grow);
  base.flex_shrink = over.flex_shrink.or(base.flex_shrink);
  base.flex_basis = over.flex_basis.or(base.flex_basis);
  base.main_axis_alignment = over.main_axis_alignment.or(base.main_axis_alignment);
  base.cross_axis_alignment = over.cross_axis_alignment.or(base.cross_axis_alignment);
  base.line_alignment = over.line_alignment.or(base.line_alignment);
  base.main_axis_item_alignment = over
    .main_axis_item_alignment
    .or(base.main_axis_item_alignment);
  base.self_cross_axis_alignment = over
    .self_cross_axis_alignment
    .or(base.self_cross_axis_alignment);
  base.self_main_axis_alignment = over
    .self_main_axis_alignment
    .or(base.self_main_axis_alignment);
  base.grid_auto_flow = over.grid_auto_flow.or(base.grid_auto_flow);
  base.grid_row = over.grid_row.or(base.grid_row);
  base.grid_column = over.grid_column.or(base.grid_column);
  base.grid_template_columns = over
    .grid_template_columns
    .clone()
    .or(base.grid_template_columns);
  base.grid_template_rows = over.grid_template_rows.clone().or(base.grid_template_rows);
  base.grid_auto_columns = over.grid_auto_columns.clone().or(base.grid_auto_columns);
  base.grid_auto_rows = over.grid_auto_rows.clone().or(base.grid_auto_rows);

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
      Layout::Block => Display::Block,
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

/// A [`MainAxisAlignment`](skene_core::MainAxisAlignment) as taffy's content
/// alignment.
fn main_axis(value: skene_core::MainAxisAlignment) -> taffy::style::AlignContent {
  match value {
    skene_core::MainAxisAlignment::Start => taffy::style::AlignContent::Start,
    skene_core::MainAxisAlignment::End => taffy::style::AlignContent::End,
    skene_core::MainAxisAlignment::Center => taffy::style::AlignContent::Center,
    skene_core::MainAxisAlignment::SpaceBetween => taffy::style::AlignContent::SpaceBetween,
    skene_core::MainAxisAlignment::SpaceAround => taffy::style::AlignContent::SpaceAround,
    skene_core::MainAxisAlignment::SpaceEvenly => taffy::style::AlignContent::SpaceEvenly,
  }
}

/// A [`LineAlignment`](skene_core::LineAlignment) as taffy's content alignment.
fn line(value: skene_core::LineAlignment) -> taffy::style::AlignContent {
  match value {
    skene_core::LineAlignment::Start => taffy::style::AlignContent::Start,
    skene_core::LineAlignment::End => taffy::style::AlignContent::End,
    skene_core::LineAlignment::Center => taffy::style::AlignContent::Center,
    skene_core::LineAlignment::Stretch => taffy::style::AlignContent::Stretch,
    skene_core::LineAlignment::SpaceBetween => taffy::style::AlignContent::SpaceBetween,
    skene_core::LineAlignment::SpaceAround => taffy::style::AlignContent::SpaceAround,
    skene_core::LineAlignment::SpaceEvenly => taffy::style::AlignContent::SpaceEvenly,
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

  if let Some(position) = style.position {
    out.position = match position {
      skene_core::Position::Relative => taffy::style::Position::Relative,
      skene_core::Position::Absolute => taffy::style::Position::Absolute,
    };
  }

  if let Some(inset) = style.inset {
    out.inset = dimension_rect(inset);
  }

  if style.border.is_some() {
    out.border = taffy::geometry::Rect {
      left: length(1.0),
      right: length(1.0),
      top: length(1.0),
      bottom: length(1.0),
    };
  }

  if let Some(row_spacing) = style.row_spacing {
    out.gap.height = length(cell(row_spacing));
  }

  if let Some(column_spacing) = style.column_spacing {
    out.gap.width = length(cell(column_spacing));
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

  if let Some(flex_wrap) = style.flex_wrap {
    out.flex_wrap = match flex_wrap {
      skene_core::FlexWrap::NoWrap => taffy::style::FlexWrap::NoWrap,
      skene_core::FlexWrap::Wrap => taffy::style::FlexWrap::Wrap,
      skene_core::FlexWrap::WrapReverse => taffy::style::FlexWrap::WrapReverse,
    };
  }

  if let Some(flex_grow) = style.flex_grow {
    out.flex_grow = flex_grow;
  }

  if let Some(flex_shrink) = style.flex_shrink {
    out.flex_shrink = flex_shrink;
  }

  if let Some(flex_basis) = style.flex_basis {
    out.flex_basis = dimension(flex_basis);
  }

  out.justify_content = style.main_axis_alignment.map(main_axis);
  out.align_items = style.cross_axis_alignment.map(item);
  out.align_content = style.line_alignment.map(line);
  out.justify_items = style.main_axis_item_alignment.map(item);
  out.align_self = style.self_cross_axis_alignment.map(item);
  out.justify_self = style.self_main_axis_alignment.map(item);

  if let Some(tracks) = &style.grid_template_columns {
    out.grid_template_columns = template(tracks);
  }

  if let Some(tracks) = &style.grid_template_rows {
    out.grid_template_rows = template(tracks);
  }

  if let Some(tracks) = &style.grid_auto_columns {
    out.grid_auto_columns = tracks.iter().map(|entry| track(*entry)).collect();
  }

  if let Some(tracks) = &style.grid_auto_rows {
    out.grid_auto_rows = tracks.iter().map(|entry| track(*entry)).collect();
  }

  if let Some(flow) = style.grid_auto_flow {
    out.grid_auto_flow = match flow {
      skene_core::GridAutoFlow::Row => taffy::style::GridAutoFlow::Row,
      skene_core::GridAutoFlow::Column => taffy::style::GridAutoFlow::Column,
      skene_core::GridAutoFlow::RowDense => taffy::style::GridAutoFlow::RowDense,
      skene_core::GridAutoFlow::ColumnDense => taffy::style::GridAutoFlow::ColumnDense,
    };
  }

  if let Some(row) = style.grid_row {
    out.grid_row = taffy::geometry::Line {
      start: placement(row.start),
      end: placement(row.end),
    };
  }

  if let Some(column) = style.grid_column {
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
