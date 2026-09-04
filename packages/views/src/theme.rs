//! The one theme view: a scope defining a token set beneath it; and reading the set in
//! force.

use std::{any::TypeId, marker::PhantomData};

use skene_core::next::{
  AnyKind,
  Definitions,
  Driver,
  IntoView,
  NoChild,
  NodeId,
  NodeKind,
  Scene,
  TokenSet,
  View,
};
use skene_reactive::{Effect, Get, Memo, Owner, With, provide_context, use_context};

use crate::realize::ChildBuilder;

/// A palette: fixed, or recomputed from the signals it reads.
enum Palette<P> {
  Fixed(P),
  Derived(Box<dyn Fn() -> P>),
}

/// Defines the token set `P` for everything beneath it.
///
/// Realizes one layout-invisible scope node, provides `Memo<P>` through context so
/// [`ThemedIn::current`] and [`ThemedIn::watch`] read it, and rewrites the scope's
/// definitions when the palette changes: no consumer is touched and nothing rebuilds.
/// Nesting a theme redefines beneath it. Holds [`NoChild`] until
/// [`set_child_builder`](Self::set_child_builder) supplies the child; the child is a
/// builder closure because it is built under the owner the context is provided on.
#[must_use]
pub struct Theme<D: Driver, P: TokenSet<D::TokenValue>, C = NoChild> {
  palette: Palette<P>,
  child: C,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver, P: TokenSet<D::TokenValue>> Theme<D, P> {
  /// A theme defining `palette`.
  #[inline(always)]
  pub fn new(palette: P) -> Self {
    Self {
      palette: Palette::Fixed(palette),
      child: NoChild,
      driver: PhantomData,
    }
  }

  /// A theme defining the palette `palette` returns, recomputed whenever a signal it
  /// reads changes.
  #[inline(always)]
  pub fn derive(palette: impl Fn() -> P + 'static) -> Self {
    Self {
      palette: Palette::Derived(Box::new(palette)),
      child: NoChild,
      driver: PhantomData,
    }
  }
}

impl<D: Driver, P: TokenSet<D::TokenValue>, C> Theme<D, P, C> {
  /// Sets the child, built by `child` under the theme.
  #[inline(always)]
  pub fn set_child_builder<V: IntoView<D>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Theme<D, P, ChildBuilder<D>> {
    Theme {
      palette: self.palette,
      child: ChildBuilder::new(child),
      driver: PhantomData,
    }
  }
}

impl<D: Driver, P: TokenSet<D::TokenValue>, C> AnyKind for Theme<D, P, C> {}

impl<D: Driver, P: TokenSet<D::TokenValue>> View<D> for Theme<D, P, ChildBuilder<D>> {
  fn build(self) -> NodeId {
    let driver = D::default();
    let scope = Scene::<D>::with(|scene| scene.create(NodeKind::Scope, driver.create_scope()));
    let owner = Owner::current().child();
    let child = self.child;
    let palette = self.palette;

    owner.enter(|| {
      let memo = match palette {
        Palette::Fixed(palette) => Memo::new(move |_| palette.clone()),
        Palette::Derived(palette) => Memo::new(move |_| palette()),
      };

      provide_context(memo);

      Effect::render(move |_: Option<()>| {
        let definitions = memo.with(Definitions::of);

        Scene::<D>::with(|scene| scene.define_tokens(scope, definitions));
      });

      let child = child.build();

      Scene::<D>::with(|scene| scene.attach(scope, child, None));
    });

    scope
  }
}

/// A token set read from the theme in force on driver `D`, its default standing in
/// and defined at the scene's root where no theme provides it.
///
/// A theme provides `Memo<Self>` through context. On a miss the set's default is
/// returned and its definitions are laid over the root definitions of `D`'s scene,
/// once per set, so a consumer below no theme still resolves the tokens. Core's
/// [`Themed`](skene_core::next::Themed) reads the context alone and defines nothing: a
/// root definition lives in one driver's scene, so the trait registering it is bound
/// on the driver. Every [`TokenSet`] with a default has it.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a token set with a default on this driver",
  label = "a palette read without a theme is a `TokenSet` for the driver's token value that implements `Default`"
)]
pub trait ThemedIn<D: Driver>: TokenSet<D::TokenValue> + Default {
  /// The set the nearest theme provides, or the default without one.
  fn current() -> Self;

  /// The nearest theme's set as a memo, or a memo of the default without one.
  fn watch() -> Memo<Self>;
}

#[diagnostic::do_not_recommend]
impl<D: Driver, P: TokenSet<D::TokenValue> + Default> ThemedIn<D> for P {
  #[inline(always)]
  fn current() -> Self {
    use_context::<Memo<Self>>().map_or_else(
      || {
        define_root::<D, Self>();

        Self::default()
      },
      |memo| memo.get(),
    )
  }

  #[inline(always)]
  fn watch() -> Memo<Self> {
    use_context::<Memo<Self>>().unwrap_or_else(|| {
      define_root::<D, Self>();

      Memo::new(|_| Self::default())
    })
  }
}

/// Lays `P`'s default definitions over the root of `D`'s scene, once per set.
fn define_root<D: Driver, P: TokenSet<D::TokenValue> + Default>() {
  Scene::<D>::with(|scene| {
    scene.define_root_tokens_once(TypeId::of::<P>(), || Definitions::of(&P::default()));
  });
}
