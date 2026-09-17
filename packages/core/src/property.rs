//! The names of a style's properties.

use std::{fmt::Debug, hash::Hash};

/// A property both facades share, named in the framework's own vocabulary.
///
/// Names slots, transitions and updates; a platform maps each to its medium's spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub enum Property {
  /// The width.
  Width,
  /// The height.
  Height,
  /// The minimum width.
  MinWidth,
  /// The minimum height.
  MinHeight,
  /// The maximum width.
  MaxWidth,
  /// The maximum height.
  MaxHeight,
  /// The width-to-height ratio the unsized axis follows.
  AspectRatio,
  /// The margin on each edge.
  Margin,
  /// The padding on each edge.
  Padding,
  /// The shift from each edge, keeping the box in flow.
  Offset,
  /// The edges the box is pinned to, out of flow.
  Pinned,
  /// The background color.
  Color,
  /// The border.
  Border,
  /// The mouse cursor shown over the box.
  Cursor,
  /// The spacing between children, per axis.
  Spacing,
  /// Whether and how a flex container wraps its children.
  Wrap,
  /// The box's flexibility as a flex child.
  Flex,
  /// Distribution of children along the main or inline axis.
  Arrangement,
  /// Alignment of children on the cross or block axis.
  ItemAlignment,
  /// Distribution of wrapped lines or grid tracks along the cross or block axis.
  LineArrangement,
  /// Alignment of grid children on the inline axis.
  ItemHorizontalAlignment,
  /// The box's own cross- or block-axis alignment.
  SelfAlignment,
  /// The box's own inline-axis alignment in a grid.
  SelfHorizontalAlignment,
  /// The grid's explicit column tracks.
  GridColumns,
  /// The grid's explicit row tracks.
  GridRows,
  /// The sizes given to implicitly created columns.
  GridImplicitColumns,
  /// The sizes given to implicitly created rows.
  GridImplicitRows,
  /// The direction auto-placed grid children fill.
  GridFlow,
  /// The box's row placement as a grid child.
  GridRowPlacement,
  /// The box's column placement as a grid child.
  GridColumnPlacement,
  /// How content exceeding the box's bounds is treated.
  Overflow,
  /// The text color.
  TextColor,
  /// The text color while selected.
  SelectionColor,
  /// The background behind selected text.
  SelectionBackground,
  /// The line the text is decorated with.
  Decoration,
  /// How the decoration line is drawn.
  DecorationStyle,
  /// The decoration line's color.
  DecorationColor,
}

impl Property {
  /// The property's name in the framework's vocabulary: the field it is set through.
  #[inline(always)]
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::Width => "width",
      Self::Height => "height",
      Self::MinWidth => "min_width",
      Self::MinHeight => "min_height",
      Self::MaxWidth => "max_width",
      Self::MaxHeight => "max_height",
      Self::AspectRatio => "aspect_ratio",
      Self::Margin => "margin",
      Self::Padding => "padding",
      Self::Offset => "offset",
      Self::Pinned => "pinned",
      Self::Color => "color",
      Self::Border => "border",
      Self::Cursor => "cursor",
      Self::Spacing => "spacing",
      Self::Wrap => "wrap",
      Self::Flex => "flex",
      Self::Arrangement => "arrangement",
      Self::ItemAlignment => "item_alignment",
      Self::LineArrangement => "line_arrangement",
      Self::ItemHorizontalAlignment => "item_horizontal_alignment",
      Self::SelfAlignment => "self_alignment",
      Self::SelfHorizontalAlignment => "self_horizontal_alignment",
      Self::GridColumns => "grid_columns",
      Self::GridRows => "grid_rows",
      Self::GridImplicitColumns => "grid_implicit_columns",
      Self::GridImplicitRows => "grid_implicit_rows",
      Self::GridFlow => "grid_flow",
      Self::GridRowPlacement => "grid_row_placement",
      Self::GridColumnPlacement => "grid_column_placement",
      Self::Overflow => "overflow",
      Self::TextColor => "text_color",
      Self::SelectionColor => "selection_color",
      Self::SelectionBackground => "selection_background",
      Self::Decoration => "decoration",
      Self::DecorationStyle => "decoration_style",
      Self::DecorationColor => "decoration_color",
    }
  }
}

/// A set of property names: the shared [`Property`], or a medium's own.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not name style properties",
  label = "a style extension names its properties with a fieldless enum implementing `PropertyName`"
)]
pub trait PropertyName: Copy + Eq + Hash + Debug + 'static {
  /// The property's name in the framework's vocabulary.
  fn name(&self) -> &'static str;
}

impl PropertyName for Property {
  #[inline(always)]
  fn name(&self) -> &'static str {
    Self::name(*self)
  }
}

/// A property of a style: shared by every facade, or one of the medium's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum PropertyKey<P> {
  /// A property both facades share.
  Shared(Property),
  /// A property the medium's style extension adds.
  Extension(P),
}

impl<P: PropertyName> PropertyKey<P> {
  /// The property's name in the framework's vocabulary.
  #[inline(always)]
  #[must_use]
  pub fn name(self) -> &'static str {
    match self {
      Self::Shared(property) => property.name(),
      Self::Extension(property) => property.name(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{Property, PropertyKey};
  use crate::NoProperty;

  /// A name is the setter's field, never a medium's spelling.
  #[test]
  fn names_follow_the_vocabulary() {
    assert_eq!(Property::Spacing.name(), "spacing");
    assert_eq!(Property::ItemAlignment.name(), "item_alignment");
    assert_eq!(
      PropertyKey::<NoProperty>::Shared(Property::Color).name(),
      "color"
    );
  }
}
