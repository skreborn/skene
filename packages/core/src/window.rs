//! Windowed rendering math, shared by every facade's virtual list.

/// The slice of a virtual list worth rendering: item indices `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct ItemWindow {
  /// The first rendered item's index.
  pub start: usize,
  /// One past the last rendered item's index.
  pub end: usize,
}

impl ItemWindow {
  /// The number of rendered items.
  #[inline(always)]
  #[must_use]
  pub const fn len(&self) -> usize {
    self.end - self.start
  }

  /// Whether the window renders nothing.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.start == self.end
  }
}

/// The item window covering a viewport of `viewport` at scroll `offset`, over `count`
/// items each `item_extent` long, padded by `overscan` extra items on each side.
///
/// Extents are in the caller's unit (pixels, cells); a non-positive `item_extent`
/// yields the full list.
#[inline(always)]
pub fn item_window(
  offset: f32,
  viewport: f32,
  item_extent: f32,
  count: usize,
  overscan: usize,
) -> ItemWindow {
  if item_extent <= 0.0 {
    return ItemWindow {
      start: 0,
      end: count,
    };
  }

  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  let first = (offset.max(0.0) / item_extent).floor() as usize;
  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  let visible = ((offset.max(0.0) + viewport.max(0.0)) / item_extent).ceil() as usize;

  ItemWindow {
    start: first.saturating_sub(overscan).min(count),
    end: visible.saturating_add(overscan).min(count),
  }
}

/// The item window covering a viewport of `viewport` at scroll `offset`, over items
/// whose leading edges are `edges`, padded by `overscan` extra items on each side.
///
/// `edges` holds one entry per item, its distance from the start, and one more for the
/// whole extent, so an item's length is the difference of its neighbors; the entries
/// never decrease. Extents are in the caller's unit (pixels, cells).
pub fn item_window_over(offset: f32, viewport: f32, edges: &[f32], overscan: usize) -> ItemWindow {
  let count = edges.len().saturating_sub(1);
  let offset = offset.max(0.0);
  let first = edges
    .partition_point(|edge| *edge <= offset)
    .saturating_sub(1)
    .min(count);
  let visible = edges
    .partition_point(|edge| *edge < offset + viewport.max(0.0))
    .min(count);

  ItemWindow {
    start: first.saturating_sub(overscan),
    end: visible.saturating_add(overscan).min(count).max(first),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn windows_over_edges_follow_the_measured_items() {
    let edges = [0.0, 20.0, 40.0, 60.0, 80.0, 100.0, 110.0, 120.0];

    assert_eq!(
      item_window_over(0.0, 50.0, &edges, 0),
      ItemWindow { start: 0, end: 3 }
    );
    assert_eq!(
      item_window_over(40.0, 50.0, &edges, 0),
      ItemWindow { start: 2, end: 5 }
    );
    assert_eq!(
      item_window_over(40.0, 50.0, &edges, 1),
      ItemWindow { start: 1, end: 6 }
    );
    assert_eq!(
      item_window_over(500.0, 50.0, &edges, 2),
      ItemWindow { start: 5, end: 7 }
    );
    assert_eq!(
      item_window_over(0.0, 50.0, &[0.0], 3),
      ItemWindow { start: 0, end: 0 }
    );
  }

  #[test]
  fn windows_cover_the_viewport_with_overscan() {
    let window = item_window(100.0, 50.0, 10.0, 100, 2);

    assert_eq!(window.start, 8);
    assert_eq!(window.end, 17);

    let top = item_window(0.0, 50.0, 10.0, 100, 2);

    assert_eq!(top.start, 0);
    assert_eq!(top.end, 7);

    let clamped = item_window(990.0, 50.0, 10.0, 100, 2);

    assert_eq!(clamped.end, 100);
    assert_eq!(clamped.start, 97);
  }

  #[test]
  fn degenerate_inputs_stay_in_bounds() {
    assert_eq!(item_window(0.0, 0.0, 10.0, 5, 1).len(), 1);
    assert_eq!(item_window(0.0, 100.0, 0.0, 5, 1).len(), 5);
    assert_eq!(item_window(1000.0, 100.0, 10.0, 3, 1).start, 3);
    assert!(item_window(0.0, 10.0, 5.0, 0, 3).is_empty());
  }
}
