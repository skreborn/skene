//! A ready-made tooltip in the Simple design language.

use skene_foundation::{AnyView, NoChild, View, Wraps};
use skene_simple::SimpleTooltipBase;
use skene_web::ActivePlatform;

/// A ready-made tooltip: a bubble in the foreground color carrying its text in the
/// background color, above the view it describes, opening after the standard delay on
/// hover and at once on focus.
///
/// [`new`](Self::new) shows text, [`derive`](Self::derive) text that follows a signal,
/// and [`view`](Self::view) any view; the described view comes last through
/// [`set_child`](Self::set_child), so any view takes a tooltip by wrapping. For
/// anything beyond that, compose [`Tooltip`](skene_web::Tooltip) directly.
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct SimpleTooltip<C = NoChild>(skene_simple::SimpleTooltip<ActivePlatform, C>);

impl SimpleTooltipBase<ActivePlatform> for SimpleTooltip {}

impl SimpleTooltip {
  /// Sets the view the tooltip describes.
  #[inline(always)]
  pub fn set_child(
    self,
    child: impl View<ActivePlatform>,
  ) -> SimpleTooltip<AnyView<ActivePlatform>> {
    SimpleTooltip(self.0.set_child(child))
  }
}
