//! The terminal's decoration and text setters on the facade's views: border glyphs, a
//! shadow, scrollbars, boldness.

use skene_foundation::{
  BoxStyle,
  Color,
  Condition,
  Declared,
  Length,
  RunStyle,
  terminal::{BorderGlyphs, ScrollbarGlyphs, TerminalStyle},
};

use crate::Terminal;

/// The declared style of a terminal box.
type DeclaredBox = BoxStyle<u32, Declared<Condition<u32>>, TerminalStyle>;

/// The declared style of a terminal run.
type DeclaredRun = RunStyle<Declared<Condition<u32>>, TerminalStyle>;

/// The terminal's own decoration on a box: the glyphs its border draws with, the
/// shadow it casts and its scrollbars.
///
/// Every view with a decorated box on the active platform has it: [`Container`](crate::Container),
/// [`Control`](crate::Control) and [`Overlay`](crate::Overlay).
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

impl TerminalDecoration for crate::Container {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredBox {
    self.style_mut()
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

/// The terminal's own text property: whether a run is bold.
///
/// Every text view on the active platform has it: [`Text`](crate::Text) and
/// [`Span`](crate::Span).
pub trait TerminalText: Sized {
  /// The declared style the weight lands on.
  fn terminal_style_mut(&mut self) -> &mut DeclaredRun;

  skene_foundation::trait_properties! { terminal_style_mut, Condition<u32>;
    set_bold / derive_bold (bold) => ext.bold: bool, "whether the text is bold";
  }
}

impl TerminalText for skene_foundation::Text<Terminal> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredRun {
    self.style_mut()
  }
}

impl TerminalText for skene_foundation::Span<Terminal> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredRun {
    self.style_mut()
  }
}
