//! The token definitions the web accepts, their custom-property registrations, and the
//! platform's colors the root defines.

use std::any::TypeId;

use skene_foundation::{
  Color,
  Definition,
  Definitions,
  Dimension,
  Length,
  Media,
  Scene,
  TokenName,
  TokenRef,
  TokenReferences,
  TokenValue,
};

use crate::{SystemColors, condition::Conjunction, platform::web::Web};

/// The erased token definition the web platform stores per token: a color, a number, a
/// length, or a dimension.
///
/// Each kind registers its custom property with the matching `@property` syntax, so a
/// token interpolates and inherits as a typed value.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum WebTokenValue {
  /// A color token.
  Color(Definition<Color, f32>),
  /// A unitless number token; a property measured in pixels reads it as pixels.
  Number(Definition<f32, f32>),
  /// A length token, in pixels.
  Length(Definition<Length, f32>),
  /// A dimension token.
  Dimension(Definition<Dimension<f32>, f32>),
}

impl WebTokenValue {
  /// The `@property` rule registering the token `name` with this kind's syntax.
  #[must_use]
  pub(crate) fn registration(&self, name: TokenName) -> String {
    let (syntax, initial) = match self {
      Self::Color(_) => ("<color>", "transparent"),
      Self::Number(_) => ("<number>", "0"),
      Self::Length(_) => ("<length>", "0px"),
      Self::Dimension(_) => ("<length-percentage> | auto", "0px"),
    };

    format!("@property --{name}{{syntax:\"{syntax}\";inherits:true;initial-value:{initial}}}")
  }
}

impl From<Definition<Color, f32>> for WebTokenValue {
  #[inline(always)]
  fn from(definition: Definition<Color, f32>) -> Self {
    Self::Color(definition)
  }
}

impl From<Definition<f32, f32>> for WebTokenValue {
  #[inline(always)]
  fn from(definition: Definition<f32, f32>) -> Self {
    Self::Number(definition)
  }
}

impl From<Definition<Length, f32>> for WebTokenValue {
  #[inline(always)]
  fn from(definition: Definition<Length, f32>) -> Self {
    Self::Length(definition)
  }
}

impl From<Definition<Dimension<f32>, f32>> for WebTokenValue {
  #[inline(always)]
  fn from(definition: Definition<Dimension<f32>, f32>) -> Self {
    Self::Dimension(definition)
  }
}

impl From<WebTokenValue> for TokenValue<f32> {
  /// The definition as the shared half's erased token value, of the same kind.
  fn from(value: WebTokenValue) -> Self {
    match value {
      WebTokenValue::Color(definition) => Self::Color(definition),
      WebTokenValue::Number(definition) => Self::Number(definition),
      WebTokenValue::Length(definition) => Self::Length(definition),
      WebTokenValue::Dimension(definition) => Self::Dimension(definition),
    }
  }
}

impl TryFrom<TokenValue<f32>> for WebTokenValue {
  /// A text definition, which no custom property the web registers holds.
  type Error = Definition<String, f32>;

  /// The shared half's erased token value as the web's, of the same kind; a text is
  /// refused.
  fn try_from(value: TokenValue<f32>) -> Result<Self, Self::Error> {
    match value {
      TokenValue::Color(definition) => Ok(Self::Color(definition)),
      TokenValue::Number(definition) => Ok(Self::Number(definition)),
      TokenValue::Length(definition) => Ok(Self::Length(definition)),
      TokenValue::Dimension(definition) => Ok(Self::Dimension(definition)),
      TokenValue::Text(definition) => Err(definition),
    }
  }
}

impl TokenReferences for WebTokenValue {
  fn for_each_token(&self, visit: &mut dyn FnMut(TokenName)) {
    match self {
      Self::Color(definition) => definition.for_each_token(visit),
      Self::Number(definition) => definition.for_each_token(visit),
      Self::Length(definition) => definition.for_each_token(visit),
      Self::Dimension(definition) => definition.for_each_token(visit),
    }
  }
}

/// What the root defines one of the platform's colors as.
enum SystemColor {
  /// A CSS color: a system color, or a function of them.
  Css(&'static str),
  /// The first color mixed toward the second by the percentage, in Oklab.
  Mix(TokenRef<Color>, TokenRef<Color>, u8),
}

impl SystemColor {
  /// The CSS value.
  fn render(&self) -> String {
    match self {
      Self::Css(value) => (*value).to_owned(),
      Self::Mix(from, to, percent) => {
        format!(
          "color-mix(in oklab,var(--{}),var(--{}) {percent}%)",
          from.name(),
          to.name()
        )
      }
    }
  }
}

/// What the root defines each of the platform's colors ([`SystemColors`]) as.
const SYSTEM_COLORS: [(TokenRef<Color>, SystemColor); 16] = [
  (SystemColors::INK, SystemColor::Css("CanvasText")),
  (SystemColors::MUTED_INK, SystemColor::Css("GrayText")),
  (SystemColors::WINDOW, SystemColor::Css("Canvas")),
  (SystemColors::CONTROL, SystemColor::Css("ButtonFace")),
  (
    SystemColors::CONTROL_HOVER,
    SystemColor::Mix(SystemColors::CONTROL, SystemColors::INK, 8),
  ),
  (
    SystemColors::CONTROL_PRESSED,
    SystemColor::Mix(SystemColors::CONTROL, SystemColors::INK, 16),
  ),
  (SystemColors::BORDER, SystemColor::Css("ButtonBorder")),
  (SystemColors::ACCENT, SystemColor::Css("AccentColor")),
  (
    SystemColors::ACCENT_HOVER,
    SystemColor::Mix(SystemColors::ACCENT, SystemColors::INK, 12),
  ),
  (
    SystemColors::ACCENT_PRESSED,
    SystemColor::Mix(SystemColors::ACCENT, SystemColors::INK, 24),
  ),
  (SystemColors::ON_ACCENT, SystemColor::Css("AccentColorText")),
  (
    SystemColors::TRACK,
    SystemColor::Mix(SystemColors::WINDOW, SystemColors::INK, 20),
  ),
  (
    SystemColors::KNOB,
    SystemColor::Css("light-dark(Canvas,CanvasText)"),
  ),
  (SystemColors::FIELD, SystemColor::Css("Field")),
  (SystemColors::SELECTION, SystemColor::Css("Highlight")),
  (
    SystemColors::SELECTION_INACTIVE,
    SystemColor::Css("color-mix(in oklab,GrayText 35%,transparent)"),
  ),
];

/// What an engine without `AccentColor` defines the accent and what sits on it as: the
/// blue its own controls are drawn in, per scheme.
const ACCENT_FALLBACK: [(TokenRef<Color>, SystemColor); 2] = [
  (
    SystemColors::ACCENT,
    SystemColor::Css("light-dark(#0075ff,#99c8ff)"),
  ),
  (
    SystemColors::ON_ACCENT,
    SystemColor::Css("light-dark(#ffffff,#1b1b1b)"),
  ),
];

impl WebTokenValue {
  /// The rules defining the platform's colors at the root, in the reset's layer: their
  /// registrations, each a CSS system color the root's `color-scheme` resolves, that
  /// scheme following the platform's and the app's choice through the scheme guard, so
  /// a page follows both with no script.
  ///
  /// A theme beneath redefines any of them as it does any token.
  #[must_use]
  pub(crate) fn system_colors() -> String {
    let registered = Self::Color(Definition::from(Color::TRANSPARENT));
    let declarations = |colors: &[(TokenRef<Color>, SystemColor)]| {
      colors
        .iter()
        .fold(String::new(), |mut out, (reference, color)| {
          out.push_str("--");
          out.push_str(reference.name().as_str());
          out.push(':');
          out.push_str(&color.render());
          out.push(';');

          out
        })
    };
    let mut out = SYSTEM_COLORS
      .iter()
      .map(|(reference, _)| registered.registration(reference.name()))
      .collect::<String>();

    out.push_str("@layer skene.reset{:where(:root){color-scheme:light;");
    out.push_str(&declarations(&SYSTEM_COLORS));
    out.push_str("}@supports not (color:AccentColor){:where(:root){");
    out.push_str(&declarations(&ACCENT_FALLBACK));
    out.push_str("}}");

    for conjunction in Conjunction::of_media(&Media::Dark) {
      conjunction.emit(&mut out, Some(":root"), "", "color-scheme:dark;");
    }

    out.push('}');

    out
  }

  /// Claims the root's [`SystemColors`] for [`system_colors`](Self::system_colors)'s
  /// rules, so a read of the set below no theme lays no definition over the platform's.
  pub(crate) fn claim_system_colors() {
    Scene::<Web>::with(|scene| {
      scene.define_root_set(TypeId::of::<SystemColors>(), Definitions::from(Vec::new()))
    });
  }

  /// The names of the platform's colors, which [`system_colors`](Self::system_colors)
  /// registers.
  pub(crate) fn system_color_names() -> impl Iterator<Item = TokenName> {
    SYSTEM_COLORS.iter().map(|(reference, _)| reference.name())
  }
}
