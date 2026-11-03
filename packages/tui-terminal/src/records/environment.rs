//! What the terminal reports of its environment, which the drawn runtime keeps, and the
//! colors it shows under the scheme in force.

use skene_foundation::{Color, ColorScheme, Environment, Inset, PointerPrecision, Size, Token};

use super::{DEFAULT_SIZE, Records};
use crate::detect;

type SystemColors = skene_foundation::SystemColors<i32>;

/// The colors an unset cell shows: the ink and the base a paint mixes against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Contrast {
  pub(crate) ink: (u8, u8, u8),
  pub(crate) base: (u8, u8, u8),
}

/// The terminal's own colors, each where it reported it and the scheme in force is the
/// one its background implies: a cell painted in either shows the terminal's default
/// color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OwnColors {
  /// The terminal's own foreground.
  pub(crate) foreground: Option<(u8, u8, u8)>,
  /// The terminal's own background.
  pub(crate) background: Option<(u8, u8, u8)>,
}

impl OwnColors {
  /// Neither color reported, so every cell is written in its own color.
  pub(crate) const NONE: Self = Self {
    foreground: None,
    background: None,
  };
}

/// The environment the terminal reports until it learns otherwise: its default size, a
/// dark background, no preference for reduced motion, reduced transparency or more
/// contrast, which it has no answer for, the mouse fine and hovering, and no edge kept
/// clear.
pub(crate) const LAUNCH_ENVIRONMENT: Environment<i32> = Environment {
  viewport: Size::new(DEFAULT_SIZE.0 as i32, DEFAULT_SIZE.1 as i32),
  scheme: ColorScheme::Dark,
  reduced_motion: false,
  reduced_transparency: false,
  high_contrast: false,
  pointer: PointerPrecision::Fine,
  hover: true,
  safe_area: Inset::ZERO,
};

impl Records {
  /// Whether the scheme in force is dark: the app's choice when it has made one, and
  /// the background the terminal reported otherwise.
  pub(crate) const fn dark(&self) -> bool {
    matches!(self.runtime.scheme(), ColorScheme::Dark)
  }

  /// Whether the background the terminal reported is dark.
  pub(crate) const fn detected_dark(&self) -> bool {
    matches!(self.runtime.window().reported().scheme, ColorScheme::Dark)
  }

  /// The terminal's size in cells, as it last reported it.
  pub(crate) fn screen_size(&self) -> (u16, u16) {
    let Size { width, height } = self.runtime.window().reported().viewport;
    let cells = |length: i32| u16::try_from(length.max(0)).unwrap_or(u16::MAX);

    (cells(width), cells(height))
  }

  /// The colors an unset cell shows, which a paint mixes against: the terminal's
  /// own foreground and background where it reported them and the scheme in force is
  /// the detected one, else white on black for a dark scheme and black on white for a
  /// light one.
  pub(crate) fn contrast(&self) -> Contrast {
    let dark = self.dark();
    let defaults = if dark {
      Contrast {
        ink: (255, 255, 255),
        base: (0, 0, 0),
      }
    }
    else {
      Contrast {
        ink: (0, 0, 0),
        base: (255, 255, 255),
      }
    };

    if dark != self.detected_dark() {
      return defaults;
    }

    Contrast {
      ink: self.detected_foreground.unwrap_or(defaults.ink),
      base: self.detected_background.unwrap_or(defaults.base),
    }
  }

  /// The terminal's own colors, where it reported them and the scheme in force is the
  /// one its background implies.
  pub(crate) const fn own_colors(&self) -> OwnColors {
    if self.dark() != self.detected_dark() {
      return OwnColors::NONE;
    }

    OwnColors {
      foreground: self.detected_foreground,
      background: self.detected_background,
    }
  }

  /// The platform's colors as the terminal shows them: the ink and the window are the
  /// colors an unset cell shows ([`contrast`](Self::contrast)), the accent is the blue of
  /// its base colors (OSC 4) under the lighter or the darker of the two, whichever
  /// stands out on it, and the rest are mixes of the ink into the window.
  pub(crate) fn system_colors(&self) -> SystemColors {
    let color = |(red, green, blue): (u8, u8, u8)| {
      Color::srgb(
        f32::from(red) / 255.,
        f32::from(green) / 255.,
        f32::from(blue) / 255.,
      )
    };
    let Contrast { ink, base } = self.contrast();
    let accent = self.palette.color(4);
    let (light, dark) = if detect::is_dark(ink) {
      (base, ink)
    }
    else {
      (ink, base)
    };
    let on_accent = if detect::is_dark(accent) { light } else { dark };
    let (ink, window, accent) = (color(ink), color(base), color(accent));
    let tint = |amount: f32| window.mix(ink, amount);
    let token =
      |reference: skene_foundation::TokenRef<Color>, value: Color| Token::new(reference, value);

    SystemColors {
      ink: token(SystemColors::INK, ink),
      muted_ink: token(SystemColors::MUTED_INK, tint(0.6)),
      window: token(SystemColors::WINDOW, window),
      control: token(SystemColors::CONTROL, tint(0.15)),
      control_hover: token(SystemColors::CONTROL_HOVER, tint(0.25)),
      control_pressed: token(SystemColors::CONTROL_PRESSED, tint(0.35)),
      border: token(SystemColors::BORDER, tint(0.5)),
      accent: token(SystemColors::ACCENT, accent),
      accent_hover: token(SystemColors::ACCENT_HOVER, accent.mix(ink, 0.15)),
      accent_pressed: token(SystemColors::ACCENT_PRESSED, accent.mix(ink, 0.3)),
      on_accent: token(SystemColors::ON_ACCENT, color(on_accent)),
      track: token(SystemColors::TRACK, tint(0.3)),
      knob: token(SystemColors::KNOB, ink),
      field: token(SystemColors::FIELD, tint(0.08)),
      selection: token(SystemColors::SELECTION, window.mix(accent, 0.5)),
      selection_inactive: token(SystemColors::SELECTION_INACTIVE, tint(0.25)),
    }
  }
}
