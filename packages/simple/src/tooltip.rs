//! A ready-made tooltip in the Simple design language.

use std::time::Duration;

use skene_common::{CornerRadius, Tooltip};
use skene_core::{AnyKind, AnyView, NoChild, NodeId, Placement, Platform, View};

use super::{
  SimplePalette,
  shape::{SimpleUnit, define_defaults, shape},
};

/// A ready-made tooltip: a bubble in the foreground color carrying its text in the
/// background color, above the view it describes, opening after the standard delay on
/// hover and at once on focus.
///
/// [`new`](Self::new) shows text, [`derive`](Self::derive) text that follows a signal,
/// and [`view`](Self::view) any view; the described view comes last through
/// [`set_child`](Self::set_child), so any view takes a tooltip by wrapping. For
/// anything beyond that, compose [`Tooltip`] directly.
#[derive(Debug)]
#[must_use]
pub struct SimpleTooltip<P: Platform, C = NoChild> {
  raw: Tooltip<P, C>,
}

impl<P: Platform> SimpleTooltip<P> {
  /// A tooltip showing `text`.
  pub fn new(text: impl Into<String>) -> Self {
    Self {
      raw: Tooltip::new(text),
    }
  }

  /// A tooltip whose text is recomputed whenever a signal `text` reads changes.
  pub fn derive(text: impl Fn() -> String + 'static) -> Self {
    Self {
      raw: Tooltip::derive(text),
    }
  }

  /// A tooltip showing `content`, a view of any kind.
  pub fn view(content: impl View<P>) -> Self {
    Self {
      raw: Tooltip::view(content),
    }
  }

  /// Sets the view the tooltip describes.
  pub fn set_child(self, child: impl View<P>) -> SimpleTooltip<P, AnyView<P>>
  where
    P::Scalar: SimpleUnit,
    P::StyleExt: CornerRadius,
  {
    SimpleTooltip {
      raw: dress(self.raw).set_child(child),
    }
  }
}

impl<P: Platform> SimpleTooltip<P> {
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

impl<P: Platform, C> AnyKind for SimpleTooltip<P, C> {}

impl<P: Platform> View<P> for SimpleTooltip<P, AnyView<P>>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    self.raw.build()
  }
}

/// Decorates `tooltip` in the Simple language.
fn dress<P: Platform>(tooltip: Tooltip<P>) -> Tooltip<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  define_defaults::<P>();

  tooltip
    .set_color(SimplePalette::<P::Scalar>::FOREGROUND)
    .set_text_color(SimplePalette::<P::Scalar>::BACKGROUND)
    .set_padding(shape::<P>().tooltip_inset)
    .set_corner_radius(shape::<P>().mark_radius)
}
