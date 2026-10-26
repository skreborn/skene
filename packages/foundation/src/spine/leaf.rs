//! The leaves a property's value is made of, and the conditions that pick between them.

use std::{fmt::Debug, rc::Rc};

use thin_vec::ThinVec;

use super::{
  Condition,
  Expr,
  Media,
  Scalar,
  State,
  Token,
  TokenName,
  TokenRef,
  TokenReferences,
  Tokenable,
};
use crate::spine::{Alignment, Axes, Dimension, Inset, ItemAlignment, Spacing};

/// A value that is data: a literal, a token reference, or an expression.
///
/// What a token's [`Definition`] holds, and the part of a [`Leaf`] a platform can compile
/// ahead of time.
#[derive(Debug, Clone, PartialEq, Hash)]
#[must_use]
pub enum StaticLeaf<T> {
  /// A literal value.
  Fixed(T),
  /// A token, by its name, resolved where the property is realized.
  Token(TokenName),
  /// An expression, evaluated where the property is realized.
  Expr(Expr<T>),
}

impl<T> StaticLeaf<T> {
  /// This leaf with `map` applied to a fixed value; a token or an expression keeps its
  /// name or tree under the mapped type, which `map` must leave a reading of the same
  /// quantity.
  pub(in crate::spine) fn map<U>(self, map: impl FnOnce(T) -> U) -> StaticLeaf<U> {
    match self {
      Self::Fixed(value) => StaticLeaf::Fixed(map(value)),
      Self::Token(name) => StaticLeaf::Token(name),
      Self::Expr(expr) => StaticLeaf::Expr(Expr::from_node(expr.into_node())),
    }
  }

  /// [`map`](Self::map) for a reading that may fail: `None` where `read` refuses a fixed
  /// value.
  pub(in crate::spine) fn try_map<U>(
    self,
    read: impl FnOnce(T) -> Option<U>,
  ) -> Option<StaticLeaf<U>> {
    match self {
      Self::Fixed(value) => read(value).map(StaticLeaf::Fixed),
      Self::Token(name) => Some(StaticLeaf::Token(name)),
      Self::Expr(expr) => Some(StaticLeaf::Expr(Expr::from_node(expr.into_node()))),
    }
  }
}

impl<T> TokenReferences for StaticLeaf<T> {
  fn for_each_token(&self, visit: &mut dyn FnMut(TokenName)) {
    match self {
      Self::Fixed(_) => {}
      Self::Token(name) => visit(*name),
      Self::Expr(expr) => expr.for_each_token(visit),
    }
  }
}

impl<L: TokenReferences, C> TokenReferences for Conditional<L, C> {
  fn for_each_token(&self, visit: &mut dyn FnMut(TokenName)) {
    for (_, leaf) in self.leaves() {
      leaf.for_each_token(visit);
    }
  }
}

/// What a builder stores for a property: a [`StaticLeaf`], or a leaf derived from
/// signals.
#[derive(Clone)]
#[must_use]
pub enum Leaf<T> {
  /// A value known when the view is built.
  Static(StaticLeaf<T>),
  /// A static leaf recomputed whenever the signals it reads change: a value, a token
  /// or an expression, resolved where the property is realized.
  Derived(Rc<dyn Fn() -> StaticLeaf<T>>),
}

impl<T> Leaf<T> {
  /// A leaf recomputed by `compute`, which returns a value, or a token or an expression where its
  /// type takes one.
  pub fn derive<R: Into<StaticLeaf<T>>>(compute: impl Fn() -> R + 'static) -> Self {
    Self::Derived(Rc::new(move || compute().into()))
  }
}

impl<T: PartialEq> PartialEq for Leaf<T> {
  /// Static leaves compare by value; derived leaves compare by identity of the closure.
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Static(mine), Self::Static(theirs)) => mine == theirs,
      (Self::Derived(mine), Self::Derived(theirs)) => Rc::ptr_eq(mine, theirs),
      _ => false,
    }
  }
}

impl<T: Debug> Debug for Leaf<T> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Static(leaf) => formatter.debug_tuple("Static").field(leaf).finish(),
      Self::Derived(_) => formatter.write_str("Derived(..)"),
    }
  }
}

/// A per-node slot a derived leaf writes into: the index of the leaf's entry in the
/// slot list a style's split produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct Slot(pub u16);

/// What a platform receives for a property: a [`StaticLeaf`] to compile, or the slot a
/// derived leaf's render effect writes.
#[derive(Debug, Clone, PartialEq, Hash)]
#[must_use]
pub enum KeptLeaf<T> {
  /// A value the platform compiles ahead of time.
  Static(StaticLeaf<T>),
  /// A value a render effect writes into the slot.
  Slot(Slot),
}

impl<T> KeptLeaf<T> {
  /// This leaf with `map` applied to a fixed value, as [`StaticLeaf::map`] maps it; a
  /// slot stays the slot.
  pub(in crate::spine) fn map<U>(self, map: impl FnOnce(T) -> U) -> KeptLeaf<U> {
    match self {
      Self::Static(leaf) => KeptLeaf::Static(leaf.map(map)),
      Self::Slot(slot) => KeptLeaf::Slot(slot),
    }
  }

  /// [`map`](Self::map) for a reading that may fail: `None` where `read` refuses a fixed
  /// value.
  pub(in crate::spine) fn try_map<U>(
    self,
    read: impl FnOnce(T) -> Option<U>,
  ) -> Option<KeptLeaf<U>> {
    match self {
      Self::Static(leaf) => leaf.try_map(read).map(KeptLeaf::Static),
      Self::Slot(slot) => Some(KeptLeaf::Slot(slot)),
    }
  }
}

impl<T> TokenReferences for KeptLeaf<T> {
  fn for_each_token(&self, visit: &mut dyn FnMut(TokenName)) {
    match self {
      Self::Static(leaf) => leaf.for_each_token(visit),
      Self::Slot(_) => {}
    }
  }
}

/// One derived leaf a style's split lifted out: where it lands, and how to compute it.
#[derive(Clone)]
#[must_use]
pub struct DerivedSlot<K, V> {
  /// The slot the computed value is written into.
  pub slot: Slot,
  /// The property the leaf belongs to.
  pub property: K,
  /// The leaf's place in its property's [`Conditional`]: `0` for the base, `i` for the
  /// `i`-th variant, counted from one.
  pub position: usize,
  /// The computation, tracking the signals it reads: a value, or a token or expression
  /// the platform resolves where it reads the slot.
  pub compute: Rc<dyn Fn() -> StaticLeaf<V>>,
}

impl<K: Debug, V> Debug for DerivedSlot<K, V> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("DerivedSlot")
      .field("slot", &self.slot)
      .field("property", &self.property)
      .field("position", &self.position)
      .finish_non_exhaustive()
  }
}

/// A leaf and the conditions that override it; later variants win over earlier ones.
#[derive(Debug, Clone, PartialEq, Hash)]
#[must_use]
pub struct Conditional<L, C> {
  base: L,
  variants: ThinVec<(C, L)>,
}

/// What a builder collects for one property: a [`Leaf`] under the facade's conditions.
pub type Value<T, C> = Conditional<Leaf<T>, C>;

/// What a token is defined as: a [`StaticLeaf`] under [`Media`] conditions over the
/// facade's scalar.
///
/// No closure can enter one. Only media conditions apply: a token answers the
/// environment, and a node's own state is the consuming property's to condition on.
///
/// ```compile_fail,E0277
/// use skene_foundation::{Color, Definition, Leaf};
///
/// let _ = Definition::<Color, f32>::from(Leaf::derive(|| Color::TRANSPARENT));
/// ```
pub type Definition<T, N> = Conditional<StaticLeaf<T>, Media<N>>;

/// What a platform receives for one property: a [`KeptLeaf`] under the facade's
/// conditions.
pub type StaticValue<T, C> = Conditional<KeptLeaf<T>, C>;

impl<L, C> Conditional<L, C> {
  /// A value with no variants.
  #[inline(always)]
  pub const fn new(base: L) -> Self {
    Self {
      base,
      variants: ThinVec::new(),
    }
  }

  /// This value, overridden by `value` while `condition` holds.
  pub fn when(mut self, condition: C, value: impl Into<L>) -> Self {
    self.variants.push((condition, value.into()));

    self
  }

  /// The leaf applied when no variant's condition holds.
  #[inline(always)]
  pub const fn base(&self) -> &L {
    &self.base
  }

  /// The overriding leaves, in declaration order.
  #[inline(always)]
  pub fn variants(&self) -> &[(C, L)] {
    &self.variants
  }

  /// Every leaf with its condition, the base first with none.
  #[inline(always)]
  pub fn leaves(&self) -> impl Iterator<Item = (Option<&C>, &L)> {
    std::iter::once((None, &self.base)).chain(
      self
        .variants
        .iter()
        .map(|(condition, leaf)| (Some(condition), leaf)),
    )
  }

  /// This value with `map` applied to every leaf, the base first.
  pub fn map<M>(self, mut map: impl FnMut(L) -> M) -> Conditional<M, C> {
    Conditional {
      base: map(self.base),
      variants: self
        .variants
        .into_iter()
        .map(|(condition, leaf)| (condition, map(leaf)))
        .collect(),
    }
  }

  /// [`map`](Self::map) for a reading that may fail: `None` where `read` refuses any
  /// leaf.
  pub(in crate::spine) fn try_map<M>(
    self,
    mut read: impl FnMut(L) -> Option<M>,
  ) -> Option<Conditional<M, C>> {
    Some(Conditional {
      base: read(self.base)?,
      variants: self
        .variants
        .into_iter()
        .map(|(condition, leaf)| Some((condition, read(leaf)?)))
        .collect::<Option<_>>()?,
    })
  }

  /// This value with `map` applied to every condition, in declaration order.
  pub fn map_conditions<M>(self, mut map: impl FnMut(C) -> M) -> Conditional<L, M> {
    Conditional {
      base: self.base,
      variants: self
        .variants
        .into_iter()
        .map(|(condition, leaf)| (map(condition), leaf))
        .collect(),
    }
  }
}

impl<T, C> Value<T, C> {
  /// This value, overridden while `condition` holds by a leaf derived from the signals
  /// `compute` reads: a value, or a token or an expression where its type takes one.
  pub fn derive_when<R: Into<StaticLeaf<T>>>(
    self,
    condition: C,
    compute: impl Fn() -> R + 'static,
  ) -> Self {
    self.when(condition, Leaf::derive(compute))
  }

  /// Whether no leaf is derived.
  #[inline(always)]
  pub fn is_static(&self) -> bool {
    self
      .leaves()
      .all(|(_, leaf)| matches!(leaf, Leaf::Static(_)))
  }

  /// This value with every derived leaf replaced by the slot `allocate` hands out for
  /// it, given the leaf's position (`0` the base, `i` the `i`-th variant) and its
  /// computation.
  pub fn split_derived(
    self,
    mut allocate: impl FnMut(usize, Rc<dyn Fn() -> StaticLeaf<T>>) -> Slot,
  ) -> StaticValue<T, C> {
    let mut position = 0;

    self.map(|leaf| {
      let leaf = match leaf {
        Leaf::Static(leaf) => KeptLeaf::Static(leaf),
        Leaf::Derived(compute) => KeptLeaf::Slot(allocate(position, compute)),
      };

      position += 1;

      leaf
    })
  }
}

impl<C: Clone> Value<Alignment, C> {
  /// The value's horizontal and vertical axes, each leaf projected onto its axis and a
  /// token read by its name on both.
  pub fn split_axes(&self) -> (Value<ItemAlignment, C>, Value<ItemAlignment, C>) {
    (
      self.axis(|alignment| alignment.horizontal),
      self.axis(|alignment| alignment.vertical),
    )
  }

  /// One axis of the value: each leaf projected through `pick`.
  fn axis(&self, pick: fn(Alignment) -> ItemAlignment) -> Value<ItemAlignment, C> {
    self.clone().map(|leaf| match leaf {
      Leaf::Static(leaf) => Leaf::Static(alignment_axis(&leaf, pick)),
      Leaf::Derived(compute) => Leaf::Derived(Rc::new(move || alignment_axis(&compute(), pick))),
    })
  }
}

/// One axis of a two-axis alignment leaf: a fixed alignment projected through `pick`, a
/// token read by its name.
fn alignment_axis(
  leaf: &StaticLeaf<Alignment>,
  pick: fn(Alignment) -> ItemAlignment,
) -> StaticLeaf<ItemAlignment> {
  match leaf {
    StaticLeaf::Fixed(alignment) => StaticLeaf::Fixed(pick(*alignment)),
    StaticLeaf::Token(name) => StaticLeaf::Token(*name),
    StaticLeaf::Expr(_) => unreachable!("no expression has an alignment type"),
  }
}

/// Conditions a plain value, producing a [`Value`] directly:
/// `Color::WHITE.when(State::Hovered, Color::BLACK)`.
///
/// Every plain value has it; a [`TokenRef`], [`Token`] and [`Expr`] carry the same
/// `when` as an inherent method. The condition fixes the value's condition type, and a
/// value under [`Media`] or [`State`] alone converts into one under [`Condition`], so
/// `8f32.when(Media::min_width(900), 16.)` is accepted where a
/// `Value<f32, Condition<N>>` is.
pub trait When<T>: Into<Leaf<T>> {
  /// This value, overridden by `value` while `condition` holds.
  fn when<C>(self, condition: C, value: impl Into<Leaf<T>>) -> Value<T, C> {
    Conditional::new(self.into()).when(condition, value)
  }

  /// This value, overridden while `condition` holds by a leaf derived from the
  /// signals `compute` reads: a value, or a token or an expression where its type takes one.
  fn derive_when<C, R: Into<StaticLeaf<T>>>(
    self,
    condition: C,
    compute: impl Fn() -> R + 'static,
  ) -> Value<T, C> {
    Conditional::new(self.into()).derive_when(condition, compute)
  }
}

impl<T, L: Into<Leaf<T>>> When<T> for L {}

impl<T> From<T> for StaticLeaf<T> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::Fixed(value)
  }
}

impl<T: Tokenable> From<TokenRef<T>> for StaticLeaf<T> {
  #[inline(always)]
  fn from(reference: TokenRef<T>) -> Self {
    Self::Token(reference.name())
  }
}

impl<T: Tokenable, N: Scalar> From<&Token<T, N>> for StaticLeaf<T> {
  #[inline(always)]
  fn from(token: &Token<T, N>) -> Self {
    Self::Token(token.name())
  }
}

impl<T: Tokenable, N: Scalar> From<Token<T, N>> for StaticLeaf<T> {
  #[inline(always)]
  fn from(token: Token<T, N>) -> Self {
    Self::Token(token.name())
  }
}

impl<T> From<Expr<T>> for StaticLeaf<T> {
  #[inline(always)]
  fn from(expr: Expr<T>) -> Self {
    Self::Expr(expr)
  }
}

impl<T> From<T> for Leaf<T> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::Static(StaticLeaf::Fixed(value))
  }
}

impl<T: Tokenable> From<TokenRef<T>> for Leaf<T> {
  #[inline(always)]
  fn from(reference: TokenRef<T>) -> Self {
    Self::Static(StaticLeaf::from(reference))
  }
}

impl<T: Tokenable, N: Scalar> From<&Token<T, N>> for Leaf<T> {
  #[inline(always)]
  fn from(token: &Token<T, N>) -> Self {
    Self::Static(StaticLeaf::from(token))
  }
}

impl<T: Tokenable, N: Scalar> From<Token<T, N>> for Leaf<T> {
  #[inline(always)]
  fn from(token: Token<T, N>) -> Self {
    Self::Static(StaticLeaf::from(token))
  }
}

impl<T> From<Expr<T>> for Leaf<T> {
  #[inline(always)]
  fn from(expr: Expr<T>) -> Self {
    Self::Static(StaticLeaf::Expr(expr))
  }
}

impl<T> From<StaticLeaf<T>> for Leaf<T> {
  #[inline(always)]
  fn from(leaf: StaticLeaf<T>) -> Self {
    Self::Static(leaf)
  }
}

impl<T, C> From<T> for Value<T, C> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::new(Leaf::from(value))
  }
}

impl<T: Tokenable, C> From<TokenRef<T>> for Value<T, C> {
  #[inline(always)]
  fn from(reference: TokenRef<T>) -> Self {
    Self::new(Leaf::from(reference))
  }
}

impl<T: Tokenable, N: Scalar, C> From<&Token<T, N>> for Value<T, C> {
  #[inline(always)]
  fn from(token: &Token<T, N>) -> Self {
    Self::new(Leaf::from(token))
  }
}

impl<T: Tokenable, N: Scalar, C> From<Token<T, N>> for Value<T, C> {
  #[inline(always)]
  fn from(token: Token<T, N>) -> Self {
    Self::new(Leaf::from(token))
  }
}

impl<T, C> From<Expr<T>> for Value<T, C> {
  #[inline(always)]
  fn from(expr: Expr<T>) -> Self {
    Self::new(Leaf::from(expr))
  }
}

impl<T, C> From<StaticLeaf<T>> for Value<T, C> {
  #[inline(always)]
  fn from(leaf: StaticLeaf<T>) -> Self {
    Self::new(Leaf::Static(leaf))
  }
}

impl<T, C> From<Leaf<T>> for Value<T, C> {
  #[inline(always)]
  fn from(leaf: Leaf<T>) -> Self {
    Self::new(leaf)
  }
}

impl<L, N> From<Conditional<L, Media<N>>> for Conditional<L, Condition<N>> {
  #[inline(always)]
  fn from(value: Conditional<L, Media<N>>) -> Self {
    value.map_conditions(Condition::from)
  }
}

impl<L, N> From<Conditional<L, State>> for Conditional<L, Condition<N>> {
  #[inline(always)]
  fn from(value: Conditional<L, State>) -> Self {
    value.map_conditions(Condition::from)
  }
}

impl<T, N> From<Definition<T, N>> for Value<T, Condition<N>> {
  #[inline(always)]
  fn from(definition: Definition<T, N>) -> Self {
    definition.map(Leaf::Static).map_conditions(Condition::from)
  }
}

/// The conversions letting a plain value stand for a uniform value of a type built from
/// one, at every leaf: `set_overflow(Overflow::ScrollIfNeeded)`, `when(.., Overflow::Clip)`.
macro_rules! uniform_leaves {
  ($($wrapper:ident),+ $(,)?) => {$(
    impl<T: Copy> From<T> for StaticLeaf<$wrapper<T>> {
      #[inline(always)]
      fn from(value: T) -> Self {
        Self::Fixed($wrapper::from(value))
      }
    }

    impl<T: Copy> From<T> for Leaf<$wrapper<T>> {
      #[inline(always)]
      fn from(value: T) -> Self {
        Self::Static(StaticLeaf::from(value))
      }
    }

    impl<T: Copy, C> From<T> for Value<$wrapper<T>, C> {
      #[inline(always)]
      fn from(value: T) -> Self {
        Self::new(Leaf::from(value))
      }
    }

    impl<T: Copy, N: Scalar> From<T> for Definition<$wrapper<T>, N> {
      #[inline(always)]
      fn from(value: T) -> Self {
        Self::new(StaticLeaf::from(value))
      }
    }
  )+};
}

uniform_leaves!(Axes);

/// The conversions letting a plain number, an `i32` or an `f32`, stand for a length of
/// every type measured in one, at every leaf: `set_spacing(8)`, `set_width(16.5)`,
/// `set_padding(-4)`, `when(.., 16)`; an `f32` rounds to a unit of whole cells.
macro_rules! length_leaves {
  ($($number:ty => $($target:ty),+);+ $(;)?) => {$($(
    impl<N: Scalar> From<$number> for StaticLeaf<$target> {
      #[inline(always)]
      fn from(length: $number) -> Self {
        Self::Fixed(<$target>::from(length))
      }
    }

    impl<N: Scalar> From<$number> for Leaf<$target> {
      #[inline(always)]
      fn from(length: $number) -> Self {
        Self::Static(StaticLeaf::from(length))
      }
    }

    impl<N: Scalar, C> From<$number> for Value<$target, C> {
      #[inline(always)]
      fn from(length: $number) -> Self {
        Self::new(Leaf::from(length))
      }
    }

    impl<N: Scalar, M: Scalar> From<$number> for Definition<$target, M> {
      #[inline(always)]
      fn from(length: $number) -> Self {
        Self::new(StaticLeaf::from(length))
      }
    }
  )+)+};
}

length_leaves!(
  i32 => Spacing<N>, Dimension<N>, Inset<N>, Inset<Dimension<N>>;
  f32 => Spacing<N>, Dimension<N>, Inset<N>, Inset<Dimension<N>>;
);

impl<T, N: Scalar> From<T> for Definition<T, N> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::new(StaticLeaf::Fixed(value))
  }
}

impl<T: Tokenable, N: Scalar> From<TokenRef<T>> for Definition<T, N> {
  #[inline(always)]
  fn from(reference: TokenRef<T>) -> Self {
    Self::new(StaticLeaf::from(reference))
  }
}

impl<T: Tokenable, N: Scalar, M: Scalar> From<&Token<T, M>> for Definition<T, N> {
  #[inline(always)]
  fn from(token: &Token<T, M>) -> Self {
    Self::new(StaticLeaf::from(token))
  }
}

impl<T: Tokenable, N: Scalar, M: Scalar> From<Token<T, M>> for Definition<T, N> {
  #[inline(always)]
  fn from(token: Token<T, M>) -> Self {
    Self::new(StaticLeaf::from(token))
  }
}

impl<T, N: Scalar> From<Expr<T>> for Definition<T, N> {
  #[inline(always)]
  fn from(expr: Expr<T>) -> Self {
    Self::new(StaticLeaf::Expr(expr))
  }
}

impl<T, N: Scalar> From<StaticLeaf<T>> for Definition<T, N> {
  #[inline(always)]
  fn from(leaf: StaticLeaf<T>) -> Self {
    Self::new(leaf)
  }
}

impl<T, C> From<T> for StaticValue<T, C> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::new(KeptLeaf::Static(StaticLeaf::Fixed(value)))
  }
}

impl<T, C> From<StaticLeaf<T>> for StaticValue<T, C> {
  #[inline(always)]
  fn from(leaf: StaticLeaf<T>) -> Self {
    Self::new(KeptLeaf::Static(leaf))
  }
}

impl<T, C> From<KeptLeaf<T>> for StaticValue<T, C> {
  #[inline(always)]
  fn from(leaf: KeptLeaf<T>) -> Self {
    Self::new(leaf)
  }
}

#[cfg(test)]
mod tests {
  use std::{
    hash::{BuildHasher, RandomState},
    rc::Rc,
  };

  use super::{Conditional, Definition, KeptLeaf, Leaf, Slot, StaticLeaf, Value, When};
  use crate::spine::{
    Color,
    Dimension,
    Expr,
    Flexibility,
    GridTrack,
    Inset,
    Length,
    Media,
    Spacing,
    State,
    TokenRef,
    TrackLimit,
  };

  /// A plain value conditioned once is a base and one fixed variant.
  #[test]
  fn when_produces_the_variant() {
    let value: Value<Color, State> =
      Color::TRANSPARENT.when(State::Hovered, Color::srgb(1., 0., 0.));

    assert_eq!(value.base(), &Leaf::from(Color::TRANSPARENT));
    assert_eq!(
      value.variants(),
      &[(State::Hovered, Leaf::from(Color::srgb(1., 0., 0.)))]
    );
  }

  /// A plain number stands for a length of every kind wherever one is taken: in whole
  /// cells an `f32` rounds to the nearest, an `i32` is exact in either unit, and a
  /// `Length` keeps its fraction in both.
  #[test]
  fn plain_numbers_stand_for_every_length() {
    let width: Value<Dimension<i32>, State> = Value::from(1.5).when(State::Hovered, -4);

    assert_eq!(width.base(), &Leaf::from(Dimension::Length(2)));
    assert_eq!(
      width.variants(),
      &[(State::Hovered, Leaf::from(Dimension::Length(-4)))]
    );
    assert_eq!(
      Value::<Spacing<f32>, State>::from(8).base(),
      &Leaf::from(Spacing::all(8.))
    );
    assert_eq!(
      Value::<Inset<Dimension<i32>>, State>::from(-1.4).base(),
      &Leaf::from(Inset::all(Dimension::Length(-1)))
    );
    assert_eq!(
      Value::<Length, State>::from(1.5).base(),
      &Leaf::from(Length(1.5))
    );
    assert_eq!(
      Definition::<Length, i32>::from(8).base(),
      &StaticLeaf::Fixed(Length(8.))
    );
    assert_eq!(
      Definition::<Inset<i32>, i32>::from(2.5).base(),
      &StaticLeaf::Fixed(Inset::all(3))
    );

    let Leaf::Derived(derived) = Leaf::<Dimension<i32>>::derive(|| 2.5)
    else {
      panic!("a derived leaf");
    };

    assert_eq!(derived(), StaticLeaf::Fixed(Dimension::Length(3)));
    assert_eq!(GridTrack::<i32>::from(2.5), GridTrack::Length(3));
    assert_eq!(TrackLimit::<f32>::from(16), TrackLimit::Length(16.));
    assert_eq!(
      Flexibility::<i32>::new(4, 1., 0.).initial,
      Dimension::Length(4)
    );
    assert_eq!(
      Media::<i32>::min_width(89.5),
      Media::MinWidth(90),
      "a condition's length rounds as a setter's does"
    );
  }

  /// A constructor measured in lengths places each number where its parameter names:
  /// an inset's start, top, end and bottom in that order, a spacing's horizontal then
  /// vertical, and a track's limits as its minimum then its maximum.
  #[test]
  fn length_constructors_place_each_number_by_name() {
    assert_eq!(
      Inset::<f32>::new(1, 2, 3, 4),
      Inset {
        start: 1.,
        top: 2.,
        end: 3.,
        bottom: 4.,
      }
    );

    let inset = Inset::<i32>::new(1.5, -4, 0, 2);

    assert_eq!((inset.start, inset.top), (2, -4));
    assert_eq!(
      Spacing::<i32>::new(8, -4),
      Spacing {
        horizontal: 8,
        vertical: -4,
      }
    );
    assert_eq!(GridTrack::<f32>::length(16), GridTrack::Length(16.));
    assert_eq!(GridTrack::<i32>::fit_content(2.5), GridTrack::FitContent(3));
    assert_eq!(
      GridTrack::<f32>::between(16, 1.5),
      GridTrack::Between(TrackLimit::Length(16.), TrackLimit::Length(1.5))
    );
    assert_eq!(
      GridTrack::<i32>::adaptive(4, TrackLimit::Flex(1.)),
      GridTrack::Adaptive(TrackLimit::Length(4), TrackLimit::Flex(1.))
    );
  }

  /// A token reference conditions like a plain value, and a variant may be a token.
  #[test]
  fn tokens_condition_like_values() {
    let card = TokenRef::<Color>::new("card");
    let hover = TokenRef::<Color>::new("hover");
    let value = card.when(State::Hovered, hover);

    assert_eq!(value.base(), &Leaf::Static(StaticLeaf::Token(card.name())));
    assert_eq!(
      value.variants(),
      &[(
        State::Hovered,
        Leaf::Static(StaticLeaf::Token(hover.name()))
      )]
    );
  }

  /// A derived leaf compares by closure identity, never by value.
  #[test]
  fn derived_leaves_compare_by_identity() {
    let compute: Rc<dyn Fn() -> StaticLeaf<Color>> =
      Rc::new(|| StaticLeaf::Fixed(Color::TRANSPARENT));
    let same = Leaf::Derived(Rc::clone(&compute));
    let other = Leaf::derive(|| Color::TRANSPARENT);

    assert_eq!(Leaf::Derived(compute), same);
    assert_ne!(same, other);
    assert_ne!(same, Leaf::from(Color::TRANSPARENT));
  }

  /// Splitting replaces each derived leaf with the slot allocated for its position.
  #[test]
  fn split_lifts_derived_leaves_into_slots() {
    let value = Color::TRANSPARENT
      .derive_when(State::Hovered, || Color::TRANSPARENT)
      .when(State::Pressed, Color::TRANSPARENT)
      .derive_when(State::Focused, || Color::TRANSPARENT);
    let mut positions = Vec::new();
    let split = value.split_derived(|position, _| {
      positions.push(position);

      Slot(u16::try_from(positions.len()).unwrap() + 6)
    });

    assert_eq!(positions, [1, 3]);
    assert_eq!(
      split.base(),
      &KeptLeaf::Static(StaticLeaf::Fixed(Color::TRANSPARENT))
    );
    assert_eq!(split.variants()[0].1, KeptLeaf::Slot(Slot(7)));
    assert_eq!(
      split.variants()[1].1,
      KeptLeaf::Static(StaticLeaf::Fixed(Color::TRANSPARENT))
    );
    assert_eq!(split.variants()[2].1, KeptLeaf::Slot(Slot(8)));
  }

  /// Two definitions with the same content hash alike, expressions included.
  #[test]
  fn equal_definitions_hash_alike() {
    let define = || {
      Definition::<Color, f32>::from(Color::srgb(1., 1., 1.))
        .when(Media::Dark, Color::srgb(0., 0., 0.))
        .when(
          Media::HighContrast,
          Expr::from(TokenRef::<Color>::new("ink")).with_alpha(0.5),
        )
    };
    let hasher = RandomState::new();

    assert_eq!(define(), define());
    assert_eq!(hasher.hash_one(define()), hasher.hash_one(define()));
    assert_ne!(
      hasher.hash_one(define()),
      hasher.hash_one(Definition::<Color, f32>::from(Color::srgb(1., 1., 1.))),
    );
  }

  /// A definition holds static leaves only.
  #[test]
  fn definitions_are_data() {
    let definition: Definition<Color, f32> =
      Conditional::new(StaticLeaf::Fixed(Color::TRANSPARENT));

    assert_eq!(definition.leaves().count(), 1);
  }
}
