//! The one decorated box.

use skene_foundation::{
  AnyKind,
  Children,
  Color,
  HasChild,
  Length,
  NodeId,
  PlainChild,
  Seal,
  View,
  terminal::{BorderGlyphs, ScrollbarGlyphs},
};

use crate::ActivePlatform;

/// Decoration and sizing around exactly one child.
///
/// The freely stylable box: decoration, sizing, events, alignment and a single child,
/// set through [`HasChild::set_child`] or [`HasChild::derive_child`]. The child fills
/// the container when the container is sized and the container hugs the child when it is
/// not; [`set_alignment`](Self::set_alignment) places a child that does neither. Several
/// children take a layout view, wrapped in a container to decorate it.
#[derive(Debug, Default)]
#[must_use]
pub struct Container(skene_foundation::Container<ActivePlatform>);

impl Container {
  /// An undecorated container.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::Container::new())
  }

  skene_foundation::container_surface! { 0, ActivePlatform }

  skene_foundation::properties! { style_mut, ActivePlatform;
    set_border_glyphs / derive_border_glyphs (glyphs) => ext.border_glyphs: BorderGlyphs,
      "the characters the border draws with";
    set_shadow / derive_shadow (shadow) => ext.shadow: Color,
      "the shadow color",
      "Its alpha is the strength of the mix into the colors beneath where the shadow falls.";
    set_shadow_offset / derive_shadow_offset (offset) => ext.shadow_offset: Length,
      "how far the shadow falls down and right, in cells",
      "It reaches the cells the offset rounded up covers, and its exact place where the \
       terminal shows graphics.";
    set_shadow_softening / derive_shadow_softening (softening) => ext.shadow_softening: Length,
      "how far the shadow's edge softens past its offset, in cells, where the terminal \
       shows graphics";
    set_scrollbar_thumb / derive_scrollbar_thumb (thumb) => ext.scrollbar_thumb: Color,
      "the color of the scrollbars' thumbs";
    set_scrollbar_track / derive_scrollbar_track (track) => ext.scrollbar_track: Color,
      "the color of the scrollbars' tracks";
    set_scrollbar_glyphs / derive_scrollbar_glyphs (glyphs) => ext.scrollbar_glyphs: ScrollbarGlyphs,
      "the characters the scrollbars draw with";
  }
}

impl HasChild<ActivePlatform> for Container {
  type Kind = PlainChild;

  #[inline(always)]
  fn child_mut(&mut self, seal: Seal) -> &mut Children<ActivePlatform, PlainChild> {
    self.0.child_mut(seal)
  }
}

impl AnyKind for Container {}

impl View<ActivePlatform> for Container {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
