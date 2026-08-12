//! # skene-simple
//!
//! The Simple component library's shared design language: one neutral palette and the
//! emphasis variants with their resolved colors, expressed over the platform-agnostic
//! color vocabulary. Each facade builds its own `Simple*` views over its raw controls
//! and imports this crate for the colors, so the GUI and the terminal read as one
//! design wherever the medium allows.

use skene_core::Color;

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

  /// The focused or hovered control's border ink.
  #[inline(always)]
  pub fn ring() -> Color {
    Color::srgb(0.631, 0.631, 0.667)
  }
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

/// One variant's resolved colors, for a facade realizing a Simple action.
#[must_use]
pub struct Look {
  /// The action's surface.
  pub surface: Color,
  /// The ink on that surface.
  pub ink: Color,
  /// The hairline border, on the variants that carry one.
  pub border: Option<Color>,
  /// The surface shown under the pointer.
  pub hover: Color,
}

impl SimpleVariant {
  /// The variant's resolved colors.
  pub fn look(self) -> Look {
    match self {
      Self::Primary => Look {
        surface: SimplePalette::primary(),
        ink: SimplePalette::primary_foreground(),
        border: None,
        hover: Color::srgb(0.894, 0.894, 0.906),
      },
      Self::Secondary => Look {
        surface: SimplePalette::secondary(),
        ink: SimplePalette::secondary_foreground(),
        border: None,
        hover: Color::srgb(0.247, 0.247, 0.275),
      },
      Self::Outline => Look {
        surface: Color::TRANSPARENT,
        ink: SimplePalette::foreground(),
        border: Some(SimplePalette::border()),
        hover: SimplePalette::secondary(),
      },
      Self::Ghost => Look {
        surface: Color::TRANSPARENT,
        ink: SimplePalette::foreground(),
        border: None,
        hover: SimplePalette::secondary(),
      },
      Self::Destructive => Look {
        surface: SimplePalette::destructive(),
        ink: SimplePalette::destructive_foreground(),
        border: None,
        hover: Color::srgb(0.725, 0.11, 0.11),
      },
    }
  }
}
