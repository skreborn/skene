//! Content shown beside the view it describes.

use skene_foundation::NoChild;

use crate::ActivePlatform;

/// A tooltip: content shown in a bubble beside the one view it describes, open while
/// the pointer rests on that view or it holds focus.
///
/// The bubble opens after [a delay](Self::set_delay) on hover and at once on focus that
/// shows, and never takes focus. It stays open while the pointer rests on the described
/// view or on the bubble, closing [`TOOLTIP_GRACE`](skene_foundation::TOOLTIP_GRACE) after the
/// pointer leaves both, and closes at once on blur and `Escape`. A text tooltip is reported as the
/// accessible description of the node that takes focus within the described view, the view itself
/// where it does; a view tooltip is reported as describing that node. The bubble is the
/// [`Part::Bubble`](skene_foundation::Part::Bubble) of the described view, decorated through the
/// setters here, and sits where an anchored overlay's [placement](Self::set_placement) puts it.
/// Holds [`NoChild`] until [`set_child`](Self::set_child) supplies the view it describes.
#[must_use]
pub struct Tooltip<C = NoChild>(skene_foundation::Tooltip<ActivePlatform, C>);

skene_foundation::impl_tooltip!(Tooltip, ActivePlatform);
