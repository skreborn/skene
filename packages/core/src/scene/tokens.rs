//! What a node defines for its subtree's resolution: token definitions, resolved to
//! the nearest defining ancestor, and container names.

use std::any::TypeId;

use rustc_hash::FxHashMap;

use super::{Mirror, Scene};
use crate::{ContainerName, Definitions, NodeId, Platform, TokenName};

/// The token definitions a node looked up under one definitions generation, each with
/// the node defining it.
pub(super) struct TokenCache<V> {
  pub(super) generation: u64,
  pub(super) values: FxHashMap<TokenName, Option<(Option<NodeId>, V)>>,
}

impl<P: Platform> Scene<P> {
  /// Panics, in a debug build, when a token of `definitions` was defined before as
  /// another kind: one name is one kind wherever it is defined.
  #[cfg(debug_assertions)]
  fn check_kinds(&mut self, definitions: &Definitions<P::TokenValue>) {
    for (name, value) in definitions.iter() {
      let kind = std::mem::discriminant(value);
      let first = *self.token_kinds.entry(name).or_insert(kind);

      assert!(first == kind, "the token `{name}` is defined as two kinds");
    }
  }

  /// Defines tokens on `node`, replacing what it defined before.
  ///
  /// # Panics
  ///
  /// Panics, in a debug build, when a token was defined before as another kind.
  pub fn define_tokens(&mut self, node: NodeId, definitions: Definitions<P::TokenValue>) {
    #[cfg(debug_assertions)]
    self.check_kinds(&definitions);

    let Some(record) = self.record_mut(node)
    else {
      return;
    };

    record.definitions = Some(Box::new(definitions.clone()));
    self.mirrors.push(Mirror::DefineTokens {
      scope: node,
      definitions,
    });
    self.definitions_generation += 1;
  }

  /// Defines the tokens every node falls back to, replacing the previous set.
  ///
  /// # Panics
  ///
  /// Panics, in a debug build, when a token was defined before as another kind.
  pub fn define_root_tokens(&mut self, definitions: Definitions<P::TokenValue>) {
    #[cfg(debug_assertions)]
    self.check_kinds(&definitions);

    self
      .mirrors
      .push(Mirror::DefineRootTokens(definitions.clone()));
    self.root_definitions = definitions;
    self.definitions_generation += 1;
  }

  /// Whether the definitions of `set` were laid over the root.
  #[inline(always)]
  #[must_use]
  pub fn defines_root_set(&self, set: TypeId) -> bool {
    self.root_sets.contains(&set)
  }

  /// Lays `set`'s `definitions` over the tokens every node falls back to, once per set: a
  /// later call for the same set does nothing and returns `false`.
  ///
  /// [`defines_root_set`](Self::defines_root_set) tells a caller whether the set's
  /// definitions are wanted before it builds them, outside the scene's borrow.
  pub fn define_root_tokens_once(
    &mut self,
    set: TypeId,
    definitions: Definitions<P::TokenValue>,
  ) -> bool {
    if !self.root_sets.insert(set) {
      return false;
    }

    let mut root = std::mem::replace(&mut self.root_definitions, Definitions::from(Vec::new()));

    root.overlay(definitions);
    self.define_root_tokens(root);

    true
  }

  /// The tokens every node falls back to.
  #[inline(always)]
  pub const fn root_definitions(&self) -> &Definitions<P::TokenValue> {
    &self.root_definitions
  }

  /// The definition of `name` in force at `node`: the nearest defining ancestor's, else
  /// the root's.
  ///
  /// Cached per node until a definition or the tree changes.
  pub fn resolve_token(&mut self, node: NodeId, name: TokenName) -> Option<P::TokenValue> {
    self
      .resolve_scoped_token(node, name)
      .map(|(_, value)| value)
  }

  /// The definition of `name` in force at `node`, with the node defining it: the
  /// nearest defining ancestor, else `None` for the root.
  ///
  /// Cached per node until a definition or the tree changes.
  pub fn resolve_scoped_token(
    &mut self,
    node: NodeId,
    name: TokenName,
  ) -> Option<(Option<NodeId>, P::TokenValue)> {
    let generation = self.definitions_generation;
    let cached = self
      .record(node)
      .and_then(|record| record.cache.as_ref())
      .filter(|cache| cache.generation == generation)
      .and_then(|cache| cache.values.get(&name));

    if let Some(value) = cached {
      return value.clone();
    }

    let mut current = Some(node);
    let mut found = None;

    while let Some(visited) = current {
      let record = self.record(visited)?;

      if let Some(value) = record
        .definitions
        .as_ref()
        .and_then(|definitions| definitions.get(name))
      {
        found = Some((Some(visited), value.clone()));

        break;
      }

      current = record.parent;
    }

    let found = found.or_else(|| {
      self
        .root_definitions
        .get(name)
        .map(|value| (None, value.clone()))
    });
    let record = self.record_mut(node)?;
    let cache = record.cache.get_or_insert_with(|| {
      Box::new(TokenCache {
        generation,
        values: FxHashMap::default(),
      })
    });

    if cache.generation != generation {
      cache.generation = generation;
      cache.values.clear();
    }

    cache.values.insert(name, found.clone());

    found
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
  use crate::{
    Color,
    Definition,
    Definitions,
    TokenName,
    testing::{Call, Erased, TestBox, TestPlatform, TestScene},
  };

  fn color(value: f32) -> Erased {
    Erased::Color(Definition::from(Color::srgb(value, value, value)))
  }

  fn definitions(name: &'static str, value: Erased) -> Definitions<Erased> {
    Definitions::from(vec![(TokenName::new(name), value)])
  }

  /// One name is one kind: redefining a token as another kind panics in a debug build,
  /// where redefining it as the same kind does not.
  #[test]
  #[cfg(debug_assertions)]
  #[should_panic(expected = "the token `ink` is defined as two kinds")]
  fn a_token_is_one_kind() {
    TestPlatform::reset();

    TestScene::with(|scene| {
      scene.define_root_tokens(definitions("ink", color(0.1)));
      scene.define_root_tokens(definitions("ink", color(0.9)));
      scene.define_root_tokens(definitions("ink", Erased::Number(Definition::from(1.))));
    });
  }

  /// A token resolves to the nearest defining ancestor, then to the root; the answer
  /// is cached and a redefinition invalidates it.
  #[test]
  fn tokens_resolve_to_the_nearest_definer_and_cache() {
    TestPlatform::reset();

    let card = TokenName::new("card");
    let outer = TestBox::node();
    let inner = TestBox::node();
    let leaf = TestBox::node();
    let stranger = TestBox::node();

    TestScene::with(|scene| {
      scene.attach(outer, inner, None);
      scene.attach(inner, leaf, None);
    });
    TestPlatform::take_log();
    TestScene::with(|scene| {
      scene.define_root_tokens(definitions("card", color(0.)));
      scene.define_tokens(outer, definitions("card", color(0.5)));
    });

    assert_eq!(
      TestPlatform::take_log(),
      [Call::DefineRootTokens(1), Call::DefineTokens(outer, 1)]
    );
    assert_eq!(
      TestScene::with(|scene| scene.resolve_token(leaf, card)),
      Some(color(0.5))
    );
    assert_eq!(
      TestScene::with(|scene| scene.resolve_token(stranger, card)),
      Some(color(0.))
    );
    assert_eq!(
      TestScene::with(|scene| scene.resolve_token(leaf, TokenName::new("missing"))),
      None
    );

    let generation = TestScene::with(|scene| scene.definitions_generation);

    assert_eq!(
      TestScene::with(|scene| {
        let cache = scene.record(leaf).unwrap().cache.as_ref().unwrap();

        (cache.generation, cache.values.get(&card).cloned())
      }),
      (generation, Some(Some((Some(outer), color(0.5)))))
    );
    assert_eq!(
      TestScene::with(|scene| scene.resolve_scoped_token(stranger, card)),
      Some((None, color(0.)))
    );

    TestScene::with(|scene| scene.define_tokens(inner, definitions("card", color(1.))));

    assert_eq!(
      TestScene::with(|scene| scene.resolve_token(leaf, card)),
      Some(color(1.))
    );
    assert_eq!(
      TestScene::with(|scene| scene.resolve_token(inner, card)),
      Some(color(1.))
    );
    assert_eq!(
      TestScene::with(|scene| scene.resolve_token(outer, card)),
      Some(color(0.5))
    );

    TestScene::with(|scene| scene.detach(inner, leaf));

    assert_eq!(
      TestScene::with(|scene| scene.resolve_token(leaf, card)),
      Some(color(0.))
    );
  }
}
