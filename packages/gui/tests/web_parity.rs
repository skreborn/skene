//! The parity harness: one declaration realized on the browser leaf and on the headless
//! platform, the value core's `Resolver` gives it held against the value the engine
//! computes, for every kind of leaf a property takes.
//!
//! The browser is the oracle. A case the resolution gets wrong is a divergence every
//! platform resolving through core shares, the terminal among them.
#![cfg(skene_leaf = "browser")]

use skene_common::{Container, Text, Theme, gui::Typography};
use skene_core::{
  AnyView,
  Border,
  Color,
  Condition,
  Definition,
  Dimension,
  Expr,
  HasChildren,
  Inset,
  Length,
  Media,
  NoProperty,
  NodeId,
  Platform,
  Property,
  PropertyKey,
  Size,
  StaticLeaf,
  Token,
  TokenName,
  TokenRef,
  TokenSet,
  Value,
  View,
};
use skene_gui::web::{self, mount};
use skene_headless::{
  Headless,
  box_props,
  children,
  resolve_border,
  resolve_color,
  resolve_length,
};
use skene_reactive::Owner;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Document, Element};

wasm_bindgen_test_configure!(run_in_browser);

type Web = skene_gui::ActivePlatform;

const COLOR: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Color);
const TEXT_COLOR: PropertyKey<NoProperty> = PropertyKey::Shared(Property::TextColor);
const WIDTH: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Width);
const PADDING: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Padding);

/// The tokens the cases read, one per kind and one defined under a condition.
#[derive(Debug, Clone, PartialEq)]
struct ParityPalette {
  /// A surface defined outright.
  surface: Token<Color, f32>,
  /// A surface defined under a condition that holds at any viewport.
  shifted: Token<Color, f32>,
  /// A width.
  wide: Token<Dimension<f32>, f32>,
  /// A length every length-measured property reads.
  gap: Token<Length, f32>,
  /// A surface defined through `surface`, where the palette defines it.
  shade: Token<Color, f32>,
  /// A length defined in `em`, where the palette defines it.
  indent: Token<Length, f32>,
}

impl ParityPalette {
  const SURFACE: TokenRef<Color> = TokenRef::new("parity-surface");
  const SHIFTED: TokenRef<Color> = TokenRef::new("parity-shifted");
  const WIDE: TokenRef<Dimension<f32>> = TokenRef::new("parity-wide");
  const GAP: TokenRef<Length> = TokenRef::new("parity-gap");
  const SHADE: TokenRef<Color> = TokenRef::new("parity-shade");
  const INDENT: TokenRef<Length> = TokenRef::new("parity-indent");
}

impl Default for ParityPalette {
  fn default() -> Self {
    Self {
      surface: Token::new(
        Self::SURFACE.name().as_str(),
        Definition::from(Color::srgb(0.2, 0.4, 0.6)),
      ),
      shifted: Token::new(
        Self::SHIFTED.name().as_str(),
        Definition::from(Color::srgb(0.9, 0.1, 0.1))
          .when(Media::MinWidth(1.), Color::srgb(0.1, 0.7, 0.3)),
      ),
      wide: Token::new(
        Self::WIDE.name().as_str(),
        Definition::from(Dimension::Length(90.)),
      ),
      gap: Token::new(Self::GAP.name().as_str(), Length(14.)),
      shade: Token::new(
        Self::SHADE.name().as_str(),
        Expr::from(Self::SURFACE).lighter(0.2),
      ),
      indent: Token::new(Self::INDENT.name().as_str(), Expr::<Length>::em(2.)),
    }
  }
}

impl<V> TokenSet<V> for ParityPalette
where
  V: From<Definition<Color, f32>>
    + From<Definition<Dimension<f32>, f32>>
    + From<Definition<Length, f32>>
    + 'static,
{
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![
      self.surface.define(),
      self.shifted.define(),
      self.wide.define(),
      self.gap.define(),
      self.shade.define(),
      self.indent.define(),
    ]
  }
}

/// A nested scope's tokens: the palette's surface alone, redefined.
#[derive(Debug, Clone, PartialEq)]
struct InnerPalette {
  /// The surface.
  surface: Token<Color, f32>,
}

impl<V: From<Definition<Color, f32>> + 'static> TokenSet<V> for InnerPalette {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.surface.define()]
  }
}

/// A fixed color the cases start from.
fn blue() -> Color {
  Color::srgb(0.2, 0.4, 0.6)
}

/// A second fixed color, which a condition turns to.
fn orange() -> Color {
  Color::srgb(0.95, 0.55, 0.1)
}

/// What a case reads: a property of the box, or of the run inside it.
#[derive(Debug, Clone, Copy)]
enum Read {
  /// The box's background.
  Background,
  /// The box's width.
  Width,
  /// The box's padding, the same on each edge.
  Padding,
  /// The box's border: its width and its color.
  Border,
  /// The ink of the run inside the box.
  Ink,
}

/// One declaration, realized on either platform.
#[derive(Debug, Clone, Copy)]
enum Case {
  FixedColor,
  TokenColor,
  ConditionalTokenColor,
  MixedColor,
  TranslucentMix,
  LighterColor,
  LighterThanWhite,
  NegativeChroma,
  GrayWithChroma,
  TranslucentColor,
  DerivedColor,
  DerivedTokenColor,
  HeldCondition,
  UnheldCondition,
  DarkUnchosen,
  FixedWidth,
  ViewportWidth,
  ClampedWidth,
  CrossedClampWidth,
  EmWidth,
  TokenWidth,
  LengthTokenWidth,
  LengthTokenExpressionWidth,
  DerivedWidth,
  ConditionalWidth,
  FixedPadding,
  LengthTokenPadding,
  ConditionalPadding,
  FixedBorder,
  FixedInk,
  TokenInk,
  LexicalToken,
  LexicalEm,
}

/// Every case the harness holds the resolution to.
const CASES: &[Case] = &[
  Case::FixedColor,
  Case::TokenColor,
  Case::ConditionalTokenColor,
  Case::MixedColor,
  Case::TranslucentMix,
  Case::LighterColor,
  Case::LighterThanWhite,
  Case::NegativeChroma,
  Case::GrayWithChroma,
  Case::TranslucentColor,
  Case::DerivedColor,
  Case::DerivedTokenColor,
  Case::HeldCondition,
  Case::UnheldCondition,
  Case::DarkUnchosen,
  Case::FixedWidth,
  Case::ViewportWidth,
  Case::ClampedWidth,
  Case::CrossedClampWidth,
  Case::EmWidth,
  Case::TokenWidth,
  Case::LengthTokenWidth,
  Case::LengthTokenExpressionWidth,
  Case::DerivedWidth,
  Case::ConditionalWidth,
  Case::FixedPadding,
  Case::LengthTokenPadding,
  Case::ConditionalPadding,
  Case::FixedBorder,
  Case::FixedInk,
  Case::TokenInk,
  Case::LexicalToken,
  Case::LexicalEm,
];

impl Case {
  const fn read(self) -> Read {
    match self {
      Self::FixedColor
      | Self::TokenColor
      | Self::ConditionalTokenColor
      | Self::MixedColor
      | Self::TranslucentMix
      | Self::LighterColor
      | Self::LighterThanWhite
      | Self::NegativeChroma
      | Self::GrayWithChroma
      | Self::TranslucentColor
      | Self::DerivedColor
      | Self::DerivedTokenColor
      | Self::HeldCondition
      | Self::UnheldCondition
      | Self::DarkUnchosen
      | Self::LexicalToken => Read::Background,
      Self::FixedWidth
      | Self::ViewportWidth
      | Self::ClampedWidth
      | Self::CrossedClampWidth
      | Self::EmWidth
      | Self::TokenWidth
      | Self::LengthTokenWidth
      | Self::LengthTokenExpressionWidth
      | Self::DerivedWidth
      | Self::ConditionalWidth
      | Self::LexicalEm => Read::Width,
      Self::FixedPadding | Self::LengthTokenPadding | Self::ConditionalPadding => Read::Padding,
      Self::FixedBorder => Read::Border,
      Self::FixedInk | Self::TokenInk => Read::Ink,
    }
  }

  /// The case's box, inside the scopes the case adds, on the platform `P`.
  fn view<P: Platform<Scalar = f32>>(self) -> AnyView<P> {
    match self {
      Self::LexicalToken => Theme::<P, InnerPalette>::new(InnerPalette {
        surface: Token::new(ParityPalette::SURFACE.name().as_str(), orange()),
      })
      .set_child_builder(|| Container::<P>::new().set_color(ParityPalette::SHADE))
      .into_any(),
      Self::LexicalEm => Theme::<P, Typography>::new(Typography::default().set_size(30.))
        .set_child_builder(|| Container::<P>::new().set_width(ParityPalette::INDENT))
        .into_any(),
      _ => self.container::<P>().into_any(),
    }
  }

  /// The case's box, where the case adds no scope, on the platform `P`.
  fn container<P: Platform<Scalar = f32>>(self) -> Container<P> {
    let container = Container::<P>::new();

    match self {
      Self::FixedColor => container.set_color(blue()),
      Self::TokenColor => container.set_color(ParityPalette::SURFACE),
      Self::ConditionalTokenColor => container.set_color(ParityPalette::SHIFTED),
      Self::MixedColor => {
        container.set_color(Expr::from(ParityPalette::SURFACE).mix(Color::srgb(1., 1., 1.), 0.25))
      }
      Self::TranslucentMix => {
        container.set_color(Expr::from(blue().with_alpha(0.2)).mix(orange(), 0.5))
      }
      Self::LighterColor => container.set_color(Expr::from(blue()).lighter(0.1)),
      Self::LighterThanWhite => {
        container.set_color(Expr::from(Color::srgb(0.9, 0.9, 0.9)).lighter(0.5))
      }
      Self::NegativeChroma => container.set_color(Expr::from(blue()).with_chroma(-0.1)),
      Self::GrayWithChroma => {
        container.set_color(Expr::from(Color::srgb(0.5, 0.5, 0.5)).with_chroma(0.1))
      }
      Self::TranslucentColor => container.set_color(Expr::from(blue()).with_alpha(0.5)),
      Self::DerivedColor => container.derive_color(blue),
      Self::DerivedTokenColor => container.derive_color(|| ParityPalette::SURFACE),
      Self::HeldCondition => {
        container.set_color(Value::from(blue()).when(Media::MinWidth(1.), orange()))
      }
      Self::UnheldCondition => {
        container.set_color(Value::from(blue()).when(Media::MaxWidth(1.), orange()))
      }
      Self::DarkUnchosen => {
        container.set_color(Value::from(blue()).when(Condition::from(Media::Dark), orange()))
      }
      Self::FixedWidth => container.set_width(Dimension::Length(120.)),
      Self::ViewportWidth => container.set_width(Expr::vw(10.)),
      Self::ClampedWidth => container.set_width(Expr::<Length>::from(100.).clamp(20., 80.)),
      Self::CrossedClampWidth => container.set_width(Expr::<Length>::from(50.).clamp(80., 20.)),
      Self::EmWidth => container.set_width(Expr::em(3.)),
      Self::TokenWidth => container.set_width(ParityPalette::WIDE),
      Self::LengthTokenWidth => container.set_width(ParityPalette::GAP),
      Self::LengthTokenExpressionWidth => container.set_width(Expr::from(ParityPalette::GAP) * 3.),
      Self::DerivedWidth => container.derive_width(|| Dimension::Length(70.)),
      Self::ConditionalWidth => container.set_width(
        Value::from(Dimension::Length(40.)).when(Media::MinWidth(1.), Dimension::Length(60.)),
      ),
      Self::FixedPadding => container.set_padding(Inset::all(12.)),
      Self::LengthTokenPadding => container.set_padding(ParityPalette::GAP),
      Self::ConditionalPadding => {
        container.set_padding(Value::from(Inset::all(4.)).when(Media::MinWidth(1.), Inset::all(9.)))
      }
      Self::FixedBorder => container.set_border(Border::new(3., orange())),
      Self::FixedInk => container.set_child(Text::new("ink").set_color(orange())),
      Self::TokenInk => container.set_child(Text::new("ink").set_color(ParityPalette::SURFACE)),
      Self::LexicalToken | Self::LexicalEm => container,
    }
  }

  /// The case's box under a theme of the palette, inside a typography of a size other
  /// than the default, built on the platform `P`.
  fn build<P: Platform<Scalar = f32>>(self) -> NodeId {
    Theme::<P, Typography>::new(Typography::default().set_size(20.))
      .set_child_builder(move || {
        Theme::<P, ParityPalette>::new(ParityPalette::default())
          .set_child_builder(move || self.view::<P>())
      })
      .build()
  }
}

/// A value a case reads, as each platform reports it.
#[derive(Debug, Clone, PartialEq)]
enum Reading {
  /// A color.
  Color(Color),
  /// A length, in pixels.
  Length(f32),
  /// A border's width, in pixels, and its color.
  Border(f32, Color),
  /// Nothing: the platform gave no value, or one the harness could not read.
  Missing,
}

fn document() -> Document {
  web_sys::window()
    .expect("a window")
    .document()
    .expect("a document")
}

/// A computed style `property` of `element`.
fn computed(element: &Element, property: &str) -> String {
  web_sys::window()
    .expect("a window")
    .get_computed_style(element)
    .expect("computed style")
    .expect("a computed style")
    .get_property_value(property)
    .expect("a property value")
}

/// The color the engine serialized as `text`: `rgb()`, `rgba()`, `oklab()`, `oklch()` or
/// `color(srgb …)`.
fn parse_color(text: &str) -> Option<Color> {
  let (function, rest) = text.split_once('(')?;
  let body = rest.strip_suffix(')')?;
  let (channels, alpha) = match body.split_once('/') {
    Some((channels, alpha)) => (channels, Some(alpha.trim().parse::<f32>().ok()?)),
    None => (body, None),
  };
  let channels = channels.trim().strip_prefix("srgb").unwrap_or(channels);
  let numbers: Vec<f32> = channels
    .split([',', ' '])
    .filter(|number| !number.is_empty())
    .map(|number| number.parse().ok())
    .collect::<Option<_>>()?;
  let color = match (function, numbers.as_slice()) {
    ("rgb" | "rgba", [red, green, blue, rest @ ..]) => {
      let alpha = alpha.or_else(|| rest.first().copied());

      Color::srgb(red / 255., green / 255., blue / 255.).with_alpha(alpha.unwrap_or(1.))
    }
    ("color", [red, green, blue]) => {
      Color::srgb(*red, *green, *blue).with_alpha(alpha.unwrap_or(1.))
    }
    ("oklab", [lightness, a_axis, b_axis]) => {
      Color::oklab(*lightness, *a_axis, *b_axis).with_alpha(alpha.unwrap_or(1.))
    }
    ("oklch", [lightness, chroma, hue]) => {
      Color::oklch(*lightness, *chroma, *hue).with_alpha(alpha.unwrap_or(1.))
    }
    _ => return None,
  };

  Some(color)
}

/// A computed color, parsed.
fn computed_color(element: &Element, property: &str) -> Reading {
  parse_color(&computed(element, property)).map_or(Reading::Missing, Reading::Color)
}

/// A computed length in pixels.
fn pixels(value: &str) -> f32 {
  value
    .strip_suffix("px")
    .and_then(|number| number.parse().ok())
    .unwrap_or(f32::NAN)
}

/// What the headless platform's resolution gives `case`, its viewport the browser's.
fn resolved(case: Case, viewport: Size<f32>) -> Reading {
  skene_headless::reset();
  skene_headless::trigger_resize(viewport);

  let owner = Owner::new();
  let mut node = owner.enter(|| case.build::<Headless>());

  while box_props(node).is_none() {
    node = children(node)[0];
  }

  let reading = match case.read() {
    Read::Background => resolve_color(node, COLOR).map(Reading::Color),
    Read::Width => resolve_length(node, WIDTH).map(Reading::Length),
    Read::Padding => resolve_length(node, PADDING).map(Reading::Length),
    Read::Border => resolve_border(node).and_then(|border| match border.color {
      StaticLeaf::Fixed(color) => Some(Reading::Border(border.width, color)),
      _ => None,
    }),
    Read::Ink => resolve_color(children(node)[1], TEXT_COLOR).map(Reading::Color),
  };

  owner.dispose();

  reading.unwrap_or(Reading::Missing)
}

/// What the engine computes for `case`.
fn rendered(case: Case) -> Reading {
  let root = document().create_element("div").expect("a root");

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect the root");
  web::install(root.clone());

  let _ = mount(move || case.build::<Web>());
  let element = root
    .query_selector(":not([data-skene-scope])")
    .ok()
    .flatten()
    .expect("the case's box");
  let reading = match case.read() {
    Read::Background => computed_color(&element, "background-color"),
    Read::Width => Reading::Length(pixels(&computed(&element, "width"))),
    Read::Padding => Reading::Length(pixels(&computed(&element, "padding-top"))),
    Read::Border => match computed_color(&element, "border-top-color") {
      Reading::Color(color) => {
        Reading::Border(pixels(&computed(&element, "border-top-width")), color)
      }
      _ => Reading::Missing,
    },
    Read::Ink => {
      let run = element
        .first_element_child()
        .unwrap_or_else(|| element.clone());

      computed_color(&run, "color")
    }
  };

  web::uninstall();
  root.remove();

  reading
}

/// Whether two colors agree in Oklab, within what an 8-bit serialization rounds away.
fn same_color(left: Color, right: Color) -> bool {
  left
    .to_oklab()
    .iter()
    .zip(right.to_oklab())
    .all(|(left, right)| (left - right).abs() < 0.005)
}

/// Whether two readings agree: colors within an 8-bit rounding, lengths within a
/// hundredth of a pixel.
fn agree(resolved: &Reading, rendered: &Reading) -> bool {
  match (resolved, rendered) {
    (Reading::Color(left), Reading::Color(right)) => same_color(*left, *right),
    (Reading::Length(left), Reading::Length(right)) => (left - right).abs() < 0.01,
    (Reading::Border(left_width, left), Reading::Border(right_width, right)) => {
      (left_width - right_width).abs() < 0.01 && same_color(*left, *right)
    }
    _ => false,
  }
}

/// The browser's viewport, in CSS pixels.
fn viewport() -> Size<f32> {
  let window = web_sys::window().expect("a window");
  #[expect(
    clippy::cast_possible_truncation,
    reason = "a viewport length in CSS pixels is precise enough in f32"
  )]
  let length = |value: Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>| {
    value.ok().and_then(|value| value.as_f64()).unwrap_or(0.) as f32
  };

  Size::new(length(window.inner_width()), length(window.inner_height()))
}

/// Every case resolves to the value the engine computes.
#[wasm_bindgen_test]
fn the_resolution_agrees_with_the_engine() {
  let viewport = viewport();
  let divergences: Vec<String> = CASES
    .iter()
    .filter_map(|case| {
      let resolved = resolved(*case, viewport);
      let rendered = rendered(*case);

      (!agree(&resolved, &rendered))
        .then(|| format!("{case:?}: resolved {resolved:?}, rendered {rendered:?}"))
    })
    .collect();

  assert!(divergences.is_empty(), "{}", divergences.join("\n"));
}
