//! A box painted through vello.

use skene_foundation::{PaintedBase, View, Wraps};

use crate::ActivePlatform;

/// A box drawn by a paint callback through the platform's renderer: sized like any box,
/// painted at its laid-out size now and whenever that size changes.
///
/// The portable low-level path for pixels; it exists only where the platform
/// implements [`Painting`](skene_foundation::Painting). With a
/// [`fallback`](Self::set_fallback_builder) it is an [`Enhance`](crate::Enhance) on
/// [`Painting::available`](skene_foundation::Painting::available): the fallback stands where
/// nothing is drawn as pixels, a server render and a terminal without a graphics protocol included,
/// and the painted box takes its place once something is.
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Painted(skene_foundation::Painted<ActivePlatform>);

impl PaintedBase<ActivePlatform> for Painted {}
