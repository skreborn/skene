//! The Simple component library's shared design language: one neutral palette, one
//! shape vocabulary, and the emphasis variants—the same tokens the GUI Simple library
//! carries, expressed in cells.
//!
//! Every `Simple*` view composes the raw controls through their public surface alone,
//! so a third-party component library sits on exactly the same footing.

use skene_core::Color;

use crate::props::{BorderGlyphs, BoxStyle, Condition};

/// The border every Simple surface draws with—arc corners, the terminal's radius.
pub(crate) const GLYPHS: BorderGlyphs = BorderGlyphs::ROUNDED;

/// The Simple components' design tokens: a neutral dark palette shared by every
/// platform's Simple library.
///
/// Use the tokens for chrome around the components—page backgrounds, captions,
/// separators—so an app reads as one surface.
#[must_use]
pub struct SimplePalette;

impl SimplePalette {
  /// The page background.
  #[inline(always)]
  pub fn background() -> Color {
    Color::srgb(0.035, 0.035, 0.043)
  }

  /// The default text ink.
  #[inline(always)]
  pub fn foreground() -> Color {
    Color::srgb(0.98, 0.98, 0.98)
  }

  /// A raised surface: cards, popovers, modals.
  #[inline(always)]
  pub fn card() -> Color {
    Color::srgb(0.094, 0.094, 0.106)
  }

  /// Hairline borders and separators.
  #[inline(always)]
  pub fn border() -> Color {
    Color::srgb(0.153, 0.153, 0.165)
  }

  /// The primary action surface.
  #[inline(always)]
  pub fn primary() -> Color {
    Color::srgb(0.98, 0.98, 0.98)
  }

  /// Ink on the primary surface.
  #[inline(always)]
  pub fn primary_foreground() -> Color {
    Color::srgb(0.094, 0.094, 0.106)
  }

  /// The secondary action surface.
  #[inline(always)]
  pub fn secondary() -> Color {
    Color::srgb(0.153, 0.153, 0.165)
  }

  /// Ink on the secondary surface.
  #[inline(always)]
  pub fn secondary_foreground() -> Color {
    Color::srgb(0.98, 0.98, 0.98)
  }

  /// De-emphasized text: captions, hints, section titles.
  #[inline(always)]
  pub fn muted_foreground() -> Color {
    Color::srgb(0.631, 0.631, 0.667)
  }

  /// The destructive action surface.
  #[inline(always)]
  pub fn destructive() -> Color {
    Color::srgb(0.863, 0.149, 0.149)
  }

  /// Ink on the destructive surface.
  #[inline(always)]
  pub fn destructive_foreground() -> Color {
    Color::srgb(0.98, 0.98, 0.98)
  }

  /// The focused control's border ink.
  #[inline(always)]
  pub fn ring() -> Color {
    Color::srgb(0.631, 0.631, 0.667)
  }
}

/// The shared field chrome: padded, hairline-bordered, rounded.
pub(crate) fn field_style() -> BoxStyle {
  BoxStyle::new()
    .set_padding(crate::Inset::horizontal(1))
    .set_border(SimplePalette::border())
    .set_border_glyphs(GLYPHS)
}

/// The shared field focus variant: the border brightens to the ring.
pub(crate) fn field_focus() -> (Condition, BoxStyle) {
  (
    Condition::Focused,
    BoxStyle::new().set_border(SimplePalette::ring()),
  )
}

/// A Simple action's emphasis, selecting its surface and ink.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub enum SimpleVariant {
  /// The main action: a high-contrast surface.
  #[default]
  Primary,
  /// A supporting action: a subdued surface.
  Secondary,
  /// A hairline-bordered action on the ambient background.
  Outline,
  /// A bare action, surfacing only under the pointer.
  Ghost,
  /// A dangerous action: the destructive surface.
  Destructive,
}

/// One variant's resolved colors.
pub(crate) struct Look {
  pub(crate) surface: Color,
  pub(crate) ink: Color,
  pub(crate) border: Option<Color>,
  pub(crate) hover: Color,
  pub(crate) focus: Color,
}

impl SimpleVariant {
  /// The variant's resolved colors; the terminal authors its focus shade too, since the
  /// built-in white-ink wash would vanish on the light primary surface.
  pub(crate) fn look(self) -> Look {
    match self {
      Self::Primary => Look {
        surface: SimplePalette::primary(),
        ink: SimplePalette::primary_foreground(),
        border: None,
        hover: Color::srgb(0.894, 0.894, 0.906),
        focus: Color::srgb(0.831, 0.831, 0.855),
      },
      Self::Secondary => Look {
        surface: SimplePalette::secondary(),
        ink: SimplePalette::secondary_foreground(),
        border: None,
        hover: Color::srgb(0.247, 0.247, 0.275),
        focus: Color::srgb(0.322, 0.322, 0.357),
      },
      Self::Outline => Look {
        surface: Color::TRANSPARENT,
        ink: SimplePalette::foreground(),
        border: Some(SimplePalette::border()),
        hover: SimplePalette::secondary(),
        focus: Color::srgb(0.247, 0.247, 0.275),
      },
      Self::Ghost => Look {
        surface: Color::TRANSPARENT,
        ink: SimplePalette::foreground(),
        border: None,
        hover: SimplePalette::secondary(),
        focus: Color::srgb(0.247, 0.247, 0.275),
      },
      Self::Destructive => Look {
        surface: SimplePalette::destructive(),
        ink: SimplePalette::destructive_foreground(),
        border: None,
        hover: Color::srgb(0.725, 0.11, 0.11),
        focus: Color::srgb(0.6, 0.106, 0.106),
      },
    }
  }
}
