//! Design tokens: named, typed slots a palette defines and a property references.

use std::{
  fmt::{self, Debug, Display},
  hash::{Hash, Hasher},
  marker::PhantomData,
  rc::Rc,
};

use super::{Definition, Leaf, Media, Scalar, StaticLeaf, Value};
use crate::spine::{Color, Dimension, Length, values::is_identifier};

/// A token's name: what a consumer emits, and the key a theme defines.
///
/// An implementation detail on every platform, never a public hook; it is limited to
/// ASCII letters, digits, `_` and `-`, and does not start with a digit, so it is an
/// identifier wherever a platform spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct TokenName(&'static str);

impl TokenName {
  /// Names a token; debug builds check the name is identifier-safe.
  #[inline(always)]
  pub const fn new(name: &'static str) -> Self {
    debug_assert!(
      is_identifier(name),
      "a token name is ASCII letters, digits, `_` and `-`, and does not start with a digit",
    );

    Self(name)
  }

  /// The name.
  #[inline(always)]
  #[must_use]
  pub const fn as_str(self) -> &'static str {
    self.0
  }
}

impl Display for TokenName {
  #[inline(always)]
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(self.0)
  }
}

/// A type a token holds: a color, a number, a length, a dimension or a text.
///
/// A [`TokenRef`] and a [`Token`] are of these types alone, so every reference names a
/// slot a platform can hold; a property of another type reads a token through the
/// conversions its type offers, as padding reads a length token.
#[diagnostic::on_unimplemented(
  message = "a token cannot hold a `{Self}`",
  label = "a token holds a `Color`, a number (`f32`), a `Length`, a `Dimension` or a `String`"
)]
pub trait Tokenable: 'static {}

impl Tokenable for Color {}

impl Tokenable for f32 {}

impl Tokenable for Length {}

impl<N: Scalar> Tokenable for Dimension<N> {}

impl Tokenable for String {}

/// A typed reference to a token: the name a property emits, without any definition.
///
/// A `const` in a component library's palette module, so `set_background(Palette::CARD)`
/// names the slot whatever theme is in force.
#[must_use]
pub struct TokenRef<T: Tokenable> {
  name: TokenName,
  ty: PhantomData<fn() -> T>,
}

impl<T: Tokenable> TokenRef<T> {
  /// A reference to the token named `name`.
  #[inline(always)]
  pub const fn new(name: &'static str) -> Self {
    Self {
      name: TokenName::new(name),
      ty: PhantomData,
    }
  }

  /// The token's name.
  #[inline(always)]
  pub const fn name(self) -> TokenName {
    self.name
  }

  /// This token, overridden by `value` while `condition` holds.
  pub fn when<C>(self, condition: C, value: impl Into<Leaf<T>>) -> Value<T, C> {
    Value::from(self).when(condition, value)
  }

  /// This token, overridden while `condition` holds by a leaf derived from the
  /// signals `compute` reads: a value, or a token or an expression where its type takes one.
  pub fn derive_when<C, R: Into<StaticLeaf<T>>>(
    self,
    condition: C,
    compute: impl Fn() -> R + 'static,
  ) -> Value<T, C> {
    Value::from(self).derive_when(condition, compute)
  }
}

impl<T: Tokenable> Clone for TokenRef<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: Tokenable> Copy for TokenRef<T> {}

impl<T: Tokenable> PartialEq for TokenRef<T> {
  #[inline(always)]
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name
  }
}

impl<T: Tokenable> Eq for TokenRef<T> {}

impl<T: Tokenable> Hash for TokenRef<T> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.name.hash(state);
  }
}

impl<T: Tokenable> Debug for TokenRef<T> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.debug_tuple("TokenRef").field(&self.name).finish()
  }
}

/// A token as a palette holds it: the reference, and the [`Definition`] the palette
/// gives it over the facade's scalar `N`.
///
/// Setting a property to a token emits the name alone; the definition reaches the
/// platform through the theme that provides the palette.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Token<T: Tokenable, N: Scalar> {
  reference: TokenRef<T>,
  definition: Rc<Definition<T, N>>,
}

impl<T: Tokenable, N: Scalar> Token<T, N> {
  /// A token named `name`, defined as `definition`.
  pub fn new(name: &'static str, definition: impl Into<Definition<T, N>>) -> Self {
    Self {
      reference: TokenRef::new(name),
      definition: Rc::new(definition.into()),
    }
  }

  /// This token defined anew; the name stays.
  pub fn redefine(mut self, definition: impl Into<Definition<T, N>>) -> Self {
    self.definition = Rc::new(definition.into());

    self
  }

  /// This token defined as `value` while `condition` holds, over its present
  /// definition.
  pub fn define_when(mut self, condition: Media<N>, value: impl Into<StaticLeaf<T>>) -> Self
  where
    T: Clone,
  {
    self.definition = Rc::new(Rc::unwrap_or_clone(self.definition).when(condition, value));

    self
  }

  /// The reference a property emits.
  #[inline(always)]
  pub const fn reference(&self) -> TokenRef<T> {
    self.reference
  }

  /// The token's name.
  #[inline(always)]
  pub const fn name(&self) -> TokenName {
    self.reference.name
  }

  /// The definition the palette gives the token.
  #[inline(always)]
  pub fn definition(&self) -> &Definition<T, N> {
    &self.definition
  }

  /// The token's name with its definition erased into the platform's value `V`.
  pub fn define<V>(&self) -> (TokenName, V)
  where
    T: Clone,
    Definition<T, N>: IntoTokenValue<V>,
  {
    (self.name(), (*self.definition).clone().into_token_value())
  }

  /// This token, overridden by `value` while `condition` holds.
  pub fn when<C>(&self, condition: C, value: impl Into<Leaf<T>>) -> Value<T, C> {
    self.reference.when(condition, value)
  }

  /// This token, overridden while `condition` holds by a leaf derived from the
  /// signals `compute` reads: a value, or a token or an expression where its type takes one.
  pub fn derive_when<C, R: Into<StaticLeaf<T>>>(
    &self,
    condition: C,
    compute: impl Fn() -> R + 'static,
  ) -> Value<T, C> {
    self.reference.derive_when(condition, compute)
  }
}

/// Erases a typed definition into the value a facade's platform stores per token.
///
/// Every `From` conversion provides it, so a facade implements
/// `From<Definition<Color, N>>` and its kin on one erased enum and every token defines
/// through it.
#[diagnostic::on_unimplemented(
  message = "`{V}` cannot hold a `{Self}`",
  label = "the facade's token value must implement `From` for this definition type"
)]
pub trait IntoTokenValue<V> {
  /// This definition, erased.
  fn into_token_value(self) -> V;
}

impl<P, V: From<P>> IntoTokenValue<V> for P {
  #[inline(always)]
  fn into_token_value(self) -> V {
    V::from(self)
  }
}

/// A set of tokens a theme defines together: a component library's palette, a
/// facade's typography, the terminal's glyphs.
///
/// A struct of [`Token`]s implements it by listing them; `V` is the facade's erased
/// token value.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a token set",
  label = "a palette is a struct of `Token`s implementing `TokenSet` by listing their definitions"
)]
pub trait TokenSet<V>: Clone + PartialEq + 'static {
  /// Every token's name with its erased definition.
  fn definitions(&self) -> Vec<(TokenName, V)>;
}

/// The definitions a theme scope holds, looked up by name.
#[derive(Debug, Clone, PartialEq, Default)]
#[must_use]
pub struct Definitions<V>(Vec<(TokenName, V)>);

impl<V> Definitions<V> {
  /// The definitions of `set`.
  pub fn of(set: &impl TokenSet<V>) -> Self {
    Self(set.definitions())
  }

  /// The definition of the token named `name`, if the set defines it.
  #[inline(always)]
  #[must_use]
  pub fn get(&self, name: TokenName) -> Option<&V> {
    self
      .0
      .iter()
      .find_map(|(defined, value)| (*defined == name).then_some(value))
  }

  /// Every definition, in the set's order.
  #[inline(always)]
  pub fn iter(&self) -> impl Iterator<Item = (TokenName, &V)> {
    self.0.iter().map(|(name, value)| (*name, value))
  }

  /// How many tokens are defined.
  #[inline(always)]
  #[must_use]
  pub const fn len(&self) -> usize {
    self.0.len()
  }

  /// Whether no token is defined.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// Lays `over` on this set: every token `over` defines replaces this set's
  /// definition of the same name, and the rest are appended in `over`'s order.
  pub fn overlay(&mut self, over: Self) {
    for (name, value) in over.0 {
      match self.0.iter_mut().find(|(defined, _)| *defined == name) {
        Some(slot) => slot.1 = value,
        None => self.0.push((name, value)),
      }
    }
  }
}

impl<V> From<Vec<(TokenName, V)>> for Definitions<V> {
  #[inline(always)]
  fn from(definitions: Vec<(TokenName, V)>) -> Self {
    Self(definitions)
  }
}

impl<V> FromIterator<(TokenName, V)> for Definitions<V> {
  fn from_iter<I: IntoIterator<Item = (TokenName, V)>>(iter: I) -> Self {
    Self(iter.into_iter().collect())
  }
}

#[cfg(test)]
mod tests {
  use super::{Definitions, Token, TokenName, TokenRef, TokenSet};
  use crate::spine::{Color, Definition, Expr, Media, StaticLeaf};

  /// The erased value a facade would keep per token.
  #[derive(Debug, Clone, PartialEq)]
  enum Erased {
    Color(Definition<Color, f32>),
    Length(Definition<f32, f32>),
  }

  impl From<Definition<Color, f32>> for Erased {
    fn from(definition: Definition<Color, f32>) -> Self {
      Self::Color(definition)
    }
  }

  impl From<Definition<f32, f32>> for Erased {
    fn from(definition: Definition<f32, f32>) -> Self {
      Self::Length(definition)
    }
  }

  #[derive(Debug, Clone, PartialEq)]
  struct Palette {
    card: Token<Color, f32>,
    gutter: Token<f32, f32>,
  }

  impl Default for Palette {
    fn default() -> Self {
      Self {
        card: Token::new("card", Color::srgb(1., 1., 1.))
          .define_when(Media::Dark, Color::srgb(0., 0., 0.)),
        gutter: Token::new("gutter", 8.),
      }
    }
  }

  impl TokenSet<Erased> for Palette {
    fn definitions(&self) -> Vec<(TokenName, Erased)> {
      vec![self.card.define(), self.gutter.define()]
    }
  }

  /// Redefining keeps the name; conditioning layers a variant over the definition.
  #[test]
  fn redefine_keeps_the_name() {
    let card = Palette::default().card;
    let redefined = card
      .clone()
      .redefine(Expr::from(TokenRef::<Color>::new("ink")).with_alpha(0.1));

    assert_eq!(redefined.name(), card.name());
    assert_eq!(redefined.reference(), card.reference());
    assert_eq!(redefined.definition().variants(), &[]);
    assert_eq!(card.definition().variants().len(), 1);
    assert_eq!(
      card.definition().base(),
      &StaticLeaf::Fixed(Color::srgb(1., 1., 1.))
    );
    assert_eq!(card.name().to_string(), "card");
  }

  /// A set erases each token through the facade's `From`, and a lookup finds it by name.
  #[test]
  fn definitions_erase_and_look_up_by_name() {
    let palette = Palette::default();
    let definitions = Definitions::of(&palette);

    assert_eq!(definitions.len(), 2);
    assert_eq!(
      definitions.get(TokenName::new("gutter")),
      Some(&Erased::Length(Definition::from(8.)))
    );
    assert_eq!(
      definitions.get(TokenName::new("card")),
      Some(&Erased::Color(palette.card.definition().clone()))
    );
    assert_eq!(definitions.get(TokenName::new("missing")), None);
  }

  /// A name that is not an identifier is rejected in debug builds.
  #[test]
  #[should_panic(expected = "a token name is ASCII letters")]
  #[cfg(debug_assertions)]
  fn names_must_be_identifiers() {
    let _ = TokenName::new("1st");
  }

  /// Identifier-safe names are accepted.
  #[test]
  fn identifier_names_are_accepted() {
    assert_eq!(TokenName::new("muted-ink_2").as_str(), "muted-ink_2");
  }
}
