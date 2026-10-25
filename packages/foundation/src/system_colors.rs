//! The platform's colors, as CSS's system colors are: what the built-in controls'
//! defaults name, defined at the root by each realization from its platform.

use crate::{Color, Definition, Media, Scalar, Token, TokenName, TokenRef, TokenSet};

/// The platform's colors, as CSS's system colors are: what the built-ins' defaults name,
/// defined at the root by each realization from its platform and redefined by any theme
/// beneath.
///
/// The web defines them as CSS's system colors, so a page follows the platform and its
/// color scheme with no script; the terminal defines them from the colors it reports.
/// A set of one's own that defines one of the names restyles every built-in beneath its
/// theme that reads it, and a theme of this set replaces every one of them with the set's
/// definitions. The default is a neutral light look turning dark under the dark scheme,
/// which [`ThemedIn`](crate::ThemedIn) hands out where no theme provides the set.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct SystemColors<N: Scalar> {
  /// [`SystemColors::INK`].
  pub ink: Token<Color, N>,
  /// [`SystemColors::MUTED_INK`].
  pub muted_ink: Token<Color, N>,
  /// [`SystemColors::WINDOW`].
  pub window: Token<Color, N>,
  /// [`SystemColors::CONTROL`].
  pub control: Token<Color, N>,
  /// [`SystemColors::CONTROL_HOVER`].
  pub control_hover: Token<Color, N>,
  /// [`SystemColors::CONTROL_PRESSED`].
  pub control_pressed: Token<Color, N>,
  /// [`SystemColors::BORDER`].
  pub border: Token<Color, N>,
  /// [`SystemColors::ACCENT`].
  pub accent: Token<Color, N>,
  /// [`SystemColors::ACCENT_HOVER`].
  pub accent_hover: Token<Color, N>,
  /// [`SystemColors::ACCENT_PRESSED`].
  pub accent_pressed: Token<Color, N>,
  /// [`SystemColors::ON_ACCENT`].
  pub on_accent: Token<Color, N>,
  /// [`SystemColors::TRACK`].
  pub track: Token<Color, N>,
  /// [`SystemColors::KNOB`].
  pub knob: Token<Color, N>,
  /// [`SystemColors::FIELD`].
  pub field: Token<Color, N>,
  /// [`SystemColors::SELECTION`].
  pub selection: Token<Color, N>,
  /// [`SystemColors::SELECTION_INACTIVE`].
  pub selection_inactive: Token<Color, N>,
}

impl<N: Scalar> SystemColors<N> {
  /// Text on the window and on a control.
  pub const INK: TokenRef<Color> = TokenRef::new("system-ink");
  /// Text that says less: a placeholder.
  pub const MUTED_INK: TokenRef<Color> = TokenRef::new("system-muted-ink");
  /// The window behind every view.
  pub const WINDOW: TokenRef<Color> = TokenRef::new("system-window");
  /// A control's fill.
  pub const CONTROL: TokenRef<Color> = TokenRef::new("system-control");
  /// A control's fill under the pointer.
  pub const CONTROL_HOVER: TokenRef<Color> = TokenRef::new("system-control-hover");
  /// A control's fill while pressed.
  pub const CONTROL_PRESSED: TokenRef<Color> = TokenRef::new("system-control-pressed");
  /// A control's border.
  pub const BORDER: TokenRef<Color> = TokenRef::new("system-border");
  /// The accent: a prominent button, an on switch, a checked mark's face, a slider's
  /// fill, a focus ring.
  pub const ACCENT: TokenRef<Color> = TokenRef::new("system-accent");
  /// The accent under the pointer.
  pub const ACCENT_HOVER: TokenRef<Color> = TokenRef::new("system-accent-hover");
  /// The accent while pressed.
  pub const ACCENT_PRESSED: TokenRef<Color> = TokenRef::new("system-accent-pressed");
  /// What sits on the accent.
  pub const ON_ACCENT: TokenRef<Color> = TokenRef::new("system-on-accent");
  /// A switch's track while off, a slider's track.
  pub const TRACK: TokenRef<Color> = TokenRef::new("system-track");
  /// A switch's and a slider's thumb.
  pub const KNOB: TokenRef<Color> = TokenRef::new("system-knob");
  /// A text field's fill, a checkbox's face, an open list.
  pub const FIELD: TokenRef<Color> = TokenRef::new("system-field");
  /// A text entry's selection while it has focus.
  pub const SELECTION: TokenRef<Color> = TokenRef::new("system-selection");
  /// A text entry's selection while it has none.
  pub const SELECTION_INACTIVE: TokenRef<Color> = TokenRef::new("system-selection-inactive");
  /// Every name the set defines, in the order of its fields.
  pub const NAMES: [TokenRef<Color>; 16] = [
    Self::INK,
    Self::MUTED_INK,
    Self::WINDOW,
    Self::CONTROL,
    Self::CONTROL_HOVER,
    Self::CONTROL_PRESSED,
    Self::BORDER,
    Self::ACCENT,
    Self::ACCENT_HOVER,
    Self::ACCENT_PRESSED,
    Self::ON_ACCENT,
    Self::TRACK,
    Self::KNOB,
    Self::FIELD,
    Self::SELECTION,
    Self::SELECTION_INACTIVE,
  ];
}

/// A definition taking `dark`'s value under the dark scheme and `light`'s otherwise.
fn pair<N: Scalar>(light: Color, dark: Color) -> Definition<Color, N> {
  Definition::from(light).when(Media::Dark, dark)
}

/// An opaque sRGB color from its `0xRRGGBB` packing.
fn srgb(rgb: u32) -> Color {
  let [_, red, green, blue] = rgb.to_be_bytes().map(|channel| f32::from(channel) / 255.);

  Color::srgb(red, green, blue)
}

impl<N: Scalar> Default for SystemColors<N> {
  /// A near-black ink on a near-white window with light gray controls, turning to a
  /// near-white ink on a dark gray window with darker controls under the dark scheme,
  /// and a blue accent under white in both.
  fn default() -> Self {
    let accent = srgb(0x35_84e4);
    let black = Color::srgb(0., 0., 0.);
    let white = Color::srgb(1., 1., 1.);
    let token = |reference: TokenRef<Color>, definition: Definition<Color, N>| {
      Token::new(reference.name().as_str(), definition)
    };

    Self {
      ink: token(Self::INK, pair(srgb(0x1d_1d1f), srgb(0xf5_f5f7))),
      muted_ink: token(Self::MUTED_INK, pair(srgb(0x8e_8e93), srgb(0x98_989d))),
      window: token(Self::WINDOW, pair(srgb(0xfa_fafa), srgb(0x24_2424))),
      control: token(Self::CONTROL, pair(srgb(0xe8_e8ed), srgb(0x3a_3a3c))),
      control_hover: token(Self::CONTROL_HOVER, pair(srgb(0xdc_dce2), srgb(0x48_484a))),
      control_pressed: token(
        Self::CONTROL_PRESSED,
        pair(srgb(0xc8_c8d0), srgb(0x5a_5a5e)),
      ),
      border: token(Self::BORDER, pair(srgb(0xc6_c6cc), srgb(0x54_5458))),
      accent: token(Self::ACCENT, Definition::from(accent)),
      accent_hover: token(Self::ACCENT_HOVER, Definition::from(accent.mix(black, 0.1))),
      accent_pressed: token(
        Self::ACCENT_PRESSED,
        Definition::from(accent.mix(black, 0.2)),
      ),
      on_accent: token(Self::ON_ACCENT, Definition::from(white)),
      track: token(Self::TRACK, pair(srgb(0xd1_d1d6), srgb(0x48_484a))),
      knob: token(Self::KNOB, pair(white, srgb(0xf2_f2f7))),
      field: token(Self::FIELD, pair(white, srgb(0x1c_1c1e))),
      selection: token(Self::SELECTION, Definition::from(accent.with_alpha(0.33))),
      selection_inactive: token(
        Self::SELECTION_INACTIVE,
        pair(
          srgb(0x8e_8e93).with_alpha(0.25),
          srgb(0x98_989d).with_alpha(0.25),
        ),
      ),
    }
  }
}

impl<N: Scalar, V: From<Definition<Color, N>>> TokenSet<V> for SystemColors<N> {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![
      self.ink.define(),
      self.muted_ink.define(),
      self.window.define(),
      self.control.define(),
      self.control_hover.define(),
      self.control_pressed.define(),
      self.border.define(),
      self.accent.define(),
      self.accent_hover.define(),
      self.accent_pressed.define(),
      self.on_accent.define(),
      self.track.define(),
      self.knob.define(),
      self.field.define(),
      self.selection.define(),
      self.selection_inactive.define(),
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::SystemColors;
  use crate::{Color, Definition, Definitions, Media, StaticLeaf, TokenRef};

  /// The default defines every name once, in the order of the names, the window and the
  /// controls turning darker and the ink lighter under the dark scheme, the accent and
  /// what sits on it alike in both.
  #[test]
  fn the_default_defines_every_name_for_both_schemes() {
    let colors = SystemColors::<f32>::default();
    let definitions: Definitions<Definition<Color, f32>> = Definitions::of(&colors);
    let names = definitions.iter().map(|(name, _)| name).collect::<Vec<_>>();
    let schemes = |reference: TokenRef<Color>| {
      let definition = definitions
        .get(reference.name())
        .expect("the default defines every name");
      let StaticLeaf::Fixed(light) = definition.base()
      else {
        panic!("{} is a fixed color", reference.name());
      };
      let dark = match definition.variants() {
        [] => *light,
        [(Media::Dark, StaticLeaf::Fixed(dark))] => *dark,
        _ => panic!("{} answers the dark scheme alone", reference.name()),
      };

      (*light, dark)
    };

    assert_eq!(names, SystemColors::<f32>::NAMES.map(TokenRef::name));
    assert_eq!(colors.ink.reference(), SystemColors::<f32>::INK);

    for reference in [
      SystemColors::<f32>::WINDOW,
      SystemColors::<f32>::CONTROL,
      SystemColors::<f32>::FIELD,
    ] {
      let (light, dark) = schemes(reference);

      assert!(
        dark.lightness() < light.lightness(),
        "{} darkens",
        reference.name()
      );
    }

    let (light, dark) = schemes(SystemColors::<f32>::INK);

    assert!(dark.lightness() > light.lightness(), "the ink lightens");

    for reference in [SystemColors::<f32>::ACCENT, SystemColors::<f32>::ON_ACCENT] {
      let (light, dark) = schemes(reference);

      assert_eq!(
        light,
        dark,
        "{} is one color in both schemes",
        reference.name()
      );
    }

    let (on_accent, _) = schemes(SystemColors::<f32>::ON_ACCENT);
    let (accent, _) = schemes(SystemColors::<f32>::ACCENT);

    assert!(
      on_accent.lightness() > accent.lightness(),
      "white sits on the accent"
    );
  }
}
