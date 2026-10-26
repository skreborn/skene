//! The one decorated box.

use skene_foundation::{ContainerBase, DecorationBase, HasBoxStyle, MotionBase, View, Wraps};

use crate::Active;

/// Decoration and sizing around exactly one child.
///
/// The freely stylable box: decoration, sizing, events, alignment and a single child,
/// set through [`HasChild::set_child`](skene_foundation::HasChild::set_child) or
/// [`HasChild::derive_child`](skene_foundation::HasChild::derive_child). The child fills
/// the container when the container is sized and the container hugs the child when it is
/// not; [`set_alignment`](Self::set_alignment) places a child that does neither. Several
/// children take a layout view, wrapped in a container to decorate it.
///
/// ```
/// use skene_tui::prelude::*;
///
/// let card = Container::new()
///   .set_padding(Inset::all(1))
///   .set_background(Color::srgb(1., 1., 1.))
///   .set_corner_radius(1)
///   .set_child(Text::new("Hello"));
/// ```
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct Container(skene_foundation::Container<Active>);

// A box view is at most a kibibyte on the stack, however many properties its style holds.
const _: () = assert!(size_of::<Container>() <= 1024);

impl ContainerBase<Active> for Container {}

impl DecorationBase<Active> for Container {}

impl MotionBase<Active> for Container {}

impl Container {
  crate::views::terminal_decoration!(box_style_mut);
}
