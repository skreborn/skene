//! The one control builder, over a behavior.

use skene_foundation::{Behavior, NoChild};

use crate::ActivePlatform;

/// A control: focus, events, decoration, a label, parts and accessibility once, over
/// the [`Behavior`](skene_foundation::Behavior) that says what the interaction is.
///
/// The behavior runs first on every event it acts on and its verdict is the whole
/// verdict; every `handle_` here observes. The control is controlled: it reports, and
/// the signal its behavior is bound to is the truth. Its parts and its child resolve
/// the control-scoped conditions (`Checked`, `Disabled`, …) and their focus against it.
/// Holds [`NoChild`] until [`set_child`](Self::set_child) or
/// [`set_child_builder`](Self::set_child_builder) supplies the drawn body; a control
/// with a label or parts alone builds without one. Its parts, its body and its label
/// sit in one row half an em apart, centered on the cross axis, the label where the
/// behavior places it ([`Behavior::label_placement`](skene_foundation::Behavior::label_placement)):
/// leading or trailing in the row, or beside the box, before it among its siblings.
#[must_use]
pub struct Control<B: Behavior<ActivePlatform>, C = NoChild>(
  skene_foundation::Control<ActivePlatform, B, C>,
);

skene_foundation::impl_control!(Control, ActivePlatform);

impl<B: Behavior<ActivePlatform>> Control<B> {
  crate::views::terminal_decoration!();
}
