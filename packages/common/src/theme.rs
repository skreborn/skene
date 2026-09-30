//! The one theme view: a scope defining a token set beneath it; and reading the set in
//! force.

use std::{any::TypeId, marker::PhantomData};

use skene_core::{
  AnyKind,
  Definitions,
  IntoView,
  NoChild,
  NodeId,
  NodeKind,
  Platform,
  Scene,
  TokenSet,
  View,
};
use skene_reactive::{Effect, Get, Memo, Owner, With, provide_context, use_context};

use crate::realize::{ChildBuilder, Source};

/// Defines the token set `S` for everything beneath it.
///
/// Realizes one layout-invisible scope node, provides `Memo<S>` through context so
/// [`ThemedIn::current`] and [`ThemedIn::watch`] read it, and rewrites the scope's
/// definitions when the palette changes: no consumer is touched and nothing rebuilds.
/// Nesting a theme redefines beneath it. Holds [`NoChild`] until
/// [`set_child_builder`](Self::set_child_builder) supplies the child; the child is a
/// builder closure because it is built under the owner the context is provided on.
#[must_use]
pub struct Theme<P: Platform, S: TokenSet<P::TokenValue>, C = NoChild> {
  palette: Source<S>,
  child: C,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform, S: TokenSet<P::TokenValue>, C> std::fmt::Debug for Theme<P, S, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Theme").finish_non_exhaustive()
  }
}

impl<P: Platform, S: TokenSet<P::TokenValue>> Theme<P, S> {
  /// A theme defining `palette`.
  #[inline(always)]
  pub fn new(palette: S) -> Self {
    Self {
      palette: Source::Fixed(palette),
      child: NoChild,
      platform: PhantomData,
    }
  }

  /// A theme defining the palette `palette` returns, recomputed whenever a signal it
  /// reads changes.
  #[inline(always)]
  pub fn derive(palette: impl Fn() -> S + 'static) -> Self {
    Self {
      palette: Source::derive(palette),
      child: NoChild,
      platform: PhantomData,
    }
  }
}

impl<P: Platform, S: TokenSet<P::TokenValue>> Theme<P, S> {
  /// Sets the child, built by `child` under the theme.
  #[inline(always)]
  pub fn set_child_builder<V: IntoView<P>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Theme<P, S, ChildBuilder<P>> {
    Theme {
      palette: self.palette,
      child: ChildBuilder::new(child),
      platform: PhantomData,
    }
  }
}

impl<P: Platform, S: TokenSet<P::TokenValue>, C> AnyKind for Theme<P, S, C> {}

impl<P: Platform, S: TokenSet<P::TokenValue>> View<P> for Theme<P, S, ChildBuilder<P>> {
  fn build(self) -> NodeId {
    let scope = Scene::<P>::with(|scene| scene.create(NodeKind::Scope));

    P::create_scope(scope);

    let owner = Owner::current().child();
    let child = self.child;
    let palette = self.palette;

    owner.enter(|| {
      let memo = match palette {
        Source::Fixed(palette) => Memo::new(move |_| palette.clone()),
        Source::Derived(palette) => Memo::new(move |_| palette()),
      };

      provide_context(memo);

      Effect::render(move |_: Option<()>| {
        let definitions = memo.with(Definitions::of);

        Scene::<P>::with(|scene| scene.define_tokens(scope, definitions));
      });

      let child = child.build();

      Scene::<P>::with(|scene| scene.attach(scope, child, None));
    });

    scope
  }
}

/// A token set read from the theme in force on platform `P`, its default standing in
/// and defined at the scene's root where no theme provides it.
///
/// A theme provides `Memo<Self>` through context. On a miss the set's default is
/// returned and its definitions are laid over the root definitions of `P`'s scene,
/// once per set, so a consumer below no theme still resolves the tokens. Core's
/// [`Themed`](skene_core::Themed) reads the context alone and defines nothing: a
/// root definition lives in one platform's scene, so the trait registering it is bound
/// on the platform. Every [`TokenSet`] with a default has it.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a token set with a default on this platform",
  label = "a palette read without a theme is a `TokenSet` for the platform's token value that implements `Default`"
)]
pub trait ThemedIn<P: Platform>: TokenSet<P::TokenValue> + Default {
  /// The set the nearest theme provides, or the default without one.
  fn current() -> Self;

  /// The nearest theme's set as a memo, or a memo of the default without one.
  fn watch() -> Memo<Self>;
}

#[diagnostic::do_not_recommend]
impl<P: Platform, S: TokenSet<P::TokenValue> + Default> ThemedIn<P> for S {
  #[inline(always)]
  fn current() -> Self {
    use_context::<Memo<Self>>().map_or_else(
      || {
        define_root::<P, Self>();

        Self::default()
      },
      |memo| memo.get(),
    )
  }

  #[inline(always)]
  fn watch() -> Memo<Self> {
    use_context::<Memo<Self>>().unwrap_or_else(|| {
      define_root::<P, Self>();

      Memo::new(|_| Self::default())
    })
  }
}

/// Lays `S`'s default definitions over the root of `P`'s scene, once per set, built
/// outside the scene's borrow.
fn define_root<P: Platform, S: TokenSet<P::TokenValue> + Default>() {
  let set = TypeId::of::<S>();

  if Scene::<P>::with(|scene| scene.defines_root_set(set)) {
    return;
  }

  let definitions = Definitions::of(&S::default());

  Scene::<P>::with(|scene| scene.define_root_tokens_once(set, definitions));
}
