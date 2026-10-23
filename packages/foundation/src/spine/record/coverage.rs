//! Samples of every property, condition, expression and leaf kind, and the check that a
//! record holds a style made of them whole and resolves it as the typed style does.
//!
//! Each sampled enum is enumerated by a match naming every variant without a wildcard,
//! each arm giving the sample that follows its own, so a variant added later fails to
//! compile until it is sampled.

use std::{cell::Cell, fmt::Debug, hash::Hasher, mem::discriminant};

use rustc_hash::FxHasher;

use crate::spine::{
  Animation,
  Arrangement,
  AspectRatio,
  Axes,
  BoxDynamics,
  BoxField,
  BoxFieldVisitor,
  BoxStyle,
  BoxValue,
  BoxVisitor,
  CheckState,
  Color,
  ColorScheme,
  Condition,
  Conditional,
  ContainerName,
  ContainerQuery,
  ControlState,
  Definition,
  Dimension,
  Easing,
  Edge,
  Environment,
  Expr,
  ExprEnv,
  ExprNode,
  ExprValue,
  Fingerprint,
  Flexibility,
  Focus,
  Frame,
  GridAreas,
  GridFlow,
  GridName,
  GridPlacement,
  GridTemplate,
  GridTrack,
  Inset,
  Interaction,
  IntoBoxValue,
  IntoRunValue,
  ItemAlignment,
  Iterations,
  KeptLeaf,
  Keyframe,
  Length,
  Line,
  Media,
  MouseCursor,
  Overflow,
  Plain,
  PointerPrecision,
  Property,
  PropertyKey,
  Record,
  Resolved,
  Resolver,
  RunDynamics,
  RunField,
  RunFieldVisitor,
  RunStyle,
  RunValue,
  RunVisitor,
  Scalar,
  Size,
  Slot,
  SlotLeaf,
  SlotValue,
  Spacing,
  State,
  Static,
  StaticLeaf,
  StaticValue,
  StyleExtension,
  StyleProperty,
  TextDecoration,
  TextDecorationStyle,
  TokenName,
  TokenValue,
  TrackLimit,
  Transition,
  Wrap,
};

/// The color token the samples define.
const ACCENT: TokenName = TokenName::new("coverage-accent");
/// The length token the samples define.
const GUTTER: TokenName = TokenName::new("coverage-gutter");
/// The number token the samples define.
const RATIO: TokenName = TokenName::new("coverage-ratio");
/// The dimension token the samples define.
const WIDE: TokenName = TokenName::new("coverage-wide");
/// The container the samples query by name.
const CARD: ContainerName = ContainerName("card");

/// A medium held to the record: its own properties, values and token definitions beside
/// the shared ones.
///
/// [`check`](Self::check) builds a style setting every property of the medium, each
/// under variants of every condition kind with every leaf and expression kind among
/// them, and its motion, and proves that a [`Record`] holds them whole and resolves
/// them, under environments and interactions making every condition both hold and
/// fail, as the typed styles resolve.
#[derive(Debug)]
#[must_use]
pub struct Medium<N: Scalar, X: StyleExtension> {
  /// Every property of the medium's own, in declaration order.
  pub properties: Vec<X::Property>,
  /// Values of the medium's own types, at least two of each.
  pub values: Vec<X::Value>,
  /// The medium's token definitions, each name read by some leaf.
  pub definitions: Vec<(TokenName, TokenValue<N>)>,
}

impl<N: Scalar, X: StyleExtension> Medium<N, X> {
  /// A medium with no property, value or definition of its own.
  pub const fn shared() -> Self {
    Self {
      properties: Vec::new(),
      values: Vec::new(),
      definitions: Vec::new(),
    }
  }

  /// Proves that every property, condition, expression and leaf kind of the medium
  /// crosses a record whole and resolves through it as through the typed style.
  ///
  /// # Panics
  ///
  /// Where any of it does not, naming what.
  pub fn check(self) {
    let keys = Keys::<X::Property>::of::<N, X>();

    assert_eq!(
      keys.shared(),
      Property::samples(),
      "the visitors visit every shared property, in declaration order"
    );
    assert_eq!(
      keys.extension(),
      self.properties,
      "the visitors visit every property of the medium, in declaration order"
    );

    let mut definitions = Self::definitions();

    definitions.extend(self.definitions);

    let mut sampler = Sampler::<N, X>::new(&self.values, &definitions);

    BoxStyle::<N, Static<Condition<N>>, X>::for_each_field(&mut sampler);
    RunStyle::<Static<Condition<N>>, X>::for_each_field(&mut sampler);
    sampler.assert_covers(&definitions);

    let box_dynamics = sampler.box_dynamics(&keys.boxed);
    let run_dynamics = sampler.run_dynamics(&keys.run);
    let record = sampler.assert_crosses(&keys, &box_dynamics, &run_dynamics);

    sampler.assert_resolves(&record, &keys, &definitions, &box_dynamics, &run_dynamics);
  }

  /// The definitions of the tokens the samples name beside the medium's own, each
  /// varying under a media condition.
  fn definitions() -> Vec<(TokenName, TokenValue<N>)> {
    vec![
      (
        ACCENT,
        Definition::<Color, N>::from(Color::srgb(0.2, 0.4, 0.8))
          .when(Media::Dark, Color::srgb(0.6, 0.7, 1.))
          .into(),
      ),
      (
        GUTTER,
        Definition::<Length, N>::from(Length(12.))
          .when(Media::MinWidth(N::from_f32(500.)), Length(16.))
          .into(),
      ),
      (
        RATIO,
        Definition::<f32, N>::from(1.5)
          .when(Media::Hover, 2.)
          .into(),
      ),
      (
        WIDE,
        Definition::<Dimension<N>, N>::from(Dimension::Length(N::from_f32(640.)))
          .when(
            Media::Container {
              name: None,
              query: ContainerQuery::MaxWidth(N::from_f32(200.)),
            },
            Dimension::Length(N::from_f32(80.)),
          )
          .into(),
      ),
    ]
  }
}

/// The samples of `first`'s enum, one per variant: `next` names the sample following the
/// one it is given, `None` after the last.
///
/// # Panics
///
/// Where `next` names a variant a second time.
pub fn chain<T: Debug>(first: T, mut next: impl FnMut(&T) -> Option<T>) -> Vec<T> {
  let mut samples: Vec<T> = Vec::new();
  let mut sample = Some(first);

  while let Some(current) = sample {
    assert!(
      samples
        .iter()
        .all(|seen| discriminant(seen) != discriminant(&current)),
      "{current:?} is sampled once"
    );

    sample = next(&current);
    samples.push(current);
  }

  samples
}

impl Property {
  /// Every shared property, in declaration order.
  fn samples() -> Vec<Self> {
    chain(Self::Width, |property| match property {
      Self::Width => Some(Self::Height),
      Self::Height => Some(Self::MinWidth),
      Self::MinWidth => Some(Self::MinHeight),
      Self::MinHeight => Some(Self::MaxWidth),
      Self::MaxWidth => Some(Self::MaxHeight),
      Self::MaxHeight => Some(Self::AspectRatio),
      Self::AspectRatio => Some(Self::Margin),
      Self::Margin => Some(Self::Padding),
      Self::Padding => Some(Self::Offset),
      Self::Offset => Some(Self::Pinned),
      Self::Pinned => Some(Self::Background),
      Self::Background => Some(Self::BorderWidth),
      Self::BorderWidth => Some(Self::BorderColor),
      Self::BorderColor => Some(Self::Cursor),
      Self::Cursor => Some(Self::Spacing),
      Self::Spacing => Some(Self::Wrap),
      Self::Wrap => Some(Self::Flex),
      Self::Flex => Some(Self::Arrangement),
      Self::Arrangement => Some(Self::ItemAlignment),
      Self::ItemAlignment => Some(Self::LineArrangement),
      Self::LineArrangement => Some(Self::ItemHorizontalAlignment),
      Self::ItemHorizontalAlignment => Some(Self::SelfAlignment),
      Self::SelfAlignment => Some(Self::SelfHorizontalAlignment),
      Self::SelfHorizontalAlignment => Some(Self::GridColumns),
      Self::GridColumns => Some(Self::GridRows),
      Self::GridRows => Some(Self::GridAreas),
      Self::GridAreas => Some(Self::GridImplicitColumns),
      Self::GridImplicitColumns => Some(Self::GridImplicitRows),
      Self::GridImplicitRows => Some(Self::GridFlow),
      Self::GridFlow => Some(Self::GridRowPlacement),
      Self::GridRowPlacement => Some(Self::GridColumnPlacement),
      Self::GridColumnPlacement => Some(Self::Overflow),
      Self::Overflow => Some(Self::Hidden),
      Self::Hidden => Some(Self::TextColor),
      Self::TextColor => Some(Self::SelectionColor),
      Self::SelectionColor => Some(Self::SelectionBackground),
      Self::SelectionBackground => Some(Self::Decoration),
      Self::Decoration => Some(Self::DecorationStyle),
      Self::DecorationStyle => Some(Self::DecorationColor),
      Self::DecorationColor => None,
    })
  }
}

impl<N: Scalar> ContainerQuery<N> {
  /// One query per kind, each on 200 units, which the readings' containers straddle.
  fn samples() -> Vec<Self> {
    let length = N::from_f32(200.);

    chain(Self::MinWidth(length), |query| match query {
      Self::MinWidth(_) => Some(Self::MaxWidth(length)),
      Self::MaxWidth(_) => Some(Self::MinHeight(length)),
      Self::MinHeight(_) => Some(Self::MaxHeight(length)),
      Self::MaxHeight(_) => None,
    })
  }
}

impl<N: Scalar> Media<N> {
  /// One media condition per kind, each holding in one of the readings' environments
  /// and failing in the other.
  fn samples() -> Vec<Self> {
    let length = N::from_f32;

    chain(Self::MinWidth(length(500.)), |media| match media {
      Self::MinWidth(_) => Some(Self::MaxWidth(length(500.))),
      Self::MaxWidth(_) => Some(Self::MinHeight(length(400.))),
      Self::MinHeight(_) => Some(Self::MaxHeight(length(400.))),
      Self::MaxHeight(_) => Some(Self::Dark),
      Self::Dark => Some(Self::Light),
      Self::Light => Some(Self::ReducedMotion),
      Self::ReducedMotion => Some(Self::ReducedTransparency),
      Self::ReducedTransparency => Some(Self::HighContrast),
      Self::HighContrast => Some(Self::Pointer(PointerPrecision::Fine)),
      Self::Pointer(_) => Some(Self::Hover),
      Self::Hover => Some(Self::Container {
        name: Some(CARD),
        query: ContainerQuery::MinWidth(length(200.)),
      }),
      Self::Container { .. } => Some(Self::and([Self::MinWidth(length(500.)), Self::Hover])),
      Self::And(_) => Some(Self::or([
        Self::MaxWidth(length(500.)),
        Self::Pointer(PointerPrecision::Coarse),
      ])),
      Self::Or(_) => None,
    })
  }
}

impl State {
  /// Every state.
  fn samples() -> Vec<Self> {
    chain(Self::Hovered, |state| match state {
      Self::Hovered => Some(Self::Focused),
      Self::Focused => Some(Self::FocusVisible),
      Self::FocusVisible => Some(Self::Pressed),
      Self::Pressed => Some(Self::Dragged),
      Self::Dragged => Some(Self::Targeted),
      Self::Targeted => Some(Self::Checked),
      Self::Checked => Some(Self::Indeterminate),
      Self::Indeterminate => Some(Self::Disabled),
      Self::Disabled => Some(Self::Invalid),
      Self::Invalid => Some(Self::Open),
      Self::Open => None,
    })
  }
}

impl<N: Scalar> Condition<N> {
  /// One condition per kind, each holding in some reading and failing in another.
  fn samples() -> Vec<Self> {
    chain(Self::Media(Media::Dark), |condition| match condition {
      Self::Media(_) => Some(Self::State(State::Hovered)),
      Self::State(_) => Some(Self::and([
        Media::MinWidth(N::from_f32(500.)).into(),
        State::Pressed.into(),
      ])),
      Self::And(_) => Some(Self::or([Media::Light.into(), State::Indeterminate.into()])),
      Self::Or(_) => None,
    })
  }

  /// Every condition the sampled styles vary under: each media condition, each container
  /// query of the nearest container, each state, and each condition kind.
  fn every() -> Vec<Self> {
    Media::samples()
      .into_iter()
      .chain(
        ContainerQuery::samples()
          .into_iter()
          .map(|query| Media::Container { name: None, query }),
      )
      .map(Self::Media)
      .chain(State::samples().into_iter().map(Self::State))
      .chain(Self::samples())
      .collect()
  }
}

impl ExprNode {
  /// One expression per node kind, each a whole expression naming the samples' tokens.
  fn samples() -> Vec<Self> {
    let length = |value: f32| Box::new(Self::Length(value));
    let number = |value: f32| Box::new(Self::Number(value));
    let color = || Box::new(Self::Color(Color::srgb(0.8, 0.3, 0.1)));

    chain(Self::Number(2.), |node| match node {
      Self::Number(_) => Some(Self::Length(4.)),
      Self::Length(_) => Some(Self::ViewportWidth(0.01)),
      Self::ViewportWidth(_) => Some(Self::ViewportHeight(0.02)),
      Self::ViewportHeight(_) => Some(Self::ContainerWidth(0.1)),
      Self::ContainerWidth(_) => Some(Self::ContainerHeight(0.2)),
      Self::ContainerHeight(_) => Some(Self::FontSize(1.5)),
      Self::FontSize(_) => Some(Self::SafeArea(Edge::Top)),
      Self::SafeArea(_) => Some(Self::Token(GUTTER)),
      Self::Token(_) => Some(Self::Color(Color::srgb(0.1, 0.6, 0.3))),
      Self::Color(_) => Some(Self::Add(length(3.), Box::new(Self::FontSize(0.5)))),
      Self::Add(..) => Some(Self::Sub(Box::new(Self::ViewportWidth(0.1)), length(2.))),
      Self::Sub(..) => Some(Self::Mul(length(3.), number(2.))),
      Self::Mul(..) => Some(Self::Div(length(9.), number(3.))),
      Self::Div(..) => Some(Self::Min(length(8.), Box::new(Self::ViewportHeight(0.05)))),
      Self::Min(..) => Some(Self::Max(length(1.), Box::new(Self::ContainerWidth(0.05)))),
      Self::Max(..) => Some(Self::Clamp(
        Box::new(Self::ViewportWidth(0.5)),
        length(10.),
        length(200.),
      )),
      Self::Clamp(..) => Some(Self::Mix(color(), Box::new(Self::Token(ACCENT)), 0.25)),
      Self::Mix(..) => Some(Self::WithAlpha(color(), 0.5)),
      Self::WithAlpha(..) => Some(Self::Lighter(color(), 0.1)),
      Self::Lighter(..) => Some(Self::Darker(color(), 0.1)),
      Self::Darker(..) => Some(Self::WithChroma(color(), 0.05)),
      Self::WithChroma(..) => Some(Self::WithHue(color(), 200.)),
      Self::WithHue(..) => None,
    })
  }
}

impl<T: Debug> StaticLeaf<T> {
  /// One static leaf per kind the property takes: `fixed`, then the token and the
  /// expression where it reads them.
  fn samples(fixed: T, token: Option<TokenName>, mut expr: Option<ExprNode>) -> Vec<Self> {
    chain(Self::Fixed(fixed), |leaf| match leaf {
      Self::Fixed(_) => token.map(Self::Token),
      Self::Token(_) => expr.take().map(|node| Self::Expr(Expr::from_node(node))),
      Self::Expr(_) => None,
    })
  }

  /// The leaf's kind.
  const fn kind(&self) -> &'static str {
    match self {
      Self::Fixed(_) => "fixed",
      Self::Token(_) => "token",
      Self::Expr(_) => "expression",
    }
  }
}

impl<T: Debug> KeptLeaf<T> {
  /// One kept leaf per kind: `leaf` compiled ahead, and `slot` written by a derived
  /// leaf.
  fn samples(leaf: StaticLeaf<T>, slot: Slot) -> Vec<Self> {
    chain(Self::Static(leaf), |kept| match kept {
      Self::Static(_) => Some(Self::Slot(slot)),
      Self::Slot(_) => None,
    })
  }

  /// The leaf's kind.
  const fn kind(&self) -> &'static str {
    match self {
      Self::Static(_) => "static",
      Self::Slot(_) => "slot",
    }
  }
}

impl<N: Scalar, X: StyleExtension> BoxValue<N, X> {
  /// Two values of each shared box type, then `extension`'s.
  fn samples(extension: &[X::Value]) -> Vec<Self> {
    let length = N::from_f32;
    let name = GridName::new;
    let template = GridTemplate::new()
      .add_line(name("first"))
      .add_track(GridTrack::Length(length(40.)))
      .add_track(GridTrack::Adaptive(
        TrackLimit::Length(length(20.)),
        TrackLimit::Flex(1.),
      ))
      .add_line(name("last"));

    [
      Self::Dimension(Dimension::Length(length(10.))),
      Self::Dimension(Dimension::Fraction(0.25)),
      Self::Length(Length(6.)),
      Self::Length(Length(9.)),
      Self::Number(3.),
      Self::Number(0.5),
      Self::AspectRatio(AspectRatio::new(16., 9.)),
      Self::AspectRatio(AspectRatio(1.)),
      Self::DimensionInset(Inset::all(Dimension::Fraction(0.1))),
      Self::DimensionInset(Inset {
        start: Dimension::Length(length(1.)),
        top: Dimension::Auto,
        end: Dimension::Length(length(3.)),
        bottom: Dimension::Fraction(0.5),
      }),
      Self::Inset(Inset::all(length(2.))),
      Self::Inset(Inset {
        start: length(1.),
        top: length(2.),
        end: length(3.),
        bottom: length(4.),
      }),
      Self::Color(Color::srgb(0.9, 0.9, 0.9)),
      Self::Color(Color::srgb(0.1, 0.2, 0.3).with_alpha(0.5)),
      Self::Cursor(MouseCursor::Click),
      Self::Cursor(MouseCursor::Grab),
      Self::Spacing(Spacing::all(length(8.))),
      Self::Spacing(Spacing {
        horizontal: length(4.),
        vertical: length(12.),
      }),
      Self::Wrap(Wrap::Forward),
      Self::Wrap(Wrap::Reverse),
      Self::Flexibility(Flexibility::grow(1.)),
      Self::Flexibility(Flexibility::new(Dimension::Length(length(30.)), 0., 2.)),
      Self::Arrangement(Arrangement::SpaceBetween),
      Self::Arrangement(Arrangement::Center),
      Self::ItemAlignment(ItemAlignment::Baseline),
      Self::ItemAlignment(ItemAlignment::End),
      Self::Tracks(vec![
        GridTrack::Flex(1.),
        GridTrack::FitContent(length(50.)),
      ]),
      Self::Tracks(vec![GridTrack::Between(
        TrackLimit::MinContent,
        TrackLimit::Fraction(0.5),
      )]),
      Self::Template(template),
      Self::Template(
        GridTemplate::new()
          .add_track(GridTrack::Auto)
          .add_track(GridTrack::MaxContent),
      ),
      Self::Areas(GridAreas::new(["head head", "side main"])),
      Self::Areas(GridAreas::new(["main ."])),
      Self::GridFlow(GridFlow::DenseColumns),
      Self::GridFlow(GridFlow::Rows),
      Self::Placement(Line::area(name("head"))),
      Self::Placement(Line {
        start: GridPlacement::Named(name("first"), 1),
        end: GridPlacement::NamedSpan(name("last"), 2),
      }),
      Self::Overflow(Axes::new(Overflow::Clip, Overflow::ScrollIfNeeded)),
      Self::Overflow(Axes::all(Overflow::Scroll)),
      Self::Hidden(true),
      Self::Hidden(false),
    ]
    .into_iter()
    .chain(extension.iter().cloned().map(Self::Extension))
    .collect()
  }
}

impl<X: StyleExtension> RunValue<X> {
  /// Two values of each shared run type, then `extension`'s.
  fn samples(extension: &[X::Value]) -> Vec<Self> {
    [
      Self::Color(Color::srgb(0.2, 0.2, 0.2)),
      Self::Color(Color::TRANSPARENT),
      Self::Length(Length(14.)),
      Self::Length(Length(0.5)),
      Self::Decoration(TextDecoration::LineThrough),
      Self::Decoration(TextDecoration::Overline),
      Self::DecorationStyle(TextDecorationStyle::Wavy),
      Self::DecorationStyle(TextDecorationStyle::Dotted),
    ]
    .into_iter()
    .chain(extension.iter().cloned().map(Self::Extension))
    .collect()
  }
}

/// The kind of value a token or an expression gives the property reading it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
  /// A length, read by every property measured in lengths.
  Length,
  /// A unitless number.
  Number,
  /// A color.
  Color,
}

impl Kind {
  /// The kind of the token `definition` defines, where a property reads it.
  const fn of_token<N: Scalar>(definition: &TokenValue<N>) -> Option<Self> {
    match definition {
      TokenValue::Length(_) | TokenValue::Dimension(_) => Some(Self::Length),
      TokenValue::Number(_) => Some(Self::Number),
      TokenValue::Color(_) => Some(Self::Color),
      TokenValue::Text(_) => None,
    }
  }

  /// The kind an expression's value has.
  const fn of_value(value: ExprValue) -> Self {
    match value {
      ExprValue::Length(_) => Self::Length,
      ExprValue::Number(_) => Self::Number,
      ExprValue::Color(_) => Self::Color,
    }
  }

  /// A value of this kind, where an expression evaluates a token.
  const fn value(self) -> ExprValue {
    match self {
      Self::Length => ExprValue::Length(1.),
      Self::Number => ExprValue::Number(1.),
      Self::Color => ExprValue::Color(Color::TRANSPARENT),
    }
  }

  /// The kind a box property of type `T` reads from a token or an expression.
  fn of_box<N: Scalar, X: StyleExtension, T: IntoBoxValue<N, X>>() -> Option<Self> {
    [
      (Self::Length, BoxValue::Length(Length(1.))),
      (Self::Number, BoxValue::Number(1.)),
      (Self::Color, BoxValue::Color(Color::TRANSPARENT)),
    ]
    .into_iter()
    .find_map(|(kind, value)| T::from_box_value(value).map(|_| kind))
  }

  /// The kind a run property of type `T` reads from a token or an expression.
  fn of_run<X: StyleExtension, T: IntoRunValue<X>>() -> Option<Self> {
    [
      (Self::Length, Some(RunValue::Length(Length(1.)))),
      (Self::Number, X::run_value_from_number(1.)),
      (Self::Color, Some(RunValue::Color(Color::TRANSPARENT))),
    ]
    .into_iter()
    .find_map(|(kind, value)| value.and_then(T::from_run_value).map(|_| kind))
  }
}

/// Builds a style setting every property, each under variants drawn in turn from every
/// condition, its leaves of every kind the property takes, and records the leaves its
/// slots hold.
struct Sampler<N: Scalar, X: StyleExtension> {
  box_values: Vec<BoxValue<N, X>>,
  run_values: Vec<RunValue<X>>,
  conditions: Vec<Condition<N>>,
  next_condition: usize,
  tokens: Vec<(Kind, TokenName)>,
  nodes: Vec<(Kind, ExprNode)>,
  token_turns: [usize; 3],
  node_turns: [usize; 3],
  slots: Vec<SlotLeaf<N, X>>,
  boxed: BoxStyle<N, Static<Condition<N>>, X>,
  run: RunStyle<Static<Condition<N>>, X>,
}

impl<N: Scalar, X: StyleExtension> Sampler<N, X> {
  /// A sampler over the medium's `values` and the tokens `definitions` defines.
  fn new(values: &[X::Value], definitions: &[(TokenName, TokenValue<N>)]) -> Self {
    let tokens: Vec<_> = definitions
      .iter()
      .filter_map(|(name, definition)| Kind::of_token(definition).map(|kind| (kind, *name)))
      .collect();
    let environment = ExprEnv {
      viewport: Size::new(100., 100.),
      container: None,
      font_size: 16.,
      safe_area: Inset::all(1.),
    };
    let token_value = |name: TokenName| {
      tokens
        .iter()
        .find(|(_, token)| *token == name)
        .map(|(kind, _)| kind.value())
    };
    let nodes = ExprNode::samples()
      .into_iter()
      .map(|node| {
        let value = node.evaluate(&environment, &token_value);

        (
          Kind::of_value(value.unwrap_or_else(|| panic!("{node:?} evaluates"))),
          node,
        )
      })
      .collect();

    Self {
      box_values: BoxValue::samples(values),
      run_values: RunValue::samples(values),
      conditions: Condition::every(),
      next_condition: 0,
      tokens,
      nodes,
      token_turns: [0; 3],
      node_turns: [0; 3],
      slots: Vec::new(),
      boxed: BoxStyle::default(),
      run: RunStyle::default(),
    }
  }

  /// The next condition in turn.
  fn condition(&mut self) -> Condition<N> {
    let condition = self.conditions[self.next_condition % self.conditions.len()].clone();

    self.next_condition += 1;

    condition
  }

  /// The next of `items` of `kind`, taken in turn per kind as `turns` counts; `None`
  /// where there is none.
  fn next_of<T: Clone>(items: &[(Kind, T)], turns: &mut [usize; 3], kind: Kind) -> Option<T> {
    let of_kind: Vec<_> = items
      .iter()
      .filter(|(item, _)| *item == kind)
      .map(|(_, item)| item)
      .collect();
    let turn = &mut turns[kind as usize];
    let item = (!of_kind.is_empty()).then(|| of_kind[*turn % of_kind.len()].clone());

    *turn += 1;

    item
  }

  /// The value of a property of type `T` reading `kind`: the first of `values` fixed, and
  /// a variant per leaf the property takes, fixed, a token, two expressions and a slot
  /// holding one of them, erased by `erase`.
  fn value<T: Clone + Debug>(
    &mut self,
    key: PropertyKey<X::Property>,
    values: &[T],
    kind: Option<Kind>,
    erase: impl Fn(StaticLeaf<T>) -> SlotLeaf<N, X>,
  ) -> StaticValue<T, Condition<N>> {
    let [base, other, ..] = values
    else {
      panic!("`{}` reads two of the sampled values", key.name());
    };
    let token = kind.and_then(|kind| Self::next_of(&self.tokens, &mut self.token_turns, kind));
    let expr = kind.and_then(|kind| Self::next_of(&self.nodes, &mut self.node_turns, kind));
    let mut leaves = StaticLeaf::samples(other.clone(), token, expr);

    if let Some(node) = kind.and_then(|kind| Self::next_of(&self.nodes, &mut self.node_turns, kind))
    {
      leaves.push(StaticLeaf::Expr(Expr::from_node(node)));
    }

    let slot = Slot(u16::try_from(self.slots.len()).expect("the slots fit"));

    self
      .slots
      .push(erase(leaves[self.slots.len() % leaves.len()].clone()));

    let Some(last) = leaves.pop()
    else {
      panic!("a fixed leaf at least");
    };
    let kept = leaves
      .into_iter()
      .map(KeptLeaf::Static)
      .chain(KeptLeaf::samples(last, slot));
    let mut value = Conditional::new(KeptLeaf::Static(StaticLeaf::Fixed(base.clone())));

    for leaf in kept {
      value = value.when(self.condition(), leaf);
    }

    value
  }

  /// Proves that every condition, every token `definitions` defines, every expression
  /// and every leaf kind sits under some property's variants or in some slot.
  fn assert_covers(&self, definitions: &[(TokenName, TokenValue<N>)]) {
    let mut seen = Seen::default();

    self.boxed.visit(&mut seen);
    self.run.visit(&mut seen);

    for leaf in &self.slots {
      seen.leaf(leaf);
    }

    for condition in &self.conditions {
      assert!(
        seen.conditions.contains(condition),
        "{condition:?} varies a property"
      );
    }

    for node in ExprNode::samples() {
      assert!(seen.nodes.contains(&node), "{node:?} is a property's leaf");
    }

    for (name, definition) in definitions {
      if Kind::of_token(definition).is_some() {
        assert!(
          seen.tokens.contains(name),
          "`{name:?}` is a property's leaf"
        );
      }
    }

    let kinds = StaticLeaf::samples((), Some(ACCENT), Some(ExprNode::Number(1.)))
      .iter()
      .map(StaticLeaf::kind)
      .chain(
        KeptLeaf::samples(StaticLeaf::Fixed(()), Slot(0))
          .iter()
          .map(KeptLeaf::kind),
      )
      .collect::<Vec<_>>();

    for kind in kinds {
      assert!(seen.kinds.contains(&kind), "a {kind} leaf is a property's");
    }
  }

  /// Projects the sampled styles and `box_dynamics` and `run_dynamics` into a record,
  /// proving that each crosses whole, and returns it.
  fn assert_crosses(
    &self,
    keys: &Keys<X::Property>,
    box_dynamics: &BoxDynamics<N, X>,
    run_dynamics: &RunDynamics<X>,
  ) -> Record<N, X> {
    let mut record = Record::<N, X>::new();

    record.add_box(&self.boxed);
    record.add_run(&self.run);
    record.add_box_dynamics(box_dynamics);
    record.add_run_dynamics(run_dynamics);

    assert_eq!(record.box_style(), self.boxed, "a box crosses whole");
    assert_eq!(record.run_style(), self.run, "a run crosses whole");
    assert_eq!(
      record.entries().len(),
      keys.boxed.len() + keys.run.len(),
      "every property is an entry"
    );
    assert_eq!(
      record.transitions(),
      [
        box_dynamics.transitions.clone(),
        run_dynamics.transitions.clone()
      ]
      .concat(),
      "transitions cross whole"
    );

    let (box_animations, run_animations) =
      record.animations().split_at(box_dynamics.animations.len());

    assert_eq!(
      box_animations
        .iter()
        .map(|animation| animation.map_keyframes(Frame::box_keyframe))
        .collect::<Vec<_>>(),
      box_dynamics.animations,
      "box keyframes cross whole"
    );
    assert_eq!(
      run_animations
        .iter()
        .map(|animation| animation.map_keyframes(Frame::run_keyframe))
        .collect::<Vec<_>>(),
      run_dynamics.animations,
      "run keyframes cross whole"
    );

    record
  }

  /// Proves that `record`, holding the sampled styles and their motion, resolves as they
  /// do in every reading, that every sampled condition both holds and fails among the
  /// readings, and that every property resolves to a value in some reading.
  fn assert_resolves(
    &self,
    record: &Record<N, X>,
    keys: &Keys<X::Property>,
    definitions: &[(TokenName, TokenValue<N>)],
    box_dynamics: &BoxDynamics<N, X>,
    run_dynamics: &RunDynamics<X>,
  ) {
    let (box_animations, run_animations) =
      record.animations().split_at(box_dynamics.animations.len());
    let mut resolved_keys = Vec::new();
    let mut fingerprints = Vec::new();
    let mut held = vec![(false, false); self.conditions.len()];

    for scenario in Scenario::<N>::all() {
      scenario.with_resolver(definitions, &self.slots, |resolver: &Resolver<'_, N, X>| {
        let resolved = record.resolve(resolver);
        let boxed = resolver.box_style(&self.boxed);
        let run = resolver.run_style(&self.run);

        assert_same(&resolved.boxed, &boxed, "a box resolves as its typed style");
        assert_same(&resolved.run, &run, "a run resolves as its typed style");

        for (frames, animation) in box_animations.iter().zip(&box_dynamics.animations) {
          for ((_, frame), (_, keyframe)) in frames.keyframes.iter().zip(&animation.keyframes) {
            assert_same(
              &frame.resolve(resolver).boxed,
              &resolver.box_keyframe(keyframe),
              "a box keyframe resolves as its typed keyframe",
            );
          }
        }

        for (frames, animation) in run_animations.iter().zip(&run_dynamics.animations) {
          for ((_, frame), (_, keyframe)) in frames.keyframes.iter().zip(&animation.keyframes) {
            assert_same(
              &frame.resolve(resolver).run,
              &resolver.run_keyframe(keyframe),
              "a run keyframe resolves as its typed keyframe",
            );
          }
        }

        for ((holds, fails), condition) in held.iter_mut().zip(&self.conditions) {
          if *resolver.winning(&Conditional::new(false).when(condition.clone(), true)) {
            *holds = true;
          }
          else {
            *fails = true;
          }
        }

        assert!(resolver.interaction_read.get(), "the states were read");

        boxed.visit(&mut SetKeys(&mut resolved_keys));
        run.visit(&mut SetKeys(&mut resolved_keys));
        fingerprints.push((fingerprint(&boxed), fingerprint(&run)));
      });
    }

    for ((holds, fails), condition) in held.iter().zip(&self.conditions) {
      assert!(
        *holds && *fails,
        "{condition:?} both holds and fails among the readings"
      );
    }

    for key in keys.boxed.iter().chain(&keys.run) {
      assert!(
        resolved_keys.contains(key),
        "`{}` resolves to a value in some reading",
        key.name()
      );
    }

    for (index, fingerprint) in fingerprints.iter().enumerate() {
      assert!(
        !fingerprints[..index].contains(fingerprint),
        "every reading resolves the style differently"
      );
    }
  }

  /// The motion of the sampled box: a transition per property in `keys`, each easing
  /// differently, and two animations over keyframes of the box's static leaves.
  fn box_dynamics(&self, keys: &[PropertyKey<X::Property>]) -> BoxDynamics<N, X> {
    BoxDynamics {
      transitions: Self::transitions(keys.iter().copied().map(StyleProperty::from)),
      animations: vec![
        Animation::new(
          1.,
          [0., 25., 75., 100.]
            .into_iter()
            .zip(0..)
            .map(|(percent, index)| (percent, self.box_keyframe(index))),
        )
        .set_delay(0.5)
        .set_easing(Easing::CubicBezier(0.2, 0., 0.4, 1.))
        .set_iterations(Iterations::Infinite)
        .set_alternate(true)
        .set_respects_reduced_motion(false),
        Animation::new(0.5, [(100., self.box_keyframe(3))]).set_iterations(2),
      ],
    }
  }

  /// The motion of the sampled run, as [`box_dynamics`](Self::box_dynamics) gives the
  /// box's, with a transition of every property besides.
  fn run_dynamics(&self, keys: &[PropertyKey<X::Property>]) -> RunDynamics<X> {
    RunDynamics {
      transitions: Self::transitions(
        keys
          .iter()
          .copied()
          .map(StyleProperty::from)
          .chain([StyleProperty::All]),
      ),
      animations: vec![
        Animation::new(
          2.,
          [0., 50., 75., 100.]
            .into_iter()
            .zip(0..)
            .map(|(percent, index)| (percent, self.run_keyframe(index))),
        )
        .set_easing(Easing::Linear),
      ],
    }
  }

  /// A transition of each of `properties`, the easings, delays and reduced-motion
  /// choices taken in turn.
  fn transitions(
    properties: impl Iterator<Item = StyleProperty<X::Property>>,
  ) -> Vec<Transition<X::Property>> {
    let easings = [
      Easing::Linear,
      Easing::Ease,
      Easing::EaseIn,
      Easing::EaseOut,
      Easing::EaseInOut,
      Easing::CubicBezier(0.1, 0.7, 1., 0.1),
    ];

    properties
      .zip(easings.into_iter().cycle())
      .zip([0., 0.1, 0.2].into_iter().cycle())
      .zip([true, false].into_iter().cycle())
      .map(|(((property, easing), delay), respects)| {
        Transition::new(property, 0.3)
          .set_easing(easing)
          .set_delay(delay)
          .set_respects_reduced_motion(respects)
      })
      .collect()
  }

  /// A box keyframe of each property's `index`-th static leaf, counted round its value.
  fn box_keyframe(&self, index: usize) -> BoxStyle<N, Keyframe, X> {
    let mut frame = KeyframeOf {
      boxed: &self.boxed,
      run: &self.run,
      index,
      box_frame: BoxStyle::default(),
      run_frame: RunStyle::default(),
    };

    BoxStyle::<N, Keyframe, X>::for_each_field(&mut frame);

    frame.box_frame
  }

  /// A run keyframe of each property's `index`-th static leaf, counted round its value.
  fn run_keyframe(&self, index: usize) -> RunStyle<Keyframe, X> {
    let mut frame = KeyframeOf {
      boxed: &self.boxed,
      run: &self.run,
      index,
      box_frame: BoxStyle::default(),
      run_frame: RunStyle::default(),
    };

    RunStyle::<Keyframe, X>::for_each_field(&mut frame);

    frame.run_frame
  }
}

/// The values among `samples` a property's type reads as exactly themselves: those `read`
/// reads and `erase` gives back unchanged.
fn readings<V: Clone + PartialEq, T: Clone>(
  samples: &[V],
  read: impl Fn(V) -> Option<T>,
  erase: impl Fn(T) -> V,
) -> Vec<T> {
  samples
    .iter()
    .filter_map(|sample| read(sample.clone()).filter(|value| erase(value.clone()) == *sample))
    .collect()
}

impl<N: Scalar, X: StyleExtension> BoxFieldVisitor<N, X> for Sampler<N, X> {
  fn field<F: BoxField<N, X>>(&mut self) {
    let values = readings(
      &self.box_values,
      <F::Value as IntoBoxValue<N, X>>::from_box_value,
      <F::Value as IntoBoxValue<N, X>>::into_box_value,
    );
    let value = self.value(F::KEY, &values, Kind::of_box::<N, X, F::Value>(), |leaf| {
      leaf.map(|fixed| SlotValue::Box(fixed.into_box_value()))
    });

    *F::get_mut(&mut self.boxed) = Some(value);
  }
}

impl<N: Scalar, X: StyleExtension> RunFieldVisitor<X> for Sampler<N, X> {
  fn field<F: RunField<X>>(&mut self) {
    let values = readings(
      &self.run_values,
      <F::Value as IntoRunValue<X>>::from_run_value,
      <F::Value as IntoRunValue<X>>::into_run_value,
    );
    let value = self.value(F::KEY, &values, Kind::of_run::<X, F::Value>(), |leaf| {
      leaf.map(|fixed| SlotValue::Run(fixed.into_run_value()))
    });

    *F::get_mut(&mut self.run) = Some(value);
  }
}

/// Builds a keyframe of a sampled style's static leaves: each property's `index`-th,
/// counted round its value.
struct KeyframeOf<'a, N: Scalar, X: StyleExtension> {
  boxed: &'a BoxStyle<N, Static<Condition<N>>, X>,
  run: &'a RunStyle<Static<Condition<N>>, X>,
  index: usize,
  box_frame: BoxStyle<N, Keyframe, X>,
  run_frame: RunStyle<Keyframe, X>,
}

impl<N: Scalar, X: StyleExtension> KeyframeOf<'_, N, X> {
  /// The `index`-th static leaf of `value`, counted round it.
  fn leaf<T: Clone>(index: usize, value: &StaticValue<T, Condition<N>>) -> StaticLeaf<T> {
    let leaves: Vec<_> = value
      .leaves()
      .filter_map(|(_, leaf)| match leaf {
        KeptLeaf::Static(leaf) => Some(leaf),
        KeptLeaf::Slot(_) => None,
      })
      .collect();

    leaves[index % leaves.len()].clone()
  }
}

impl<N: Scalar, X: StyleExtension> BoxFieldVisitor<N, X> for KeyframeOf<'_, N, X> {
  fn field<F: BoxField<N, X>>(&mut self) {
    *F::get_mut(&mut self.box_frame) = F::get(self.boxed)
      .as_ref()
      .map(|value| Self::leaf(self.index, value));
  }
}

impl<N: Scalar, X: StyleExtension> RunFieldVisitor<X> for KeyframeOf<'_, N, X> {
  fn field<F: RunField<X>>(&mut self) {
    *F::get_mut(&mut self.run_frame) = F::get(self.run)
      .as_ref()
      .map(|value| Self::leaf(self.index, value));
  }
}

/// The properties a style's visitors visit, the box's then the run's, in order.
struct Keys<P> {
  boxed: Vec<PropertyKey<P>>,
  run: Vec<PropertyKey<P>>,
}

impl<P: Copy> Keys<P> {
  /// The properties of a style under the extension `X`.
  fn of<N: Scalar, X: StyleExtension<Property = P>>() -> Self {
    let mut keys = Self {
      boxed: Vec::new(),
      run: Vec::new(),
    };

    BoxStyle::<N, Plain, X>::for_each_field(&mut keys);
    RunStyle::<Plain, X>::for_each_field(&mut keys);

    keys
  }

  /// The shared properties among them, in order.
  fn shared(&self) -> Vec<Property> {
    self
      .boxed
      .iter()
      .chain(&self.run)
      .filter_map(|key| match key {
        PropertyKey::Shared(property) => Some(*property),
        PropertyKey::Extension(_) => None,
      })
      .collect()
  }

  /// The medium's own properties among them, in order.
  fn extension(&self) -> Vec<P> {
    self
      .boxed
      .iter()
      .chain(&self.run)
      .filter_map(|key| match key {
        PropertyKey::Shared(_) => None,
        PropertyKey::Extension(property) => Some(*property),
      })
      .collect()
  }
}

impl<N: Scalar, X: StyleExtension> BoxFieldVisitor<N, X> for Keys<X::Property> {
  fn field<F: BoxField<N, X>>(&mut self) {
    self.boxed.push(F::KEY);
  }
}

impl<X: StyleExtension> RunFieldVisitor<X> for Keys<X::Property> {
  fn field<F: RunField<X>>(&mut self) {
    self.run.push(F::KEY);
  }
}

/// Collects the properties a resolved style sets.
struct SetKeys<'a, P>(&'a mut Vec<PropertyKey<P>>);

impl<N: Scalar, X: StyleExtension> BoxVisitor<N, Plain, X> for SetKeys<'_, X::Property> {
  fn value<T: IntoBoxValue<N, X>>(&mut self, key: PropertyKey<X::Property>, value: &Option<T>) {
    if value.is_some() {
      self.0.push(key);
    }
  }
}

impl<X: StyleExtension> RunVisitor<Plain, X> for SetKeys<'_, X::Property> {
  fn value<T: IntoRunValue<X>>(&mut self, key: PropertyKey<X::Property>, value: &Option<T>) {
    if value.is_some() {
      self.0.push(key);
    }
  }
}

/// What a sampled style's values hold: their conditions, expressions, tokens and leaf
/// kinds.
struct Seen<N> {
  conditions: Vec<Condition<N>>,
  nodes: Vec<ExprNode>,
  tokens: Vec<TokenName>,
  kinds: Vec<&'static str>,
}

impl<N> Default for Seen<N> {
  fn default() -> Self {
    Self {
      conditions: Vec::new(),
      nodes: Vec::new(),
      tokens: Vec::new(),
      kinds: Vec::new(),
    }
  }
}

impl<N: Scalar> Seen<N> {
  /// Notes a value's conditions and leaves.
  fn note<T: Debug>(&mut self, value: Option<&StaticValue<T, Condition<N>>>) {
    for (condition, leaf) in value.into_iter().flat_map(Conditional::leaves) {
      self.conditions.extend(condition.cloned());
      self.kinds.push(leaf.kind());

      if let KeptLeaf::Static(leaf) = leaf {
        self.leaf(leaf);
      }
    }
  }

  /// Notes a static leaf's kind, and its token or expression.
  fn leaf<T: Debug>(&mut self, leaf: &StaticLeaf<T>) {
    self.kinds.push(leaf.kind());

    match leaf {
      StaticLeaf::Fixed(_) => {}
      StaticLeaf::Token(name) => self.tokens.push(*name),
      StaticLeaf::Expr(expr) => self.nodes.push(expr.node().clone()),
    }
  }
}

impl<N: Scalar, X: StyleExtension> BoxVisitor<N, Static<Condition<N>>, X> for Seen<N> {
  fn value<T: IntoBoxValue<N, X>>(
    &mut self,
    _: PropertyKey<X::Property>,
    value: &Option<StaticValue<T, Condition<N>>>,
  ) {
    self.note(value.as_ref());
  }
}

impl<N: Scalar, X: StyleExtension> RunVisitor<Static<Condition<N>>, X> for Seen<N> {
  fn value<T: IntoRunValue<X>>(
    &mut self,
    _: PropertyKey<X::Property>,
    value: &Option<StaticValue<T, Condition<N>>>,
  ) {
    self.note(value.as_ref());
  }
}

/// One reading of a style: the environment, the named containers and the interaction it
/// resolves under, and the font size.
struct Scenario<N> {
  environment: Environment<N>,
  containers: Vec<(ContainerName, Option<Size<N>>)>,
  interaction: Interaction,
  font_size: f32,
}

impl<N: Scalar> Scenario<N> {
  /// Two environments, each with its container's size, under two interactions: every
  /// sampled condition holds in some reading and fails in another.
  fn all() -> Vec<Self> {
    let length = N::from_f32;
    let wide = Environment {
      viewport: Size::new(length(1000.), length(800.)),
      scheme: ColorScheme::Dark,
      reduced_motion: true,
      reduced_transparency: true,
      high_contrast: true,
      pointer: PointerPrecision::Fine,
      hover: true,
      safe_area: Inset {
        start: length(1.),
        top: length(2.),
        end: length(3.),
        bottom: length(4.),
      },
    };
    let narrow = Environment {
      viewport: Size::new(length(300.), length(200.)),
      scheme: ColorScheme::Light,
      reduced_motion: false,
      reduced_transparency: false,
      high_contrast: false,
      pointer: PointerPrecision::Coarse,
      hover: false,
      safe_area: Inset::ZERO,
    };
    let engaged = Interaction {
      hovered: true,
      focus: Focus::Shown,
      pressed: true,
      dragged: true,
      targeted: true,
      control: Some(ControlState {
        checked: CheckState::On,
        disabled: true,
        invalid: true,
        open: true,
      }),
    };
    let idle = Interaction {
      control: Some(ControlState {
        checked: CheckState::Mixed,
        ..ControlState::DEFAULT
      }),
      ..Interaction::default()
    };

    [(wide, 300., 16.), (narrow, 100., 12.)]
      .into_iter()
      .flat_map(|(environment, container, font_size)| {
        [engaged, idle].map(|interaction| Self {
          environment,
          containers: vec![(CARD, Some(Size::new(length(container), length(container))))],
          interaction,
          font_size,
        })
      })
      .collect()
  }

  /// Runs `read` with a resolver for this reading: a token resolves through its
  /// definition among `definitions`, and a slot to its leaf among `slots`.
  fn with_resolver<X: StyleExtension>(
    &self,
    definitions: &[(TokenName, TokenValue<N>)],
    slots: &[SlotLeaf<N, X>],
    read: impl FnOnce(&Resolver<'_, N, X>),
  ) {
    let defined_read = Cell::new(false);
    let no_token = |_: TokenName| None::<Resolved<N>>;
    let no_slot = |_: Slot| None::<SlotLeaf<N, X>>;
    let defining = Resolver::<N, X> {
      environment: self.environment,
      interaction: Interaction::default(),
      interaction_read: &defined_read,
      containers: &self.containers,
      font_size: self.font_size,
      token: &no_token,
      slot: &no_slot,
    };
    let token = |name: TokenName| {
      definitions
        .iter()
        .find(|(defined, _)| *defined == name)
        .and_then(|(_, definition)| defining.definition(definition))
    };
    let slot = |slot: Slot| slots.get(usize::from(slot.0)).cloned();
    let interaction_read = Cell::new(false);

    read(&Resolver {
      environment: self.environment,
      interaction: self.interaction,
      interaction_read: &interaction_read,
      containers: &self.containers,
      font_size: self.font_size,
      token: &token,
      slot: &slot,
    });
  }
}

/// A value's fingerprint, which compares lengths by their bits.
fn fingerprint(value: &impl Fingerprint) -> u64 {
  let mut hasher = FxHasher::default();

  value.fingerprint(&mut hasher);

  hasher.finish()
}

/// Asserts that `record`'s resolution and `typed`'s are the same, by their fingerprints.
fn assert_same<T: Fingerprint + Debug>(record: &T, typed: &T, what: &str) {
  assert_eq!(
    fingerprint(record),
    fingerprint(typed),
    "{what}: {record:#?} through the record, {typed:#?} typed"
  );
}
