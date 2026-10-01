//! The one decorated box.

use skene_foundation::{ContainerBase, DecorationBase, HasBoxStyle, MotionBase, View, Wraps};

use crate::ActivePlatform;

/// Decoration and sizing around exactly one child.
///
/// The freely stylable box: decoration, sizing, events, alignment and a single child,
/// set through [`HasChild::set_child`](skene_foundation::HasChild::set_child) or
/// [`HasChild::derive_child`](skene_foundation::HasChild::derive_child). The child fills
/// the container when the container is sized and the container hugs the child when it is
/// not; [`set_alignment`](Self::set_alignment) places a child that does neither. Several
/// children take a layout view, wrapped in a container to decorate it.
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct Container(skene_foundation::Container<ActivePlatform>);

impl ContainerBase<ActivePlatform> for Container {}

impl DecorationBase<ActivePlatform> for Container {}

impl MotionBase<ActivePlatform> for Container {}

impl Container {
  crate::views::terminal_decoration!(box_style_mut);
}
