//! The parity harness: one declaration realized on the browser leaf, the value the
//! shared half resolves it to held against the value the engine computes, for every kind
//! of leaf a property takes.
//!
//! The browser is the oracle. The resolved side is the shared half's
//! [`Keep::Nothing`](skene_foundation::Keep::Nothing) entry over the real scene: each
//! case's read node is a low-level view that keeps the style the shared half hands the
//! platform, and that [`CompiledStyle`] resolves through [`CompiledStyle::resolve`] with
//! the resolver of its [`ResolutionContext`], reading tokens, scopes, containers and the
//! enclosing control's state from the web's own scene. A case the resolution gets wrong
//! is a divergence every platform resolving through the shared half shares, the
//! terminal among them. A case in motion steps both sides alike: the resolved one
//! through the foundation's [`Motion`], retargeted as each step writes and sampled at
//! its elapsed time, the engine's by seeking its animations.
#![cfg(all(skene_platform = "web", not(skene_server)))]

use std::cell::{Cell, RefCell};

use skene_foundation::{
  AccessProp,
  Activate,
  Animation,
  AnyKind,
  AnyView,
  AspectRatio,
  Axes,
  BasicButton,
  BasicCheckbox,
  BoxKeyframe,
  BoxSlot,
  BoxValue,
  Children,
  Color,
  CompiledStyle,
  Condition,
  Container,
  ContainerBase,
  ContainerName,
  ContainerQuery,
  DeclaredBoxStyle,
  DecorationBase,
  Definition,
  Dimension,
  Easing,
  Edge,
  Environment,
  Expr,
  Frame,
  HasBoxStyle,
  HasChild,
  HasChildren,
  HasLayoutBox,
  HasRun,
  Inset,
  Layout,
  LayoutBox,
  Length,
  Media,
  MotionBase,
  NodeFacts,
  NodeHandle,
  NodeId,
  Overflow,
  PlainChild,
  Property,
  PropertyKey,
  Realization,
  Record,
  ResolutionContext,
  ResolutionSource,
  ResolvedStyle,
  RunBase,
  RunValue,
  Seal,
  Size,
  Slot,
  SlotLeaf,
  SlotValue,
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
  interpolate::Motion,
  web::{Typography, WebProperty, WebStyle},
};
use skene_gui::{
  platform::web::{self, mount},
  testing::{computed, document},
};
use skene_reactive::{Get, Set, Signal, untrack};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::Element;

wasm_bindgen_test_configure!(run_in_browser);

type Web = skene_gui::Active;

const COLOR: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Background);
const TEXT_COLOR: PropertyKey<WebProperty> = PropertyKey::Shared(Property::TextColor);
const WIDTH: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Width);
const PADDING: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Padding);
const BORDER_WIDTH: PropertyKey<WebProperty> = PropertyKey::Shared(Property::BorderWidth);
const BORDER_COLOR: PropertyKey<WebProperty> = PropertyKey::Shared(Property::BorderColor);
const HIDDEN: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Hidden);
const ASPECT_RATIO: PropertyKey<WebProperty> = PropertyKey::Shared(Property::AspectRatio);
const OVERFLOW: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Overflow);

/// The accessible name a case gives the box it reads, where that box is not its own.
const TARGET: &str = "parity-target";

/// The query container a case's box sits in, where it asks for one.
const BOX: ContainerName = ContainerName("parity-box");

/// The width the query container holds.
const BOX_WIDTH: f32 = 400.;

thread_local! {
  /// The number a moving case derives its property from, written as the case steps.
  static MOVING: Cell<Option<Signal<f32>>> = const { Cell::new(None) };
  /// The box a case reads, as the platform last realized it.
  static READ_BOX: RefCell<Option<Realized>> = const { RefCell::new(None) };
  /// The run a case reads, as the platform last realized it.
  static READ_RUN: RefCell<Option<Realized>> = const { RefCell::new(None) };
  /// The control a case reads, where the case reads a control's own box.
  static READ_CONTROL: Cell<Option<NodeHandle<Web>>> = const { Cell::new(None) };
  /// The count a resolution's tokens are kept under, moved for every case.
  static GENERATION: Cell<u64> = const { Cell::new(0) };
}

/// A read node as the platform realized it: the node, once known, the style the shared
/// half handed the platform, and the derived leaves its slots hold.
#[derive(Debug, Clone)]
struct Realized {
  /// The node; `None` for a control's box, whose node its handle names.
  node: Option<NodeId>,
  /// The style the platform was handed, kept at `Keep::Everything`, with its motion.
  style: CompiledStyle<f32, WebStyle>,
  /// The derived leaves the style's slots stand for.
  slots: Vec<BoxSlot<f32, WebStyle>>,
}

impl Realized {
  /// The box the platform was handed `style` for at `node`, its derived leaves lifted
  /// into `slots`.
  fn of_box(
    node: Option<NodeId>,
    style: &CompiledStyle<f32, WebStyle>,
    slots: Vec<BoxSlot<f32, WebStyle>>,
  ) -> Self {
    Self {
      node,
      style: style.clone(),
      slots,
    }
  }

  /// The run the platform was handed `style` for at `node`.
  fn of_run(node: NodeId, style: &CompiledStyle<f32, WebStyle>) -> Self {
    Self {
      node: Some(node),
      style: style.clone(),
      slots: Vec::new(),
    }
  }

  /// The style resolved against `source` at `node` through the `Keep::Nothing` entry.
  fn resolve(&self, source: &ParitySource, node: NodeId) -> ResolvedStyle<f32, WebStyle> {
    ResolutionContext::<Web, ParitySource>::of(source, node)
      .expect("the read node is live")
      .with_resolver(|resolver| self.style.resolve(resolver))
  }

  /// A keyframe of the style resolved against `source` at `node`.
  fn resolve_frame(
    source: &ParitySource,
    node: NodeId,
    frame: &Frame<f32, WebStyle>,
  ) -> ResolvedStyle<f32, WebStyle> {
    ResolutionContext::<Web, ParitySource>::of(source, node)
      .expect("the read node is live")
      .with_resolver(|resolver| frame.resolve(resolver))
  }
}

/// A case's read box: a low-level view over the public [`LayoutBox`], laid out as a
/// [`Container`] is, that keeps the style the platform realizes it with in
/// [`READ_BOX`].
///
/// Its derived leaves are lifted from the style it declares, so it sits inside no
/// wrapper, whose annotation would fold in leaves of its own.
#[must_use]
struct ReadBox {
  inner: LayoutBox<Web>,
}

impl ReadBox {
  /// An undecorated box.
  fn new() -> Self {
    Self {
      inner: LayoutBox::new(Layout::Single),
    }
  }
}

impl HasBoxStyle<Web> for ReadBox {
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<Web> {
    self.inner.style_mut()
  }
}

impl HasLayoutBox<Web> for ReadBox {
  type Kind = PlainChild;

  fn map_layout_box(
    self,
    map: impl FnOnce(LayoutBox<Web, PlainChild>) -> LayoutBox<Web, PlainChild>,
  ) -> Self {
    Self {
      inner: map(self.inner),
    }
  }
}

impl HasChild<Web> for ReadBox {
  type Kind = PlainChild;

  fn child_mut(&mut self, seal: Seal) -> &mut Children<Web, PlainChild> {
    self.inner.children_mut(seal)
  }
}

impl DecorationBase<Web> for ReadBox {}

impl MotionBase<Web> for ReadBox {}

impl ContainerBase<Web> for ReadBox {}

impl AnyKind for ReadBox {}

impl View<Web> for ReadBox {
  fn build(mut self) -> NodeId {
    let (_, slots) = self.inner.style_mut().clone().split();

    self.inner.realize(NodeFacts::Box, move |node, _, style| {
      READ_BOX.set(Some(Realized::of_box(Some(node), style, slots)));
    })
  }
}

/// `text` as a run that keeps the style the platform realizes it with in [`READ_RUN`].
fn read_run(text: Text<Web>) -> Text<Web> {
  text.map_run(|run| {
    run.add_realization(|node, _, style| {
      READ_RUN.set(Some(Realized::of_run(node, style)));
    })
  })
}

/// `button` as the read node, keeping the style its box declares in [`READ_BOX`] and
/// its node in [`READ_CONTROL`].
fn read_control(button: BasicButton<Web>) -> BasicButton<Web> {
  let handle = NodeHandle::new();

  READ_CONTROL.set(Some(handle));

  button.map_layout_box(|mut layout| {
    let (style, slots) = layout.style_mut().clone().split();
    let mut record = Record::new();

    record.add_box(&style);
    READ_BOX.set(Some(Realized::of_box(None, &record.into_kept(), slots)));

    layout.bind_handle(handle)
  })
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
      surface: Token::new(Self::SURFACE, Definition::from(Color::srgb(0.2, 0.4, 0.6))),
      shifted: Token::new(
        Self::SHIFTED,
        Definition::from(Color::srgb(0.9, 0.1, 0.1))
          .when(Media::min_width(1), Color::srgb(0.1, 0.7, 0.3)),
      ),
      wide: Token::new(Self::WIDE, 90),
      gap: Token::new(Self::GAP, 14),
      shade: Token::new(Self::SHADE, Expr::from(Self::SURFACE).lighter(0.2)),
      indent: Token::new(Self::INDENT, Expr::<Length>::font_size(2.)),
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
  /// Whether the box is hidden.
  Hidden,
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
  FontSizeWidth,
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
  HiddenTransition,
  RatioTransition,
  PaddingTransition,
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
  Case::FontSizeWidth,
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
  Case::HiddenTransition,
  Case::RatioTransition,
  Case::PaddingTransition,
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
      | Self::FontSizeWidth
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
      | Self::SafeAreaPadding
      | Self::PaddingTransition => Read::Padding,
      Self::FixedBorder => Read::Border(Edge::Top),
      Self::EdgeBorder => Read::Border(Edge::Start),
      Self::FixedInk | Self::TokenInk => Read::Ink,
      Self::HiddenTransition => Read::Hidden,
      Self::RatioTransition => Read::AspectRatio,
      Self::AxisOverflow | Self::VisibleBesideScroll => Read::Overflow,
    }
  }

  /// The case's box, inside the scopes the case adds.
  fn view(self) -> AnyView<Web> {
    match self {
      Self::LexicalToken => Theme::<Web, InnerPalette>::new(InnerPalette {
        surface: Token::new(ParityPalette::SURFACE, orange()),
      })
      .set_child_builder(|| ReadBox::new().set_background(ParityPalette::SHADE))
      .into_any(),
      Self::LexicalEm => Theme::<Web, Typography>::new(Typography::default().set_size(30.))
        .set_child_builder(|| ReadBox::new().set_width(ParityPalette::INDENT))
        .into_any(),
      Self::ContainerUnitWidth => Container::<Web>::new()
        .set_width(Dimension::Length(BOX_WIDTH))
        .set_container_name(BOX)
        .set_child(
          ReadBox::new()
            .set_width(Expr::container_width(0.25))
            .set_accessibility(target()),
        )
        .into_any(),
      Self::ContainerQueryColor => Container::<Web>::new()
        .set_width(Dimension::Length(BOX_WIDTH))
        .set_container_name(BOX)
        .set_child(
          ReadBox::new()
            .set_background(Value::from(blue()).when(
              Media::Container {
                name: Some(BOX),
                query: ContainerQuery::min_width(300),
              },
              orange(),
            ))
            .set_accessibility(target()),
        )
        .into_any(),
      Self::CheckedColor => BasicCheckbox::<Web>::new(Toggle::new().set_checked(true))
        .set_child(Self::state_box(State::Checked))
        .into_any(),
      Self::DisabledColor => BasicButton::<Web>::new(Activate::new())
        .set_disabled(true)
        .set_child(Self::state_box(State::Disabled))
        .into_any(),
      Self::FocusedColor => read_control(
        BasicButton::<Web>::new(Activate::new())
          .set_accessible_name(TARGET)
          .set_background(Value::from(blue()).when(State::Focused, orange())),
      )
      .into_any(),
      _ => self.container().into_any(),
    }
  }

  /// The case's box, where the case adds no scope.
  fn container(self) -> ReadBox {
    let container = ReadBox::new();

    match self {
      Self::FixedColor => container.set_background(blue()),
      Self::TokenColor => container.set_background(ParityPalette::SURFACE),
      Self::ConditionalTokenColor => container.set_background(ParityPalette::SHIFTED),
      Self::MixedColor => container
        .set_background(Expr::from(ParityPalette::SURFACE).mix(Color::srgb(1., 1., 1.), 0.25)),
      Self::TranslucentMix => {
        container.set_background(Expr::from(blue().with_alpha(0.2)).mix(orange(), 0.5))
      }
      Self::LighterColor => container.set_background(Expr::from(blue()).lighter(0.1)),
      Self::LighterThanWhite => {
        container.set_background(Expr::from(Color::srgb(0.9, 0.9, 0.9)).lighter(0.5))
      }
      Self::NegativeChroma => container.set_background(Expr::from(blue()).with_chroma(-0.1)),
      Self::GrayWithChroma => {
        container.set_background(Expr::from(Color::srgb(0.5, 0.5, 0.5)).with_chroma(0.1))
      }
      Self::TranslucentColor => container.set_background(Expr::from(blue()).with_alpha(0.5)),
      Self::DerivedColor => container.derive_background(blue),
      Self::DerivedTokenColor => container.derive_background(|| ParityPalette::SURFACE),
      Self::HeldCondition => {
        container.set_background(Value::from(blue()).when(Media::min_width(1), orange()))
      }
      Self::UnheldCondition => {
        container.set_background(Value::from(blue()).when(Media::max_width(1), orange()))
      }
      Self::DarkUnchosen => {
        container.set_background(Value::from(blue()).when(Condition::from(Media::Dark), orange()))
      }
      Self::FixedWidth => container.set_width(120),
      Self::ViewportWidth => container.set_width(Expr::viewport_width(0.1)),
      Self::ClampedWidth => container.set_width(Expr::<Length>::from(100).clamp(20, 80)),
      Self::CrossedClampWidth => container.set_width(Expr::<Length>::from(50).clamp(80, 20)),
      Self::FontSizeWidth => container.set_width(Expr::font_size(3.)),
      Self::TokenWidth => container.set_width(ParityPalette::WIDE),
      Self::LengthTokenWidth => container.set_width(ParityPalette::GAP),
      Self::LengthTokenExpressionWidth => container.set_width(Expr::from(ParityPalette::GAP) * 3.),
      Self::DerivedWidth => container.derive_width(|| 70),
      Self::ConditionalWidth => container.set_width(Value::from(40).when(Media::min_width(1), 60)),
      Self::FixedPadding => container.set_padding(Inset::all(12)),
      Self::LengthTokenPadding => container.set_padding(ParityPalette::GAP),
      Self::ConditionalPadding => {
        container.set_padding(Value::from(Inset::all(4)).when(Media::min_width(1), Inset::all(9)))
      }
      Self::SafeAreaPadding => container.set_padding(Expr::<Length>::safe_area(Edge::Top) + 6),
      Self::FixedBorder => container
        .set_border_width(Inset::all(3))
        .set_border_color(orange()),
      Self::EdgeBorder => container
        .set_border_width(Inset {
          top: 1.,
          end: 2.,
          bottom: 3.,
          start: 4.,
        })
        .set_border_color(ParityPalette::SURFACE),
      Self::FixedInk => container.set_child(read_run(Text::new("ink").set_color(orange()))),
      Self::AxisOverflow => container.set_overflow(Axes::new(Overflow::Clip, Overflow::Scroll)),
      Self::VisibleBesideScroll => {
        container.set_overflow(Axes::new(Overflow::Visible, Overflow::Scroll))
      }
      Self::NeutralKeyframe
      | Self::ReversedTransition
      | Self::HiddenTransition
      | Self::RatioTransition
      | Self::PaddingTransition => self.moving(container),
      Self::TokenInk => {
        container.set_child(read_run(Text::new("ink").set_color(ParityPalette::SURFACE)))
      }
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
  fn moving(self, container: ReadBox) -> ReadBox {
    match self {
      Self::NeutralKeyframe => container.set_width(40).add_animation(
        Animation::new(
          1.,
          [(
            50.,
            BoxKeyframe::<Web> {
              width: Some(StaticLeaf::Fixed(Dimension::Length(80.))),
              ..BoxKeyframe::<Web>::default()
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
          .add_transition(Transition::new(Property::Width, 1.).set_easing(Easing::Linear))
      }
      Self::HiddenTransition => {
        let moving = Signal::new(0f32);

        MOVING.set(Some(moving));

        container
          .derive_hidden(move || moving.get() > 0.)
          .add_transition(Transition::new(Property::Hidden, 1.).set_easing(Easing::Linear))
      }
      Self::RatioTransition => {
        let moving = Signal::new(0f32);

        MOVING.set(Some(moving));

        container
          .set_width(40)
          .derive_aspect_ratio(move || AspectRatio(1. + 3. * moving.get()))
          .add_transition(Transition::new(Property::AspectRatio, 1.).set_easing(Easing::Linear))
      }
      Self::PaddingTransition => {
        let moving = Signal::new(0f32);

        MOVING.set(Some(moving));

        container
          .derive_padding(move || Inset::all(moving.get()))
          .add_transition(Transition::new(Property::Padding, 1.).set_easing(Easing::Linear))
      }
      _ => container,
    }
  }

  /// A box inside a control, blue until `state` holds and orange while it does.
  fn state_box(state: State) -> ReadBox {
    ReadBox::new()
      .set_width(10)
      .set_height(10)
      .set_background(Value::from(blue()).when(state, orange()))
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
      Self::HiddenTransition => &[(Some(1.), 250)],
      Self::RatioTransition => &[(Some(1.), 500)],
      Self::PaddingTransition => &[(Some(20.), 250)],
      _ => &[],
    }
  }

  /// The case's box under a theme of the palette, inside a typography of a size other
  /// than the default.
  fn build(self) -> NodeId {
    Theme::<Web, Typography>::new(Typography::default().set_size(20.))
      .set_child_builder(move || {
        Theme::<Web, ParityPalette>::new(ParityPalette::default())
          .set_child_builder(move || self.view())
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
  Hidden(bool),
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
    "hidden" => Some(Overflow::Clip),
    "scroll" => Some(Overflow::Scroll),
    "auto" => Some(Overflow::ScrollIfNeeded),
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

/// Where a case's resolution reads what the scene does not hold: the browser's
/// environment, the query container's width, and the read node's derived leaves.
struct ParitySource {
  /// The environment the browser reports, at the viewport the test reads.
  environment: Environment<f32>,
  /// The read node.
  node: NodeId,
  /// The derived leaves the read node's slots stand for.
  slots: Vec<BoxSlot<f32, WebStyle>>,
  /// The count the case's tokens are kept under.
  generation: u64,
}

impl ResolutionSource<Web> for ParitySource {
  fn environment(&self) -> Environment<f32> {
    self.environment
  }

  fn container_size(&self, name: ContainerName) -> Option<Size<f32>> {
    (name == BOX).then(|| Size::new(BOX_WIDTH, 0.))
  }

  fn slot(&self, node: NodeId, slot: Slot) -> Option<SlotLeaf<f32, WebStyle>> {
    if node != self.node {
      return None;
    }

    self
      .slots
      .iter()
      .find(|derived| derived.slot == slot)
      .map(|derived| untrack(|| (derived.compute)()).into())
  }

  fn sizes_text(&self) -> bool {
    true
  }

  fn generation(&self) -> u64 {
    self.generation
  }
}

impl Read {
  /// The properties the reading takes.
  const fn keys(self) -> &'static [PropertyKey<WebProperty>] {
    match self {
      Self::Background => &[COLOR],
      Self::Width => &[WIDTH],
      Self::Padding => &[PADDING],
      Self::Border(_) => &[BORDER_WIDTH, BORDER_COLOR],
      Self::Ink => &[TEXT_COLOR],
      Self::Hidden => &[HIDDEN],
      Self::AspectRatio => &[ASPECT_RATIO],
      Self::Overflow => &[OVERFLOW],
    }
  }

  /// The reading of the values `value` gives each property.
  fn reading(
    self,
    value: impl Fn(PropertyKey<WebProperty>) -> Option<SlotValue<f32, WebStyle>>,
  ) -> Reading {
    let reading = match self {
      Self::Background => match value(COLOR) {
        Some(SlotValue::Box(BoxValue::Color(color))) => Some(Reading::Color(color)),
        _ => None,
      },
      Self::Width => match value(WIDTH) {
        Some(SlotValue::Box(BoxValue::Dimension(Dimension::Length(length)))) => {
          Some(Reading::Length(length))
        }
        _ => None,
      },
      Self::Padding => match value(PADDING) {
        Some(SlotValue::Box(BoxValue::Inset(inset))) if inset == Inset::all(inset.top) => {
          Some(Reading::Length(inset.top))
        }
        _ => None,
      },
      Self::Border(edge) => match (value(BORDER_WIDTH), value(BORDER_COLOR)) {
        (
          Some(SlotValue::Box(BoxValue::Inset(width))),
          Some(SlotValue::Box(BoxValue::Color(color))),
        ) => Some(Reading::Border(width.edge(edge), color)),
        _ => None,
      },
      Self::Ink => match value(TEXT_COLOR) {
        Some(SlotValue::Run(RunValue::Color(color))) => Some(Reading::Color(color)),
        _ => None,
      },
      Self::Hidden => match value(HIDDEN) {
        Some(SlotValue::Box(BoxValue::Hidden(hidden))) => Some(Reading::Hidden(hidden)),
        _ => None,
      },
      Self::AspectRatio => match value(ASPECT_RATIO) {
        Some(SlotValue::Box(BoxValue::AspectRatio(ratio))) => Some(Reading::Ratio(ratio.0)),
        _ => None,
      },
      Self::Overflow => match value(OVERFLOW) {
        Some(SlotValue::Box(BoxValue::Overflow(overflow))) => {
          Some(Reading::Overflow(overflow.computed()))
        }
        _ => None,
      },
    };

    reading.unwrap_or(Reading::Missing)
  }
}

/// The value `resolved`, a style or a frame resolved at the read node, gives the property
/// `key`.
fn resolved_value(
  resolved: &ResolvedStyle<f32, WebStyle>,
  key: PropertyKey<WebProperty>,
) -> Option<SlotValue<f32, WebStyle>> {
  resolved
    .boxed
    .value(key)
    .map(SlotValue::Box)
    .or_else(|| resolved.run.value(key).map(SlotValue::Run))
}

/// `value` held to the range of the property `key`, as CSS holds an interpolated value.
fn in_range(
  value: SlotValue<f32, WebStyle>,
  key: PropertyKey<WebProperty>,
) -> SlotValue<f32, WebStyle> {
  match value {
    SlotValue::Box(value) => SlotValue::Box(value.in_range_of(key)),
    SlotValue::Run(value) => SlotValue::Run(value),
  }
}

/// The value `case`'s read node presents once its steps are taken: each step retargets
/// the node's motion from the style it held to the style the step's write compiles to,
/// and the reading samples the tweens and animations at the steps' elapsed time.
fn present(case: Case, read: &Realized, source: &ParitySource) -> Reading {
  let node = source.node;
  let keys = case.read().keys();
  let reduced = source.environment.reduced_motion;
  let mut motion = Motion::<WebProperty, SlotValue<f32, WebStyle>>::started_at(0.);
  let mut now = 0.;
  let mut style = read.resolve(source, node);

  for (written, milliseconds) in case.steps() {
    if let Some(written) = written {
      MOVING
        .get()
        .expect("the moving case's number")
        .set(*written);
      web::end_turn();

      let next = read.resolve(source, node);

      for key in keys {
        motion.retarget(
          *key,
          resolved_value(&style, *key).as_ref(),
          resolved_value(&next, *key).as_ref(),
          read.style.transitions(),
          now,
          reduced,
        );
      }

      style = next;
    }

    #[expect(
      clippy::cast_precision_loss,
      reason = "a step lasts far fewer milliseconds than f64 holds exactly"
    )]
    let elapsed = *milliseconds as f64 / 1000.;

    now += elapsed;
  }

  let presented: Vec<_> = keys
    .iter()
    .map(|key| {
      let target = resolved_value(&style, *key);
      let underlying = motion
        .sample(*key, now)
        .map(|sample| in_range(sample, *key))
        .or_else(|| target.clone());
      let value = motion.animate(
        read.style.animations(),
        underlying,
        |frame| resolved_value(&Realized::resolve_frame(source, node, frame), *key),
        now,
        reduced,
      );
      let value = if value == target {
        value
      }
      else {
        value.map(|sample| in_range(sample, *key))
      };

      (*key, value)
    })
    .collect();

  case.read().reading(|key| {
    presented
      .iter()
      .find(|(presented, _)| *presented == key)
      .and_then(|(_, value)| value.clone())
  })
}

/// What the shared half resolves `case` to over the browser leaf's scene, its viewport
/// the browser's.
fn resolved(case: Case, viewport: Size<f32>) -> Reading {
  let root = document().create_element("div").expect("a root");

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect the root");
  web::install(root.clone());
  READ_BOX.set(None);
  READ_RUN.set(None);
  READ_CONTROL.set(None);

  let _ = mount(move || case.build());

  if case.focuses()
    && let Some(control) = root
      .query_selector("[data-skene-control]")
      .ok()
      .flatten()
      .and_then(|control| control.dyn_into::<web_sys::HtmlElement>().ok())
  {
    let _ = control.focus();
  }

  let read = match case.read() {
    Read::Ink => READ_RUN.take(),
    _ => READ_BOX.take(),
  }
  .expect("the case's read node");
  let node = read
    .node
    .or_else(|| READ_CONTROL.get().and_then(NodeHandle::node))
    .expect("the read node's id");
  let generation = GENERATION.get() + 1;

  GENERATION.set(generation);

  let environment = Environment {
    viewport,
    ..untrack(|| <Web as Realization>::media().environment())
  };
  let source = ParitySource {
    environment,
    node,
    slots: read.slots.clone(),
    generation,
  };
  let reading = present(case, &read, &source);

  web::uninstall();
  root.remove();

  reading
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

  let _ = mount(move || case.build());

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
      web::end_turn();
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
        Edge::End => "right",
        Edge::Bottom => "bottom",
        Edge::Start => "left",
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
    Read::Hidden => match computed(&element, "visibility").as_str() {
      "visible" => Reading::Hidden(false),
      "hidden" => Reading::Hidden(true),
      _ => Reading::Missing,
    },
    Read::AspectRatio => ratio(&computed(&element, "aspect-ratio")),
    Read::Overflow => overflow_keyword(&computed(&element, "overflow-x"))
      .zip(overflow_keyword(&computed(&element, "overflow-y")))
      .map_or(Reading::Missing, |(horizontal, vertical)| {
        Reading::Overflow(Axes::new(horizontal, vertical))
      }),
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
/// hundredth of a pixel, ratios within a thousandth, visibilities and overflows exactly.
fn agree(resolved: &Reading, rendered: &Reading) -> bool {
  match (resolved, rendered) {
    (Reading::Color(left), Reading::Color(right)) => same_color(*left, *right),
    (Reading::Length(left), Reading::Length(right)) => (left - right).abs() < 0.01,
    (Reading::Border(left_width, left), Reading::Border(right_width, right)) => {
      (left_width - right_width).abs() < 0.01 && same_color(*left, *right)
    }
    (Reading::Hidden(left), Reading::Hidden(right)) => left == right,
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
