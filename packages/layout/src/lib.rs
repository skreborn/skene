//! # Layout
//!
//! The layout every platform but the web computes, on taffy: the shared style model
//! mapped onto taffy's styles, generic over the platform's scalar, a unit of which is
//! one unit of the layout ([`BoxLayout`]), a tree keyed by the scene's nodes, retained
//! across layouts and written node by node or by a walk of the scene, each node's part
//! classified by its platform ([`Tree`]), what moved since a platform last restyled its
//! tree and so the nodes it revisits next ([`Restyle`]), and a paragraph's runs and inline
//! boxes laid out as one text through a realization's text engine ([`TextFlow`]). The web
//! lays out through CSS and needs none of it.

mod restyle;
mod text;
mod tree;

use skene_foundation::{
  Arrangement,
  Axis,
  BoxStyle,
  Dimension,
  GridFlow,
  GridPlacement,
  GridTemplate,
  GridTrack,
  Inset,
  ItemAlignment,
  Layout,
  Overflow,
  Plain,
  Scalar,
  StyleExtension,
  TrackLimit,
  Wrap,
};
pub use taffy;
use taffy::{
  prelude::{TaffyAuto, TaffyGridLine, TaffyGridSpan, fit_content, fr, length, minmax, percent},
  style::{Display, Style},
};

pub use self::{
  restyle::{Restyle, Revisit},
  text::{RunHit, TextFlow},
  tree::{Frames, Leaf, Part, Tree},
};

/// How a box's border takes room in its layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Borders {
  /// Each edge of a width above zero takes one unit, as a grid of cells draws it.
  Unit,
  /// Each edge takes its width.
  Width,
}

impl Borders {
  /// The room the border of `style` takes on each edge.
  pub fn room<N: Scalar, X: StyleExtension>(self, style: &BoxStyle<N, Plain, X>) -> Inset<f32> {
    let width = style.border_width.unwrap_or_default();
    let edge = |width: N| match self {
      Self::Unit if width.to_f32() > 0. => 1.,
      Self::Unit => 0.,
      Self::Width => extent(width),
    };

    Inset {
      top: edge(width.top),
      end: edge(width.end),
      bottom: edge(width.bottom),
      start: edge(width.start),
    }
  }
}

/// Where a box sits and how its border takes room, beside its style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
  /// Whether its parent is a stack, which places every child in its one cell.
  pub in_stack: bool,
  /// How its border takes room.
  pub borders: Borders,
}

impl Place {
  /// The place outside any stack's cell, as an overlay above its parent's flow takes.
  #[inline(always)]
  #[must_use]
  pub const fn detached(self) -> Self {
    Self {
      in_stack: false,
      ..self
    }
  }

  /// Places a node laid out as `out` in its parent: in a stack's one cell where the
  /// parent is a stack.
  pub fn apply(self, out: &mut Style) {
    if self.in_stack {
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
}

/// A box's resolved style read as taffy's.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a resolved box style",
  label = "a box lays out from its style with every property resolved: `BoxStyle<N, Plain, X>`"
)]
pub trait BoxLayout {
  /// The taffy style of the box laid out as `layout`: its mode, then its sizing,
  /// spacing, alignment and placement where `place` puts it.
  fn taffy_style(&self, layout: Layout, place: Place) -> Style;

  /// Folds the style's sizing, spacing, alignment and placement into `out`, and places
  /// it where `place` puts it.
  fn fold_into(&self, out: &mut Style, place: Place);
}

impl<N: Scalar, X: StyleExtension> BoxLayout for BoxStyle<N, Plain, X> {
  fn taffy_style(&self, layout: Layout, place: Place) -> Style {
    let mut out = Style {
      display: match layout {
        Layout::Document => Display::Block,
        Layout::Flex { .. } => Display::Flex,
        Layout::Grid | Layout::Stack | Layout::Single => Display::Grid,
      },
      ..Style::default()
    };

    if layout == Layout::Single {
      let single = vec![taffy::style::GridTemplateComponent::Single(fr(1.))];

      out.grid_template_columns = single.clone();
      out.grid_template_rows = single;
    }

    if let Layout::Flex { axis, reversed } = layout {
      out.flex_direction = match (axis, reversed) {
        (Axis::Horizontal, false) => taffy::style::FlexDirection::Row,
        (Axis::Vertical, false) => taffy::style::FlexDirection::Column,
        (Axis::Horizontal, true) => taffy::style::FlexDirection::RowReverse,
        (Axis::Vertical, true) => taffy::style::FlexDirection::ColumnReverse,
      };
    }

    self.fold_into(&mut out, place);

    out
  }

  #[expect(
    clippy::too_many_lines,
    reason = "one fold per property; length is the property count, not complexity"
  )]
  fn fold_into(&self, out: &mut Style, place: Place) {
    out.size = taffy::geometry::Size {
      width: self.width.map_or(taffy::style::Dimension::AUTO, dimension),
      height: self.height.map_or(taffy::style::Dimension::AUTO, dimension),
    };

    out.min_size = taffy::geometry::Size {
      width: self
        .min_width
        .map_or(taffy::style::LengthPercentageAuto::AUTO, dimension_lpa),
      height: self
        .min_height
        .map_or(taffy::style::LengthPercentageAuto::AUTO, dimension_lpa),
    };

    out.max_size = taffy::geometry::Size {
      width: self
        .max_width
        .map_or(taffy::style::LengthPercentageAuto::AUTO, dimension_lpa),
      height: self
        .max_height
        .map_or(taffy::style::LengthPercentageAuto::AUTO, dimension_lpa),
    };

    out.aspect_ratio = self.aspect_ratio.map(|ratio| ratio.0);

    if let Some(margin) = self.margin {
      out.margin = dimension_rect(margin);
    }

    if let Some(padding) = self.padding {
      out.padding = taffy::geometry::Rect {
        left: length(extent(padding.start)),
        right: length(extent(padding.end)),
        top: length(extent(padding.top)),
        bottom: length(extent(padding.bottom)),
      };
    }

    // A pinned box leaves the flow; an offset one shifts within it. Pinned wins when
    // both are set.
    if let Some(pinned) = self.pinned {
      out.position = taffy::style::Position::Absolute;
      out.inset = dimension_rect(pinned);
    }
    else if let Some(offset) = self.offset {
      out.position = taffy::style::Position::Relative;
      out.inset = dimension_rect(offset);
    }

    let border = place.borders.room(self);

    if border != Inset::default() {
      out.border = taffy::geometry::Rect {
        left: length(border.start),
        right: length(border.end),
        top: length(border.top),
        bottom: length(border.bottom),
      };
    }

    if let Some(spacing) = self.spacing {
      out.gap = taffy::geometry::Size {
        width: length(extent(spacing.horizontal)),
        height: length(extent(spacing.vertical)),
      };
    }

    if let Some(overflow) = self.overflow {
      let overflow = overflow.computed();
      let mapped = |overflow| match overflow {
        Overflow::Visible => taffy::style::Overflow::Visible,
        Overflow::Clip => taffy::style::Overflow::Clip,
        Overflow::Scroll | Overflow::ScrollIfNeeded => taffy::style::Overflow::Scroll,
      };

      out.overflow = taffy::geometry::Point {
        x: mapped(overflow.horizontal),
        y: mapped(overflow.vertical),
      };
    }

    if let Some(wrap) = self.wrap {
      out.flex_wrap = match wrap {
        Wrap::None => taffy::style::FlexWrap::NoWrap,
        Wrap::Forward => taffy::style::FlexWrap::Wrap,
        Wrap::Reverse => taffy::style::FlexWrap::WrapReverse,
      };
    }

    if let Some(flex) = self.flex {
      out.flex_basis = dimension(flex.initial);
      out.flex_grow = flex.grow_factor;
      out.flex_shrink = flex.shrink_factor;
    }

    out.justify_content = self
      .arrangement
      .map_or(taffy::style::JustifyContent::NORMAL, arrangement);
    out.align_items = self
      .item_alignment
      .map_or(taffy::style::AlignItems::NORMAL, item);
    out.align_content = self
      .line_arrangement
      .map_or(taffy::style::AlignContent::NORMAL, arrangement);
    out.justify_items = self
      .item_horizontal_alignment
      .map_or(taffy::style::JustifyItems::NORMAL, item);
    out.align_self = self.self_alignment.map(item);
    out.justify_self = self.self_horizontal_alignment.map(item);

    if let Some(template) = &self.grid_columns {
      (out.grid_template_columns, out.grid_template_column_names) = explicit(template);
    }

    if let Some(template) = &self.grid_rows {
      (out.grid_template_rows, out.grid_template_row_names) = explicit(template);
    }

    if let Some(areas) = &self.grid_areas {
      let rows = areas.rows();

      out.grid_template_areas = Some(taffy::style::GridTemplateAreas {
        areas: areas
          .areas()
          .unwrap_or_default()
          .into_iter()
          .map(|area| taffy::style::GridTemplateArea {
            name: area.name.as_str().to_owned(),
            row_start: area.rows.start,
            row_end: area.rows.end,
            column_start: area.columns.start,
            column_end: area.columns.end,
          })
          .collect(),
        row_count: u16::try_from(rows.len()).unwrap_or(u16::MAX),
        column_count: u16::try_from(rows.first().map_or(0, Vec::len)).unwrap_or(u16::MAX),
      });
    }

    if let Some(tracks) = &self.grid_implicit_columns {
      out.grid_auto_columns = implicit(tracks);
    }

    if let Some(tracks) = &self.grid_implicit_rows {
      out.grid_auto_rows = implicit(tracks);
    }

    if let Some(flow) = self.grid_flow {
      out.grid_auto_flow = match flow {
        GridFlow::Rows => taffy::style::GridAutoFlow::Row,
        GridFlow::Columns => taffy::style::GridAutoFlow::Column,
        GridFlow::DenseRows => taffy::style::GridAutoFlow::RowDense,
        GridFlow::DenseColumns => taffy::style::GridAutoFlow::ColumnDense,
      };
    }

    if let Some(row) = self.grid_row_placement {
      out.grid_row = taffy::geometry::Line {
        start: placement(row.start),
        end: placement(row.end),
      };
    }

    if let Some(column) = self.grid_column_placement {
      out.grid_column = taffy::geometry::Line {
        start: placement(column.start),
        end: placement(column.end),
      };
    }

    place.apply(out);
  }
}

/// A length a padding, border or spacing takes: none below zero.
fn extent<N: Scalar>(units: N) -> f32 {
  units.to_f32().max(0.)
}

/// A [`Dimension`] as a taffy size; auto for a length or a fraction below zero, as CSS
/// drops such a size.
fn dimension<N: Scalar>(value: Dimension<N>) -> taffy::style::Dimension {
  match value {
    Dimension::Length(units) if units.to_f32() >= 0. => length(units.to_f32()),
    Dimension::Fraction(value) if value >= 0. => percent(value),
    Dimension::Length(_) | Dimension::Fraction(_) | Dimension::Auto => {
      taffy::style::Dimension::AUTO
    }
  }
}

/// A [`Dimension`] as a taffy auto-capable size; auto for a length or a fraction below
/// zero, as CSS drops such a size.
fn dimension_lpa<N: Scalar>(value: Dimension<N>) -> taffy::style::LengthPercentageAuto {
  match value {
    Dimension::Length(units) if units.to_f32() >= 0. => length(units.to_f32()),
    Dimension::Fraction(value) if value >= 0. => percent(value),
    Dimension::Length(_) | Dimension::Fraction(_) | Dimension::Auto => {
      taffy::style::LengthPercentageAuto::AUTO
    }
  }
}

/// A [`Dimension`] as a taffy auto-capable offset, its sign kept.
fn offset_lpa<N: Scalar>(value: Dimension<N>) -> taffy::style::LengthPercentageAuto {
  match value {
    Dimension::Length(units) => length(units.to_f32()),
    Dimension::Fraction(value) => percent(value),
    Dimension::Auto => taffy::style::LengthPercentageAuto::AUTO,
  }
}

/// A margin or an inset as a taffy rect, each edge's sign kept.
fn dimension_rect<N: Scalar>(
  inset: Inset<Dimension<N>>,
) -> taffy::geometry::Rect<taffy::style::LengthPercentageAuto> {
  taffy::geometry::Rect {
    left: offset_lpa(inset.start),
    right: offset_lpa(inset.end),
    top: offset_lpa(inset.top),
    bottom: offset_lpa(inset.bottom),
  }
}

/// One [`TrackLimit`] as a taffy min-track sizing function.
fn min_track<N: Scalar>(limit: TrackLimit<N>) -> taffy::style::MinTrackSizingFunction {
  match limit {
    TrackLimit::Length(units) => length(units.to_f32()),
    TrackLimit::Fraction(value) => percent(value),
    TrackLimit::Flex(_) | TrackLimit::Auto => taffy::style::MinTrackSizingFunction::AUTO,
    TrackLimit::MinContent => taffy::prelude::min_content(),
    TrackLimit::MaxContent => taffy::prelude::max_content(),
  }
}

/// One [`TrackLimit`] as a taffy max-track sizing function.
fn max_track<N: Scalar>(limit: TrackLimit<N>) -> taffy::style::MaxTrackSizingFunction {
  match limit {
    TrackLimit::Length(units) => length(units.to_f32()),
    TrackLimit::Fraction(value) => percent(value),
    TrackLimit::Flex(value) => fr(value),
    TrackLimit::Auto => taffy::style::MaxTrackSizingFunction::AUTO,
    TrackLimit::MinContent => taffy::prelude::min_content(),
    TrackLimit::MaxContent => taffy::prelude::max_content(),
  }
}

/// One [`GridTrack`] as a taffy track sizing function; `None` for an adaptive track,
/// which only a template holds, and for a track CSS rejects.
fn track<N: Scalar>(value: GridTrack<N>) -> Option<taffy::style::TrackSizingFunction> {
  if value.is_below_zero() {
    return None;
  }

  Some(match value {
    GridTrack::Length(units) => length(units.to_f32()),
    GridTrack::Fraction(value) => percent(value),
    GridTrack::Flex(value) => fr(value),
    GridTrack::Auto => taffy::style::TrackSizingFunction::AUTO,
    GridTrack::MinContent => taffy::prelude::min_content(),
    GridTrack::MaxContent => taffy::prelude::max_content(),
    GridTrack::FitContent(units) => fit_content(length(units.to_f32())),
    GridTrack::Between(min, max) => minmax(min_track(min), max_track(max)),
    GridTrack::Adaptive(..) => return None,
  })
}

/// A template as taffy's tracks and line names, the names empty where no line has one;
/// both empty where CSS drops the list: more than one adaptive track, one with neither
/// limit a length or a fraction, or a track below zero.
fn explicit<N: Scalar>(
  template: &GridTemplate<N>,
) -> (
  Vec<taffy::style::GridTemplateComponent<String>>,
  Vec<Vec<String>>,
) {
  if template.is_dropped() {
    return (Vec::new(), Vec::new());
  }

  let tracks = template
    .tracks()
    .filter_map(|entry| match entry {
      GridTrack::Adaptive(min, max) => Some(taffy::style::GridTemplateComponent::Repeat(
        taffy::style::GridTemplateRepetition {
          count: taffy::style::RepetitionCount::AutoFill,
          tracks: vec![minmax(min_track(min), max_track(max))],
          line_names: Vec::new(),
        },
      )),
      entry => track(entry).map(taffy::style::GridTemplateComponent::Single),
    })
    .collect();
  let lines = template.line_names();
  let names = if lines.iter().any(|names| !names.is_empty()) {
    lines
      .into_iter()
      .map(|names| {
        names
          .into_iter()
          .map(|name| name.as_str().to_owned())
          .collect()
      })
      .collect()
  }
  else {
    Vec::new()
  };

  (tracks, names)
}

/// A track list as taffy's implicit tracks; empty where CSS drops the list, an adaptive
/// track or a track below zero in it.
fn implicit<N: Scalar>(tracks: &[GridTrack<N>]) -> Vec<taffy::style::TrackSizingFunction> {
  if !tracks.iter().all(|entry| entry.is_implicit()) {
    return Vec::new();
  }

  tracks.iter().filter_map(|entry| track(*entry)).collect()
}

/// One [`GridPlacement`] as taffy's: the first line a name finds is taffy's name with no
/// index, CSS's bare `<custom-ident>`, which an area's name matches by the area's edge.
fn placement(value: GridPlacement) -> taffy::style::GridPlacement<String> {
  match value {
    GridPlacement::Auto => taffy::style::GridPlacement::Auto,
    GridPlacement::Line(index) => taffy::style::GridPlacement::from_line_index(index),
    GridPlacement::Span(count) => taffy::style::GridPlacement::from_span(count),
    GridPlacement::Named(name, 1) => {
      taffy::style::GridPlacement::NamedLine(name.as_str().to_owned(), 0)
    }
    GridPlacement::Named(name, index) => {
      taffy::style::GridPlacement::NamedLine(name.as_str().to_owned(), index)
    }
    GridPlacement::NamedSpan(name, count) => {
      taffy::style::GridPlacement::NamedSpan(name.as_str().to_owned(), count)
    }
  }
}

/// An [`Arrangement`] as taffy's content alignment.
const fn arrangement(value: Arrangement) -> taffy::style::AlignContent {
  match value {
    Arrangement::Start => taffy::style::AlignContent::START,
    Arrangement::End => taffy::style::AlignContent::END,
    Arrangement::Center => taffy::style::AlignContent::CENTER,
    Arrangement::Stretch => taffy::style::AlignContent::STRETCH,
    Arrangement::SpaceBetween => taffy::style::AlignContent::SPACE_BETWEEN,
    Arrangement::SpaceAround => taffy::style::AlignContent::SPACE_AROUND,
    Arrangement::SpaceEvenly => taffy::style::AlignContent::SPACE_EVENLY,
  }
}

/// An [`ItemAlignment`] as taffy's item alignment.
const fn item(value: ItemAlignment) -> taffy::style::AlignItems {
  match value {
    ItemAlignment::Start => taffy::style::AlignItems::START,
    ItemAlignment::End => taffy::style::AlignItems::END,
    ItemAlignment::Center => taffy::style::AlignItems::CENTER,
    ItemAlignment::Baseline => taffy::style::AlignItems::BASELINE,
    ItemAlignment::Stretch => taffy::style::AlignItems::STRETCH,
  }
}

#[cfg(test)]
mod tests {
  use skene_foundation::{
    Axis,
    BoxStyle,
    Dimension,
    GridTemplate,
    GridTrack,
    Inset,
    Layout,
    Plain,
    Spacing,
    TrackLimit,
  };
  use taffy::{
    prelude::{TaffyAuto, TaffyGridLine, length, percent},
    style::{Display, FlexDirection},
  };

  use super::{Borders, BoxLayout, Place};

  /// A border takes one unit on each edge drawn where it is cells, and its width where
  /// it is lengths.
  #[test]
  fn a_border_takes_its_room_by_the_platforms_rule() {
    let style = BoxStyle::<i32, Plain> {
      border_width: Some(Inset {
        top: 2,
        end: -1,
        bottom: 1,
        start: 0,
      }),
      ..BoxStyle::default()
    };

    assert_eq!(
      Borders::Unit.room(&style),
      Inset {
        top: 1.,
        end: 0.,
        bottom: 1.,
        start: 0.,
      }
    );
    assert_eq!(
      Borders::Width.room(&style),
      Inset {
        top: 2.,
        end: 0.,
        bottom: 1.,
        start: 0.,
      }
    );
  }

  /// A vertical flex box lays out as a taffy column, its lengths and fractions and its
  /// spacing carried over, and a stack's child takes the stack's one cell.
  #[test]
  fn a_flex_column_carries_its_direction_sizes_and_spacing() {
    let style = BoxStyle::<f32, Plain> {
      width: Some(Dimension::Length(10.)),
      height: Some(Dimension::Fraction(0.5)),
      spacing: Some(Spacing {
        horizontal: 2.,
        vertical: 3.,
      }),
      ..BoxStyle::default()
    };
    let taffy = style.taffy_style(
      Layout::Flex {
        axis: Axis::Vertical,
        reversed: false,
      },
      Place {
        in_stack: true,
        borders: Borders::Width,
      },
    );

    assert_eq!(taffy.display, Display::Flex);
    assert_eq!(taffy.flex_direction, FlexDirection::Column);
    assert_eq!(taffy.size.width, length(10.));
    assert_eq!(taffy.size.height, percent(0.5));
    assert_eq!(taffy.gap.width, length(2.));
    assert_eq!(taffy.gap.height, length(3.));
    assert_eq!(
      taffy.grid_row.start,
      taffy::style::GridPlacement::from_line_index(1)
    );
  }

  /// A size or a track list below zero is dropped as CSS drops it, a padding, border
  /// or spacing below zero takes no room, and a margin and an inset keep their sign.
  #[test]
  fn a_negative_length_is_dropped_or_zero_where_css_takes_none() {
    let style = BoxStyle::<i32, Plain> {
      width: Some(Dimension::Length(-4)),
      height: Some(Dimension::Fraction(-0.5)),
      min_height: Some(Dimension::Length(-2)),
      max_width: Some(Dimension::Length(6)),
      padding: Some(Inset::all(-1)),
      border_width: Some(Inset::all(-3)),
      spacing: Some(Spacing {
        horizontal: -2,
        vertical: 1,
      }),
      margin: Some(Inset::all(Dimension::Length(-4))),
      offset: Some(Inset {
        top: Dimension::Length(-1),
        end: Dimension::Auto,
        bottom: Dimension::Auto,
        start: Dimension::Length(2),
      }),
      grid_columns: Some(
        GridTemplate::new()
          .add_track(GridTrack::Length(2))
          .add_track(GridTrack::Length(-3)),
      ),
      grid_rows: Some(GridTemplate::new().add_track(GridTrack::Length(2))),
      grid_implicit_columns: Some(vec![
        GridTrack::Auto,
        GridTrack::Between(TrackLimit::Length(-1), TrackLimit::Auto),
      ]),
      ..BoxStyle::default()
    };
    let taffy = style.taffy_style(
      Layout::Grid,
      Place {
        in_stack: false,
        borders: Borders::Width,
      },
    );

    assert_eq!(taffy.size.width, taffy::style::Dimension::AUTO);
    assert_eq!(taffy.size.height, taffy::style::Dimension::AUTO);
    assert_eq!(
      taffy.min_size.height,
      taffy::style::LengthPercentageAuto::AUTO
    );
    assert_eq!(taffy.max_size.width, length(6.));
    assert_eq!(taffy.padding, taffy::geometry::Rect::zero());
    assert_eq!(taffy.border, taffy::geometry::Rect::zero());
    assert_eq!(taffy.gap.width, length(0.));
    assert_eq!(taffy.gap.height, length(1.));
    assert_eq!(taffy.margin.left, length(-4.));
    assert_eq!(taffy.margin.bottom, length(-4.));
    assert_eq!(taffy.inset.top, length(-1.));
    assert_eq!(taffy.inset.left, length(2.));
    assert_eq!(taffy.grid_template_columns, Vec::new());
    assert_eq!(
      taffy.grid_template_rows,
      vec![taffy::style::GridTemplateComponent::Single(length(2.))]
    );
    assert_eq!(taffy.grid_auto_columns, Vec::new());
  }
}
