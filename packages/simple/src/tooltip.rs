//! A ready-made tooltip in the Simple design language.

use std::time::Duration;

use skene_core::{AnyKind, AnyView, Driver, IntoView, NoChild, NodeId, Placement, View};
use skene_views::{CornerRadius, Tooltip};

use super::{
  SimplePalette,
  shape::{MARK_RADIUS, SimpleUnit, define_defaults, shape},
};

/// A ready-made tooltip: a bubble in the foreground color carrying its text in the
/// background color, above the view it describes, opening after the standard delay on
/// hover and at once on focus.
///
/// [`new`](Self::new) shows text, [`derive`](Self::derive) text that follows a signal,
/// and [`view`](Self::view) any view; the described view comes last through
/// [`set_child`](Self::set_child), so any view takes a tooltip by wrapping. For
/// anything beyond that, compose [`Tooltip`] directly.
#[must_use]
pub struct SimpleTooltip<D: Driver, C = NoChild> {
  raw: Tooltip<D, C>,
}

impl<D: Driver> SimpleTooltip<D> {
  /// A tooltip showing `text`.
  #[inline(always)]
  pub fn new(text: impl Into<String>) -> Self {
    Self {
      raw: Tooltip::new(text),
    }
  }

  /// A tooltip whose text is recomputed whenever a signal `text` reads changes.
  #[inline(always)]
  pub fn derive(text: impl Fn() -> String + 'static) -> Self {
    Self {
      raw: Tooltip::derive(text),
    }
  }

  /// A tooltip showing `content`, a view of any kind.
  #[inline(always)]
  pub fn view(content: impl IntoView<D>) -> Self {
    Self {
      raw: Tooltip::view(content),
    }
  }

  /// Sets the view the tooltip describes.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<D>) -> SimpleTooltip<D, AnyView<D>> {
    SimpleTooltip {
      raw: self.raw.set_child(child),
    }
  }
}

impl<D: Driver, C> SimpleTooltip<D, C> {
  /// Sets where the bubble sits against the described view: above it for `Top`, below
  /// it for `Bottom`.
  #[inline(always)]
  pub fn set_placement(mut self, placement: Placement) -> Self {
    self.raw = self.raw.set_placement(placement);

    self
  }

  /// Sets how long the pointer rests on the described view before the bubble opens.
  #[inline(always)]
  pub fn set_delay(mut self, delay: Duration) -> Self {
    self.raw = self.raw.set_delay(delay);

    self
  }
}

impl<D: Driver, C> AnyKind for SimpleTooltip<D, C> {}

impl<D: Driver> View<D> for SimpleTooltip<D, AnyView<D>>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<D>();

    self
      .raw
      .set_color(SimplePalette::<D::Scalar>::FOREGROUND)
      .set_text_color(SimplePalette::<D::Scalar>::BACKGROUND)
      .set_padding(shape::<D>().tooltip_inset)
      .set_corner_radius(MARK_RADIUS)
      .build()
  }
}
