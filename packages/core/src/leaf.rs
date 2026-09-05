//! The leaves a property's value is made of, and the conditions that pick between them.

use std::{fmt::Debug, rc::Rc};

use smallvec::SmallVec;

use super::{Condition, Expr, Media, Scalar, State, Token, TokenRef};
use crate::{Dimension, Spacing};

/// A value that is data: a literal, a token reference, or an expression.
///
/// What a token's [`Definition`] holds, and the part of a [`Leaf`] a driver can compile
/// ahead of time.
#[derive(Debug, Clone, PartialEq, Hash)]
#[must_use]
pub enum StaticLeaf<T> {
  /// A literal value.
  Fixed(T),
  /// A reference to a token, resolved where the property is realized.
  Token(TokenRef<T>),
  /// An expression, evaluated where the property is realized.
  Expr(Expr<T>),
}

/// What a builder stores for a property: a [`StaticLeaf`], or a value derived from
/// signals.
#[derive(Clone)]
#[must_use]
pub enum Leaf<T> {
  /// A value known when the view is built.
  Static(StaticLeaf<T>),
  /// A value recomputed whenever the signals it reads change.
  Derived(Rc<dyn Fn() -> T>),
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

/// What a driver receives for a property: a [`StaticLeaf`] to compile, or the slot a
/// derived leaf's render effect writes.
#[derive(Debug, Clone, PartialEq, Hash)]
#[must_use]
pub enum DriverLeaf<T> {
  /// A value the driver compiles ahead of time.
  Static(StaticLeaf<T>),
  /// A value a render effect writes into the slot.
  Slot(Slot),
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
  /// The computation, tracking the signals it reads.
  pub compute: Rc<dyn Fn() -> V>,
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
  variants: SmallVec<[(C, L); 1]>,
}

/// What a builder collects for one property: a [`Leaf`] under the facade's conditions.
pub type Value<T, C> = Conditional<Leaf<T>, C>;

/// What a token is defined as: a [`StaticLeaf`] under [`Media`] conditions over the
/// facade's scalar.
///
/// No closure can enter one, so a palette is data a server can serialize and a driver
/// can compile once. Only media conditions apply: a token answers the environment, and a
/// node's own state is the consuming property's to condition on.
///
/// ```compile_fail
/// use std::rc::Rc;
///
/// use skene_core::{Color, Definition, Leaf};
///
/// let _: Definition<Color, f32> = Definition::from(Leaf::Derived(Rc::new(|| Color::TRANSPARENT)));
/// ```
pub type Definition<T, N> = Conditional<StaticLeaf<T>, Media<N>>;

/// What a driver receives for one property: a [`DriverLeaf`] under the facade's
/// conditions.
pub type StaticValue<T, C> = Conditional<DriverLeaf<T>, C>;

impl<L, C> Conditional<L, C> {
  /// A value with no variants.
  #[inline(always)]
  pub fn new(base: L) -> Self {
    Self {
      base,
      variants: SmallVec::new(),
    }
  }

  /// This value, overridden by `value` while `condition` holds.
  #[inline(always)]
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

  /// The base and the variants.
  #[inline(always)]
  pub fn into_parts(self) -> (L, SmallVec<[(C, L); 1]>) {
    (self.base, self.variants)
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
  /// This value, overridden while `condition` holds by a value derived from the signals
  /// `compute` reads.
  #[inline(always)]
  pub fn derive_when(self, condition: C, compute: impl Fn() -> T + 'static) -> Self {
    self.when(condition, Leaf::Derived(Rc::new(compute)))
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
    mut allocate: impl FnMut(usize, Rc<dyn Fn() -> T>) -> Slot,
  ) -> StaticValue<T, C> {
    let mut position = 0;

    self.map(|leaf| {
      let leaf = match leaf {
        Leaf::Static(leaf) => DriverLeaf::Static(leaf),
        Leaf::Derived(compute) => DriverLeaf::Slot(allocate(position, compute)),
      };

      position += 1;

      leaf
    })
  }
}

/// Conditions a plain value, producing a [`Value`] directly:
/// `Color::WHITE.when(State::Hovered, Color::BLACK)`.
///
/// Every plain value has it; a [`TokenRef`], [`Token`] and [`Expr`] carry the same
/// `when` as an inherent method. The condition fixes the value's condition type, and a
/// value under [`Media`] or [`State`] alone converts into one under [`Condition`], so
/// `8f32.when(Media::MinWidth(900.), 16.)` is accepted where a
/// `Value<f32, Condition<N>>` is.
pub trait When<T>: Into<Leaf<T>> {
  /// This value, overridden by `value` while `condition` holds.
  #[inline(always)]
  fn when<C>(self, condition: C, value: impl Into<Leaf<T>>) -> Value<T, C> {
    Conditional::new(self.into()).when(condition, value)
  }

  /// This value, overridden while `condition` holds by a value derived from the
  /// signals `compute` reads.
  #[inline(always)]
  fn derive_when<C>(self, condition: C, compute: impl Fn() -> T + 'static) -> Value<T, C> {
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

impl<T> From<TokenRef<T>> for StaticLeaf<T> {
  #[inline(always)]
  fn from(reference: TokenRef<T>) -> Self {
    Self::Token(reference)
  }
}

impl<T, N: Scalar> From<&Token<T, N>> for StaticLeaf<T> {
  #[inline(always)]
  fn from(token: &Token<T, N>) -> Self {
    Self::Token(token.reference())
  }
}

impl<T, N: Scalar> From<Token<T, N>> for StaticLeaf<T> {
  #[inline(always)]
  fn from(token: Token<T, N>) -> Self {
    Self::Token(token.reference())
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

impl<T> From<TokenRef<T>> for Leaf<T> {
  #[inline(always)]
  fn from(reference: TokenRef<T>) -> Self {
    Self::Static(StaticLeaf::Token(reference))
  }
}

impl<T, N: Scalar> From<&Token<T, N>> for Leaf<T> {
  #[inline(always)]
  fn from(token: &Token<T, N>) -> Self {
    Self::Static(StaticLeaf::Token(token.reference()))
  }
}

impl<T, N: Scalar> From<Token<T, N>> for Leaf<T> {
  #[inline(always)]
  fn from(token: Token<T, N>) -> Self {
    Self::Static(StaticLeaf::Token(token.reference()))
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

impl<T, C> From<TokenRef<T>> for Value<T, C> {
  #[inline(always)]
  fn from(reference: TokenRef<T>) -> Self {
    Self::new(Leaf::from(reference))
  }
}

impl<T, N: Scalar, C> From<&Token<T, N>> for Value<T, C> {
  #[inline(always)]
  fn from(token: &Token<T, N>) -> Self {
    Self::new(Leaf::from(token))
  }
}

impl<T, N: Scalar, C> From<Token<T, N>> for Value<T, C> {
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

/// The conversions letting a plain length stand for a uniform value of a type built
/// from one, at every leaf: `set_spacing(8.)`, `set_width(10.)`, `when(.., 16.)`.
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

uniform_leaves!(Spacing, Dimension);

impl<T, N: Scalar> From<T> for Definition<T, N> {
  #[inline(always)]
  fn from(value: T) -> Self {
    Self::new(StaticLeaf::Fixed(value))
  }
}

impl<T, N: Scalar> From<TokenRef<T>> for Definition<T, N> {
  #[inline(always)]
  fn from(reference: TokenRef<T>) -> Self {
    Self::new(StaticLeaf::Token(reference))
  }
}

impl<T, N: Scalar, M: Scalar> From<&Token<T, M>> for Definition<T, N> {
  #[inline(always)]
  fn from(token: &Token<T, M>) -> Self {
    Self::new(StaticLeaf::Token(token.reference()))
  }
}

impl<T, N: Scalar, M: Scalar> From<Token<T, M>> for Definition<T, N> {
  #[inline(always)]
  fn from(token: Token<T, M>) -> Self {
    Self::new(StaticLeaf::Token(token.reference()))
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
    Self::new(DriverLeaf::Static(StaticLeaf::Fixed(value)))
  }
}

impl<T, C> From<StaticLeaf<T>> for StaticValue<T, C> {
  #[inline(always)]
  fn from(leaf: StaticLeaf<T>) -> Self {
    Self::new(DriverLeaf::Static(leaf))
  }
}

impl<T, C> From<DriverLeaf<T>> for StaticValue<T, C> {
  #[inline(always)]
  fn from(leaf: DriverLeaf<T>) -> Self {
    Self::new(leaf)
  }
}

#[cfg(test)]
mod tests {
  use std::{
    hash::{BuildHasher, RandomState},
    rc::Rc,
  };

  use super::{Conditional, Definition, DriverLeaf, Leaf, Slot, StaticLeaf, Value, When};
  use crate::{Color, Expr, Media, State, TokenRef};

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

  /// A token reference conditions like a plain value, and a variant may be a token.
  #[test]
  fn tokens_condition_like_values() {
    let card = TokenRef::<Color>::new("card");
    let hover = TokenRef::<Color>::new("hover");
    let value = card.when(State::Hovered, hover);

    assert_eq!(value.base(), &Leaf::Static(StaticLeaf::Token(card)));
    assert_eq!(
      value.variants(),
      &[(State::Hovered, Leaf::Static(StaticLeaf::Token(hover)))]
    );
  }

  /// A derived leaf compares by closure identity, never by value.
  #[test]
  fn derived_leaves_compare_by_identity() {
    let compute: Rc<dyn Fn() -> Color> = Rc::new(|| Color::TRANSPARENT);
    let same = Leaf::Derived(Rc::clone(&compute));
    let other = Leaf::Derived(Rc::new(|| Color::TRANSPARENT));

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
      &DriverLeaf::Static(StaticLeaf::Fixed(Color::TRANSPARENT))
    );
    assert_eq!(split.variants()[0].1, DriverLeaf::Slot(Slot(7)));
    assert_eq!(
      split.variants()[1].1,
      DriverLeaf::Static(StaticLeaf::Fixed(Color::TRANSPARENT))
    );
    assert_eq!(split.variants()[2].1, DriverLeaf::Slot(Slot(8)));
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
