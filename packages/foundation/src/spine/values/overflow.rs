//! Overflow behavior.

use super::Axes;

/// How a box treats content that exceeds its bounds along one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Overflow {
  /// Content spills out visibly.
  Visible,
  /// Content is clipped.
  Clip,
  /// Content is clipped and the box scrolls, with scrollbars always shown.
  Scroll,
  /// Content is clipped and the box scrolls when it overflows.
  ScrollIfNeeded,
}

impl Overflow {
  /// Whether the axis scrolls: `Scroll` or `ScrollIfNeeded`.
  #[inline(always)]
  #[must_use]
  pub const fn scrolls(self) -> bool {
    matches!(self, Self::Scroll | Self::ScrollIfNeeded)
  }
}

impl Axes<Overflow> {
  /// The overflow each axis computes to, as CSS computes it: an axis left visible beside
  /// one that is not scrolls where its content needs.
  #[inline(always)]
  pub const fn computed(self) -> Self {
    match (self.horizontal, self.vertical) {
      (Overflow::Visible, Overflow::Visible) => self,
      (Overflow::Visible, vertical) => Self::new(Overflow::ScrollIfNeeded, vertical),
      (horizontal, Overflow::Visible) => Self::new(horizontal, Overflow::ScrollIfNeeded),
      _ => self,
    }
  }

  /// Whether either axis scrolls once computed.
  #[inline(always)]
  #[must_use]
  pub const fn scrolls(self) -> bool {
    let computed = self.computed();

    computed.horizontal.scrolls() || computed.vertical.scrolls()
  }
}

#[cfg(test)]
mod tests {
  use super::{Axes, Overflow};

  /// An axis left visible beside one that is not scrolls where needed, as in CSS, and two
  /// visible axes stay visible.
  #[test]
  fn a_visible_axis_beside_a_clipping_one_scrolls_if_needed() {
    assert_eq!(
      Axes::new(Overflow::Visible, Overflow::Clip).computed(),
      Axes::new(Overflow::ScrollIfNeeded, Overflow::Clip)
    );
    assert_eq!(
      Axes::new(Overflow::Scroll, Overflow::Visible).computed(),
      Axes::new(Overflow::Scroll, Overflow::ScrollIfNeeded)
    );
    assert_eq!(
      Axes::all(Overflow::Visible).computed(),
      Axes::all(Overflow::Visible)
    );
    assert!(Axes::new(Overflow::Visible, Overflow::Clip).scrolls());
    assert!(!Axes::all(Overflow::Clip).scrolls());
  }
}
