//! Design tokens: a token is a named, inherited slot ([`TokenRef`]); a theme defines it
//! ([`Token`], a name with its [`Definition`]); a palette is a library's set of definitions
//! ([`TokenSet`]); and a value names the slot, resolved through the scopes above it
//! ([`Tokens`]).
//!
//! A definition carries its own variants, conditioned on [`Media`] alone, so dark mode lives in
//! the definition: `Token::new(CARD, Color::WHITE).when(Media::Dark, Color::rgb(0x1c1c1e))`.
//! A definition is plain data: no closure, no state.

use std::{fmt, marker::PhantomData, rc::Rc};

use crate::{Color, Environment, Media, Property, Signal};

/// A token's slot, named once as a `const` and holding a `T`: the name is what a value refers
/// to and what a theme defines.
pub struct TokenRef<T> {
  name: &'static str,
  ty: PhantomData<fn() -> T>,
}

impl<T> Clone for TokenRef<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Copy for TokenRef<T> {}

impl<T> PartialEq for TokenRef<T> {
  #[inline(always)]
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name
  }
}

impl<T> fmt::Debug for TokenRef<T> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(formatter, "TokenRef({:?})", self.name)
  }
}

impl<T> TokenRef<T> {
  /// The slot named `name`, which each medium realizes under this name.
  #[inline(always)]
  #[must_use]
  pub const fn new(name: &'static str) -> Self {
    Self {
      name,
      ty: PhantomData,
    }
  }

  /// The slot's name.
  #[inline(always)]
  #[must_use]
  pub const fn name(self) -> &'static str {
    self.name
  }
}

/// A value a token holds, erased: what every medium resolves and compiles.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TokenValue {
  /// A color.
  Color(Color),
  /// A number: a length in pixels, or a plain number.
  Number(f32),
}

/// A type a token holds: a color, or a number (a length in pixels, or a plain number).
#[diagnostic::on_unimplemented(
  message = "a token cannot hold `{Self}`",
  label = "not a type a token holds",
  note = "a token holds a `Color` or a number (`f32`, a length in pixels or a plain number)"
)]
pub trait Tokenable: Property {
  /// This value, erased.
  fn into_token(self) -> TokenValue;
}

impl Tokenable for Color {
  #[inline(always)]
  fn into_token(self) -> TokenValue {
    TokenValue::Color(self)
  }
}

impl Tokenable for f32 {
  #[inline(always)]
  fn into_token(self) -> TokenValue {
    TokenValue::Number(self)
  }
}

/// A leaf of a [`Definition`].
#[derive(Clone, PartialEq, Debug)]
pub enum DefinitionLeaf {
  /// A value.
  Value(TokenValue),
  /// Another token's value, resolved in the scope that defines this one.
  Token(&'static str),
}

/// What defines a token of type `T`: a `T`, or another token of the same type.
#[diagnostic::on_unimplemented(
  message = "`{Self}` cannot define a token holding `{T}`",
  label = "not a `{T}` here",
  note = "a token is defined by a value of its own type, or by another token of that type; a token holds a `Color` or a number"
)]
pub trait IntoDefinition<T> {
  /// This as a definition's leaf.
  fn into_leaf(self) -> DefinitionLeaf;
}

impl<T: Tokenable> IntoDefinition<T> for T {
  #[inline(always)]
  fn into_leaf(self) -> DefinitionLeaf {
    DefinitionLeaf::Value(self.into_token())
  }
}

impl<T> IntoDefinition<T> for TokenRef<T> {
  #[inline(always)]
  fn into_leaf(self) -> DefinitionLeaf {
    DefinitionLeaf::Token(self.name)
  }
}

/// An integer literal defines a length token.
impl IntoDefinition<f32> for i32 {
  #[inline(always)]
  fn into_leaf(self) -> DefinitionLeaf {
    DefinitionLeaf::Value(TokenValue::Number(self as f32))
  }
}

/// A token's definition: a leaf, and the leaves that replace it while media conditions hold,
/// the last holding winning.
#[derive(Clone, PartialEq, Debug)]
pub struct Definition {
  /// The leaf in force where no condition holds.
  pub base: DefinitionLeaf,
  /// Each condition with the leaf it puts in force, in the order written.
  pub variants: Vec<(Media, DefinitionLeaf)>,
}

/// A token's slot with the definition a palette gives it.
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct Token<T> {
  reference: TokenRef<T>,
  definition: Definition,
}

impl<T> Token<T> {
  /// Defines `reference` as `value`.
  pub fn new(reference: TokenRef<T>, value: impl IntoDefinition<T>) -> Self {
    Self {
      reference,
      definition: Definition {
        base: value.into_leaf(),
        variants: Vec::new(),
      },
    }
  }

  /// This definition, replaced by `value` while `media` holds.
  pub fn when(mut self, media: Media, value: impl IntoDefinition<T>) -> Self {
    self.definition.variants.push((media, value.into_leaf()));

    self
  }

  /// The slot this token defines.
  #[inline(always)]
  pub const fn reference(&self) -> TokenRef<T> {
    self.reference
  }

  /// The definition.
  #[inline(always)]
  pub const fn definition(&self) -> &Definition {
    &self.definition
  }
}

/// A set of definitions by name, in the order added.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Definitions(Vec<(&'static str, Definition)>);

impl Definitions {
  /// No definitions.
  #[inline(always)]
  #[must_use]
  pub const fn new() -> Self {
    Self(Vec::new())
  }

  /// Adds `token`'s definition, replacing one of the same name.
  pub fn add<T>(&mut self, token: &Token<T>) -> &mut Self {
    let name = token.reference.name;

    match self.0.iter_mut().find(|(defined, _)| *defined == name) {
      Some((_, definition)) => *definition = token.definition.clone(),
      None => self.0.push((name, token.definition.clone())),
    }

    self
  }

  /// The definition of `name`, if this set has one.
  #[must_use]
  pub fn get(&self, name: &str) -> Option<&Definition> {
    self
      .0
      .iter()
      .find(|(defined, _)| *defined == name)
      .map(|(_, definition)| definition)
  }

  /// Each name with its definition, in the order added.
  pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Definition)> {
    self.0.iter().map(|(name, definition)| (*name, definition))
  }

  /// How many tokens this set defines.
  #[inline(always)]
  #[must_use]
  pub const fn len(&self) -> usize {
    self.0.len()
  }

  /// Whether this set defines nothing.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

/// A palette: a set of tokens a theme defines at once.
///
/// A palette is a struct of [`Token`]s that adds each of them; it belongs to its library, and
/// nothing below a component library knows what a palette is.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a palette",
  label = "a theme defines a palette here",
  note = "a palette is a struct of `Token`s implementing `TokenSet`, adding each of them in `define`"
)]
pub trait TokenSet: 'static {
  /// Adds each of this set's definitions.
  fn define(&self, definitions: &mut Definitions);

  /// This set's definitions.
  #[must_use]
  fn definitions(&self) -> Definitions {
    let mut definitions = Definitions::new();

    self.define(&mut definitions);

    definitions
  }
}

/// The token scopes a node sits in, innermost first: what a token resolves through.
///
/// Cloning shares the chain. A scope's definitions are a signal, so a theme whose palette
/// changes rewrites one signal and every value that read a token follows.
#[derive(Clone, Default)]
pub struct Tokens(Option<Rc<Scope>>);

struct Scope {
  parent: Tokens,
  definitions: Signal<Rc<Definitions>>,
}

impl fmt::Debug for Tokens {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut depth = 0;
    let mut next = &self.0;

    while let Some(scope) = next {
      depth += 1;
      next = &scope.parent.0;
    }

    write!(formatter, "Tokens({depth} scopes)")
  }
}

impl Tokens {
  /// No scope: every token is undefined.
  #[inline(always)]
  #[must_use]
  pub const fn root() -> Self {
    Self(None)
  }

  /// A scope inside this one, defining what `definitions` holds.
  #[must_use]
  pub fn scope(&self, definitions: Signal<Rc<Definitions>>) -> Self {
    Self(Some(Rc::new(Scope {
      parent: self.clone(),
      definitions,
    })))
  }

  /// The definitions of the innermost scope, if there is one.
  #[must_use]
  pub fn innermost(&self) -> Option<Signal<Rc<Definitions>>> {
    self.0.as_ref().map(|scope| scope.definitions)
  }

  /// The value of the token `name` now: its definition in the nearest scope that defines it,
  /// under this thread's [`Environment`], subscribing the running effect to what it read.
  #[must_use]
  pub fn resolve(&self, name: &str) -> Option<TokenValue> {
    self.resolve_at(name, 0)
  }

  fn resolve_at(&self, name: &str, depth: u32) -> Option<TokenValue> {
    if depth > 16 {
      return None;
    }

    let mut next = self;

    while let Some(scope) = &next.0 {
      let definition = scope
        .definitions
        .with(|definitions| definitions.get(name).cloned());

      if let Some(definition) = definition {
        let environment = Environment::current();
        let mut leaf = &definition.base;

        for (media, variant) in &definition.variants {
          if environment.holds(media) {
            leaf = variant;
          }
        }

        return match leaf {
          DefinitionLeaf::Value(value) => Some(*value),
          DefinitionLeaf::Token(other) => next.resolve_at(other, depth + 1),
        };
      }

      next = &scope.parent;
    }

    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const CARD: TokenRef<Color> = TokenRef::new("card");
  const SURFACE: TokenRef<Color> = TokenRef::new("surface");

  struct Palette {
    card: Token<Color>,
    surface: Token<Color>,
  }

  impl TokenSet for Palette {
    fn define(&self, definitions: &mut Definitions) {
      definitions.add(&self.card).add(&self.surface);
    }
  }

  #[test]
  fn a_definition_answers_dark_mode_itself() {
    let palette = Palette {
      card: Token::new(CARD, Color::WHITE).when(Media::Dark, Color::rgb(0x1c1c1e)),
      surface: Token::new(SURFACE, CARD),
    };
    let tokens = Tokens::root().scope(Signal::new(Rc::new(palette.definitions())));
    let environment = Environment::current();

    assert_eq!(
      tokens.resolve("surface"),
      Some(TokenValue::Color(Color::WHITE))
    );

    environment.dark.set(true);

    assert_eq!(
      tokens.resolve("surface"),
      Some(TokenValue::Color(Color::rgb(0x1c1c1e)))
    );

    environment.dark.set(false);
  }

  #[test]
  fn an_inner_scope_redefines() {
    let outer = Token::new(CARD, Color::WHITE);
    let inner = Token::new(CARD, Color::BLACK);
    let mut first = Definitions::new();
    let mut second = Definitions::new();

    first.add(&outer);
    second.add(&inner);

    let tokens = Tokens::root()
      .scope(Signal::new(Rc::new(first)))
      .scope(Signal::new(Rc::new(second)));

    assert_eq!(
      tokens.resolve("card"),
      Some(TokenValue::Color(Color::BLACK))
    );
  }
}
