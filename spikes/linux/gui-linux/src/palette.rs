//! The colors and sizes the realization draws with where a view sets none: the ink of a text
//! no ancestor colors, and the default look of the controls it draws, each answering the
//! scheme, the contrast and the reader's accent in force.

use skene_foundation::{Color, Environment};

/// The realization's defaults for the environment in force.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Palette {
  /// Text with no color of its own.
  pub ink: Color,
  /// Text that says less (a placeholder).
  pub dim: Color,
  /// The window behind every view.
  pub window: Color,
  /// A control's fill.
  pub control: Color,
  /// A control's fill under the pointer.
  pub control_hover: Color,
  /// A control's fill while pressed.
  pub control_pressed: Color,
  /// A control's outline.
  pub outline: Color,
  /// The accent: a prominent button, an on switch, a slider's fill, the focus ring.
  pub accent: Color,
  /// Text on the accent.
  pub on_accent: Color,
  /// A switch's track while off, a slider's track.
  pub track: Color,
  /// A switch's or a slider's knob.
  pub knob: Color,
  /// A field's fill.
  pub field: Color,
  /// The title bar's fill.
  pub header: Color,
}

/// The accent where the reader states none.
pub const DEFAULT_ACCENT: Color = Color::rgb(0x3584e4);

/// The font size of a text whose ancestors set none, in pixels.
pub const DEFAULT_SIZE: f32 = 14.;

impl Palette {
  /// The defaults for the environment in force now, subscribing the running effect to the
  /// scheme, the contrast and the accent.
  #[must_use]
  pub fn current() -> Self {
    let environment = Environment::current();

    Self::of(
      environment.dark.get(),
      environment.high_contrast.get(),
      environment.accent.get(),
    )
  }

  /// The defaults for the environment in force now, subscribing nothing.
  #[must_use]
  pub fn now() -> Self {
    skene_foundation::untrack(Self::current)
  }

  /// The defaults in the dark scheme or not, with more contrast or not, in `accent`.
  #[must_use]
  pub const fn of(dark: bool, high_contrast: bool, accent: Option<Color>) -> Self {
    let accent = match accent {
      Some(accent) => accent,
      None => DEFAULT_ACCENT,
    };

    match (dark, high_contrast) {
      (false, false) => Self {
        ink: Color::rgb(0x1d1d1f),
        dim: Color::rgb(0x8e8e93),
        window: Color::rgb(0xfafafa),
        control: Color::rgb(0xe8e8ed),
        control_hover: Color::rgb(0xdcdce2),
        control_pressed: Color::rgb(0xc8c8d0),
        outline: Color::rgb(0xc6c6cc),
        accent,
        on_accent: Color::WHITE,
        track: Color::rgb(0xd1d1d6),
        knob: Color::WHITE,
        field: Color::WHITE,
        header: Color::rgb(0xebebeb),
      },
      (true, false) => Self {
        ink: Color::rgb(0xf5f5f7),
        dim: Color::rgb(0x98989d),
        window: Color::rgb(0x242424),
        control: Color::rgb(0x3a3a3c),
        control_hover: Color::rgb(0x48484a),
        control_pressed: Color::rgb(0x5a5a5e),
        outline: Color::rgb(0x545458),
        accent,
        on_accent: Color::WHITE,
        track: Color::rgb(0x48484a),
        knob: Color::rgb(0xf2f2f7),
        field: Color::rgb(0x1c1c1e),
        header: Color::rgb(0x303030),
      },
      (false, true) => Self {
        ink: Color::BLACK,
        dim: Color::rgb(0x3a3a3c),
        window: Color::WHITE,
        control: Color::WHITE,
        control_hover: Color::rgb(0xe5e5ea),
        control_pressed: Color::rgb(0xc7c7cc),
        outline: Color::BLACK,
        accent: Color::rgb(0x0040dd),
        on_accent: Color::WHITE,
        track: Color::rgb(0x8e8e93),
        knob: Color::WHITE,
        field: Color::WHITE,
        header: Color::rgb(0xe5e5ea),
      },
      (true, true) => Self {
        ink: Color::WHITE,
        dim: Color::rgb(0xd1d1d6),
        window: Color::BLACK,
        control: Color::BLACK,
        control_hover: Color::rgb(0x2c2c2e),
        control_pressed: Color::rgb(0x48484a),
        outline: Color::WHITE,
        accent: Color::rgb(0x409cff),
        on_accent: Color::BLACK,
        track: Color::rgb(0x8e8e93),
        knob: Color::WHITE,
        field: Color::BLACK,
        header: Color::rgb(0x1c1c1e),
      },
    }
  }
}
