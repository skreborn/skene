//! A ready-made tooltip in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimpleTooltipBase;
use skene_terminal::Active;

/// A ready-made tooltip: a bubble in the foreground color carrying its text in the
/// background color, above the view it describes, opening after the standard delay on
/// hover and at once on focus.
///
/// [`new`](Self::new) shows text, [`derive`](Self::derive) text that follows a signal,
/// and [`view`](Self::view) any view; the described view comes last through
/// [`set_child`](Self::set_child), so any view takes a tooltip by wrapping. For
/// anything beyond that, compose [`Tooltip`](skene_terminal::Tooltip) directly.
///
/// ```no_run
/// use skene_simple_terminal::prelude::*;
/// use skene_terminal::prelude::*;
///
/// let save = SimpleTooltip::new("Saves the document").set_child(SimpleButton::new("Save"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct SimpleTooltip(skene_simple::SimpleTooltip<Active>);

impl SimpleTooltipBase<Active> for SimpleTooltip {}
