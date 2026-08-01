//! Windowed rendering math, shared by every facade's virtual list.

/// The slice of a virtual list worth rendering: item indices `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct VirtualWindow {
  /// The first rendered item's index.
  pub start: usize,
  /// One past the last rendered item's index.
  pub end: usize,
}

impl VirtualWindow {
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
pub fn virtual_window(
  offset: f32,
  viewport: f32,
  item_extent: f32,
  count: usize,
  overscan: usize,
) -> VirtualWindow {
  if item_extent <= 0.0 {
    return VirtualWindow {
      start: 0,
      end: count,
    };
  }

  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  let first = (offset.max(0.0) / item_extent).floor() as usize;
  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  let visible = ((offset.max(0.0) + viewport.max(0.0)) / item_extent).ceil() as usize;

  VirtualWindow {
    start: first.saturating_sub(overscan).min(count),
    end: visible.saturating_add(overscan).min(count),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn windows_cover_the_viewport_with_overscan() {
    let window = virtual_window(100.0, 50.0, 10.0, 100, 2);

    assert_eq!(window.start, 8);
    assert_eq!(window.end, 17);

    let top = virtual_window(0.0, 50.0, 10.0, 100, 2);

    assert_eq!(top.start, 0);
    assert_eq!(top.end, 7);

    let clamped = virtual_window(990.0, 50.0, 10.0, 100, 2);

    assert_eq!(clamped.end, 100);
    assert_eq!(clamped.start, 97);
  }

  #[test]
  fn degenerate_inputs_stay_in_bounds() {
    assert_eq!(virtual_window(0.0, 0.0, 10.0, 5, 1).len(), 1);
    assert_eq!(virtual_window(0.0, 100.0, 0.0, 5, 1).len(), 5);
    assert_eq!(virtual_window(1000.0, 100.0, 10.0, 3, 1).start, 3);
    assert!(virtual_window(0.0, 10.0, 5.0, 0, 3).is_empty());
  }
}
