//! Overlay placement.

/// Where an overlay sits: against the viewport, or—when anchored—against its anchor.
///
/// Unanchored, the edges are the viewport's. Anchored, [`Bottom`](Placement::Bottom)
/// and [`Top`](Placement::Top) open below and above the anchor, start-aligned; the
/// others keep their viewport meaning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub enum Placement {
  /// Centered both ways.
  #[default]
  Center,
  /// Against the top edge, centered horizontally—or above the anchor.
  Top,
  /// The top-start corner.
  TopStart,
  /// The top-end corner.
  TopEnd,
  /// Against the bottom edge, centered horizontally—or below the anchor.
  Bottom,
  /// The bottom-start corner.
  BottomStart,
  /// The bottom-end corner.
  BottomEnd,
}
