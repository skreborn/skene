//! The parity harness: one declaration realized on the browser leaf and on the headless
//! platform, the value core's `Resolver` gives it held against the value the engine
//! computes, for every kind of leaf a property takes.
//!
//! The browser is the oracle. A case the resolution gets wrong is a divergence every
//! platform resolving through core shares, the terminal among them. A case in motion
//! steps both platforms' clocks alike: the headless one by its frame, the engine's by
//! seeking its animations.
#![cfg(skene_web_platform = "browser")]

use std::{cell::Cell, time::Duration};

use skene_foundation::{
  AccessProp,
  Activate,
  Animation,
  AnyView,
  AspectRatio,
  Axes,
  BasicButton,
  BasicCheckbox,
  BoxKeyframe,
  Color,
  Condition,
  Container,
  ContainerBase,
  ContainerName,
  ContainerQuery,
  DecorationBase,
  Definition,
  Dimension,
  Easing,
  Edge,
  Expr,
  HasChild,
  Inset,
  Length,
  Media,
  MotionBase,
  NodeId,
  Overflow,
  Platform,
  Property,
  PropertyKey,
  Size,
  State,
  StaticLeaf,
  Text,
  Theme,
  Toggle,
  Token,
  TokenName,
  TokenRef,
  TokenSet,
  Transition,
  Value,
  View,
  Visibility,
  web::{Typography, WebProperty},
};
use skene_headless::{
  Headless,
  box_props,
  children,
  control_props,
  resolve_aspect_ratio,
  resolve_color,
  resolve_inset,
  resolve_length,
  resolve_overflow,
  resolve_visibility,
  trigger_container_size,
  trigger_focus,
  trigger_frame,
};
use skene_reactive::{Get, Owner, Set, Signal};
use skene_web::{
  platform::{self, mount},
  testing::{computed, document},
};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::Element;

wasm_bindgen_test_configure!(run_in_browser);

type Web = skene_web::ActivePlatform;

const COLOR: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Color);
const TEXT_COLOR: PropertyKey<WebProperty> = PropertyKey::Shared(Property::TextColor);
const WIDTH: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Width);
const PADDING: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Padding);
const BORDER_WIDTH: PropertyKey<WebProperty> = PropertyKey::Shared(Property::BorderWidth);
const BORDER_COLOR: PropertyKey<WebProperty> = PropertyKey::Shared(Property::BorderColor);

/// The accessible name a case gives the box it reads, where that box is not its own.
const TARGET: &str = "parity-target";

/// The query container a case's box sits in, where it asks for one.
const BOX: ContainerName = ContainerName("parity-box");

/// The width the query container holds.
const BOX_WIDTH: f32 = 400.;

thread_local! {
  /// The number a moving case derives its property from, written as the case steps.
  static MOVING: Cell<Option<Signal<f32>>> = const { Cell::new(None) };
}

/// The box a case reads, where it is not the case's own: named [`TARGET`].
fn target() -> [AccessProp; 1] {
  [AccessProp::Name(TARGET.to_owned())]
}

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
  /// The box's border at an edge: its width there, and its color.
  Border(Edge),
  /// The ink of the run inside the box.
  Ink,
  /// Whether the box is drawn.
  Visibility,
  /// The box's aspect ratio.
  AspectRatio,
  /// The box's overflow per axis.
  Overflow,
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
  SafeAreaPadding,
  FixedBorder,
  EdgeBorder,
  FixedInk,
  TokenInk,
  LexicalToken,
  LexicalEm,
  ContainerUnitWidth,
  ContainerQueryColor,
  CheckedColor,
  DisabledColor,
  FocusedColor,
  NeutralKeyframe,
  ReversedTransition,
  VisibilityTransition,
  RatioTransition,
  AxisOverflow,
  VisibleBesideScroll,
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
  Case::SafeAreaPadding,
  Case::FixedBorder,
  Case::EdgeBorder,
  Case::FixedInk,
  Case::TokenInk,
  Case::LexicalToken,
  Case::LexicalEm,
  Case::ContainerUnitWidth,
  Case::ContainerQueryColor,
  Case::CheckedColor,
  Case::DisabledColor,
  Case::FocusedColor,
  Case::NeutralKeyframe,
  Case::ReversedTransition,
  Case::VisibilityTransition,
  Case::RatioTransition,
  Case::AxisOverflow,
  Case::VisibleBesideScroll,
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
      | Self::LexicalToken
      | Self::ContainerQueryColor
      | Self::CheckedColor
      | Self::DisabledColor
      | Self::FocusedColor => Read::Background,
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
      | Self::LexicalEm
      | Self::ContainerUnitWidth
      | Self::NeutralKeyframe
      | Self::ReversedTransition => Read::Width,
      Self::FixedPadding
      | Self::LengthTokenPadding
      | Self::ConditionalPadding
      | Self::SafeAreaPadding => Read::Padding,
      Self::FixedBorder => Read::Border(Edge::Top),
      Self::EdgeBorder => Read::Border(Edge::Left),
      Self::FixedInk | Self::TokenInk => Read::Ink,
      Self::VisibilityTransition => Read::Visibility,
      Self::RatioTransition => Read::AspectRatio,
      Self::AxisOverflow | Self::VisibleBesideScroll => Read::Overflow,
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
      Self::ContainerUnitWidth => Container::<P>::new()
        .set_width(Dimension::Length(BOX_WIDTH))
        .set_container_name(BOX)
        .set_child(
          Container::<P>::new()
            .set_width(Expr::cqw(25.))
            .set_accessibility(target()),
        )
        .into_any(),
      Self::ContainerQueryColor => Container::<P>::new()
        .set_width(Dimension::Length(BOX_WIDTH))
        .set_container_name(BOX)
        .set_child(
          Container::<P>::new()
            .set_color(Value::from(blue()).when(
              Media::Container {
                name: Some(BOX),
                query: ContainerQuery::MinWidth(300.),
              },
              orange(),
            ))
            .set_accessibility(target()),
        )
        .into_any(),
      Self::CheckedColor => BasicCheckbox::<P>::new(Toggle::new().set_checked(true))
        .set_child(Self::state_box::<P>(State::Checked))
        .into_any(),
      Self::DisabledColor => BasicButton::<P>::new(Activate::new())
        .set_disabled(true)
        .set_child(Self::state_box::<P>(State::Disabled))
        .into_any(),
      Self::FocusedColor => BasicButton::<P>::new(Activate::new())
        .set_accessible_name(TARGET)
        .set_color(Value::from(blue()).when(State::Focused, orange()))
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
      Self::SafeAreaPadding => container.set_padding(Expr::<Length>::safe_area(Edge::Top) + 6.),
      Self::FixedBorder => container
        .set_border_width(Inset::all(3.))
        .set_border_color(orange()),
      Self::EdgeBorder => container
        .set_border_width(Inset {
          top: 1.,
          right: 2.,
          bottom: 3.,
          left: 4.,
        })
        .set_border_color(ParityPalette::SURFACE),
      Self::FixedInk => container.set_child(Text::new("ink").set_color(orange())),
      Self::AxisOverflow => container.set_overflow(Axes::new(Overflow::Hidden, Overflow::Scroll)),
      Self::VisibleBesideScroll => {
        container.set_overflow(Axes::new(Overflow::Visible, Overflow::Scroll))
      }
      Self::NeutralKeyframe
      | Self::ReversedTransition
      | Self::VisibilityTransition
      | Self::RatioTransition => self.moving(container),
      Self::TokenInk => container.set_child(Text::new("ink").set_color(ParityPalette::SURFACE)),
      Self::LexicalToken
      | Self::LexicalEm
      | Self::ContainerUnitWidth
      | Self::ContainerQueryColor
      | Self::CheckedColor
      | Self::DisabledColor
      | Self::FocusedColor => container,
    }
  }

  /// The case's box in motion, where the case moves; the box as it is otherwise.
  fn moving<P: Platform<Scalar = f32>>(self, container: Container<P>) -> Container<P> {
    match self {
      Self::NeutralKeyframe => container.set_width(Dimension::Length(40.)).set_animation(
        Animation::new(
          1.,
          [(
            50.,
            BoxKeyframe::<P> {
              width: Some(StaticLeaf::Fixed(Dimension::Length(80.))),
              ..BoxKeyframe::<P>::default()
            },
          )],
        )
        .set_easing(Easing::Linear),
      ),
      Self::ReversedTransition => {
        let moving = Signal::new(0f32);

        MOVING.set(Some(moving));

        container
          .derive_width(move || Dimension::Length(moving.get()))
          .set_transition(Transition::new(Property::Width, 1.).set_easing(Easing::Linear))
      }
      Self::VisibilityTransition => {
        let moving = Signal::new(0f32);

        MOVING.set(Some(moving));

        container
          .derive_visibility(move || {
            if moving.get() > 0. {
              Visibility::Hidden
            }
            else {
              Visibility::Visible
            }
          })
          .set_transition(Transition::new(Property::Visibility, 1.).set_easing(Easing::Linear))
      }
      Self::RatioTransition => {
        let moving = Signal::new(0f32);

        MOVING.set(Some(moving));

        container
          .set_width(Dimension::Length(40.))
          .derive_aspect_ratio(move || AspectRatio(1. + 3. * moving.get()))
          .set_transition(Transition::new(Property::AspectRatio, 1.).set_easing(Easing::Linear))
      }
      _ => container,
    }
  }

  /// A box inside a control, blue until `state` holds and orange while it does.
  fn state_box<P: Platform<Scalar = f32>>(state: State) -> Container<P> {
    Container::<P>::new()
      .set_width(Dimension::Length(10.))
      .set_height(Dimension::Length(10.))
      .set_color(Value::from(blue()).when(state, orange()))
      .set_accessibility(target())
  }

  /// Whether the case focuses its control before it is read.
  const fn focuses(self) -> bool {
    matches!(self, Self::FocusedColor)
  }

  /// The steps the case takes before it is read, each a number written to the moving
  /// case's signal, where it writes one, and the milliseconds the clock then advances.
  const fn steps(self) -> &'static [(Option<f32>, u64)] {
    match self {
      Self::NeutralKeyframe => &[(None, 250)],
      Self::ReversedTransition => &[(Some(100.), 250), (Some(0.), 125)],
      Self::VisibilityTransition => &[(Some(1.), 250)],
      Self::RatioTransition => &[(Some(1.), 500)],
      _ => &[],
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
  /// A visibility.
  Visibility(Visibility),
  /// A width-to-height ratio.
  Ratio(f32),
  /// An overflow per axis.
  Overflow(Axes<Overflow>),
  /// Nothing: the platform gave no value, or one the harness could not read.
  Missing,
}

/// Pauses every animation and transition running on `element`, `milliseconds` into
/// each.
fn seek(element: &Element, milliseconds: u64) {
  let method = |target: &wasm_bindgen::JsValue, name: &str| {
    js_sys::Reflect::get(target, &name.into())
      .expect("the method")
      .dyn_into::<js_sys::Function>()
      .expect("a function")
  };
  let animations = method(element, "getAnimations")
    .call0(element)
    .expect("the animations")
    .dyn_into::<js_sys::Array>()
    .expect("an array");

  for animation in animations.iter() {
    method(&animation, "pause")
      .call0(&animation)
      .expect("a pause");
    #[expect(
      clippy::cast_precision_loss,
      reason = "a step lasts far fewer milliseconds than f64 holds exactly"
    )]
    let time = milliseconds as f64;

    js_sys::Reflect::set(&animation, &"currentTime".into(), &time.into()).expect("a seek");
  }
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

/// A computed aspect ratio: `auto`, a ratio, or `auto` with a ratio.
fn ratio(value: &str) -> Reading {
  let value = value.trim_start_matches("auto").trim();
  let parsed = match value.split_once('/') {
    Some((width, height)) => width
      .trim()
      .parse::<f32>()
      .ok()
      .zip(height.trim().parse::<f32>().ok())
      .map(|(width, height)| width / height),
    None => value.parse().ok(),
  };

  parsed.map_or(Reading::Missing, Reading::Ratio)
}

/// A computed overflow keyword.
fn overflow_keyword(value: &str) -> Option<Overflow> {
  match value {
    "visible" => Some(Overflow::Visible),
    "hidden" => Some(Overflow::Hidden),
    "scroll" => Some(Overflow::Scroll),
    "auto" => Some(Overflow::Auto),
    _ => None,
  }
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
  trigger_container_size(BOX, Size::new(BOX_WIDTH, 0.));

  let owner = Owner::new();
  let built = owner.enter(|| case.build::<Headless>());
  let mut node = built;

  while box_props(node).is_none() {
    node = children(node)[0];
  }

  if case.focuses()
    && let Some(control) = headless_control(built)
  {
    trigger_focus(control);
  }

  let node = headless_target(built).unwrap_or(node);
  let read = || match case.read() {
    Read::Background => resolve_color(node, COLOR).map(Reading::Color),
    Read::Width => resolve_length(node, WIDTH).map(Reading::Length),
    Read::Padding => resolve_length(node, PADDING).map(Reading::Length),
    Read::Border(edge) => resolve_inset(node, BORDER_WIDTH)
      .zip(resolve_color(node, BORDER_COLOR))
      .map(|(width, color)| Reading::Border(width.edge(edge), color)),
    Read::Ink => resolve_color(children(node)[1], TEXT_COLOR).map(Reading::Color),
    Read::Visibility => resolve_visibility(node).map(Reading::Visibility),
    Read::AspectRatio => resolve_aspect_ratio(node).map(|ratio| Reading::Ratio(ratio.0)),
    Read::Overflow => resolve_overflow(node).map(Reading::Overflow),
  };
  let _ = read();

  for (width, milliseconds) in case.steps() {
    if let Some(width) = width {
      MOVING.get().expect("the moving case's width").set(*width);
    }

    trigger_frame(Duration::from_millis(*milliseconds));
  }

  let reading = read();

  owner.dispose();

  reading.unwrap_or(Reading::Missing)
}

/// The node under `node` a case names [`TARGET`], if it names one.
fn headless_target(node: NodeId) -> Option<NodeId> {
  let named = box_props(node)
    .is_some_and(|props| props.access.contains(&AccessProp::Name(TARGET.to_owned())))
    || control_props(node).is_some_and(|props| props.label.as_deref() == Some(TARGET));

  if named {
    return Some(node);
  }

  children(node).into_iter().find_map(headless_target)
}

/// The first control at or under `node`.
fn headless_control(node: NodeId) -> Option<NodeId> {
  if control_props(node).is_some() {
    return Some(node);
  }

  children(node).into_iter().find_map(headless_control)
}

/// What the engine computes for `case`.
fn rendered(case: Case) -> Reading {
  let root = document().create_element("div").expect("a root");

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect the root");
  platform::install(root.clone());

  let _ = mount(move || case.build::<Web>());

  if case.focuses()
    && let Some(control) = root
      .query_selector("[data-skene-control]")
      .ok()
      .flatten()
      .and_then(|control| control.dyn_into::<web_sys::HtmlElement>().ok())
  {
    let _ = control.focus();
  }

  let element = root
    .query_selector(&format!("[aria-label='{TARGET}']"))
    .ok()
    .flatten()
    .or_else(|| {
      root
        .query_selector(":not([data-skene-scope])")
        .ok()
        .flatten()
    })
    .expect("the case's box");
  let _ = computed(&element, "width");

  for (width, milliseconds) in case.steps() {
    if let Some(width) = width {
      MOVING.get().expect("the moving case's width").set(*width);
      platform::end_turn();
      let _ = computed(&element, "width");
    }

    seek(&element, *milliseconds);
  }

  let reading = match case.read() {
    Read::Background => computed_color(&element, "background-color"),
    Read::Width => Reading::Length(pixels(&computed(&element, "width"))),
    Read::Padding => Reading::Length(pixels(&computed(&element, "padding-top"))),
    Read::Border(edge) => {
      let side = match edge {
        Edge::Top => "top",
        Edge::Right => "right",
        Edge::Bottom => "bottom",
        Edge::Left => "left",
      };

      match computed_color(&element, &format!("border-{side}-color")) {
        Reading::Color(color) => Reading::Border(
          pixels(&computed(&element, &format!("border-{side}-width"))),
          color,
        ),
        _ => Reading::Missing,
      }
    }
    Read::Ink => {
      let run = element
        .first_element_child()
        .unwrap_or_else(|| element.clone());

      computed_color(&run, "color")
    }
    Read::Visibility => match computed(&element, "visibility").as_str() {
      "visible" => Reading::Visibility(Visibility::Visible),
      "hidden" => Reading::Visibility(Visibility::Hidden),
      _ => Reading::Missing,
    },
    Read::AspectRatio => ratio(&computed(&element, "aspect-ratio")),
    Read::Overflow => overflow_keyword(&computed(&element, "overflow-x"))
      .zip(overflow_keyword(&computed(&element, "overflow-y")))
      .map_or(Reading::Missing, |(horizontal, vertical)| {
        Reading::Overflow(Axes::new(horizontal, vertical))
      }),
  };

  platform::uninstall();
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
/// hundredth of a pixel, ratios within a thousandth, visibilities and overflows exactly.
fn agree(resolved: &Reading, rendered: &Reading) -> bool {
  match (resolved, rendered) {
    (Reading::Color(left), Reading::Color(right)) => same_color(*left, *right),
    (Reading::Length(left), Reading::Length(right)) => (left - right).abs() < 0.01,
    (Reading::Border(left_width, left), Reading::Border(right_width, right)) => {
      (left_width - right_width).abs() < 0.01 && same_color(*left, *right)
    }
    (Reading::Visibility(left), Reading::Visibility(right)) => left == right,
    (Reading::Ratio(left), Reading::Ratio(right)) => (left - right).abs() < 0.001,
    (Reading::Overflow(left), Reading::Overflow(right)) => left == right,
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
