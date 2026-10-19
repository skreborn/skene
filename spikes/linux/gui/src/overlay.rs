//! Content above the app: a popup placed against an anchor.

use skene_foundation::{
  view::{self as base, OverlayBase},
  wraps,
};

use crate::Active;

/// Content above the app: a popup placed against an anchor, opened and closed by the app.
///
/// It takes no room where it stands. Opening and closing it show and hide it, so focus and
/// scroll inside it survive; its child is built the first time it opens. It never closes
/// itself: a press outside it and its anchor, or the platform's own dismissal, only runs its
/// `handle_dismiss`. Its first control that asks for focus (`set_autofocus`) takes it as it
/// opens, and closing it returns focus to what held it when it opened.
///
/// ```
/// use skene_gui::prelude::*;
///
/// let open = Signal::new(false);
/// let anchor = NodeHandle::new();
///
/// Column::new()
///   .add_child(
///     Button::new("More")
///       .set_handle(&anchor)
///       .handle_activate(move || open.set(true)),
///   )
///   .add_child(
///     Overlay::popup()
///       .set_anchor(&anchor)
///       .bind_open(open)
///       .set_child(Text::new("Details")),
///   );
/// ```
#[must_use]
pub struct Overlay(base::Overlay<Active>);

wraps!([] Overlay => base::Overlay<Active>, Active);

impl OverlayBase<Active> for Overlay {}
