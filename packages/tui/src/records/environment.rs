//! The environment the terminal reports: the scheme in force and its colors, the
//! viewport and the pointer, as values and as signals, and the resolution's source.

use skene_foundation::{
  Color,
  ColorScheme,
  ContainerName,
  Environment,
  Inset,
  MediaSignals,
  NodeId,
  PointerPrecision,
  ResolutionSource,
  Size,
  Slot,
};
use skene_reactive::{Owner, ReadSignal, Signal, WriteSignal};

use super::{Records, record::SlotLeaf};
use crate::{SystemColors, Terminal, Token, detect};

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

/// The environment as signals, with the write halves the platform drives.
pub(crate) struct MediaState {
  /// The root owner the signals live under.
  pub(crate) owner: Owner,
  /// Writes the viewport.
  pub(crate) set_viewport: WriteSignal<Size<i32>>,
  /// Writes the color scheme in force.
  pub(crate) set_scheme: WriteSignal<ColorScheme>,
  /// Writes whether the reader prefers reduced motion.
  pub(crate) set_reduced_motion: WriteSignal<bool>,
  /// Whether the terminal proved a graphics protocol.
  pub(crate) graphics: ReadSignal<bool>,
  /// Writes whether the terminal proved a graphics protocol.
  pub(crate) set_graphics: WriteSignal<bool>,
  /// The environment's signals.
  pub(crate) signals: MediaSignals<i32>,
}

impl ResolutionSource<Terminal> for Records {
  /// The terminal draws every size at one cell where it proves no text sizing.
  fn environment(&self) -> Environment<i32> {
    Self::environment(self)
  }

  fn container_size(&self, name: ContainerName) -> Option<Size<i32>> {
    self
      .media
      .as_ref()?
      .signals
      .container_sizes
      .get_untracked(name)
  }

  fn slot(&self, node: NodeId, slot: Slot) -> Option<SlotLeaf> {
    self.record(node)?.slots.get(&slot).cloned()
  }

  fn sizes_text(&self) -> bool {
    crate::text::sizing()
  }

  fn generation(&self) -> u64 {
    self.restyle.generation()
  }
}

impl Records {
  /// Whether the scheme in force is dark: the app's choice when it has made one, and
  /// the background the terminal reported otherwise.
  pub(crate) const fn dark(&self) -> bool {
    match self.choice {
      Some(ColorScheme::Light) => false,
      Some(ColorScheme::Dark) => true,
      None => self.detected_dark,
    }
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

    if dark != self.detected_dark {
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
    if self.dark() != self.detected_dark {
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

  /// The color scheme in force.
  #[inline(always)]
  pub(crate) const fn scheme(&self) -> ColorScheme {
    if self.dark() {
      ColorScheme::Dark
    }
    else {
      ColorScheme::Light
    }
  }

  /// The viewport as the last layout measured it.
  #[inline(always)]
  pub(crate) fn viewport(&self) -> Size<i32> {
    Size::new(i32::from(self.last_size.0), i32::from(self.last_size.1))
  }

  /// The environment the terminal reports; it has no answer for reduced transparency
  /// or more contrast, which never hold, its pointer is the mouse, fine and hovering
  /// where the app takes it and absent where it does not, and it keeps no edge clear.
  pub(crate) fn environment(&self) -> Environment<i32> {
    Environment {
      viewport: self.viewport(),
      scheme: self.scheme(),
      reduced_motion: self.reduced_motion,
      reduced_transparency: false,
      high_contrast: false,
      pointer: self.pointer(),
      hover: self.mouse,
      safe_area: Inset::ZERO,
    }
  }

  /// The pointer the terminal reports: the mouse, where the app takes it.
  pub(crate) const fn pointer(&self) -> PointerPrecision {
    if self.mouse {
      PointerPrecision::Fine
    }
    else {
      PointerPrecision::Absent
    }
  }

  /// Marks every resolved style stale and the frame in need of layout.
  #[inline(always)]
  pub(crate) const fn invalidate_styles(&mut self) {
    self.restyle.invalidate();
    self.layout_dirty = true;
    self.paint_dirty = true;
  }

  /// The environment's signals, created on first use under a root owner of their own.
  pub(crate) fn media(&mut self) -> &MediaState {
    let viewport = self.viewport();
    let scheme = self.scheme();
    let reduced = self.reduced_motion;
    let pointer = self.pointer();
    let hover = self.mouse;
    let graphics = self.graphics;

    self.media.get_or_insert_with(|| {
      let owner = Owner::new();

      owner.enter(|| {
        let (signals, writers) = MediaSignals::new(Environment {
          viewport,
          scheme,
          reduced_motion: reduced,
          reduced_transparency: false,
          high_contrast: false,
          pointer,
          hover,
          safe_area: Inset::ZERO,
        });
        let (graphics, set_graphics) = Signal::new(graphics).split();

        MediaState {
          owner,
          set_viewport: writers.viewport,
          set_scheme: writers.scheme,
          set_reduced_motion: writers.reduced_motion,
          graphics,
          set_graphics,
          signals,
        }
      })
    })
  }

  /// The owner of the environment's signals, for a teardown to dispose.
  pub(crate) fn take_media_owner(&mut self) -> Option<Owner> {
    self.media.take().map(|media| media.owner)
  }
}
