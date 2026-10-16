//! A property's value ([`Value`]): a leaf (fixed, a token, or derived from signals) and the
//! leaves that replace it under conditions, the last holding condition winning; and the types
//! the properties hold.
//!
//! Conditions nest one level, the same on every platform: a derived base leaf's value brings
//! its own conditions, before the property's; a derived leaf under a condition, as `when`'s
//! other value, is its value's base alone.
//!
//! Every setter takes `impl Into<Value<T>>`, so a fixed value, a token and a conditional value
//! are the same call: `set_padding(16)`, `set_padding(16.5)`, `set_padding(Palette::GAP)`,
//! `set_padding(16.when(Media::MinWidth(900), 24))`. Integer lengths are `i32`, so `-4` is a
//! length too.

use std::{fmt, rc::Rc};

use crate::{Color, Condition, Environment, State, StateSet, TokenRef, TokenValue, Tokens};

/// A type a property holds as a fixed value.
pub trait Property: Clone + 'static {
  /// This type from a token's value, where a token of that kind stands for it.
  #[must_use]
  fn from_token(value: &TokenValue) -> Option<Self> {
    let _ = value;

    None
  }
}

impl Property for Color {
  #[inline(always)]
  fn from_token(value: &TokenValue) -> Option<Self> {
    match value {
      TokenValue::Color(color) => Some(*color),
      TokenValue::Number(_) => None,
    }
  }
}

impl Property for f32 {
  #[inline(always)]
  fn from_token(value: &TokenValue) -> Option<Self> {
    match value {
      TokenValue::Number(number) => Some(*number),
      TokenValue::Color(_) => None,
    }
  }
}

impl Property for u32 {
  #[inline(always)]
  fn from_token(value: &TokenValue) -> Option<Self> {
    match value {
      TokenValue::Number(number) => Some(number.round().max(0.) as Self),
      TokenValue::Color(_) => None,
    }
  }
}

impl Property for bool {}

/// A number of pixels: an integer (`i32`, so `-4` is one) or a float.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a length in pixels",
  label = "not a length here",
  note = "lengths are plain numbers of pixels: `16`, `-4` or `16.5` (an `i32` or an `f32`)"
)]
pub trait IntoUnit<L>: Copy {
  /// This number in the unit.
  fn into_unit(self) -> L;
}

impl IntoUnit<Self> for f32 {
  #[inline(always)]
  fn into_unit(self) -> Self {
    self
  }
}

impl IntoUnit<f32> for i32 {
  #[inline(always)]
  fn into_unit(self) -> f32 {
    self as f32
  }
}

/// A leaf of a [`Value`].
pub enum Leaf<T> {
  /// A fixed value.
  Fixed(T),
  /// The token of this name, resolved where the property is read.
  Token(&'static str),
  /// The value the closure returns, recomputed whenever a signal it reads changes.
  Derived(Rc<dyn Fn() -> Value<T>>),
}

impl<T: Clone> Clone for Leaf<T> {
  fn clone(&self) -> Self {
    match self {
      Self::Fixed(value) => Self::Fixed(value.clone()),
      Self::Token(name) => Self::Token(name),
      Self::Derived(derive) => Self::Derived(Rc::clone(derive)),
    }
  }
}

impl<T: fmt::Debug> fmt::Debug for Leaf<T> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Fixed(value) => write!(formatter, "{value:?}"),
      Self::Token(name) => write!(formatter, "token {name:?}"),
      Self::Derived(_) => write!(formatter, "derived"),
    }
  }
}

impl<T: 'static> Leaf<T> {
  /// This leaf with its value `map` turns into another type's: a fixed value mapped, a token
  /// kept by name, a derived value mapped each time it is recomputed.
  pub fn map<U: 'static>(self, map: impl Fn(T) -> U + Clone + 'static) -> Leaf<U> {
    match self {
      Self::Fixed(value) => Leaf::Fixed(map(value)),
      Self::Token(name) => Leaf::Token(name),
      Self::Derived(derive) => Leaf::Derived(Rc::new(move || derive().map(map.clone()))),
    }
  }
}

impl<T: Property> Leaf<T> {
  /// The value this leaf holds now, a derived leaf's value without its conditions,
  /// subscribing the running effect to what it reads; `None` where a token is defined nowhere
  /// above.
  #[must_use]
  pub fn resolve(&self, tokens: &Tokens) -> Option<T> {
    match self {
      Self::Fixed(value) => Some(value.clone()),
      Self::Token(name) => tokens.resolve(name).and_then(|value| T::from_token(&value)),
      Self::Derived(derive) => derive().base.resolve(tokens),
    }
  }
}

/// A property's value: a leaf, and the leaves that replace it while conditions hold, the last
/// holding condition winning.
#[must_use]
pub struct Value<T> {
  base: Leaf<T>,
  variants: Vec<(Condition, Leaf<T>)>,
}

impl<T: Clone> Clone for Value<T> {
  fn clone(&self) -> Self {
    Self {
      base: self.base.clone(),
      variants: self.variants.clone(),
    }
  }
}

impl<T: fmt::Debug> fmt::Debug for Value<T> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(formatter, "{:?}", self.base)?;

    for (condition, leaf) in &self.variants {
      write!(formatter, ", {leaf:?} when {condition:?}")?;
    }

    Ok(())
  }
}

impl<T> Value<T> {
  /// A value of one leaf.
  #[inline(always)]
  pub const fn leaf(base: Leaf<T>) -> Self {
    Self {
      base,
      variants: Vec::new(),
    }
  }

  /// The value `derive` returns, recomputed whenever a signal it reads changes.
  pub fn derived<V: Into<Self>>(derive: impl Fn() -> V + 'static) -> Self {
    Self::leaf(Leaf::Derived(Rc::new(move || derive().into())))
  }

  /// This value, replaced by `other`'s leaf while `condition` holds.
  pub fn when(mut self, condition: impl Into<Condition>, other: impl Into<Self>) -> Self {
    self.variants.push((condition.into(), other.into().base));

    self
  }

  /// The leaf in force where no condition holds.
  #[inline(always)]
  pub const fn base(&self) -> &Leaf<T> {
    &self.base
  }

  /// Each condition with the leaf it puts in force, in the order written.
  #[inline(always)]
  #[must_use]
  pub fn variants(&self) -> &[(Condition, Leaf<T>)] {
    &self.variants
  }
}

impl<T: 'static> Value<T> {
  /// The states this value's conditions name, a derived base's own as it reads now (without
  /// subscribing): what a realization that does not keep states reports where the value is
  /// used.
  #[must_use]
  pub fn states(&self) -> StateSet {
    self.states_at(0)
  }

  /// Whether it changes with a node's state or a signal: a leaf derived, or a condition on a
  /// state.
  #[must_use]
  pub fn is_dynamic(&self) -> bool {
    matches!(self.base, Leaf::Derived(_))
      || self.variants.iter().any(|(condition, leaf)| {
        matches!(condition, Condition::State(_)) || matches!(leaf, Leaf::Derived(_))
      })
  }

  fn states_at(&self, depth: u8) -> StateSet {
    let mut states = match &self.base {
      Leaf::Derived(derive) if depth < 8 => crate::untrack(|| derive()).states_at(depth + 1),
      _ => StateSet::EMPTY,
    };

    for (condition, _) in &self.variants {
      if let Condition::State(state) = condition {
        states = states.with(*state);
      }
    }

    states
  }

  /// This value with each of its leaves' values `map` turns into another type's, the
  /// conditions kept: what a view does to hand a property of its own type to another.
  pub fn map<U: 'static>(self, map: impl Fn(T) -> U + Clone + 'static) -> Value<U> {
    Value {
      base: self.base.map(map.clone()),
      variants: self
        .variants
        .into_iter()
        .map(|(condition, leaf)| (condition, leaf.map(map.clone())))
        .collect(),
    }
  }
}

impl<T: Property> Value<T> {
  /// The value in force now against this thread's [`Environment`], every state taken as not
  /// holding; `None` where a token is defined nowhere above.
  #[must_use]
  pub fn resolve(&self, tokens: &Tokens) -> Option<T> {
    self.resolve_with(tokens, &|_| false)
  }

  /// The value in force now, `state` answering for the node's states, subscribing the running
  /// effect to everything it read.
  #[must_use]
  pub fn resolve_with(&self, tokens: &Tokens, state: &dyn Fn(State) -> bool) -> Option<T> {
    let environment = Environment::current();
    let derived = match &self.base {
      Leaf::Derived(derive) => Some(derive()),
      _ => None,
    };
    let (mut leaf, inner) = match &derived {
      Some(value) => (&value.base, value.variants.as_slice()),
      None => (&self.base, &[][..]),
    };

    for (condition, variant) in inner.iter().chain(&self.variants) {
      let holds = match condition {
        Condition::Media(media) => environment.holds(media),
        Condition::State(node) => state(*node),
      };

      if holds {
        leaf = variant;
      }
    }

    leaf.resolve(tokens)
  }
}

/// A condition riding any value: `16.when(Media::MinWidth(900), 24)`, the property's type
/// taken from the setter the value is passed to.
pub trait When: Sized {
  /// This value, replaced by `other` while `condition` holds.
  fn when<T>(self, condition: impl Into<Condition>, other: impl Into<Value<T>>) -> Value<T>
  where
    Value<T>: From<Self>,
  {
    Value::from(self).when(condition, other)
  }
}

impl<X> When for X {}

impl<T: Property> From<T> for Value<T> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::leaf(Leaf::Fixed(value))
  }
}

impl<T: Property> From<TokenRef<T>> for Value<T> {
  #[inline(always)]
  fn from(token: TokenRef<T>) -> Self {
    Self::leaf(Leaf::Token(token.name()))
  }
}

/// An integer is a length in pixels.
impl From<i32> for Value<f32> {
  #[inline(always)]
  fn from(length: i32) -> Self {
    Self::leaf(Leaf::Fixed(length as f32))
  }
}

/// An integer is a count, a negative one none.
impl From<i32> for Value<u32> {
  #[inline(always)]
  fn from(count: i32) -> Self {
    Self::leaf(Leaf::Fixed(count.max(0) as u32))
  }
}

/// A value made of lengths takes a length, an integer one and a length token alike.
macro_rules! from_lengths {
  ($($ty:ty),*) => {$(
    /// An integer number of pixels.
    impl From<i32> for Value<$ty> {
      #[inline(always)]
      fn from(length: i32) -> Self {
        Self::leaf(Leaf::Fixed(<$ty>::from(length as f32)))
      }
    }

    /// An integer number of pixels.
    impl From<i32> for $ty {
      #[inline(always)]
      fn from(length: i32) -> Self {
        <$ty>::from(length as f32)
      }
    }

    /// A number of pixels.
    impl From<f32> for Value<$ty> {
      #[inline(always)]
      fn from(length: f32) -> Self {
        Self::leaf(Leaf::Fixed(<$ty>::from(length)))
      }
    }

    /// A length token.
    impl From<TokenRef<f32>> for Value<$ty> {
      #[inline(always)]
      fn from(token: TokenRef<f32>) -> Self {
        Self::leaf(Leaf::Token(token.name()))
      }
    }
  )*};
}

from_lengths!(Inset, Spacing, Corners, Dimension);

/// Space inside a box's edges, in pixels, by side.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub struct Inset {
  /// The top edge.
  pub top: f32,
  /// The trailing edge: the right in a left-to-right layout.
  pub trailing: f32,
  /// The bottom edge.
  pub bottom: f32,
  /// The leading edge: the left in a left-to-right layout.
  pub leading: f32,
}

impl Inset {
  /// No space.
  pub const ZERO: Self = Self {
    top: 0.,
    trailing: 0.,
    bottom: 0.,
    leading: 0.,
  };

  /// Each side's length.
  #[inline(always)]
  pub fn new(
    top: impl IntoUnit<f32>,
    trailing: impl IntoUnit<f32>,
    bottom: impl IntoUnit<f32>,
    leading: impl IntoUnit<f32>,
  ) -> Self {
    Self {
      top: top.into_unit(),
      trailing: trailing.into_unit(),
      bottom: bottom.into_unit(),
      leading: leading.into_unit(),
    }
  }

  /// The same length on every side.
  #[inline(always)]
  pub fn all(length: impl IntoUnit<f32>) -> Self {
    let length = length.into_unit();

    Self::new(length, length, length, length)
  }

  /// `horizontal` on the leading and trailing sides, `vertical` on the top and bottom.
  #[inline(always)]
  pub fn symmetric(horizontal: impl IntoUnit<f32>, vertical: impl IntoUnit<f32>) -> Self {
    let (horizontal, vertical) = (horizontal.into_unit(), vertical.into_unit());

    Self::new(vertical, horizontal, vertical, horizontal)
  }
}

impl From<f32> for Inset {
  #[inline(always)]
  fn from(length: f32) -> Self {
    Self::new(length, length, length, length)
  }
}

/// A length token stands for an inset of its length on every side.
impl Property for Inset {
  #[inline(always)]
  fn from_token(value: &TokenValue) -> Option<Self> {
    f32::from_token(value).map(Self::from)
  }
}

/// A length along an axis: automatic, in pixels, or a ratio of the parent.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Dimension {
  /// Sized by the content and the layout.
  #[default]
  Auto,
  /// A length in pixels.
  Length(f32),
  /// A ratio of the parent's size along the same axis: `0.5` is half, not clamped.
  Fraction(f32),
}

impl Dimension {
  /// A length in pixels: an integer or a float.
  #[inline(always)]
  pub fn length(length: impl IntoUnit<f32>) -> Self {
    Self::Length(length.into_unit())
  }
}

impl From<f32> for Dimension {
  #[inline(always)]
  fn from(length: f32) -> Self {
    Self::Length(length)
  }
}

/// A length token stands for a dimension of that length.
impl Property for Dimension {
  #[inline(always)]
  fn from_token(value: &TokenValue) -> Option<Self> {
    f32::from_token(value).map(Self::Length)
  }
}

/// Space between neighbors, per axis.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub struct Spacing {
  /// Between neighbors side by side.
  pub horizontal: f32,
  /// Between neighbors one above the other.
  pub vertical: f32,
}

impl Spacing {
  /// `horizontal` between neighbors side by side, `vertical` between those one above the
  /// other.
  #[inline(always)]
  pub fn new(horizontal: impl IntoUnit<f32>, vertical: impl IntoUnit<f32>) -> Self {
    Self {
      horizontal: horizontal.into_unit(),
      vertical: vertical.into_unit(),
    }
  }
}

impl From<f32> for Spacing {
  #[inline(always)]
  fn from(length: f32) -> Self {
    Self::new(length, length)
  }
}

/// A length token stands for the same spacing on both axes.
impl Property for Spacing {
  #[inline(always)]
  fn from_token(value: &TokenValue) -> Option<Self> {
    f32::from_token(value).map(Self::from)
  }
}

/// The radius of each corner, in pixels.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[must_use]
pub struct Corners {
  /// The top leading corner.
  pub top_leading: f32,
  /// The top trailing corner.
  pub top_trailing: f32,
  /// The bottom trailing corner.
  pub bottom_trailing: f32,
  /// The bottom leading corner.
  pub bottom_leading: f32,
}

impl Corners {
  /// The same radius at every corner.
  #[inline(always)]
  pub fn all(radius: impl IntoUnit<f32>) -> Self {
    let radius = radius.into_unit();

    Self {
      top_leading: radius,
      top_trailing: radius,
      bottom_trailing: radius,
      bottom_leading: radius,
    }
  }
}

impl From<f32> for Corners {
  #[inline(always)]
  fn from(radius: f32) -> Self {
    Self::all(radius)
  }
}

/// A length token stands for the same radius at every corner.
impl Property for Corners {
  #[inline(always)]
  fn from_token(value: &TokenValue) -> Option<Self> {
    f32::from_token(value).map(Self::all)
  }
}

/// How a layout distributes its children, or its lines, along an axis.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Arrangement {
  /// Packed at the start.
  #[default]
  Start,
  /// Packed in the middle.
  Center,
  /// Packed at the end.
  End,
  /// The first at the start, the last at the end, the rest evenly between.
  SpaceBetween,
  /// Equal space around each.
  SpaceAround,
  /// Equal space between each and at both ends.
  SpaceEvenly,
}

impl Property for Arrangement {}

/// Where an item sits along one axis of the room its layout gives it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum AxisAlignment {
  /// At the start.
  Start,
  /// Centered.
  Center,
  /// At the end.
  End,
  /// Stretched across.
  #[default]
  Stretch,
  /// On the first baseline shared with its siblings.
  Baseline,
}

impl Property for AxisAlignment {}

/// Where an item sits on both axes: a grid's or a stack's cell, a container's child.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[must_use]
pub struct Alignment {
  /// Along the horizontal axis; leading is the start.
  pub horizontal: AxisAlignment,
  /// Along the vertical axis.
  pub vertical: AxisAlignment,
}

impl Alignment {
  /// Centered on both axes.
  pub const CENTER: Self = Self::new(AxisAlignment::Center, AxisAlignment::Center);
  /// At the top leading corner.
  pub const TOP_LEADING: Self = Self::new(AxisAlignment::Start, AxisAlignment::Start);
  /// Stretched on both axes.
  pub const STRETCH: Self = Self::new(AxisAlignment::Stretch, AxisAlignment::Stretch);

  /// `horizontal` and `vertical`.
  #[inline(always)]
  pub const fn new(horizontal: AxisAlignment, vertical: AxisAlignment) -> Self {
    Self {
      horizontal,
      vertical,
    }
  }
}

impl Property for Alignment {}

/// A grid track's size.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GridTrack {
  /// A length in pixels.
  Length(f32),
  /// A ratio of the grid's size.
  Fraction(f32),
  /// A weight of the space left once the other tracks are sized.
  Flex(f32),
  /// Sized by the content.
  Auto,
}

impl GridTrack {
  /// A track of `length` pixels: an integer or a float.
  #[inline(always)]
  pub fn length(length: impl IntoUnit<f32>) -> Self {
    Self::Length(length.into_unit())
  }
}

/// A list of grid tracks.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Tracks(pub Rc<[GridTrack]>);

impl<const N: usize> From<[GridTrack; N]> for Tracks {
  #[inline(always)]
  fn from(tracks: [GridTrack; N]) -> Self {
    Self(Rc::from(tracks))
  }
}

impl From<Vec<GridTrack>> for Tracks {
  #[inline(always)]
  fn from(tracks: Vec<GridTrack>) -> Self {
    Self(Rc::from(tracks))
  }
}

impl<const N: usize> From<[GridTrack; N]> for Value<Tracks> {
  #[inline(always)]
  fn from(tracks: [GridTrack; N]) -> Self {
    Self::leaf(Leaf::Fixed(Tracks::from(tracks)))
  }
}

impl Property for Tracks {}

/// The order a grid places children it was not told where to put.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Flow {
  /// Row by row.
  #[default]
  Rows,
  /// Column by column.
  Columns,
}

impl Property for Flow {}

/// How a child of a row or a column grows and shrinks: `Flexible`'s value.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub struct Flexibility {
  /// Its weight in sharing the space left over.
  pub grow: f32,
  /// Its weight in giving up space when there is too little.
  pub shrink: f32,
  /// Its size before growing or shrinking.
  pub initial: Dimension,
}

impl Flexibility {
  /// Growing with weight `grow` from nothing, and shrinking: a share of the space by weight.
  #[inline(always)]
  pub const fn grow(grow: f32) -> Self {
    Self {
      grow,
      shrink: 1.,
      initial: Dimension::Length(0.),
    }
  }
}

/// A weight: `Flexibility::grow(weight)`.
impl From<f32> for Value<Flexibility> {
  #[inline(always)]
  fn from(weight: f32) -> Self {
    Self::leaf(Leaf::Fixed(Flexibility::grow(weight)))
  }
}

/// A weight: `Flexibility::grow(weight)`.
impl From<i32> for Value<Flexibility> {
  #[inline(always)]
  fn from(weight: i32) -> Self {
    Self::leaf(Leaf::Fixed(Flexibility::grow(weight as f32)))
  }
}

impl Property for Flexibility {}

/// The edges a pinned child keeps its distance to, in pixels; `None` leaves an edge free.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Edges {
  /// The top edge.
  pub top: Option<f32>,
  /// The trailing edge.
  pub trailing: Option<f32>,
  /// The bottom edge.
  pub bottom: Option<f32>,
  /// The leading edge.
  pub leading: Option<f32>,
}

impl Edges {
  /// Every edge at `distance`: the child covers its stack, inset by it.
  #[inline(always)]
  pub fn all(distance: impl IntoUnit<f32>) -> Self {
    let distance = Some(distance.into_unit());

    Self {
      top: distance,
      trailing: distance,
      bottom: distance,
      leading: distance,
    }
  }
}

impl Property for Edges {}

/// A placement on one axis of a grid: a start line counted from 1, or the next free one, and
/// a span.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GridLine {
  /// The line the child starts at, or `None` to be placed automatically.
  pub start: Option<i16>,
  /// How many tracks it spans.
  pub span: u16,
}

/// Where `GridCell` puts its child.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub struct GridPlacement {
  /// The row placement.
  pub row: GridLine,
  /// The column placement.
  pub column: GridLine,
}

impl GridPlacement {
  /// The cell at `row` and `column`, counted from 1, one track each way.
  #[inline(always)]
  pub const fn at(row: i16, column: i16) -> Self {
    Self {
      row: GridLine {
        start: Some(row),
        span: 1,
      },
      column: GridLine {
        start: Some(column),
        span: 1,
      },
    }
  }

  /// This placement spanning `rows` and `columns` tracks.
  #[inline(always)]
  pub const fn spanning(mut self, rows: u16, columns: u16) -> Self {
    self.row.span = rows;
    self.column.span = columns;

    self
  }
}

impl Property for GridPlacement {}

/// How a border's line is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum BorderStyle {
  /// One unbroken line.
  #[default]
  Solid,
  /// Dashes.
  Dashed,
  /// Dots.
  Dotted,
  /// Two lines.
  Double,
}

/// A line around a box: it takes room inside the box's edges on every platform, as a CSS
/// border does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Border {
  /// The line's width, in pixels.
  pub width: f32,
  /// How the line is drawn.
  pub style: BorderStyle,
}

impl Border {
  /// A solid line `width` pixels wide, in the box's border color.
  #[inline(always)]
  pub fn new(width: impl IntoUnit<f32>) -> Self {
    Self {
      width: width.into_unit(),
      style: BorderStyle::Solid,
    }
  }
}

impl Property for Border {}

/// A line drawn around a box, outside its edges by its offset (inside where the offset is
/// negative), its corners grown by the offset: it takes no room and is never hit, as a CSS
/// outline. A focus ring is one.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Ring {
  /// The line's width, in pixels.
  pub width: f32,
  /// How far beyond the box's edges the line starts, in pixels; inward where negative.
  pub offset: f32,
}

impl Ring {
  /// No ring.
  pub const NONE: Self = Self {
    width: 0.,
    offset: 0.,
  };

  /// A ring `width` pixels wide, against the box's edges.
  #[inline(always)]
  pub fn new(width: impl IntoUnit<f32>) -> Self {
    Self {
      width: width.into_unit(),
      offset: 0.,
    }
  }

  /// This ring, `offset` pixels beyond the box's edges (inward where negative).
  #[inline(always)]
  pub fn set_offset(mut self, offset: impl IntoUnit<f32>) -> Self {
    self.offset = offset.into_unit();

    self
  }

  /// Whether it draws anything.
  #[inline(always)]
  #[must_use]
  pub const fn is_visible(self) -> bool {
    self.width > 0.
  }

  /// How far beyond the box's edges the ring reaches, in pixels.
  #[inline(always)]
  #[must_use]
  pub const fn reach(self) -> f32 {
    if self.is_visible() {
      (self.offset + self.width).max(0.)
    } else {
      0.
    }
  }
}

impl Property for Ring {}

/// A box drawn moved from where it was laid out, with everything inside it: it takes no room,
/// and where it is hit and found moves with it, as a CSS translation.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Offset {
  /// How far right it moves: pixels, or a fraction of its own width where `of_size`.
  pub x: f32,
  /// How far down it moves: pixels, or a fraction of its own height where `of_size`.
  pub y: f32,
  /// Whether `x` and `y` are fractions of the box's own size.
  pub of_size: bool,
}

impl Offset {
  /// Not moved.
  pub const ZERO: Self = Self {
    x: 0.,
    y: 0.,
    of_size: false,
  };

  /// Moved `x` pixels right and `y` pixels down.
  #[inline(always)]
  pub fn new(x: impl IntoUnit<f32>, y: impl IntoUnit<f32>) -> Self {
    Self {
      x: x.into_unit(),
      y: y.into_unit(),
      of_size: false,
    }
  }

  /// Moved by `x` of its own width right and `y` of its own height down (`1.` its whole
  /// width).
  #[inline(always)]
  pub const fn of_size(x: f32, y: f32) -> Self {
    Self {
      x,
      y,
      of_size: true,
    }
  }

  /// How far a box `width` by `height` pixels moves, in pixels.
  #[inline(always)]
  #[must_use]
  pub const fn in_pixels(self, width: f32, height: f32) -> (f32, f32) {
    if self.of_size {
      (self.x * width, self.y * height)
    } else {
      (self.x, self.y)
    }
  }
}

impl Property for Offset {}

/// One shadow a box casts, or casts inside itself.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shadow {
  /// The horizontal offset, in pixels, trailing positive.
  pub x: f32,
  /// The vertical offset, in pixels, down positive.
  pub y: f32,
  /// The blur radius, in pixels.
  pub blur: f32,
  /// How far the shadow grows beyond the box, in pixels.
  pub spread: f32,
  /// The shadow's color.
  pub color: Color,
  /// Whether the shadow falls inside the box.
  pub inset: bool,
}

impl Shadow {
  /// A shadow offset by `y` pixels down, blurred by `blur`, in `color`.
  #[inline(always)]
  pub fn new(y: impl IntoUnit<f32>, blur: impl IntoUnit<f32>, color: Color) -> Self {
    Self {
      x: 0.,
      y: y.into_unit(),
      blur: blur.into_unit(),
      spread: 0.,
      color,
      inset: false,
    }
  }
}

/// A box's shadows, the first drawn on top.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Shadows(pub Rc<[Shadow]>);

impl From<Shadow> for Shadows {
  #[inline(always)]
  fn from(shadow: Shadow) -> Self {
    Self(Rc::from([shadow]))
  }
}

/// One shadow.
impl From<Shadow> for Value<Shadows> {
  #[inline(always)]
  fn from(shadow: Shadow) -> Self {
    Self::leaf(Leaf::Fixed(Shadows::from(shadow)))
  }
}

impl Property for Shadows {}

/// A font's weight, from 1 to 1000.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FontWeight(pub u16);

impl FontWeight {
  /// 400.
  pub const REGULAR: Self = Self(400);
  /// 500.
  pub const MEDIUM: Self = Self(500);
  /// 600.
  pub const SEMIBOLD: Self = Self(600);
  /// 700.
  pub const BOLD: Self = Self(700);
}

impl Property for FontWeight {}

/// Whether a run is upright or slanted.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum FontStyle {
  /// Upright.
  #[default]
  Normal,
  /// Slanted.
  Italic,
}

impl Property for FontStyle {}

/// The typeface a run is set in.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum FontFamily {
  /// The platform's interface typeface.
  System,
  /// The platform's fixed-width typeface.
  Monospace,
  /// A typeface by name, the platform's own where it has none.
  Named(Rc<str>),
}

impl Property for FontFamily {}

/// The lines a run is decorated with.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct TextDecoration {
  /// A line under the text.
  pub underline: bool,
  /// A line through the text.
  pub line_through: bool,
  /// A line over the text.
  pub overline: bool,
}

impl TextDecoration {
  /// No line.
  pub const NONE: Self = Self {
    underline: false,
    line_through: false,
    overline: false,
  };
  /// A line under the text.
  pub const UNDERLINE: Self = Self {
    underline: true,
    ..Self::NONE
  };
  /// A line through the text.
  pub const LINE_THROUGH: Self = Self {
    line_through: true,
    ..Self::NONE
  };
}

impl Property for TextDecoration {}

/// How a decoration's lines are drawn.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TextDecorationStyle {
  /// One unbroken line.
  #[default]
  Solid,
  /// Two lines.
  Double,
  /// Dots.
  Dotted,
  /// Dashes.
  Dashed,
  /// A wave.
  Wavy,
}

impl Property for TextDecorationStyle {}

/// Where a paragraph's lines sit across its width.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TextAlignment {
  /// At the leading edge.
  #[default]
  Leading,
  /// Centered.
  Center,
  /// At the trailing edge.
  Trailing,
  /// Stretched to both edges, the last line leading.
  Justify,
}

impl Property for TextAlignment {}

/// A transition's easing curve.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Curve {
  /// Constant speed.
  Linear,
  /// Slow at both ends.
  #[default]
  EaseInOut,
  /// Slow at the start.
  EaseIn,
  /// Slow at the end.
  EaseOut,
  /// A cubic Bézier through `(x1, y1)` and `(x2, y2)`.
  Bezier(f32, f32, f32, f32),
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Media;

  fn padding(value: impl Into<Value<Inset>>) -> Value<Inset> {
    value.into()
  }

  fn spacing(value: impl Into<Value<Spacing>>) -> Value<Spacing> {
    value.into()
  }

  #[test]
  fn literals_of_both_kinds_are_lengths() {
    let tokens = Tokens::root();

    assert_eq!(padding(16).resolve(&tokens), Some(Inset::all(16)));
    assert_eq!(padding(-4).resolve(&tokens), Some(Inset::all(-4)));
    assert_eq!(padding(16.5).resolve(&tokens), Some(Inset::all(16.5)));
    assert_eq!(
      padding(Inset::symmetric(16, 8)).resolve(&tokens),
      Some(Inset::symmetric(16., 8.))
    );
  }

  #[test]
  fn the_last_holding_condition_wins() {
    let tokens = Tokens::root();
    let environment = Environment::current();
    let value = spacing(
      8.when(Media::MinWidth(600), 12)
        .when(Media::MinWidth(900), 16),
    );

    environment.width.set(1000.);
    assert_eq!(value.resolve(&tokens), Some(Spacing::from(16.)));
    environment.width.set(700.);
    assert_eq!(value.resolve(&tokens), Some(Spacing::from(12.)));
    environment.width.set(320.);
    assert_eq!(value.resolve(&tokens), Some(Spacing::from(8.)));
  }
}
