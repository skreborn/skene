//! The views of the terminal's own, each forwarding to the foundation's implementation.

mod button;
mod checkbox;
mod container;
mod context_menu;
mod control;
mod enhance;
mod error_boundary;
mod flex;
mod flexible;
mod grid;
mod grid_cell;
mod keymap;
mod lazy_list;
mod link;
mod one_of;
mod overlay;
mod painted;
mod picker;
mod pinned;
mod radio;
mod slider;
mod span;
mod stack;
mod suspense;
mod text;
mod theme;
mod toggle;
mod tooltip;
mod visibility;

pub use self::{
  button::Button,
  checkbox::Checkbox,
  container::Container,
  context_menu::ContextMenu,
  control::{
    BasicButton,
    BasicCheckbox,
    BasicListbox,
    BasicListboxOption,
    BasicMenu,
    BasicMenuItem,
    BasicRadio,
    BasicRadioGroup,
    BasicSlider,
    BasicTextEditor,
    BasicTextField,
    Control,
  },
  enhance::Enhance,
  error_boundary::ErrorBoundary,
  flex::{Column, Row},
  flexible::Flexible,
  grid::Grid,
  grid_cell::GridCell,
  keymap::Keymap,
  lazy_list::{LazyColumn, LazyRow},
  link::Link,
  one_of::OneOf,
  overlay::Overlay,
  painted::Painted,
  picker::BasicPicker,
  pinned::Pinned,
  radio::{Radio, RadioGroup},
  slider::Slider,
  span::{RichText, Span},
  stack::Stack,
  suspense::Suspense,
  text::Text,
  theme::Theme,
  toggle::Toggle,
  tooltip::Tooltip,
  visibility::Visibility,
};

/// Generates the terminal's own decoration on a view whose declared box style `$access`
/// returns: the glyphs its border draws with, the shadow it casts and its scrollbars.
macro_rules! terminal_decoration {
  ($access:ident) => {
    ::skene_foundation::properties! { $access, crate::Active;
      set_border_glyphs / derive_border_glyphs (glyphs) => ext.border_glyphs: ::skene_foundation::terminal::BorderGlyphs,
        "the characters the border draws with";
      set_shadow / derive_shadow (shadow) => ext.shadow: ::skene_foundation::Color,
        "the shadow color",
        "Its alpha is the strength of the mix into the colors beneath where the shadow falls.";
      set_shadow_offset / derive_shadow_offset (offset) => ext.shadow_offset: ::skene_foundation::Length,
        "how far the shadow falls down and right, in cells",
        "It reaches the cells the offset rounded up covers, and its exact place where the \
         terminal shows graphics.";
      set_shadow_softening / derive_shadow_softening (softening) => ext.shadow_softening: ::skene_foundation::Length,
        "how far the shadow's edge softens past its offset, in cells, where the terminal \
         shows graphics";
      set_scrollbar_thumb / derive_scrollbar_thumb (thumb) => ext.scrollbar_thumb: ::skene_foundation::Color,
        "the color of the scrollbars' thumbs";
      set_scrollbar_track / derive_scrollbar_track (track) => ext.scrollbar_track: ::skene_foundation::Color,
        "the color of the scrollbars' tracks";
      set_scrollbar_glyphs / derive_scrollbar_glyphs (glyphs) => ext.scrollbar_glyphs: ::skene_foundation::terminal::ScrollbarGlyphs,
        "the characters the scrollbars draw with";
    }
  };
}

pub(crate) use terminal_decoration;
