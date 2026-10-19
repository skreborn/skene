//! What a view is, and what composing views takes: the foundation's view layer, declared for
//! this build's realization.

use skene_foundation::view::{self as base, DerivedBase};
pub use skene_foundation::view::{Apply, ChildOf, Node, NodeHandle};

use crate::Active;

skene_foundation::facade!(Active);

/// A view whose content is rebuilt whenever what its closure reads changes: the closure
/// returns the view to show, the old one removed as the new one takes its place.
///
/// ```
/// use skene_gui::prelude::*;
///
/// let signed_in = Signal::new(false);
/// let greeting = Derived::new(move || match signed_in.get() {
///   true => Text::new("Welcome back").into_any(),
///   false => Button::new("Sign in").into_any(),
/// });
/// ```
#[must_use]
pub struct Derived(base::Derived<Active>);

skene_foundation::wraps!([] Derived => base::Derived<Active>, Active);

impl DerivedBase<Active> for Derived {}
