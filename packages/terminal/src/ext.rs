//! The terminal's decoration on the facade's views without it of their own: border
//! glyphs, a shadow, scrollbars.

use skene_foundation::{
  BoxStyle,
  Color,
  Condition,
  Declared,
  Length,
  terminal::{BorderGlyphs, ScrollbarGlyphs, TerminalStyle},
};

use crate::Terminal;

/// The declared style of a terminal box.
type DeclaredBox = BoxStyle<u32, Declared<Condition<u32>>, TerminalStyle>;

/// The terminal's own decoration on a box: the glyphs its border draws with, the
/// shadow it casts and its scrollbars.
///
/// The decorated views on the active platform without the decoration of their own have
/// it: [`Control`](crate::Control) and [`Overlay`](crate::Overlay).
pub trait TerminalDecoration: Sized {
  /// The declared style the decoration lands on.
  fn terminal_style_mut(&mut self) -> &mut DeclaredBox;

  skene_foundation::trait_properties! { terminal_style_mut, Condition<u32>;
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

impl<B: skene_foundation::Behavior<Terminal>> TerminalDecoration
  for skene_foundation::Control<Terminal, B>
{
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredBox {
    self.style_mut()
  }
}

impl TerminalDecoration for skene_foundation::Overlay<Terminal> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredBox {
    self.style_mut()
  }
}
