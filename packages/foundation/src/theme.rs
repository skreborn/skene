//! The one theme view: a scope defining a token set beneath it; and reading the set in
//! force.

use std::{any::TypeId, marker::PhantomData};

use skene_reactive::{Effect, Get, Memo, Owner, With, provide_context, use_context};

use crate::{
  AnyKind,
  Definitions,
  NodeId,
  NodeSpec,
  Realization,
  Scene,
  TokenScope,
  TokenSet,
  View,
  Wraps,
  empty,
  realize::{ChildBuilder, Source},
};

/// Defines the token set `S` for everything beneath it.
///
/// Realizes one layout-invisible scope node, provides `Memo<S>` through context so
/// [`ThemedIn::current`] and [`ThemedIn::watch`] read it, and rewrites the scope's
/// definitions when the palette changes: no consumer is touched and nothing rebuilds.
/// Nesting a theme redefines beneath it. The child is a builder closure
/// [`set_child_builder`](Self::set_child_builder) supplies, built under the owner the
/// context is provided on; without it the theme realizes no scope and renders nothing.
#[must_use]
pub struct Theme<P: Realization, S: TokenSet<P::TokenValue>> {
  palette: Source<S>,
  child: Option<ChildBuilder<P>>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization, S: TokenSet<P::TokenValue>> std::fmt::Debug for Theme<P, S> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Theme").finish_non_exhaustive()
  }
}

impl<P: Realization, S: TokenSet<P::TokenValue>> Theme<P, S> {
  /// A theme defining `palette`.
  #[inline(always)]
  pub fn new(palette: S) -> Self {
    Self {
      palette: Source::Fixed(palette),
      child: None,
      platform: PhantomData,
    }
  }

  /// A theme defining the palette `palette` returns, recomputed whenever a signal it
  /// reads changes.
  pub fn derive(palette: impl Fn() -> S + 'static) -> Self {
    Self {
      palette: Source::derive(palette),
      child: None,
      platform: PhantomData,
    }
  }
}

impl<P: Realization, S: TokenSet<P::TokenValue>> Theme<P, S> {
  /// Sets the child, built by `child` under the theme.
  pub fn set_child_builder<V: View<P>>(mut self, child: impl FnOnce() -> V + 'static) -> Self {
    self.child = Some(ChildBuilder::new(child));

    self
  }
}

impl<P: Realization, S: TokenSet<P::TokenValue>> AnyKind for Theme<P, S> {}

impl<P: Realization, S: TokenSet<P::TokenValue>> View<P> for Theme<P, S> {
  fn build(self) -> NodeId {
    let Some(child) = self.child
    else {
      return empty::<P>().build();
    };
    let spec = NodeSpec::scope();
    let scope = Scene::<P>::realize(&spec, None);

    let owner = Owner::current().child();
    let palette = self.palette;

    owner.enter(|| {
      let memo = match palette {
        Source::Fixed(palette) => Memo::new(move |_| palette.clone()),
        Source::Derived(palette) => Memo::new(move |_| palette()),
      };

      provide_context(memo);

      Effect::render(move |_: Option<()>| {
        let definitions = memo.with(Definitions::of);

        Scene::<P>::with(|scene| scene.define_tokens(TokenScope::Node(scope), definitions));
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
/// once per set, so a consumer below no theme still resolves the tokens. Every
/// [`TokenSet`] with a default has it.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a token set with a default on this platform",
  label = "a palette read without a theme is a `TokenSet` for the platform's token value that implements `Default`"
)]
pub trait ThemedIn<P: Realization>: TokenSet<P::TokenValue> + Default {
  /// The set the nearest theme provides, or the default without one.
  fn current() -> Self;

  /// The nearest theme's set as a memo, or a memo of the default without one.
  fn watch() -> Memo<Self>;

  /// Lays the set's default definitions over the root of `P`'s scene, once per set, so
  /// a node below no theme resolves its tokens.
  fn define_root();
}

#[diagnostic::do_not_recommend]
impl<P: Realization, S: TokenSet<P::TokenValue> + Default> ThemedIn<P> for S {
  #[inline(always)]
  fn current() -> Self {
    use_context::<Memo<Self>>().map_or_else(
      || {
        <Self as ThemedIn<P>>::define_root();

        Self::default()
      },
      |memo| memo.get(),
    )
  }

  #[inline(always)]
  fn watch() -> Memo<Self> {
    use_context::<Memo<Self>>().unwrap_or_else(|| {
      <Self as ThemedIn<P>>::define_root();

      Memo::new(|_| Self::default())
    })
  }

  fn define_root() {
    let set = TypeId::of::<Self>();

    if Scene::<P>::with(|scene| scene.defines_root_set(set)) {
      return;
    }

    let definitions = Definitions::of(&Self::default());

    Scene::<P>::with(|scene| scene.define_root_set(set, definitions));
  }
}

/// What a facade's [`Theme`] carries: its constructors and its child.
pub trait ThemeBase<P: Realization, S: TokenSet<P::TokenValue>>:
  Wraps<Inner = Theme<P, S>>
{
  /// A theme defining `palette`.
  #[must_use]
  fn new(palette: S) -> Self {
    Self::wrap(Theme::new(palette))
  }

  /// A theme defining the palette `palette` returns, recomputed whenever a signal it
  /// reads changes.
  #[must_use]
  fn derive(palette: impl Fn() -> S + 'static) -> Self {
    Self::wrap(Theme::derive(palette))
  }

  /// Sets the child, built by `child` under the theme.
  #[must_use]
  fn set_child_builder<V: View<P>>(self, child: impl FnOnce() -> V + 'static) -> Self {
    Self::wrap(self.into_inner().set_child_builder(child))
  }
}
