//! The Simple design language's color data: the token set, its light and dark
//! instances, and the emphasis variants resolved against it.

use skene_core::{Color, ColorScheme};

/// One emphasis variant's colors: the surface it paints, the ink on that surface, and
/// the surfaces its interaction states paint instead.
#[derive(Debug, Clone, Copy, PartialEq)]
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
  /// The surface shown while focused, on platforms without a focus ring of their own.
  pub focus: Color,
}

/// The Simple components' design tokens: the chrome colors and one [`Look`] per
/// emphasis variant.
///
/// Two instances ship—[`light`](Self::light) and [`dark`](Self::dark)—and a
/// facade's theme view holds both, so which one a subtree paints with is the
/// platform's answer rather than the app's. Use the tokens for the chrome around the
/// components too—page backgrounds, captions, separators—so an app reads as one
/// surface.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Palette {
  /// The page background.
  pub background: Color,
  /// The default text ink.
  pub foreground: Color,
  /// De-emphasized text: captions, hints, section titles.
  pub muted_foreground: Color,
  /// A raised surface: cards, popovers, modals.
  pub card: Color,
  /// Hairline borders and separators.
  pub border: Color,
  /// The focused or hovered control's border ink.
  pub ring: Color,
  /// The main action: a high-contrast surface.
  pub primary: Look,
  /// A supporting action: a subdued surface.
  pub secondary: Look,
  /// A hairline-bordered action on the ambient background.
  pub outline: Look,
  /// A bare action, surfacing only under the pointer.
  pub ghost: Look,
  /// A dangerous action.
  pub destructive: Look,
}

impl Palette {
  /// The light palette: dark ink on a near-white page.
  pub fn light() -> Self {
    let background = Color::srgb(0.98, 0.98, 0.98);
    let foreground = Color::srgb(0.035, 0.035, 0.043);
    let border = Color::srgb(0.894, 0.894, 0.906);
    let subdued = Color::srgb(0.957, 0.957, 0.961);
    let muted_foreground = Color::srgb(0.443, 0.443, 0.478);

    Self {
      background,
      foreground,
      muted_foreground,
      card: Color::srgb(1., 1., 1.),
      border,
      ring: muted_foreground,
      primary: Look {
        surface: Color::srgb(0.094, 0.094, 0.106),
        ink: Color::srgb(0.98, 0.98, 0.98),
        border: None,
        hover: Color::srgb(0.153, 0.153, 0.165),
        focus: Color::srgb(0.247, 0.247, 0.275),
      },
      secondary: Look {
        surface: subdued,
        ink: foreground,
        border: None,
        hover: border,
        focus: Color::srgb(0.831, 0.831, 0.847),
      },
      outline: Look {
        surface: Color::TRANSPARENT,
        ink: foreground,
        border: Some(border),
        hover: subdued,
        focus: border,
      },
      ghost: Look {
        surface: Color::TRANSPARENT,
        ink: foreground,
        border: None,
        hover: subdued,
        focus: border,
      },
      destructive: destructive(),
    }
  }

  /// The dark palette: light ink on a near-black page.
  pub fn dark() -> Self {
    let foreground = Color::srgb(0.98, 0.98, 0.98);
    let border = Color::srgb(0.153, 0.153, 0.165);
    let raised = Color::srgb(0.247, 0.247, 0.275);
    let muted_foreground = Color::srgb(0.631, 0.631, 0.667);

    Self {
      background: Color::srgb(0.035, 0.035, 0.043),
      foreground,
      muted_foreground,
      card: Color::srgb(0.094, 0.094, 0.106),
      border,
      ring: muted_foreground,
      primary: Look {
        surface: foreground,
        ink: Color::srgb(0.094, 0.094, 0.106),
        border: None,
        hover: Color::srgb(0.894, 0.894, 0.906),
        focus: Color::srgb(0.831, 0.831, 0.855),
      },
      secondary: Look {
        surface: border,
        ink: foreground,
        border: None,
        hover: raised,
        focus: Color::srgb(0.322, 0.322, 0.357),
      },
      outline: Look {
        surface: Color::TRANSPARENT,
        ink: foreground,
        border: Some(border),
        hover: border,
        focus: raised,
      },
      ghost: Look {
        surface: Color::TRANSPARENT,
        ink: foreground,
        border: None,
        hover: border,
        focus: raised,
      },
      destructive: destructive(),
    }
  }

  /// The palette of `scheme`.
  #[inline(always)]
  pub fn of(scheme: ColorScheme) -> Self {
    match scheme {
      ColorScheme::Light => Self::light(),
      ColorScheme::Dark => Self::dark(),
    }
  }
}

/// The destructive action's colors, which carry their own contrast and so read the
/// same in both schemes.
fn destructive() -> Look {
  Look {
    surface: Color::srgb(0.863, 0.149, 0.149),
    ink: Color::srgb(0.98, 0.98, 0.98),
    border: None,
    hover: Color::srgb(0.725, 0.11, 0.11),
    focus: Color::srgb(0.6, 0.106, 0.106),
  }
}

/// A Simple action's emphasis, selecting its surface and ink.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

impl SimpleVariant {
  /// The variant's colors in `palette`.
  #[inline(always)]
  pub fn look(self, palette: &Palette) -> Look {
    match self {
      Self::Primary => palette.primary,
      Self::Secondary => palette.secondary,
      Self::Outline => palette.outline,
      Self::Ghost => palette.ghost,
      Self::Destructive => palette.destructive,
    }
  }
}
