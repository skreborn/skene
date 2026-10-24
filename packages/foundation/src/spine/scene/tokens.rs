//! What a node defines for its subtree's resolution: token definitions, resolved to
//! the nearest defining ancestor, and container names.

use std::any::TypeId;

use rustc_hash::{FxHashMap, FxHashSet};

use super::{Mirror, Scene};
use crate::spine::{ContainerName, Definitions, NodeId, Realization, Resolved, Scalar, TokenName};

/// Where tokens are defined: on a node, for its subtree, or at the root, the definitions
/// every node falls back to.
///
/// A token in force at a node is defined on its nearest defining ancestor, else at the
/// root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenScope {
  /// On a node, for its subtree.
  Node(NodeId),
  /// At the root, for every node.
  Root,
}

/// Where each token a node looked up under one definitions generation is defined;
/// `None` for a token nothing defines.
pub(super) struct TokenCache {
  generation: u64,
  scopes: FxHashMap<TokenName, Option<TokenScope>>,
}

impl TokenCache {
  /// Where the cache found `name` defined under `generation`, while it holds that.
  #[expect(
    clippy::option_option,
    reason = "a scope is absent until kept, and one kept may be nowhere"
  )]
  pub(super) fn scope(&self, generation: u64, name: TokenName) -> Option<Option<TokenScope>> {
    (self.generation == generation)
      .then(|| self.scopes.get(&name).copied())
      .flatten()
  }

  /// Keeps where `name` is defined under `generation` in `cache`, forgetting what an
  /// older generation kept.
  pub(super) fn keep(
    cache: &mut Option<Box<Self>>,
    generation: u64,
    name: TokenName,
    scope: Option<TokenScope>,
  ) {
    let cache = cache.get_or_insert_with(|| {
      Box::new(Self {
        generation,
        scopes: FxHashMap::default(),
      })
    });

    if cache.generation != generation {
      cache.generation = generation;
      cache.scopes.clear();
    }

    cache.scopes.insert(name, scope);
  }
}

impl TokenScope {
  /// Where `name` is defined for a node: the nearest of `chain`, the node and its
  /// ancestors nearest first with what each defines, to define it, else `root`, the
  /// definitions every node falls back to; `None` where nothing does.
  pub(in crate::spine) fn nearest<'a, V: 'a>(
    chain: impl IntoIterator<Item = (NodeId, Option<&'a Definitions<V>>)>,
    root: &Definitions<V>,
    name: TokenName,
  ) -> Option<Self> {
    chain
      .into_iter()
      .find(|(_, definitions)| {
        definitions.is_some_and(|definitions| definitions.get(name).is_some())
      })
      .map(|(node, _)| Self::Node(node))
      .or_else(|| root.get(name).map(|_| Self::Root))
  }
}

/// What tokens resolved to where they are defined, and the font size in force there,
/// under one stamp: the definitions generation, then the resolution source's.
pub(super) struct ResolvedTokens<N: Scalar> {
  stamp: (u64, u64),
  values: FxHashMap<(TokenScope, TokenName), Option<Resolved<N>>>,
  font_sizes: FxHashMap<TokenScope, f32>,
}

impl<N: Scalar> Default for ResolvedTokens<N> {
  fn default() -> Self {
    Self {
      stamp: (0, 0),
      values: FxHashMap::default(),
      font_sizes: FxHashMap::default(),
    }
  }
}

impl<N: Scalar> ResolvedTokens<N> {
  /// These resolutions under `stamp`, emptied where it moved since they were kept.
  pub(super) fn under(&mut self, stamp: (u64, u64)) -> &mut Self {
    if self.stamp != stamp {
      self.stamp = stamp;
      self.values.clear();
      self.font_sizes.clear();
    }

    self
  }
}

/// The definitions a scene holds apart from its nodes': the root's every node falls back
/// to, the token sets laid over them, and a generation that moves whenever what a node
/// resolves a token to may have changed.
#[must_use]
pub(super) struct TokenState<V> {
  root: Definitions<V>,
  root_sets: FxHashSet<TypeId>,
  generation: u64,
  #[cfg(debug_assertions)]
  kinds: FxHashMap<TokenName, std::mem::Discriminant<V>>,
}

impl<V: Clone + PartialEq> TokenState<V> {
  /// No definitions, at the first generation.
  pub(super) fn new() -> Self {
    Self {
      root: Definitions::from(Vec::new()),
      root_sets: FxHashSet::default(),
      generation: 0,
      #[cfg(debug_assertions)]
      kinds: FxHashMap::default(),
    }
  }

  /// The generation of the definitions in force.
  #[inline(always)]
  pub(super) const fn generation(&self) -> u64 {
    self.generation
  }

  /// Moves the generation, for a change to the tree the definitions hang off.
  #[inline(always)]
  pub(super) const fn touch(&mut self) {
    self.generation += 1;
  }

  /// The tokens every node falls back to.
  #[inline(always)]
  pub(super) const fn root(&self) -> &Definitions<V> {
    &self.root
  }

  /// Panics, in a debug build, when a token of `definitions` was defined before as
  /// another kind: one name is one kind wherever it is defined.
  #[cfg(debug_assertions)]
  pub(super) fn check_kinds(&mut self, definitions: &Definitions<V>) {
    for (name, value) in definitions.iter() {
      let kind = std::mem::discriminant(value);
      let first = *self.kinds.entry(name).or_insert(kind);

      assert!(first == kind, "the token `{name}` is defined as two kinds");
    }
  }

  /// Puts `definitions` in `held`, what a node defines, moving the generation and
  /// returning them to mirror; definitions equal to those held change nothing.
  pub(super) fn define(
    &mut self,
    held: &mut Option<Box<Definitions<V>>>,
    definitions: Definitions<V>,
  ) -> Option<Definitions<V>> {
    if held.as_deref() == Some(&definitions) {
      return None;
    }

    *held = Some(Box::new(definitions.clone()));
    self.generation += 1;

    Some(definitions)
  }

  /// Replaces the tokens every node falls back to, moving the generation and returning
  /// them to mirror; definitions equal to those in force change nothing.
  pub(super) fn define_root(&mut self, definitions: Definitions<V>) -> Option<Definitions<V>> {
    if self.root == definitions {
      return None;
    }

    self.root = definitions.clone();
    self.generation += 1;

    Some(definitions)
  }

  /// Whether the definitions of `set` were laid over the root.
  #[inline(always)]
  #[must_use]
  pub(super) fn has_root_set(&self, set: TypeId) -> bool {
    self.root_sets.contains(&set)
  }

  /// Notes that the definitions of `set` are laid over the root, returning whether they
  /// were not before.
  pub(super) fn claim_root_set(&mut self, set: TypeId) -> bool {
    self.root_sets.insert(set)
  }

  /// Takes the tokens every node falls back to, leaving none.
  pub(super) fn take_root(&mut self) -> Definitions<V> {
    std::mem::replace(&mut self.root, Definitions::from(Vec::new()))
  }
}

impl<P: Realization> Scene<P> {
  /// Defines tokens on a scope, replacing what the scope defined before: on a node for
  /// its subtree, or at the root, the tokens every node falls back to; definitions equal
  /// to those in force change nothing.
  ///
  /// # Panics
  ///
  /// Panics, in a debug build, when a token was defined before as another kind.
  pub fn define_tokens(&mut self, scope: TokenScope, definitions: Definitions<P::TokenValue>) {
    #[cfg(debug_assertions)]
    self.tokens.check_kinds(&definitions);

    let definitions = match scope {
      TokenScope::Node(node) => {
        let Some(record) = self.nodes.get_mut(node)
        else {
          return;
        };

        self.tokens.define(&mut record.definitions, definitions)
      }
      TokenScope::Root => self.tokens.define_root(definitions),
    };

    if let Some(definitions) = definitions {
      self
        .mirrors
        .push(Mirror::DefineTokens { scope, definitions });
    }
  }

  /// Whether the definitions of `set` were laid over the root.
  #[inline(always)]
  #[must_use]
  pub fn defines_root_set(&self, set: TypeId) -> bool {
    self.tokens.has_root_set(set)
  }

  /// Lays `set`'s `definitions` over the tokens every node falls back to, once per set: a
  /// later call for the same set does nothing and returns `false`.
  ///
  /// [`defines_root_set`](Self::defines_root_set) tells a caller whether the set's
  /// definitions are wanted before it builds them, outside the scene's borrow.
  pub fn define_root_set(&mut self, set: TypeId, definitions: Definitions<P::TokenValue>) -> bool {
    if !self.tokens.claim_root_set(set) {
      return false;
    }

    let mut root = self.tokens.take_root();

    root.overlay(definitions);
    self.define_tokens(TokenScope::Root, root);

    true
  }

  /// The tokens every node falls back to.
  #[inline(always)]
  pub const fn root_definitions(&self) -> &Definitions<P::TokenValue> {
    self.tokens.root()
  }

  /// The definition of `name` in force at `node`: the nearest defining ancestor's, else
  /// the root's.
  pub fn resolve_token(&mut self, node: NodeId, name: TokenName) -> Option<P::TokenValue> {
    self
      .resolve_scoped_token(node, name)
      .map(|(_, value)| value)
  }

  /// The definition of `name` in force at `node`, with the node defining it: the
  /// nearest defining ancestor, else `None` for the root.
  pub fn resolve_scoped_token(
    &mut self,
    node: NodeId,
    name: TokenName,
  ) -> Option<(Option<NodeId>, P::TokenValue)> {
    let scope = self.token_scope(node, name)?;
    let definition = self.definition_on(scope, name)?;

    Some((
      match scope {
        TokenScope::Node(node) => Some(node),
        TokenScope::Root => None,
      },
      definition,
    ))
  }

  /// Where `name` in force at `node` is defined; `None` where nothing defines it.
  ///
  /// Cached per node until a definition or the tree changes.
  pub(in crate::spine) fn token_scope(
    &mut self,
    node: NodeId,
    name: TokenName,
  ) -> Option<TokenScope> {
    let generation = self.tokens.generation();
    let record = self.record(node)?;

    if let Some(scope) = record
      .cache
      .as_ref()
      .and_then(|cache| cache.scope(generation, name))
    {
      return scope;
    }

    let chain = std::iter::successors(Some(node), |visited| self.parent(*visited)).map(|visited| {
      (
        visited,
        self
          .record(visited)
          .and_then(|record| record.definitions.as_deref()),
      )
    });
    let found = TokenScope::nearest(chain, self.tokens.root(), name);

    TokenCache::keep(&mut self.record_mut(node)?.cache, generation, name, found);

    found
  }

  /// The definition `scope` gives `name`.
  pub(in crate::spine) fn definition_on(
    &self,
    scope: TokenScope,
    name: TokenName,
  ) -> Option<P::TokenValue> {
    match scope {
      TokenScope::Node(node) => self.record(node)?.definitions.as_ref()?.get(name).cloned(),
      TokenScope::Root => self.tokens.root().get(name).cloned(),
    }
  }

  /// What `name` resolved to where `scope` defines it, while neither the definitions
  /// nor the source's `generation` moved since.
  #[expect(
    clippy::option_option,
    reason = "a resolution is absent until kept, and one kept may have come to nothing"
  )]
  pub(in crate::spine) fn resolved_token(
    &mut self,
    scope: TokenScope,
    name: TokenName,
    generation: u64,
  ) -> Option<Option<Resolved<P::Scalar>>> {
    self
      .resolved_under(generation)
      .values
      .get(&(scope, name))
      .cloned()
  }

  /// Keeps what `name` resolved to where `scope` defines it, under the source's
  /// `generation`.
  pub(in crate::spine) fn keep_resolved_token(
    &mut self,
    scope: TokenScope,
    name: TokenName,
    generation: u64,
    value: Option<Resolved<P::Scalar>>,
  ) {
    self
      .resolved_under(generation)
      .values
      .insert((scope, name), value);
  }

  /// The font size in force where `scope` defines the size token, while neither the
  /// definitions nor the source's `generation` moved since.
  pub(in crate::spine) fn resolved_font_size(
    &mut self,
    scope: TokenScope,
    generation: u64,
  ) -> Option<f32> {
    self
      .resolved_under(generation)
      .font_sizes
      .get(&scope)
      .copied()
  }

  /// Keeps the font size in force where `scope` defines the size token, under the
  /// source's `generation`.
  pub(in crate::spine) fn keep_font_size(&mut self, scope: TokenScope, generation: u64, size: f32) {
    self
      .resolved_under(generation)
      .font_sizes
      .insert(scope, size);
  }

  /// The resolved tokens, emptied where the definitions or the source's `generation`
  /// moved since they were kept.
  fn resolved_under(&mut self, generation: u64) -> &mut ResolvedTokens<P::Scalar> {
    self
      .resolved_tokens
      .under((self.tokens.generation(), generation))
  }

  /// Names `node` for container queries against its size.
  #[inline(always)]
  pub fn set_container(&mut self, node: NodeId, name: ContainerName) {
    if let Some(record) = self.record_mut(node) {
      record.container = Some(name);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{ResolvedTokens, TokenCache, TokenScope, TokenState};
  use crate::spine::{Definitions, NodeId, Resolved, TokenName};

  /// A token value of two kinds.
  #[derive(Debug, Clone, PartialEq)]
  enum Value {
    Color(f32),
    Number(f32),
  }

  fn definitions(name: &'static str, value: Value) -> Definitions<Value> {
    Definitions::from(vec![(TokenName::new(name), value)])
  }

  /// One name is one kind: redefining a token as another kind panics in a debug build,
  /// where redefining it as the same kind does not.
  #[test]
  #[cfg(debug_assertions)]
  #[should_panic(expected = "the token `ink` is defined as two kinds")]
  fn a_token_is_one_kind() {
    let mut tokens = TokenState::new();

    tokens.check_kinds(&definitions("ink", Value::Color(0.1)));
    tokens.check_kinds(&definitions("ink", Value::Color(0.9)));
    tokens.check_kinds(&definitions("ink", Value::Number(1.)));
  }

  /// A token resolves to the nearest defining ancestor, then to the root; the answer
  /// is cached under its generation, and a resolution under its stamp and scope.
  #[test]
  fn tokens_resolve_to_the_nearest_definer_and_cache() {
    let card = TokenName::new("card");
    let [outer, inner, leaf] = std::array::from_fn(NodeId::plain);
    let root = definitions("card", Value::Color(0.));
    let at_outer = definitions("card", Value::Color(0.5));
    let at_inner = definitions("ink", Value::Color(1.));

    assert_eq!(
      TokenScope::nearest(
        [
          (leaf, None),
          (inner, Some(&at_inner)),
          (outer, Some(&at_outer))
        ],
        &root,
        card
      ),
      Some(TokenScope::Node(outer)),
      "an ancestor defining other tokens is passed over"
    );
    assert_eq!(
      TokenScope::nearest(
        [(inner, Some(&at_outer)), (outer, Some(&at_outer))],
        &root,
        card
      ),
      Some(TokenScope::Node(inner))
    );
    assert_eq!(
      TokenScope::nearest([(leaf, None)], &root, card),
      Some(TokenScope::Root)
    );
    assert_eq!(
      TokenScope::nearest([(leaf, Some(&at_outer))], &root, TokenName::new("missing")),
      None
    );

    let mut cache = None;

    TokenCache::keep(&mut cache, 1, card, Some(TokenScope::Node(outer)));

    let held = cache.as_deref().unwrap();

    assert_eq!(held.scope(1, card), Some(Some(TokenScope::Node(outer))));
    assert_eq!(held.scope(1, TokenName::new("ink")), None);
    assert_eq!(held.scope(2, card), None, "a moved generation misses");

    TokenCache::keep(&mut cache, 2, TokenName::new("ink"), None);

    let held = cache.as_deref().unwrap();

    assert_eq!(held.scope(2, TokenName::new("ink")), Some(None));
    assert_eq!(held.scope(2, card), None, "a moved generation forgets");

    let mut resolved = ResolvedTokens::<f32>::default();
    let value = Some(Resolved::Number(1.));

    resolved
      .under((1, 1))
      .values
      .insert((TokenScope::Node(outer), card), value.clone());
    resolved
      .under((1, 1))
      .font_sizes
      .insert(TokenScope::Root, 16.);

    assert_eq!(
      resolved
        .under((1, 1))
        .values
        .get(&(TokenScope::Node(outer), card)),
      Some(&value)
    );
    assert_eq!(
      resolved.under((1, 1)).values.get(&(TokenScope::Root, card)),
      None
    );
    assert!(
      resolved.under((1, 2)).values.is_empty(),
      "a moved source's stamp forgets"
    );
    assert!(resolved.under((1, 2)).font_sizes.is_empty());
  }

  /// Redefining tokens as the ones in force changes nothing: nothing to mirror, and the
  /// generation cached resolutions are kept under stays.
  #[test]
  fn an_equal_redefinition_changes_nothing() {
    let mut tokens = TokenState::new();
    let mut held = None;

    assert!(
      tokens
        .define_root(definitions("ink", Value::Color(0.1)))
        .is_some()
    );
    assert!(
      tokens
        .define(&mut held, definitions("gap", Value::Number(0.5)))
        .is_some()
    );

    let generation = tokens.generation();

    assert_eq!(
      tokens.define_root(definitions("ink", Value::Color(0.1))),
      None
    );
    assert_eq!(
      tokens.define(&mut held, definitions("gap", Value::Number(0.5))),
      None
    );
    assert_eq!(tokens.generation(), generation);
    assert_eq!(
      tokens.define(&mut held, definitions("gap", Value::Number(0.6))),
      Some(definitions("gap", Value::Number(0.6)))
    );
    assert_eq!(
      held.as_deref(),
      Some(&definitions("gap", Value::Number(0.6)))
    );
    assert_ne!(tokens.generation(), generation);
  }
}
