//! A view with two forms: a fallback and the form a signal makes possible.

use skene_foundation::{EnhanceBase, View, Wraps};

use crate::Active;

/// Builds one of two forms between its anchors: the fallback while `live` is false,
/// the child once it is true.
///
/// Only the form in force is built, and each at most once: the fallback stands where
/// the signal never turns, and where it turns later the fallback is destroyed and the
/// child built in its place, under a replacement owner, so state bound beneath keeps
/// its snapshot identity. Once built, the child stands whatever the signal does after.
/// A form builds untracked, so a signal it reads while building never rebuilds it. With
/// no fallback the segment stays empty until the signal turns, and with no child
/// [`set_child_builder`](Self::set_child_builder) supplies it stays empty from then on;
/// both forms are builder closures, and only one of them is built.
///
/// The signal the built-ins wait on is the terminal's own:
/// [`Painting::available`](skene_foundation::Painting::available) for a painted box's
/// pixels, true once the terminal's graphics probe is answered.
///
/// ```
/// use skene_tui::prelude::*;
///
/// let ready = Signal::new(false);
/// let chart = Enhance::new(ready.read_only())
///   .set_fallback_builder(|| Text::new("42 visits"))
///   .set_child_builder(|| Text::new("A chart of the visits"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Enhance(skene_foundation::Enhance<Active>);

impl EnhanceBase<Active> for Enhance {}
